// SPDX-License-Identifier: Apache-2.0
//! The TIFF probe (README.md): `larmorx_io::tiff` against libtiff 3.6.1 as FreeSurfer 8.2.0 builds
//! it (`libtf_fs.so`) and as released (`libtf_up.so`), each original call in a forked child.
//!
//! Usage:
//!   fs-tiff cases <out> <scratch> <seed> <n> <threads>   generated files read (read transcript)
//!   fs-tiff write <out> <scratch> <seed> <n> <threads>   FreeSurfer's TiffWriteImage, files compared
//!   fs-tiff one <out> <scratch> <seed> <case>            one generated file's transcripts
//!   fs-tiff file <out> <scratch> <file.tif>              one file's transcripts

use larmorx_core::defects::UpstreamDefects;
use larmorx_io::tiff::{Options, cases};
use std::ffi::{CString, c_char, c_int, c_void};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

unsafe extern "C" {
    fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

type RunFn = unsafe extern "C" fn(
    c_int,
    *const c_char,
    *const c_int,
    *const f64,
    *const u8,
    c_int,
    c_int,
    *mut *mut u8,
    *mut usize,
    *mut *mut u8,
    *mut usize,
    *mut c_int,
) -> c_int;
type FreeFn = unsafe extern "C" fn(*mut c_void);

#[derive(Clone, Copy)]
struct Lib {
    run: RunFn,
    free: FreeFn,
}

fn load(path: &Path) -> Lib {
    let c = CString::new(path.to_str().expect("path")).expect("path");
    // SAFETY: dlopen of the probe's own library; RTLD_NOW | RTLD_LOCAL.
    let h = unsafe { dlopen(c.as_ptr(), 2) };
    assert!(!h.is_null(), "dlopen {}", path.display());
    let sym = |n: &str| {
        let s = CString::new(n).expect("name");
        // SAFETY: a symbol of the library just opened.
        let p = unsafe { dlsym(h, s.as_ptr()) };
        assert!(!p.is_null(), "dlsym {n}");
        p
    };
    // SAFETY: the shim's functions have these signatures (shim_tiff.c).
    unsafe {
        Lib {
            run: std::mem::transmute::<*mut c_void, RunFn>(sym("tf_run")),
            free: std::mem::transmute::<*mut c_void, FreeFn>(sym("tf_free")),
        }
    }
}

/// One run's outcome: kind (0 normal, 1 exit status, 2 signal, 3 timeout), status, streams.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Out {
    kind: i32,
    status: i32,
    out: String,
    err: String,
}

fn c_run(lib: Lib, mode: i32, path: &Path, p: &[i32], d: &[f64], data: &[u8]) -> Out {
    let c = CString::new(path.to_str().expect("path")).expect("path");
    let mut o: *mut u8 = std::ptr::null_mut();
    let mut ol = 0usize;
    let mut e: *mut u8 = std::ptr::null_mut();
    let mut el = 0usize;
    let mut st: c_int = 0;
    let mut params = [0i32; 8];
    params[..p.len()].copy_from_slice(p);
    let mut dd = [0f64; 2];
    dd[..d.len()].copy_from_slice(d);
    // SAFETY: the shim's contract: it allocates *o and *e with malloc (freed below).
    let kind = unsafe {
        (lib.run)(
            mode,
            c.as_ptr(),
            params.as_ptr(),
            dd.as_ptr(),
            data.as_ptr(),
            20_000,
            4096,
            &mut o,
            &mut ol,
            &mut e,
            &mut el,
            &mut st,
        )
    };
    let take = |p: *mut u8, n: usize| {
        if p.is_null() {
            String::new()
        } else {
            // SAFETY: n bytes the shim wrote.
            let v = unsafe { std::slice::from_raw_parts(p, n) }.to_vec();
            // SAFETY: malloc'd by the shim.
            unsafe { (lib.free)(p as *mut c_void) };
            String::from_utf8_lossy(&v).into_owned()
        }
    };
    Out {
        kind,
        status: st,
        out: take(o, ol),
        err: take(e, el),
    }
}

/// The defects whose original behaviour depends on memory contents, a crash or the stack: a
/// difference with one of them noted is expected.
const VARY: &[&str] = &[
    "FS-815", "FS-816", "FS-818", "FS-819", "FS-820", "FS-821", "FS-822", "FS-823", "FS-824",
    "FS-825", "FS-826", "FS-828", "FS-830", "FS-831", "FS-832", "FS-833", "FS-834",
];

