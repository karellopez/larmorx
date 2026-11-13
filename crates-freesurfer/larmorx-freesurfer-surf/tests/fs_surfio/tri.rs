// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of queue item 3.1-48 (relay 14): `tri`, the binary triangle and quadrangle
//! surface files read and written, in an empty work folder (with a subfolder `d`): Gdiag's bits,
//! the environment (a mask: `TRIANGULARSURFACE_NOEXTRA_READ`, `TRIANGULARSURFACE_NOEXTRA_WRITE`,
//! `FS_SKIP_TAGS`, `USER`, `LOGNAME`; the creation time's replacement always set), the case's input
//! files, a surface from `MRISalloc` for the writers, 1 to 3 operations (a reader may read what a
//! writer before it wrote), then every file in the folder: the generator and the port's answers,
//! as `fssurfio.cpp`'s.

use super::cfile::maybe_cut;
use super::sfile::{creatable, list_files, utf8};
use super::*;

use larmorx_freesurfer_core::cstd::format_double;
use larmorx_freesurfer_mri::vol_geom::VolGeom;
use larmorx_freesurfer_surf::{alloc, trifile};

const OPS: [&str; 4] = ["rt", "rq", "wt", "wq"];

fn printable(g: &mut Gen, lo: i64, hi: i64) -> Vec<u8> {
    let n = g.int(lo, hi) as usize;
    (0..n).map(|_| g.int(0x21, 0x7e) as u8).collect()
}

/// A file name of operation `k` (its own), in the work folder, in `d` or in a folder that does
/// not exist.
fn name(g: &mut Gen, k: usize) -> String {
    match g.below(6) {
        0 => format!("d/lh.s{k}"),
        1 => format!("s{k}"),
        2 => format!("nodir/s{k}"),
        _ => format!("rh.s{k}"),
    }
}

fn be3(b: &mut Vec<u8>, v: i64) {
    b.extend_from_slice(&(v as u32).to_be_bytes()[1..]);
}

fn be4(b: &mut Vec<u8>, v: i64) {
    b.extend_from_slice(&(v as i32).to_be_bytes());
}

fn be8(b: &mut Vec<u8>, v: i64) {
    b.extend_from_slice(&v.to_be_bytes());
}

/// A coordinate: mostly small, now and then beyond 10000 or not finite.
fn coord(g: &mut Gen, heavy: bool) -> f32 {
    if g.chance(40) {
        return f32::from_bits(g.pick(&[
            0x7fc0_0000,
            0xff80_0000,
            0x461c_4400,
            0xc61c_4000,
            0x461c_3c00,
        ]));
    }
    if heavy && g.chance(8) {
        return f32::from_bits(smallish(g, true));
    }
    g.int(-4000, 4000) as f32 / 16.0
}

/// A volume geometry's values (as `set_vg` reads them): valid or not, a plausible extent and
/// voxel size (now and then a zero size, a singular geometry), axis-aligned direction cosines
/// with signs (now and then any), a centre, a name.
fn vg_values(g: &mut Gen) -> VolGeom {
    // (the fields in the order the generator draws them)
    let mut vg = VolGeom {
        valid: if g.chance(4) { 0 } else { 1 },
        width: g.pick(&[256, 1, 64, 32, 3]),
        height: g.pick(&[256, 1, 64, 32, 5]),
        depth: g.pick(&[256, 1, 64, 32, 7]),
        ..Default::default()
    };
    let size = |g: &mut Gen| -> f32 {
        if g.chance(30) {
            0.0
        } else {
            g.pick(&[1.0, 0.5, 2.0, 1.25, 0.9375])
        }
    };
    (vg.xsize, vg.ysize, vg.zsize) = (size(g), size(g), size(g));
    vg.ras_good_flag = i32::from(!g.chance(4));
    let mut cos = [[0.0f32; 3]; 3];
    if g.chance(6) {
        for r in &mut cos {
            for c in r.iter_mut() {
                *c = f32::from_bits(small(g)) / 64.0;
            }
        }
    } else {
        let perms = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        let p = perms[g.below(6) as usize];
        for (k, r) in cos.iter_mut().enumerate() {
            r[p[k]] = if g.chance(2) { 1.0 } else { -1.0 };
        }
    }
    (vg.x_r, vg.x_a, vg.x_s) = (cos[0][0], cos[0][1], cos[0][2]);
    (vg.y_r, vg.y_a, vg.y_s) = (cos[1][0], cos[1][1], cos[1][2]);
    (vg.z_r, vg.z_a, vg.z_s) = (cos[2][0], cos[2][1], cos[2][2]);
    let c = |g: &mut Gen| f32::from_bits(small(g)) / 8.0;
    (vg.c_r, vg.c_a, vg.c_s) = (c(g), c(g), c(g));
    vg.fname.strcpy_bytes(&printable(g, 0, 20));
    vg
}

