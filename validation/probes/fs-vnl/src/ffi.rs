// SPDX-License-Identifier: Apache-2.0
//! The shim's entry points (`shim.cxx`), loaded with the dynamic loader, and the capture of what
//! the original writes on its standard output and error.

use std::ffi::{CString, c_char, c_int, c_long, c_void};
use std::io::{Read, Seek, SeekFrom};
use std::os::fd::AsRawFd;

pub type Cost = unsafe extern "C" fn(*mut c_void, *const f64, c_int) -> f64;

unsafe extern "C" {
    fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dup(fd: c_int) -> c_int;
    fn dup2(oldfd: c_int, newfd: c_int) -> c_int;
    fn close(fd: c_int) -> c_int;
    fn ftruncate(fd: c_int, length: i64) -> c_int;
}

macro_rules! lib {
    ($($name:ident : $sym:literal => $ty:ty,)*) => {
        pub struct Lib { $(pub $name: $ty,)* }
        pub fn load(path: &str) -> Lib {
            let c = CString::new(path).unwrap();
            // SAFETY: the dynamic loader with a NUL-terminated name (RTLD_NOW).
            let h = unsafe { dlopen(c.as_ptr(), 2) };
            assert!(!h.is_null(), "dlopen({path})");
            let sym = |name: &str| {
                let c = CString::new(name).unwrap();
                // SAFETY: a symbol lookup in the library opened above.
                let f = unsafe { dlsym(h, c.as_ptr()) };
                assert!(!f.is_null(), "dlsym({name})");
                f
            };
            // SAFETY: each symbol is the shim.cxx function with this signature.
            unsafe { Lib { $($name: std::mem::transmute::<*mut c_void, $ty>(sym($sym)),)* } }
        }
    };
}

