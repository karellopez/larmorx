// SPDX-License-Identifier: Apache-2.0
//! The probe's commands of part 10c (`specs/freesurfer-volume.md` §52): the volume histograms
//! (the commands and the histogram encoding H:
//! `validation/investigations/freesurfer-volume-probe/README.md`).

use larmorx_fs_core::maths::Returned;
use larmorx_fs_core::maths::histo::Histogram;
use larmorx_fs_core::mri::{Mri, Region};
use larmorx_fs_core::{Process, Stop};

use super::args::Line;
use super::{dst, getdst, getreg, getw, paint, putres};

/// A painted W.
fn getwp(p: &mut Process, l: &mut Line) -> Result<Mri, Stop> {
    let mut m = getw(p, l)?;
    paint(p, l, &mut m)?;
    Ok(m)
}

/// H, a given histogram: `null`, or `<alloc> <nbins> <bin_size> <min> <max>`, the `nbins` bin
/// values and the `nbins` counts (`HISTOalloc(alloc)`, the fields written over it).
fn geth(p: &mut Process, l: &mut Line) -> Result<Option<Histogram>, Stop> {
    if l.take("null") {
        return Ok(None);
    }
    let alloc = l.geti();
    let nbins = l.geti();
    let mut h = p.histo_alloc(alloc)?;
    h.nbins = nbins;
    h.bin_size = l.getf();
    h.min = l.getf();
    h.max = l.getf();
    for b in 0..nbins.max(0) {
        let v = l.getf();
        h.set_bin(b, v);
    }
    for b in 0..nbins.max(0) {
        let v = l.getf();
        h.set_count(b, v);
    }
    Ok(Some(h))
}

/// A histogram (printed): `null`, or `nbins max_bins bin_size min max`, then the values and the
/// counts of its first `min(nbins, max_bins)` bins.
fn puth(l: &mut Line, h: Option<&Histogram>) {
    let Some(h) = h else {
        l.out("null");
        return;
    };
    l.puti(i64::from(h.nbins));
    l.puti(i64::from(h.max_bins));
    l.putf(h.bin_size);
    l.putf(h.min);
    l.putf(h.max);
    let n = h.nbins.min(h.max_bins).max(0);
    for b in 0..n {
        l.putf(h.bin(b));
    }
    for b in 0..n {
        l.putf(h.count(b));
    }
}

/// A histogram result: `given` when the given histogram was returned, else the histogram; then
/// the given histogram's state.
fn puthres(l: &mut Line, r: &Returned<Histogram>, given: Option<&Histogram>) {
    match r {
        Returned::Output if given.is_some() => l.out("given"),
        Returned::New(h) => puth(l, Some(h)),
        _ => l.out("null"),
    }
    if let Some(g) = given {
        puth(l, Some(g));
    }
}

/// R that FreeSurfer's histograms need (without one it crashes): the call ends here.
fn region(l: &mut Line) -> Result<Region, Stop> {
    getreg(l).ok_or(Stop::Exit(1))
}

/// `self` (the volume itself) or a painted W.
fn self_or_w(p: &mut Process, l: &mut Line) -> Result<Option<Mri>, Stop> {
    if l.take("self") {
        return Ok(None);
    }
    Ok(Some(getwp(p, l)?))
}

/// One of part 10c's commands, or `false` when `c` is none.
pub(super) fn call(p: &mut Process, l: &mut Line, c: &str) -> Result<bool, Stop> {
    match c {
        "hreg" => {
            let a = getwp(p, l)?;
            let mut g = geth(p, l)?;
            let n = l.geti();
            let r = region(l)?;
            p.flush();
            let res = p.mri_histogram_region(&a, n, g.as_mut(), &r)?;
            p.flush();
            puthres(l, &res, g.as_ref());
        }
        "hvox" => {
            let a = getwp(p, l)?;
            let mut g = geth(p, l)?;
            let n = l.geti();
            let (x, y, z, w) = (l.geti(), l.geti(), l.geti(), l.geti());
            let t = if l.take("null") {
                None
            } else {
                Some(getwp(p, l)?)
            };
            let th = l.getf();
            p.flush();
            let res = p.mri_histogram_voxel(&a, n, g.as_mut(), (x, y, z), w, t.as_ref(), th)?;
            p.flush();
            puthres(l, &res, g.as_ref());
        }
        "hthr" => {
            let a = getwp(p, l)?;
            let mut g = geth(p, l)?;
            let n = l.geti();
            let r = region(l)?;
            let t = getwp(p, l)?;
            let (th, f) = (l.getf(), l.geti());
            p.flush();
            let res = p.mri_histogram_region_with_threshold(&a, n, g.as_mut(), &r, &t, th, f)?;
            p.flush();
            puthres(l, &res, g.as_ref());
        }
        "hist" => {
            let a = getwp(p, l)?;
            let n = l.geti();
            p.flush();
            let h = p.mri_histogram(&a, n)?;
            p.flush();
            puth(l, Some(&h));
        }
        "geq" => {
            let a = getwp(p, l)?;
            let mut g = geth(p, l)?;
            let (lo, hi, norm) = (l.geti(), l.geti(), l.geti());
            p.flush();
            let res = p.mri_get_equalize_histo(&a, g.as_mut(), lo, hi, norm != 0)?;
            p.flush();
            puthres(l, &res, g.as_ref());
        }
        "heq" => {
            let mut a = getwp(p, l)?;
            let mut b = getwp(p, l)?;
            let mut d = getdst(p, l, true)?;
            let (lo, hi) = (l.geti(), l.geti());
            p.flush();
            let r = p.mri_histo_equalize(&mut a, &mut b, dst(&mut d), lo, hi)?;
            p.flush();
            putres(l, r, &a, Some(&b), &d);
        }
        "hlr" => {
            let a = getwp(p, l)?;
            let lab = self_or_w(p, l)?;
            let r = region(l)?;
            let (label, n) = (l.geti(), l.geti());
            p.flush();
            let h = p.mri_histogram_label_region(&a, lab.as_ref().unwrap_or(&a), &r, label, n)?;
            p.flush();
            puth(l, h.as_ref());
        }
        "hlab" => {
            let a = getwp(p, l)?;
            let lab = self_or_w(p, l)?;
            let (label, n) = (l.geti(), l.geti());
            p.flush();
            let h = p.mri_histogram_label(&a, lab.as_ref().unwrap_or(&a), label, n)?;
            p.flush();
            puth(l, Some(&h));
        }
        "hsegm" => {
            let mut a = getwp(p, l)?;
            let mut d = getdst(p, l, false)?;
            let (wl, wh, gh, ws) = (l.geti(), l.geti(), l.geti(), l.geti());
            let sg = l.getf();
            p.flush();
            let r = p.mri_histo_segment(&mut a, dst(&mut d), wl, wh, gh, ws, sg)?;
            p.flush();
            putres(l, r, &a, None, &d);
        }
        _ => return Ok(false),
    }
    Ok(true)
}