fn vg_tokens(t: &mut Vec<String>, vg: &VolGeom) {
    for x in [vg.valid, vg.width, vg.height, vg.depth] {
        ix(t, i64::from(x));
    }
    for x in [vg.xsize, vg.ysize, vg.zsize] {
        t.push(format!("{:08x}", x.to_bits()));
    }
    ix(t, i64::from(vg.ras_good_flag));
    for x in [
        vg.x_r, vg.x_a, vg.x_s, vg.y_r, vg.y_a, vg.y_s, vg.z_r, vg.z_a, vg.z_s, vg.c_r, vg.c_a,
        vg.c_s, vg.s_r, vg.s_a, vg.s_s,
    ] {
        t.push(format!("{:08x}", x.to_bits()));
    }
    hx(t, vg.fname.c_bytes());
}

fn e15(x: f32) -> String {
    format_double("%.15e", f64::from(x)).expect("a format")
}

/// `writeVolGeom`'s text block of `vg`, now and then with a line missing or a blank line.
fn vg_text(g: &mut Gen, vg: &VolGeom) -> Vec<u8> {
    let mut lines = vec![
        if vg.valid == 0 {
            format!("valid = {}  # volume info invalid\n", vg.valid)
        } else {
            format!("valid = {}  # volume info valid\n", vg.valid)
        },
        format!(
            "filename = {}\n",
            String::from_utf8_lossy(vg.fname.c_bytes())
        ),
        format!("volume = {} {} {}\n", vg.width, vg.height, vg.depth),
        format!(
            "voxelsize = {} {} {}\n",
            e15(vg.xsize),
            e15(vg.ysize),
            e15(vg.zsize)
        ),
        format!("xras   = {} {} {}\n", e15(vg.x_r), e15(vg.x_a), e15(vg.x_s)),
        format!("yras   = {} {} {}\n", e15(vg.y_r), e15(vg.y_a), e15(vg.y_s)),
        format!("zras   = {} {} {}\n", e15(vg.z_r), e15(vg.z_a), e15(vg.z_s)),
        format!("cras   = {} {} {}\n", e15(vg.c_r), e15(vg.c_a), e15(vg.c_s)),
    ];
    if g.chance(10) {
        let k = g.below(8) as usize;
        lines.remove(k);
    }
    lines.concat().into_bytes()
}

/// A command-line tag: its length the line's with its NUL mostly, now and then 0, -1, below -1 or
/// 2^63 - 1, and as the last tag shorter or longer (each length's data as many bytes as it says,
/// at most the line's: a tag after one of another length would be read from inside it, and its
/// length from text could be one the machine's memory decides).
fn cmdline_tag(g: &mut Gen, b: &mut Vec<u8>, last: bool) {
    let mut line = printable(g, 0, 30);
    if g.chance(8) {
        line.push(0);
        line.extend_from_slice(&printable(g, 1, 5));
    }
    line.push(0);
    be4(b, 3);
    let n = line.len() as i64;
    let len = match g.below(40) {
        0 => -1,
        1 => g.pick(&[-2, -7, i64::MIN]),
        2 => i64::MAX,
        3 => 0,
        4..=6 if last => (n - g.int(1, 3)).max(0),
        7..=8 if last => n + g.int(1, 5),
        _ => n,
    };
    be8(b, len);
    let data = if len < 0 { n } else { len.min(n) };
    b.extend_from_slice(&line[..data as usize]);
}

