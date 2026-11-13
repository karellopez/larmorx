// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.io probe's piece-6 cases (relay 16, for track D's surfaces): reading volumes and
//! registration files. `mrh` (`MRIreadHeader`), `mri` (`MRIreadInfo`), `mrd` (`MRIreadType`),
//! `mgr` (`mghRead` with its frame), `rrr` (`regio_read_register`). The MGH files are built here
//! from the format (the header, the voxels, the scan parameters, the tags: command lines, the
//! talairach transform with an `.xfm` beside it, the phase-encoding direction, the field strength,
//! matrices, the morph's tags, tags of the NIfTI extension, unknown ones), plain or compressed,
//! whole or cut; names with `@format` and `#frames`.

use std::io::Write;

use flate2::Compression;
use flate2::write::GzEncoder;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::matrix::Matrix;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_io::{mriio, registerio};
use larmorx_freesurfer_mri::mri::{Mri, mri_sizeof};
use larmorx_freesurfer_mri::vol_geom::VolGeom;

use super::piece4::{file_entry, setup};
use super::{Gen, Out, Work, dec, enc, hex};

/// The piece-6 commands.
pub const KINDS: [&str; 5] = ["mrh", "mri", "mrd", "mgr", "rrr"];

// ---------------------------------------------------------------- generation

fn f32_special(g: &mut Gen) -> f32 {
    match g.below(14) {
        0 => f32::NAN,
        1 => f32::INFINITY,
        2 => -0.0,
        3 => f32::from_bits(g.next() as u32),
        _ => g.int(-40, 40) as f32 / 4.0,
    }
}

fn be32(b: &mut Vec<u8>, v: i32) {
    b.extend_from_slice(&v.to_be_bytes());
}
fn bef(b: &mut Vec<u8>, v: f32) {
    b.extend_from_slice(&v.to_bits().to_be_bytes());
}
fn tag_long(b: &mut Vec<u8>, tag: i32, data: &[u8]) {
    be32(b, tag);
    b.extend_from_slice(&(data.len() as i64).to_be_bytes());
    b.extend_from_slice(data);
}

/// A matrix tag's 1,600 bytes of text (`Matrix` and sixteen numbers, zeros after).
fn matrix_text(g: &mut Gen) -> Vec<u8> {
    let mut t = g.pick(&["Matrix", "AutoAlign", "M"]).as_bytes().to_vec();
    for k in 0..16 {
        let v = if g.chance(10) {
            g.pick(&["nan", "inf", "x", "1e40"]).to_owned()
        } else {
            format!("{:10.6}", g.int(-200, 200) as f64 / 8.0)
        };
        t.push(if k % 4 == 0 { b'\n' } else { b' ' });
        t.extend_from_slice(v.as_bytes());
    }
    let n = if g.chance(10) {
        g.int(0, 1600) as usize
    } else {
        1600
    };
    t.resize(n.max(t.len().min(n)), 0);
    t.truncate(n);
    t
}

fn vol_geom_bytes(g: &mut Gen, shear: bool) -> Vec<u8> {
    let mut b = Vec::new();
    for v in [
        g.int(0, 1) as i32,
        g.int(1, 4) as i32,
        g.int(1, 4) as i32,
        g.int(1, 4) as i32,
    ] {
        be32(&mut b, v);
    }
    for _ in 0..(if shear { 18 } else { 15 }) {
        bef(&mut b, f32_special(g));
    }
    let mut name = vec![0u8; 512];
    let s = g.pick(&["", "orig.mgz", "/x/y/T1.mgz"]).as_bytes();
    name[..s.len()].copy_from_slice(s);
    b.extend_from_slice(&name);
    b
}

