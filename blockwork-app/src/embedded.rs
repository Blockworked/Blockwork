//! In-process editor connection. The application owns the runtime; an
//! activity/window owns only a subscription, so closing it cannot stop runs.

use crate::{Backend, Event, host::HostConfig};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;

pub struct EmbeddedRuntime {
    runtime: tokio::runtime::Runtime,
    backend: Backend,
}

impl EmbeddedRuntime {
    /// Call once, before anything else accesses configuration. `storage_root`
    /// must be the host's private files directory, not a document-provider URI.
    pub fn start(storage_root: PathBuf, host: HostConfig) -> Result<Self, String> {
        std::fs::create_dir_all(&storage_root).map_err(|e| e.to_string())?;
        blockwork_core::config::set_config_dir_override(storage_root);
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let backend = Backend::start_with_host(runtime.handle().clone(), host);
        Ok(Self { runtime, backend })
    }

    pub fn backend(&self) -> &Backend {
        &self.backend
    }

    pub fn call(&self, command: &str, args: Value) -> Result<Value, String> {
        self.runtime.block_on(self.backend.dispatch(command, args))
    }

    pub fn attach(&self) -> Result<EditorSession, String> {
        // Subscribe before the snapshot so no update can disappear between
        // fetching the initial state and starting to receive events.
        let events = self.backend.subscribe();
        let snapshot = self.backend.state_json()?;
        Ok(EditorSession {
            backend: self.backend.clone(),
            events,
            snapshot: Some(snapshot.into()),
        })
    }
}

pub struct EditorSession {
    backend: Backend,
    events: broadcast::Receiver<Event>,
    snapshot: Option<Arc<str>>,
}

impl EditorSession {
    /// Nonblocking poll for a native event loop. A slow editor gets a fresh
    /// snapshot and trace rather than retaining stale state after queue loss.
    pub fn poll(&mut self) -> Result<Option<Event>, String> {
        if let Some(snapshot) = self.snapshot.take() {
            return Ok(Some(Event::State(snapshot)));
        }
        match self.events.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(broadcast::error::TryRecvError::Empty) => Ok(None),
            Err(broadcast::error::TryRecvError::Lagged(_)) => {
                self.snapshot = Some(self.backend.state_json()?.into());
                Ok(Some(Event::Trace(
                    serde_json::to_value(blockwork_core::macros::trace::snapshot())
                        .map_err(|e| e.to_string())?,
                )))
            }
            Err(broadcast::error::TryRecvError::Closed) => {
                Err("embedded runtime disconnected".into())
            }
        }
    }
}
