// SPDX-License-Identifier: Apache-2.0
//! The JPEG probe (README.md): `larmorx_io::jpeg` against IJG 6b as FreeSurfer 8.2.0 builds it
//! (`libjp_fs.so`) and as released (`libjp_up.so`), each original call in a forked child.
//!
//! Usage:
//!   fs-jpeg cases <libdir> <scratch> <seed> <n> <threads> [<max-side>]   generated files compared
//!   fs-jpeg one <libdir> <scratch> <seed> <case>                           one case, its files kept
//!   fs-jpeg testset <libdir> <scratch>                                     the unit test's recipes
//!   fs-jpeg cuts <libdir> <scratch> <seed> <n>                              progressive files cut short
//!   fs-jpeg file <libdir> <scratch> <file>                                  one file, every reading shown

use larmorx_core::cfmt::{Captured, Stream};
use larmorx_core::defects::{Notices, UpstreamDefects};
use larmorx_io::jpeg::cases::{self, Recipe, Rng};
use larmorx_io::jpeg::{self, Options};
use std::ffi::{CString, c_char, c_int, c_void};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

unsafe extern "C" {
    fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

type RunFn = unsafe extern "C" fn(
    c_int,
    *const c_char,
    c_int,
    c_int,
    c_int,
    *const u8,
    *const c_int,
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
    // SAFETY: the shim's functions have these signatures (shim_jpeg.c).
    unsafe {
        Lib {
            run: std::mem::transmute::<*mut c_void, RunFn>(sym("jp_run")),
            free: std::mem::transmute::<*mut c_void, FreeFn>(sym("jp_free")),
        }
    }
}

/// One run's outcome: (kind, status, standard output, standard error).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Out {
    kind: i32,
    status: i32,
    out: Vec<u8>,
    err: Vec<u8>,
}

#[allow(clippy::too_many_arguments)]
fn c_run(lib: Lib, mode: i32, path: &Path, w: i32, h: i32, incomp: i32, pix: &[u8], p: &[i32]) -> Out {
    let c = CString::new(path.to_str().expect("path")).expect("path");
    let mut o: *mut u8 = std::ptr::null_mut();
    let mut ol = 0usize;
    let mut e: *mut u8 = std::ptr::null_mut();
    let mut el = 0usize;
    let mut st: c_int = 0;
    let mut params = [0i32; 32];
    params[..p.len()].copy_from_slice(p);
    // SAFETY: the shim's contract: it allocates *o and *e with malloc (freed below).
    let kind = unsafe {
        (lib.run)(
            mode,
            c.as_ptr(),
            w,
            h,
            incomp,
            pix.as_ptr(),
            params.as_ptr(),
            20_000,
            2048,
            &mut o,
            &mut ol,
            &mut e,
            &mut el,
            &mut st,
        )
    };
    let take = |p: *mut u8, n: usize| {
        if p.is_null() {
            Vec::new()
        } else {
            // SAFETY: n bytes the shim wrote.
            let v = unsafe { std::slice::from_raw_parts(p, n) }.to_vec();
            // SAFETY: malloc'd by the shim.
            unsafe { (lib.free)(p as *mut c_void) };
            v
        }
    };
    Out { kind, status: st, out: take(o, ol), err: take(e, el) }
}

/// The port's decode as the shim's child reports it.
fn rust_decode(data: &[u8], mode: UpstreamDefects) -> (Out, Vec<String>) {
    let mut cap = Captured::default();
    let mut rows = Vec::new();
    let mut notices = Notices::new(mode);
    let opts = Options { upstream_defects: mode, max_alloc: Some(1 << 30) };
    let r = jpeg::decode_rgb(data, &mut cap, opts, &mut notices, &mut |row| {
        rows.extend_from_slice(row)
    });
    let (kind, status) = match &r {
        Ok(h) => {
            rows.extend_from_slice(format!("ok {} {}\n", h.image_width, h.image_height).as_bytes());
            (0, 0)
        }
        Err(_) => (1, 1),
    };
    let ids = notices.as_slice().iter().map(|n| n.defect.id.to_string()).collect();
    (Out { kind, status, out: rows, err: cap.text(Stream::Err).into_bytes() }, ids)
}

