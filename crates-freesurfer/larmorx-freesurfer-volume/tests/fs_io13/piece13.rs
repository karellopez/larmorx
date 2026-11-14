// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.io probe's piece 13 (`validation/probes/fs-io/`, `fsio.cpp`'s `run_piece13`): the label
//! coordinate conversions (`lcv`: `LabelToScannerRAS`, `LabelToSurfaceRAS`, `LabelToVoxel`,
//! `LabelVoxelToSurfaceRAS`, a label with or without a surface, into a new label, itself or a given
//! one) and one frame of a volume written (`mwf`: `MRIwriteFrame`), larmorx-freesurfer-volume's
//! `label_coords` and `io_frames`. The cases' framing and helpers are fs.io's (`cases.rs` of
//! larmorx-freesurfer-io's tests, included by this crate's test and example).

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_mri::mri;
use larmorx_freesurfer_mri::vol_geom::VolGeom;
use larmorx_freesurfer_volume::io_frames;
use larmorx_freesurfer_volume::label_coords::{self, LabelDst};

use crate::cases::piece4::setup;
use crate::cases::piece10::Toks;
use crate::cases::piece11::{
    dbits, env_of, f32_of, fval, print_label, read_label, read_mri, subjects_dir,
};
use crate::cases::piece12::{fhex, odir, read_volume, volume};
use crate::cases::{Gen, Out, Shared, Work, enc};

pub const KINDS: [&str; 2] = ["lcv", "mwf"];

/// The recorded test's cases: seed 1's first `RECORDED`.
pub const RECORDED: usize = 1200;

/// A coordinate: the special values, small halves, or a wide range in eighths.
fn coord(g: &mut Gen) -> f32 {
    if g.chance(4) {
        fval(g)
    } else {
        g.int(-2400, 2400) as f32 / 8.0
    }
}

/// A label's tokens (`fsio`'s `read_label11`): `<max_points> <coords> <n_points> <name> <subject>
/// <space> <avg_stat> <k>`, `k` places, then `N` or the `max_points` vertex indices.
fn label13(g: &mut Gen, with_ind: bool) -> Vec<String> {
    let max_points = if g.chance(8) {
        g.int(20, 40)
    } else {
        g.int(0, 6)
    };
    let n_points = if g.chance(6) {
        max_points
    } else {
        g.int(0, max_points)
    };
    let coords = g.pick(&[0i64, 1, 1, 2, 2, 3, 3, 4]);
    let mut t = vec![
        max_points.to_string(),
        coords.to_string(),
        n_points.to_string(),
        enc(Some(g.pick(&[
            b"".as_slice(),
            b"lh.cortex.label",
            b"w/a.label",
        ]))),
        enc(Some(g.pick(&[b"".as_slice(), b"s1", b"bert"]))),
        enc(Some(g.pick(&[
            b"TkReg".as_slice(),
            b"scanner",
            b"voxel",
            b"",
        ]))),
        dbits(g.int(-4, 4) as f64 * 0.25),
        max_points.to_string(),
    ];
    for _ in 0..max_points {
        let vno = if g.chance(3) { -1 } else { g.int(0, 6) };
        t.push(vno.to_string());
        for _ in 0..3 {
            t.push(fhex(coord(g)));
        }
        for _ in 0..3 {
            t.push(g.int(-2, 9).to_string());
        }
        t.push(if g.chance(6) { "1" } else { "0" }.into());
        t.push(fhex(fval(g)));
    }
    if with_ind && g.chance(6) {
        t.push(max_points.to_string());
        for _ in 0..max_points {
            t.push(g.int(-1, 50).to_string());
        }
    } else {
        t.push("N".into());
    }
    t
}

/// An extent: mostly small, sometimes FreeSurfer's conformed 256 (one axis at a time).
fn extent(g: &mut Gen) -> [i64; 3] {
    let mut e = [g.int(1, 12), g.int(1, 12), g.int(1, 12)];
    if g.chance(6) {
        e[g.below(3) as usize] = g.pick(&[64, 128, 256]);
    }
    e
}

