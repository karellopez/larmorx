// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.io probe's piece-14 cases (track A relay 23): `mgw`, `mghWrite` (MGH and MGZ files) with
//! `MRITAGwrite`, called directly (a chosen frame, an intent) or through `MRIwrite` (b-values beside
//! the volume). The volume carries every field the writer writes: extent, every type (the six the
//! writer handles, and long, RGB and tensor, which it refuses), frames, voxels or none, geometry,
//! RAS flag, scan parameters, field strength, degrees of freedom, its buffer's bytes, the transform's
//! name, the phase-encoding direction, command lines, the first frame's label and threshold, the
//! auto-align and original RAS-to-voxel matrices, a GCA morph's fields, a colour table, b-values.
//! After the write: the status, `errno`, the volume's intent, version and voxels (FS-443), every
//! file under `w` (an `.mgz` inflated: piece 12's `odir`), and, when asked, the file read back with
//! `mghRead` (piece 6's printing).

use std::io::Write;

use larmorx_freesurfer_core::cname::CName;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_io::{mriio, mriio_write};
use larmorx_freesurfer_mri::mri::{self, Mri};
use larmorx_freesurfer_mri::vol_geom::VolGeom;

use super::piece4::setup;
use super::piece10::{Toks, read_table, table};
use super::piece11::{f32_of, f64_of, fnv};
use super::piece12::{fhex, odir};
use super::{Gen, Out, Work, enc, hex};

/// The piece-14 commands.
pub const KINDS: [&str; 1] = ["mgw"];

// ---------------------------------------------------------------- generation