#[derive(Default)]
struct Tally {
    cases: u64,
    same: u64,
    vary: u64,
    crash_noted: u64,
    hang_noted: u64,
    unported: u64,
    alloc: u64,
    up_fs_differ: u64,
    diffs: Vec<String>,
    builds: Vec<String>,
    notices: std::collections::BTreeMap<&'static str, u64>,
}

fn opts(mode: UpstreamDefects) -> Options {
    Options {
        upstream_defects: mode,
        max_alloc: None,
        messages: Default::default(),
    }
}

fn first_diff(a: &str, b: &str) -> String {
    let la: Vec<&str> = a.lines().collect();
    let lb: Vec<&str> = b.lines().collect();
    for i in 0..la.len().max(lb.len()) {
        let x = la.get(i).copied().unwrap_or("<end>");
        let y = lb.get(i).copied().unwrap_or("<end>");
        if x != y {
            return format!("line {i}: original {x:?} port {y:?}");
        }
    }
    "same lines".into()
}

fn one_read(fs: Lib, up: Lib, path: &Path, t: &Mutex<Tally>, label: &str, verbose: bool) {
    let cfs = c_run(fs, 0, path, &[], &[], &[]);
    let cup = c_run(up, 0, path, &[], &[], &[]);
    cases::reset_thread_state();
    let (pout, perr, ids, unp) = cases::port_read(path.to_str().expect("path"), opts(UpstreamDefects::Reproduce));
    cases::reset_thread_state();
    let (fout, ferr, fids, _) = cases::port_read(path.to_str().expect("path"), opts(UpstreamDefects::Fix));
    let mut t = t.lock().expect("tally");
    t.cases += 1;
    for id in ids.iter().chain(fids.iter()) {
        *t.notices.entry(id).or_default() += 1;
    }
    if cfs != cup {
        // Informational: the port follows FreeSurfer's build; the two builds differ where
        // memory contents decide (listed for the record).
        t.up_fs_differ += 1;
        let noted = ids.iter().any(|i| VARY.contains(i));
        if !noted && unp.is_none() && cfs.kind != 2 && cfs.kind != 3 && t.builds.len() < 20 {
            t.builds.push(label.to_string());
        }
    }
    if verbose {
        eprintln!("== original (FreeSurfer's build), kind {} status {}\n{}-- stderr\n{}", cfs.kind, cfs.status, cfs.out, cfs.err);
        eprintln!("== port (reproduce), notices {ids:?}, unported {unp:?}\n{pout}-- stderr\n{perr}");
        eprintln!("== port (fix), notices {fids:?}\n{fout}-- stderr\n{ferr}");
    }
    if unp.is_some() {
        t.unported += 1;
        return;
    }
    let same = cfs.kind == 0 && cfs.out == pout && cfs.err == perr;
    if same {
        // The fix may differ only with a notice.
        if (fout != pout || ferr != perr) && fids.is_empty() && t.diffs.len() < 40 {
            t.diffs.push(format!("{label}: fix differs without a notice: {}", first_diff(&pout, &fout)));
        }
        t.same += 1;
        return;
    }
    let noted_vary = ids.iter().any(|i| VARY.contains(i));
    if cfs.kind == 3 && ids.contains(&"FS-814") {
        t.hang_noted += 1;
    } else if cfs.kind == 2 && !ids.is_empty() {
        t.crash_noted += 1;
    } else if noted_vary {
        t.vary += 1;
    } else if cfs.err.contains("No space") != perr.contains("No space") {
        t.alloc += 1;
    } else if t.diffs.len() < 40 {
        let d = if cfs.out != pout {
            first_diff(&cfs.out, &pout)
        } else {
            first_diff(&cfs.err, &perr)
        };
        t.diffs.push(format!("{label}: kind {} status {}: {d}", cfs.kind, cfs.status));
    } else {
        t.diffs.push(String::new());
    }
}

fn report(what: &str, t: &Tally) {
    let real: Vec<&String> = t.diffs.iter().filter(|d| !d.is_empty()).collect();
    for d in &real {
        eprintln!("DIFF {d}");
    }
    println!(
        "{what}: {} cases: {} identical, {} vary (memory/crash defects noted), {} crashes of the original noted, {} hangs noted, {} unported codecs, {} allocation-limit, {} release/FreeSurfer differ, {} differing",
        t.cases, t.same, t.vary, t.crash_noted, t.hang_noted, t.unported, t.alloc, t.up_fs_differ,
        t.diffs.len()
    );
    println!("notices: {:?}", t.notices);
    if !t.builds.is_empty() {
        println!("release and FreeSurfer's build differ without a noted defect: {:?}", t.builds);
    }
    println!("{}", if t.diffs.is_empty() { "VERIFY-PASS" } else { "VERIFY-FAIL" });
}

