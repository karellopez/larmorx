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
use larmorx_core::cfmt::Captured;
use larmorx_core::defects::{Notices, UpstreamDefects};
use larmorx_io::dicom::{self, Ctn, Data, DcmObject};
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

/// The object larmorx-io's port reads from `path` with `options` (CTN's dialect, reproduce): the
/// element script asks for its elements.
fn port_object(path: &Path, options: u64) -> Option<DcmObject> {
    let mut object = None;
    let mut stack = dicom::CondStack::new();
    let mut out = Captured::default();
    let mut notices = Notices::new(UpstreamDefects::Reproduce);
    let _ = dicom::open_file(path, options, &mut object, &Ctn, &mut stack, &mut out, &mut notices);
    object
}

const PIXEL: u32 = 0x7fe0_0010;
const MANUFACTURER: u32 = 0x0008_0070;

/// The element requests on one object (FreeSurfer's way first: DCM_GetElement, then the value
/// whole; then pieces, a context past the value, a request that says SQ, a sequence asked for as
/// another representation, large requests of pixel data left in the file, a tag only the value
/// request names), each starting from a context of 0.
fn requests(s: &mut Vec<i64>, o: Option<&DcmObject>, rng: &mut Rng) {
    let mut present: Vec<&dicom::ElementItem> = Vec::new();
    // absent tags: one past each group's last element (a group that holds no element first: its
    // own message, FS-894), then FreeSurfer's two and a group no file holds
    let mut absent: Vec<u32> = Vec::new();
    if let Some(o) = o {
        for g in &o.groups {
            for e in &g.elements {
                present.push(e);
            }
            let next = g.elements.iter().map(|e| e.tag & 0xffff).max().unwrap_or(0x10) + 2;
            let t = ((g.group as u32) << 16) | next.min(0xffff);
            if g.elements.is_empty() {
                absent.insert(0, t);
            } else {
                absent.push(t);
            }
        }
    }
    absent.truncate(3);
    absent.extend_from_slice(&[MANUFACTURER, PIXEL, 0x0009_1001]);
    let mut chosen: Vec<&dicom::ElementItem> = Vec::new();
    for e in &present {
        if e.tag == PIXEL || e.tag == MANUFACTURER || rng.chance(8.0 / present.len().max(8) as f64) {
            chosen.push(e);
        }
    }
    absent.retain(|t| !present.iter().any(|e| e.tag == *t));
    for e in chosen {
        let t = e.tag as i64;
        let len = e.length as i64;
        let seq = matches!(e.data, Data::Sequence(_));
        let on_disk = matches!(e.data, Data::OnDisk);
        s.extend_from_slice(&[9, 0, 7, t, 8, -1, 8, -1]);
        if rng.chance(0.5) {
            s.extend_from_slice(&[9, 0]);
            for _ in 0..1 + rng.below(4) {
                let c = rng.below(len.clamp(0, 40) as u64 + 4) as i64;
                s.extend_from_slice(&[8, c]);
            }
        }
        if rng.chance(0.2) && len < 0xffff_ff00 {
            s.extend_from_slice(&[9, len + 1 + rng.below(8) as i64, 8, -1]);
        }
        if rng.chance(0.15) {
            s.extend_from_slice(&[11, 15, 8, -1, 11, e.representation as i64]);
        }
        if seq && rng.chance(0.3) {
            let mut v = rng.below(31) as i64;
            if v == 15 {
                v = 0;
            }
            s.extend_from_slice(&[9, 0, 11, v, 8, 1 + rng.below(16) as i64, 11, 15]);
        }
        if on_disk && rng.chance(0.5) {
            let big = *rng.pick(&[0x7fff_ffffi64, 0x8000_0000, 0xffff_ffff, len + 2, len, 6]);
            s.extend_from_slice(&[9, 0, 8, big]);
        }
        if on_disk && rng.chance(0.3) {
            s.extend_from_slice(&[9, 1, 8, 6, 8, -1]);
        }
        if rng.chance(0.1) {
            let a = *rng.pick(&absent);
            s.extend_from_slice(&[10, a as i64, 8, -1, 10, t]);
        }
        if rng.chance(0.2) {
            s.extend_from_slice(&[3, 0]);
        }
    }
    for &a in &absent {
        s.extend_from_slice(&[9, 0, 7, a as i64, 10, a as i64, 8, -1]);
    }
}

