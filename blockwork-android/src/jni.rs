use ::jni::{
    JNIEnv,
    objects::{JClass, JIntArray, JString},
    sys::{jboolean, jint, jlong, jstring},
};

fn string(env: &mut JNIEnv, value: JString) -> Result<String, String> {
    env.get_string(&value)
        .map(Into::into)
        .map_err(|e| e.to_string())
}

fn output(env: &mut JNIEnv, value: String) -> jstring {
    match env.new_string(value) {
        Ok(value) => value.into_raw(),
        Err(_) => {
            let _ = env.throw_new(
                "java/lang/OutOfMemoryError",
                "Blockwork reply allocation failed",
            );
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_blockworked_blockwork_NativeRuntime_displayChanged(
    mut env: JNIEnv,
    _: JClass,
    width: jint,
    height: jint,
) -> jstring {
    output(
        &mut env,
        super::envelope(|| {
            super::automation::display_changed(width.max(0) as u32, height.max(0) as u32)
        }),
    )
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_blockworked_blockwork_NativeRuntime_screenshotReply(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    width: jint,
    height: jint,
    argb: JIntArray,
) -> jstring {
    let result = super::envelope(|| {
        let length = env.get_array_length(&argb).map_err(|e| e.to_string())?;
        if width <= 0
            || height <= 0
            || i64::from(width) * i64::from(height) != i64::from(length)
            || length > 32_000_000
        {
            return super::automation::screenshot_reply(id as u64, 0, 0, &[]);
        }
        let mut pixels = vec![0; length as usize];
        env.get_int_array_region(&argb, 0, &mut pixels)
            .map_err(|e| e.to_string())?;
        super::automation::screenshot_reply(id as u64, width as u32, height as u32, &pixels)
    });
    output(&mut env, result)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_blockworked_blockwork_NativeRuntime_start(
    mut env: JNIEnv,
    _: JClass,
    root: JString,
) -> jstring {
    let result = super::envelope(|| super::start(string(&mut env, root)?.into()));
    output(&mut env, result)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_blockworked_blockwork_NativeRuntime_call(
    mut env: JNIEnv,
    _: JClass,
    command: JString,
    args: JString,
) -> jstring {
    let result =
        super::envelope(|| super::call(&string(&mut env, command)?, &string(&mut env, args)?));
    output(&mut env, result)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_blockworked_blockwork_NativeRuntime_pendingPrompt(
    mut env: JNIEnv,
    _: JClass,
) -> jstring {
    output(&mut env, super::envelope(super::prompts::pending))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_blockworked_blockwork_NativeRuntime_answerPrompt(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    reply: JString,
) -> jstring {
    let result = super::envelope(|| {
        super::prompts::answer(
            id as u64,
            serde_json::from_str(&string(&mut env, reply)?).map_err(|e| e.to_string())?,
        )
    });
    output(&mut env, result)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_blockworked_blockwork_NativeRuntime_serviceReady(
    mut env: JNIEnv,
    _: JClass,
    ready: jboolean,
) -> jstring {
    output(
        &mut env,
        super::envelope(|| super::automation::ready(ready != 0)),
    )
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_blockworked_blockwork_NativeRuntime_takeActions(
    mut env: JNIEnv,
    _: JClass,
) -> jstring {
    output(&mut env, super::envelope(super::automation::take))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_blockworked_blockwork_NativeRuntime_actionReply(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    reply: JString,
) -> jstring {
    let result = super::envelope(|| {
        super::automation::reply(
            id as u64,
            serde_json::from_str(&string(&mut env, reply)?).map_err(|e| e.to_string())?,
        )
    });
    output(&mut env, result)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_blockworked_blockwork_NativeRuntime_runningCount(
    _: JNIEnv,
    _: JClass,
) -> jint {
    std::panic::catch_unwind(|| {
        blockwork_core::macros::run_registry::count().min(i32::MAX as usize) as jint
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_blockworked_blockwork_NativeRuntime_foregroundApp(
    mut env: JNIEnv,
    _: JClass,
    package: JString,
) -> jstring {
    let result = super::envelope(|| {
        let package = string(&mut env, package)?;
        super::process()?
            .runtime
            .backend()
            .observe_foreground_app(if package.is_empty() {
                None
            } else {
                Some(&package)
            })
            .map(|count| serde_json::json!(count))
    });
    output(&mut env, result)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_blockworked_blockwork_NativeRuntime_alarmPlan(
    mut env: JNIEnv,
    _: JClass,
    now: jlong,
    zone: JString,
    exact: jboolean,
) -> jstring {
    let result = super::envelope(|| {
        let backend = super::process()?.runtime.backend();
        match backend.android_alarm_plan(now, &string(&mut env, zone)?, exact != 0) {
            Ok(plan) => serde_json::to_value(plan).map_err(|e| e.to_string()),
            Err(error) => {
                backend.update_host_services(vec![(
                    "schedules".into(),
                    blockwork_app::host::Capability::unavailable(true, error.clone()),
                )])?;
                Err(error)
            }
        }
    });
    output(&mut env, result)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_blockworked_blockwork_NativeRuntime_deliverAlarm(
    mut env: JNIEnv,
    _: JClass,
    token: JString,
    now: jlong,
    zone: JString,
    exact: jboolean,
) -> jstring {
    let result = super::envelope(|| {
        let backend = super::process()?.runtime.backend();
        backend.update_host_services(vec![(
            "exact_alarms".into(),
            if exact != 0 {
                blockwork_app::host::Capability::ready()
            } else {
                blockwork_app::host::Capability::unavailable(
                    true,
                    "Allow exact alarms in Android settings",
                )
            },
        )])?;
        backend
            .deliver_android_alarm(
                &string(&mut env, token)?,
                now,
                &string(&mut env, zone)?,
                std::sync::Arc::new(super::automation::schedule_ui_is_ready),
            )
            .map(serde_json::Value::Bool)
    });
    output(&mut env, result)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_blockworked_blockwork_NativeRuntime_scheduleUiReady(
    _: JNIEnv,
    _: JClass,
    ready: jboolean,
) {
    super::automation::schedule_ui_ready(ready != 0);
}
