// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.io probe's piece-2 cases (queue item 3.1-25's second piece): FreeSurfer's tags on C
//! streams and znzlib files, FStagsIO's writers, readers and lengths, VOL_GEOM's write and read,
//! the binary colour tables. Generated files are built here from the formats' rules (and then
//! sometimes cut or changed); the port answers each case as `fsio` prints FreeSurfer's.

use std::io::Write;

use larmorx_freesurfer_core::cname::{CName, STRLEN};
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::diag::{DIAG_INFO, DIAG_SHOW};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc};
use larmorx_freesurfer_io::cfile::CFile;
use larmorx_freesurfer_io::colortab_bin;
use larmorx_freesurfer_io::fio;
use larmorx_freesurfer_io::fstagsio::{self, FsTagsIo};
use larmorx_freesurfer_io::tags;
use larmorx_freesurfer_mri::ctab::{ColorTable, ColorTableEntry};
use larmorx_freesurfer_mri::mri::{self, MRI_UCHAR, Mri, MriFrame};
use larmorx_freesurfer_mri::vol_geom::VolGeom;
use larmorx_io::znz::ZnzFile;

use super::{Gen, Out, Work, dec, enc, hex, unhex};

/// The piece-2 commands.
pub const KINDS: [&str; 12] = [
    "vgw", "vgr", "fsw", "fsr", "fsl", "ctw", "ctr", "tw", "tr", "ztw", "ztr", "cmdl",
];

// ---------------------------------------------------------------- generation

fn fbits(g: &mut Gen) -> u32 {
    match g.below(10) {
        0 => g.pick(&[
            0u32,
            0x8000_0000,
            0x3f80_0000,
            0xbf80_0000,
            0x7f80_0000,
            0xff80_0000,
            0x7fc0_0000,
            0xffc0_0000,
            0x7f80_0001,
            0x7f7f_ffff,
            0x0000_0001,
            0x7149_f2ca,
        ]),
        1 | 2 => g.next() as u32,
        _ => (g.int(-4000, 4000) as f32 / 8.0).to_bits(),
    }
}

fn dbits(g: &mut Gen) -> u64 {
    match g.below(8) {
        0 => g.pick(&[
            0u64,
            0x8000_0000_0000_0000,
            0x7ff0_0000_0000_0000,
            0x7ff8_0000_0000_0000,
            0x47ef_ffff_e000_0000,
            0x4800_0000_0000_0000,
            0x3ff0_0000_0000_0001,
        ]),
        1 => g.next(),
        _ => (g.int(-100_000, 100_000) as f64 / 64.0).to_bits(),
    }
}

fn name(g: &mut Gen) -> Vec<u8> {
    match g.below(12) {
        0 => Vec::new(),
        1 => vec![b'n'; g.pick(&[500usize, 511, 512, 513, 600, 1022, 1023])],
        2 => (0..g.int(1, 40)).map(|_| g.int(1, 255) as u8).collect(),
        _ => g
            .pick(&[
                b"lh.white".as_slice(),
                b"/x/orig.mgz",
                b"a",
                b"Left-Cerebral-White-Matter",
                b"talairach.m3z",
                b"\xc3\xa9t\xc3\xa9",
            ])
            .to_vec(),
    }
}

fn ename(b: &[u8]) -> String {
    enc(Some(b))
}

fn small_int(g: &mut Gen) -> i64 {
    if g.chance(10) {
        g.pick(&[-1i64, i64::from(i32::MIN), i64::from(i32::MAX), 1 << 20])
    } else {
        g.int(0, 300)
    }
}

/// A geometry's tokens and the geometry (two names strcpy'd in turn).
fn gen_vg(g: &mut Gen) -> (Vec<String>, VolGeom) {
    let mut t = Vec::new();
    let mut v = VolGeom::default();
    let ints = [g.int(0, 1), small_int(g), small_int(g), small_int(g)];
    for x in ints {
        t.push(x.to_string());
    }
    v.valid = ints[0] as i32;
    v.width = ints[1] as i32;
    v.height = ints[2] as i32;
    v.depth = ints[3] as i32;
    let mut f = [0u32; 18];
    for (i, x) in f.iter_mut().enumerate() {
        *x = if i >= 15 && g.chance(2) { 0 } else { fbits(g) };
        t.push(format!("{x:08x}"));
    }
    set_floats(&mut v, &f);
    let (a, b) = (name(g), name(g));
    t.push(ename(&a));
    t.push(ename(&b));
    v.fname = CName::new(&a);
    v.fname.strcpy_bytes(&b);
    (t, v)
}

fn set_floats(v: &mut VolGeom, f: &[u32; 18]) {
    let p: [&mut f32; 18] = [
        &mut v.xsize,
        &mut v.ysize,
        &mut v.zsize,
        &mut v.x_r,
        &mut v.x_a,
        &mut v.x_s,
        &mut v.y_r,
        &mut v.y_a,
        &mut v.y_s,
        &mut v.z_r,
        &mut v.z_a,
        &mut v.z_s,
        &mut v.c_r,
        &mut v.c_a,
        &mut v.c_s,
        &mut v.s_r,
        &mut v.s_a,
        &mut v.s_s,
    ];
    for (d, &b) in p.into_iter().zip(f) {
        *d = f32::from_bits(b);
    }
}

fn floats_of(v: &VolGeom) -> [f32; 18] {
    [
        v.xsize, v.ysize, v.zsize, v.x_r, v.x_a, v.x_s, v.y_r, v.y_a, v.y_s, v.z_r, v.z_a, v.z_s,
        v.c_r, v.c_a, v.c_s, v.s_r, v.s_a, v.s_s,
    ]
}

fn be32(d: &mut Vec<u8>, v: i32) {
    d.extend_from_slice(&v.to_be_bytes());
}
fn be64(d: &mut Vec<u8>, v: i64) {
    d.extend_from_slice(&v.to_be_bytes());
}
fn bef(d: &mut Vec<u8>, v: f32) {
    d.extend_from_slice(&v.to_bits().to_be_bytes());
}

/// A geometry as `VOL_GEOM::write` stores it.
fn ser_vg(d: &mut Vec<u8>, v: &VolGeom, nifti: bool, shearless: bool) {
    for x in [v.valid, v.width, v.height, v.depth] {
        be32(d, x);
    }
    let f = floats_of(v);
    for x in &f[..15] {
        bef(d, *x);
    }
    if !shearless {
        for x in &f[15..] {
            bef(d, *x);
        }
    }
    if !nifti {
        d.extend_from_slice(&v.fname.buffer()[..512]);
    } else {
        let n = v.fname.len().min(512);
        be32(d, n as i32);
        d.extend_from_slice(&v.fname.buffer()[..n]);
    }
}

/// A matrix's tokens (`4` or `3` and its numbers) and the matrix.
fn gen_mat(g: &mut Gen, k: i32) -> (Vec<String>, Matrix, [u32; 16]) {
    let mut t = vec![k.to_string()];
    let mut m = alloc(k);
    let mut bits = [0u32; 16];
    for r in 1..=k {
        for c in 1..=k {
            let b = if g.chance(3) {
                if r == c { 0x3f80_0000 } else { 0 }
            } else {
                fbits(g)
            };
            if k == 4 {
                bits[((r - 1) * 4 + c - 1) as usize] = b;
            }
            t.push(format!("{b:08x}"));
            m.set_relt(r, c, f32::from_bits(b));
        }
    }
    (t, m, bits)
}

