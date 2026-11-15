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

type MincLoadFn = unsafe extern "C" fn(
    *const c_char,
    c_int,
    c_int,
    *mut *mut u8,
    *mut usize,
    *mut *mut u8,
    *mut usize,
    *mut *mut u8,
    *mut usize,
    *mut c_int,
) -> c_int;

/// `WriteCase` of `shim_minc.c` (the same layout).
#[repr(C)]
pub struct WriteCaseC {
    pub n_dims: c_int,
    pub names_null: c_int,
    pub names: [*const c_char; 5],
    pub nc_type: c_int,
    pub signed_flag: c_int,
    pub vmin: f64,
    pub vmax: f64,
    pub set_real: c_int,
    pub rmin: f64,
    pub rmax: f64,
    pub sizes: [c_int; 5],
    pub do_alloc: c_int,
    pub sep: [f64; 5],
    pub n_cos: c_int,
    pub cos_unit: c_int,
    pub cos_axis: [c_int; 5],
    pub cos: [[f64; 3]; 5],
    pub do_translation: c_int,
    pub tvoxel: [f64; 5],
    pub tworld: [f64; 3],
    pub n_values: std::ffi::c_long,
    pub values: *const f64,
    pub fname: *const c_char,
    pub out_type: c_int,
    pub out_signed: c_int,
    pub out_min: f64,
    pub out_max: f64,
    pub history: *const c_char,
    pub set_space: c_int,
    pub space: *const c_char,
}

type MincWriteFn = unsafe extern "C" fn(
    *const WriteCaseC,
    c_int,
    *mut *mut u8,
    *mut usize,
    *mut *mut u8,
    *mut usize,
    *mut *mut u8,
    *mut usize,
    *mut c_int,
) -> c_int;

/// A MINC original with the shim's `fm_minc_load`, `fm_minc_write` and `fm_minc_free`.
pub struct MincLib {
    load: MincLoadFn,
    write: MincWriteFn,
    free: FreeFn,
}

impl MincLib {
    pub fn open(path: &str) -> MincLib {
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
        // SAFETY: the shim's functions have these signatures (shim_minc.c).
        unsafe {
            MincLib {
                load: std::mem::transmute::<*mut c_void, MincLoadFn>(sym("fm_minc_load")),
                write: std::mem::transmute::<*mut c_void, MincWriteFn>(sym("fm_minc_write")),
                free: std::mem::transmute::<*mut c_void, FreeFn>(sym("fm_minc_free")),
            }
        }
    }

    /// Runs the original on `name` in a child process (`cache_threshold` >= 0 sets
    /// `VOLUME_CACHE_THRESHOLD`): its outcome, standard output and standard error.
    pub fn run(
        &self,
        name: &[u8],
        timeout_ms: i32,
        cache_threshold: i32,
    ) -> (Outcome, Vec<u8>, Vec<u8>) {
        let c = CString::new(name.to_vec()).expect("a name without NUL");
        let mut p = [std::ptr::null_mut(); 3];
        let mut l = [0usize; 3];
        let mut st = 0 as c_int;
        // SAFETY: the shim fills the pointers with malloc'ed buffers (or NULL) and lengths.
        let r = unsafe {
            (self.load)(
                c.as_ptr(),
                timeout_ms,
                cache_threshold,
                &mut p[0],
                &mut l[0],
                &mut p[1],
                &mut l[1],
                &mut p[2],
                &mut l[2],
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
        let dump = take(p[0], l[0]);
        let out = take(p[1], l[1]);
        let err = take(p[2], l[2]);
        let oc = match r {
            0 => Outcome::Returned(String::from_utf8_lossy(&dump).into_owned()),
            1 => Outcome::Exit(st),
            2 => Outcome::Signal(st),
            3 => Outcome::Hang,
            _ => panic!("the shim failed ({r})"),
        };
        (oc, out, err)
    }
}

impl MincLib {
    /// Runs the write case in a child process: its outcome, its dump (kept when it exits),
    /// standard output and standard error.
    pub fn run_write(
        &self,
        w: &WriteCaseC,
        timeout_ms: i32,
    ) -> (Outcome, String, Vec<u8>, Vec<u8>) {
        let mut p = [std::ptr::null_mut(); 3];
        let mut l = [0usize; 3];
        let mut st = 0 as c_int;
        // SAFETY: the case's pointers are valid for the call; the shim fills the buffers.
        let r = unsafe {
            (self.write)(
                w, timeout_ms, &mut p[0], &mut l[0], &mut p[1], &mut l[1], &mut p[2], &mut l[2],
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
        let dump = String::from_utf8_lossy(&take(p[0], l[0])).into_owned();
        let out = take(p[1], l[1]);
        let err = take(p[2], l[2]);
        let oc = match r {
            0 => Outcome::Returned(dump.clone()),
            1 => Outcome::Exit(st),
            2 => Outcome::Signal(st),
            3 => Outcome::Hang,
            _ => panic!("the shim failed ({r})"),
        };
        (oc, dump, out, err)
    }
}

/// `fm_invert` of the shim: the library's `compute_transform_inverse` in this process.
pub type InvertFn = unsafe extern "C" fn(*const f64, *mut f64, *mut c_char, c_int) -> c_int;

/// The library's `fm_invert`.
pub fn invert_fn(path: &str) -> InvertFn {
    let c = CString::new(path).unwrap();
    // SAFETY: the dynamic loader with a NUL-terminated name (RTLD_NOW | RTLD_LOCAL); the library
    // is already loaded, so this returns its handle.
    let h = unsafe { dlopen(c.as_ptr(), 2) };
    assert!(!h.is_null(), "dlopen({path})");
    let n = CString::new("fm_invert").unwrap();
    // SAFETY: a symbol lookup in the library opened above.
    let p = unsafe { dlsym(h, n.as_ptr()) };
    assert!(!p.is_null(), "fm_invert in {path}");
    // SAFETY: the shim's function has this signature.
    unsafe { std::mem::transmute::<*mut c_void, InvertFn>(p) }
}

/// Calls `fm_invert`: whether the inverse exists, the inverse (row-major), what the library
/// printed on standard output.
pub fn call_invert(f: InvertFn, m: &[[f64; 4]; 4]) -> (bool, [[f64; 4]; 4], String) {
    let mut a = [0.0f64; 16];
    for r in 0..4 {
        for c in 0..4 {
            a[4 * r + c] = m[r][c];
        }
    }
    let mut out = [0.0f64; 16];
    let mut msg = vec![0u8; 4096];
    // SAFETY: the buffers have the sizes the shim uses.
    let ok = unsafe {
        f(
            a.as_ptr(),
            out.as_mut_ptr(),
            msg.as_mut_ptr() as *mut c_char,
            4096,
        )
    };
    let n = msg.iter().position(|&b| b == 0).unwrap_or(0);
    let mut inv = [[0.0f64; 4]; 4];
    for r in 0..4 {
        for c in 0..4 {
            inv[r][c] = out[4 * r + c];
        }
    }
    (
        ok == 1,
        inv,
        String::from_utf8_lossy(&msg[..n]).into_owned(),
    )
}