/// The 15 geometry floats: voxel sizes, direction cosines (an axis permutation with signs, a
/// rotation, or rarely none: a matrix that cannot be inverted), centre.
fn geometry(g: &mut Gen) -> Vec<f32> {
    let mut f = Vec::with_capacity(15);
    for _ in 0..3 {
        f.push(if g.chance(24) {
            0.0
        } else {
            g.pick(&[0.5f32, 1.0, 1.0, 1.5, 2.0, 3.0, 0.9375, 1.2])
        });
    }
    let mut cols = [[0f32; 3]; 3];
    match g.below(8) {
        0 => {}
        1 | 2 => {
            let (c, s) = g.pick(&[(0.6f32, 0.8f32), (0.707_106_77, 0.707_106_77), (0.8, -0.6)]);
            let k = g.below(3) as usize;
            let (a, b) = ((k + 1) % 3, (k + 2) % 3);
            cols[k][k] = 1.0;
            cols[a][a] = c;
            cols[a][b] = s;
            cols[b][a] = -s;
            cols[b][b] = c;
        }
        _ => {
            let perm = g.pick(&[[0usize, 1, 2], [0, 2, 1], [1, 0, 2], [2, 0, 1], [1, 2, 0]]);
            for (axis, &p) in perm.iter().enumerate() {
                cols[axis][p] = if g.chance(2) { -1.0 } else { 1.0 };
            }
        }
    }
    for c in cols {
        f.extend_from_slice(&c);
    }
    for _ in 0..3 {
        f.push(g.int(-480, 480) as f32 / 8.0);
    }
    f
}

/// A volume for `read_mri11`: `<w> <h> <d> <type>`, the 15 geometry floats, no voxels.
fn volume13(g: &mut Gen, t: &mut Vec<String>) {
    for v in extent(g) {
        t.push(v.to_string());
    }
    t.push(g.pick(&[mri::MRI_UCHAR, mri::MRI_FLOAT]).to_string());
    for v in geometry(g) {
        t.push(fhex(v));
    }
    t.push("0".into());
}

/// A surface's volume geometry for `read_vg`: valid, extent, the 18 floats (the 15 and the
/// shears), two names.
fn surface13(g: &mut Gen, t: &mut Vec<String>) {
    t.push(if g.chance(5) { "0" } else { "1" }.into());
    for v in extent(g) {
        t.push(v.to_string());
    }
    for v in geometry(g) {
        t.push(fhex(v));
    }
    for _ in 0..3 {
        t.push(fhex(if g.chance(6) { 0.25 } else { 0.0 }));
    }
    t.push(enc(Some(b"")));
    t.push(enc(Some(b"")));
}

/// A case line's tokens after its command.
pub fn generate(g: &mut Gen, kind: &str) -> Vec<String> {
    let mut t: Vec<String> = vec![kind.into()];
    match kind {
        "lcv" => {
            t.push(g.pick(&["s", "s", "t", "t", "v", "v", "x"]).into());
            t.push(if g.chance(10) { "40" } else { "0" }.into());
            t.push(subjects_dir(g));
            t.extend(label13(g, true));
            volume13(g, &mut t);
            let dst = g.pick(&[0i64, 0, 1, 1, 2]);
            t.push(dst.to_string());
            if dst == 2 {
                t.extend(label13(g, false));
            }
            if g.chance(3) {
                t.push("1".into());
                surface13(g, &mut t);
            } else {
                t.push("0".into());
            }
        }
        _ => {
            t.push(if g.chance(10) { "40" } else { "0" }.into());
            let name = match g.below(12) {
                0 => "w/no/o.gii",
                1 => "w/o.xyz",
                2 => "w/o.mgz",
                3 => "w/O.GII",
                _ => "w/o.gii",
            };
            t.push("-".into());
            let mut v = Vec::new();
            volume(g, &mut v, false);
            let nf: i64 = v[4].parse().unwrap();
            t.extend(v);
            t.push(g.pick(&[0, 0, 0, nf - 1, nf - 1, nf, nf + 1]).to_string());
            t.push(enc(Some(name.as_bytes())));
        }
    }
    t
}

/// `n` case lines of seed `seed` (every case is portable).
pub fn cases(seed: u64, n: usize) -> Vec<String> {
    let mut g = Gen::new(seed);
    (0..n)
        .map(|i| generate(&mut g, KINDS[i % KINDS.len()]).join(" "))
        .collect()
}