fn alloc(k: i32) -> Matrix {
    let mut a = Vec::new();
    let mut b = Vec::new();
    let mut g = Globals::new(larmorx_core::defects::UpstreamDefects::Reproduce);
    matrix_alloc(&mut g, &mut a, &mut b, k, k, MATRIX_REAL)
        .ok()
        .flatten()
        .expect("a matrix")
}

/// A matrix's text tag body: `Matrix %10lf x 16` in 1,600 bytes (sometimes other words or
/// numbers).
fn mat_text(g: &mut Gen, bits: &[u32; 16]) -> Vec<u8> {
    let mut s: Vec<u8> = match g.below(14) {
        0 => b"AutoAlign".to_vec(),
        1 => vec![b'W'; g.pick(&[99usize, 100, 150])],
        _ => b"Matrix".to_vec(),
    };
    for b in bits {
        s.push(b' ');
        if g.chance(40) {
            s.extend_from_slice(g.pick(&[b"x".as_slice(), b"1e", b"infin", b"0x1p3", b"nan(1)"]));
        } else {
            s.extend_from_slice(fmt_f(f64::from(f32::from_bits(*b)), 10, 6).as_bytes());
        }
    }
    let mut buf = vec![0u8; tags::MATRIX_STRLEN];
    if g.chance(20) {
        // no NUL in the 1,600 bytes
        for (i, x) in buf.iter_mut().enumerate() {
            *x = if i % 7 == 0 { b' ' } else { b'7' };
        }
    }
    let n = s.len().min(buf.len());
    buf[..n].copy_from_slice(&s[..n]);
    buf
}

/// A colour table's tokens and the table.
fn gen_ct(g: &mut Gen) -> (Vec<String>, ColorTable) {
    let version = g.pick(&[1, 2, 2, 2, 3]);
    let n = g.int(0, 6) as i32;
    let fname = name(g);
    let mut t = vec![version.to_string(), n.to_string(), ename(&fname)];
    let mut ct = ColorTable {
        entries: vec![None; n as usize],
        fname: fname.clone(),
        version,
        ..ColorTable::default()
    };
    for i in 0..n as usize {
        if g.chance(3) {
            t.push("0".into());
            continue;
        }
        let nm = name(g);
        let c: Vec<i32> = (0..4)
            .map(|_| {
                if g.chance(12) {
                    g.next() as i32
                } else {
                    g.int(0, 255) as i32
                }
            })
            .collect();
        t.push("1".into());
        t.push(ename(&nm));
        for x in &c {
            t.push(x.to_string());
        }
        ct.entries[i] = Some(Box::new(ColorTableEntry {
            name: nm,
            ri: c[0],
            gi: c[1],
            bi: c[2],
            ai: c[3],
            ..ColorTableEntry::default()
        }));
    }
    (t, ct)
}

/// The binary table as the writers store it (with odd lengths, counts and versions sometimes).
fn ser_ct(g: &mut Gen, d: &mut Vec<u8>, ct: &ColorTable) {
    let name = |g: &mut Gen, d: &mut Vec<u8>, s: &[u8]| {
        let len = match g.below(30) {
            0 => -1,
            1 => 0,
            2 => s.len() as i32,
            3 => s.len() as i32 + 50,
            4 => g.pick(&[1023, 1024, 1030, 1100, 1 << 30]),
            _ => s.len() as i32 + 1,
        };
        be32(d, len);
        d.extend_from_slice(s);
        d.push(0);
        if len > s.len() as i32 + 1 && len < 5000 {
            d.extend(std::iter::repeat_n(b'q', (len as usize) - s.len() - 1));
        }
    };
    let v2 = ct.version == 2 || g.chance(3);
    let n = if g.chance(25) {
        g.pick(&[100_000, 2_000_000, 1 << 25])
    } else {
        ct.nentries()
    };
    if v2 {
        be32(d, if g.chance(30) { -3 } else { -2 });
        be32(d, n);
        name(g, d, &ct.fname);
        let k = ct.entries.iter().flatten().count() as i32;
        be32(d, if g.chance(15) { k + 1 } else { k });
        for (i, e) in ct.entries.iter().enumerate() {
            if let Some(e) = e {
                let s = match g.below(25) {
                    0 => -1,
                    1 => n,
                    2 => 0,
                    _ => i as i32,
                };
                be32(d, s);
                name(g, d, &e.name);
                for x in [e.ri, e.gi, e.bi, 255i32.wrapping_sub(e.ai)] {
                    be32(d, x);
                }
            }
        }
    } else {
        be32(d, n.max(1));
        name(g, d, &ct.fname);
        for e in ct.entries.iter().flatten() {
            name(g, d, &e.name);
            for x in [e.ri, e.gi, e.bi, 255i32.wrapping_sub(e.ai)] {
                be32(d, x);
            }
        }
    }
}

/// A frame's tokens and the frame.
fn gen_frame(g: &mut Gen) -> (Vec<String>, MriFrame) {
    let mut t = Vec::new();
    let mut f = MriFrame {
        ftype: if g.chance(2) { 1 } else { g.int(0, 3) as i32 },
        ..MriFrame::default()
    };
    t.push(f.ftype.to_string());
    let fl = |g: &mut Gen, t: &mut Vec<String>| {
        let b = fbits(g);
        t.push(format!("{b:08x}"));
        f32::from_bits(b)
    };
    f.te = fl(g, &mut t);
    f.tr = fl(g, &mut t);
    f.flip = fl(g, &mut t);
    f.ti = fl(g, &mut t);
    f.td = fl(g, &mut t);
    let tm = dbits(g);
    t.push(format!("{tm:016x}"));
    f.tm = f64::from_bits(tm);
    f.sequence_type = g.int(0, 5) as i32;
    t.push(f.sequence_type.to_string());
    f.echo_spacing = fl(g, &mut t);
    f.echo_train_len = fl(g, &mut t);
    for i in 0..3 {
        f.read_dir[i] = fl(g, &mut t);
    }
    for i in 0..3 {
        f.pe_dir[i] = fl(g, &mut t);
    }
    for i in 0..3 {
        f.slice_dir[i] = fl(g, &mut t);
    }
    f.label = g.int(-5, 5000) as i32;
    t.push(f.label.to_string());
    let (a, b) = (name(g), name(g));
    t.push(ename(&a));
    t.push(ename(&b));
    f.name = CName::new(&a);
    f.name.strcpy_bytes(&b);
    f.dof = g.int(0, 99) as i32;
    t.push(f.dof.to_string());
    match g.below(12) {
        0 => {
            t.push("0".into());
            f.m_ras2vox = None;
        }
        1 => {
            let (mt, m, _) = gen_mat(g, 3);
            t.extend(mt);
            f.m_ras2vox = Some(m);
        }
        _ => {
            let (mt, m, _) = gen_mat(g, 4);
            t.extend(mt);
            f.m_ras2vox = Some(m);
        }
    }
    f.thresh = fl(g, &mut t);
    f.units = g.int(0, 3) as i32;
    t.push(f.units.to_string());
    let mut dv = [0f64; 7];
    for x in &mut dv {
        let b = dbits(g);
        t.push(format!("{b:016x}"));
        *x = f64::from_bits(b);
    }
    [f.dx, f.dy, f.dz, f.dr, f.dp, f.ds, f.bvalue] = dv;
    let mut l = [0i64; 8];
    let mut a = [0f64; 4];
    for i in 0..4 {
        for k in 0..2 {
            l[2 * i + k] = if g.chance(4) {
                g.next() as i64
            } else {
                g.int(-9, 99999)
            };
            t.push(l[2 * i + k].to_string());
        }
        let b = dbits(g);
        t.push(format!("{b:016x}"));
        a[i] = f64::from_bits(b);
    }
    [
        f.d1_ramp, f.d1_flat, f.d2_ramp, f.d2_flat, f.d3_ramp, f.d3_flat, f.d4_ramp, f.d4_flat,
    ] = l;
    [f.d1_amp, f.d2_amp, f.d3_amp, f.d4_amp] = a;
    (t, f)
}

