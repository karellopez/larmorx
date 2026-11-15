// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The atlas as volumes: `GCAbuildMostLikelyVolume`, `GCAbuildMostLikelyLabelVolume`,
//! `GCAbuildMostLikelyLabelProbabilityVolume` (`utils/gca.cpp`) and `readGCA` (`utils/mriio.cpp`
//! 12025-12084, `mri_read`'s reader of type `MRI_GCA_FILE`), of FreeSurfer 8.2.0.
//!
//! At each voxel of the volume the prior point and the node it maps to (the voxel through the
//! prior grid's RAS-to-voxel matrix, rounded and clamped; the node from the prior point): the
//! most likely label is the prior point's label of the largest prior (the last of equal ones),
//! its prior the probability, and the mean of the node's class of that label (the last such
//! class) the intensity; a voxel outside the grids is 0, one whose prior point has no label or
//! whose node has no class of the label is left as it is.

use std::io::Write;
use std::path::Path;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::sse::mul;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_SHORT, Mri, SAMPLE_NEAREST, mri_alloc_sequence};
use larmorx_freesurfer_mri::voxel::mri_set_vox_val;
use larmorx_freesurfer_volume::arith::{Pair, mri_copy_frames};
use larmorx_freesurfer_volume::resample::mri_resample;
use larmorx_freesurfer_volume::resize::mri_set_resolution;

use crate::gca::{
    Gca, GcaPrior, gca_copy_dc_to_mri, gca_voxel_to_node, gca_voxel_to_prior, voxel_to_prior_matrix,
};
use crate::read::gca_read;

/// The prior point's most likely label and its prior (`gcap->nlabels > 0`): the largest prior,
/// the last of equal ones (`>=`), compared as `double`s. The official build (its object code)
/// passes a point's only prior on as the `float` it is, and keeps the largest of two or more in a
/// `double` made a `float` again: there a signalling NaN comes back quiet. Written here without
/// the conversions, so that debug and release builds agree.
fn most_likely(gcap: &GcaPrior) -> (i32, f32) {
    let mut max_prior = gcap.priors[0];
    let mut max_label = i32::from(gcap.labels[0]);
    for n in 1..gcap.nlabels as usize {
        if f64::from(gcap.priors[n]) >= f64::from(max_prior) {
            max_prior = gcap.priors[n];
            max_label = i32::from(gcap.labels[n]);
        }
    }
    if gcap.nlabels >= 2 && max_prior.is_nan() {
        max_prior = f32::from_bits(max_prior.to_bits() | 0x0040_0000);
    }
    (max_label, max_prior)
}

/// A voxel's prior point and node (`None` outside either grid, for which the builders write 0),
/// or the prior point's labels when it has none (`Some(None)`).
fn lookup<'a>(
    g: &mut Globals,
    gca: &'a Gca,
    m: &larmorx_freesurfer_core::affine::AffineMatrix,
    v: [i32; 3],
) -> Option<Option<(&'a GcaPrior, [i32; 3])>> {
    let (nc, n) = gca_voxel_to_node(g, gca, m, v);
    if nc != NO_ERROR {
        return None;
    }
    let (pc, p) = gca_voxel_to_prior(g, gca, m, v);
    if pc != NO_ERROR {
        return None;
    }
    let gcap = gca.prior(p[0], p[1], p[2]).expect("inside the prior grid");
    if gcap.nlabels <= 0 {
        return Some(None);
    }
    Some(Some((gcap, n)))
}

