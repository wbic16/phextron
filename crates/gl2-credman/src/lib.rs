//! G-L2 — thin Windows Credential Manager binding.
//!
//! Direct FFI ONLY: hand-declared `extern "system"` bindings to
//! advapi32 (`CredWriteW`, `CredReadW`, `CredDeleteW`, `CredFree`)
//! and shell32 (`ShellExecuteW`). No `windows` crate, no keyring-class
//! abstraction (SPEC §6.2 strikes them). Zero dependencies.
//!
//! On non-Windows platforms the same API compiles and returns a typed
//! [`Error::Unsupported`], so platform-independent tests run anywhere.

use std::fmt;

/// Generic-credential blob limit enforced by Windows Credential Manager.
pub const MAX_BLOB_BYTES: usize = 2560; // CRED_MAX_CREDENTIAL_BLOB_SIZE (5 * 512)

#[derive(Debug)]
pub enum Error {
    /// Blob exceeds the generic-credential limit.
    BlobTooLarge { len: usize, max: usize },
    /// Target name is empty or otherwise invalid.
    InvalidTarget,
    /// Platform is not Windows.
    Unsupported,
    /// Win32 failure, carrying the OS error code.
    Os(u32),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BlobTooLarge { len, max } => {
                write!(f, "credential blob too large: {len} bytes (max {max})")
            }
            Error::InvalidTarget => write!(f, "invalid credential target name"),
            Error::Unsupported => write!(f, "Windows Credential Manager is not available on this platform"),
            Error::Os(code) => write!(f, "Win32 credential error (code {code})"),
        }
    }
}
impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

pub fn validate_target(target: &str) -> Result<()> {
    if target.is_empty() || target.len() > 32767 {
        return Err(Error::InvalidTarget);
    }
    Ok(())
}

pub fn validate_blob(blob: &[u8]) -> Result<()> {
    if blob.len() > MAX_BLOB_BYTES {
        return Err(Error::BlobTooLarge { len: blob.len(), max: MAX_BLOB_BYTES });
    }
    Ok(())
}

#[cfg(target_os = "windows")]
mod win {
    use super::*;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::null_mut;

    pub const CRED_TYPE_GENERIC: u32 = 1;
    pub const CRED_PERSIST_LOCAL_MACHINE: u32 = 2;
    pub const ERROR_NOT_FOUND: u32 = 1168;

    #[repr(C)]
    pub struct CredentialW {
        pub flags: u32,
        pub cred_type: u32,
        pub target_name: *mut u16,
        pub comment: *mut u16,
        pub last_written: u64, // FILETIME
        pub credential_blob_size: u32,
        pub credential_blob: *mut u8,
        pub persist: u32,
        pub attribute_count: u32,
        pub attributes: *mut core::ffi::c_void,
        pub target_alias: *mut u16,
        pub user_name: *mut u16,
    }

    #[link(name = "advapi32")]
    #[link(name = "shell32")]
    extern "system" {
        fn CredWriteW(credential: *const CredentialW, flags: u32) -> i32;
        fn CredReadW(target_name: *const u16, cred_type: u32, flags: u32, credential: *mut *mut CredentialW) -> i32;
        fn CredDeleteW(target_name: *const u16, cred_type: u32, flags: u32) -> i32;
        fn CredFree(buffer: *mut core::ffi::c_void);
        fn ShellExecuteW(hwnd: *mut core::ffi::c_void, operation: *const u16, file: *const u16, parameters: *const u16, directory: *const u16, show_cmd: i32) -> isize;
    }

    fn wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    fn last_error() -> u32 {
        // GetLastError via kernel32, hand-declared like the rest.
        #[link(name = "kernel32")]
        extern "system" { fn GetLastError() -> u32; }
        unsafe { GetLastError() }
    }

    pub fn store(target: &str, blob: &[u8]) -> Result<()> {
        validate_target(target)?;
        validate_blob(blob)?;
        let mut target_w = wide(target);
        let mut blob_vec = blob.to_vec();
        let cred = CredentialW {
            flags: 0,
            cred_type: CRED_TYPE_GENERIC,
            target_name: target_w.as_mut_ptr(),
            comment: null_mut(),
            last_written: 0,
            credential_blob_size: blob_vec.len() as u32,
            credential_blob: blob_vec.as_mut_ptr(),
            persist: CRED_PERSIST_LOCAL_MACHINE,
            attribute_count: 0,
            attributes: null_mut(),
            target_alias: null_mut(),
            user_name: null_mut(),
        };
        let ok = unsafe { CredWriteW(&cred, 0) };
        if ok == 0 { Err(Error::Os(last_error())) } else { Ok(()) }
    }