/// A file cut or changed now and then.
fn mutate(g: &mut Gen, d: &mut Vec<u8>) {
    if d.is_empty() {
        return;
    }
    match g.below(8) {
        0 => {
            let n = g.below(d.len() as u64) as usize;
            d.truncate(n);
        }
        1 => {
            let i = g.below(d.len() as u64) as usize;
            d[i] = g.below(256) as u8;
        }
        2 => d.extend((0..g.int(1, 20)).map(|_| g.below(256) as u8)),
        _ => {}
    }
}

fn mode_tok(g: &mut Gen) -> i64 {
    g.pick(&[0, 1, 1, 2])
}

fn gdiag(g: &mut Gen) -> u64 {
    if g.chance(4) {
        g.pick(&[DIAG_INFO, DIAG_SHOW, DIAG_INFO | DIAG_SHOW])
    } else {
        0
    }
}

/// One item of an FStagsIO tag stream: its bytes and the reader's script.
fn fsr_item(g: &mut Gen, d: &mut Vec<u8>, script: &mut Vec<String>, nifti: bool) {
    let head = |d: &mut Vec<u8>, tag: i32, len: i64, lengthless: bool| {
        be32(d, tag);
        if nifti || !lengthless {
            be64(d, len);
        }
    };
    script.push("t".into());
    script.push("0".into());
    match g.below(13) {
        0 => {
            let (_, _, bits) = gen_mat(g, 4);
            if nifti {
                head(d, 44, 64, false);
                for b in bits {
                    be32(d, b as i32);
                }
            } else {
                head(d, 44, 1600, false);
                d.extend(mat_text(g, &bits));
            }
            script.push("m".into());
        }
        1 => {
            let (_, ct) = gen_ct(g);
            let mut body = Vec::new();
            ser_ct(g, &mut body, &ct);
            head(d, 1, body.len() as i64, true);
            d.extend(body);
            script.push("c".into());
        }
        2 => {
            let n = g.int(0, 2);
            let mut body = Vec::new();
            for _ in 0..n {
                let (_, f) = gen_frame(g);
                ser_frame(g, &mut body, &f, nifti);
            }
            let len = if nifti { body.len() as i64 } else { 12880 * n };
            if !nifti {
                body.resize(len as usize, 0);
            }
            let given = if g.chance(10) {
                len + g.int(-20, 40)
            } else {
                len
            };
            head(d, 42, len, false);
            d.extend(body);
            script.extend(["f".into(), n.to_string(), given.to_string()]);
        }
        3 => {
            let shearless = g.chance(2);
            let tag = if shearless { 10 } else { 15 };
            let (_, a) = gen_vg(g);
            let (_, b) = gen_vg(g);
            let mut body = Vec::new();
            ser_vg(&mut body, &a, nifti, shearless);
            ser_vg(&mut body, &b, nifti, shearless);
            head(d, tag, body.len() as i64, shearless);
            d.extend(body);
            script.push("g".into());
            script.push(i32::from(shearless).to_string());
            for _ in 0..2 {
                let (t0, _) = gen_vg(g);
                script.extend(t0);
            }
        }
        4 => {
            head(d, 13, 12, false);
            be32(d, g.int(0, 3) as i32);
            be32(d, g.int(1, 8) as i32);
            be32(d, fbits(g) as i32);
            script.push("e".into());
        }
        5 => {
            let (x, y, z) = (g.int(0, 3), g.int(0, 3), g.int(0, 3));
            head(d, 12, x * y * z * 4, true);
            for _ in 0..x * y * z {
                be32(d, g.int(-1, 99) as i32);
            }
            script.extend(["l".into(), x.to_string(), y.to_string(), z.to_string()]);
        }
        6 => {
            head(d, 7, 4, false);
            be32(d, g.int(0, 99) as i32);
            script.push("o".into());
        }
        7 => {
            let pe = name(g);
            let mut body = Vec::new();
            for _ in 0..2 {
                be32(&mut body, fbits(g) as i32);
            }
            be64(&mut body, dbits(g) as i64);
            be32(&mut body, fbits(g) as i32);
            body.extend_from_slice(&pe);
            if g.chance(2) {
                body.push(0);
            }
            let len = body.len() as i64;
            let given = match g.below(12) {
                0 => g.int(0, 19),
                1 => len + 30,
                _ => len,
            };
            head(d, 45, len, false);
            d.extend(body);
            script.extend(["s".into(), given.to_string()]);
        }
        8 | 9 => {
            let (tag, n) = if g.chance(2) { (8, 12) } else { (9, 9) };
            head(d, tag, n * 4, false);
            for _ in 0..n {
                be32(
                    d,
                    if g.chance(3) {
                        g.int(-3, 300) as i32
                    } else {
                        fbits(g) as i32
                    },
                );
            }
            script.push(if tag == 8 { "r" } else { "x" }.into());
        }
        10 => {
            let n = g.int(0, 40);
            head(d, 99, n, false);
            d.extend((0..n).map(|_| g.below(256) as u8));
            let len = match g.below(15) {
                0 => -1,
                1 => g.pick(&[1i64 << 32, 1 << 40, -(1 << 33)]),
                2 => n + 100,
                _ => n,
            };
            script.extend(["k".into(), "99".into(), len.to_string()]);
        }
        11 => {
            let n = g.int(0, 30);
            head(d, 3, n, false);
            d.extend((0..n).map(|_| g.int(32, 126) as u8));
            script.extend(["d".into(), n.to_string()]);
        }
        _ => {
            // TAG_MGH_XFORM without a length (m3z's form), read with tagwithzerolen
            be32(d, 31);
            let n = g.int(0, 20);
            d.extend((0..n).map(|_| g.int(32, 126) as u8));
            script.pop();
            script.push("31".into());
            script.extend(["d".into(), n.to_string()]);
        }
    }
}

