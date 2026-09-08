use crate::mobile::{Engine, Request};
use jni::{
    JNIEnv,
    objects::{JByteArray, JClass, JObjectArray, JString},
    sys::jstring,
};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

static CONFIGURED: OnceLock<std::result::Result<(), String>> = OnceLock::new();
static ENGINE: OnceLock<Mutex<Engine>> = OnceLock::new();
static GIT_CONFIG: OnceLock<PathBuf> = OnceLock::new();

/// Android shared storage does not report files as owned by the app's UID.
/// Allow only repositories explicitly selected by the user, in the app's
/// private Git configuration. Keep libgit2 ownership validation enabled.
pub(crate) fn trust_repository(path: &Path) -> crate::Result<()> {
    let canonical = path.canonicalize()?;
    let path = canonical
        .to_str()
        .ok_or_else(|| crate::SynchrogitError::Other("repository path is not UTF-8".into()))?;
    let config_path = GIT_CONFIG.get().ok_or_else(|| {
        crate::SynchrogitError::Other("Android Git configuration is not initialized".into())
    })?;
    let update = || -> std::result::Result<(), git2::Error> {
        let mut config = git2::Config::open(config_path)?;
        let mut present = false;
        config
            .entries(Some("^safe\\.directory$"))?
            .for_each(|entry| {
                if entry.value().ok() == Some(path) {
                    present = true;
                }
            })?;
        if !present {
            config.set_multivar("safe.directory", "^$", path)?;
        }
        Ok(())
    };
    update().map_err(|error| crate::SynchrogitError::Other(error.to_string()))
}

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
    certificates: JObjectArray,
    git_config_directory: JString,
) {
    let result = CONFIGURED.get_or_init(|| {
        let directory = PathBuf::from(String::from(
            env.get_string(&git_config_directory)
                .map_err(|e| e.to_string())?,
        ));
        let config_path = directory.join(".gitconfig");
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&config_path)
            .map_err(|e| e.to_string())?;
        let count = env
            .get_array_length(&certificates)
            .map_err(|e| e.to_string())?;
        if count == 0 {
            return Err("Android trust store is empty".into());
        }
        // SAFETY: configure is called before the first native request; call()
        // rejects requests until this OnceLock is complete. No libgit2 workers
        // can exist yet, and these global options are never changed afterward.
        unsafe {
            for level in [
                git2::ConfigLevel::Global,
                git2::ConfigLevel::XDG,
                git2::ConfigLevel::System,
            ] {
                git2::opts::set_search_path(level, &directory).map_err(|e| e.to_string())?;
            }
            git2::opts::set_server_connect_timeout_in_milliseconds(60_000)
                .map_err(|e| e.to_string())?;
            git2::opts::set_server_timeout_in_milliseconds(60_000).map_err(|e| e.to_string())?;
        }
        GIT_CONFIG
            .set(config_path)
            .map_err(|_| "Android Git configuration already initialized")?;
        // openssl-src builds Android with no-stdio; loading a PEM file through
        // OpenSSL always fails. Parse the OS trust anchors from DER in memory.
        for index in 0..count {
            let array = JByteArray::from(
                env.get_object_array_element(&certificates, index)
                    .map_err(|e| e.to_string())?,
            );
            let der = env.convert_byte_array(&array).map_err(|e| e.to_string())?;
            let length = der
                .len()
                .try_into()
                .map_err(|_| "certificate is too large")?;
            let mut pointer = der.as_ptr();
            // SAFETY: DER remains alive during parsing. d2i_X509 creates an
            // owned certificate. libgit2's store takes its own reference; free
            // ours after adding it. Initialization is serialized above.
            unsafe {
                let cert = openssl_sys::d2i_X509(std::ptr::null_mut(), &mut pointer, length);
                if cert.is_null() {
                    return Err(format!("invalid Android trust anchor {index}"));
                }
                let result = libgit2_sys::git_libgit2_opts(
                    libgit2_sys::GIT_OPT_ADD_SSL_X509_CERT as libc::c_int,
                    cert,
                );
                openssl_sys::X509_free(cert);
                if result < 0 {
                    return Err(format!("cannot install Android trust anchor {index}"));
                }
            }
            env.delete_local_ref(array).map_err(|e| e.to_string())?;
        }
        Ok(())
    });
    if let Err(message) = result {
        let _ = env.throw_new("java/lang/IllegalStateException", message);
    }
}
