// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The connected segments (`mrisegment.cpp`): the segments are the 6-connected components, and
//! the defects FS-577, FS-578, FS-579, FS-581, FS-582.

use std::collections::VecDeque;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_filter::segment as sg;
use larmorx_freesurfer_mri::mri::{MRI_UCHAR, Mri, mri_alloc};

const MODES: [UpstreamDefects; 2] = [UpstreamDefects::Fix, UpstreamDefects::Reproduce];

fn noted(g: &Globals, id: &str) -> bool {
    g.notices.as_slice().iter().any(|n| n.defect.id == id)
}

fn vol(g: &mut Globals, w: i32, h: i32, d: i32, mut on: impl FnMut(i32, i32, i32) -> bool) -> Mri {
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let mut m = mri_alloc(g, &mut o, &mut e, w, h, d, MRI_UCHAR).unwrap();
    let c = m.chunk.as_mut().unwrap();
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                c[((z * h + y) * w + x) as usize] = u8::from(on(x, y, z));
            }
        }
    }
    m
}

/// The sizes of the 6-connected components of the voxels set, largest first.
fn components(m: &Mri) -> Vec<usize> {
    let (w, h, d) = (m.vg.width, m.vg.height, m.vg.depth);
    let c = m.chunk.as_ref().unwrap();
    let idx = |x: i32, y: i32, z: i32| ((z * h + y) * w + x) as usize;
    let mut seen = vec![false; c.len()];
    let mut sizes = Vec::new();
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                if c[idx(x, y, z)] == 0 || seen[idx(x, y, z)] {
                    continue;
                }
                let mut q = VecDeque::from([(x, y, z)]);
                seen[idx(x, y, z)] = true;
                let mut n = 0;
                while let Some((a, b, cc)) = q.pop_front() {
                    n += 1;
                    for (dx, dy, dz) in [
                        (1, 0, 0),
                        (-1, 0, 0),
                        (0, 1, 0),
                        (0, -1, 0),
                        (0, 0, 1),
                        (0, 0, -1),
                    ] {
                        let (p, r, s) = (a + dx, b + dy, cc + dz);
                        if (0..w).contains(&p)
                            && (0..h).contains(&r)
                            && (0..d).contains(&s)
                            && c[idx(p, r, s)] != 0
                            && !seen[idx(p, r, s)]
                        {
                            seen[idx(p, r, s)] = true;
                            q.push_back((p, r, s));
                        }
                    }
                }
                sizes.push(n);
            }
        }
    }
    sizes.sort_unstable_by(|a, b| b.cmp(a));
    sizes
}

fn sizes(s: &sg::MriSegmentation) -> Vec<usize> {
    let mut v: Vec<usize> = s.segments.iter().map(|m| m.voxels.len()).collect();
    v.sort_unstable_by(|a, b| b.cmp(a));
    v
}

/// MRIsegment's segments are the 6-connected components, with their areas, centroids and bounds.
#[test]
fn segments_are_the_components() {
    let mut g = Globals::new(UpstreamDefects::Fix);
    // a seeded mix of runs (plain arithmetic)
    let mut r: u32 = 12345;
    let m = vol(&mut g, 9, 8, 7, |_, _, _| {
        r = r.wrapping_mul(1_103_515_245).wrapping_add(12345);
        (r >> 16) % 3 == 0
    });
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let s = sg::mri_segment(&mut g, &mut o, &mut e, &m, 1.0, 1.0).unwrap();
    assert_eq!(sizes(&s), components(&m));
    assert_eq!(s.nsegments, s.max_segments());
    for seg in &s.segments {
        assert_eq!(seg.area, seg.voxels.len() as f32);
        assert!(seg.x0 <= seg.x1 && seg.cx >= f32::from(seg.x0) && seg.cx <= f32::from(seg.x1));
    }
    assert!(o.is_empty() && e.is_empty());
}

/// FS-577: a checkerboard of 64 x 64 x 17 under a slab: more than 32,767 single-voxel segments,
/// then one slab that touches half of the top layer. By default the slab joins every voxel it
/// touches (the components); FreeSurfer's 16-bit map loses the segments above 32,767.
#[test]
fn fs577_segment_numbers_above_32767() {
    for mode in MODES {
        let mut g = Globals::new(mode);
        let m = vol(&mut g, 64, 64, 18, |x, y, z| {
            z == 17 || (x + y + z) % 2 == 0
        });
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let s = sg::mri_segment(&mut g, &mut o, &mut e, &m, 1.0, 1.0).unwrap();
        assert!(noted(&g, "FS-577"));
        let true_sizes = components(&m);
        if mode == UpstreamDefects::Fix {
            assert_eq!(sizes(&s), true_sizes);
        } else {
            assert!(sizes(&s)[0] < true_sizes[0]);
        }
    }
}

/// FS-578: a pillar beside a checkerboard; MRImaxsegment removes the single voxels mid-scan and
/// moves the pillar's segment to the front. By default the pillar stays one segment.
#[test]
fn fs578_removal_during_the_scan() {
    for mode in MODES {
        let mut g = Globals::new(mode);
        let m = vol(&mut g, 64, 64, 6, |x, y, z| x >= 56 || (x + y + z) % 2 == 0);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let s = sg::mri_max_segment(&mut g, &mut o, &mut e, &m, 1.0, 1.0, &mut |_| 1000).unwrap();
        assert!(noted(&g, "FS-578"));
        let text = String::from_utf8(o).unwrap();
        assert!(text.contains("removing small segments"));
        let pillar = components(&m)[0];
        if mode == UpstreamDefects::Fix {
            assert_eq!(sizes(&s)[0], pillar);
        } else {
            assert!(sizes(&s)[0] < pillar);
        }
    }
}

/// FS-579, FS-581, FS-582: refused in both modes.
#[test]
fn fs579_581_582_refused() {
    for mode in MODES {
        let mut g = Globals::new(mode);
        let mut m = vol(&mut g, 4, 3, 2, |x, _, _| x < 2);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let s = sg::mri_segment(&mut g, &mut o, &mut e, &m, 1.0, 1.0).unwrap();
        let r = sg::mri_set_segment_value(&mut g, &mut e, &mut m, &s, 7, 3.0).unwrap();
        assert_ne!(r, 0);
        assert!(noted(&g, "FS-579"));

        let mut g = Globals::new(mode);
        let wide = vol(&mut g, 32769, 1, 1, |x, _, _| x % 2 == 0);
        assert!(sg::mri_segment(&mut g, &mut o, &mut e, &wide, 1.0, 1.0).is_err());
        assert!(noted(&g, "FS-581"));

        let mut g = Globals::new(mode);
        let long = vol(&mut g, 600, 1, 1, |x, _, _| x == 599);
        let mut s = sg::mri_segment(&mut g, &mut o, &mut e, &long, 1.0, 1.0).unwrap();
        let small = vol(&mut g, 2, 1, 1, |_, _, _| true);
        assert!(sg::mri_segment_dilate(&mut g, &mut o, &mut e, &mut s, &small).is_err());
        assert!(noted(&g, "FS-582"));
    }
}

/// `csf_labels`: CSF, Unknown, the lateral and inferior lateral ventricles, the third and fourth.
#[test]
fn csf_labels() {
    assert_eq!(sg::CSF_LABELS, [24, 0, 4, 43, 5, 44, 14, 15]);
}