/// A frame as the writers store it (the matrix as its text tag in an `.mgz`).
fn ser_frame(g: &mut Gen, d: &mut Vec<u8>, f: &MriFrame, nifti: bool) {
    if nifti {
        be32(d, f.label);
        d.extend_from_slice(f.name.buffer());
        bef(d, f.thresh);
        return;
    }
    be32(d, f.ftype);
    for x in [f.te, f.tr, f.flip, f.ti, f.td, f.tm as f32] {
        bef(d, x);
    }
    be32(d, f.sequence_type);
    bef(d, f.echo_spacing);
    bef(d, f.echo_train_len);
    for x in f.read_dir.iter().chain(&f.pe_dir).chain(&f.slice_dir) {
        bef(d, *x);
    }
    be32(d, f.label);
    d.extend_from_slice(f.name.buffer());
    be32(d, f.dof);
    be32(d, 0);
    be64(d, 1600);
    let mut bits = [0u32; 16];
    for (i, b) in bits.iter_mut().enumerate() {
        *b = if i % 5 == 0 { 0x3f80_0000 } else { fbits(g) };
    }
    d.extend(mat_text(g, &bits));
    bef(d, f.thresh);
    be32(d, f.units);
    if f.ftype == 1 {
        for x in [f.dx, f.dy, f.dz, f.dr, f.dp, f.ds, f.bvalue, f.tm] {
            d.extend_from_slice(&x.to_bits().to_be_bytes());
        }
        for (r, fl, a) in [
            (f.d1_ramp, f.d1_flat, f.d1_amp),
            (f.d2_ramp, f.d2_flat, f.d2_amp),
            (f.d3_ramp, f.d3_flat, f.d3_amp),
            (f.d4_ramp, f.d4_flat, f.d4_amp),
        ] {
            be64(d, r);
            be64(d, fl);
            d.extend_from_slice(&a.to_bits().to_be_bytes());
        }
    }
}

/// A piece-2 case of command `kind`.
pub fn generate(g: &mut Gen, kind: &str) -> Vec<String> {
    let mut t: Vec<String> = vec![kind.into()];
    match kind {
        "vgw" => {
            t.extend([g.int(0, 1), g.int(0, 1), g.int(0, 1)].map(|x| x.to_string()));
            t.extend(gen_vg(g).0);
        }
        "vgr" => {
            let (nifti, shearless) = (g.chance(2), g.chance(2));
            t.extend([
                mode_tok(g).to_string(),
                i32::from(nifti).to_string(),
                i32::from(shearless).to_string(),
                g.pick(&["-1", "0"]).into(),
            ]);
            let (t0, _) = gen_vg(g);
            t.extend(t0);
            let (_, v) = gen_vg(g);
            let mut d = Vec::new();
            ser_vg(&mut d, &v, nifti, shearless);
            if nifti && g.chance(6) {
                // a name length beyond 512
                let at = d.len() - v.fname.len().min(512) - 4;
                let n = g.pick(&[513i32, 700, 4000, 20_000_000, -5, 0]);
                d.truncate(at);
                be32(&mut d, n);
                d.extend(std::iter::repeat_n(b'z', (n.clamp(0, 5000)) as usize));
            }
            mutate(g, &mut d);
            t.push(hex(&d));
        }
        "fsw" => {
            let op = g.pick(&[
                "tag", "mat", "ctab", "frames", "frames", "geom", "geom", "meta", "labels", "dof",
                "scan", "ras", "rasx", "end",
            ]);
            t.push(op.into());
            t.push(g.int(0, 1).to_string());
            t.push(g.int(0, 1).to_string());
            t.push(gdiag(g).to_string());
            match op {
                "tag" => {
                    t.push(g.pick(&[3, 1, 10, 31, 44, 99, 0]).to_string());
                    let d: Vec<u8> = (0..g.int(0, 30)).map(|_| g.below(256) as u8).collect();
                    let n = d.len() as i64;
                    t.push(hex(&d));
                    t.push((if g.chance(5) { g.int(0, n) } else { n }).to_string());
                }
                "mat" => {
                    t.push(g.pick(&[0, 44, 31]).to_string());
                    let k = if g.chance(15) { 3 } else { 4 };
                    t.extend(gen_mat(g, k).0);
                }
                "ctab" => t.extend(gen_ct(g).0),
                "frames" => {
                    let n = g.int(0, 2);
                    t.push(n.to_string());
                    for _ in 0..n {
                        t.extend(gen_frame(g).0);
                    }
                }
                "geom" => {
                    t.push(g.int(0, 1).to_string());
                    t.extend(gen_vg(g).0);
                    t.extend(gen_vg(g).0);
                }
                "meta" => {
                    t.push(g.int(0, 3).to_string());
                    t.push(g.int(1, 8).to_string());
                    t.push(format!("{:016x}", dbits(g)));
                }
                "labels" => {
                    let (x, y, z) = (g.int(0, 3), g.int(0, 3), g.int(0, 3));
                    t.extend([x, y, z].map(|v| v.to_string()));
                    for _ in 0..x * y * z {
                        t.push(g.int(-1, 99).to_string());
                    }
                }
                "dof" => t.push(g.int(-1, 99).to_string()),
                "scan" => {
                    for _ in 0..2 {
                        t.push(format!("{:08x}", fbits(g)));
                    }
                    t.push(format!("{:016x}", dbits(g)));
                    t.push(format!("{:08x}", fbits(g)));
                    t.push(if g.chance(4) {
                        "N".into()
                    } else {
                        ename(&name(g))
                    });
                }
                "ras" | "rasx" => t.extend(gen_vg(g).0),
                _ => {}
            }
        }
        "fsr" => {
            let nifti = g.chance(2);
            t.extend([
                mode_tok(g).to_string(),
                i32::from(nifti).to_string(),
                gdiag(g).to_string(),
                g.pick(&["-1", "0"]).into(),
            ]);
            let mut d = Vec::new();
            let mut script = Vec::new();
            for _ in 0..g.int(1, 3) {
                fsr_item(g, &mut d, &mut script, nifti);
            }
            if g.chance(4) {
                script.extend(["t".into(), "0".into()]);
            }
            mutate(g, &mut d);
            t.push(hex(&d));
            t.extend(script);
        }
        "fsl" => {
            let op = g.pick(&[
                "tag", "mat", "ctab", "frames", "geom", "meta", "labels", "dof", "scan", "ras",
                "rasx", "end",
            ]);
            t.push(op.into());
            let b = |g: &mut Gen| g.int(0, 1).to_string();
            match op {
                "tag" => {
                    t.push(g.pick(&[1, 3, 10, 11, 12, 13, 44]).to_string());
                    t.push(g.int(-5, 3000).to_string());
                    t.push(b(g));
                    t.push(b(g));
                }
                "mat" | "end" => {
                    if op == "mat" {
                        t.push(b(g));
                    }
                    t.push(b(g));
                }
                "ctab" => {
                    t.push(b(g));
                    t.push(b(g));
                    t.extend(gen_ct(g).0);
                }
                "frames" => {
                    t.push(g.int(0, 5).to_string());
                    t.push(b(g));
                    t.push(b(g));
                }
                "geom" => {
                    t.push(ename(&name(g)));
                    t.push(ename(&name(g)));
                    t.push(b(g));
                    t.push(b(g));
                    t.push(b(g));
                }
                "labels" => {
                    let big = g.chance(4);
                    for _ in 0..3 {
                        t.push(if big { g.int(800, 2000) } else { g.int(0, 300) }.to_string());
                    }
                    t.push(g.pick(&[1, 4]).to_string());
                    t.push(b(g));
                    t.push(b(g));
                }
                "scan" => {
                    t.push(b(g));
                    t.push(if g.chance(3) {
                        "N".into()
                    } else {
                        ename(&name(g))
                    });
                }
                _ => t.push(b(g)),
            }
        }
        "ctw" => {
            t.extend([g.int(0, 1), g.int(0, 1), gdiag(g) as i64].map(|x| x.to_string()));
            t.extend(gen_ct(g).0);
        }
        "ctr" => {
            t.extend([g.int(0, 1), mode_tok(g), gdiag(g) as i64].map(|x| x.to_string()));
            t.push(g.pick(&["-1", "0"]).into());
            let (_, ct) = gen_ct(g);
            let mut d = Vec::new();
            ser_ct(g, &mut d, &ct);
            mutate(g, &mut d);
            t.push(hex(&d));
        }
        "tw" | "ztw" => {
            if kind == "ztw" {
                t.push(g.int(0, 1).to_string());
            }
            for _ in 0..g.int(1, 3) {
                match g.below(if kind == "tw" { 4 } else { 3 }) {
                    0 => {
                        t.push("s".into());
                        t.push(g.pick(&[3, 20, 22, 32, 44]).to_string());
                        t.push(g.pick(&[0i64, 4, 1600, -1, 1 << 40]).to_string());
                    }
                    1 => {
                        t.push("m".into());
                        t.push(g.pick(&[23, 0, 33]).to_string());
                        let k = if g.chance(20) { 3 } else { 4 };
                        t.extend(gen_mat(g, k).0);
                    }
                    2 => t.push("e".into()),
                    _ => {
                        t.push("w".into());
                        t.push(g.pick(&[3, 22, 24]).to_string());
                        let d: Vec<u8> = (0..g.int(0, 20)).map(|_| g.int(32, 126) as u8).collect();
                        let n = d.len();
                        t.push(hex(&d));
                        t.push(n.to_string());
                    }
                }
            }
        }
        "tr" | "ztr" => {
            let zn = kind == "ztr";
            if zn {
                t.push(mode_tok(g).to_string());
            }
            t.push(g.pick(&["-1", "0"]).into());
            let mut d = Vec::new();
            let mut s: Vec<String> = Vec::new();
            for _ in 0..g.int(1, 3) {
                match g.below(5) {
                    0 | 1 => {
                        let (_, _, bits) = gen_mat(g, 4);
                        let mut text = mat_text(g, &bits);
                        if !zn {
                            let n = text.iter().position(|&c| c == 0).unwrap_or(text.len());
                            text.truncate(n);
                            if g.chance(30) {
                                text = vec![b'W'; 1700];
                            }
                        }
                        be32(&mut d, 23);
                        be64(&mut d, text.len() as i64);
                        d.extend(text);
                        if zn {
                            s.push("m".into());
                        } else {
                            s.extend(["t".into(), "m".into()]);
                        }
                    }
                    2 => {
                        let tag = g.pick(&[20, 2, 1, 30, 10, 11, 12]);
                        be32(&mut d, tag);
                        if tag == 30 {
                            be32(&mut d, g.int(0, 9) as i32);
                        }
                        s.push("t".into());
                        if zn {
                            s.push(g.pick(&["0", "0", "31", "10"]).into());
                        }
                    }
                    _ => {
                        let n = g.int(0, 25);
                        be32(&mut d, 3);
                        be64(&mut d, n);
                        d.extend((0..n).map(|_| g.int(32, 126) as u8));
                        s.push("t".into());
                        if zn {
                            s.push("0".into());
                        } else if g.chance(2) {
                            s.extend(["d".into(), n.to_string()]);
                        } else {
                            let len = match g.below(10) {
                                0 => -1,
                                1 => 1 << 40,
                                _ => n,
                            };
                            s.extend(["k".into(), "3".into(), len.to_string()]);
                        }
                    }
                }
            }
            if zn {
                s.push("t".into());
                s.push("0".into());
            }
            mutate(g, &mut d);
            t.push(hex(&d));
            t.extend(s);
        }
        _ => {
            for _ in 0..g.int(0, 5) {
                let a: Vec<u8> = if g.chance(20) {
                    vec![b'a'; 2100]
                } else {
                    (0..g.int(0, 12)).map(|_| g.int(33, 126) as u8).collect()
                };
                t.push(enc(Some(&a)));
            }
        }
    }
    t
}