/// An `.xfm` file's text.
fn xfm_text(g: &mut Gen) -> Vec<u8> {
    let mut t = String::from("MNI Transform File\n% made for the probe\n\n");
    let ty = if g.chance(10) {
        g.pick(&["Grid_Transform", "Thin_Plate_Spline_Transform", "Lin"])
    } else {
        "Linear"
    };
    t.push_str(&format!("Transform_Type = {ty};\n"));
    if g.chance(8) {
        t.push_str("Invert_Flag = True;\n");
    }
    t.push_str("Linear_Transform =\n");
    for r in 0..3 {
        for c in 0..4 {
            let v = if g.chance(12) {
                g.pick(&["0", "nan", "x1", "-0", "1e308"]).to_owned()
            } else if r == c {
                format!("{}", g.int(1, 9) as f64 / 4.0)
            } else {
                format!("{}", g.int(-20, 20) as f64 / 8.0)
            };
            t.push(' ');
            t.push_str(&v);
        }
        t.push_str(if r == 2 { ";\n" } else { "\n" });
    }
    let mut b = t.into_bytes();
    if g.chance(10) {
        b.truncate(g.int(0, b.len() as i64) as usize);
    }
    b
}

const TYPES: [i32; 12] = [0, 1, 3, 4, 10, 6, 7, 2, 5, 9, 8, 77];

/// An MGH file's bytes (uncompressed), with the extra files it names (relative to the fixture).
fn mgh_bytes(g: &mut Gen, small: bool) -> (Vec<u8>, Vec<(String, Vec<u8>)>) {
    let mut extra = Vec::new();
    let mut b = Vec::new();
    let version = if g.chance(6) {
        (g.int(-1, 5) as i32) << 8 | 1
    } else {
        1
    };
    be32(&mut b, version);
    let dim = |g: &mut Gen| -> i32 {
        if g.chance(12) {
            g.pick(&[0, -1, if small { 2 } else { 300 }])
        } else {
            g.int(1, 4) as i32
        }
    };
    let (w, h, d) = (dim(g), dim(g), dim(g));
    let mut nf = if g.chance(10) { 0 } else { g.int(1, 3) as i32 };
    let ty = if g.chance(3) { 0 } else { g.pick(&TYPES) };
    for v in [w, h, d, nf, ty, g.int(-1, 3) as i32] {
        be32(&mut b, v);
    }
    let flag: i16 = g.pick(&[1, 1, 1, 0, -1, 2]);
    b.extend_from_slice(&flag.to_be_bytes());
    let mut used = 2;
    if flag > 0 {
        for k in 0..15 {
            let v = if k < 3 {
                if g.chance(10) {
                    f32_special(g)
                } else {
                    g.int(1, 12) as f32 / 4.0
                }
            } else {
                f32_special(g)
            };
            bef(&mut b, v);
        }
        used += 60;
    }
    b.extend(std::iter::repeat_n(0u8, 256 - used));
    if ty == 6 {
        nf = 9;
    }
    let size = mri_sizeof(ty);
    let bpv = if size == usize::MAX { 0 } else { size };
    let n = (nf.max(0) as usize)
        * (w.max(0) as usize)
        * (h.max(0) as usize)
        * (d.max(0) as usize)
        * bpv;
    // at most 4 KiB of voxels (a larger extent is the skip's and the read's matter: the file ends)
    let n = n.min(1 << 12);
    for _ in 0..n {
        b.push(if g.chance(3) {
            0
        } else if g.chance(30) {
            0x7f
        } else {
            g.below(256) as u8
        });
    }
    if g.chance(10) {
        let cut = g.int(0, b.len() as i64) as usize;
        b.truncate(cut);
        return (b, extra);
    }
    for _ in 0..g.int(0, 5) {
        bef(&mut b, f32_special(g));
    }
    let ntags = if g.chance(3) { 0 } else { g.int(1, 4) };
    for _ in 0..ntags {
        match g.below(16) {
            0 | 1 => {
                let s = g
                    .pick(&["mri_convert a b", "", "recon-all -s x"])
                    .as_bytes()
                    .to_vec();
                let mut s = s;
                if g.chance(3) {
                    s.push(0);
                }
                tag_long(&mut b, 3, &s);
            }
            2..=4 => {
                // the talairach transform: a named file, the folder's transforms/talairach.xfm, or none
                let name = g.pick(&["w/t.xfm", "w/none.xfm", "talairach.xfm", "w/t"]);
                let mut s = name.as_bytes().to_vec();
                if g.chance(2) {
                    s.push(0);
                }
                if g.chance(8) {
                    s = vec![b'a'; g.int(990, 1100) as usize];
                }
                if g.chance(4) {
                    be32(&mut b, 30);
                    be32(&mut b, s.len() as i32 + 1);
                    b.extend_from_slice(&s);
                } else {
                    tag_long(&mut b, 31, &s);
                }
                if g.chance(2) {
                    extra.push(("w/t.xfm".to_owned(), xfm_text(g)));
                }
                if g.chance(3) {
                    extra.push(("w/transforms/talairach.xfm".to_owned(), xfm_text(g)));
                }
            }
            5 => {
                let s = g.pick(&["ROW", "COL", "", "-ROW"]).as_bytes().to_vec();
                tag_long(&mut b, 41, &s);
            }
            6 => {
                let v = f32_special(g);
                let len = if g.chance(5) { g.int(0, 8) } else { 4 };
                be32(&mut b, 43);
                b.extend_from_slice(&len.to_be_bytes());
                b.extend_from_slice(&v.to_le_bytes());
            }
            7 | 8 => {
                let t = matrix_text(g);
                let tag = g.pick(&[33, 44, 14]);
                tag_long(&mut b, tag, &t);
            }
            9 => {
                let mut d = Vec::new();
                be32(&mut d, g.int(-1, 3) as i32);
                be32(&mut d, g.int(0, 3) as i32);
                bef(&mut d, f32_special(g));
                tag_long(&mut b, 13, &d);
            }
            10 => {
                if g.chance(2) {
                    be32(&mut b, 10);
                    let a = vol_geom_bytes(g, false);
                    let c = vol_geom_bytes(g, false);
                    b.extend_from_slice(&a);
                    b.extend_from_slice(&c);
                } else {
                    let mut d = vol_geom_bytes(g, true);
                    d.extend(vol_geom_bytes(g, true));
                    tag_long(&mut b, 15, &d);
                }
            }
            11 => {
                be32(&mut b, 12);
                let k = (w.max(0) * h.max(0) * d.max(0)) as usize;
                for _ in 0..k.min(4096) {
                    be32(&mut b, g.int(-2, 40) as i32);
                }
            }
            12 => {
                let tag = g.pick(&[7, 45, 8, 9]);
                let n = g.int(0, 40) as usize;
                let d: Vec<u8> = (0..n).map(|_| g.below(256) as u8).collect();
                tag_long(&mut b, tag, &d);
            }
            13 => {
                let n = g.int(0, 20) as usize;
                let d: Vec<u8> = (0..n).map(|_| g.below(256) as u8).collect();
                tag_long(&mut b, g.pick(&[50, 99, 1000, 4]), &d);
            }
            14 => {
                // a length the file does not have
                be32(&mut b, g.pick(&[3, 41, 31, 77]));
                b.extend_from_slice(&g.pick(&[-1i64, -5, 1 << 20, 100]).to_be_bytes());
                b.extend_from_slice(b"abc");
            }
            _ => {
                b.extend_from_slice(&[0, 0, 0]);
            }
        }
    }
    (b, extra)
}

