// SPDX-License-Identifier: LicenseRef-FSL
// Rust translation Copyright 2026 Karel Lopez Vilaret. Distributed under the FSL Licence (see
// LICENSE).
//! The replica's side of `validation/probes/fsl-warpfns/`: writes, into `replica<n>.txt` of every
//! case directory under `WF_PROBE_ROOT` (`n` = `WF_PROBE_NTHR`, default 1), the lines
//! `warpfns_probe.cc` prints from FSL's own libraries, from `larmorx_nc_core::splinterpolator`,
//! `::interp_partial` and `::warpfns`. Run with `--ignored`.

use larmorx_nc_core::arma::Mat;
use larmorx_nc_core::basisfield::{FieldIndex, SplineField};
use larmorx_nc_core::interp_partial::{ExtrapParams, Interpolator, valid_f};
use larmorx_nc_core::splinterpolator::{ExtrapolationType, SplineValue, Splinterpolator};
use larmorx_nc_core::volume::{Extrapolation, FslError, Interpolation, Volume};
use larmorx_nc_core::warpfns;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;

type Params = HashMap<String, Vec<String>>;

fn read_params(dir: &Path) -> Params {
    let mut p = Params::new();
    for line in std::fs::read_to_string(dir.join("params.txt"))
        .unwrap()
        .lines()
    {
        let mut it = line.split_whitespace();
        if let Some(k) = it.next() {
            p.insert(k.to_owned(), it.map(str::to_owned).collect());
        }
    }
    p
}

fn i(p: &Params, k: &str, n: usize) -> i64 {
    p[k][n].parse().unwrap()
}

fn d(p: &Params, k: &str, n: usize) -> f64 {
    p[k][n].parse().unwrap()
}

fn read_f32(f: &Path) -> Vec<f32> {
    std::fs::read(f)
        .unwrap()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_le_bytes(*c))
        .collect()
}

fn read_f64(f: &Path) -> Vec<f64> {
    std::fs::read(f)
        .unwrap()
        .as_chunks::<8>()
        .0
        .iter()
        .map(|c| f64::from_le_bytes(*c))
        .collect()
}

fn read_u32(f: &Path) -> Vec<u32> {
    std::fs::read(f)
        .unwrap()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes(*c))
        .collect()
}

fn read_mat(f: &Path) -> Mat {
    Mat::from_rows(4, 4, &read_f64(f))
}

trait Bits: SplineValue {
    fn line(out: &mut String, tag: &str, v: Self);
}
impl Bits for f32 {
    fn line(out: &mut String, tag: &str, v: f32) {
        writeln!(out, "{tag} {:08x}", v.to_bits()).unwrap();
    }
}
impl Bits for f64 {
    fn line(out: &mut String, tag: &str, v: f64) {
        writeln!(out, "{tag} {:016x}", v.to_bits()).unwrap();
    }
}

fn pf(out: &mut String, tag: &str, v: f32) {
    f32::line(out, tag, v);
}

fn pd(out: &mut String, tag: &str, v: f64) {
    f64::line(out, tag, v);
}

fn perr(out: &mut String, tag: &str, e: &FslError) {
    writeln!(out, "{tag} error {}", e.what).unwrap();
}

fn ext_of(s: &str) -> Extrapolation {
    match s {
        "zeropad" => Extrapolation::Zeropad,
        "constpad" => Extrapolation::Constpad,
        "extraslice" => Extrapolation::Extraslice,
        "mirror" => Extrapolation::Mirror,
        _ => Extrapolation::Periodic,
    }
}

fn interp_of(s: &str) -> Interpolation {
    match s {
        "nearestneighbour" => Interpolation::NearestNeighbour,
        "trilinear" => Interpolation::Trilinear,
        "sinc" => Interpolation::Sinc,
        _ => Interpolation::Spline,
    }
}

fn et_of(n: i64) -> ExtrapolationType {
    [
        ExtrapolationType::Zeros,
        ExtrapolationType::Constant,
        ExtrapolationType::Mirror,
        ExtrapolationType::Periodic,
        ExtrapolationType::SoftZeros,
        ExtrapolationType::CorrectConstant,
    ][n as usize]
}

