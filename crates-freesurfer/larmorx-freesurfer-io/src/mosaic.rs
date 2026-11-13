// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Mosaics as FreeSurfer 8.2.0 converts them (`utils/mosaic.cpp`, queue item 3.1-27): a volume's
//! column, row and slice (all from 0) to a pixel of a mosaic of tiles (one slice per tile, the
//! tiles row by row), and back; `CheckMosaic`, the diagnostic that prints a round trip.
//!
//! The arithmetic is the official build's (GCC 8.4.0 at `-O3`): 32-bit integer division
//! truncating toward zero, products and sums wrapping at 32 bits, and the bounds tested as
//! `0 <= x < n` (the source's `x > n - 1`, which differs only for `n` = -2^31). A division by
//! zero, or of -2^31 by -1, ends FreeSurfer with `SIGFPE`; here both modes return 1 after an
//! error message (FS-229).

use std::io::Write;

use larmorx_core::defects::FS_229;
use larmorx_freesurfer_core::globals::Globals;

/// `a / b` and `a % b` as `idiv`, or `None` where `idiv` faults.
fn idiv(a: i32, b: i32) -> Option<(i32, i32)> {
    Some((a.checked_div(b)?, a.checked_rem(b)?))
}

fn no_tiles(g: &mut Globals, err: &mut dyn Write, who: &str, sizes: [i32; 4]) -> i32 {
    let [ncvol, nrvol, ncmos, nrmos] = sizes;
    g.notices.add(
        &FS_229,
        format!(
            "{who}: an image of {ncvol} x {nrvol} in a mosaic of {ncmos} x {nrmos}: a division \
             by zero or overflow, where FreeSurfer crashes (SIGFPE)"
        ),
    );
    let _ = writeln!(
        err,
        "ERROR: {who}: the image ({ncvol} x {nrvol}) cannot tile the mosaic ({ncmos} x {nrmos})"
    );
    1
}

/// The messages of a mosaic whose columns or rows are not a whole number of the image's.
fn not_multiple(err: &mut dyn Write, who: &str, what: (&str, &str), mosaic: i32, image: i32) {
    let _ = write!(
        err,
        "ERROR: {who}: number of {} in the mosaic ({mosaic}) \nis not an integer multiple of the \
         number of {} in \nthe image ({image})\n",
        what.0, what.1
    );
}

/// `VolSS2MosSS(cvol, rvol, svol, ncvol, nrvol, ncmos, nrmos, &cmos, &rmos, OutOfBounds)`: the
/// mosaic pixel (`cmos`, `rmos`) of the volume's (`cvol`, `rvol`, `svol`); `out_of_bounds` (when
/// given) 1 when the pixel is outside the mosaic, else 0. Returns 0; 1 (`cmos`, `rmos` and
/// `out_of_bounds` unchanged) after a message on standard error when the mosaic's columns or rows
/// are not a whole number of the image's.
#[allow(clippy::too_many_arguments)]
pub fn vol_ss2_mos_ss(
    g: &mut Globals,
    err: &mut dyn Write,
    cvol: i32,
    rvol: i32,
    svol: i32,
    ncvol: i32,
    nrvol: i32,
    ncmos: i32,
    nrmos: i32,
    cmos: &mut i32,
    rmos: &mut i32,
    out_of_bounds: Option<&mut i32>,
) -> i32 {
    let who = "VolSS2MosSS";
    let sizes = [ncvol, nrvol, ncmos, nrmos];
    let Some((nctmos, crem)) = idiv(ncmos, ncvol) else {
        return no_tiles(g, err, who, sizes);
    };
    if crem != 0 {
        not_multiple(err, who, ("columns", "columns"), ncmos, ncvol);
        return 1;
    }
    let Some((_, rrem)) = idiv(nrmos, nrvol) else {
        return no_tiles(g, err, who, sizes);
    };
    if rrem != 0 {
        not_multiple(err, who, ("row", "rows"), nrmos, nrvol);
        return 1;
    }
    let Some((rtmos, ctmos)) = idiv(svol, nctmos) else {
        return no_tiles(g, err, who, sizes);
    };
    *cmos = ctmos.wrapping_mul(ncvol).wrapping_add(cvol);
    *rmos = rtmos.wrapping_mul(nrvol).wrapping_add(rvol);
    if let Some(oob) = out_of_bounds {
        let out = *cmos < 0 || *cmos >= ncmos || *rmos < 0 || *rmos >= nrmos;
        *oob = i32::from(out);
    }
    0
}