fn gz(b: &[u8]) -> Vec<u8> {
    let mut e = GzEncoder::new(Vec::new(), Compression::new(6));
    e.write_all(b).unwrap();
    e.finish().unwrap()
}

/// The case's name and setup for a volume read.
fn volume_case(g: &mut Gen, small: bool) -> (String, String) {
    let ext = g.pick(&[
        ".mgz", ".mgz", ".mgh", ".mgh.gz", ".MGZ", ".nii", "", ".mgh.gz",
    ]);
    let base = format!("w/a{ext}");
    let (raw, extra) = mgh_bytes(g, small);
    let compressed = ext.eq_ignore_ascii_case(".mgz") || ext == ".mgh.gz";
    let mut bytes = if compressed { gz(&raw) } else { raw };
    if compressed && g.chance(12) {
        let cut = g.int(10, bytes.len() as i64) as usize;
        bytes.truncate(cut);
    }
    let mut entries = vec![];
    let mut dirs = vec![];
    if extra.iter().any(|(n, _)| n.starts_with("w/transforms/")) {
        dirs.push(format!("D{}", hex(b"w/transforms")));
    }
    for (n, b) in &extra {
        entries.push(file_entry(n, b));
    }
    if !g.chance(15) {
        entries.push(file_entry(&base, &bytes));
    }
    let mut name = match g.below(12) {
        0 => "a.mgz".to_owned(),
        1 => "f".to_owned(),
        2 => "w/nowhere.mgz".to_owned(),
        _ => base,
    };
    match g.below(10) {
        0 => name.push_str(g.pick(&[
            "#0", "#1", "#0:1", "#1:2", "#2:1", "#x", "#-1", "#3:", "#", "#1x:2",
        ])),
        1 => name.push_str(g.pick(&["@mgz", "@mgh", "@nii", "@zz", "@"])),
        2 => name.push_str(g.pick(&["#0@mgz", "@mgz#0"])),
        _ => {}
    }
    dirs.extend(entries);
    let spec = if dirs.is_empty() {
        "-".to_owned()
    } else {
        dirs.join(",")
    };
    (name, spec)
}