/// The tags after a surface: the real-RAS flag, the volume geometry, the group-average area,
/// command lines, the three space tags, an unknown tag; in any order, each now and then; the last
/// now and then of an odd length, or a partial tag number.
fn tags(g: &mut Gen, b: &mut Vec<u8>) {
    let n = g.int(0, 6);
    for i in 0..n {
        let last = i == n - 1;
        match g.below(9) {
            0 => {
                be4(b, 2);
                be4(b, g.pick(&[0, 1, 1, 2, -1]));
            }
            1 | 2 => {
                be4(b, 20);
                let vg = vg_values(g);
                b.extend_from_slice(&vg_text(g, &vg));
            }
            3 => {
                be4(b, 32);
                be8(b, if last { g.pick(&[4, 4, 4, 8, 0]) } else { 4 });
                let a = if g.chance(4) {
                    f32::from_bits(f32bits(g))
                } else {
                    g.int(0, 400000) as f32 / 4.0
                };
                b.extend_from_slice(&a.to_be_bytes());
            }
            4 | 5 => cmdline_tag(g, b, last),
            6 => {
                let (tag, text): (i64, &[u8]) = match g.below(3) {
                    0 => (22, b"NIFTI_XFORM_UNKNOWN"),
                    1 => (24, b"NIFTI_XFORM_SCANNER_ANAT"),
                    _ => (
                        23,
                        b"Matrix 1.000000 0.000000 0.000000 0.000000 0.000000 1.000000 0.000000 0.000000 0.000000 0.000000 1.000000 0.000000 0.000000 0.000000 0.000000 1.000000",
                    ),
                };
                be4(b, tag);
                be8(b, text.len() as i64);
                b.extend_from_slice(text);
            }
            7 => {
                be4(b, g.pick(&[99, 1, 30, 41]));
                let len = g.int(0, 12);
                be8(b, len);
                for _ in 0..len {
                    b.push(g.int(0, 255) as u8);
                }
            }
            _ if last => {
                // a partial tag number at the end
                let k = g.int(1, 3) as usize;
                b.extend_from_slice(&[0u8, 0, 0][..k]);
                return;
            }
            _ => {
                be4(b, 2);
                be4(b, 1);
            }
        }
    }
}

/// Faces of `nv` vertices in strips (consecutive faces that share an edge in opposite directions,
/// as `quadCombine` joins them), now and then any vertices.
fn faces(g: &mut Gen, nv: i64, nf: i64) -> Vec<[i64; 3]> {
    let mut f = Vec::new();
    while (f.len() as i64) < nf {
        let pick = |g: &mut Gen| g.int(0, (nv - 1).max(0));
        if g.chance(3) || nv < 4 {
            f.push([pick(g), pick(g), pick(g)]);
        } else {
            let (a, b, c, d) = (pick(g), pick(g), pick(g), pick(g));
            f.push([a, b, c]);
            if (f.len() as i64) < nf {
                f.push([a, c, d]);
            }
        }
    }
    f
}

/// A binary triangle file: the magic number, the creator's line and its blank line (unless the
/// case reads without them), the counts (now and then negative), the vertices, the faces (now and
/// then a vertex out of range), the tags; cut now and then.
fn triangle_file(g: &mut Gen, noextra: bool, heavy: bool) -> (Vec<u8>, bool) {
    let mut b = vec![0xff, 0xff, 0xfe];
    if !noextra {
        // a line of more than 199 bytes (FS-354) not with seed 1: FreeSurfer then reads text as
        // the counts, whose allocations the machine's memory decides
        match g.below(40) {
            0 if g.1 => {
                b.extend_from_slice(format!("created by {} on today\n", "u".repeat(210)).as_bytes())
            }
            1..=3 => b.extend_from_slice(b"created by me on today\n"),
            _ => b.extend_from_slice(b"created by me on Thu Nov 12 20:55:19 2026\n\n"),
        }
    }
    let nv = g.pick(&[0, 1, 3, 4, 5, 6, 4]);
    let nf = if nv >= 3 { g.int(0, 6) } else { 0 };
    let (cv, cf) = match g.below(30) {
        0 => (g.pick(&[-1, -5]), nf),
        1 => (nv, -1),
        _ => (nv, nf),
    };
    be4(&mut b, cv);
    be4(&mut b, cf);
    if cv >= 0 && cf >= 0 {
        for _ in 0..nv * 3 {
            b.extend_from_slice(&coord(g, heavy).to_be_bytes());
        }
        for mut f in faces(g, nv, nf) {
            if g.chance(25) {
                f[g.int(0, 2) as usize] = g.pick(&[nv, -1, nv + 3]);
            }
            for v in f {
                be4(&mut b, v);
            }
        }
        tags(g, &mut b);
    }
    let before = b.len();
    maybe_cut(g, &mut b);
    (b.clone(), b.len() != before)
}

