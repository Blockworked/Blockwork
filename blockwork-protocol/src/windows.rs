use super::{Connection, NAME};
use std::io;
use std::os::windows::io::AsRawHandle;
use std::ptr::null_mut;
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions};
use tokio::sync::Mutex;
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, LocalFree},
    Security::{
        Authorization::*, GetTokenInformation, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
        TokenUser,
    },
    System::{
        Pipes::{GetNamedPipeClientProcessId, GetNamedPipeServerProcessId},
        Threading::*,
    },
};

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
struct LocalAllocation(*mut core::ffi::c_void);
impl Drop for LocalAllocation {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}

fn process_sid(process: HANDLE) -> io::Result<String> {
    unsafe {
        let mut token = null_mut();
        if OpenProcessToken(process, TOKEN_QUERY, &mut token) == 0 {
            return Err(io::Error::last_os_error());
        }
        let token = Handle(token);
        let mut size = 0;
        GetTokenInformation(token.0, TokenUser, null_mut(), 0, &mut size);
        if size == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut buffer = vec![0usize; (size as usize).div_ceil(size_of::<usize>())];
        if GetTokenInformation(
            token.0,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            size,
            &mut size,
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
        let mut text = null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut text) == 0 {
            return Err(io::Error::last_os_error());
        }
        let _text = LocalAllocation(text.cast());
        let mut len = 0;
        while *text.add(len) != 0 {
            len += 1;
        }
        Ok(String::from_utf16_lossy(std::slice::from_raw_parts(
            text, len,
        )))
    }
}

fn current_sid() -> io::Result<String> {
    process_sid(unsafe { GetCurrentProcess() })
}

pub fn pipe_name() -> io::Result<String> {
    Ok(format!(r"\\.\pipe\blockwork-{NAME}-{}", current_sid()?))
}

fn check_peer(pipe: HANDLE, server: bool) -> io::Result<()> {
    unsafe {
        let mut pid = 0;
        let ok = if server {
            GetNamedPipeServerProcessId(pipe, &mut pid)
        } else {
            GetNamedPipeClientProcessId(pipe, &mut pid)
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return Err(io::Error::last_os_error());
        }
        let process = Handle(process);
        if process_sid(process.0)? != current_sid()? {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "untrusted IPC peer",
            ));
        }
        Ok(())
    }
}

pub async fn connect() -> io::Result<Connection> {
    let name = pipe_name()?;
    tokio::time::timeout(super::CONNECT_TIMEOUT, async {
        loop {
            match ClientOptions::new().open(&name) {
                Ok(client) => {
                    check_peer(client.as_raw_handle(), true)?;
                    return Ok(Box::new(client) as Connection);
                }
                Err(e) if e.raw_os_error() == Some(231) => {
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await
                }
                Err(e) => return Err(e),
            }
        }
    })
    .await?
}

fn create(first: bool) -> io::Result<NamedPipeServer> {
    let sid = current_sid()?;
    let sddl: Vec<u16> = format!("O:{sid}D:P(A;;GA;;;{sid})")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    unsafe {
        let mut descriptor = null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION,
            &mut descriptor,
            null_mut(),
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let _descriptor = LocalAllocation(descriptor);
        let mut attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .create_with_security_attributes_raw(
                pipe_name()?,
                (&mut attributes as *mut SECURITY_ATTRIBUTES).cast(),
            )
    }
}

pub struct Listener(Mutex<NamedPipeServer>);
impl Listener {
    pub fn cleanup(&self) {}
    pub async fn bind() -> io::Result<Self> {
        Ok(Self(Mutex::new(create(true)?)))
    }
    pub async fn accept(&self) -> io::Result<Connection> {
        let mut server = self.0.lock().await;
        server.connect().await?;
        if let Err(e) = check_peer(server.as_raw_handle(), false) {
            let _ = server.disconnect();
            return Err(e);
        }
        let next = create(false)?;
        Ok(Box::new(std::mem::replace(&mut *server, next)))
    }
}
