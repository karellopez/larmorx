// SPDX-License-Identifier: Apache-2.0
//! The shim's entry points (`shim.c`), loaded with the dynamic loader, and the forked child each
//! call of an original runs in: its standard output and error captured in files, its dump written
//! to a file, a crash or a hang (killed after a time limit) reported as such.

use std::ffi::{CString, c_char, c_int, c_long, c_void};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

unsafe extern "C" {
    fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dup2(oldfd: c_int, newfd: c_int) -> c_int;
    fn fflush(stream: *mut c_void) -> c_int;
    fn fork() -> c_int;
    fn waitpid(pid: c_int, status: *mut c_int, options: c_int) -> c_int;
    fn kill(pid: c_int, sig: c_int) -> c_int;
    fn _exit(status: c_int) -> !;
    fn chdir(path: *const c_char) -> c_int;
}

pub type XmlFn =
    unsafe extern "C" fn(*const u8, c_long, *const c_int, c_int, *mut u8, c_long) -> c_long;
pub type ReadFn = unsafe extern "C" fn(
    *const c_char,
    c_int,
    c_int,
    *const c_int,
    *const c_int,
    c_int,
    c_int,
    *const c_char,
    c_int,
    c_int,
    *const c_char,
    *mut u8,
    c_long,
) -> c_long;
pub type ScanFn = unsafe extern "C" fn(*const c_char, *mut u32) -> c_int;

pub struct Lib {
    pub name: &'static str,
    pub xml: XmlFn,
    pub read: ReadFn,
    pub scanf_f32: ScanFn,
}

pub fn load(path: &Path, name: &'static str) -> Lib {
    let c = CString::new(path.to_string_lossy().as_bytes()).unwrap();
    // SAFETY: the dynamic loader with a NUL-terminated name (RTLD_NOW | RTLD_LOCAL).
    let h = unsafe { dlopen(c.as_ptr(), 2) };
    assert!(!h.is_null(), "dlopen({})", path.display());
    let sym = |s: &str| {
        let c = CString::new(s).unwrap();
        // SAFETY: a symbol lookup in the library opened above.
        let p = unsafe { dlsym(h, c.as_ptr()) };
        assert!(!p.is_null(), "{s} missing in {}", path.display());
        p
    };
    // SAFETY: each symbol is the shim function with this signature.
    unsafe {
        Lib {
            name,
            xml: std::mem::transmute::<*mut c_void, XmlFn>(sym("gp_xml")),
            read: std::mem::transmute::<*mut c_void, ReadFn>(sym("gp_read")),
            scanf_f32: std::mem::transmute::<*mut c_void, ScanFn>(sym("gp_scanf_f32")),
        }
    }
}

pub fn cstr(b: &[u8]) -> CString {
    CString::new(b.iter().copied().filter(|&c| c != 0).collect::<Vec<u8>>()).unwrap()
}

/// How a child ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    Exit(i32),
    Signal(i32),
    Timeout,
}

/// What an original's call in a child left: how it ended, its dump, its standard output and error.
pub struct Child {
    pub end: End,
    pub dump: Vec<u8>,
    pub out: Vec<u8>,
    pub err: Vec<u8>,
}

/// The work directory of the children (their files, and the current directory of both sides).
pub struct Work {
    pub dir: PathBuf,
}

impl Work {
    pub fn new(dir: PathBuf) -> Work {
        std::fs::create_dir_all(&dir).unwrap();
        let c = CString::new(dir.to_string_lossy().as_bytes()).unwrap();
        // SAFETY: changing this single-threaded process's current directory.
        assert_eq!(unsafe { chdir(c.as_ptr()) }, 0);
        Work { dir }
    }

    /// Runs `f` in a forked child with standard output and error in files; `f` returns the dump.
    pub fn run(&self, limit_ms: u64, f: impl FnOnce() -> Vec<u8>) -> Child {
        let p_out = self.dir.join("child-stdout.txt");
        let p_err = self.dir.join("child-stderr.txt");
        let p_dump = self.dir.join("child-dump.bin");
        let _ = std::fs::remove_file(&p_dump);
        use std::io::Write;
        let _ = std::io::stdout().flush();
        let _ = std::io::stderr().flush();
        // SAFETY: fork in a single-threaded process; the child only redirects descriptors, runs
        // the call, writes a file and leaves with _exit.
        let pid = unsafe { fork() };
        if pid == 0 {
            let o = std::fs::File::create(&p_out).unwrap();
            let e = std::fs::File::create(&p_err).unwrap();
            // SAFETY: descriptor operations in the child.
            unsafe {
                dup2(o.as_raw_fd(), 1);
                dup2(e.as_raw_fd(), 2);
            }
            let dump = f();
            // SAFETY: flushing every C stream before leaving.
            unsafe { fflush(std::ptr::null_mut()) };
            std::fs::write(&p_dump, &dump).unwrap();
            // SAFETY: leaving the child without running the parent's destructors.
            unsafe { _exit(0) };
        }
        assert!(pid > 0, "fork");
        let t0 = std::time::Instant::now();
        let mut status: c_int = 0;
        let end = loop {
            // SAFETY: waiting for the child just forked (WNOHANG).
            let r = unsafe { waitpid(pid, &mut status, 1) };
            if r == pid {
                break if status & 0x7f == 0 {
                    End::Exit((status >> 8) & 0xff)
                } else {
                    End::Signal(status & 0x7f)
                };
            }
            if t0.elapsed().as_millis() as u64 > limit_ms {
                // SAFETY: killing and reaping the child.
                unsafe {
                    kill(pid, 9);
                    waitpid(pid, &mut status, 0);
                }
                break End::Timeout;
            }
            std::thread::sleep(std::time::Duration::from_micros(100));
        };
        Child {
            end,
            dump: std::fs::read(&p_dump).unwrap_or_default(),
            out: std::fs::read(&p_out).unwrap_or_default(),
            err: std::fs::read(&p_err).unwrap_or_default(),
        }
    }
}