/// The element script: the format probe's four opens (FreeSurfer's options), each followed by
/// element requests on the object it leaves, and a fifth open with other options (`extra`, or one
/// picked), after which the file may be cut before its pixel data are asked for again; then the
/// requests on the closed handle and the stack.
fn element_script(path: &Path, rng: &mut Rng, extra: Option<i64>) -> (Vec<i64>, bool) {
    let mut s = vec![4, 1];
    let extra = extra.unwrap_or(*rng.pick(&[
        0x04i64,
        0x01 | 0x400,
        0x80 | 0x400 | ACCEPT,
        0x03 | 0x400,
        0x05 | ACCEPT,
        0x02 | 0x400 | ACCEPT,
    ]));
    let mut cut = false;
    let opts = [0x80 | ACCEPT, 0x02 | ACCEPT, 0x03 | ACCEPT, 0x100 | ACCEPT, extra];
    for (k, &o) in opts.iter().enumerate() {
        s.extend_from_slice(&[1, o]);
        let obj = port_object(path, o as u64);
        requests(&mut s, obj.as_ref(), rng);
        if k == opts.len() - 1
            && let Some(px) = obj.as_ref().and_then(|o| {
                o.groups.iter().flat_map(|g| &g.elements).find(|e| e.tag == PIXEL)
            })
            && matches!(px.data, Data::OnDisk)
            && px.length != u32::MAX
            && px.length > 0
            && rng.chance(0.5)
        {
            let at = px.data_offset + rng.below(px.length as u64) as i64;
            s.extend_from_slice(&[12, at, 9, 0, 7, PIXEL as i64, 8, -1, 3, 0]);
            cut = true;
        }
        s.extend_from_slice(&[3, 0, 2, 0]);
    }
    s.extend_from_slice(&[7, MANUFACTURER as i64, 8, -1, 3, 0, 5, 0, 3, 0]);
    (s, cut)
}