fn par(n: u64, threads: usize, f: impl Fn(u64) + Sync) {
    let next = AtomicU64::new(0);
    std::thread::scope(|sc| {
        for _ in 0..threads.max(1) {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= n {
                    break;
                }
                f(i);
            });
        }
    });
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let out = PathBuf::from(&a[2]);
    let fs = load(&std::fs::canonicalize(&out).expect("out").join("libtf_fs.so"));
    let up = load(&std::fs::canonicalize(&out).expect("out").join("libtf_up.so"));
    std::fs::create_dir_all(&a[3]).expect("scratch");
    // Files are named relative to the scratch folder (short names in the messages).
    let file_arg = a.get(4).map(|f| std::fs::canonicalize(f).unwrap_or_else(|_| PathBuf::from(f)));
    std::env::set_current_dir(&a[3]).expect("cd scratch");
    let scratch = PathBuf::from(".");
    let num = |i: usize| a[i].parse::<u64>().expect("number");
    let _ = &scratch;
    match a[1].as_str() {
        "cases" => {
            let (seed, n, threads) = (num(4), num(5), num(6) as usize);
            let t = Mutex::new(Tally::default());
            par(n, threads, |i| {
                let p = PathBuf::from(format!("r{seed}-{i}.tif"));
                std::fs::write(&p, cases::make(seed, i)).expect("write");
                one_read(fs, up, &p, &t, &format!("seed {seed} case {i}"), false);
                let _ = std::fs::remove_file(&p);
            });
            report(&format!("read seed {seed}"), &t.lock().expect("tally"));
        }
        "one" => {
            let (seed, i) = (num(4), num(5));
            let p = PathBuf::from(format!("one{seed}-{i}.tif"));
            std::fs::write(&p, cases::make(seed, i)).expect("write");
            let t = Mutex::new(Tally::default());
            one_read(fs, up, &p, &t, "one", true);
            report("one", &t.lock().expect("tally"));
        }
        "file" => {
            let p = file_arg.expect("file");
            let t = Mutex::new(Tally::default());
            one_read(fs, up, &p, &t, "file", true);
            report("file", &t.lock().expect("tally"));
        }
        "write" => {
            let (seed, n, threads) = (num(4), num(5), num(6) as usize);
            let t = Mutex::new(Tally::default());
            par(n, threads, |i| {
                let c = cases::write_case(seed, i);
                // The same name for the three writers (it is in their messages), one after the
                // other.
                let pc = PathBuf::from(format!("w{seed}-{i}.tif"));
                let pu = pc.clone();
                let pp = pc.clone();
                let pr = [
                    c.cols as i32,
                    c.rows as i32,
                    i32::from(c.spp),
                    i32::from(c.bps),
                    i32::from(c.sample_format),
                    i32::from(c.compression),
                ];
                let oc = c_run(fs, 1, &pc, &pr, &[c.xres, c.yres], &c.data);
                let bc = std::fs::read(&pc).unwrap_or_default();
                let _ = std::fs::remove_file(&pc);
                let ou = c_run(up, 1, &pu, &pr, &[c.xres, c.yres], &c.data);
                let bu = std::fs::read(&pu).unwrap_or_default();
                let _ = std::fs::remove_file(&pu);
                cases::reset_thread_state();
                let (po, pe, ids) = cases::port_write(pp.to_str().expect("path"), &c, opts(UpstreamDefects::Reproduce));
                let bp = std::fs::read(&pp).unwrap_or_default();
                let mut t = t.lock().expect("tally");
                t.cases += 1;
                for id in &ids {
                    *t.notices.entry(id).or_default() += 1;
                }
                if oc != ou || bc != bu {
                    t.up_fs_differ += 1;
                }
                if oc.kind == 0 && oc.out == po && oc.err == pe && bc == bp {
                    t.same += 1;
                } else if t.diffs.len() < 40 {
                    let d = if oc.out != po {
                        first_diff(&oc.out, &po)
                    } else if oc.err != pe {
                        first_diff(&oc.err, &pe)
                    } else {
                        let k = bc.iter().zip(&bp).position(|(x, y)| x != y).unwrap_or(bc.len().min(bp.len()));
                        format!("files differ at byte {k} (lengths {} {})", bc.len(), bp.len())
                    };
                    t.diffs.push(format!("write seed {seed} case {i}: kind {} {d}", oc.kind));
                }
                for p in [&pc, &pu, &pp] {
                    let _ = std::fs::remove_file(p);
                }
            });
            report(&format!("write seed {seed}"), &t.lock().expect("tally"));
        }
        _ => {
            eprintln!("usage: see the source's header");
            std::process::exit(2);
        }
    }
}
