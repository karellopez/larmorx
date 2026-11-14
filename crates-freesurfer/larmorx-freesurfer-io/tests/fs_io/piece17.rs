// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Piece 17 (track A relay 27): COR folders and bshort/bfloat volumes. `vrd` and `vrh` (`mri_read`
//! with voxels and `MRIreadHeader`, as piece 6's `mrd` and `mrh`) of COR folders (`COR-.info`
//! headers line by line: every field, missing, without its number, duplicated, the flip angle's
//! long and short lines, direction cosines, Talairach transforms beside the folder or not; slices
//! of the right size, cut, missing) and of bvolumes (`<stem>_<nnn>.bshort` or `.bfloat` slices
//! and their `.hdr` headers whole or broken, both byte orders, `.bhdr` geometry files of every
//! line form, a final line without a newline, NUL bytes, a folder in its place); `vwr` (piece
//! 14's `mgw` after a subject name, `SUBJECTS_DIR` and `BFILE_LITTLE_ENDIAN`, writing with
//! `MRIwriteType` as COR, bshort, bfloat or bhdr, the read back with the same type).

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;

use super::piece4::file_entry;
use super::{Gen, Out, Work, enc, hex};

pub const KINDS: [&str; 3] = ["vrd", "vrh", "vwr"];

// ---------------------------------------------------------------- generation

/// A folder entry of a case's files.
fn dir_entry(name: &str) -> String {
    format!("D{}", hex(name.as_bytes()))
}

/// An integer as a header writes it, now and then in another form (a sign, blanks, a word, a
/// fraction, nothing).
fn int_text(g: &mut Gen, v: i64) -> String {
    if !g.chance(30) {
        return v.to_string();
    }
    match g.below(12) {
        0 => format!("+{v}"),
        1 => format!("  {v}"),
        2 => "abc".into(),
        3 => format!("{v}.5"),
        4 => String::new(),
        5 => format!("{v}x"),
        _ => v.to_string(),
    }
}

/// A float as a header writes it, now and then in another form.
fn float_text(g: &mut Gen, v: f64) -> String {
    if !g.chance(30) {
        return format!("{v:.6}");
    }
    match g.below(16) {
        0 => "nan".into(),
        1 => "inf".into(),
        2 => "abc".into(),
        3 => format!("{v:e}"),
        4 => String::new(),
        5 => format!("{v}junk"),
        _ => format!("{v:.6}"),
    }
}

/// A line ending: mostly `\n`, now and then `\r\n`, none or two.
fn eol(g: &mut Gen) -> &'static str {
    match g.below(30) {
        0 => "\r\n",
        1 => "\n\n",
        _ => "\n",
    }
}

/// The voxels of a bvolume slice file: `n` values of 2 (shorts) or 4 (floats) bytes, big-endian
/// or little-endian.
fn bvoxels(g: &mut Gen, n: usize, float: bool, little: bool) -> Vec<u8> {
    let mut b = Vec::new();
    for _ in 0..n {
        if float {
            let v: f32 = match g.below(40) {
                0 => f32::NAN,
                1 => f32::INFINITY,
                2 => -0.0,
                3 => 1.0e30,
                _ => g.int(-300, 300) as f32 / 4.0,
            };
            b.extend_from_slice(&if little {
                v.to_le_bytes()
            } else {
                v.to_be_bytes()
            });
        } else {
            let v: i16 = if g.chance(10) {
                g.pick(&[i16::MIN, i16::MAX, -1, 0])
            } else {
                g.int(-500, 500) as i16
            };
            b.extend_from_slice(&if little {
                v.to_le_bytes()
            } else {
                v.to_be_bytes()
            });
        }
    }
    b
}

