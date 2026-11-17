// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Piece 30 (track A relay 37): `reo` (`MRIreorder` of a volume made in the case: extents 0 to 4,
//! frames 0 to 3, bytes, ints, floats (signalling NaNs among them), shorts, unsigned shorts and
//! the types it refuses (long, RGB, complex), voxel sizes and direction cosines ordinary,
//! oblique or zero; the dimension numbers every signed permutation, repeated, 0, 4, the
//! overflowing triples of FS-843 and `INT_MIN`; into `NULL`, a given volume of the right or a
//! wrong extent, another type, fewer or more frames, or a header without voxels; `Gdiag` on or
//! off), `sxl` (`is_sxa_volume`, `ld_sxadat_from_stem` or `ld_sxadat`, `sxa_framepower` of
//! headers of versions 0 (a missing value), 1 and 2, ended early, with counts negative, 0,
//! small or past 2^31 (FS-848), 0 to 25 gamma functions (FS-847), numbers out of range, NaN,
//! hexadecimal or not numbers; names with and without a stem, missing, a folder, `NULL`), `sxw`
//! (`sv_sxadat_by_stem` or `sv_sxadat` of a header read, into a file or a folder) and `lro`
//! (`LabelRemoveOverlap` of two labels of vertices and points without one, deleted or not).
//! Since relay 44 `ro4` (`MRIreorder4` of a source made as `reo`'s, of every voxel type, the
//! value-less ones now and then; the order every permutation of 1 to 4, now and then with a
//! value repeated (FS-938) or outside 1 to 4 (FS-937: SKIP)).

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_io::label::{Label, LabelVertex, label_remove_overlap};
use larmorx_freesurfer_io::reorder::{mri_reorder, mri_reorder4};
use larmorx_freesurfer_io::selxavgio::{self, SxaDat};
use larmorx_freesurfer_mri::mri::{self, Mri};
use larmorx_freesurfer_mri::vol_geom::reinit_cache;

use super::piece4::{file_entry, setup};
use super::piece6::print_mri;
use super::{Gen, Out, Work, dec, enc};

pub const KINDS: [&str; 5] = ["reo", "sxl", "sxw", "lro", "ro4"];

fn hx(v: f32) -> String {
    format!("{:08x}", v.to_bits())
}

fn signed_perm(g: &mut Gen) -> [i32; 3] {
    let perms = [
        [1, 2, 3],
        [1, 3, 2],
        [2, 1, 3],
        [2, 3, 1],
        [3, 1, 2],
        [3, 2, 1],
    ];
    let mut p = g.pick(&perms);
    for v in p.iter_mut() {
        if g.chance(3) {
            *v = -*v;
        }
    }
    p
}