    pub fn read(target: &str) -> Result<Option<Vec<u8>>> {
        validate_target(target)?;
        let target_w = wide(target);
        let mut out: *mut CredentialW = null_mut();
        let ok = unsafe { CredReadW(target_w.as_ptr(), CRED_TYPE_GENERIC, 0, &mut out) };
        if ok == 0 {
            let code = last_error();
            if code == ERROR_NOT_FOUND { return Ok(None); }
            return Err(Error::Os(code));
        }
        let data = unsafe {
            let c = &*out;
            let slice = std::slice::from_raw_parts(c.credential_blob, c.credential_blob_size as usize);
            let v = slice.to_vec();
            CredFree(out as *mut core::ffi::c_void);
            v
        };
        Ok(Some(data))
    }

    pub fn delete(target: &str) -> Result<()> {
        validate_target(target)?;
        let target_w = wide(target);
        let ok = unsafe { CredDeleteW(target_w.as_ptr(), CRED_TYPE_GENERIC, 0) };
        if ok == 0 {
            let code = last_error();
            if code == ERROR_NOT_FOUND { return Ok(()); }
            return Err(Error::Os(code));
        }
        Ok(())
    }

    pub fn open_url(url: &str) -> Result<()> {
        let op = wide("open");
        let file = wide(url);
        let rc = unsafe { ShellExecuteW(null_mut(), op.as_ptr(), file.as_ptr(), null_mut(), null_mut(), 1) };
        if rc <= 32 { Err(Error::Os(rc as u32)) } else { Ok(()) }
    }
}

#[cfg(not(target_os = "windows"))]
mod win {
    use super::*;
    pub fn store(target: &str, blob: &[u8]) -> Result<()> {
        validate_target(target)?;
        validate_blob(blob)?;
        Err(Error::Unsupported)
    }
    pub fn read(target: &str) -> Result<Option<Vec<u8>>> {
        validate_target(target)?;
        Err(Error::Unsupported)
    }
    pub fn delete(target: &str) -> Result<()> {
        validate_target(target)?;
        Err(Error::Unsupported)
    }
    pub fn open_url(_url: &str) -> Result<()> {
        Err(Error::Unsupported)
    }
}

pub fn store(target: &str, blob: &[u8]) -> Result<()> { win::store(target, blob) }
pub fn read(target: &str) -> Result<Option<Vec<u8>>> { win::read(target) }
pub fn delete(target: &str) -> Result<()> { win::delete(target) }
pub fn open_url(url: &str) -> Result<()> { win::open_url(url) }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blob_limit_enforced() {
        assert!(validate_blob(&vec![0u8; MAX_BLOB_BYTES]).is_ok());
        match validate_blob(&vec![0u8; MAX_BLOB_BYTES + 1]) {
            Err(Error::BlobTooLarge { len, max }) => {
                assert_eq!(len, MAX_BLOB_BYTES + 1);
                assert_eq!(max, MAX_BLOB_BYTES);
            }
            other => panic!("expected BlobTooLarge, got {other:?}"),
        }
    }

    #[test]
    fn target_validation() {
        assert!(validate_target("phextron/test").is_ok());
        assert!(matches!(validate_target(""), Err(Error::InvalidTarget)));
    }

    #[test]
    fn store_rejects_oversize_before_platform() {
        // Even on non-Windows, the typed limit error wins over Unsupported.
        assert!(matches!(
            store("phextron/test", &vec![0u8; MAX_BLOB_BYTES + 1]),
            Err(Error::BlobTooLarge { .. })
        ));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn non_windows_is_typed_unsupported() {
        assert!(matches!(store("phextron/test", b"x"), Err(Error::Unsupported)));
        assert!(matches!(read("phextron/test"), Err(Error::Unsupported)));
        assert!(matches!(delete("phextron/test"), Err(Error::Unsupported)));
        assert!(matches!(open_url("https://example.com"), Err(Error::Unsupported)));
    }
}