struct Case {
    name: String,
    path: PathBuf,
    script: Vec<i64>,
    /// The file's bytes, written again before each run (the script cuts the file).
    restore: Option<Vec<u8>>,
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
    // the element requests' cases (part 10 files): 16-bit and 8-bit pixel data of big-endian
    // files, encapsulated pixel data, an object kept by a failure after its pixel data (its
    // descriptor closed), pixel data only inside a sequence's item; and a bare data set whose
    // first group holds only its group length (read with DCM_NOGROUPLENGTH)
    let p10 = |ts: &[u8], body: &[u8]| {
        let mut o = vec![0u8; 128];
        o.extend_from_slice(b"DICM");
        let mut meta = Vec::new();
        make::element(&mut meta, 0x0002_0010, *b"UI", ts.len() as u32, ts, le(true));
        make::element(&mut o, 0x0002_0000, *b"UL", 4, &(meta.len() as u32).to_le_bytes(), le(true));
        o.extend_from_slice(&meta);
        o.extend_from_slice(body);
        o
    };
    const TS_LE: &[u8] = b"1.2.840.10008.1.2.1\0";
    const TS_BE: &[u8] = b"1.2.840.10008.1.2.2\0";
    for (n, bits) in [("element-be-ow16", 16u16), ("element-be-ob8", 8)] {
        let mut o = Vec::new();
        id(&mut o, be(true));
        el(&mut o, 0x0008_0070, b"LO", 8, b"SIEMENS ", be(true));
        el(&mut o, 0x0028_0100, b"US", 2, &bits.to_be_bytes(), be(true));
        let px: Vec<u8> = (1..=12).collect();
        let vr = if bits == 8 { b"OB" } else { b"OW" };
        make::element(&mut o, PIXEL, *vr, 12, &px, be(true));
        v.push((n.into(), p10(TS_BE, &o)));
    }
    for (n, big) in [("element-encapsulated-le", false), ("element-encapsulated-be", true)] {
        let enc = if big { be(true) } else { le(true) };
        let mut o = Vec::new();
        id(&mut o, enc);
        let bits = if big { 16u16.to_be_bytes() } else { 16u16.to_le_bytes() };
        el(&mut o, 0x0028_0100, b"US", 2, &bits, enc);
        let mut body = Vec::new();
        make::element(&mut body, 0xfffe_e000, *b"\0\0", 0, &[], enc);
        make::element(&mut body, 0xfffe_e000, *b"\0\0", 6, &[9, 8, 7, 6, 5, 4], enc);
        make::element(&mut body, 0xfffe_e0dd, *b"\0\0", 0, &[], enc);
        make::element(&mut o, PIXEL, *b"OB", 0xffff_ffff, &body, enc);
        v.push((n.into(), p10(if big { TS_BE } else { TS_LE }, &o)));
    }
    {
        let mut o = Vec::new();
        el(&mut o, 0x0008_0000, b"UL", 4, &4u32.to_le_bytes(), le(false));
        el(&mut o, 0x0010_0010, b"PN", 4, b"DOE ", le(false));
        v.push(("element-nogrouplength".into(), o));
    }
    for (n, encapsulated) in [("element-kept-native", false), ("element-kept-encapsulated", true)] {
        let mut o = Vec::new();
        id(&mut o, le(true));
        el(&mut o, 0x0028_0100, b"US", 2, &16u16.to_le_bytes(), le(true));
        if encapsulated {
            let mut body = Vec::new();
            make::element(&mut body, 0xfffe_e000, *b"\0\0", 4, &[1, 2, 3, 4], le(true));
            make::element(&mut body, 0xfffe_e0dd, *b"\0\0", 0, &[], le(true));
            make::element(&mut o, PIXEL, *b"OB", 0xffff_ffff, &body, le(true));
        } else {
            make::element(&mut o, PIXEL, *b"OW", 4, &[1, 2, 3, 4], le(true));
        }
        o.extend_from_slice(&[0xe0, 0x7f, 0x20, 0x00, b'Z', b'Z', 2, 0, 0, 0]);
        v.push((n.into(), p10(TS_LE, &o)));
    }
    {
        let mut item = Vec::new();
        el(&mut item, 0x0028_0100, b"US", 2, &8u16.to_le_bytes(), le(true));
        make::element(&mut item, PIXEL, *b"OB", 4, &[1, 2, 3, 4], le(true));
        let mut items = Vec::new();
        make::element(&mut items, 0xfffe_e000, *b"\0\0", item.len() as u32, &item, le(true));
        let mut o = Vec::new();
        id(&mut o, le(true));
        make::element(&mut o, 0x0088_0200, *b"SQ", items.len() as u32, &items, le(true));
        v.push(("element-pixels-in-item".into(), p10(TS_LE, &o)));
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
    let mut erng = Rng::new(seed ^ 0xe1e3_e47);
    let mut element_cases = |cases: &mut Vec<Case>, name: &str, p: &Path, bytes: Option<&[u8]>| {
        let extra = name.contains("nogrouplength").then_some(0x01 | 0x400);
        let (script, cut) = element_script(p, &mut erng, extra);
        cases.push(Case {
            name: format!("{name} (elements)"),
            path: p.to_path_buf(),
            script,
            restore: if cut { bytes.map(|b| b.to_vec()) } else { None },
        });
    };
    for (name, bytes) in fixed(&files) {
        let p = write(&files, cases.len(), &bytes);
        cases.push(Case { name: name.clone(), path: p.clone(), script: std_script.clone(), restore: None });
        element_cases(&mut cases, &name, &p, Some(&bytes));
    }
    cases.push(Case { name: "missing".into(), path: files.join("no-such-file.dcm"), script: std_script.clone(), restore: None });
    element_cases(&mut cases, "missing", &files.join("no-such-file.dcm"), None);
    cases.push(Case { name: "directory".into(), path: files.clone(), script: std_script.clone(), restore: None });
    element_cases(&mut cases, "directory", &files.clone(), None);
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
        cases.push(Case { name: "long-name".into(), path: p.clone(), script: std_script.clone(), restore: None });
        element_cases(&mut cases, "long-name", &p, Some(&o));
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
        let name = format!("random-{i}-family-{fam}");
        cases.push(Case { name: name.clone(), path: p.clone(), script, restore: None });
        element_cases(&mut cases, &name, &p, Some(&f));
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
            if let Some(b) = &case.restore {
                std::fs::write(&case.path, b).unwrap();
            }
            let c = work.run(lib, &name, &case.script, 20_000);
            if let Some(b) = &case.restore {
                std::fs::write(&case.path, b).unwrap();
            }
            let pr = port::run(&case.path, &case.script, dialect);
            let kind = if case.name.ends_with("(elements)") { "elements" } else { "opens" };
            let t = totals.entry(format!("{} {kind}", lib.name)).or_default();
            *t.entry("cases".into()).or_default() += 1;
            // the streams take text: bytes that are not UTF-8 are replaced on both sides
            let lossy = |b: &[u8]| String::from_utf8_lossy(b).into_owned().into_bytes();
            let same = c.end == ffi::End::Exit(0) && c.dump == pr.dump && lossy(&c.out) == pr.out && lossy(&c.err) == pr.err;
            // the defects that leave the original's result undefined (memory never written, writes
            // past memory, crashes) or that the port bounds (FS-97); FS-94 and FS-96 are exact in
            // reproduce
            const UNDEFINED: [&str; 9] =
                ["FS-90", "FS-91", "FS-92", "FS-93", "FS-95", "FS-97", "FS-98", "FS-951", "FS-953"];
            let explained: Vec<&str> = pr.notices.iter().copied().filter(|id| UNDEFINED.contains(id)).collect();
            // the element requests, checked one by one where the original finished: every request
            // and value equal, but a value the port names as one it cannot give (memory never
            // written, a swap past the value)
            if kind == "elements" && c.end == ffi::End::Exit(0) && !same {
                let pick = |d: &[u8]| -> Vec<String> {
                    render::render(d)
                        .into_iter()
                        .filter(|l| l.starts_with("value ->") || l.starts_with("get ("))
                        .collect()
                };
                let (a, b) = (pick(&c.dump), pick(&pr.dump));
                let mut k = 0;
                let mut bad = a.len() != b.len();
                for (x, y) in a.iter().zip(&b) {
                    if x.starts_with("value ->") {
                        let flagged = pr.value_flags.get(k).copied().unwrap_or(false);
                        k += 1;
                        if x != y && !flagged {
                            bad = true;
                        }
                    } else if x != y {
                        bad = true;
                    }
                }
                *t.entry("element requests checked".into()).or_default() += 1;
                if bad {
                    *t.entry("ELEMENT REQUESTS DIFFERENT".into()).or_default() += 1;
                    failures += 1;
                    if shown < 12 {
                        shown += 1;
                        println!("== case {n} {} vs {}: element requests different", case.name, lib.name);
                        for (i, (x, y)) in a.iter().zip(&b).enumerate() {
                            if x != y {
                                println!("  {i:4} C   : {x}\n  {i:4} port: {y}");
                            }
                        }
                    }
                }
            }
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
