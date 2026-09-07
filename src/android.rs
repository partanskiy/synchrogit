use crate::mobile::{Engine, Request};
use jni::{
    JNIEnv,
    objects::{JClass, JString},
    sys::jstring,
};
use serde_json::json;
use std::sync::{Mutex, OnceLock};

static CONFIGURED: OnceLock<std::result::Result<(), String>> = OnceLock::new();
static ENGINE: OnceLock<Mutex<Engine>> = OnceLock::new();

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_synchrogit_app_NativeBridge_call(
    mut env: JNIEnv,
    _class: JClass,
    input: JString,
) -> jstring {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || -> crate::Result<serde_json::Value> {
            if !matches!(CONFIGURED.get(), Some(Ok(()))) {
                return Err(crate::SynchrogitError::Other(
                    "Android trust store is not initialized".into(),
                ));
            }
            let text: String = env
                .get_string(&input)
                .map_err(|e| crate::SynchrogitError::Other(e.to_string()))?
                .into();
            let request: Request = serde_json::from_str(&text)?;
            if ENGINE.get().is_none() {
                let engine = Engine::new()?;
                let _ = ENGINE.set(Mutex::new(engine));
            }
            ENGINE
                .get()
                .expect("engine initialized")
                .lock()
                .map_err(|_| {
                    crate::SynchrogitError::Other(
                        "native engine lock is poisoned; restart the app".into(),
                    )
                })?
                .call(request)
        },
    ));
    let response = match result {
        Ok(Ok(value)) => json!({"ok": true, "result": value}),
        Ok(Err(error)) => json!({"ok": false, "error": error.to_string()}),
        Err(_) => json!({"ok": false, "error": "native engine panicked; restart the app"}),
    }
    .to_string();
    match env.new_string(response) {
        Ok(text) => text.into_raw(),
        Err(_) => std::ptr::null_mut(), // JVM retains the pending allocation exception.
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_synchrogit_app_NativeBridge_configure(
    mut env: JNIEnv,
    _class: JClass,
    ca_file: JString,
) {
    let result = CONFIGURED.get_or_init(|| {
        let path: String = env.get_string(&ca_file).map_err(|e| e.to_string())?.into();
        // SAFETY: configure is called before the first native request; call()
        // rejects requests until this OnceLock is complete. No libgit2 workers
        // can exist yet, and these global options are never changed afterward.
        unsafe {
            git2::opts::set_ssl_cert_file(path).map_err(|e| e.to_string())?;
            git2::opts::set_server_connect_timeout_in_milliseconds(60_000)
                .map_err(|e| e.to_string())?;
            git2::opts::set_server_timeout_in_milliseconds(60_000).map_err(|e| e.to_string())?;
        }
        Ok(())
    });
    if let Err(message) = result {
        let _ = env.throw_new("java/lang/IllegalStateException", message);
    }
}