/// A quadrangle file: the old number (hundredths in shorts), the new one (floats), or none (the
/// old format: each vertex's faces too); the counts (now and then no vertices, or too many
/// quadrangles), the vertices, the quadrangles (now and then degenerate, as the writer stores a
/// lone triangle; now and then a vertex outside the surface or its room), the tags; cut now and
/// then. Also the vertices' face counts of the old format (for the dump: none when cut).
fn quad_file(g: &mut Gen, heavy: bool) -> (Vec<u8>, Vec<i64>) {
    let kind = g.below(4);
    let mut b = Vec::new();
    match kind {
        0 => b.extend_from_slice(&[0xff, 0xff, 0xff]),
        1 | 2 => b.extend_from_slice(&[0xff, 0xff, 0xfd]),
        _ => {}
    }
    let nv = if g.chance(25) {
        0
    } else {
        g.pick(&[1, 3, 4, 5, 6, 8])
    };
    let nq = if g.chance(25) {
        4 * nv + 1
    } else {
        g.int(0, 4.min(4 * nv))
    };
    be3(&mut b, nv);
    be3(&mut b, nq);
    let mut nums = Vec::new();
    if nv > 0 && nq <= 4 * nv {
        for _ in 0..nv {
            if kind == 0 {
                for _ in 0..3 {
                    b.extend_from_slice(&(g.int(-32768, 32767) as i16).to_be_bytes());
                }
            } else {
                for _ in 0..3 {
                    b.extend_from_slice(&coord(g, heavy).to_be_bytes());
                }
            }
            if kind == 3 {
                let num = g.pick(&[0, 1, 2, 3, 6]);
                b.push(num as u8);
                nums.push(num);
                for _ in 0..num {
                    be3(&mut b, g.int(0, 9));
                }
            }
        }
        for _ in 0..nq {
            let p = |g: &mut Gen| g.int(0, nv - 1);
            let q = match g.below(10) {
                0..=2 => {
                    let (a, bb, c) = (p(g), p(g), p(g));
                    [a, bb, bb, c]
                }
                3 => {
                    let k = g.int(0, 3) as usize;
                    let mut q = [p(g), p(g), p(g), p(g)];
                    q[k] = g.pick(&[nv, nv + 1, 20, 100_000]);
                    q
                }
                _ => [p(g), p(g), p(g), p(g)],
            };
            for v in q {
                be3(&mut b, v);
            }
        }
        tags(g, &mut b);
    }
    let before = b.len();
    maybe_cut(g, &mut b);
    if b.len() != before {
        nums.clear();
    }
    (b, nums)
}

pub(super) fn tri_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(3);
    ix(t, g.int(0, 3));
    // the two NOEXTRA variables together (a file read without the creator's line it was written
    // with, or the reverse, gives counts from text: allocations of the machine's memory)
    let mut env = if g.chance(6) { 3 } else { 0 };
    for (bit, k) in [(4, 8), (8, 2), (16, 3)] {
        if g.chance(k) {
            env |= bit;
        }
    }
    ix(t, env);
    hx(t, &printable(g, 1, 12));
    let date: &[u8] = g.pick(&[b"Thu Nov 12 20:55:19 2026".as_slice(), b"", b"x y"]);
    hx(t, date);
    let nops = if g.chance(3) { g.int(2, 3) } else { 1 };
    let mut ops: Vec<String> = Vec::new();
    let mut inputs: Vec<(String, Vec<u8>)> = Vec::new();
    // the files the writers wrote, each with the reader of its kind
    let mut written: Vec<(String, &str)> = Vec::new();
    for k in 0..nops as usize {
        let op = g.pick(&OPS);
        ops.push(op.to_owned());
        let mine: Vec<String> = written
            .iter()
            .filter(|w| w.1 == op)
            .map(|w| w.0.clone())
            .collect();
        let back = !mine.is_empty() && g.chance(2);
        let n = if back {
            mine[g.below(mine.len() as u64) as usize].clone()
        } else {
            name(g, k)
        };
        hx(&mut ops, n.as_bytes());
        if op == "wt" || op == "wq" {
            written.push((n, if op == "wt" { "rt" } else { "rq" }));
            continue;
        }
        let mult: f64 = if g.chance(40) {
            0.5
        } else {
            g.pick(&[1.0, 1.1, 1.5, 2.0])
        };
        ops.push(format!("{:016x}", mult.to_bits()));
        let mut nums = Vec::new();
        if !back {
            let b = if op == "rt" {
                triangle_file(g, env & 1 != 0, heavy).0
            } else {
                let (b, n) = quad_file(g, heavy);
                nums = n;
                b
            };
            if creatable(&n) && !g.chance(10) {
                inputs.push((n.clone(), b));
            }
        }
        ix(&mut ops, nums.len() as i64);
        for x in nums {
            ix(&mut ops, x);
        }
    }
    inputs.sort();
    inputs.dedup_by(|a, b| a.0 == b.0);
    ix(t, inputs.len() as i64);
    for (n, b) in &inputs {
        hx(t, n.as_bytes());
        hx(t, b);
    }
    let nv = g.pick(&[0, 1, 3, 4, 5, 6]);
    let nf = if nv >= 3 { g.int(0, 6) } else { 0 };
    ix(t, nv);
    ix(t, nf);
    for _ in 0..nv * 3 {
        t.push(format!("{:08x}", smallish(g, heavy)));
    }
    for f in faces(g, nv, nf) {
        for v in f {
            ix(t, v);
        }
    }
    ix(t, g.pick(&[0, 1, 0, 1, 2]));
    vg_tokens(t, &vg_values(g));
    match g.below(4) {
        0 => ix(t, 0),
        k => {
            ix(t, 1);
            let (r, c) = if k == 3 {
                g.pick(&[(3, 4), (4, 3), (1, 1)])
            } else {
                (4, 4)
            };
            ix(t, r);
            ix(t, c);
            for _ in 0..r * c {
                t.push(format!("{:08x}", small(g)));
            }
        }
    }
    let area: u32 = match g.below(6) {
        0..=2 => 0,
        3 => (g.int(0, 400000) as f32 / 4.0).to_bits(),
        4 => g.pick(&[0x3380_0000, 0x7fc0_0000, 0x7f80_0000, 0x3400_0000]),
        _ => f32bits(g),
    };
    t.push(format!("{area:08x}"));
    let nl = g.int(0, 3);
    ix(t, nl);
    for _ in 0..nl {
        hx(t, &printable(g, 0, 25));
    }
    let ncmds = if g.chance(30) { nl + 1 } else { nl };
    ix(t, ncmds);
    ix(t, nops);
    t.extend(ops);
}

