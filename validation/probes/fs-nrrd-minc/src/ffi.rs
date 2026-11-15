// SPDX-License-Identifier: Apache-2.0
//! The shims' entry points, loaded with the dynamic loader.

use std::ffi::{CString, c_char, c_int, c_void};

unsafe extern "C" {
    fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

type LoadFn = unsafe extern "C" fn(
    *const c_char,
    c_int,
    *mut *mut u8,
    *mut usize,
    *mut *mut u8,
    *mut usize,
    *mut c_int,
) -> c_int;
type FreeFn = unsafe extern "C" fn(*mut c_void);

/// One original library with the shim's `fm_<name>_load` and `fm_free`.
pub struct Lib {
    load: LoadFn,
    free: FreeFn,
}

/// How the original's child process ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// It returned: its dump.
    Returned(String),
    Exit(i32),
    Signal(i32),
    Hang,
}

impl Lib {
    pub fn open(path: &str, entry: &str) -> Lib {
        let c = CString::new(path).unwrap();
        // SAFETY: the dynamic loader with a NUL-terminated name (RTLD_NOW | RTLD_LOCAL).
        let h = unsafe { dlopen(c.as_ptr(), 2) };
        assert!(!h.is_null(), "dlopen({path})");
        let sym = |name: &str| {
            let c = CString::new(name).unwrap();
            // SAFETY: a symbol lookup in the library opened above.
            let p = unsafe { dlsym(h, c.as_ptr()) };
            assert!(!p.is_null(), "{name} in {path}");
            p
        };
        // SAFETY: the shim's functions have these signatures (shim_*.c).
        unsafe {
            Lib {
                load: std::mem::transmute::<*mut c_void, LoadFn>(sym(entry)),
                free: std::mem::transmute::<*mut c_void, FreeFn>(sym("fm_free")),
            }
        }
    }

    /// Runs the original on `name` in a child process: its outcome and standard error.
    pub fn run(&self, name: &[u8], timeout_ms: i32) -> (Outcome, Vec<u8>) {
        let c = CString::new(name.to_vec()).expect("a name without NUL");
        let (mut o, mut ol, mut e, mut el, mut st) = (
            std::ptr::null_mut(),
            0usize,
            std::ptr::null_mut(),
            0usize,
            0 as c_int,
        );
        // SAFETY: the shim fills the pointers with malloc'ed buffers (or NULL) and lengths.
        let r = unsafe {
            (self.load)(
                c.as_ptr(),
                timeout_ms,
                &mut o,
                &mut ol,
                &mut e,
                &mut el,
                &mut st,
            )
        };
        let take = |p: *mut u8, n: usize| -> Vec<u8> {
            if p.is_null() {
                return Vec::new();
            }
            // SAFETY: `n` bytes the shim wrote at `p`, freed once here.
            let v = unsafe { std::slice::from_raw_parts(p, n).to_vec() };
            unsafe { (self.free)(p as *mut c_void) };
            v
        };
        let out = take(o, ol);
        let err = take(e, el);
        let oc = match r {
            0 => Outcome::Returned(String::from_utf8_lossy(&out).into_owned()),
            1 => Outcome::Exit(st),
            2 => Outcome::Signal(st),
            3 => Outcome::Hang,
            _ => panic!("the shim failed ({r})"),
        };
        (oc, err)
    }
}