fn rust_header(data: &[u8]) -> Out {
    let mut cap = Captured::default();
    let r = jpeg::read_header(data, &mut cap, Options::default());
    let (kind, status, out) = match &r {
        Ok(h) => (0, 0, format!("{} {}\n", h.image_width, h.image_height).into_bytes()),
        Err(_) => (1, 1, Vec::new()),
    };
    Out { kind, status, out, err: cap.text(Stream::Err).into_bytes() }
}

fn rust_gray(w: u32, h: u32, pix: &[u8]) -> Out {
    let mut cap = Captured::default();
    let mut file = Vec::new();
    let r = jpeg::encode_gray(
        w,
        h,
        &mut |r, row| row.copy_from_slice(&pix[(r * w) as usize..((r + 1) * w) as usize]),
        &mut file,
        &mut cap,
    );
    let (kind, status) = if r.is_ok() { (0, 0) } else { (1, 1) };
    Out { kind, status, out: file, err: cap.text(Stream::Err).into_bytes() }
}

/// The input pixels of IJG-made files: noise, gradients, stripes or flat areas.
fn pixels(r: &mut Rng, w: u32, h: u32, comps: u32) -> Vec<u8> {
    let kind = r.below(5);
    let base = r.next();
    let mut v = Vec::with_capacity((w * h * comps) as usize);
    for y in 0..h {
        for x in 0..w {
            for c in 0..comps {
                v.push(match kind {
                    0 => r.next() as u8,
                    1 => ((x * 255) / w.max(1) + c * 30) as u8,
                    2 => ((((x / 2) ^ (y / 3) ^ c) & 1) * 255) as u8,
                    3 => (base >> (8 * c)) as u8,
                    _ => ((x.wrapping_mul(7) ^ y.wrapping_mul(13)).wrapping_add(c * 50)
                        ^ (r.next() as u32 & 15)) as u8,
                });
            }
        }
    }
    v
}

/// The bytes of one case's input file, made by IJG's compressor or larmorx's test encoder.
fn make_input(up: Lib, dir: &Path, seed: u64, max_side: u32) -> Option<Vec<u8>> {
    let mut r = Rng(seed ^ 0x6669_6c65);
    if r.below(5) < 2 {
        return Some(cases::make(&Recipe::random(seed, max_side)));
    }
    let w = 1 + r.below(max_side as u64) as i32;
    let h = 1 + r.below(max_side as u64) as i32;
    // in colour space: grey 1, RGB 2, YCbCr 3, CMYK 4
    let (incomp, inspace) = [(1, 1), (3, 2), (3, 2), (3, 3), (4, 4)][r.below(5) as usize];
    let mut p = [0i32; 31];
    p[0] = if r.below(4) == 0 {
        match inspace {
            1 => 1,
            2 | 3 => [2, 3][r.below(2) as usize],
            _ => [4, 5][r.below(2) as usize],
        }
    } else {
        -1
    };
    p[1] = 1 + r.below(100) as i32;
    p[2] = (r.below(3) == 0) as i32;
    p[3] = (r.below(3) == 0) as i32;
    p[4] = if r.below(4) == 0 { 1 + r.below(9) as i32 } else { 0 };
    p[5] = if p[4] == 0 && r.below(6) == 0 { 1 + r.below(3) as i32 } else { 0 };
    p[6] = r.below(3) as i32;
    p[7] = if r.below(6) == 0 { r.below(101) as i32 } else { 0 };
    p[8] = if r.below(6) == 0 { r.below(2) as i32 } else { -1 };
    p[9] = if r.below(6) == 0 { r.below(2) as i32 } else { -1 };
    if r.below(3) == 0 {
        for c in 0..incomp as usize {
            p[10 + 2 * c] = 1 + r.below(4) as i32;
            p[11 + 2 * c] = 1 + r.below(4) as i32;
        }
    }
    p[30] = inspace;
    let pix = pixels(&mut r, w as u32, h as u32, incomp as u32);
    let path = dir.join("made.jpg");
    let _ = std::fs::remove_file(&path);
    let o = c_run(up, 3, &path, w, h, incomp, &pix, &p);
    if o.kind != 0 {
        return None; // the compressor refused these parameters
    }
    let mut f = std::fs::read(&path).ok()?;
    if r.below(3) == 0 {
        cases::damage(&mut f, seed, 1 + r.below(4) as u32);
    }
    Some(f)
}