fn dump_tri_surface(o: &mut Out, s: Option<&Mris>, nf: &[i32]) {
    let Some(s) = s else {
        o.s("NULL");
        return;
    };
    for v in [s.nvertices, s.nfaces, s.r#type, s.max_vertices, s.max_faces] {
        o.i(i64::from(v));
    }
    for vno in 0..s.nvertices as usize {
        let v = s.vertex(vno);
        for x in [v.x, v.y, v.z] {
            o.f(x);
        }
        let vt = s.topology(vno);
        o.i(i64::from(vt.num));
        o.i(i64::from(vt.f.is_some()));
        let k = nf.get(vno).copied().unwrap_or(0);
        if let Some(f) = &vt.f {
            for j in 0..(k.max(0) as usize).min(usize::from(vt.num)) {
                o.i(i64::from(f[j]));
            }
        }
    }
    for fno in 0..s.nfaces as usize {
        for &v in &s.face(fno).v {
            o.i(i64::from(v));
        }
    }
    o.i(i64::from(s.useRealRAS));
    o.f(s.group_avg_surface_area);
    let vg = &s.vg;
    for x in [vg.valid, vg.width, vg.height, vg.depth] {
        o.i(i64::from(x));
    }
    for x in [vg.xsize, vg.ysize, vg.zsize] {
        o.f(x);
    }
    o.i(i64::from(vg.ras_good_flag));
    for x in [
        vg.x_r, vg.x_a, vg.x_s, vg.y_r, vg.y_a, vg.y_s, vg.z_r, vg.z_a, vg.z_s, vg.c_r, vg.c_a,
        vg.c_s, vg.s_r, vg.s_a, vg.s_s,
    ] {
        o.f(x);
    }
    o.h(Some(vg.fname.c_bytes()));
    o.i(i64::from(vg.i_to_r.is_some()));
    o.i(i64::from(vg.r_to_i.is_some()));
    o.i(i64::from(s.ncmds));
    for i in 0..(s.ncmds.max(0) as usize).min(s.cmdlines.len()) {
        o.h(s.cmdlines[i].as_deref());
    }
}