// ---------------------------------------------------------------- the port

struct Toks<'a> {
    t: &'a [&'a str],
    i: usize,
}

impl<'a> Toks<'a> {
    fn s(&mut self) -> &'a str {
        let x = self.t[self.i];
        self.i += 1;
        x
    }
    fn i(&mut self) -> i64 {
        self.s().parse().unwrap()
    }
    fn f(&mut self) -> f32 {
        f32::from_bits(u32::from_str_radix(self.s(), 16).unwrap())
    }
    fn d(&mut self) -> f64 {
        f64::from_bits(u64::from_str_radix(self.s(), 16).unwrap())
    }
    fn b(&mut self) -> Vec<u8> {
        dec(self.s()).unwrap_or_default()
    }
    fn more(&self) -> bool {
        self.i < self.t.len()
    }
}

fn read_vg(t: &mut Toks) -> VolGeom {
    let mut v = VolGeom {
        valid: t.i() as i32,
        width: t.i() as i32,
        height: t.i() as i32,
        depth: t.i() as i32,
        ..VolGeom::default()
    };
    let mut f = [0u32; 18];
    for x in &mut f {
        *x = t.f().to_bits();
    }
    set_floats(&mut v, &f);
    v.fname = CName::new(&t.b());
    v.fname.strcpy_bytes(&t.b());
    v
}

fn read_mat(t: &mut Toks) -> Option<Matrix> {
    let k = t.i() as i32;
    if k == 0 {
        return None;
    }
    let mut m = alloc(k);
    for r in 1..=k {
        for c in 1..=k {
            m.set_relt(r, c, t.f());
        }
    }
    Some(m)
}

fn read_ct(t: &mut Toks) -> ColorTable {
    let version = t.i() as i32;
    let n = t.i() as i32;
    let fname = t.b();
    let mut ct = ColorTable {
        entries: vec![None; n.max(0) as usize],
        fname,
        version,
        ..ColorTable::default()
    };
    for i in 0..n.max(0) as usize {
        if t.i() == 0 {
            continue;
        }
        let name = t.b();
        let (ri, gi, bi, ai) = (t.i() as i32, t.i() as i32, t.i() as i32, t.i() as i32);
        ct.entries[i] = Some(Box::new(ColorTableEntry {
            name,
            ri,
            gi,
            bi,
            ai,
            ..ColorTableEntry::default()
        }));
    }
    ct
}

fn read_frame(t: &mut Toks, f: &mut MriFrame) {
    f.ftype = t.i() as i32;
    f.te = t.f();
    f.tr = t.f();
    f.flip = t.f();
    f.ti = t.f();
    f.td = t.f();
    f.tm = t.d();
    f.sequence_type = t.i() as i32;
    f.echo_spacing = t.f();
    f.echo_train_len = t.f();
    for i in 0..3 {
        f.read_dir[i] = t.f();
    }
    for i in 0..3 {
        f.pe_dir[i] = t.f();
    }
    for i in 0..3 {
        f.slice_dir[i] = t.f();
    }
    f.label = t.i() as i32;
    f.name.strcpy_bytes(&t.b());
    f.name.strcpy_bytes(&t.b());
    f.dof = t.i() as i32;
    f.m_ras2vox = read_mat(t);
    f.thresh = t.f();
    f.units = t.i() as i32;
    f.dx = t.d();
    f.dy = t.d();
    f.dz = t.d();
    f.dr = t.d();
    f.dp = t.d();
    f.ds = t.d();
    f.bvalue = t.d();
    let mut l = [0i64; 8];
    let mut a = [0f64; 4];
    for i in 0..4 {
        l[2 * i] = t.i();
        l[2 * i + 1] = t.i();
        a[i] = t.d();
    }
    [
        f.d1_ramp, f.d1_flat, f.d2_ramp, f.d2_flat, f.d3_ramp, f.d3_flat, f.d4_ramp, f.d4_flat,
    ] = l;
    [f.d1_amp, f.d2_amp, f.d3_amp, f.d4_amp] = a;
}

