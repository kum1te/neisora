use std::{
    io,
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::Path,
};
use windows_sys::Win32::{
    Foundation::LocalFree,
    Security::{
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
            SDDL_REVISION_1,
        },
        GetTokenInformation, SetFileSecurityW, TokenUser, DACL_SECURITY_INFORMATION,
        PROTECTED_DACL_SECURITY_INFORMATION, TOKEN_QUERY, TOKEN_USER,
    },
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};
pub fn wide(path: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    path.as_ref().encode_wide().chain(Some(0)).collect()
}
pub fn private_directory(path: &Path) -> io::Result<()> {
    unsafe {
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(io::Error::last_os_error());
        }
        let token = OwnedHandle::from_raw_handle(token);
        let mut length = 0;
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            std::ptr::null_mut(),
            0,
            &mut length,
        );
        // TOKEN_USER contains pointers and must be naturally aligned.
        let mut buffer = vec![0usize; (length as usize).div_ceil(std::mem::size_of::<usize>())];
        if GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            buffer.as_mut_ptr().cast(),
            length,
            &mut length,
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
        let mut sid = std::ptr::null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut sid) == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut n = 0;
        while *sid.add(n) != 0 {
            n += 1;
        }
        let sid_text = String::from_utf16_lossy(std::slice::from_raw_parts(sid, n));
        LocalFree(sid.cast());
        let sddl = wide(format!("D:P(A;OICI;FA;;;{sid_text})(A;OICI;FA;;;SY)"));
        let mut descriptor = std::ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            std::ptr::null_mut(),
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let ok = SetFileSecurityW(
            wide(path).as_ptr(),
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            descriptor,
        );
        let error = io::Error::last_os_error();
        LocalFree(descriptor);
        if ok == 0 {
            Err(error)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_settings_replace_existing_file_and_keep_other_files() {
        let root =
            std::env::temp_dir().join(format!("neisora-windows-private-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        private_directory(&root).unwrap();
        let settings = root.join("settings.json");
        let other = root.join("other");
        std::fs::write(&other, b"original").unwrap();
        crate::api::private_write(&settings, b"first").unwrap();
        crate::api::private_write(&settings, b"second").unwrap();
        assert_eq!(std::fs::read(&settings).unwrap(), b"second");
        assert_eq!(std::fs::read(&other).unwrap(), b"original");
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 2);
        assert!(crate::platform::volume(&root).is_some());
        assert!(crate::platform::process_alive(std::process::id()));
        std::fs::remove_dir_all(root).unwrap();
    }
}