fn gen_reo(g: &mut Gen, t: &mut Vec<String>) {
    t.push(if g.chance(8) { "1" } else { "0" }.into());
    let mut ext = [0i32; 3];
    for e in ext.iter_mut() {
        *e = if g.chance(25) { 0 } else { g.int(1, 4) as i32 };
    }
    let nf = if g.chance(30) {
        0
    } else {
        g.pick(&[1, 1, 1, 2, 3])
    };
    let ty = if g.chance(12) {
        g.pick(&[2, 9, 7])
    } else {
        g.pick(&[0, 0, 1, 3, 3, 4, 10])
    };
    for v in ext {
        t.push(v.to_string());
    }
    t.push(nf.to_string());
    t.push(ty.to_string());
    for _ in 0..3 {
        t.push(hx(g.pick(&[1.0f32, 0.5, 2.0, 1.5, 3.0, 0.9375])));
    }
    let cos: [f32; 9] = match g.below(8) {
        0 => [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        1 => [0.6, 0.8, 0.0, -0.8, 0.6, 0.0, 0.0, 0.0, 1.0],
        2 if g.chance(2) => [0.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0],
        3 => [0.0, 1.0, 0.0, 0.0, 0.0, -1.0, -1.0, 0.0, 0.0],
        _ => [-1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0],
    };
    for c in cos {
        t.push(hx(c));
    }
    for _ in 0..3 {
        t.push(hx(g.int(-40, 40) as f32 / 4.0));
    }
    let n = g.int(0, 13) as usize;
    let mut pat: Vec<u8> = (0..n).map(|_| g.below(256) as u8).collect();
    if ty == 3 && g.chance(3) {
        pat.extend_from_slice(&[0x01, 0x00, 0xa0, 0x7f, 0x00, 0x00, 0x80, 0x3f]);
    }
    t.push(enc(Some(&pat)));
    let dims: [i32; 3] = if g.chance(40) {
        g.pick(&[
            [99577, 2147399842, 2147467883],
            [106555, -2147423458, 2147437289],
            [-2147483648, 1, 3],
        ])
    } else if g.chance(8) {
        g.pick(&[
            [1, 1, 3],
            [0, 2, 3],
            [4, 2, 3],
            [1, 2, -1],
            [3, 3, 0],
            [2, -2, 3],
        ])
    } else {
        signed_perm(g)
    };
    for d in dims {
        t.push(d.to_string());
    }
    // the destination
    let mut dd = [0i32; 3];
    let valid = {
        let mut a = dims.map(|v| v.unsigned_abs());
        a.sort_unstable();
        a == [1, 2, 3]
    };
    if valid {
        for k in 0..3 {
            dd[dims[k].unsigned_abs() as usize - 1] = ext[k];
        }
    } else {
        dd = ext;
    }
    let dmode = if g.chance(20) {
        2
    } else if g.chance(3) {
        1
    } else {
        0
    };
    let mut dnf = nf;
    let mut dty = ty;
    if dmode > 0 {
        if g.chance(5) {
            let k = g.below(3) as usize;
            dd[k] = (dd[k] + 1) % 5;
        }
        if g.chance(4) {
            dty = g.pick(&[0, 1, 3, 4, 10]);
        }
        match g.below(6) {
            0 => dnf = (nf - 1).max(0),
            1 => dnf = nf + 1,
            _ => {}
        }
    }
    t.push(dmode.to_string());
    for v in [dd[0], dd[1], dd[2], dnf, dty] {
        t.push(v.to_string());
    }
}

/// `ro4`'s case: a source as `reo`'s (extents 0 to 4, frames 0 to 3, every voxel type), then the
/// order's four values.
fn gen_ro4(g: &mut Gen, t: &mut Vec<String>) {
    for _ in 0..3 {
        t.push(if g.chance(25) { 0 } else { g.int(1, 4) }.to_string());
    }
    t.push(
        if g.chance(30) {
            0
        } else {
            g.pick(&[1, 1, 2, 3])
        }
        .to_string(),
    );
    let ty = if g.chance(15) {
        g.pick(&[5, 6, 8])
    } else {
        g.pick(&[0, 0, 1, 2, 3, 3, 4, 7, 9, 10])
    };
    t.push(ty.to_string());
    for _ in 0..3 {
        t.push(hx(g.pick(&[1.0f32, 0.5, 2.0, 1.5, 3.0, 0.9375])));
    }
    let cos: [f32; 9] = match g.below(4) {
        0 => [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        1 => [0.6, 0.8, 0.0, -0.8, 0.6, 0.0, 0.0, 0.0, 1.0],
        _ => [-1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0],
    };
    for c in cos {
        t.push(hx(c));
    }
    for _ in 0..3 {
        t.push(hx(g.int(-40, 40) as f32 / 4.0));
    }
    let n = g.int(0, 13) as usize;
    let mut pat: Vec<u8> = (0..n).map(|_| g.below(256) as u8).collect();
    if (ty == 3 || ty == 7) && g.chance(3) {
        // a signalling NaN and 1.0: the NaN's payload must come through
        pat.extend_from_slice(&[0x01, 0x00, 0xa0, 0x7f, 0x00, 0x00, 0x80, 0x3f]);
    }
    t.push(enc(Some(&pat)));
    let mut order = [1, 2, 3, 4];
    for i in (1..4).rev() {
        let j = g.below(i as u64 + 1) as usize;
        order.swap(i, j);
    }
    if g.chance(8) {
        // a value repeated (FS-938)
        let k = g.below(4) as usize;
        order[k] = order[(k + 1 + g.below(3) as usize) % 4];
    } else if g.chance(15) {
        // outside 1 to 4 (FS-937)
        order[g.below(4) as usize] = g.pick(&[0, 5, -1, 2147483647]);
    }
    for v in order {
        t.push(v.to_string());
    }
}

/// A number's text for the headers.
fn num(g: &mut Gen, int: bool) -> String {
    if g.chance(14) {
        return g
            .pick(&[
                "1e40",
                "nan",
                "-0",
                "0x1p3",
                "2147483648",
                "+5",
                "x",
                "-7",
                "inf",
                "1e-50",
            ])
            .into();
    }
    if int {
        g.int(-3, 40).to_string()
    } else {
        format!("{}", g.int(-4000, 4000) as f64 / 16.0)
    }
}

fn header(g: &mut Gen) -> Vec<u8> {
    let mut s = String::new();
    let (nc, nh): (i64, i64) = if g.chance(30) {
        g.pick(&[(101, 1000), (40000, 30000)])
    } else {
        (g.pick(&[0, 1, 2, 3, 4, -2]), g.pick(&[0, 1, 2, 3, -1]))
    };
    let early = g.chance(10);
    for (k, v) in [
        ("TR", num(g, false)),
        ("TimeWindow", num(g, false)),
        ("TPreStim", num(g, false)),
    ] {
        s += &format!("{k} {v}\n");
    }
    s += &format!("nCond {nc}\nNh {nh}\n");
    if early || nc >= 40000 {
        // version 0: the word is there, its number not (or the file ends)
        if g.chance(2) || nc >= 40000 {
            s += "version x\n";
        }
        return s.into_bytes();
    }
    let version = g.pick(&[1, 2, 2]);
    s += &format!(
        "version {version}\nTER {}\nDOF {}\n",
        num(g, false),
        num(g, false)
    );
    s += "NPerCond";
    for _ in 0..nc.max(0) {
        s += &format!(" {}", num(g, true));
    }
    s += "\n";
    for k in ["nruns", "ntp", "nrows", "ncols", "nskip", "DTOrder"] {
        s += &format!("{k} {}\n", num(g, true));
    }
    s += &format!(
        "RescaleFactor {}\nHanningRadius {}\nnNoiseAC {}\nBrainAirSeg {}\n",
        num(g, false),
        num(g, false),
        num(g, true),
        num(g, true)
    );
    let gf = if g.chance(8) {
        g.int(11, 20)
    } else if g.chance(20) {
        g.int(21, 25)
    } else if g.chance(15) {
        -2
    } else {
        g.pick(&[0, 0, 0, 1, 2, 3, 10])
    };
    s += &format!("GammaFit {gf}\n");
    if gf != 0 {
        for w in ["gfDelta", "gfTau"] {
            s += w;
            for _ in 0..gf.max(0) {
                s += &format!(" {}", num(g, false));
            }
            s += "\n";
        }
    }
    s += &format!("NullCondId {}\n", num(g, true));
    let nch = nh * (nc - 1);
    let nn = if nch > 0 { (nch * nch).min(400) } else { 0 };
    let cut = g.chance(8);
    s += "SumXtX\n";
    for i in 0..nn {
        if cut && i == nn / 2 {
            return s.into_bytes();
        }
        s += &format!("{} \n", num(g, false));
    }
    if version == 1 {
        return s.into_bytes();
    }
    s += "hCovMtx\n";
    for _ in 0..nn {
        s += &format!("{} \n", num(g, false));
    }
    s += "CondIdMap";
    for _ in 0..nc.max(0) {
        s += &format!(" {}", num(g, true));
    }
    s += "\n";
    s.into_bytes()
}

fn gen_sxl(g: &mut Gen, t: &mut Vec<String>) {
    let h = header(g);
    let mut entries = Vec::new();
    let (direct, name) = match g.below(9) {
        0 => (0, None),
        1 => (1, Some("w/h.dat")),
        2 => (0, Some("w/h")),
        3 => (0, Some("w/h.nii.gz")),
        4 => (0, Some("w/q.mgz")),
        5 => (1, Some("w/q.dat")),
        6 => {
            entries.push(format!("D{}", super::hex(b"w/d.dat")));
            (g.below(2) as i32, Some("w/d.mgz"))
        }
        _ => (0, Some("w/h.mgz")),
    };
    let target = match name {
        Some("w/h.nii.gz") => "w/h.dat",
        _ => "w/h.dat",
    };
    entries.push(file_entry(target, &h));
    t.push(direct.to_string());
    t.push(enc(name.map(str::as_bytes)));
    t.push(entries.join(","));
}

fn gen_sxw(g: &mut Gen, t: &mut Vec<String>) {
    let h = header(g);
    let mut entries = vec![file_entry("w/h.dat", &h)];
    let (out, dat, by_stem) = match g.below(4) {
        0 => ("w/o.dat", "w/o.dat", 0),
        1 => {
            entries.push(format!("D{}", super::hex(b"w/dd.dat")));
            ("w/dd.mgz", "w/dd.dat", 1)
        }
        _ => ("w/o.mgz", "w/o.dat", 1),
    };
    t.push(enc(Some(b"w/h.dat")));
    t.push(enc(Some(out.as_bytes())));
    t.push(enc(Some(dat.as_bytes())));
    t.push(by_stem.to_string());
    t.push(entries.join(","));
}

fn gen_lro(g: &mut Gen, t: &mut Vec<String>) {
    for _ in 0..2 {
        let n = g.int(0, 6);
        t.push(n.to_string());
        for _ in 0..n {
            t.push(g.int(-1, 5).to_string());
            t.push(if g.chance(5) { "1" } else { "0" }.into());
        }
    }
}

pub fn generate(g: &mut Gen, kind: &str, _portable: bool) -> Vec<String> {
    let mut t = vec![kind.to_owned()];
    match kind {
        "reo" => gen_reo(g, &mut t),
        "sxl" => gen_sxl(g, &mut t),
        "sxw" => gen_sxw(g, &mut t),
        "ro4" => gen_ro4(g, &mut t),
        _ => gen_lro(g, &mut t),
    }
    t
}

fn f32_of(s: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(s, 16).unwrap())
}

fn fh(o: &mut Out, v: f32) {
    o.x(&v.to_bits().to_be_bytes(), 4);
}

fn fnv(o: &mut Out, b: &[u8]) {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &c in b {
        h ^= u64::from(c);
        h = h.wrapping_mul(1_099_511_628_211);
    }
    o.0.push_str(&format!("fnv{h:016x} "));
}

/// `oarr30`: `N`, or `c<count>` and the values (FNV-1a 64 of their bytes beyond 64).
fn oarr<T: Copy>(
    o: &mut Out,
    a: Option<&[T]>,
    n: i64,
    bytes: fn(T) -> [u8; 4],
    show: fn(&mut Out, T),
) {
    let Some(a) = a else {
        o.0.push_str("N ");
        return;
    };
    o.0.push_str(&format!("c{n} "));
    let a = &a[..(n.max(0) as usize).min(a.len())];
    if n > 64 {
        let b: Vec<u8> = a.iter().flat_map(|&v| bytes(v)).collect();
        fnv(o, &b);
        return;
    }
    for &v in a {
        show(o, v);
    }
}

fn ofloats(o: &mut Out, a: Option<&[f32]>, n: i64) {
    oarr(o, a, n, |v: f32| v.to_le_bytes(), fh);
}

fn oints(o: &mut Out, a: Option<&[i32]>, n: i64) {
    oarr(o, a, n, |v: i32| v.to_le_bytes(), |o, v| o.i(i64::from(v)));
}

fn print_sxa(o: &mut Out, s: Option<&SxaDat>) {
    let Some(s) = s else {
        o.0.push_str("N ");
        return;
    };
    o.i(i64::from(s.version));
    for v in [s.tr, s.ter, s.time_window, s.t_pre_stim] {
        fh(o, v);
    }
    for v in [s.nc, s.nnnc, s.nh] {
        o.i(i64::from(v));
    }
    fh(o, s.dof);
    oints(o, s.npercond.as_deref(), i64::from(s.nc));
    for v in [s.nruns, s.ntp, s.nrows, s.ncols, s.nskip, s.dt_order] {
        o.i(i64::from(v));
    }
    fh(o, s.rescale_factor);
    fh(o, s.hanning_radius);
    o.i(i64::from(s.brain_air_seg));
    o.i(i64::from(s.gamma_fit));
    for k in 0..10 {
        fh(o, s.gf_delta[k]);
    }
    for k in 0..10 {
        fh(o, s.gf_tau[k]);
    }
    o.i(i64::from(s.null_cond_id));
    let nch = s.nh.wrapping_mul(s.nnnc);
    let nn = i64::from(nch.wrapping_mul(nch));
    ofloats(o, s.sum_xtx.as_deref(), nn);
    ofloats(o, s.h_cov_mtx.as_deref(), nn);
    o.i(i64::from(s.n_noise_ac));
    oints(o, s.cond_id_map.as_deref(), i64::from(s.nc));
}

fn ofile(o: &mut Out, w: &Work, name: &str) {
    let p = w.fix.join(name);
    match std::fs::metadata(&p) {
        Ok(m) if m.is_file() => o.bytes(&std::fs::read(&p).unwrap()),
        _ => o.0.push_str("N "),
    }
}

fn has(g: &Globals, id: &str) -> bool {
    g.notices.as_slice().iter().any(|x| x.defect.id == id)
}

pub fn run(
    tk: &[&str],
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
    w: &Work,
    extra: &mut Vec<&'static str>,
) -> Result<(), u8> {
    let fatal = |f: larmorx_freesurfer_core::log::Fatal| f.status();
    let dir = Some(w.fix.as_path());
    match tk[0] {
        "reo" => {
            g.gdiag = u64::from_str_radix(tk[1], 16).unwrap();
            let p: Vec<i32> = tk[2..7].iter().map(|s| s.parse().unwrap()).collect();
            let geo: Vec<f32> = tk[7..22].iter().map(|s| f32_of(s)).collect();
            let pat = dec(tk[22]).unwrap_or_default();
            let dims: Vec<i32> = tk[23..26].iter().map(|s| s.parse().unwrap()).collect();
            let dmode: i32 = tk[26].parse().unwrap();
            let q: Vec<i32> = tk[27..32].iter().map(|s| s.parse().unwrap()).collect();
            let fill = |m: &mut Mri, pat: &[u8]| {
                if let Some(c) = m.chunk.as_mut()
                    && !pat.is_empty()
                {
                    let n = (m.bytes_total as usize).min(c.len());
                    for (i, b) in c[..n].iter_mut().enumerate() {
                        *b = pat[i % pat.len()];
                    }
                }
            };
            let mut src = mri::mri_alloc_sequence(g, out, err, p[0], p[1], p[2], p[4], p[3])
                .map_err(fatal)?;
            {
                let v = &mut src.vg;
                (v.xsize, v.ysize, v.zsize) = (geo[0], geo[1], geo[2]);
                (v.x_r, v.x_a, v.x_s) = (geo[3], geo[4], geo[5]);
                (v.y_r, v.y_a, v.y_s) = (geo[6], geo[7], geo[8]);
                (v.z_r, v.z_a, v.z_s) = (geo[9], geo[10], geo[11]);
                (v.c_r, v.c_a, v.c_s) = (geo[12], geo[13], geo[14]);
                v.ras_good_flag = 1;
            }
            reinit_cache(g, out, err, &mut src.vg).map_err(fatal)?;
            fill(&mut src, &pat);
            let mut dst = match dmode {
                1 => {
                    let mut d = mri::mri_alloc_sequence(g, out, err, q[0], q[1], q[2], q[4], q[3])
                        .map_err(fatal)?;
                    fill(&mut d, &[0x5a]);
                    Some(d)
                }
                2 => Some(
                    mri::mri_alloc_header(g, out, err, q[0], q[1], q[2], q[4], q[3])
                        .map_err(fatal)?,
                ),
                _ => None,
            };
            g.errno = 0;
            let r = mri_reorder(g, out, err, &src, dst.as_mut(), dims[0], dims[1], dims[2]);
            if has(g, "FS-843") || has(g, "FS-844") || has(g, "FS-58") {
                // FreeSurfer crashes or reads memory past the destination's slices
                extra.push("SKIP");
            }
            let r = r.map_err(fatal)?;
            if has(g, "FS-845") && r.is_null() {
                // the port refuses what FreeSurfer writes past the destination's voxels
                extra.push("SKIP");
            }
            match r {
                Ret::New(m) => print_mri(o, w, Some(&m)),
                Ret::Given => print_mri(o, w, dst.as_ref()),
                Ret::Null => print_mri(o, w, None),
            }
            if let Some(d) = dst.as_ref() {
                print_mri(o, w, Some(d));
            }
            o.i(i64::from(g.errno));
        }
        "ro4" => {
            let p: Vec<i32> = tk[1..6].iter().map(|s| s.parse().unwrap()).collect();
            let geo: Vec<f32> = tk[6..21].iter().map(|s| f32_of(s)).collect();
            let pat = dec(tk[21]).unwrap_or_default();
            let ord: Vec<i32> = tk[22..26].iter().map(|s| s.parse().unwrap()).collect();
            let mut src = mri::mri_alloc_sequence(g, out, err, p[0], p[1], p[2], p[4], p[3])
                .map_err(fatal)?;
            {
                let v = &mut src.vg;
                (v.xsize, v.ysize, v.zsize) = (geo[0], geo[1], geo[2]);
                (v.x_r, v.x_a, v.x_s) = (geo[3], geo[4], geo[5]);
                (v.y_r, v.y_a, v.y_s) = (geo[6], geo[7], geo[8]);
                (v.z_r, v.z_a, v.z_s) = (geo[9], geo[10], geo[11]);
                (v.c_r, v.c_a, v.c_s) = (geo[12], geo[13], geo[14]);
                v.ras_good_flag = 1;
            }
            reinit_cache(g, out, err, &mut src.vg).map_err(fatal)?;
            if let Some(c) = src.chunk.as_mut()
                && !pat.is_empty()
            {
                let n = (src.bytes_total as usize).min(c.len());
                for (i, b) in c[..n].iter_mut().enumerate() {
                    *b = pat[i % pat.len()];
                }
            }
            g.errno = 0;
            let r = mri_reorder4(g, out, err, &src, [ord[0], ord[1], ord[2], ord[3]]);
            if has(g, "FS-937") || has(g, "FS-58") {
                // FreeSurfer reads stack memory or crashes
                extra.push("SKIP");
            }
            let r = r.map_err(fatal)?;
            print_mri(o, w, r.as_ref());
            o.i(i64::from(g.errno));
        }
        "sxl" => {
            let direct: i32 = tk[1].parse().unwrap();
            let name = dec(tk[2]).map(|b| String::from_utf8(b).unwrap());
            setup(w, tk[3]);
            g.errno = 0;
            o.i(i64::from(selxavgio::is_sxa_volume(g, dir, name.as_deref())));
            if let Some(name) = name {
                let s = if direct == 1 {
                    selxavgio::ld_sxadat(g, err, dir, &name)
                } else {
                    selxavgio::ld_sxadat_from_stem(g, err, dir, &name)
                }
                .map_err(fatal)?;
                if has(g, "FS-848") || (has(g, "FS-847") && s.is_none()) {
                    // FreeSurfer loops for billions of reads, writes outside its arrays or past
                    // its header
                    extra.push("SKIP");
                }
                print_sxa(o, s.as_ref());
                if let Some(s) = s {
                    let r = selxavgio::sxa_framepower(g, err, &s);
                    if has(g, "FS-848") {
                        extra.push("SKIP");
                    }
                    let (fp, n) = r.map_err(fatal)?;
                    o.i(i64::from(n));
                    ofloats(o, fp.as_deref(), i64::from(n));
                }
            }
            o.i(i64::from(g.errno));
        }
        "sxw" => {
            let name = String::from_utf8(dec(tk[1]).unwrap()).unwrap();
            let outname = String::from_utf8(dec(tk[2]).unwrap()).unwrap();
            let datname = String::from_utf8(dec(tk[3]).unwrap()).unwrap();
            let by_stem: i32 = tk[4].parse().unwrap();
            setup(w, tk[5]);
            g.errno = 0;
            let s = selxavgio::ld_sxadat(g, err, dir, &name).map_err(fatal)?;
            if has(g, "FS-848") || (has(g, "FS-847") && s.is_none()) {
                // as in sxl
                extra.push("SKIP");
            }
            match s {
                None => o.0.push_str("N "),
                Some(mut s) => {
                    s.nrows = 1;
                    s.ncols = 1;
                    let r = if by_stem == 1 {
                        selxavgio::sv_sxadat_by_stem(g, err, dir, &s, &outname)
                    } else {
                        selxavgio::sv_sxadat(g, err, dir, &s, &outname)
                    };
                    if has(g, "FS-849") {
                        extra.push("SKIP");
                    }
                    o.i(i64::from(r));
                    ofile(o, w, &datname);
                }
            }
            o.i(i64::from(g.errno));
        }
        _ => {
            let mut at = 1;
            let mut label = || {
                let n: i32 = tk[at].parse().unwrap();
                at += 1;
                let mut a = Label {
                    max_points: n,
                    n_points: n,
                    lv: vec![LabelVertex::default(); n.max(1) as usize],
                    ..Label::default()
                };
                for i in 0..n as usize {
                    a.lv[i].vno = tk[at].parse().unwrap();
                    a.lv[i].deleted = tk[at + 1].parse().unwrap();
                    at += 2;
                }
                a
            };
            let mut a = label();
            let b = label();
            o.i(i64::from(label_remove_overlap(g, &mut a, &b)));
            for v in &a.lv[..a.n_points as usize] {
                o.i(i64::from(v.deleted));
            }
        }
    }
    Ok(())
}