impl Out {
    fn f(&mut self, v: f32) {
        self.x(&v.to_bits().to_be_bytes(), 99);
    }
    fn d(&mut self, v: f64) {
        self.x(&v.to_bits().to_be_bytes(), 99);
    }
    /// A buffer up to its last non-zero byte.
    fn buf(&mut self, b: &[u8]) {
        let n = b.iter().rposition(|&c| c != 0).map_or(0, |i| i + 1);
        self.0.push_str(&enc(Some(&b[..n])));
        self.0.push(' ');
    }
    /// A C string of at most `STRLEN` bytes.
    fn cs(&mut self, b: &[u8]) {
        let n = b
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(b.len())
            .min(STRLEN);
        self.0.push_str(&enc(Some(&b[..n])));
        self.0.push(' ');
    }
    fn vg(&mut self, v: &VolGeom) {
        for x in [v.valid, v.width, v.height, v.depth] {
            self.i(i64::from(x));
        }
        for x in floats_of(v) {
            self.f(x);
        }
        self.buf(v.fname.buffer());
    }
    fn mat(&mut self, m: Option<&Matrix>) {
        let Some(m) = m else {
            self.0.push_str("N ");
            return;
        };
        self.i(i64::from(m.rows));
        for r in 1..=m.rows {
            for c in 1..=m.cols {
                self.f(m.relt(r, c));
            }
        }
    }
    fn ct(&mut self, ct: Option<&ColorTable>) {
        let Some(ct) = ct else {
            self.0.push_str("N ");
            return;
        };
        self.i(i64::from(ct.nentries()));
        self.i(i64::from(ct.version));
        self.cs(&ct.fname);
        // the non-empty entries, each after its index (as fsio prints them)
        for (i, e) in ct.entries.iter().enumerate() {
            match e {
                None => {}
                Some(e) => {
                    self.0.push_str(&format!("@{i} "));
                    self.cs(&e.name);
                    for x in [e.ri, e.gi, e.bi, e.ai] {
                        self.i(i64::from(x));
                    }
                    for x in [e.rf, e.gf, e.bf, e.af] {
                        self.f(x);
                    }
                }
            }
        }
    }
    fn frame(&mut self, f: &MriFrame) {
        self.i(i64::from(f.ftype));
        for x in [f.te, f.tr, f.flip, f.ti, f.td] {
            self.f(x);
        }
        self.d(f.tm);
        self.i(i64::from(f.sequence_type));
        self.f(f.echo_spacing);
        self.f(f.echo_train_len);
        for x in f.read_dir.iter().chain(&f.pe_dir).chain(&f.slice_dir) {
            self.f(*x);
        }
        self.i(i64::from(f.label));
        self.buf(f.name.buffer());
        self.i(i64::from(f.dof));
        self.mat(f.m_ras2vox.as_ref());
        self.f(f.thresh);
        self.i(i64::from(f.units));
        for x in [f.dx, f.dy, f.dz, f.dr, f.dp, f.ds, f.bvalue] {
            self.d(x);
        }
        for (r, fl, a) in [
            (f.d1_ramp, f.d1_flat, f.d1_amp),
            (f.d2_ramp, f.d2_flat, f.d2_amp),
            (f.d3_ramp, f.d3_flat, f.d3_amp),
            (f.d4_ramp, f.d4_flat, f.d4_amp),
        ] {
            self.i(r);
            self.i(fl);
            self.d(a);
        }
    }
}

/// The data file of mode 0 (plain), 1 (compressed) or 2 (plain, opened compressed).
fn data_znz(w: &Work, mode: i64, b: &[u8]) -> ZnzFile {
    let path = w.dat.to_str().unwrap().as_bytes().to_vec();
    if mode == 1 {
        let mut z = ZnzFile::open(&path, b"wb", true).unwrap();
        if !b.is_empty() {
            fio::fs_znzwrite(&mut z, b, 1, b.len());
        }
        z.close();
    } else {
        std::fs::write(&w.dat, b).unwrap();
    }
    ZnzFile::open(&path, b"rb", mode != 0).unwrap()
}

/// The decompressed bytes of the data file.
fn back(w: &Work, gz: bool) -> Vec<u8> {
    let path = w.dat.to_str().unwrap().as_bytes().to_vec();
    let mut z = ZnzFile::open(&path, b"rb", gz).unwrap();
    let mut v = Vec::new();
    let mut c = [0u8; 1];
    while fio::fs_znzread(&mut z, &mut c, 1, 1) == 1 {
        v.push(c[0]);
    }
    z.close();
    v
}

fn header(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    nframes: i32,
) -> Result<Mri, Fatal> {
    mri::mri_alloc_header(g, out, err, 1, 1, 1, MRI_UCHAR, nframes)
}