fn register_case(g: &mut Gen) -> (String, String) {
    let mut t = String::new();
    let subject = g.pick(&["bert", "", "fsaverage", "a b"]);
    t.push_str(subject);
    t.push('\n');
    let items = g.int(0, 21);
    for k in 0..items {
        let v = if g.chance(12) {
            g.pick(&["x", "nan", "inf", "1e40", "-"]).to_owned()
        } else {
            format!("{}", g.int(-40, 40) as f64 / 8.0)
        };
        t.push_str(&v);
        t.push(if k % 4 == 2 { '\n' } else { ' ' });
    }
    if items >= 19 && g.chance(2) {
        t.push_str(g.pick(&[
            "round\n",
            "floor",
            "tkregister",
            "TKREG\n",
            "nearest\n",
            "rint",
        ]));
    }
    let name = g.pick(&[
        "w/register.dat",
        "w/register.dat",
        "w/x.lta",
        "w/nowhere.dat",
    ]);
    let spec = if name == "w/nowhere.dat" {
        "-".to_owned()
    } else {
        file_entry(name, t.as_bytes())
    };
    (name.to_owned(), spec)
}

pub fn generate(g: &mut Gen, kind: &str, _portable: bool) -> Vec<String> {
    let mut t = vec![kind.to_owned()];
    if kind == "rrr" {
        let (name, spec) = register_case(g);
        t.push(enc(Some(name.as_bytes())));
        t.push(spec);
        return t;
    }
    let gd = if g.chance(10) {
        g.pick(&["10000", "10000040", "40"])
    } else {
        "0"
    };
    t.push(gd.to_owned());
    if kind != "mgr" {
        t.push(if g.chance(10) { "0" } else { "1" }.to_owned());
    }
    t.push(if g.chance(8) { g.int(1, 3) } else { 0 }.to_string());
    if kind == "mrh" || kind == "mrd" {
        t.push(
            if g.chance(5) {
                g.pick(&[3, 18, 20, 0])
            } else {
                -1
            }
            .to_string(),
        );
    }
    if kind == "mgr" {
        t.push(g.int(0, 1).to_string());
        t.push(g.pick(&[-1, -1, 0, 1, 2, -2, -3]).to_string());
    }
    let (name, spec) = volume_case(g, kind != "mrh" && kind != "mri");
    t.push(enc(Some(name.as_bytes())));
    t.push(spec);
    t
}

// ---------------------------------------------------------------- the port's answers

fn fh(o: &mut Out, v: f32) {
    o.x(&v.to_bits().to_be_bytes(), 4);
}
fn dh(o: &mut Out, v: f64) {
    o.x(&v.to_bits().to_be_bytes(), 8);
}

/// A name with the fixture's path removed, escaped (`n<text>`), or `N`.
pub(super) fn oname(o: &mut Out, w: &Work, s: Option<&[u8]>) {
    let Some(s) = s else {
        o.0.push_str("N ");
        return;
    };
    let mut pfx = w.fix.to_string_lossy().into_owned().into_bytes();
    pfx.push(b'/');
    let s = super::replace_all(s, &pfx, b"");
    o.0.push('n');
    for c in s {
        if c <= 0x20 || c > 0x7e || c == b'\\' {
            o.0.push_str(&format!("\\x{c:02x}"));
        } else {
            o.0.push(c as char);
        }
    }
    o.0.push(' ');
}

