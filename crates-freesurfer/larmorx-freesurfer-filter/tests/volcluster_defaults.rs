// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The volume clusters' defects (FS-570, FS-571) and the clusters' order.

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::matrix::Matrix;
use larmorx_freesurfer_filter::volcluster as vc;
use larmorx_freesurfer_mri::mri::{MRI_UCHAR, Mri, mri_alloc};

const MODES: [UpstreamDefects; 2] = [UpstreamDefects::Fix, UpstreamDefects::Reproduce];

fn noted(g: &Globals, id: &str) -> bool {
    g.notices.as_slice().iter().any(|n| n.defect.id == id)
}

/// A uchar row of 7 voxels: 1 1 0 1 0 1 1 (clusters of 2, 1 and 2 voxels).
fn row(g: &mut Globals) -> Mri {
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let mut m = mri_alloc(g, &mut o, &mut e, 7, 1, 1, MRI_UCHAR).unwrap();
    m.chunk.as_mut().unwrap()[..7].copy_from_slice(&[1, 1, 0, 1, 0, 1, 1]);
    m
}

/// The clusters sorted by size, ties in the order found (glibc's merge order), in both modes.
#[test]
fn clusters_sorted_by_size_ties_kept() {
    for mode in MODES {
        let mut g = Globals::new(mode);
        let v = row(&mut g);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let l = vc::clust_get_clusters(
            &mut g, &mut o, &mut e, &v, 0, 0.5, 0.0, 0, 0.0, None, None, false,
        )
        .unwrap();
        let firsts: Vec<i32> = l.iter().map(|c| c.col[0]).collect();
        assert_eq!(firsts, [0, 5, 3]);
    }
}

/// FS-570: pruning by distance without coordinates reads unset ones (0 here), noted.
#[test]
fn fs570_unset_coordinates() {
    for mode in MODES {
        let mut g = Globals::new(mode);
        let v = row(&mut g);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let l = vc::clust_get_clusters(
            &mut g, &mut o, &mut e, &v, 0, 0.5, 0.0, 0, 0.0, None, None, false,
        )
        .unwrap();
        let p = vc::clust_prune_by_distance(&mut g, &l, 1.0).unwrap();
        assert!(noted(&g, "FS-570"));
        assert_eq!(p.len(), 3);
    }
}

/// FS-571: a transform that is not 4 x 4: an error in both modes.
#[test]
fn fs571_bad_transform() {
    for mode in MODES {
        let mut g = Globals::new(mode);
        let v = row(&mut g);
        let x = Matrix::real(3, 3);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let r = vc::clust_get_clusters(
            &mut g,
            &mut o,
            &mut e,
            &v,
            0,
            0.5,
            0.0,
            0,
            0.0,
            None,
            Some(&x),
            false,
        );
        assert!(r.is_err());
        assert!(noted(&g, "FS-571"));
    }
}