/// The port's answer to a piece-2 case; `extra` takes `SKIP` / `VARY` where the inputs make
/// FreeSurfer's line not comparable.
pub fn run(
    tk: &[&str],
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
    w: &Work,
    extra: &mut Vec<&'static str>,
) -> Result<(), u8> {
    let mut t = Toks { t: &tk[1..], i: 0 };
    let path = w.dat.to_str().unwrap().as_bytes().to_vec();
    let fatal = |f: Fatal| f.status();
    match tk[0] {
        "vgw" => {
            let gz = t.i() != 0;
            let (nifti, shearless) = (t.i() != 0, t.i() != 0);
            let v = read_vg(&mut t);
            let mut z = ZnzFile::open(&path, b"wb", gz).unwrap();
            fstagsio::vol_geom_write(&mut z, &v, nifti, shearless);
            z.close();
            o.bytes(&back(w, gz));
        }
        "vgr" => {
            let mode = t.i();
            let (nifti, shearless) = (t.i() != 0, t.i() != 0);
            g.gdiag_no = t.i() as i32;
            let mut v = read_vg(&mut t);
            let mut z = data_znz(w, mode, &unhex(t.s()));
            fstagsio::vol_geom_read(g, err, &mut z, &mut v, nifti, shearless);
            o.vg(&v);
            o.i(i64::from(z.eof()));
            o.i(z.tell());
            z.close();
        }
        "fsw" => {
            let op = t.s();
            let gz = t.i() != 0;
            let nifti = t.i() != 0;
            g.gdiag = t.i() as u64;
            let mut z = ZnzFile::open(&path, b"wb", gz).unwrap();
            let r = {
                let mut io = FsTagsIo::new(&mut z, nifti);
                match op {
                    "tag" => {
                        let tag = t.i() as i32;
                        let d = unhex(t.s());
                        let dlen = t.i();
                        io.write_tag(g, out, tag, &d, dlen)
                    }
                    "mat" => {
                        let tag = t.i() as i32;
                        let m = read_mat(&mut t).unwrap();
                        io.write_matrix(g, out, err, &m, tag)
                    }
                    "ctab" => {
                        let ct = read_ct(&mut t);
                        io.write_old_colortable(g, out, err, &ct)
                    }
                    "frames" => {
                        let n = t.i() as i32;
                        let mut mri = header(g, out, err, n).map_err(fatal)?;
                        for f in mri.frames.iter_mut() {
                            read_frame(&mut t, f);
                        }
                        io.write_mri_frames(g, out, err, &mri).map_err(fatal)?
                    }
                    "geom" => {
                        let sh = t.i() != 0;
                        let (a, b) = (read_vg(&mut t), read_vg(&mut t));
                        io.write_gcamorph_geom(g, out, err, &a, &b, sh)
                            .map_err(fatal)?
                    }
                    "meta" => {
                        let (a, b) = (t.i() as i32, t.i() as i32);
                        let k = t.d();
                        io.write_gcamorph_meta(g, out, a, b, k)
                    }
                    "labels" => {
                        let (x, y, zz) = (t.i() as i32, t.i() as i32, t.i() as i32);
                        let l: Vec<i32> = (0..x * y * zz).map(|_| t.i() as i32).collect();
                        io.write_gcamorph_labels(g, out, x, y, zz, &l)
                    }
                    "dof" => {
                        let d = t.i() as i32;
                        io.write_dof(g, out, d)
                    }
                    "scan" => {
                        let mut mri = header(g, out, err, 1).map_err(fatal)?;
                        mri.te = t.f();
                        mri.ti = t.f();
                        mri.flip_angle = t.d();
                        mri.field_strength = t.f();
                        mri.pedir = dec(t.s());
                        io.write_scan_parameters(g, out, &mri)
                    }
                    "ras" | "rasx" => {
                        let v = read_vg(&mut t);
                        if op == "ras" {
                            io.write_ras_xform(g, out, &v)
                        } else {
                            io.write_ras_xform_extra(g, out, &v)
                        }
                    }
                    _ => io.write_endtag(g, out),
                }
            };
            z.close();
            o.i(i64::from(r));
            o.bytes(&back(w, gz));
        }
        "fsr" => {
            let mode = t.i();
            let nifti = t.i() != 0;
            g.gdiag = t.i() as u64;
            g.gdiag_no = t.i() as i32;
            let mut z = data_znz(w, mode, &unhex(t.s()));
            {
                let mut io = FsTagsIo::new(&mut z, nifti);
                while t.more() {
                    let op = t.s();
                    o.0.push_str(&format!("{op}: "));
                    match op {
                        "t" => {
                            let zl = t.i() as i32;
                            let mut len = -7777;
                            o.i(i64::from(io.read_tagid_len(g, err, &mut len, zl)));
                            o.i(len);
                        }
                        "d" => {
                            let n = t.i();
                            let mut b = vec![0xeeu8; n as usize];
                            o.i(i64::from(io.read_data(&mut b, n)));
                            o.bytes(&b);
                        }
                        "m" => {
                            g.errno = 0;
                            let m = io.read_matrix(g, out, err);
                            o.mat(Some(&m));
                            o.i(i64::from(g.errno));
                        }
                        "c" => {
                            let ct = io.read_old_colortable(g, out, err);
                            o.ct(ct.as_ref());
                        }
                        "f" => {
                            let n = t.i() as i32;
                            let len = t.i();
                            let mut mri = header(g, out, err, n).map_err(fatal)?;
                            o.i(i64::from(io.read_mri_frames(g, out, err, &mut mri, len)));
                            for f in &mri.frames {
                                o.frame(f);
                            }
                        }
                        "g" => {
                            let sh = t.i() != 0;
                            let (mut a, mut b) = (read_vg(&mut t), read_vg(&mut t));
                            o.i(i64::from(io.read_gcamorph_geom(g, err, &mut a, &mut b, sh)));
                            o.vg(&a);
                            o.vg(&b);
                        }
                        "e" => {
                            let (a, b, c) = io.read_gcamorph_meta(g, err);
                            o.i(0);
                            o.i(i64::from(a));
                            o.i(i64::from(b));
                            o.d(c);
                        }
                        "l" => {
                            let (x, y, zz) = (t.i() as i32, t.i() as i32, t.i() as i32);
                            let mut l = vec![-1i32; (x * y * zz).max(0) as usize];
                            o.i(i64::from(io.read_gcamorph_labels(g, err, x, y, zz, &mut l)));
                            for v in l {
                                o.i(i64::from(v));
                            }
                        }
                        "o" => {
                            let d = io.read_dof(g, err);
                            o.i(0);
                            o.i(i64::from(d));
                        }
                        "s" => {
                            let dlen = t.i();
                            let mut mri = header(g, out, err, 1).map_err(fatal)?;
                            o.i(i64::from(
                                io.read_scan_parameters(g, out, err, &mut mri, dlen),
                            ));
                            o.f(mri.te);
                            o.f(mri.ti);
                            o.d(mri.flip_angle);
                            o.f(mri.field_strength);
                            match &mri.pedir {
                                Some(p) => o.cs(p),
                                None => o.0.push_str("N "),
                            }
                        }
                        "r" | "x" => {
                            let mut v = VolGeom::default();
                            let r = if op == "r" {
                                io.read_ras_xform(g, out, err, &mut v)
                            } else {
                                io.read_ras_xform_extra(g, out, err, &mut v)
                            };
                            o.i(i64::from(r));
                            o.vg(&v);
                        }
                        _ => {
                            let tag = t.i() as i32;
                            let len = t.i();
                            if len > 1 << 28 {
                                extra.push("SKIP");
                            }
                            let r = io.skip_tag(g, err, tag, len).map_err(fatal)?;
                            o.i(i64::from(r));
                        }
                    }
                }
            }
            o.i(i64::from(z.eof()));
            o.i(z.tell());
            z.close();
        }
        "fsl" => {
            let op = t.s();
            let b = |t: &mut Toks| t.i() != 0;
            let r = match op {
                "tag" => {
                    let tag = t.i() as i32;
                    let len = t.i();
                    let (n, a) = (b(&mut t), b(&mut t));
                    fstagsio::getlen_tag(tag, len, n, a)
                }
                "mat" => {
                    let (n, a) = (b(&mut t), b(&mut t));
                    fstagsio::getlen_matrix(n, a)
                }
                "ctab" => {
                    let (n, a) = (b(&mut t), b(&mut t));
                    let ct = read_ct(&mut t);
                    fstagsio::getlen_old_colortable(&ct, n, a)
                }
                "frames" => {
                    let nf = t.i() as i32;
                    let (n, a) = (b(&mut t), b(&mut t));
                    let mri = header(g, out, err, nf).map_err(fatal)?;
                    fstagsio::getlen_mri_frames(&mri, n, a)
                }
                "geom" => {
                    let (s, tt) = (t.b(), t.b());
                    let (n, a, sh) = (b(&mut t), b(&mut t), b(&mut t));
                    fstagsio::getlen_gcamorph_geom(g, &s, &tt, n, a, sh)
                }
                "meta" => fstagsio::getlen_gcamorph_meta(b(&mut t)),
                "labels" => {
                    let (x, y, zz, l) = (t.i() as i32, t.i() as i32, t.i() as i32, t.i() as i32);
                    let (n, a) = (b(&mut t), b(&mut t));
                    fstagsio::getlen_gcamorph_labels(g, x, y, zz, l, n, a)
                }
                "dof" => fstagsio::getlen_dof(b(&mut t)),
                "scan" => {
                    let a = b(&mut t);
                    let mut mri = header(g, out, err, 1).map_err(fatal)?;
                    mri.pedir = dec(t.s());
                    fstagsio::getlen_scan_parameters(&mri, a)
                }
                "ras" => fstagsio::getlen_ras_xform(b(&mut t)),
                "rasx" => fstagsio::getlen_ras_xform_extra(b(&mut t)),
                _ => fstagsio::getlen_endtag(b(&mut t)),
            };
            o.i(r);
        }
        "ctw" => {
            let stream = t.i();
            let gz = t.i() != 0;
            g.gdiag = t.i() as u64;
            let ct = read_ct(&mut t);
            if stream == 0 {
                let mut fp = CFile::open(None, w.dat.to_str().unwrap(), "wb").unwrap();
                let r = colortab_bin::ctab_write_into_binary(g, out, err, Some(&ct), Some(&mut fp));
                fp.close();
                o.i(i64::from(r));
                o.bytes(&std::fs::read(&w.dat).unwrap());
            } else {
                let mut z = ZnzFile::open(&path, b"wb", gz).unwrap();
                let r =
                    colortab_bin::znz_ctab_write_into_binary(g, out, err, Some(&ct), Some(&mut z));
                z.close();
                o.i(i64::from(r));
                o.bytes(&back(w, gz));
            }
        }
        "ctr" => {
            let stream = t.i();
            let mode = t.i();
            g.gdiag = t.i() as u64;
            g.gdiag_no = t.i() as i32;
            let b = unhex(t.s());
            if stream == 0 {
                std::fs::write(&w.dat, &b).unwrap();
                let mut fp = CFile::open(None, w.dat.to_str().unwrap(), "rb").unwrap();
                let ct = colortab_bin::ctab_read_from_binary(g, out, err, Some(&mut fp));
                o.ct(ct.as_ref());
                o.i(i64::from(fp.feof()));
                o.i(fp.tell());
                fp.close();
            } else {
                let mut z = data_znz(w, mode, &b);
                let ct = colortab_bin::znz_ctab_read_from_binary(g, out, err, Some(&mut z));
                o.ct(ct.as_ref());
                o.i(i64::from(z.eof()));
                o.i(z.tell());
                z.close();
            }
        }
        "tw" | "ztw" => {
            let zn = tk[0] == "ztw";
            let gz = zn && t.i() != 0;
            let mut fp = (!zn).then(|| CFile::open(None, w.dat.to_str().unwrap(), "wb").unwrap());
            let mut z = zn.then(|| ZnzFile::open(&path, b"wb", gz).unwrap());
            while t.more() {
                let op = t.s();
                o.0.push_str(&format!("{op}: "));
                match op {
                    "s" => {
                        let tag = t.i() as i32;
                        let len = t.i();
                        let mut here = -7777;
                        let r = match (&mut fp, &mut z) {
                            (Some(fp), _) => tags::tag_write_start(g, fp, tag, &mut here, len),
                            (_, Some(z)) => tags::znz_tag_write_start(g, z, tag, &mut here, len),
                            _ => unreachable!(),
                        };
                        o.i(i64::from(r));
                        o.i(here);
                    }
                    "w" => {
                        let tag = t.i() as i32;
                        let d = unhex(t.s());
                        let len = t.i();
                        o.i(i64::from(tags::tag_write(
                            g,
                            fp.as_mut().unwrap(),
                            tag,
                            &d,
                            len,
                        )));
                    }
                    "m" => {
                        let tag = t.i() as i32;
                        let m = read_mat(&mut t).unwrap();
                        let r = match (&mut fp, &mut z) {
                            (Some(fp), _) => tags::tag_write_matrix(g, fp, &m, tag),
                            (_, Some(z)) => tags::znz_write_matrix(g, z, &m, tag),
                            _ => unreachable!(),
                        };
                        o.i(i64::from(r));
                    }
                    _ => {
                        let r = match (&mut fp, &mut z) {
                            (Some(fp), _) => tags::tag_write_end(fp, 0),
                            (_, Some(z)) => tags::znz_tag_write_end(z, 0),
                            _ => unreachable!(),
                        };
                        o.i(i64::from(r));
                    }
                }
            }
            if let Some(fp) = fp {
                fp.close();
                o.bytes(&std::fs::read(&w.dat).unwrap());
            }
            if let Some(z) = z {
                z.close();
                o.bytes(&back(w, gz));
            }
        }
        "tr" | "ztr" => {
            let zn = tk[0] == "ztr";
            let mode = if zn { t.i() } else { 0 };
            g.gdiag_no = t.i() as i32;
            let b = unhex(t.s());
            let mut fp = None;
            let mut z = None;
            if zn {
                z = Some(data_znz(w, mode, &b));
            } else {
                std::fs::write(&w.dat, &b).unwrap();
                fp = Some(CFile::open(None, w.dat.to_str().unwrap(), "rb").unwrap());
            }
            while t.more() {
                let op = t.s();
                o.0.push_str(&format!("{op}: "));
                match op {
                    "t" => {
                        let mut len = -7777;
                        let r = match (&mut fp, &mut z) {
                            (Some(fp), _) => tags::tag_read_start(g, err, fp, &mut len),
                            (_, Some(z)) => {
                                let zl = t.i() as i32;
                                tags::znz_tag_read_start(g, err, z, &mut len, zl)
                            }
                            _ => unreachable!(),
                        };
                        o.i(i64::from(r));
                        o.i(len);
                    }
                    "k" => {
                        let tag = t.i() as i32;
                        let len = t.i();
                        if len > 1 << 28 {
                            extra.push("SKIP");
                        }
                        let r = tags::tag_skip(g, err, fp.as_mut().unwrap(), tag, len)
                            .map_err(fatal)?;
                        o.i(i64::from(r));
                    }
                    "d" => {
                        let n = t.i();
                        let mut d = vec![0xeeu8; n as usize];
                        o.i(i64::from(tags::tag_read(fp.as_mut().unwrap(), &mut d, n)));
                        o.bytes(&d);
                    }
                    _ => {
                        g.errno = 0;
                        let m = match (&mut fp, &mut z) {
                            (Some(fp), _) => tags::tag_read_matrix(g, fp),
                            (_, Some(z)) => tags::znz_read_matrix(g, err, z),
                            _ => unreachable!(),
                        };
                        o.mat(Some(&m));
                        o.i(i64::from(g.errno));
                    }
                }
            }
            if let Some(mut fp) = fp {
                o.i(i64::from(fp.feof()));
                o.i(fp.tell());
                fp.close();
            }
            if let Some(mut z) = z {
                o.i(i64::from(z.eof()));
                o.i(z.tell());
                z.close();
            }
        }
        _ => {
            let mut args = Vec::new();
            while t.more() {
                args.push(t.b());
            }
            let argv: Vec<&[u8]> = args.iter().map(Vec::as_slice).collect();
            let line = tags::tag_make_command_line_string(g, &argv, 4096);
            o.i(0);
            o.0.push_str(&enc(Some(&line)));
            o.0.push(' ');
        }
    }
    Ok(())
}
