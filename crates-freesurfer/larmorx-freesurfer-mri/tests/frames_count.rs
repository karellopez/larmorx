// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FS-760: a volume of more frame records than 16 GiB (more than 13,338,407 frames) is refused
//! before any frame is made, with FreeSurfer's message and status (`error: Cannot allocate
//! memory`, `error: MRIalloc: could not allocate <n> frame`, 254), in both modes. FreeSurfer
//! 8.2.0's `mri_info` (measured in a 12 GiB memory box, 2026-11-15): 2,000,000 frames of one voxel
//! 3.2 GB and exit 0; 12,000,000 and 20,000,000 killed by the box; 449,657,401 refused at once
//! (exit 254, 10 MB): the outcome between depends on the machine's memory.

use larmorx_core::defects::{FS_760, UpstreamDefects};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_mri::mri::{MRI_UCHAR, Mri, frames_calloc_fails};

fn alloc(mode: UpstreamDefects, nframes: i32, voxels: bool) -> (Globals, u8, String) {
    let mut g = Globals::new(mode);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let r = Mri::new(
        &mut g,
        &mut out,
        &mut err,
        &[1, 1, 1, nframes],
        MRI_UCHAR,
        voxels,
    );
    let status = match r {
        Ok(_) => 0,
        Err(f) => f.status(),
    };
    out.extend_from_slice(&err);
    (g, status, String::from_utf8_lossy(&out).into_owned())
}

#[test]
fn the_limit_is_16_gib_of_frame_records() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut g = Globals::new(mode);
        assert!(!frames_calloc_fails(&mut g, 13_338_407));
        assert!(g.notices.is_empty());
        assert_eq!(g.errno, 0);
        assert!(frames_calloc_fails(&mut g, 13_338_408));
        assert!(g.notices.contains(&FS_760));
        assert_eq!(g.errno, 12);
        let mut g = Globals::new(mode);
        assert!(frames_calloc_fails(&mut g, -1));
        assert!(frames_calloc_fails(&mut g, i32::MIN));
        assert!(
            g.notices.is_empty(),
            "a negative count is calloc's own failure"
        );
        assert!(!frames_calloc_fails(&mut g, 0));
    }
}

#[test]
fn an_absurd_frame_count_is_refused_before_any_frame_is_made() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        for voxels in [false, true] {
            let t = std::time::Instant::now();
            let (g, status, text) = alloc(mode, 449_657_401, voxels);
            assert_eq!(status, 254, "{text}");
            assert!(
                text.contains("Cannot allocate memory")
                    && text.contains("MRIalloc: could not allocate 449657401 frame"),
                "{text}"
            );
            assert!(g.notices.contains(&FS_760));
            assert!(t.elapsed().as_secs() < 5, "refused at once");
            let (_, status, text) = alloc(mode, 13_338_408, voxels);
            assert_eq!(status, 254, "{text}");
            assert!(text.contains("could not allocate 13338408 frame"), "{text}");
        }
    }
}
