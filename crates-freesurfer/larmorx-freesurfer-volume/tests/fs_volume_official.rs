// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.volume against FreeSurfer 8.2.0's official code on every platform: the SHA-256 of the
//! official build's output lines for the probe's cases of seed 1, recorded by
//! `validation/probes/fs-volume/check.py --seed 1 --n <N>` on Linux x64 from the unit oracle's build,
//! against the port's output with `reproduce` (lines where FreeSurfer reads or writes memory it does
//! not own or crashes, FS-53, FS-58, FS-288, FS-290, FS-294, FS-300, FS-302 to FS-310, FS-314,
//! FS-315, FS-322 to FS-325, FS-327, FS-163, FS-330, FS-463, FS-465, FS-467, and the
//! eigen-systems it never returns from, FS-47, FS-48, ITK-112, ITK-115, ITK-116, hashed as `SKIP`).

#[path = "fs_volume/cases.rs"]
mod cases;

use larmorx_core::defects::UpstreamDefects;
use sha2::{Digest, Sha256};

/// Recorded 2026-11-13 (121 commands: transform.cpp's VOL_GEOM functions and `LTAreadEx` with
/// `USE_AVERAGE305` unset, `true` or empty, 3.1-35's voxel arithmetic, coordinates and geometry,
/// moves and head box, `MRIaverage`, 3.1-36's samplers, resolution, planes, fills, resampling by whole
/// factors and changes of type, frame-range samplers, the trilinear kernel and splatting, derivatives,
/// gradients, neighbour counts and linear transforms, 3.1-37's resampling onto a template, 3.1-38's
/// value replacement, the CMA labels and segmentation functions; relay 19: 3.1-37's statistics,
/// orientation strings, voxel-to-RAS matrices, neighbourhoods and labels, and 3.1-35's 2-D images):
/// 2,420 cases, 2,254 lines identical, 166 hashed as `SKIP`.
const N: usize = 2420;
const OFFICIAL_SHA256: &str = "3834ef65feb37ff13903d5bf1458c793ceae37307d8779a87e70c4b755ef40a7";
const SKIPPED: usize = 166;

/// `check.py`'s `SKIP`.
const SKIP: [&str; 38] = [
    "FS-288", "FS-53", "FS-58", "FS-290", "FS-294", "FS-47", "FS-48", "ITK-112", "ITK-115",
    "ITK-116", "FS-300", "FS-302", "FS-303", "FS-304", "FS-305", "FS-306", "FS-307", "FS-308",
    "FS-309", "FS-310", "FS-314", "FS-315", "FS-322", "FS-323", "FS-324", "FS-325", "FS-327",
    "FS-330", "FS-463", "FS-465", "FS-467", "FS-163", "FS-469", "FS-470", "FS-471", "FS-473",
    "FS-475", "FS-479",
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

/// FS-329: the statistics of a volume of 64-bit integers: FreeSurfer's 32-bit halves of the first
/// voxels (`reproduce`), the voxels themselves by default.
#[test]
fn long_statistics_read_the_voxels() {
    use larmorx_freesurfer_core::globals::Globals;
    use larmorx_freesurfer_mri::mri::{MRI_LONG, mri_alloc};
    use larmorx_freesurfer_volume::stats::{Stats, mri_stats};
    for (mode, want_max) in [
        (UpstreamDefects::Fix, 7.0f32),
        (UpstreamDefects::Reproduce, 5.0),
    ] {
        let mut g = Globals::new(mode);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let mut m = mri_alloc(&mut g, &mut out, &mut err, 2, 1, 1, MRI_LONG).unwrap();
        let c = m.chunk.as_mut().unwrap();
        c[..8].copy_from_slice(&5i64.to_le_bytes());
        c[8..16].copy_from_slice(&7i64.to_le_bytes());
        let mut s = Stats::default();
        mri_stats(&mut g, &m, &mut s);
        assert_eq!(s.max, want_max, "{mode}");
        assert!(g.notices.as_slice().iter().any(|n| n.defect.id == "FS-329"));
    }
}

/// FS-466: an RGB image's second frame: FreeSurfer reads it 1 byte per pixel into the first
/// (`reproduce`); the default reads its own pixels.
#[test]
fn rgb_image_frames_read_their_own_pixels() {
    use larmorx_freesurfer_core::globals::Globals;
    use larmorx_freesurfer_io::image::{PFRGB, image_alloc};
    use larmorx_freesurfer_mri::voxel::mri_get_vox_val;
    use larmorx_freesurfer_volume::image::image_to_mri;
    for (mode, want) in [
        (UpstreamDefects::Fix, 0x0a0b0c),
        (UpstreamDefects::Reproduce, 0x090c02),
    ] {
        let mut g = Globals::new(mode);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let mut im = image_alloc(&mut g, &mut err, 1, 2, PFRGB, 2, 0).unwrap();
        im.image.as_mut().unwrap()[..12].copy_from_slice(&[0, 1, 2, 12, 9, 9, 12, 11, 10, 0, 0, 0]);
        let m = image_to_mri(&mut g, &mut out, &mut err, &mut im).unwrap();
        // frame 1's first pixel: bytes 6..8 (its own), or bytes 2..4 (FreeSurfer's)
        let v = mri_get_vox_val(&mut g, &mut err, &m, 0, 0, 0, 1, 0).unwrap();
        assert_eq!(v as i32, want, "{mode}");
        assert!(g.notices.as_slice().iter().any(|n| n.defect.id == "FS-466"));
    }
}