fn fnv(o: &mut Out, b: &[u8]) {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &c in b {
        h ^= u64::from(c);
        h = h.wrapping_mul(1_099_511_628_211);
    }
    o.0.push_str(&format!("fnv{h:016x} "));
}

fn rmat(o: &mut Out, m: Option<&Matrix>) {
    let Some(m) = m else {
        o.0.push_str("N ");
        return;
    };
    o.i(i64::from(m.rows));
    o.i(i64::from(m.cols));
    for r in 1..=m.rows {
        for c in 1..=m.cols {
            fh(o, m.relt(r, c));
        }
    }
}

/// A morph geometry without `valid` and the extent (memory contents in FreeSurfer until a morph tag
/// sets them).
fn vg6(o: &mut Out, w: &Work, v: &VolGeom) {
    for x in [
        v.xsize, v.ysize, v.zsize, v.x_r, v.x_a, v.x_s, v.y_r, v.y_a, v.y_s, v.z_r, v.z_a, v.z_s,
        v.c_r, v.c_a, v.c_s, v.s_r, v.s_a, v.s_s,
    ] {
        fh(o, x);
    }
    oname(o, w, Some(v.fname.c_bytes()));
}

pub(super) fn print_mri(o: &mut Out, w: &Work, m: Option<&Mri>) {
    let Some(m) = m else {
        o.0.push_str("N ");
        return;
    };
    let v = &m.vg;
    for x in [
        v.width, v.height, v.depth, m.nframes, m.mtype, m.imnr0, m.imnr1,
    ] {
        o.i(i64::from(x));
    }
    for x in [
        v.xsize, v.ysize, v.zsize, v.x_r, v.x_a, v.x_s, v.y_r, v.y_a, v.y_s, v.z_r, v.z_a, v.z_s,
        v.c_r, v.c_a, v.c_s,
    ] {
        fh(o, x);
    }
    o.i(i64::from(v.ras_good_flag));
    for x in [
        m.xstart, m.xend, m.ystart, m.yend, m.zstart, m.zend, m.fov, m.thick, m.ps, m.tr, m.te,
        m.ti,
    ] {
        fh(o, x);
    }
    dh(o, m.flip_angle);
    fh(o, m.field_strength);
    for x in [m.dof, m.version, m.intent, m.cmdlines.len() as i32] {
        o.i(i64::from(x));
    }
    for c in &m.cmdlines {
        oname(o, w, Some(c.as_bytes()));
    }
    oname(o, w, Some(m.transform_fname.as_bytes()));
    o.i(i64::from(m.free_transform));
    for t in [&m.linear_transform, &m.inverse_linear_transform] {
        match t {
            None => o.0.push_str("N "),
            Some(t) => {
                for row in t.iter() {
                    for &x in row {
                        dh(o, x);
                    }
                }
            }
        }
    }
    oname(o, w, m.pedir.as_deref());
    rmat(o, m.auto_align.as_ref());
    rmat(o, m.orig_ras2vox.as_ref());
    o.i(i64::from(m.warp_field_format));
    o.i(i64::from(m.gcamorph_spacing));
    dh(o, m.gcamorph_exp_k);
    vg6(o, w, &m.gcamorph_image_vg);
    vg6(o, w, &m.gcamorph_atlas_vg);
    rmat(o, m.gcamorph_affine.as_ref());
    match &m.gcamorph_label {
        None => o.0.push_str("N "),
        Some(l) => {
            let b: Vec<u8> = l.iter().flat_map(|v| v.to_le_bytes()).collect();
            fnv(o, &b);
        }
    }
    oname(o, w, Some(v.fname.c_bytes()));
    match &v.i_to_r {
        None => o.0.push_str("N "),
        Some(a) => {
            for &x in &a.mat {
                fh(o, x);
            }
        }
    }
    rmat(o, v.r_to_i.as_ref());
    match &m.chunk {
        Some(c) if m.ischunked => fnv(o, c),
        _ => o.0.push_str("nochunk "),
    }
}

