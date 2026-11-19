// SPDX-License-Identifier: Apache-2.0
//! The probe's fs.bspline commands (`specs/freesurfer-bspline.md` §9; the commands and encodings:
//! `validation/investigations/freesurfer-volume-probe/README.md`).

use larmorx_fs_core::bspline::BSpline;
use larmorx_fs_core::mri::geom::fixed_name;
use larmorx_fs_core::mri::{LinearTransform, Lta, Mri, types};
use larmorx_fs_core::volume::VolOut;
use larmorx_fs_core::{Process, Stop};

use super::args::{Line, bad};
use super::{D, UNSET, UNSETD, dst, getdst, getw, putres};

/// LT: `x0 y0 z0 sigma m_L m_dL m_last_dL type label src dst` (each geometry Gf).
fn getlt(p: &mut Process, l: &mut Line, lt: &mut LinearTransform) -> Result<(), Stop> {
    lt.x0 = l.getf();
    lt.y0 = l.getf();
    lt.z0 = l.getf();
    lt.sigma = l.getf();
    lt.m_l = l.getm(p)?;
    lt.m_dl = l.getm(p)?;
    lt.m_last_dl = l.getm(p)?;
    lt.kind = l.geti();
    lt.label = l.geti();
    l.getg_full(p, &mut lt.src)?;
    l.getg_full(p, &mut lt.dst)?;
    Ok(())
}

/// LTA: `<n> <type> <fscale> <subject>`, the transforms and inverse transforms.
fn getlta(p: &mut Process, l: &mut Line) -> Result<Lta, Stop> {
    let n = l.geti();
    let mut lta = p.lta_alloc(n, None)?;
    lta.kind = l.geti();
    lta.fscale = l.getf();
    lta.subject = fixed_name(&l.getx(), 1024);
    for i in 0..lta.xforms.len() {
        getlt(p, l, &mut lta.xforms[i])?;
    }
    for i in 0..lta.inv_xforms.len() {
        getlt(p, l, &mut lta.inv_xforms[i])?;
    }
    Ok(lta)
}

/// The points of `bsamp` and `bsraw`: `n`, then `x y z frame` each; per point the value and the
/// sample.
fn samples(p: &mut Process, l: &mut Line, b: &BSpline) -> Result<(), Stop> {
    let n = l.geti();
    for _ in 0..n {
        let (x, y, z) = (l.getd(), l.getd(), l.getd());
        let f = l.geti();
        let (r, v) = p.mri_sample_bspline(b, x, y, z, f)?;
        l.puti(i64::from(r));
        l.putd(v.unwrap_or(UNSETD));
    }
    Ok(())
}

/// A transform's result: into the source itself is the source.
fn putres_src(l: &mut Line, r: VolOut, a: &Mri, d: &D) {
    let r = match (r, d) {
        (VolOut::Given, D::Src) => VolOut::Src,
        (r, _) => r,
    };
    putres(l, r, a, None, d);
}