/// `MosSS2VolSS(cmos, rmos, ncmos, nrmos, ncvol, nrvol, nsvol, &cvol, &rvol, &svol,
/// OutOfBounds)`: the volume's (`cvol`, `rvol`, `svol`) of the mosaic pixel (`cmos`, `rmos`);
/// `out_of_bounds` 1 when it is outside the volume of `nsvol` slices. The returns and messages of
/// [`vol_ss2_mos_ss`] (its messages name it `MosSS2Vol`).
#[allow(clippy::too_many_arguments)]
pub fn mos_ss2_vol_ss(
    g: &mut Globals,
    err: &mut dyn Write,
    cmos: i32,
    rmos: i32,
    ncmos: i32,
    nrmos: i32,
    ncvol: i32,
    nrvol: i32,
    nsvol: i32,
    cvol: &mut i32,
    rvol: &mut i32,
    svol: &mut i32,
    out_of_bounds: Option<&mut i32>,
) -> i32 {
    let who = "MosSS2VolSS";
    let sizes = [ncvol, nrvol, ncmos, nrmos];
    let Some((nctmos, crem)) = idiv(ncmos, ncvol) else {
        return no_tiles(g, err, who, sizes);
    };
    if crem != 0 {
        not_multiple(err, "MosSS2Vol", ("columns", "columns"), ncmos, ncvol);
        return 1;
    }
    let Some((_, rrem)) = idiv(nrmos, nrvol) else {
        return no_tiles(g, err, who, sizes);
    };
    if rrem != 0 {
        not_multiple(err, "MosSS2Vol", ("row", "rows"), nrmos, nrvol);
        return 1;
    }
    let Some((ctmos, c)) = idiv(cmos, ncvol) else {
        return no_tiles(g, err, who, sizes);
    };
    let Some((rtmos, r)) = idiv(rmos, nrvol) else {
        return no_tiles(g, err, who, sizes);
    };
    *cvol = c;
    *rvol = r;
    *svol = ctmos.wrapping_add(rtmos.wrapping_mul(nctmos));
    if let Some(oob) = out_of_bounds {
        let out = *cvol >= ncvol
            || *cvol < 0
            || *rvol < 0
            || *rvol >= nrvol
            || *svol < 0
            || *svol >= nsvol;
        *oob = i32::from(out);
    }
    0
}

fn check_line(out: &mut dyn Write, v: [i32; 11]) {
    let _ = writeln!(
        out,
        "{:5}  {:2} {:2} {:2}   {:3} {:3}    {:2} {:2} {:2}  {}  {}",
        v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8], v[9], v[10]
    );
}

/// `CheckMosaic()`: a volume of 8 x 3 x 11 in a mosaic of 4 x 3 tiles, each voxel to the mosaic
/// and back, one line per voxel on standard output (its number, the voxel, the pixel, the voxel
/// back, the out-of-bounds flag and 1 when the round trip agrees), a rule, then the slice past
/// the last (11) both ways, printed with the loops' last values. Returns 0.
pub fn check_mosaic(g: &mut Globals, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let (ncvol, nrvol, nsvol) = (8, 3, 11);
    let (ncmos, nrmos) = (4 * ncvol, 3 * nrvol);
    let (mut cmos, mut rmos, mut oob) = (0, 0, 0);
    let (mut c2, mut r2, mut s2) = (0, 0, 0);
    let mut ok = 0;
    let mut n = 0;
    for svol in 0..nsvol {
        for rvol in 0..nrvol {
            for cvol in 0..ncvol {
                let (nc, nr) = (ncvol, nrvol);
                vol_ss2_mos_ss(
                    g,
                    err,
                    cvol,
                    rvol,
                    svol,
                    nc,
                    nr,
                    ncmos,
                    nrmos,
                    &mut cmos,
                    &mut rmos,
                    Some(&mut oob),
                );
                mos_ss2_vol_ss(
                    g,
                    err,
                    cmos,
                    rmos,
                    ncmos,
                    nrmos,
                    nc,
                    nr,
                    nsvol,
                    &mut c2,
                    &mut r2,
                    &mut s2,
                    Some(&mut oob),
                );
                ok = i32::from(cvol == c2 && rvol == r2 && svol == s2);
                check_line(out, [n, cvol, rvol, svol, cmos, rmos, c2, r2, s2, oob, ok]);
                n += 1;
            }
        }
    }
    let _ = out.write_all(b"-----------------------------------\n");
    // the loops' variables after their ends
    let (cvol, rvol, svol) = (ncvol, nrvol, nsvol);
    vol_ss2_mos_ss(
        g,
        err,
        0,
        0,
        11,
        ncvol,
        nrvol,
        ncmos,
        nrmos,
        &mut cmos,
        &mut rmos,
        Some(&mut oob),
    );
    check_line(out, [n, cvol, rvol, svol, cmos, rmos, c2, r2, s2, oob, ok]);
    mos_ss2_vol_ss(
        g,
        err,
        cmos,
        rmos,
        ncmos,
        nrmos,
        ncvol,
        nrvol,
        nsvol,
        &mut c2,
        &mut r2,
        &mut s2,
        Some(&mut oob),
    );
    check_line(out, [n, cvol, rvol, svol, cmos, rmos, c2, r2, s2, oob, ok]);
    0
}