lib! {
    flush: "fv_flush" => unsafe extern "C" fn(),
    snprintf: "fv_snprintf" => unsafe extern "C" fn(*mut c_char, c_int, *const c_char, f64) -> c_int,
    det_fixed_f32: "fv_det_fixed_f32" => unsafe extern "C" fn(c_int, *const f32) -> f32,
    det_fixed_f64: "fv_det_fixed_f64" => unsafe extern "C" fn(c_int, *const f64) -> f64,
    inverse_fixed_f32: "fv_inverse_fixed_f32" => unsafe extern "C" fn(c_int, *const f32, *mut f32),
    inverse_fixed_f64: "fv_inverse_fixed_f64" => unsafe extern "C" fn(c_int, *const f64, *mut f64),
    determinant_f32: "fv_determinant_f32" => unsafe extern "C" fn(c_int, *const f32, c_int) -> f32,
    determinant_f64: "fv_determinant_f64" => unsafe extern "C" fn(c_int, *const f64, c_int) -> f64,
    qr_f32: "fv_qr_f32" => unsafe extern "C" fn(c_int, c_int, *const f32, *mut f32, *mut f32) -> f32,
    qr_f64: "fv_qr_f64" => unsafe extern "C" fn(c_int, c_int, *const f64, *mut f64, *mut f64) -> f64,
    qr_inverse_f32: "fv_qr_inverse_f32" => unsafe extern "C" fn(c_int, *const f32, *mut f32),
    qr_inverse_f64: "fv_qr_inverse_f64" => unsafe extern "C" fn(c_int, *const f64, *mut f64),
    qr_solve_f32: "fv_qr_solve_f32" => unsafe extern "C" fn(c_int, c_int, *const f32, *const f32, *mut f32),
    qr_solve_f64: "fv_qr_solve_f64" => unsafe extern "C" fn(c_int, c_int, *const f64, *const f64, *mut f64),
    qr_qtb_f32: "fv_qr_qtb_f32" => unsafe extern "C" fn(c_int, c_int, *const f32, *const f32, *mut f32),
    qr_qtb_f64: "fv_qr_qtb_f64" => unsafe extern "C" fn(c_int, c_int, *const f64, *const f64, *mut f64),
    svd_f32: "fv_svd_f32" => unsafe extern "C" fn(c_int, c_int, *const f32, *mut f32, *mut f32, *mut f32) -> c_int,
    matrix_inverse_f64: "fv_matrix_inverse_f64" => unsafe extern "C" fn(c_int, *const f64, *mut f64),
    ssvdc_f32: "fv_ssvdc_f32" => unsafe extern "C" fn(c_int, c_int, *const f32, *mut f32, *mut f32, *mut f32) -> c_long,
    pythag: "fv_pythag" => unsafe extern "C" fn(f64, f64) -> f64,
    symeig_f32: "fv_symeig_f32" => unsafe extern "C" fn(c_int, *const f32, *mut f32, *mut f32),
    symeig_f64: "fv_symeig_f64" => unsafe extern "C" fn(c_int, *const f64, *mut f64, *mut f64),
    realeig: "fv_realeig" => unsafe extern "C" fn(c_int, *const f64, *mut f64, *mut f64, *mut f64, *mut f64, *mut f64),
    cholesky: "fv_cholesky" => unsafe extern "C" fn(c_int, *const f64, c_int, *mut f64, *mut f64, *mut f64, c_int, *const f64, *mut f64, *mut f64, *mut f64) -> c_long,
    bracket: "fv_bracket" => unsafe extern "C" fn(Cost, *mut c_void, f64, f64, *mut f64),
    brent_minimizer: "fv_brent_minimizer" => unsafe extern "C" fn(Cost, *mut c_void, c_int, f64, *const f64, *mut f64) -> f64,
    vnl_brent_powell_step: "fv_vnl_brent_powell_step" => unsafe extern "C" fn(Cost, *mut c_void, f64, *mut f64),
    vnl_brent_minimize: "fv_vnl_brent_minimize" => unsafe extern "C" fn(Cost, *mut c_void, f64) -> f64,
    powell: "fv_powell" => unsafe extern "C" fn(Cost, *mut c_void, c_int, *mut f64, f64, f64, f64, c_int, c_int, *mut c_int, *mut c_long, *mut f64, *mut f64) -> c_int,
    lbfgs_new: "fv_lbfgs_new" => unsafe extern "C" fn() -> *mut c_void,
    lbfgs_free: "fv_lbfgs_free" => unsafe extern "C" fn(*mut c_void),
    lbfgs_set: "fv_lbfgs_set" => unsafe extern "C" fn(*mut c_void, f64, f64, c_long),
    lbfgs_state: "fv_lbfgs_state" => unsafe extern "C" fn(*mut c_void, *mut c_long, *mut f64),
    lbfgs: "fv_lbfgs" => unsafe extern "C" fn(c_long, c_long, *mut f64, f64, *mut f64, c_long, *mut f64, *mut c_long, f64, f64, *mut f64, *mut c_long, *mut c_void),
    fft: "fv_fft" => unsafe extern "C" fn(c_int, c_int, *mut f64),
}

/// Captures what a call writes to file descriptors 1 and 2 (the C library's and libstdc++'s
/// streams, flushed by `fv_flush`): two temporary files swapped in for the call.
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
            out: open("fs-vnl-stdout.txt"),
            err: open("fs-vnl-stderr.txt"),
        }
    }

    /// Runs `f` with standard output and error captured; returns `(f's value, stdout, stderr)`.
    pub fn run<R>(&mut self, lib: &Lib, f: impl FnOnce() -> R) -> (R, String, String) {
        use std::io::Write;
        let _ = std::io::stdout().flush();
        let _ = std::io::stderr().flush();
        // SAFETY: plain descriptor operations on descriptors this process owns.
        let r = unsafe {
            let (s1, s2) = (dup(1), dup(2));
            dup2(self.out.as_raw_fd(), 1);
            dup2(self.err.as_raw_fd(), 2);
            let r = f();
            (lib.flush)();
            dup2(s1, 1);
            dup2(s2, 2);
            close(s1);
            close(s2);
            r
        };
        let take = |file: &mut std::fs::File| {
            let mut s = String::new();
            file.seek(SeekFrom::Start(0)).unwrap();
            file.read_to_string(&mut s).unwrap();
            // SAFETY: truncating a file this process opened.
            unsafe { ftruncate(file.as_raw_fd(), 0) };
            file.seek(SeekFrom::Start(0)).unwrap();
            s
        };
        let o = take(&mut self.out);
        let e = take(&mut self.err);
        (r, o, e)
    }
}
