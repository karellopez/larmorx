// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.volume against FreeSurfer 8.2.0's official code on every platform: the SHA-256 of the
//! official build's output lines for the probe's cases of seed 1, recorded by
//! `validation/probes/fs-volume/check.py --seed 1 --n <N>` on Linux x64 from the unit oracle's build,
//! against the port's output with `reproduce` (lines where FreeSurfer reads or writes memory it does
//! not own or crashes, FS-53, FS-58, FS-288, FS-290, FS-294, FS-300, FS-302 to FS-310, FS-314,
//! FS-315, FS-322 to FS-325, FS-327, and the
//! eigen-systems it never returns from, FS-47, FS-48, ITK-112, ITK-115, ITK-116, hashed as `SKIP`).

#[path = "fs_volume/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-13 (95 commands: transform.cpp's VOL_GEOM functions and `LTAreadEx` with
/// `USE_AVERAGE305` unset, `true` or empty, 3.1-35's voxel arithmetic, coordinates and geometry,
/// moves and head box, `MRIaverage`, 3.1-36's samplers, resolution, planes, fills, resampling by whole
/// factors and changes of type, frame-range samplers, the trilinear kernel and splatting, derivatives,
/// gradients, neighbour counts and linear transforms, 3.1-37's resampling onto a template, 3.1-38's
/// value replacement, the CMA labels and segmentation functions): 1,900 cases, 1,745 lines
/// identical, 155 hashed as `SKIP`.
const N: usize = 1900;
const OFFICIAL_SHA256: &str = "0b5f671441495eed2d7a7aa8001aa5317d3c1cf986fe06d3c1bf0a911498246b";
const SKIPPED: usize = 155;

/// `check.py`'s `SKIP`.
const SKIP: [&str; 27] = [
    "FS-288", "FS-53", "FS-58", "FS-290", "FS-294", "FS-47", "FS-48", "ITK-112", "ITK-115",
    "ITK-116", "FS-300", "FS-302", "FS-303", "FS-304", "FS-305", "FS-306", "FS-307", "FS-308",
    "FS-309", "FS-310", "FS-314", "FS-315", "FS-322", "FS-323", "FS-324", "FS-325", "FS-327",
];

#[test]
fn reproduce_gives_the_official_outputs() {
    let mut h = Sha256::new();
    let mut skipped = 0;
    for line in cases::generate(1, N) {
        let (out, ids) = cases::run_line(&line, UpstreamDefects::Reproduce);
        if ids.iter().any(|x| SKIP.contains(x)) {
            skipped += 1;
            h.update(b"SKIP\n");
        } else {
            h.update(out.as_bytes());
            h.update(b"\n");
        }
    }
    assert_eq!(skipped, SKIPPED);
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, OFFICIAL_SHA256);
}

/// The default differs from `reproduce` only where a defect it corrects is noted.
#[test]
fn fix_differs_only_with_a_notice() {
    for line in cases::generate(1, N) {
        let (r, _) = cases::run_line(&line, UpstreamDefects::Reproduce);
        let (f, ids) = cases::run_line(&line, UpstreamDefects::Fix);
        assert!(r == f || !ids.is_empty(), "{line}");
    }
}

/// `MRIextractInto` with an integral (negative) voxel size: the default gives FreeSurfer's start and
/// end, the sign of a zero included (found at seed 7: `-0 + 0 * -1` is +0 in FreeSurfer's integer
/// product, -0 in a float one).
#[test]
fn fix_keeps_freesurfers_start_for_integral_voxel_sizes() {
    let line = "extract 1 1 1 3 1 0 182 1 2 4 4 40000000 bf800000 bf800000 1 bf19999a 3dcccccd \
        3f7eb852 3f4ccccd 3f7eb852 bf800000 bf4ccccd bf800000 bf800000 c1a60000 c0e00000 c1f50000 \
        be99999a 37a7c5ac 00000000 - 0 2 c2878000 c2310000 425a0000 c22a0000 80000000 41a00000 \
        00000000 00000000 44fa0000 3a83126f 00000000 3ff8000000000000 3 1 0 0 -1 0 2 1 2 2 -1 1";
    let (r, _) = cases::run_line(line, UpstreamDefects::Reproduce);
    let (f, ids) = cases::run_line(line, UpstreamDefects::Fix);
    assert!(ids.is_empty(), "{ids:?}");
    assert_eq!(r, f);
}

/// FS-302: a sinc half-width whose window holds more than 128 voxels on an axis is an error in both
/// modes (FreeSurfer writes past its weight arrays); 64 is the widest that always fits.
#[test]
fn sinc_with_more_than_128_weights_is_an_error() {
    use larmorx_freesurfer_core::globals::Globals;
    use larmorx_freesurfer_mri::mri::{MRI_UCHAR, mri_alloc};
    use larmorx_freesurfer_volume::sample::sinc_sample_volume;
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut g = Globals::new(mode);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let m = mri_alloc(&mut g, &mut out, &mut err, 300, 1, 1, MRI_UCHAR).unwrap();
        let mut v = -7.0;
        assert!(sinc_sample_volume(&mut g, &mut err, &m, 150.5, 0.0, 0.0, 64, &mut v).is_ok());
        let r = sinc_sample_volume(&mut g, &mut err, &m, 150.5, 0.0, 0.0, 65, &mut v);
        assert!(r.is_err());
        assert!(g.notices.as_slice().iter().any(|n| n.defect.id == "FS-302"));
    }
}