/// A `.bhdr` geometry file: its lines in the writer's order, now and then in another order,
/// left out, given twice, with other spacing, an unknown name, an odd flip-angle name, a NUL byte
/// first, a last line without a newline.
fn bhdr_text(g: &mut Gen, nx: i64, ny: i64, ns: i64, nt: i64) -> Vec<u8> {
    let corner = |g: &mut Gen| -> f64 { g.int(-200, 200) as f64 / 4.0 };
    let (tlr, tla, tls) = (corner(g), corner(g), corner(g));
    let (sx, sy) = (
        g.pick(&[1.0f64, 2.0, 0.5, 1.5]),
        g.pick(&[1.0f64, 2.0, 0.5]),
    );
    // a coronal or other axis pair (now and then degenerate)
    let (xd, yd): ([f64; 3], [f64; 3]) = match g.below(6) {
        0 => ([0.0, 1.0, 0.0], [0.0, 0.0, -1.0]),
        1 => ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        2 => ([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
        3 => ([0.6, 0.8, 0.0], [0.0, 0.0, -1.0]),
        _ => ([-1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
    };
    let w = nx as f64;
    let h = ny as f64;
    let trr = tlr + xd[0] * sx * w;
    let tra = tla + xd[1] * sx * w;
    let trs = tls + xd[2] * sx * w;
    let brr = trr + yd[0] * sy * h;
    let bra = tra + yd[1] * sy * h;
    let brs = trs + yd[2] * sy * h;
    let normal = match g.below(5) {
        0 => [0.0, 1.0, 0.0],
        1 => [0.0, 0.0, 2.0],
        2 => [0.0, 0.0, 0.0],
        _ => [0.0, 1.0, 0.0],
    };
    let mut lines: Vec<(String, String)> = vec![
        ("          cols".into(), int_text(g, nx)),
        ("          rows".into(), int_text(g, ny)),
        ("       nslices".into(), int_text(g, ns)),
        (" n_time_points".into(), int_text(g, nt)),
    ];
    let fl = |g: &mut Gen, name: &str, v: f64| (name.to_owned(), float_text(g, v));
    let thick = g.pick(&[1.0, 2.0, 0.5, 3.0]);
    lines.push(fl(g, "   slice_thick", thick));
    for (n, v) in [
        ("    top_left_r", tlr),
        ("    top_left_a", tla),
        ("    top_left_s", tls),
        ("   top_right_r", trr),
        ("   top_right_a", tra),
        ("   top_right_s", trs),
        ("bottom_right_r", brr),
        ("bottom_right_a", bra),
        ("bottom_right_s", brs),
        ("      normal_r", normal[0]),
        ("      normal_a", normal[1]),
        ("      normal_s", normal[2]),
    ] {
        lines.push(fl(g, n, v));
    }
    let te = g.pick(&[0.0, 30.0, 2.5]);
    lines.push(fl(g, "      image_te", te));
    let tr = g.pick(&[0.0, 2.0, 0.75, 1.0e-3]);
    lines.push(fl(g, "      image_tr", tr));
    let ti = g.pick(&[0.0, 1.1]);
    lines.push(fl(g, "      image_ti", ti));
    let flip = g.pick(&[0.0, 90.0, 0.5]);
    lines.push(fl(g, "    flip_angle", flip));
    // drop, repeat, reorder, rename
    let mut out: Vec<String> = Vec::new();
    for (n, v) in lines {
        if g.chance(12) {
            continue;
        }
        let sep = match g.below(15) {
            0 => ":",
            1 => ":\t",
            2 => ":  ",
            _ => ": ",
        };
        let name = if g.chance(30) {
            n.trim_start().to_owned()
        } else if g.chance(40) {
            format!("\t{}", n.trim_start())
        } else {
            n.clone()
        };
        out.push(format!("{name}{sep}{v}"));
        if g.chance(25) {
            out.push(format!("{n}: {}", g.int(1, 5)));
        }
    }
    if g.chance(8) {
        let k = g.below(out.len() as u64 + 1) as usize;
        out.insert(
            k,
            g.pick(&["foo: 1", "flip_angleX 33", "", "   ", "cols:5", "#comment"])
                .to_owned(),
        );
    }
    if g.chance(10) && out.len() > 2 {
        let a = g.below(out.len() as u64) as usize;
        let b = g.below(out.len() as u64) as usize;
        out.swap(a, b);
    }
    // now and then one line that starts with a NUL byte
    let nul = if g.chance(12) {
        g.below(out.len().max(1) as u64) as usize
    } else {
        usize::MAX
    };
    let mut b = Vec::new();
    for (i, l) in out.iter().enumerate() {
        if i == nul {
            b.push(0);
        }
        b.extend_from_slice(l.as_bytes());
        if i + 1 < out.len() || !g.chance(6) {
            b.extend_from_slice(eol(g).as_bytes());
        }
    }
    b
}

/// A bvolume's files and a name for them: slices `<stem>_<nnn>.<ext>` with their headers, the
/// geometry file now and then (or a folder in its place).
fn bvol_case(g: &mut Gen, entries: &mut Vec<String>) -> (String, i32) {
    let stem = g.pick(&["w/b", "w/b", "w/b", "w/x_y", "w/b.c"]);
    let float = g.chance(2);
    let ext = if float { "bfloat" } else { "bshort" };
    let ns = g.int(1, 3);
    let ny = if g.chance(20) { 0 } else { g.int(1, 3) };
    let nx = if g.chance(20) { 0 } else { g.int(1, 4) };
    let nt = g.pick(&[1i64, 1, 1, 2, 3, 0]);
    let little = g.chance(3);
    for s in 0..ns {
        let mut h = format!(
            "{} {} {} {}",
            int_text(g, ny),
            int_text(g, nx),
            int_text(g, nt),
            i64::from(little)
        );
        match g.below(40) {
            0 => h = format!("{ny} {nx}"),
            1 => h = format!("{ny} {nx} {nt}"),
            2 => h = format!("\t{ny}\n{nx}  {nt} {}", i64::from(little)),
            3 => h = format!("{ny} {nx} {nt} {}", g.pick(&[2, -1, 7])),
            _ => {}
        }
        if !g.chance(8) {
            h.push('\n');
        }
        if s == 0 || !g.chance(3) {
            entries.push(file_entry(&format!("{stem}_{s:03}.hdr"), h.as_bytes()));
        }
        if s > 0 && g.chance(15) {
            continue;
        }
        let mut d = bvoxels(g, (nx * ny * nt.max(0)) as usize, float, little);
        match g.below(12) {
            0 if !d.is_empty() => {
                let k = g.below(d.len() as u64) as usize;
                d.truncate(k);
            }
            1 => d.extend_from_slice(&[1, 2, 3]),
            _ => {}
        }
        entries.push(file_entry(&format!("{stem}_{s:03}.{ext}"), &d));
    }
    let bhdr = g.below(10);
    if bhdr < 4 {
        let (bx, by, bs) = if g.chance(4) {
            (g.int(0, 4), g.int(0, 3), g.int(0, 4))
        } else {
            (nx, ny, ns)
        };
        let t = bhdr_text(g, bx, by, bs, nt);
        entries.push(file_entry(&format!("{stem}.bhdr"), &t));
    } else if bhdr == 4 && g.chance(4) {
        entries.push(dir_entry(&format!("{stem}.bhdr")));
    }
    let (name, ty) = match g.below(16) {
        0 => (format!("{stem}.{ext}"), -1),
        1 => (format!("{stem}_001.{ext}"), -1),
        2 => (stem.to_owned(), if float { 9 } else { 8 }),
        3 => (format!("{stem}_.{ext}"), -1),
        4 | 5 => (format!("{stem}.bhdr"), g.pick(&[-1, -1, 23])),
        6 => (format!("{stem}_000.{ext}"), if float { 8 } else { 9 }),
        7 => ("w/nope.bshort".into(), -1),
        8 => (format!("{stem}_000"), if float { 9 } else { 8 }),
        9 => ("w/b.mgz".into(), 23),
        _ => (format!("{stem}_000.{ext}"), g.pick(&[-1, -1, -1, 8, 9])),
    };
    (name, ty)
}

/// A COR header's lines: every field the writer writes (now and then left out, given twice,
/// without its number, in another form), short and long flip lines, transforms.
fn cor_info(g: &mut Gen, x: i64, y: i64, imnr0: i64, imnr1: i64) -> (Vec<u8>, u64) {
    let mut lines: Vec<String> = Vec::new();
    let i = |g: &mut Gen, n: &str, v: i64| format!("{n} {}", int_text(g, v));
    let f = |g: &mut Gen, n: &str, v: f64| format!("{n} {}", float_text(g, v));
    lines.push(i(g, "imnr0", imnr0));
    lines.push(i(g, "imnr1", imnr1));
    let pt = g.pick(&[2, 2, 0, 1]);
    lines.push(i(g, "ptype", pt));
    lines.push(i(g, "x", x));
    lines.push(i(g, "y", y));
    let fov = g.pick(&[0.256, 0.0, 0.003]);
    lines.push(f(g, "fov", fov));
    let thick = g.pick(&[0.001, 0.001, 0.002, 0.0015]);
    lines.push(f(g, "thick", thick));
    let psiz = g.pick(&[0.001, 0.001, 0.0005]);
    lines.push(f(g, "psiz", psiz));
    lines.push(f(g, "locatn", 0.0));
    for (n, v) in [
        ("strtx", -0.128),
        ("endx", 0.128),
        ("strty", -0.128),
        ("endy", 0.128),
        ("strtz", -0.128),
        ("endz", 0.128),
    ] {
        let v = if g.chance(5) { v / 2.0 } else { v };
        lines.push(f(g, n, v));
    }
    let tr = g.pick(&[0.0, 2000.0, 9.7]);
    lines.push(f(g, "tr", tr));
    let te = g.pick(&[0.0, 30.0]);
    lines.push(f(g, "te", te));
    let ti = g.pick(&[0.0, 1100.0]);
    lines.push(f(g, "ti", ti));
    lines.push(match g.below(8) {
        0 => "flip 30".into(),
        1 => "flip angle".into(),
        2 => format!("flip_angle {}", g.int(1, 90)),
        3 => "flip".into(),
        _ => {
            let v = g.pick(&[0.0, 90.0, 12.5]);
            format!("flip angle {}", float_text(g, v))
        }
    });
    let rg = g.pick(&[1, 1, 0, 2]);
    lines.push(i(g, "ras_good_flag", rg));
    let axes: [[f64; 3]; 4] = match g.below(5) {
        0 => [[0.0; 3]; 4],
        1 => [
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.5, -2.0, 3.25],
        ],
        _ => [
            [-1.0, 0.0, 0.0],
            [0.0, 0.0, -1.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0],
        ],
    };
    for (n, a) in ["x_ras", "y_ras", "z_ras", "c_ras"].iter().zip(axes) {
        let mut l = format!("{n} {:.6} {:.6} {:.6}", a[0], a[1], a[2]);
        if g.chance(12) {
            l = format!("{n} {:.6}", a[0]);
        }
        lines.push(l);
    }
    // the transform: 1 a file beside the slices, 2 the fallback ../transforms, 3 an absolute
    // name that does not exist, 4 transform instead of xform
    let xf = g.below(8);
    match xf {
        1 | 2 => lines.push("xform talairach.xfm".into()),
        3 => lines.push("xform /nonexistent/talairach.xfm".into()),
        4 => lines.push("transform talairach.xfm".into()),
        5 => lines.push("xform".into()),
        _ => {}
    }
    // now and then one line left out, one given twice, one without its value
    let mut out: Vec<String> = Vec::new();
    let pick = |g: &mut Gen, k: u64| {
        if g.chance(k) {
            g.below(lines.len() as u64) as usize
        } else {
            usize::MAX
        }
    };
    let (drop, twice, bare) = (pick(g, 10), pick(g, 10), pick(g, 10));
    for (i, l) in lines.iter().enumerate() {
        if i == drop {
            continue;
        }
        if i == bare {
            out.push(format!("{} ", l.split(' ').next().unwrap()));
            continue;
        }
        out.push(l.clone());
        if i == twice {
            out.push(l.clone());
        }
    }
    if g.chance(6) {
        // a long line first, so that a short flip line reads its bytes
        out.insert(0, format!("comment {}", "9".repeat(g.int(5, 40) as usize)));
    }
    if g.chance(8) && out.len() > 3 {
        let a = g.below(out.len() as u64) as usize;
        let b = g.below(out.len() as u64) as usize;
        out.swap(a, b);
    }
    let mut b = Vec::new();
    for l in &out {
        b.extend_from_slice(l.as_bytes());
        b.extend_from_slice(eol(g).as_bytes());
    }
    (b, xf)
}

/// A COR folder `w/c` and a name for it: the header, slices from `imnr0` to `imnr1` of `x` by
/// `y` bytes (now and then cut or missing), a Talairach transform.
fn cor_case(g: &mut Gen, entries: &mut Vec<String>) -> (String, i32) {
    let x = if g.chance(20) { 0 } else { g.int(1, 4) };
    let y = if g.chance(20) { 0 } else { g.int(1, 3) };
    let imnr0 = g.pick(&[1i64, 1, 1, 1, 0, 2, -1]);
    let n = g.int(1, 3);
    let imnr1 = imnr0 + n - 1
        + if g.chance(10) {
            g.pick(&[-1, 1, -3])
        } else {
            0
        };
    entries.push(dir_entry("w/c"));
    let (info, xf) = cor_info(g, x, y, imnr0, imnr1);
    if !g.chance(25) {
        entries.push(file_entry("w/c/COR-.info", &info));
    }
    for i in imnr0..imnr0 + n {
        if g.chance(20) {
            continue;
        }
        let mut d: Vec<u8> = (0..x * y).map(|_| g.below(256) as u8).collect();
        if g.chance(10) && !d.is_empty() {
            let k = g.below(d.len() as u64) as usize;
            d.truncate(k);
        }
        entries.push(file_entry(&format!("w/c/COR-{i:03}"), &d));
    }
    match xf {
        1 | 4 => {
            let t = super::piece6::xfm_text(g);
            entries.push(file_entry("w/c/talairach.xfm", &t));
        }
        2 => {
            entries.push(dir_entry("w/transforms"));
            let t = super::piece6::xfm_text(g);
            entries.push(file_entry("w/transforms/talairach.xfm", &t));
        }
        _ => {}
    }
    let name = match g.below(12) {
        0 => "w/c/",
        1 => "w/c/COR-001",
        2 => "w/c/COR-.info",
        3 => "w/nodir",
        4 => {
            entries.push(file_entry("w/f", b"x"));
            "w/f"
        }
        _ => "w/c",
    };
    let ty = if g.chance(4) { 0 } else { -1 };
    (name.into(), ty)
}

/// A subject for the bvolume writer: `SUBJECTS_DIR` (`w/subj`) with the subject `s1` and its
/// `mri/T1` or `mri/orig` COR folder (or neither, or a file in its place).
fn subject(g: &mut Gen, entries: &mut Vec<String>) -> (Option<&'static str>, Option<&'static str>) {
    if !g.chance(5) {
        return (None, None);
    }
    let sd = if g.chance(6) { None } else { Some("w/subj") };
    if sd.is_none() || g.chance(8) {
        return (Some("s1"), sd);
    }
    entries.push(dir_entry("w/subj"));
    if g.chance(8) {
        entries.push(file_entry("w/subj/s1", b"x"));
        return (Some("s1"), sd);
    }
    entries.push(dir_entry("w/subj/s1"));
    entries.push(dir_entry("w/subj/s1/mri"));
    let which = g.pick(&["T1", "T1", "orig", "none"]);
    if which != "none" {
        let d = format!("w/subj/s1/mri/{which}");
        entries.push(dir_entry(&d));
        let mut info = String::from(
            "imnr0 1\nimnr1 2\nptype 2\nx 3\ny 2\nfov 0.256\nthick 0.001\npsiz 0.001\nlocatn 0\nstrtx -0.128\nendx 0.128\nstrty -0.128\nendy 0.128\nstrtz -0.128\nendz 0.128\n",
        );
        info.push_str(g.pick(&[
            "ras_good_flag 1\nx_ras -1 0 0\ny_ras 0 0 -1\nz_ras 0 1 0\nc_ras 0 0 0\n",
            "ras_good_flag 0\n",
            "ras_good_flag 1\nx_ras 0.6 0.8 0\ny_ras 0 0 -1\nz_ras 0.8 -0.6 0\nc_ras 1.5 2 -3\n",
            "ras_good_flag 1\nx_ras 0.5 0.3 0.81240384\ny_ras 0.7 -0.2 -0.68556546\nz_ras 0.4 0.9 0.17320508\nc_ras 1 2 3\n",
        ]));
        entries.push(file_entry(&format!("{d}/COR-.info"), info.as_bytes()));
    }
    (Some("s1"), sd)
}

pub fn generate(g: &mut Gen, kind: &str, _portable: bool) -> Vec<String> {
    let mut t = vec![kind.to_owned()];
    if kind == "vwr" {
        return write_case(g, t);
    }
    let gd = if g.chance(8) {
        g.pick(&["10000", "10000040", "40"])
    } else {
        "0"
    };
    t.push(gd.into());
    t.push(if g.chance(12) { "0" } else { "1" }.into());
    t.push("0".into());
    let mut entries = Vec::new();
    let (name, ty) = if g.chance(2) {
        cor_case(g, &mut entries)
    } else {
        bvol_case(g, &mut entries)
    };
    t.push(ty.to_string());
    t.push(enc(Some(name.as_bytes())));
    t.push(if entries.is_empty() {
        "-".into()
    } else {
        entries.join(",")
    });
    t
}

/// A write case: `vwr` <subject> <SUBJECTS_DIR> <BFILE_LITTLE_ENDIAN> then piece 14's `mgw` tokens
/// with `how` 10 (COR), 18 (bshort), 19 (bfloat) or 33 (bhdr).
fn write_case(g: &mut Gen, mut t: Vec<String>) -> Vec<String> {
    let ty = g.pick(&[0i64, 0, 8, 8, 9, 9, 23, 12, 12]);
    let name = match ty {
        0 => g.pick(&["w/c", "w/c", "w/c/", "w/new/c", "w/f"]),
        8 => g.pick(&[
            "w/o_000.bshort",
            "w/o_000.bshort",
            "w/o.bshort",
            "w/o",
            "w/o_000",
            "w/no/o.bshort",
        ]),
        9 => g.pick(&["w/o_000.bfloat", "w/o.bfloat", "w/o_001.bfloat", "w/o"]),
        12 => g.pick(&["w/o.gdf", "w/o.gdf", "w/no/o.gdf"]),
        _ => "w/o.bhdr",
    };
    let mut entries = Vec::new();
    if name == "w/f" {
        entries.push(file_entry("w/f", b"x"));
    }
    let (sn, sd) = subject(g, &mut entries);
    t.push(enc(sn.map(str::as_bytes)));
    t.push(enc(sd.map(str::as_bytes)));
    t.push(if g.chance(4) { "1" } else { "0" }.into());
    // the GDF image stem (piece 18): GDF writes mostly with one
    let stem: Option<&[u8]> = if ty == 12 && !g.chance(8) {
        Some(g.pick(&[b"w/gi".as_slice(), b"w/gi", b"w/no/gi", b"w/o"]))
    } else {
        None
    };
    t.push(enc(stem));
    t.push(g.pick(&["0", "0", "10000040", "10000000"]).into());
    t.push(if entries.is_empty() {
        "-".into()
    } else {
        entries.join(",")
    });
    t.push("0".into());
    t.push((10 + ty).to_string());
    let (_, _, _, _, nf, alloc) = super::piece14::volume14(g, &mut t, true, false);
    t.push("-1".into());
    t.push(g.pick(&[-1i64, -1, 0, 3]).to_string());
    t.push(enc(Some(name.as_bytes())));
    let back = alloc && nf > 0 && g.chance(2);
    t.push(i64::from(back).to_string());
    t
}

// ---------------------------------------------------------------- the port's answers

/// The port's answer to a piece-17 case: `vrd`/`vrh` are piece 6's `mrd`/`mrh`, `vwr` piece 14's.
pub fn run(
    tk: &[&str],
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
    w: &Work,
    extra: &mut Vec<&'static str>,
) -> Result<(), u8> {
    let r = if tk[0] == "vwr" {
        super::piece14::run(tk, g, out, err, o, w, extra)
    } else {
        let mut t: Vec<&str> = vec![if tk[0] == "vrd" { "mrd" } else { "mrh" }];
        t.extend_from_slice(&tk[1..]);
        super::piece6::run_env(&t, g, out, err, o, w, extra, &[])
    };
    for n in g.notices.as_slice() {
        // FreeSurfer reads for ever, reads or writes outside its buffers, or crashes
        if matches!(
            n.defect.id,
            "FS-661" | "FS-669" | "FS-665" | "FS-671" | "FS-672"
        ) || (n.defect.id == "FS-660" && n.detail.ends_with("(FreeSurfer crashes)"))
        {
            extra.push("SKIP");
        }
        // FreeSurfer's numbers from memory contents
        if matches!(n.defect.id, "FS-657" | "FS-662")
            || (n.defect.id == "FS-658" && n.detail.contains("memory"))
        {
            extra.push("VARY");
        }
    }
    r
}