/// One of fs.bspline's commands, or `false` when `c` is none.
pub(super) fn call(p: &mut Process, l: &mut Line, c: &str) -> Result<bool, Stop> {
    match c {
        "bsp" => {
            let k = l.geti();
            let a = getw(p, l)?;
            let deg = l.geti();
            let b = match k {
                0 => None,
                1 => Some(p.mri_alloc_bspline(
                    a.geom.width,
                    a.geom.height,
                    a.geom.depth,
                    a.nframes,
                )?),
                2 => {
                    let mut b = p.mri_alloc_bspline(1, 1, 1, 1)?;
                    p.mri_free(&mut b.coeff);
                    b.coeff = Some(getw(p, l)?);
                    Some(b)
                }
                3 => {
                    let mut b = p.mri_alloc_bspline(1, 1, 1, 1)?;
                    p.mri_free(&mut b.coeff);
                    Some(b)
                }
                _ => bad("unknown B-spline kind"),
            };
            let given = b.is_some();
            let r = p.mri_to_bspline(&a, b, deg)?;
            l.out(if given { "given" } else { "new" });
            l.puti(i64::from(r.degree));
            l.puti(i64::from(r.srctype));
            l.puti(i64::from(r.srcneg));
            l.put_volume(r.coeff.as_ref().expect("coefficients"));
        }
        "bsamp" => {
            let deg = l.geti();
            let mut a = getw(p, l)?;
            a.outside_val = l.getd();
            let b = p.mri_to_bspline(&a, None, deg)?;
            samples(p, l, &b)?;
        }
        "bsraw" => {
            let (deg, neg) = (l.geti(), l.geti());
            let a = getw(p, l)?;
            let mut b =
                p.mri_alloc_bspline(a.geom.width, a.geom.height, a.geom.depth, a.nframes)?;
            b.degree = deg;
            b.srctype = a.kind;
            b.srcneg = neg;
            let c = b.coeff.as_mut().expect("coefficients");
            if a.kind == types::FLOAT
                && let (Some(src), Some(v)) = (a.voxels.as_ref(), c.voxels.as_mut())
            {
                let n = src.len().min(v.len());
                v[..n].copy_from_slice(&src[..n]);
            }
            c.outside_val = l.getd();
            samples(p, l, &b)?;
        }
        "bsseq" => {
            let deg = l.geti();
            let mut a = getw(p, l)?;
            a.outside_val = l.getd();
            let b = p.mri_to_bspline(&a, None, deg)?;
            let n = l.geti();
            for _ in 0..n {
                let pt = [l.getd(), l.getd(), l.getd()];
                let (first, last) = (l.geti(), l.geti());
                if first < 0 {
                    bad("a first frame below 0 writes before the array");
                }
                let len = (first.max(last) + 1).max(1) as usize;
                let mut vals = vec![UNSET; len];
                let r = p.mri_sample_seq_bspline(&b, pt, &mut vals, first, last)?;
                l.puti(i64::from(r));
                for f in first..=last {
                    l.putf(vals[f as usize]);
                }
            }
        }
        "bslxf" | "bsras" | "bslta" => {
            let deg = l.geti();
            let mut a = getw(p, l)?;
            let mut d = getdst(p, l, false)?;
            let r = if c == "bslta" {
                let mut lta = getlta(p, l)?;
                match l.geti() {
                    0 => p.env.remove(b"USE_AVERAGE305"),
                    1 => p.env.set(b"USE_AVERAGE305", b"true"),
                    _ => p.env.set(b"USE_AVERAGE305", b""),
                }
                let b = p.mri_to_bspline(&a, None, deg)?;
                let out = match &mut d {
                    D::New | D::Other => None,
                    D::Src => Some(&mut a),
                    D::W(m) => Some(m.as_mut()),
                };
                p.lta_transform_bspline(&b, out, &mut lta)?
            } else {
                let m = l.getm(p)?;
                let b = p.mri_to_bspline(&a, None, deg)?;
                let out = match &mut d {
                    D::New | D::Other => None,
                    D::Src => Some(&mut a),
                    D::W(m) => Some(m.as_mut()),
                };
                if c == "bslxf" {
                    p.mri_linear_transform_bspline(&b, out, m.as_ref())?
                } else {
                    p.mri_apply_ras_linear_transform_bspline(&b, out, m.as_ref())?
                }
            };
            putres_src(l, r, &a, &d);
        }
        "bsdown" | "bsup" => {
            let mut a = getw(p, l)?;
            let mut d = getdst(p, l, false)?;
            let r = if c == "bsdown" {
                p.mri_downsample2_bspline(&mut a, dst(&mut d))?
            } else {
                p.mri_upsample2_bspline(&mut a, dst(&mut d))?
            };
            putres(l, r, &a, None, &d);
        }
        "bsfree" => {
            let k = l.geti();
            let mut b = None;
            if k >= 1 {
                b = Some(p.mri_alloc_bspline(1, 1, 1, 1)?);
            }
            if k == 1
                && let Some(s) = b.as_mut()
            {
                p.mri_free(&mut s.coeff);
            }
            let r = p.mri_free_bspline(&mut b);
            l.puti(i64::from(r));
            l.out(if b.is_some() { "kept" } else { "none" });
        }
        _ => return Ok(false),
    }
    Ok(true)
}
