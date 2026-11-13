// SPDX-License-Identifier: Apache-2.0
//! The DICOM probe (README.md): generated files opened by a script of CTN calls (the format
//! probe's four attempts with their closes and stack snapshots, then other options, the stack's
//! top, pop and dump), run on CTN 3.0.6 (libup.so) and on FreeSurfer 8.2.0's copy (libfs.so), each
//! in a forked child, and on larmorx-io's port (CTN's dialect, and the probe's FreeSurfer
//! dialect), in reproduce mode; every record, standard output and error compared byte for byte.
//! Usage: fs-dicom <out-dir> [<scale>] (environment: LXPROBE_ONLY_CASE=<n> shows one case in full,
//! LXPROBE_SEED=<n>).

mod ffi;
mod make;
mod port;
mod render;

use make::{Enc, Gen, Rng};
use larmorx_io::dicom::Ctn;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const ACCEPT: i64 = 0x4000;

/// The format probe's attempts (part 10, little and big endian, format conversion), each with its
/// close and the stack after both, then the other byte orders and options.
fn standard_script() -> Vec<i64> {
    let mut s = vec![4, 1];
    for o in [0x80 | ACCEPT, 0x02 | ACCEPT, 0x03 | ACCEPT, 0x100 | ACCEPT] {
        s.extend_from_slice(&[1, o, 3, 0, 2, 0, 3, 0]);
    }
    s.extend_from_slice(&[6, 64, 6, 1, 6, 300, 4, 0, 3, 0]);
    for o in [0x04, 0x05 | ACCEPT, 0x01 | 0x400, 0x80, 0x02, 0x03 | 0x400 | ACCEPT, 0x80 | 0x400 | ACCEPT] {
        s.extend_from_slice(&[1, o, 3, 0, 2, 0]);
    }
    s.extend_from_slice(&[3, 0, 5, 0, 3, 0]);
    s
}

/// Opens that fail, repeated until the condition stack overflows.
fn overflow_script() -> Vec<i64> {
    let mut s = vec![4, 1];
    for _ in 0..45 {
        s.extend_from_slice(&[1, 0x80 | ACCEPT, 2, 0]);
    }
    s.extend_from_slice(&[3, 0, 6, 1024, 5, 0]);
    s
}

struct Case {
    name: String,
    path: PathBuf,
    script: Vec<i64>,
}

fn write(dir: &Path, n: usize, bytes: &[u8]) -> PathBuf {
    let p = dir.join(format!("case-{n}.dcm"));
    std::fs::write(&p, bytes).unwrap();
    p
}

fn le(explicit: bool) -> Enc {
    Enc { big: false, explicit }
}
fn be(explicit: bool) -> Enc {
    Enc { big: true, explicit }
}

