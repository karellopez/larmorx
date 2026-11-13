// SPDX-License-Identifier: Apache-2.0
//! The shims' entry points (`shim_common.h`), loaded with the dynamic loader, and the capture of
//! what the originals write on their standard output and error.

use std::ffi::{CString, c_char, c_int, c_void};
use std::io::{Read, Seek, SeekFrom};
use std::os::fd::AsRawFd;

unsafe extern "C" {
    fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dup(fd: c_int) -> c_int;
    fn dup2(oldfd: c_int, newfd: c_int) -> c_int;
    fn close(fd: c_int) -> c_int;
    fn ftruncate(fd: c_int, length: i64) -> c_int;
    fn fflush(stream: *mut c_void) -> c_int;
}

macro_rules! lib {
    ($($name:ident : $sym:literal => $ty:ty,)*) => {
        pub struct Lib { $(pub $name: Option<$ty>,)* }
        pub fn load(path: &str) -> Lib {
            let c = CString::new(path).unwrap();
            // SAFETY: the dynamic loader with a NUL-terminated name (RTLD_NOW | RTLD_LOCAL).
            let h = unsafe { dlopen(c.as_ptr(), 2) };
            assert!(!h.is_null(), "dlopen({path})");
            let sym = |name: &str| {
                let c = CString::new(name).unwrap();
                // SAFETY: a symbol lookup in the library opened above.
                unsafe { dlsym(h, c.as_ptr()) }
            };
            // SAFETY: each symbol is the shim function with this signature (absent: None).
            unsafe {
                Lib { $($name: {
                    let p = sym($sym);
                    if p.is_null() { None } else { Some(std::mem::transmute::<*mut c_void, $ty>(p)) }
                },)* }
            }
        }
    };
}

pub type Buf = unsafe extern "C" fn(*const c_char, *mut u8, c_int, *mut c_int) -> c_int;

lib! {
    input_transform_file: "fm_input_transform_file" => Buf,
    concat_files: "fm_concat_files" => unsafe extern "C" fn(*const c_char, *const c_char, c_int, c_int, *mut u8, c_int, *mut c_int) -> c_int,
    compute_transform_inverse: "fm_compute_transform_inverse" => unsafe extern "C" fn(*const f64, *mut f64) -> c_int,
    concat_transforms: "fm_concat_transforms" => unsafe extern "C" fn(*const f64, *const f64, *mut f64),
    transform_point: "fm_transform_point" => unsafe extern "C" fn(*const f64, f64, f64, f64, *mut f64),
    expand_filename: "fm_expand_filename" => unsafe extern "C" fn(*const c_char, *mut c_char, c_int) -> c_int,
    rewrite: "fm_rewrite" => unsafe extern "C" fn(*const c_char, *const c_char, *const c_char) -> c_int,
    general_point: "fm_general_point" => unsafe extern "C" fn(*const c_char, c_int, f64, f64, f64, *mut f64) -> c_int,
    is_nifti_file: "fm_is_nifti_file" => unsafe extern "C" fn(*const c_char) -> c_int,
    nifti_names: "fm_nifti_names" => unsafe extern "C" fn(*const c_char, *mut c_char, c_int) -> c_int,
    znz_script: "fm_znz_script" => unsafe extern "C" fn(*const c_char, *const c_char, c_int, *const u8, c_int, *mut u8, c_int, *mut c_int) -> c_int,
}

/// Captures what a call writes to file descriptors 1 and 2: two temporary files swapped in for
/// the call (the C streams are flushed by the shims before they return, and here after).
pub struct Capture {
    out: std::fs::File,
    err: std::fs::File,
}

impl Capture {
    pub fn new(dir: &std::path::Path) -> Capture {
        let open = |name: &str| {
            std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(dir.join(name))
                .expect("capture file")
        };
        Capture {
            out: open("capture-stdout.txt"),
            err: open("capture-stderr.txt"),
        }
    }

    /// Runs `f` with standard output and error captured: `(f's value, stdout, stderr)`.
    pub fn run<R>(&mut self, f: impl FnOnce() -> R) -> (R, String, String) {
        use std::io::Write;
        let _ = std::io::stdout().flush();
        let _ = std::io::stderr().flush();
        // SAFETY: plain descriptor operations on descriptors this process owns; fflush(NULL)
        // flushes every C stream.
        let r = unsafe {
            let (s1, s2) = (dup(1), dup(2));
            dup2(self.out.as_raw_fd(), 1);
            dup2(self.err.as_raw_fd(), 2);
            let r = f();
            fflush(std::ptr::null_mut());
            dup2(s1, 1);
            dup2(s2, 2);
            close(s1);
            close(s2);
            r
        };
        let take = |file: &mut std::fs::File| {
            let mut s = Vec::new();
            file.seek(SeekFrom::Start(0)).unwrap();
            file.read_to_end(&mut s).unwrap();
            // SAFETY: truncating a file this process opened.
            unsafe { ftruncate(file.as_raw_fd(), 0) };
            file.seek(SeekFrom::Start(0)).unwrap();
            String::from_utf8_lossy(&s).into_owned()
        };
        let o = take(&mut self.out);
        let e = take(&mut self.err);
        (r, o, e)
    }
}

pub fn cstr(b: &[u8]) -> CString {
    CString::new(b.iter().copied().filter(|&c| c != 0).collect::<Vec<u8>>()).unwrap()
}