fn setup(v: &mut Volume, p: &Params, pre: &str) {
    if let Some(s) = p.get(&format!("{pre}interp")) {
        v.interp = interp_of(&s[0]);
    }
    if p.contains_key(&format!("{pre}order")) {
        v.splineorder = i(p, &format!("{pre}order"), 0) as i32;
    }
    if let Some(s) = p.get(&format!("{pre}extrap")) {
        v.extrap = ext_of(&s[0]);
    }
    if p.contains_key(&format!("{pre}pad")) {
        v.padvalue = d(p, &format!("{pre}pad"), 0) as f32;
    }
    if p.contains_key(&format!("{pre}valid")) {
        let k = format!("{pre}valid");
        v.ep_valid = [i(p, &k, 0) != 0, i(p, &k, 1) != 0, i(p, &k, 2) != 0];
    }
}

fn vol3(p: &Params, sz: &str, vxs: &str, t: i64) -> Volume {
    let mut v: Volume = Volume::new(i(p, sz, 0), i(p, sz, 1), i(p, sz, 2), t);
    v.setdims(
        d(p, vxs, 0) as f32,
        d(p, vxs, 1) as f32,
        d(p, vxs, 2) as f32,
    );
    v
}

// ------------------------------------------------------------------------------------------------

fn spl<T: Bits>(dir: &Path, p: &Params, pfx: &str, conv: fn(f32) -> T, out: &mut String) {
    let nd = i(p, "ndim", 0) as usize;
    let dims: Vec<u32> = (0..nd).map(|k| i(p, "dims", k) as u32).collect();
    let ets: Vec<ExtrapolationType> = (0..nd).map(|k| et_of(i(p, "et", k))).collect();
    let n: usize = dims.iter().map(|&x| x as usize).product();
    let order = i(p, "order", 0) as u32;
    let data: Vec<T> = read_f32(&dir.join("data.bin"))[..n]
        .iter()
        .map(|&x| conv(x))
        .collect();
    let pts = read_f64(&dir.join("points.bin"));
    let grid = read_u32(&dir.join("grid.bin"));
    let tag = |t: &str| format!("{pfx}{t}");
    let s = match Splinterpolator::new(&data, &dims, &ets, order, 1e-8, false) {
        Ok(s) => s,
        Err(e) => return perr(out, &tag("new"), &e),
    };
    for &c in s.coefs() {
        T::line(out, &tag("coef"), c);
    }
    for q in pts.as_chunks::<5>().0 {
        let co: Vec<f32> = q[..nd].iter().map(|&x| x as f32).collect();
        match s.value(&co) {
            Ok(v) => T::line(out, &tag("val"), v),
            Err(e) => perr(out, &tag("val"), &e),
        }
        for dd in 0..nd as u32 {
            match s.value_deriv(&co, dd) {
                Ok((v, dv)) => {
                    T::line(out, &tag("vd"), v);
                    T::line(out, &tag("vd"), dv);
                }
                Err(e) => perr(out, &tag("vd"), &e),
            }
        }
        match s.val_and_derivs_all(&co) {
            Ok((v, r)) => {
                T::line(out, &tag("vads"), v);
                for x in r {
                    T::line(out, &tag("vads"), x);
                }
            }
            Err(e) => perr(out, &tag("vads"), &e),
        }
        let c = |k: usize| if nd > k { q[k] } else { 0.0 };
        if nd <= 4 {
            match s.at(q[0], c(1), c(2), c(3)) {
                Ok(v) => T::line(out, &tag("at"), v),
                Err(e) => perr(out, &tag("at"), &e),
            }
        }
        if nd <= 3 {
            for dd in 0..nd as u32 {
                match s.at_deriv(q[0], c(1), c(2), dd) {
                    Ok((v, dv)) => {
                        T::line(out, &tag("atd"), v);
                        T::line(out, &tag("atd"), dv);
                    }
                    Err(e) => perr(out, &tag("atd"), &e),
                }
            }
        }
        if nd == 3 {
            match s.val_and_derivs3(q[0], q[1], q[2]) {
                Ok((v, r)) => {
                    T::line(out, &tag("vad3"), v);
                    for x in r {
                        T::line(out, &tag("vad3"), x);
                    }
                }
                Err(e) => perr(out, &tag("vad3"), &e),
            }
        }
    }
    for g in grid.as_chunks::<5>().0 {
        let ix = &g[..nd];
        match s.grad(ix) {
            Ok(r) => {
                for x in r {
                    T::line(out, &tag("grad"), x);
                }
            }
            Err(e) => perr(out, &tag("grad"), &e),
        }
        for dd in 0..nd as u32 {
            match s.deriv(ix, dd) {
                Ok(v) => T::line(out, &tag("deriv"), v),
                Err(e) => perr(out, &tag("deriv"), &e),
            }
        }
        if nd == 3 {
            match s.grad3d(ix[0], ix[1], ix[2]) {
                Ok(r) => {
                    for x in r {
                        T::line(out, &tag("g3"), x);
                    }
                }
                Err(e) => perr(out, &tag("g3"), &e),
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------

fn vol(dir: &Path, p: &Params, out: &mut String) {
    let mut f = vol3(p, "fsize", "fvxs", 1);
    let n = f.data.len();
    f.data.copy_from_slice(&read_f32(&dir.join("f.bin"))[..n]);
    setup(&mut f, p, "f");
    let pts = read_f32(&dir.join("points.bin"));
    let ep2 = ExtrapParams {
        method: Some(ext_of(&p["ep2extrap"][0])),
        padvalue: None,
        validity: Some([
            i(p, "ep2valid", 0) != 0,
            i(p, "ep2valid", 1) != 0,
            i(p, "ep2valid", 2) != 0,
        ]),
    };
    let it = Interpolator::new(&f);
    for (e, ep) in [(1, ExtrapParams::DEFAULT), (2, ep2)] {
        let t = |s: &str| format!("e{e}{s}");
        for q in pts.as_chunks::<3>().0 {
            let (x, y, z) = (q[0], q[1], q[2]);
            match &it {
                Err(err) => {
                    perr(out, &t("ip"), err);
                    for _ in 0..3 {
                        perr(out, &t("i1"), err);
                    }
                    perr(out, &t("i3"), err);
                }
                Ok(it) => {
                    match it.interpolate(x, y, z, &ep) {
                        Ok(v) => pf(out, &t("ip"), v),
                        Err(err) => perr(out, &t("ip"), &err),
                    }
                    for dd in 0..3 {
                        match it.interp1partial(x, y, z, dd, &ep) {
                            Ok((v, dv)) => {
                                pf(out, &t("i1"), v);
                                pf(out, &t("i1"), dv);
                            }
                            Err(err) => perr(out, &t("i1"), &err),
                        }
                    }
                    match it.interp3partial(x, y, z, &ep) {
                        Ok((v, dv)) => {
                            pf(out, &t("i3"), v);
                            for x in dv {
                                pf(out, &t("i3"), x);
                            }
                        }
                        Err(err) => perr(out, &t("i3"), &err),
                    }
                }
            }
            writeln!(
                out,
                "{} {}",
                t("vf"),
                i32::from(valid_f(&f, x, y, z, 1e-8, &ep))
            )
            .unwrap();
        }
    }
}

// ------------------------------------------------------------------------------------------------

fn pmat(out: &mut String, tag: &str, m: &Mat) {
    for r in 0..4 {
        for c in 0..4 {
            pd(out, tag, m.at(r, c));
        }
    }
}

fn pout(out: &mut String, tag: &str, o: &Volume) {
    for &v in &o.data[..o.nvoxels() as usize] {
        pf(out, tag, v);
    }
    writeln!(out, "{tag} forms {} {}", o.sform_code, o.qform_code).unwrap();
    pmat(out, &format!("{tag}s"), &o.sform);
    pmat(out, &format!("{tag}q"), &o.qform);
}

fn pderiv(out: &mut String, tag: &str, d: &Volume) {
    for &v in &d.data {
        pf(out, tag, v);
    }
    writeln!(out, "{tag} forms {} {}", d.sform_code, d.qform_code).unwrap();
}

fn pmask(out: &mut String, tag: &str, m: &Volume<i8>) {
    for &v in &m.data {
        writeln!(out, "{tag} {v}").unwrap();
    }
}

fn warp(dir: &Path, p: &Params, nthr: u32, out: &mut String) {
    let mut f = vol3(p, "fsize", "fvxs", 1);
    let n = f.data.len();
    f.data.copy_from_slice(&read_f32(&dir.join("f.bin"))[..n]);
    setup(&mut f, p, "f");
    if i(p, "fsform", 0) != 0 {
        f.sform_code = i(p, "fsform", 0) as i32;
        f.sform = read_mat(&dir.join("fsform.bin"));
    }
    let mut dv = vol3(p, "dsize", "dvxs", 3);
    let n = dv.data.len();
    dv.data.copy_from_slice(&read_f32(&dir.join("d.bin"))[..n]);
    if i(p, "dsform", 0) != 0 {
        dv.sform_code = i(p, "dsform", 0) as i32;
        dv.sform = read_mat(&dir.join("dsform.bin"));
    }
    let a = read_mat(&dir.join("A.bin"));
    let tt = read_mat(&dir.join("TT.bin"));
    let mm = read_mat(&dir.join("M.bin"));
    let dir1 = i(p, "dir1", 0) as i32;
    let mkout = || vol3(p, "osize", "ovxs", 1);
    let mkder = |n: i64| vol3(p, "osize", "ovxs", n);
    let mkmask =
        || -> Volume<i8> { Volume::new(i(p, "osize", 0), i(p, "osize", 1), i(p, "osize", 2), 1) };
    let def = ExtrapParams::DEFAULT;
    {
        let (mut o, mut de, mut m) = (mkout(), mkder(3), mkmask());
        match warpfns::general_transform_3partial(&f, &a, &dv, &mut o, &mut de, Some(&mut m), nthr)
        {
            Ok(()) => {
                pout(out, "g3p", &o);
                pderiv(out, "g3pd", &de);
                pmask(out, "g3pm", &m);
            }
            Err(e) => perr(out, "g3p", &e),
        }
    }
    {
        let mut o = mkout();
        match warpfns::general_transform(&f, &a, &dv, &mut o, None, nthr) {
            Ok(()) => pout(out, "g", &o),
            Err(e) => perr(out, "g", &e),
        }
    }
    {
        let (mut o, mut m) = (mkout(), mkmask());
        match warpfns::general_transform(&f, &a, &dv, &mut o, Some(&mut m), nthr) {
            Ok(()) => {
                pout(out, "gm", &o);
                pmask(out, "gmm", &m);
            }
            Err(e) => perr(out, "gm", &e),
        }
    }
    {
        let (mut o, mut m) = (mkout(), mkmask());
        match warpfns::affine_transform(&f, &a, &mut o, Some(&mut m), nthr) {
            Ok(()) => {
                pout(out, "a", &o);
                pmask(out, "am", &m);
            }
            Err(e) => perr(out, "a", &e),
        }
    }
    {
        let (mut o, mut de) = (mkout(), mkder(3));
        match warpfns::affine_transform_3partial(&f, &a, &mut o, &mut de, None, nthr) {
            Ok(()) => {
                pout(out, "a3p", &o);
                pderiv(out, "a3pd", &de);
            }
            Err(e) => perr(out, "a3p", &e),
        }
    }
    {
        let (mut o, mut de) = (mkout(), mkder(3));
        let d0 = dv.sub_volume(0).unwrap();
        match warpfns::displacement_transform_1d_3partial(
            &f, &a, &d0, dir1, &mut o, &mut de, None, nthr,
        ) {
            Ok(()) => {
                pout(out, "d1", &o);
                pderiv(out, "d1d", &de);
            }
            Err(e) => perr(out, "d1", &e),
        }
    }
    {
        let (mut o, mut de, mut m) = (mkout(), mkder(1), mkmask());
        match warpfns::raw_general_transform(
            &f,
            &a,
            &dv,
            &[0, 1, 2],
            &[dir1],
            &[],
            None,
            None,
            &mut o,
            &mut de,
            Some(&mut m),
            nthr,
            &def,
            &def,
        ) {
            Ok(()) => {
                pout(out, "r1", &o);
                pderiv(out, "r1d", &de);
                pmask(out, "r1m", &m);
            }
            Err(e) => perr(out, "r1", &e),
        }
    }
    {
        let (mut o, mut m) = (mkout(), mkmask());
        match warpfns::apply_warp(&f, &a, &dv, &tt, &mm, &mut o, Some(&mut m), nthr) {
            Ok(()) => {
                pout(out, "aw", &o);
                pmask(out, "awm", &m);
            }
            Err(e) => perr(out, "aw", &e),
        }
    }
    {
        let (mut o, mut m) = (mkout(), mkmask());
        let oz = i(p, "osize", 2) as u32;
        match warpfns::affine_transform_slices(&f, &a, &[oz - 1, 0], &mut o, &mut m, nthr) {
            Ok(()) => {
                pout(out, "slc", &o);
                pmask(out, "slcm", &m);
            }
            Err(e) => perr(out, "slc", &e),
        }
    }
}

// ------------------------------------------------------------------------------------------------

fn jac(dir: &Path, p: &Params, out: &mut String) {
    let sz: Vec<u32> = (0..3).map(|k| i(p, "fsize", k) as u32).collect();
    let vxs: Vec<f64> = (0..3).map(|k| d(p, "fvxs", k)).collect();
    let ksp: Vec<u32> = (0..3).map(|k| i(p, "ksp", k) as u32).collect();
    let order = i(p, "order", 0) as i32;
    let cb = read_f64(&dir.join("coef.bin"));
    let aff = read_mat(&dir.join("aff.bin"));
    let field = |set: usize| {
        let mut fl = SplineField::new(&sz, &vxs, &ksp, order, 1).unwrap();
        let n = fl.coef_sz() as usize;
        fl.set_coef(&cb[set * n..(set + 1) * n]).unwrap();
        fl
    };
    let mkjac = || {
        let mut j = vol3(p, "jsize", "fvxs", 1);
        if let Some(s) = p.get("jextrap") {
            j.extrap = ext_of(&s[0]);
        }
        j
    };
    let pj = |out: &mut String, tag: &str, j: &Volume| {
        for &v in &j.data {
            pf(out, tag, v);
        }
    };
    let (sx, sy) = (d(p, "scale", 0), d(p, "scale", 1));
    for fi in 1..=3usize {
        let tag = format!("j1_{fi}");
        let mut fl = field(0);
        fl.scale_field(if fi == 2 { sy } else { sx });
        let mut j = mkjac();
        match warpfns::deffield2jacobian_1d(&mut fl, FieldIndex::from_index(fi), &mut j) {
            Ok(()) => pj(out, &tag, &j),
            Err(e) => perr(out, &tag, &e),
        }
    }
    {
        let (mut x, mut y, mut z) = (field(0), field(0), field(0));
        x.scale_field(sx);
        y.scale_field(sy);
        z.scale_field(0.0);
        let mut j = mkjac();
        match warpfns::deffield2jacobian(&mut x, &mut y, &mut z, &mut j) {
            Ok(()) => pj(out, "j3t", &j),
            Err(e) => perr(out, "j3t", &e),
        }
    }
    {
        let (mut x, mut y, mut z) = (field(0), field(1), field(2));
        let mut j = mkjac();
        match warpfns::deffield2jacobian_aff(&mut x, &mut y, &mut z, &aff, &mut j) {
            Ok(()) => pj(out, "j3a", &j),
            Err(e) => perr(out, "j3a", &e),
        }
    }
    {
        let (mut x, mut y, mut z) = (field(0), field(1), field(2));
        let mut jm = vol3(p, "jsize", "fvxs", 9);
        match warpfns::deffield2jacobian_matrix(&mut x, &mut y, &mut z, &aff, &mut jm) {
            Ok(()) => pj(out, "jm", &jm),
            Err(e) => perr(out, "jm", &e),
        }
    }
}

#[test]
#[ignore]
fn warpfns_probe() {
    let root = std::env::var("WF_PROBE_ROOT").expect("WF_PROBE_ROOT");
    let nthr: u32 = std::env::var("WF_PROBE_NTHR")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let mut dirs: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.join("params.txt").exists())
        .collect();
    dirs.sort();
    for dir in dirs {
        let p = read_params(&dir);
        let mut out = String::new();
        match p["mode"][0].as_str() {
            "spl" => {
                spl::<f32>(&dir, &p, "f_", |x| x, &mut out);
                spl::<f64>(&dir, &p, "d_", f64::from, &mut out);
            }
            "vol" => vol(&dir, &p, &mut out),
            "warp" => warp(&dir, &p, nthr, &mut out),
            _ => jac(&dir, &p, &mut out),
        }
        std::fs::write(dir.join(format!("replica{nthr}.txt")), out).unwrap();
    }
}