pub(super) fn tri_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let td = tempfile::tempdir().expect("a work folder");
    let dir = td.path();
    std::fs::create_dir(dir.join("d")).expect("the folder d");
    cx.g.progname = "fssurfio".into();
    let gd = t.i();
    if gd & 1 != 0 {
        cx.g.gdiag |= DIAG_SHOW;
    }
    if gd & 2 != 0 {
        cx.g.gdiag |= DIAG_VERBOSE;
    }
    let mask = t.i();
    let user = utf8(t.h());
    let date = utf8(t.h());
    let mut pairs: Vec<(String, String)> = Vec::new();
    for (bit, var) in [
        (1, "TRIANGULARSURFACE_NOEXTRA_READ"),
        (2, "TRIANGULARSURFACE_NOEXTRA_WRITE"),
        (4, "FS_SKIP_TAGS"),
    ] {
        if mask & bit != 0 {
            pairs.push((var.into(), "1".into()));
        }
    }
    if mask & 8 != 0 {
        pairs.push(("USER".into(), user.clone()));
    }
    if mask & 16 != 0 {
        pairs.push(("LOGNAME".into(), user.clone()));
    }
    pairs.push((
        "FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING".into(),
        date,
    ));
    let env = Environment::from_pairs(pairs);
    let nin = t.i();
    for _ in 0..nin {
        let (n, b) = (utf8(t.h()), t.h());
        std::fs::write(dir.join(&n), b).expect("an input file");
    }
    let (nv, nf) = (t.i(), t.i());
    let mut m = alloc::alloc(&mut cx.g, &mut cx.a, &mut cx.b, &cx.sg, nv, nf)?;
    for vno in 0..nv as usize {
        let (x, y, z) = (t.f(), t.f(), t.f());
        let v = m.vertex_mut(vno);
        (v.x, v.y, v.z) = (x, y, z);
    }
    for fno in 0..nf as usize {
        let v = [t.i(), t.i(), t.i()];
        m.face_mut(fno).v = v;
    }
    m.useRealRAS = t.i();
    let vg = &mut m.vg;
    vg.valid = t.i();
    vg.width = t.i();
    vg.height = t.i();
    vg.depth = t.i();
    (vg.xsize, vg.ysize, vg.zsize) = (t.f(), t.f(), t.f());
    vg.ras_good_flag = t.i();
    (vg.x_r, vg.x_a, vg.x_s) = (t.f(), t.f(), t.f());
    (vg.y_r, vg.y_a, vg.y_s) = (t.f(), t.f(), t.f());
    (vg.z_r, vg.z_a, vg.z_s) = (t.f(), t.f(), t.f());
    (vg.c_r, vg.c_a, vg.c_s) = (t.f(), t.f(), t.f());
    (vg.s_r, vg.s_a, vg.s_s) = (t.f(), t.f(), t.f());
    vg.fname.strcpy_bytes(&t.h());
    if t.i() != 0 {
        let (rows, cols) = (t.i(), t.i());
        let mut a = Matrix::real(rows, cols);
        for r in 1..=rows {
            for c in 1..=cols {
                a.set_relt(r, c, t.f());
            }
        }
        m.SRASToTalSRAS_ = Some(a);
    }
    m.group_avg_surface_area = t.f();
    let nl = t.i();
    for i in 0..nl as usize {
        m.cmdlines[i] = Some(t.h());
    }
    m.ncmds = t.i();
    let nops = t.i();
    let d = Some(dir);
    for _ in 0..nops {
        let op = t.next();
        let n = utf8(t.h());
        let mut io = coords::Io {
            out: &mut cx.a.clone(),
            err: &mut cx.b.clone(),
            env: &env,
        };
        let g = &mut cx.g;
        match op {
            "rt" | "rq" => {
                let mult = t.d();
                let kn = t.i();
                let nums: Vec<i32> = (0..kn).map(|_| t.i()).collect();
                let r = if op == "rt" {
                    trifile::read_triangle_file(g, &cx.sg, &mut io, d, &n, mult)?
                } else {
                    trifile::read_quadrangle_file(g, &cx.sg, &mut io, d, &n, mult)?
                };
                dump_tri_surface(o, r.as_ref(), &nums);
            }
            "wt" => o.i(i64::from(trifile::write_triangular_surface(
                g, &mut io, d, &m, &n,
            )?)),
            "wq" => o.i(i64::from(trifile::write_quadrangle_file(
                g, &mut io, d, &m, &n,
            )?)),
            _ => panic!("tri op {op}"),
        }
    }
    o.0 += "| ";
    let mut files = Vec::new();
    list_files(dir, "", &mut files);
    files.sort();
    for n in files {
        o.h(Some(n.as_bytes()));
        match std::fs::read(dir.join(&n)) {
            Ok(b) => {
                let mut s = String::from("x");
                for c in b {
                    s += &format!("{c:02x}");
                }
                o.s(&s);
            }
            Err(_) => o.s("NOFILE"),
        }
    }
    Ok(())
}