/// `MRIfillFG` into a destination smaller than the source ends in both modes: the bytes FreeSurfer
/// writes where the destination holds no voxel are read back (the first probe run looped for ever).
#[test]
fn fill_into_a_smaller_destination_ends() {
    use larmorx_freesurfer_core::globals::Globals;
    use larmorx_freesurfer_mri::mri::{MRI_UCHAR, mri_alloc};
    use larmorx_freesurfer_volume::arith::Pair;
    use larmorx_freesurfer_volume::resize::mri_fill_fg;
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut g = Globals::new(mode);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let mut src = mri_alloc(&mut g, &mut out, &mut err, 4, 3, 2, MRI_UCHAR).unwrap();
        src.chunk.as_mut().unwrap().fill(200);
        let mut dst = mri_alloc(&mut g, &mut out, &mut err, 2, 1, 1, MRI_UCHAR).unwrap();
        let (_, npix) = mri_fill_fg(
            &mut g,
            &mut out,
            &mut err,
            Pair::Given(&src, &mut dst),
            0,
            0,
            0,
            100,
            1,
        )
        .unwrap();
        assert_eq!(npix, 24);
        assert_eq!(dst.chunk.as_ref().unwrap()[..2], [1, 1]);
    }
}

/// A volume geometry with unit cosines, voxels of `size` and its centre at `c` on each axis.
fn axes(m: &mut larmorx_freesurfer_mri::mri::Mri, size: f32, c: f32) {
    let vg = &mut m.vg;
    (vg.xsize, vg.ysize, vg.zsize) = (size, size, size);
    vg.ras_good_flag = 1;
    (vg.x_r, vg.x_a, vg.x_s) = (1.0, 0.0, 0.0);
    (vg.y_r, vg.y_a, vg.y_s) = (0.0, 1.0, 0.0);
    (vg.z_r, vg.z_a, vg.z_s) = (0.0, 0.0, 1.0);
    (vg.c_r, vg.c_a, vg.c_s) = (c, c, c);
}

/// FS-326: 280 votes for label 3 and 320 for label 7 in one template voxel: FreeSurfer's byte
/// counts stop at 255 and the lower label wins (`reproduce`); the default counts every vote.
#[test]
fn vote_counts_past_255() {
    use larmorx_freesurfer_core::globals::Globals;
    use larmorx_freesurfer_mri::mri::{MRI_UCHAR, mri_alloc};
    use larmorx_freesurfer_volume::resample::mri_resample;
    use larmorx_freesurfer_volume::sample::SAMPLE_VOTE;
    for (mode, want) in [(UpstreamDefects::Fix, 7u8), (UpstreamDefects::Reproduce, 3)] {
        let mut g = Globals::new(mode);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let mut src = mri_alloc(&mut g, &mut out, &mut err, 600, 1, 1, MRI_UCHAR).unwrap();
        for (i, b) in src.chunk.as_mut().unwrap()[..600].iter_mut().enumerate() {
            *b = if i < 280 { 3 } else { 7 };
        }
        axes(&mut src, 1.0, 0.0);
        let mut t = mri_alloc(&mut g, &mut out, &mut err, 1, 1, 1, MRI_UCHAR).unwrap();
        axes(&mut t, 1000.0, 500.0);
        let r = mri_resample(&mut g, &mut out, &mut err, &src, &t, SAMPLE_VOTE)
            .unwrap()
            .unwrap();
        assert_eq!(r.chunk.as_ref().unwrap()[0], want, "{mode}");
        assert!(g.notices.as_slice().iter().any(|n| n.defect.id == "FS-326"));
    }
}

/// FS-461: replacing a value in a two-frame byte volume: FreeSurfer's frame 0 only (`reproduce`),
/// every frame by default.
#[test]
fn byte_replacement_in_every_frame() {
    use larmorx_freesurfer_core::globals::Globals;
    use larmorx_freesurfer_mri::mri::{MRI_UCHAR, mri_alloc_sequence};
    use larmorx_freesurfer_volume::arith::{Pair, mri_replace_values};
    for (mode, second) in [(UpstreamDefects::Fix, 9u8), (UpstreamDefects::Reproduce, 4)] {
        let mut g = Globals::new(mode);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let mut m = mri_alloc_sequence(&mut g, &mut out, &mut err, 2, 1, 1, MRI_UCHAR, 2).unwrap();
        m.chunk.as_mut().unwrap().fill(4);
        mri_replace_values(&mut g, &mut out, &mut err, Pair::Same(&mut m), 4.0, 9.0).unwrap();
        let c = m.chunk.as_ref().unwrap();
        assert_eq!((c[0], c[1]), (9, 9), "{mode}");
        let n = c.len();
        assert_eq!(c[n - 1], second, "{mode}");
        assert!(g.notices.as_slice().iter().any(|n| n.defect.id == "FS-461"));
    }
}