fn read_vg(tk: &mut Toks) -> VolGeom {
    let mut v = VolGeom {
        valid: tk.int() as i32,
        width: tk.int() as i32,
        height: tk.int() as i32,
        depth: tk.int() as i32,
        ..VolGeom::default()
    };
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
    for d in p {
        *d = f32_of(tk.next());
    }
    // the file name: two names copied one over the other (unused by the conversions)
    tk.next();
    tk.next();
    v
}

fn answer(
    tk: &[&str],
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
    w: &Work,
) -> Result<(), u8> {
    let fatal = |f: Fatal| f.status();
    let mut tk = Toks { t: tk, i: 1 };
    g.progname = "fsio".into();
    match tk.t[0] {
        "lcv" => {
            let which = tk.next();
            g.gdiag = u64::from_str_radix(tk.next(), 16).unwrap();
            let mut env = Environment::default();
            env_of(&mut tk, &mut env, "SUBJECTS_DIR");
            let mut a = read_label(&mut tk).expect("a label");
            let mut m = read_mri(g, &mut tk);
            let dk = tk.int();
            let given = if dk == 2 { read_label(&mut tk) } else { None };
            let surface = (tk.int() != 0).then(|| read_vg(&mut tk));
            if surface.is_some() {
                a.mris = Some(1);
            }
            g.errno = 0;
            let ldst = match dk {
                0 => LabelDst::New,
                1 => LabelDst::Same,
                _ => LabelDst::Given(Box::new(given.expect("a label"))),
            };
            let mut io = label_coords::Io {
                out,
                err,
                env: &env,
            };
            let s = surface.as_ref();
            let r = match which {
                "s" => label_coords::label_to_scanner_ras(g, &mut io, &a, &mut m, s, ldst),
                "t" => label_coords::label_to_surface_ras(g, &mut io, &a, &mut m, s, ldst),
                "v" => label_coords::label_to_voxel(g, &mut io, &a, &mut m, s, ldst),
                _ => label_coords::label_voxel_to_surface_ras(g, &mut io, &a, &mut m, s, ldst),
            }
            .map_err(fatal)?;
            print_label(o, Some(&r));
        }
        _ => {
            g.gdiag = u64::from_str_radix(tk.next(), 16).unwrap();
            // a user name on every platform (piece 12's mwr)
            let env = Environment::from_pairs([
                ("FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING", "T"),
                ("USER", "U"),
            ]);
            setup(w, tk.next());
            let m = read_volume(g, &mut tk);
            let frame = tk.int() as i32;
            let name = tk.string();
            g.errno = 0;
            let r = io_frames::mri_write_frame(g, out, err, &env, Some(&w.fix), &m, &name, frame)
                .map_err(fatal)?;
            o.0.push_str(&format!("{r} {} ", g.errno));
            odir(o, w);
        }
    }
    o.0.push_str(&format!("{} ", g.gerror));
    Ok(())
}

/// The port's answer to a piece-13 case (`None`: not one; `Err`: the exit status).
pub fn run(
    tk: &[&str],
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
    w: &Work,
    extra: &mut Vec<&'static str>,
) -> Option<Result<(), u8>> {
    if !KINDS.contains(&tk[0]) {
        return None;
    }
    let s = Shared::default();
    let (mut o2, mut e2) = (s.clone(), s.clone());
    let r = answer(tk, g, &mut o2, &mut e2, o, w);
    let text = s.0.borrow().clone();
    let _ = out.write_all(&text);
    let _ = err;
    // writers not ported yet
    if text.windows(17).any(|x| x == b"is not ported yet") {
        extra.push("SKIP");
    }
    for n in g.notices.as_slice() {
        // FreeSurfer writes past its buffers, crashes or reads memory it never wrote (pieces 11
        // and 12's): counted, never compared
        if matches!(
            n.defect.id,
            "FS-423"
                | "FS-424"
                | "FS-426"
                | "FS-427"
                | "FS-429"
                | "FS-430"
                | "FS-431"
                | "FS-11"
                | "FS-13"
                | "FS-53"
                | "FS-76"
                | "FS-521"
        ) {
            extra.push("SKIP");
        }
        if matches!(n.defect.id, "FS-49" | "FS-167" | "FS-214") {
            extra.push("VARY");
        }
    }
    Some(r)
}