#[derive(Default)]
struct Tally {
    cases: usize,
    identical: usize,
    fix_differs_noted: usize,
    big: usize,
    not_comparable: usize,
    made_refused: usize,
    up_fs_differ: usize,
    different: usize,
    headers: usize,
    encodes: usize,
    warnings: usize,
    errors: usize,
}

fn show(tag: &str, o: &Out) -> String {
    format!(
        "{tag}: kind {} status {} out {} bytes err {:?}",
        o.kind,
        o.status,
        o.out.len(),
        String::from_utf8_lossy(&o.err)
    )
}

/// One case: returns a line for each difference found.
fn one_case(fs: Lib, up: Lib, dir: &Path, seed: u64, max_side: u32, t: &Mutex<Tally>, keep: bool) -> Vec<String> {
    let mut diffs = Vec::new();
    let Some(data) = make_input(up, dir, seed, max_side) else {
        t.lock().expect("tally").made_refused += 1;
        return diffs;
    };
    let path = dir.join("case.jpg");
    std::fs::write(&path, &data).expect("write");
    if keep {
        eprintln!("input {} ({} bytes)", path.display(), data.len());
    }
    // a header first: images too large to decode here are counted, not compared
    let hd = rust_header(&data);
    let chd = c_run(fs, 1, &path, 0, 0, 0, &[], &[]);
    if hd != chd {
        diffs.push(format!("seed {seed} header: {} / {}", show("port", &hd), show("fs", &chd)));
    }
    let big = std::str::from_utf8(&hd.out)
        .ok()
        .and_then(|s| {
            let mut it = s.split_whitespace().map(|x| x.parse::<u64>().ok());
            Some(it.next()?? * it.next()??)
        })
        .is_some_and(|px| px > 4_000_000);
    {
        let mut g = t.lock().expect("tally");
        g.cases += 1;
        g.headers += 1;
        if big {
            g.big += 1;
        }
    }
    if big {
        return diffs;
    }
    let cfs = c_run(fs, 0, &path, 0, 0, 0, &[], &[]);
    let cup = c_run(up, 0, &path, 0, 0, 0, &[], &[]);
    if cfs != cup {
        t.lock().expect("tally").up_fs_differ += 1;
        diffs.push(format!("seed {seed} up/fs: {} / {}", show("up", &cup), show("fs", &cfs)));
    }
    if cfs.kind > 1 || cfs.status > 1 || String::from_utf8_lossy(&cfs.err).contains("Insufficient memory") {
        t.lock().expect("tally").not_comparable += 1;
        return diffs;
    }
    let (rep, _) = rust_decode(&data, UpstreamDefects::Reproduce);
    let (fix, noted) = rust_decode(&data, UpstreamDefects::Fix);
    let (_, rep_noted) = rust_decode(&data, UpstreamDefects::Reproduce);
    {
        let mut g = t.lock().expect("tally");
        if !cfs.err.is_empty() {
            if cfs.kind == 0 { g.warnings += 1 } else { g.errors += 1 }
        }
    }
    if rep != cfs {
        if rep_noted.is_empty() {
            t.lock().expect("tally").different += 1;
            diffs.push(format!("seed {seed} reproduce: {} / {}", show("port", &rep), show("fs", &cfs)));
        } else {
            // FS-813 in reproduce mode: the component no scan names holds memory contents
            t.lock().expect("tally").not_comparable += 1;
        }
    } else if fix != cfs && noted.is_empty() {
        t.lock().expect("tally").different += 1;
        diffs.push(format!("seed {seed} fix: {} / {}", show("port", &fix), show("fs", &cfs)));
    } else if fix != cfs {
        t.lock().expect("tally").fix_differs_noted += 1;
    } else {
        t.lock().expect("tally").identical += 1;
    }
    // the grey encoder on an image of the same seed
    let mut r = Rng(seed ^ 0x656e_63);
    let w = 1 + r.below(max_side as u64) as u32;
    let h = 1 + r.below(max_side as u64) as u32;
    let pix = pixels(&mut r, w, h, 1);
    let out = dir.join("gray.jpg");
    let _ = std::fs::remove_file(&out);
    let mut c = c_run(fs, 2, &out, w as i32, h as i32, 1, &pix, &[]);
    c.out = std::fs::read(&out).unwrap_or_default();
    let p = rust_gray(w, h, &pix);
    t.lock().expect("tally").encodes += 1;
    if p != c {
        t.lock().expect("tally").different += 1;
        diffs.push(format!("seed {seed} encode {w}x{h}: {} / {}", show("port", &p), show("fs", &c)));
    }
    diffs
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let cmd = a.get(1).map(String::as_str).unwrap_or("");
    let libdir = PathBuf::from(a.get(2).expect("libdir"));
    let scratch = PathBuf::from(a.get(3).expect("scratch"));
    let fs = load(&libdir.join("libjp_fs.so"));
    let up = load(&libdir.join("libjp_up.so"));
    std::fs::create_dir_all(&scratch).expect("scratch");
    match cmd {
        "cases" => {
            let seed: u64 = a[4].parse().expect("seed");
            let n: usize = a[5].parse().expect("n");
            let threads: usize = a[6].parse().expect("threads");
            let max_side: u32 = a.get(7).map(|s| s.parse().expect("max side")).unwrap_or(96);
            let next = AtomicUsize::new(0);
            let tally = Mutex::new(Tally::default());
            let lines = Mutex::new(Vec::new());
            std::thread::scope(|s| {
                for th in 0..threads {
                    let dir = scratch.join(format!("t{th}"));
                    std::fs::create_dir_all(&dir).expect("dir");
                    let (next, tally, lines) = (&next, &tally, &lines);
                    s.spawn(move || {
                        loop {
                            let i = next.fetch_add(1, Ordering::Relaxed);
                            if i >= n {
                                break;
                            }
                            let cs = seed * 1_000_000 + i as u64;
                            let d = one_case(fs, up, &dir, cs, max_side, tally, false);
                            if !d.is_empty() {
                                let mut l = lines.lock().expect("lines");
                                for x in d {
                                    if l.len() < 40 {
                                        println!("{x}");
                                    }
                                    l.push(x);
                                }
                            }
                        }
                    });
                }
            });
            let t = tally.into_inner().expect("tally");
            println!(
                "cases {} (made refused {}): identical {}, fix differs with notices {}, big {}, not comparable {}, different {}, up/fs differ {}; headers {}, encodes {}; originals with warnings {}, with errors {}",
                t.cases, t.made_refused, t.identical, t.fix_differs_noted, t.big, t.not_comparable,
                t.different, t.up_fs_differ, t.headers, t.encodes, t.warnings, t.errors
            );
            println!("{}", if t.different == 0 && t.up_fs_differ == 0 { "VERIFY-PASS" } else { "VERIFY-FAIL" });
        }
        "one" => {
            let seed: u64 = a[4].parse().expect("seed");
            let i: u64 = a[5].parse().expect("case");
            let tally = Mutex::new(Tally::default());
            for d in one_case(fs, up, &scratch, seed * 1_000_000 + i, 96, &tally, true) {
                println!("{d}");
            }
        }
        "testset" => {
            let mut bad = 0;
            let mut n = 0;
            for seed in 0..400u64 {
                let f = cases::make(&Recipe::random(seed, 40));
                let path = scratch.join("ts.jpg");
                std::fs::write(&path, &f).expect("write");
                let c = c_run(fs, 0, &path, 0, 0, 0, &[], &[]);
                let (rep, noted) = rust_decode(&f, UpstreamDefects::Reproduce);
                n += 1;
                if rep != c && noted.is_empty() {
                    bad += 1;
                    println!("recipe {seed}: {} / {}", show("port", &rep), show("fs", &c));
                }
            }
            for seed in 0..40u64 {
                let w = 1 + (seed as u32 * 7) % 37;
                let h = 1 + (seed as u32 * 5) % 29;
                let img = cases::gray_image(seed, w, h);
                let out = scratch.join("ts-gray.jpg");
                let _ = std::fs::remove_file(&out);
                let mut c = c_run(fs, 2, &out, w as i32, h as i32, 1, &img, &[]);
                c.out = std::fs::read(&out).unwrap_or_default();
                n += 1;
                if rust_gray(w, h, &img) != c {
                    bad += 1;
                    println!("gray {seed}: differs");
                }
            }
            println!("testset {n} cases, {bad} different: {}", if bad == 0 { "VERIFY-PASS" } else { "VERIFY-FAIL" });
        }
        "cuts" => {
            // IJG's progressive files (grey and colour, with and without subsampling) cut after
            // each of 24 points: later scans missing, refinement incomplete (block smoothing)
            let seed: u64 = a[4].parse().expect("seed");
            let n: u64 = a[5].parse().expect("n");
            let (mut same, mut bad, mut total) = (0, 0, 0);
            for i in 0..n {
                let mut r = Rng(seed * 1_000_000 + i);
                let w = 8 + r.below(120) as i32;
                let h = 8 + r.below(120) as i32;
                let (incomp, inspace) = [(1, 1), (3, 2), (3, 3)][r.below(3) as usize];
                let mut p = [0i32; 31];
                p[0] = -1;
                p[1] = 30 + r.below(70) as i32;
                p[2] = 1;
                p[6] = r.below(3) as i32;
                p[8] = -1;
                p[9] = -1;
                p[30] = inspace;
                if r.below(2) == 0 {
                    for c in 0..incomp as usize {
                        p[10 + 2 * c] = 1 + r.below(2) as i32;
                        p[11 + 2 * c] = 1 + r.below(2) as i32;
                    }
                }
                let pix = pixels(&mut r, w as u32, h as u32, incomp as u32);
                let made = scratch.join("cut-made.jpg");
                let _ = std::fs::remove_file(&made);
                if c_run(up, 3, &made, w, h, incomp, &pix, &p).kind != 0 {
                    continue;
                }
                let full = std::fs::read(&made).expect("made");
                for k in 1..=24usize {
                    let cut = full.len() * k / 25;
                    let data = &full[..cut];
                    let path = scratch.join("cut.jpg");
                    std::fs::write(&path, data).expect("write");
                    let c = c_run(fs, 0, &path, 0, 0, 0, &[], &[]);
                    let (rep, _) = rust_decode(data, UpstreamDefects::Reproduce);
                    let (fix, _) = rust_decode(data, UpstreamDefects::Fix);
                    total += 1;
                    if rep == c && fix == c {
                        same += 1;
                    } else {
                        bad += 1;
                        if bad < 10 {
                            println!("seed {i} cut {cut}: {} / {}", show("port", &rep), show("fs", &c));
                        }
                    }
                }
            }
            println!("cuts {total}: identical {same}, different {bad}: {}", if bad == 0 { "VERIFY-PASS" } else { "VERIFY-FAIL" });
        }
        "file" => {
            let data = std::fs::read(&a[4]).expect("file");
            let path = PathBuf::from(&a[4]);
            let c = c_run(fs, 0, &path, 0, 0, 0, &[], &[]);
            let u = c_run(up, 0, &path, 0, 0, 0, &[], &[]);
            let (rep, rn) = rust_decode(&data, UpstreamDefects::Reproduce);
            let (fix, fx) = rust_decode(&data, UpstreamDefects::Fix);
            println!("{}", show("fs", &c));
            println!("{} (same as fs: {})", show("up", &u), u == c);
            println!("{} notices {rn:?} (same as fs: {})", show("reproduce", &rep), rep == c);
            println!("{} notices {fx:?} (same as fs: {})", show("fix", &fix), fix == c);
        }
        _ => {
            eprintln!("usage: see the source's header");
            std::process::exit(2);
        }
    }
}