/// The port's answer to a piece-6 case (`Err`: the program's exit status).
pub fn run(
    tk: &[&str],
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
    w: &Work,
    extra: &mut Vec<&'static str>,
) -> Result<(), u8> {
    let s = super::Shared::default();
    let (mut o2, mut e2) = (s.clone(), s.clone());
    let r = answer(tk, g, &mut o2, &mut e2, o, w);
    let text = s.0.borrow().clone();
    let _ = out.write_all(&text);
    // formats the port does not read yet, and .lta registrations (fs.volume's LTAread)
    if text.windows(15).any(|x| x == b"is not ported y")
        || text.windows(16).any(|x| x == b"needs fs.volume'")
    {
        extra.push("SKIP");
    }
    let _ = err;
    for n in g.notices.as_slice() {
        // FreeSurfer crashes or writes outside its buffers there
        if matches!(
            n.defect.id,
            "FS-249"
                | "FS-253"
                | "FS-254"
                | "FS-255"
                | "FS-256"
                | "FS-257"
                | "FS-258"
                | "FS-13"
                | "FS-76"
        ) {
            extra.push("SKIP");
        }
        // a skip FreeSurfer allocates first: beyond 256 MiB the probe's memory limit decides
        if n.defect.id == "FS-178"
            && n.detail
                .split_whitespace()
                .nth(3)
                .and_then(|x| x.parse::<u64>().ok())
                .is_none_or(|want| want > 1 << 28)
        {
            extra.push("SKIP");
        }
        // FreeSurfer's numbers from memory contents (a header cut short), a transform name read
        // past its tag's text (FS-421)
        if matches!(n.defect.id, "FS-49" | "FS-167" | "FS-421") {
            extra.push("VARY");
        }
    }
    r
}

fn answer(
    tk: &[&str],
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
    w: &Work,
) -> Result<(), u8> {
    let fatal = |f: larmorx_freesurfer_core::log::Fatal| f.status();
    let dir = Some(w.fix.as_path());
    let mut i = 1;
    let mut next = || {
        let s = tk[i];
        i += 1;
        s
    };
    if tk[0] == "rrr" {
        let name = String::from_utf8(dec(next()).unwrap()).unwrap();
        setup(w, next());
        g.errno = 0;
        let mut reg = registerio::Register {
            inplaneres: -7.0,
            betplaneres: -7.0,
            intensity: -7.0,
            float2int: -7,
            ..Default::default()
        };
        let r = registerio::regio_read_register(
            g,
            out,
            err,
            dir,
            &name,
            &mut reg,
            &mut registerio::NoLta,
        )
        .map_err(fatal)?;
        o.i(i64::from(r));
        oname(o, w, reg.subject.as_deref());
        fh(o, reg.inplaneres);
        fh(o, reg.betplaneres);
        fh(o, reg.intensity);
        rmat(o, reg.r.as_ref());
        o.i(i64::from(reg.float2int));
        o.i(i64::from(g.errno));
        return Ok(());
    }
    g.gdiag = u64::from_str_radix(next(), 16).unwrap();
    if tk[0] != "mgr" {
        g.mriio_strip_pound = next().parse().unwrap();
    }
    let bits: i32 = next().parse().unwrap();
    let mut pairs: Vec<(&str, &str)> = Vec::new();
    if bits & 1 != 0 {
        pairs.push(("FS_SKIP_TAGS", "1"));
    }
    if bits & 2 != 0 {
        pairs.push(("FS_MGZIO_USEVOXELBUFREAD", "1"));
    }
    let env = Environment::from_pairs(pairs);
    let mut ty = -1;
    let (mut rv, mut frame) = (false, -1);
    if tk[0] == "mrh" || tk[0] == "mrd" {
        ty = next().parse().unwrap();
    }
    if tk[0] == "mgr" {
        rv = next() != "0";
        frame = next().parse().unwrap();
    }
    let name = String::from_utf8(dec(next()).unwrap()).unwrap();
    setup(w, next());
    g.errno = 0;
    let m = match tk[0] {
        "mrh" => mriio::mri_read_header(g, out, err, &env, dir, &name, ty),
        "mri" => mriio::mri_read_info(g, out, err, &env, dir, &name),
        "mrd" => mriio::mri_read(g, out, err, &env, dir, &name, ty, true, -1, -1),
        _ => mriio::mgh_read(g, out, err, &env, dir, &name, rv, frame),
    }
    .map_err(fatal)?;
    print_mri(o, w, m.as_ref());
    o.i(i64::from(g.errno));
    Ok(())
}
