use std::os::windows::io::AsRawHandle;

use windows::Win32::{
    Foundation::HANDLE,
    Security::{
        ACL,
        Authorization::{GetSecurityInfo, SE_KERNEL_OBJECT},
        DACL_SECURITY_INFORMATION,
    },
};

use super::create_named_pipe_server;

#[tokio::test]
async fn windows_pipe_creation_applies_current_user_security_descriptor() {
    let pipe_name = format!(
        r"\\.\pipe\clay-test-security-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    let pipe = create_named_pipe_server(&pipe_name)
        .expect("pipe creation with current-user-only security descriptor should succeed");

    // Read back the DACL and verify the pipe no longer uses the default
    // descriptor (which has multiple ACEs for LocalSystem/Admins/Owner/
    // Everyone/Anonymous). A single-ACE DACL confirms we installed a
    // custom, restricted descriptor.
    unsafe {
        let mut dacl: *mut ACL = std::ptr::null_mut();
        GetSecurityInfo(
            HANDLE(pipe.as_raw_handle()),
            SE_KERNEL_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(&mut dacl),
            None,
            None,
        )
        .ok()
        .expect("GetSecurityInfo should succeed on the pipe we created");

        assert!(
            !dacl.is_null(),
            "pipe must have a DACL after custom descriptor is applied"
        );
        assert_eq!(
            (*dacl).AceCount,
            1,
            "current-user-only DACL must have exactly one ACE"
        );

        // LocalFree expects the descriptor returned by GetSecurityInfo, but
        // because we passed null for ppSecurityDescriptor we only need to
        // free the DACL if GetSecurityInfo allocated it. In practice
        // GetSecurityInfo returns a self-relative descriptor whose DACL is
        // internal; passing the handle-owned descriptor pointer to LocalFree
        // is undefined. We therefore do not free `dacl` here — it is valid
        // only while the pipe handle remains open.
    }

    drop(pipe);
}