fn dhex(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

/// A float of the scan parameters: usual values, now and then an odd one.
fn param(g: &mut Gen) -> f32 {
    match g.below(12) {
        0 => f32::from_bits(0x7fc0_0001),
        1 => -0.0,
        2 => 1.0e-40,
        3 => f32::INFINITY,
        _ => g.pick(&[0.0f32, 2000.0, 2.5, 30.0, 900.0, 1.5]),
    }
}

/// A geometry's tokens (piece 2's `read_vg`: valid, extent, 18 floats, two names copied in turn).
fn vg_toks(g: &mut Gen, t: &mut Vec<String>) {
    t.push(g.int(0, 1).to_string());
    for _ in 0..3 {
        t.push(g.int(0, 4).to_string());
    }
    for i in 0..18 {
        let v = if i >= 15 && g.chance(2) {
            0.0
        } else {
            g.int(-6, 6) as f32 / 4.0
        };
        t.push(fhex(v));
    }
    let names: [&[u8]; 4] = [b"", b"orig.mgz", b"/a/b/brain.mgz", b"a"];
    t.push(enc(Some(g.pick(&names))));
    t.push(enc(Some(g.pick(&names))));
}

/// A square matrix `rmat`'s tokens, or `-1 -1` for none.
fn mat_toks(g: &mut Gen, t: &mut Vec<String>, none: u64) {
    if g.chance(none) {
        let k = if g.chance(8) { 3 } else { 4 };
        t.push(k.to_string());
        t.push(k.to_string());
        for i in 0..k * k {
            let v = if i % (k + 1) == 0 {
                1.0
            } else {
                g.int(-20, 20) as f32 / 8.0
            };
            t.push(fhex(v));
        }
    } else {
        t.push("-1".into());
        t.push("-1".into());
    }
}

/// A volume's tokens (`read_mri14`); its extent, type, frames and whether it has voxels.
pub(crate) fn volume14(
    g: &mut Gen,
    t: &mut Vec<String>,
    with_bvals: bool,
    nifti: bool,
) -> (i64, i64, i64, i32, i64, bool) {
    // NIfTI's special shapes, now and then: the ico7 surface (reshaped) and 32,768 rows (FS-449)
    let special = if nifti { g.below(16) } else { 99 };
    let w = if g.chance(25) { 0 } else { g.int(1, 6) };
    let h = if g.chance(3) { g.int(1, 3) } else { 1 };
    let d = if g.chance(25) {
        0
    } else if g.chance(4) {
        g.int(1, 2)
    } else {
        1
    };
    let nf = if g.chance(40) {
        0
    } else if g.chance(3) {
        g.int(2, 3)
    } else {
        1
    };
    let ty = g.pick(&[
        mri::MRI_FLOAT,
        mri::MRI_FLOAT,
        mri::MRI_UCHAR,
        mri::MRI_UCHAR,
        mri::MRI_SHORT,
        mri::MRI_INT,
        mri::MRI_USHRT,
        mri::MRI_FLOAT_COMPLEX,
        mri::MRI_LONG,
        mri::MRI_RGB,
        mri::MRI_TENSOR,
    ]);
    let (w, h, d, nf, ty) = match special {
        0 => (
            163842,
            1,
            1,
            g.int(1, 2),
            g.pick(&[mri::MRI_UCHAR, mri::MRI_FLOAT]),
        ),
        1 => (1, 32768, 1, 1, mri::MRI_UCHAR),
        _ => (w, h, d, nf, ty),
    };
    let alloc = !g.chance(30);
    for v in [w, h, d, i64::from(ty), nf, i64::from(alloc)] {
        t.push(v.to_string());
    }
    // FreeSurfer's coronal axes, or (one in two) a signed permutation of the axes (every
    // quaternion branch of the NIfTI writer's qform), now and then all zero (no orientation)
    let mut geom = [
        1.0f32, 1.0, 1.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0, 0.0,
    ];
    if g.chance(40) {
        geom[3..].fill(0.0);
    } else if g.chance(2) {
        let perm = g.pick(&[
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ]);
        geom[3..].fill(0.0);
        for (axis, p) in perm.iter().enumerate() {
            geom[3 + 3 * axis + p] = if g.chance(2) { 1.0 } else { -1.0 };
        }
    }
    for v in geom {
        t.push(fhex(if g.chance(10) { 2.0 * v + 0.5 } else { v }));
    }
    for _ in 0..3 {
        t.push(fhex(g.int(-8, 8) as f32 / 2.0));
    }
    t.push(g.pick(&[1i64, 1, 1, 1, 0, -1, 2]).to_string());
    for _ in 0..4 {
        t.push(fhex(param(g)));
    }
    let flip = match g.below(10) {
        0 => f64::from_bits(0x7ff8_0000_0000_0003),
        1 => 1.0e-50,
        2 => 1.0e300,
        3 => 0.1,
        _ => g.pick(&[
            0.0f64,
            0.174_532_925_199_432_95,
            std::f64::consts::FRAC_PI_2,
            90.0,
        ]),
    };
    t.push(dhex(flip));
    t.push(fhex(g.pick(&[0.0f32, 3.0, 1.5, 7.0])));
    t.push(g.pick(&[1i64, 1, 0, 21, -3]).to_string());
    let bytes = (w * h * d * nf) as usize * mri::mri_sizeof(ty);
    if g.chance(5) || special <= 1 {
        t.push("-".into());
    } else {
        let mut b = vec![0u8; bytes];
        for x in b.iter_mut() {
            *x = g.below(256) as u8;
        }
        t.push(if b.is_empty() { "-".into() } else { hex(&b) });
    }
    let tf: Option<&[u8]> = match g.below(8) {
        0 => Some(b"talairach.xfm"),
        1 => Some(b"/s/subj/mri/transforms/talairach.xfm"),
        2 => Some(b""),
        _ => None,
    };
    t.push(enc(tf));
    let pe: Option<&[u8]> = match g.below(6) {
        0 => Some(b"ROW"),
        1 => Some(b"COL"),
        2 => Some(b""),
        _ => None,
    };
    t.push(enc(pe));
    let nc = if g.chance(3) { g.int(1, 3) } else { 0 };
    t.push(nc.to_string());
    for _ in 0..nc {
        let c: &[u8] = g.pick(&[
            b"mri_convert a.nii b.mgz".as_slice(),
            b"",
            b"recon-all -s x",
        ]);
        t.push(enc(Some(c)));
    }
    t.push(if g.chance(6) { g.pick(&[5i64, -1]) } else { 0 }.to_string());
    let thresh = if g.chance(4) {
        f32::from_bits(g.pick(&[
            0x3400_0000u32,
            0x33ff_ffff,
            0x3f00_0000,
            0xc000_0000,
            0x8000_0000,
        ]))
    } else {
        0.0
    };
    t.push(fhex(thresh));
    mat_toks(g, t, 6);
    mat_toks(g, t, 8);
    if g.chance(8) {
        t.push(g.int(0, 3).to_string());
        t.push(g.pick(&[1i64, 2, 0]).to_string());
        t.push(dhex(g.pick(&[0.0f64, 3.0, 0.1])));
        vg_toks(g, t);
        vg_toks(g, t);
        mat_toks(g, t, 2);
        t.push(g.int(0, 1).to_string());
    } else {
        t.push("-1".into());
    }
    if g.chance(10) {
        t.push("1".into());
        t.extend(table(g, true));
    } else {
        t.push("0".into());
    }
    if with_bvals && g.chance(3) {
        let n = g.int(0, 3);
        t.push(n.to_string());
        for _ in 0..n {
            t.push(fhex(g.pick(&[0.0f32, 1000.0, 2000.0])));
        }
        for _ in 0..n * 3 {
            t.push(fhex(g.int(-4, 4) as f32 / 4.0));
        }
        t.push(g.int(0, 3).to_string());
    } else {
        t.push("-1".into());
    }
    (w, h, d, ty, nf, alloc)
}

pub fn generate(g: &mut Gen, kind: &str, _portable: bool) -> Vec<String> {
    let mut t = vec![kind.to_owned()];
    t.push(g.pick(&["0", "0", "0", "10000", "40"]).into());
    let pick = g.below(30);
    let pair = pick >= 26;
    let (name, spec): (&str, String) = match pick {
        // NIfTI-1 pairs (MRIwriteType as NIFTI1_FILE: nifti1Write), relay 24
        26 | 27 => ("w/o.img", "-".into()),
        28 => ("w/o.hdr", "-".into()),
        29 => (
            g.pick(&["w/O.IMG", "w/o", "w/a.b.img.gz", "w/no/o.img", "w/.hdr"]),
            "-".into(),
        ),
        20 | 21 => ("w/o.nii.gz", "-".into()),
        22 | 23 => ("w/o.nii", "-".into()),
        24 => ("w/O.NII", "-".into()),
        25 => ("w/o.nii.mgz.nii", "-".into()),
        0 => ("w/o.mgh", "-".into()),
        1 => ("w/o.MGZ", "-".into()),
        2 => ("w/o.mgh.gz", "-".into()),
        3 => ("w/O.MGH", "-".into()),
        4 => ("w/o.nii", "-".into()),
        5 => ("w/no/o.mgz", "-".into()),
        6 => ("w/d.mgz", format!("D{}", hex(b"w/d.mgz"))),
        7 => ("w/o", "-".into()),
        8 => ("w/a.b.mgh.gz.x", "-".into()),
        9 => ("w/.mgz", "-".into()),
        10 => ("w/o.mgz.mgh", "-".into()),
        11..=13 => ("w/o.mgh", "-".into()),
        _ => ("w/o.mgz", "-".into()),
    };
    t.push(spec);
    t.push(if g.chance(6) { "1" } else { "0" }.into());
    let nifti = name.to_ascii_lowercase().contains(".nii") || pair;
    let how = if pair {
        2
    } else {
        i64::from(nifti || g.chance(3))
    };
    t.push(how.to_string());
    let (_, _, _, _, nf, alloc) = volume14(g, &mut t, how >= 1, nifti);
    let frame = match g.below(10) {
        0 => -3,
        1 if nf > 0 => g.int(0, nf - 1),
        2 if g.chance(4) => nf,
        _ => -1,
    };
    t.push(frame.to_string());
    t.push(
        g.pick(&[-1i64, -1, -1, -1, 0, 1, 2, 3, 5, 65539, -7])
            .to_string(),
    );
    t.push(enc(Some(name.as_bytes())));
    // never a folder read back: Linux opens it and fails at the first read, Windows at the open
    let back = alloc && name != "w/d.mgz" && g.chance(2);
    t.push(i64::from(back).to_string());
    t
}

// ---------------------------------------------------------------- the port's answers

fn read_vg(tk: &mut Toks) -> VolGeom {
    let mut v = VolGeom {
        valid: tk.int() as i32,
        width: tk.int() as i32,
        height: tk.int() as i32,
        depth: tk.int() as i32,
        ..VolGeom::default()
    };
    let mut f = [0f32; 18];
    for x in &mut f {
        *x = f32_of(tk.next());
    }
    [
        v.xsize, v.ysize, v.zsize, v.x_r, v.x_a, v.x_s, v.y_r, v.y_a, v.y_s, v.z_r, v.z_a, v.z_s,
        v.c_r, v.c_a, v.c_s, v.s_r, v.s_a, v.s_s,
    ] = f;
    v.fname = CName::new(&tk.bytes());
    v.fname.strcpy_bytes(&tk.bytes());
    v
}

fn read_rmat(g: &mut Globals, tk: &mut Toks) -> Option<Matrix> {
    let (rows, cols) = (tk.int() as i32, tk.int() as i32);
    if rows < 0 {
        return None;
    }
    let (mut s1, mut s2) = (Vec::new(), Vec::new());
    let mut m = matrix_alloc(g, &mut s1, &mut s2, rows, cols, MATRIX_REAL)
        .unwrap()
        .expect("a matrix");
    for r in 1..=rows {
        for c in 1..=cols {
            m.set_relt(r, c, f32_of(tk.next()));
        }
    }
    Some(m)
}

pub(crate) fn read_mri14(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    tk: &mut Toks,
) -> Mri {
    let v: Vec<i32> = (0..6).map(|_| tk.int() as i32).collect();
    let mut m = if v[5] != 0 {
        mri::mri_alloc_sequence(g, out, err, v[0], v[1], v[2], v[3], v[4]).unwrap()
    } else {
        mri::mri_alloc_header(g, out, err, v[0], v[1], v[2], v[3], v[4]).unwrap()
    };
    let mut f = || f32_of(tk.next());
    let vg = &mut m.vg;
    [
        vg.xsize, vg.ysize, vg.zsize, vg.x_r, vg.x_a, vg.x_s, vg.y_r, vg.y_a, vg.y_s, vg.z_r,
        vg.z_a, vg.z_s, vg.c_r, vg.c_a, vg.c_s,
    ] = [
        f(),
        f(),
        f(),
        f(),
        f(),
        f(),
        f(),
        f(),
        f(),
        f(),
        f(),
        f(),
        f(),
        f(),
        f(),
    ];
    m.vg.ras_good_flag = tk.int() as i32;
    m.tr = f32_of(tk.next());
    m.te = f32_of(tk.next());
    m.ti = f32_of(tk.next());
    m.fov = f32_of(tk.next());
    m.flip_angle = f64_of(tk.next());
    m.field_strength = f32_of(tk.next());
    m.dof = tk.int() as i32;
    let b = super::unhex(tk.next());
    if let Some(c) = m.chunk.as_mut() {
        let n = b.len().min(c.len());
        c[..n].copy_from_slice(&b[..n]);
    }
    if let Some(tf) = super::dec(tk.next()) {
        m.transform_fname = String::from_utf8(tf).unwrap();
    }
    m.pedir = super::dec(tk.next());
    let nc = tk.int();
    m.cmdlines = (0..nc).map(|_| tk.string()).collect();
    let label = tk.int() as i32;
    let thresh = f32_of(tk.next());
    if let Some(fr) = m.frames.first_mut() {
        fr.label = label;
        fr.thresh = thresh;
    }
    m.auto_align = read_rmat(g, tk);
    m.orig_ras2vox = read_rmat(g, tk);
    m.warp_field_format = tk.int() as i32;
    if m.warp_field_format != mri::WARPFIELD_DTFMT_UNKNOWN {
        m.gcamorph_spacing = tk.int() as i32;
        m.gcamorph_exp_k = f64_of(tk.next());
        m.gcamorph_image_vg = read_vg(tk);
        m.gcamorph_atlas_vg = read_vg(tk);
        m.gcamorph_affine = read_rmat(g, tk);
        if tk.int() != 0 {
            m.init_gcamorph_label();
            let (w, h, d) = (m.vg.width, m.vg.height, m.vg.depth);
            let l = m.gcamorph_label.as_mut().unwrap();
            for c in 0..w {
                for r in 0..h {
                    for k in 0..d {
                        l[((c * h + r) * d + k) as usize] = c * 7 + r * 3 + k;
                    }
                }
            }
        }
    }
    if tk.int() != 0 {
        m.ct = Some(read_table(tk));
    }
    let nb = tk.int() as i32;
    if nb >= 0 {
        let (mut s1, mut s2) = (Vec::new(), Vec::new());
        let mut bv = matrix_alloc(g, &mut s1, &mut s2, nb, 1, MATRIX_REAL)
            .unwrap()
            .expect("a matrix");
        for i in 1..=nb {
            bv.set_relt(i, 1, f32_of(tk.next()));
        }
        let mut bc = matrix_alloc(g, &mut s1, &mut s2, nb, 3, MATRIX_REAL)
            .unwrap()
            .expect("a matrix");
        for i in 1..=nb {
            for c in 1..=3 {
                bc.set_relt(i, c, f32_of(tk.next()));
            }
        }
        m.bvals = Some(bv);
        m.bvecs = Some(bc);
        m.bvec_space = tk.int() as i32;
    }
    m
}

/// The port's answer to a piece-14 case (`Err`: the exit status).
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
    let _ = err;
    // writers not ported yet (NIfTI names through MRIwrite)
    if text.windows(17).any(|x| x == b"is not ported yet") {
        extra.push("SKIP");
    }
    for n in g.notices.as_slice() {
        // FreeSurfer crashes or reads memory: a volume without voxels (FS-58), without frames
        // (FS-441), a frame outside it (FS-444), an unknown type's size (FS-55), an access outside
        // (FS-52, FS-53)
        if matches!(
            n.defect.id,
            "FS-58"
                | "FS-441"
                | "FS-444"
                | "FS-445"
                | "FS-446"
                | "FS-55"
                | "FS-52"
                | "FS-53"
                | "FS-13"
                | "FS-76"
        ) {
            extra.push("SKIP");
        }
        // the read back of a file whose header does not match its data (FS-442, a tensor or
        // long volume written from the buffer): piece 6's rules for what FreeSurfer's reader then
        // meets (crashes, writes outside its buffers, skips beyond 256 MiB)
        if matches!(
            n.defect.id,
            "FS-249" | "FS-253" | "FS-254" | "FS-255" | "FS-256" | "FS-257" | "FS-258"
        ) {
            extra.push("SKIP");
        }
        if n.defect.id == "FS-178"
            && n.detail
                .split_whitespace()
                .nth(3)
                .and_then(|x| x.parse::<u64>().ok())
                .is_none_or(|want| want > 1 << 28)
        {
            extra.push("SKIP");
        }
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
    g.progname = "fsio".into();
    let awr = tk[0] == "awr";
    let mut tk = Toks { t: tk, i: 1 };
    // piece 16: awr <Gdiag_no> <N_Zero_Pad_Output> <N_Zero_Pad_Input for the read back>, then mgw's
    if awr {
        g.gdiag_no = tk.int() as i32;
        g.n_zero_pad_output = tk.int() as i32;
        g.n_zero_pad_input = tk.int() as i32;
    }
    g.gdiag = u64::from_str_radix(tk.next(), 16).unwrap();
    setup(w, tk.next());
    let bits = tk.int();
    let mut pairs = vec![
        ("FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING", "T"),
        ("USER", "U"),
    ];
    if bits & 1 != 0 {
        pairs.push(("FS_MGZIO_USEVOXELBUFWRITE", "1"));
    }
    let env = Environment::from_pairs(pairs);
    let how = tk.int();
    let mut m = read_mri14(g, out, err, &mut tk);
    let frame = tk.int() as i32;
    let intent = tk.int() as i32;
    let name = tk.string();
    let back = tk.int() != 0;
    g.errno = 0;
    use larmorx_freesurfer_io::identify::{
        MRI_ANALYZE_FILE, MRI_ANALYZE4D_FILE, MRI_VOLUME_TYPE_UNKNOWN, NIFTI1_FILE,
    };
    let typed = match how {
        2 => Some(NIFTI1_FILE),
        3 => Some(MRI_ANALYZE_FILE),
        4 => Some(MRI_ANALYZE4D_FILE),
        _ => None,
    };
    let r = if let Some(ty) = typed {
        mriio::mri_write_type(g, out, err, &env, dir, &mut m, &name, ty, intent)
    } else if how != 0 {
        mriio::mri_write(g, out, err, &env, dir, &mut m, &name, None, intent)
    } else {
        mriio_write::mgh_write(g, out, err, &env, dir, &mut m, &name, frame, intent)
    }
    .map_err(fatal)?;
    if awr {
        o.i(i64::from(g.n_zero_pad_output));
    }
    o.i(i64::from(r));
    o.i(i64::from(g.errno));
    o.i(i64::from(m.intent));
    o.i(i64::from(m.version));
    match &m.chunk {
        Some(c) if m.ischunked => fnv(o, c),
        _ => o.0.push_str("nochunk "),
    }
    odir(o, w);
    if back {
        g.errno = 0;
        let env = Environment::default();
        // the reader of the write (relay 25): mghRead, mri_read of the name's type after MRIwrite,
        // the NIfTI-1 pair's reader after MRIwriteType(NIFTI1_FILE)
        let b = if how == 0 {
            mriio::mgh_read(g, out, err, &env, dir, &name, true, -1)
        } else {
            let ty = typed.unwrap_or(MRI_VOLUME_TYPE_UNKNOWN);
            mriio::mri_read(g, out, err, &env, dir, &name, ty, true, -1, -1)
        }
        .map_err(fatal)?;
        super::piece6::print_mri(o, w, b.as_ref());
        o.i(i64::from(g.errno));
    }
    o.i(i64::from(g.gerror));
    Ok(())
}