/// The fixed edge cases (the defects' and the special elements').
fn fixed(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut v: Vec<(String, Vec<u8>)> = Vec::new();
    let el = |o: &mut Vec<u8>, t: u32, vr: &[u8; 2], len: u32, val: &[u8], e: Enc| {
        make::element(o, t, *vr, len, val, e)
    };
    let id = |o: &mut Vec<u8>, e: Enc| el(o, 0x0008_0018, b"UI", 4, b"1.2\0", e);
    for (n, len) in [("bits0", 0u32), ("bits2", 2), ("bits4", 4)] {
        let mut o = Vec::new();
        id(&mut o, le(false));
        el(&mut o, 0x0028_0100, b"US", len, &vec![8u8; len as usize], le(false));
        make::element(&mut o, 0x7fe0_0010, *b"OW", 4, &[1, 2, 3, 4], le(false));
        v.push((format!("special-{n}"), o));
    }
    for (n, val) in [
        ("ts16", &b"1.2.840.10008.1."[..]),
        ("ts-nonul", b"1.2.840.10008.1.2.1X"),
        ("ts0", b""),
        ("ts-big", b"1.2.840.10008.1.2.2\0"),
    ] {
        let mut o = vec![0u8; 128];
        o.extend_from_slice(b"DICM");
        let mut meta = Vec::new();
        el(&mut meta, 0x0002_0010, b"UI", val.len() as u32, val, le(true));
        el(&mut o, 0x0002_0000, b"UL", 4, &(meta.len() as u32).to_le_bytes(), le(true));
        o.extend_from_slice(&meta);
        let enc = if n == "ts-big" { be(true) } else { le(true) };
        el(&mut o, 0x0008_0018, b"UI", 4, b"1.2\0", enc);
        el(&mut o, 0x0018_0050, b"FD", 8, &2.5f64.to_be_bytes(), enc);
        el(&mut o, 0x0028_0030, b"UL", 6, &[0, 1, 0, 2, 0, 3], enc);
        v.push((format!("special-{n}"), o));
    }
    for (n, len) in [("meta0", 0u32), ("meta2", 2)] {
        let mut o = vec![0u8; 128];
        o.extend_from_slice(b"DICM");
        el(&mut o, 0x0002_0000, b"UL", len, &vec![0x10; len as usize], le(true));
        el(&mut o, 0x0002_0010, b"UI", 18, b"1.2.840.10008.1.2\0", le(true));
        id(&mut o, le(false));
        v.push((format!("special-{n}"), o));
    }
    for (n, len) in [("wave4", 4u32), ("wave20", 20)] {
        let mut o = Vec::new();
        id(&mut o, le(false));
        el(&mut o, 0x003a_0103, b"CS", len, &b"SS                    "[..len as usize], le(false));
        el(&mut o, 0x003a_1000, b"OW", 4, &[1, 2, 3, 4], le(false));
        v.push((format!("special-{n}"), o));
    }
    for (n, len) in [("gl0", 0u32), ("gl2", 2), ("gl6", 6)] {
        let mut o = Vec::new();
        el(&mut o, 0x0008_0000, b"UL", len, &vec![0u8; len as usize], le(false));
        id(&mut o, le(false));
        v.push((format!("special-{n}"), o));
    }
    for (n, len) in [("be-ul6", 6u32), ("be-ul8", 8), ("be-sl2", 2)] {
        let mut o = Vec::new();
        id(&mut o, be(false));
        let tag = if n == "be-sl2" { 0x0018_6020 } else { 0x0028_0008 };
        el(&mut o, 0x0008_0020, b"DA", 8, b"20261113", be(false));
        el(&mut o, tag, b"UL", len, &(0..len as u8).collect::<Vec<_>>(), be(false));
        v.push((format!("special-{n}"), o));
    }
    {
        let mut o = Vec::new();
        id(&mut o, be(false));
        el(&mut o, 0x0018_9087, b"FD", 16, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16], be(false));
        el(&mut o, 0x0020_0032, b"DS", 4, b"1\\2 ", be(false));
        el(&mut o, 0x0028_0009, b"AT", 8, &[0, 0x18, 0, 0x50, 0x00, 0x20, 0x00, 0x13], be(false));
        v.push(("special-be-fd-at".into(), o));
    }
    {
        let mut o = Vec::new();
        id(&mut o, le(false));
        el(&mut o, 0x0010_0010, b"PN", 0xffff_ffff, b"DOE^JOHN", le(false));
        v.push(("special-undefined-pn".into(), o));
    }
    for odd in [3u32, 0xffff_fffe] {
        let mut o = Vec::new();
        id(&mut o, le(true));
        let mut body = Vec::new();
        make::element(&mut body, 0xfffe_e000, *b"\0\0", odd, &[1, 2, 3, 4], le(true));
        make::element(&mut o, 0x7fe0_0010, *b"OB", 0xffff_ffff, &body, le(true));
        v.push((format!("special-fragment-{odd:x}"), o));
    }
    for depth in [8usize, 128, 129, 2000] {
        let mut inner = Vec::new();
        id(&mut inner, le(false));
        for _ in 0..depth {
            let mut item = Vec::new();
            make::element(&mut item, 0xfffe_e000, *b"\0\0", 0xffff_ffff, &inner, le(false));
            make::element(&mut item, 0xfffe_e00d, *b"\0\0", 0, &[], le(false));
            make::element(&mut item, 0xfffe_e0dd, *b"\0\0", 0, &[], le(false));
            let mut sq = Vec::new();
            make::element(&mut sq, 0x0008_1140, *b"SQ", 0xffff_ffff, &item, le(false));
            inner = sq;
        }
        v.push((format!("special-depth-{depth}"), inner));
    }
    {
        let mut o = Vec::new();
        id(&mut o, le(false));
        o.extend_from_slice(&[0xfe, 0xff, 0x0d, 0xe0, 0, 0, 0, 0]);
        id(&mut o, le(false));
        v.push(("special-top-delimiter".into(), o));
    }
    v.push(("empty".into(), Vec::new()));
    let _ = dir;
    v
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = PathBuf::from(args.get(1).expect("usage: fs-dicom <out-dir> [<scale>]"));
    let scale: usize = args.get(2).map(|s| s.parse().unwrap()).unwrap_or(1);
    let only: Option<usize> = std::env::var("LXPROBE_ONLY_CASE").ok().map(|s| s.parse().unwrap());
    let seed: u64 = std::env::var("LXPROBE_SEED").ok().map(|s| s.parse().unwrap()).unwrap_or(20261113);
    let up = ffi::load(&out.join("libup.so"), "ctn-3.0.6");
    let fs = ffi::load(&out.join("libfs.so"), "freesurfer");
    // SAFETY: a plain call into the shim.
    println!("sizeof(PRV_ELEMENT_ITEM): up {}, fs {}", unsafe { (up.sizes)() }, unsafe { (fs.sizes)() });
    let work = ffi::Work::new(out.join("work"));
    let files = out.join("files");
    let _ = std::fs::remove_dir_all(&files);
    std::fs::create_dir_all(&files).unwrap();
    let g = Gen::new();
    let mut rng = Rng::new(seed);
    let mut cases: Vec<Case> = Vec::new();
    let std_script = standard_script();
    for (name, bytes) in fixed(&files) {
        let p = write(&files, cases.len(), &bytes);
        cases.push(Case { name, path: p, script: std_script.clone() });
    }
    cases.push(Case { name: "missing".into(), path: files.join("no-such-file.dcm"), script: std_script.clone() });
    cases.push(Case { name: "directory".into(), path: files.clone(), script: std_script.clone() });
    {
        let mut d = files.clone();
        for i in 0..5 {
            d = d.join(format!("{}{i}", "d".repeat(200)));
        }
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("long-name.dcm");
        let mut o = Vec::new();
        make::element(&mut o, 0x0008_0018, *b"UI", 4, b"1.2\0", le(false));
        make::element(&mut o, 0x0010_0010, *b"PN", 0x100, b"", le(false));
        std::fs::write(&p, &o).unwrap();
        cases.push(Case { name: "long-name".into(), path: p, script: std_script.clone() });
    }
    let n_random = 2000 * scale;
    for i in 0..n_random {
        let fam = i % 10;
        let mut f = match fam {
            0..=2 => g.part10(&mut rng),
            3 => g.bare(&mut rng, le(false)),
            4 => g.bare(&mut rng, be(false)),
            5 => {
                let e = if rng.chance(0.5) { le(true) } else { be(true) };
                g.bare(&mut rng, e)
            }
            6 => make::other(&mut rng),
            _ => {
                let mut f = if rng.chance(0.6) {
                    g.part10(&mut rng)
                } else {
                    let e = le(rng.chance(0.3));
                    g.bare(&mut rng, e)
                };
                for _ in 0..1 + rng.below(3) {
                    make::mutate(&mut rng, &mut f);
                }
                f
            }
        };
        if fam != 6 && fam < 7 && rng.chance(0.1) {
            make::mutate(&mut rng, &mut f);
        }
        let script = if rng.chance(0.03) { overflow_script() } else { std_script.clone() };
        let p = write(&files, cases.len(), &f);
        cases.push(Case { name: format!("random-{i}-family-{fam}"), path: p, script });
    }
    let mut totals: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut shown = 0;
    let mut failures = 0;
    let ctn = Ctn;
    let fsd = port::FsDialect;
    for (n, case) in cases.iter().enumerate() {
        if only.is_some_and(|o| o != n) {
            continue;
        }
        let name = {
            use std::os::unix::ffi::OsStrExt;
            case.path.as_os_str().as_bytes().to_vec()
        };
        for (lib, dialect) in [(&up, &ctn as &dyn larmorx_io::dicom::Dialect), (&fs, &fsd as &dyn larmorx_io::dicom::Dialect)] {
            let c = work.run(lib, &name, &case.script, 20_000);
            let pr = port::run(&case.path, &case.script, dialect);
            let t = totals.entry(lib.name.to_string()).or_default();
            *t.entry("cases".into()).or_default() += 1;
            // the streams take text: bytes that are not UTF-8 are replaced on both sides
            let lossy = |b: &[u8]| String::from_utf8_lossy(b).into_owned().into_bytes();
            let same = c.end == ffi::End::Exit(0) && c.dump == pr.dump && lossy(&c.out) == pr.out && lossy(&c.err) == pr.err;
            // the defects that leave the original's result undefined (memory never written, writes
            // past memory, crashes) or that the port bounds (FS-97); FS-94 and FS-96 are exact in
            // reproduce
            const UNDEFINED: [&str; 7] = ["FS-90", "FS-91", "FS-92", "FS-93", "FS-95", "FS-97", "FS-98"];
            let explained: Vec<&str> = pr.notices.iter().copied().filter(|id| UNDEFINED.contains(id)).collect();
            let verdict = if pr.unsupported {
                "unsupported".to_string()
            } else if same {
                "identical".to_string()
            } else if !explained.is_empty() {
                format!("explained {}{}", explained.join("+"), if c.end != ffi::End::Exit(0) { " (original crashed)" } else { "" })
            } else {
                "DIFFERENT".to_string()
            };
            *t.entry(verdict.clone()).or_default() += 1;
            if !same && explained.is_empty() {
                failures += 1;
            }
            if std::env::var("LXPROBE_SHOW_EXPLAINED").is_ok() && verdict.starts_with("explained") {
                println!("-- case {n} {} vs {}: {verdict} {:?}", case.name, lib.name, pr.details);
            }
            let show = only.is_some() || (verdict == "DIFFERENT" && shown < 12);
            if show {
                shown += 1;
                println!("== case {n} {} vs {}: {verdict} (end {:?}, notices {:?})", case.name, lib.name, c.end, pr.notices);
                let a = render::render(&c.dump);
                let b = render::render(&pr.dump);
                let mut k = 0;
                for i in 0..a.len().max(b.len()) {
                    let (x, y) = (a.get(i), b.get(i));
                    if x != y || only.is_some() {
                        println!("  {i:4} C   : {}", x.map(|s| s.as_str()).unwrap_or("-"));
                        println!("  {i:4} port: {}", y.map(|s| s.as_str()).unwrap_or("-"));
                        k += 1;
                        if k > 6 && only.is_none() {
                            break;
                        }
                    }
                }
                if lossy(&c.out) != pr.out {
                    println!("  stdout C   : {:?}\n  stdout port: {:?}", String::from_utf8_lossy(&c.out), String::from_utf8_lossy(&pr.out));
                }
                if lossy(&c.err) != pr.err {
                    println!("  stderr C   : {:?}\n  stderr port: {:?}", String::from_utf8_lossy(&c.err), String::from_utf8_lossy(&pr.err));
                }
            }
        }
    }
    for (lib, t) in &totals {
        println!("{lib}: {t:?}");
    }
    println!("{}", if failures == 0 { "VERIFY-PASS" } else { "VERIFY-FAIL" });
    std::process::exit(if failures == 0 { 0 } else { 1 });
}
