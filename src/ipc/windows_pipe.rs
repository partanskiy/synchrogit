use std::path::Path;
use tokio::net::windows::named_pipe::{
    ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
};
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::{
    Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW, SECURITY_ATTRIBUTES,
};

pub fn create(path: &Path, first: bool) -> std::io::Result<NamedPipeServer> {
    // Only the owner may open this control endpoint. Reject remote clients too.
    let sddl: Vec<u16> = "D:P(A;;GA;;;OW)\0".encode_utf16().collect();
    let mut descriptor = std::ptr::null_mut();
    // SAFETY: valid nul-terminated SDDL and writable output pointer. Windows
    // allocates the descriptor; we release it after CreateNamedPipe copies it.
    unsafe {
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            std::ptr::null_mut(),
        ) == 0
        {
            return Err(std::io::Error::last_os_error());
        }
        let mut attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        let result = ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .create_with_security_attributes_raw(
                path,
                (&mut attributes as *mut SECURITY_ATTRIBUTES).cast(),
            );
        LocalFree(descriptor);
        result
    }
}

pub async fn connect(path: &Path) -> std::io::Result<NamedPipeClient> {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match ClientOptions::new().open(path) {
            Ok(client) => return Ok(client),
            Err(error)
                if error.raw_os_error() == Some(231) && tokio::time::Instant::now() < deadline =>
            {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
            Err(error) => return Err(error),
        }
    }
}