/// `GCAbuildMostLikelyVolume(gca, mri)` (`mri` given): the means of each voxel's most likely
/// class, one frame per input; the program ends when the volume's frame count is not the
/// atlas's input count (`GCAbuildMostLikelyVolume: mri->frames (<f>) does not match
/// gca->ninputs (<n>)`).
pub fn gca_build_most_likely_volume(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    gca: &Gca,
    mri: &mut Mri,
) -> Result<(), Fatal> {
    gca_copy_dc_to_mri(g, out, err, gca, mri)?;
    if mri.nframes != gca.ninputs {
        let message = format!(
            "GCAbuildMostLikelyVolume: mri->frames ({}) does not match gca->ninputs ({})",
            mri.nframes, gca.ninputs
        );
        log::error_exit(g, err, ERROR_BADPARM, &message)?;
    }
    let m = voxel_to_prior_matrix(gca, mri);
    let (w, h, d) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                match lookup(g, gca, &m, [x, y, z]) {
                    None => {
                        for r in 0..gca.ninputs {
                            mri_set_vox_val(g, err, mri, x, y, z, r, 0.0, 0)?;
                        }
                    }
                    Some(None) => {}
                    Some(Some((gcap, n))) => {
                        let (max_label, _) = most_likely(gcap);
                        let gcan = gca.node(n[0], n[1], n[2]);
                        let gc_max = gcan
                            .labels
                            .iter()
                            .zip(&gcan.gcs)
                            .filter(|(l, _)| i32::from(**l) == max_label)
                            .map(|(_, gc)| gc)
                            .last();
                        let Some(gc) = gc_max else {
                            continue;
                        };
                        for r in 0..gca.ninputs {
                            let v = gc.means.get(r as usize).copied().unwrap_or(0.0);
                            mri_set_vox_val(g, err, mri, x, y, z, r, v, 0)?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// `GCAbuildMostLikelyLabelVolume(gca, mri)` (`mri` given): each voxel's most likely label in
/// frame 0.
pub fn gca_build_most_likely_label_volume(
    g: &mut Globals,
    err: &mut dyn Write,
    gca: &Gca,
    mri: &mut Mri,
) -> Result<(), Fatal> {
    let m = voxel_to_prior_matrix(gca, mri);
    let (w, h, d) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                match lookup(g, gca, &m, [x, y, z]) {
                    None => {
                        mri_set_vox_val(g, err, mri, x, y, z, 0, 0.0, 0)?;
                    }
                    Some(None) => {}
                    Some(Some((gcap, _))) => {
                        let (max_label, _) = most_likely(gcap);
                        mri_set_vox_val(g, err, mri, x, y, z, 0, max_label as f32, 0)?;
                    }
                }
            }
        }
    }
    Ok(())
}

/// `GCAbuildMostLikelyLabelProbabilityVolume(gca)`: a float volume on the prior grid (voxel
/// sizes the atlas's times the prior spacing, its geometry, one frame per input) holding each
/// voxel's largest prior in frame 0.
pub fn gca_build_most_likely_label_probability_volume(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    gca: &Gca,
) -> Result<Mri, Fatal> {
    let mut mri = mri_alloc_sequence(
        g,
        out,
        err,
        gca.prior_width,
        gca.prior_height,
        gca.prior_depth,
        MRI_FLOAT,
        gca.ninputs,
    )?;
    mri.vg.xsize = mul(gca.xsize, gca.prior_spacing);
    mri.vg.ysize = mul(gca.ysize, gca.prior_spacing);
    mri.vg.zsize = mul(gca.zsize, gca.prior_spacing);
    gca_copy_dc_to_mri(g, out, err, gca, &mut mri)?;
    let m = voxel_to_prior_matrix(gca, &mri);
    let (w, h, d) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                match lookup(g, gca, &m, [x, y, z]) {
                    None => {
                        mri_set_vox_val(g, err, &mut mri, x, y, z, 0, 0.0, 0)?;
                    }
                    Some(None) => {}
                    Some(Some((gcap, _))) => {
                        let (_, max_prior) = most_likely(gcap);
                        mri_set_vox_val(g, err, &mut mri, x, y, z, 0, max_prior, 0)?;
                    }
                }
            }
        }
    }
    Ok(mri)
}

/// A float volume of `frames` frames and the atlas's geometry with voxel sizes `sizes`.
#[allow(clippy::too_many_arguments)]
fn grid_volume(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    gca: &Gca,
    whd: [i32; 3],
    mtype: i32,
    frames: i32,
    sizes: [f32; 3],
) -> Result<Mri, Fatal> {
    let mut m = mri_alloc_sequence(g, out, err, whd[0], whd[1], whd[2], mtype, frames)?;
    mri_set_resolution(g, out, err, &mut m, sizes[0], sizes[1], sizes[2])?;
    gca_copy_dc_to_mri(g, out, err, gca, &mut m)?;
    Ok(m)
}

/// `readGCA(fname, start_frame, end_frame)`: the atlas `fname` as a volume (`reading frame <f> of
/// gca` first); by `start_frame`: 0, the node grid's means (one frame); 1, the prior grid's most
/// likely labels (16-bit integers); 2, the prior grid's largest priors (`interpreting as
/// probability volume`); any other, three float frames over the atlas's own volume (its width,
/// height, depth and voxel sizes): the means, the labels, and the priors resampled (nearest
/// neighbour) from the prior grid. `None` when the atlas cannot be read (after `GCAread`'s
/// message).
pub fn read_gca(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
    start_frame: i32,
    _end_frame: i32,
) -> Result<Option<Mri>, Fatal> {
    let Some(gca) = gca_read(g, out, err, dir, fname)? else {
        return Ok(None);
    };
    let _ = writeln!(out, "reading frame {start_frame} of gca");
    let sizes = [gca.xsize, gca.ysize, gca.zsize];
    let mri = match start_frame {
        0 => {
            let s = gca.node_spacing;
            let mut mri = grid_volume(
                g,
                out,
                err,
                &gca,
                [gca.node_width, gca.node_height, gca.node_depth],
                MRI_FLOAT,
                1,
                [mul(sizes[0], s), mul(sizes[1], s), mul(sizes[2], s)],
            )?;
            gca_build_most_likely_volume(g, out, err, &gca, &mut mri)?;
            mri
        }
        1 => {
            let s = gca.prior_spacing;
            let mut mri = grid_volume(
                g,
                out,
                err,
                &gca,
                [gca.prior_width, gca.prior_height, gca.prior_depth],
                MRI_SHORT,
                1,
                [mul(sizes[0], s), mul(sizes[1], s), mul(sizes[2], s)],
            )?;
            gca_build_most_likely_label_volume(g, err, &gca, &mut mri)?;
            mri
        }
        2 => {
            let _ = out.write_all(b"interpreting as probability volume\n");
            gca_build_most_likely_label_probability_volume(g, out, err, &gca)?
        }
        _ => {
            let whd = [gca.width, gca.height, gca.depth];
            let mut mri = mri_alloc_sequence(g, out, err, whd[0], whd[1], whd[2], MRI_FLOAT, 3)?;
            let mut means = mri_alloc_sequence(g, out, err, whd[0], whd[1], whd[2], MRI_FLOAT, 1)?;
            let mut labels = mri_alloc_sequence(g, out, err, whd[0], whd[1], whd[2], MRI_FLOAT, 1)?;
            mri_set_resolution(g, out, err, &mut mri, sizes[0], sizes[1], sizes[2])?;
            mri_set_resolution(g, out, err, &mut labels, sizes[0], sizes[1], sizes[2])?;
            mri_set_resolution(g, out, err, &mut means, sizes[0], sizes[1], sizes[2])?;
            gca_copy_dc_to_mri(g, out, err, &gca, &mut mri)?;
            gca_copy_dc_to_mri(g, out, err, &gca, &mut means)?;
            gca_copy_dc_to_mri(g, out, err, &gca, &mut labels)?;
            gca_build_most_likely_volume(g, out, err, &gca, &mut means)?;
            gca_build_most_likely_label_volume(g, err, &gca, &mut labels)?;
            let pvals = gca_build_most_likely_label_probability_volume(g, out, err, &gca)?;
            mri_copy_frames(g, out, err, Pair::Given(&means, &mut mri), 0, 0, 0)?;
            mri_copy_frames(g, out, err, Pair::Given(&labels, &mut mri), 0, 0, 1)?;
            let resampled = mri_resample(g, out, err, &pvals, &means, SAMPLE_NEAREST)?;
            if let Some(r) = &resampled {
                mri_copy_frames(g, out, err, Pair::Given(r, &mut mri), 0, 0, 2)?;
            }
            mri
        }
    };
    Ok(Some(mri))
}

/// Registers [`read_gca`] with fs.io, whose `mri_read` reads GCA atlases through it
/// ([`larmorx_freesurfer_io::upper`]).
pub fn register_io() {
    larmorx_freesurfer_io::upper::register(|u| {
        u.read_gca = Some(read_gca);
    });
}
