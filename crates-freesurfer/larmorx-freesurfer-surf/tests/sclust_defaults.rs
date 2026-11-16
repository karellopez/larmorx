// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The surface clusters' corrections (relay 38 of track D, `sclust.rs` and `labelsurf.rs`'s
//! island and hole removal, FS-901 to FS-907) where the probe's `scl` and `lbl` write `G` or
//! FreeSurfer crashes, and the growth deeper than FreeSurfer's stack.

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_io::label::label_alloc;
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::coords::Io;
use larmorx_freesurfer_surf::mris::Mris;
use larmorx_freesurfer_surf::sclust::{self, SurfClusterSum};
use larmorx_freesurfer_surf::{alloc, labelsurf};

struct Run {
    g: Globals,
    sg: SurfGlobals,
    env: Environment,
    out: Vec<u8>,
    err: Vec<u8>,
}

impl Run {
    fn new(mode: UpstreamDefects) -> Run {
        Run {
            g: Globals::new(mode),
            sg: SurfGlobals::default(),
            env: Environment::from_pairs(vec![("SUBJECTS_DIR".to_owned(), "/".to_owned())]),
            out: Vec::new(),
            err: Vec::new(),
        }
    }

    /// A chain of `n` vertices 1 mm apart along x, each the neighbour of the one before and
    /// after it (no faces): a growth from its first vertex goes `n` deep.
    fn chain(&mut self, n: i32) -> Mris {
        let mut m =
            alloc::alloc(&mut self.g, &mut self.out, &mut self.err, &self.sg, n, 0).unwrap();
        for vno in 0..n {
            let v = m.vertex_mut(vno as usize);
            v.x = vno as f32;
            v.val = 1.0;
            let t = m.topology_mut(vno as usize);
            let nb: Vec<i32> = [vno - 1, vno + 1]
                .into_iter()
                .filter(|&k| (0..n).contains(&k))
                .collect();
            t.vnum = nb.len() as _;
            t.v = Some(nb);
        }
        m
    }

    fn noted(&self, id: &str) -> bool {
        self.g.notices.as_slice().iter().any(|n| n.defect.id == id)
    }
}

macro_rules! io {
    ($r:expr) => {
        Io {
            out: &mut $r.out,
            err: &mut $r.err,
            env: &$r.env,
        }
    };
}

/// FS-901: a cluster whose depth-first path passes what FreeSurfer's 8 MiB stack holds (80 bytes
/// a level: 104,858 levels) is grown in full in both modes and noted; a shallower one is not.
#[test]
fn fs901_a_deep_growth_is_grown_in_full() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let mut s = r.chain(110_000);
        let (l, n) =
            sclust::map_surf_clusters(&mut r.g, &mut io!(r), &mut s, 0.5, -1.0, 1, 0.0, None, None)
                .unwrap();
        assert_eq!(n, 1);
        assert_eq!(l.unwrap()[0].nmembers, 110_000);
        assert!((0..110_000).all(|v| s.vertex(v).undefval == 1));
        assert!(r.noted("FS-901"), "{mode:?}");
        let mut r = Run::new(mode);
        let mut s = r.chain(1_000);
        let (st, depth) =
            sclust::grow_surf_cluster_depth(&mut r.g, &mut io!(r), 1, 0, &mut s, 0.5, -1.0, 1);
        assert_eq!((st, depth), (0, 1_000));
        assert!(!r.noted("FS-901"));
    }
}

/// FS-902: removing the islands of a label without points: an empty label (one place) by
/// default, FreeSurfer's crash (status 139) with `reproduce`.
#[test]
fn fs902_the_islands_of_an_empty_label() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let mut s = r.chain(4);
        let a = label_alloc(&mut r.g, &r.env, &mut r.err, 2, None, None).unwrap();
        let res = labelsurf::label_remove_islands_surf(&mut r.g, &mut io!(r), &mut s, &a);
        if mode.reproduce() {
            assert!(matches!(res, Err(Fatal::Exit(139))));
        } else {
            let l = res.unwrap();
            assert_eq!((l.n_points, l.max_points), (0, 1));
        }
        assert!(r.noted("FS-902"));
    }
}

/// The islands of a label in two pieces: the larger piece, its vertices' `stat`.
#[test]
fn the_largest_piece_of_a_label() {
    let mut r = Run::new(UpstreamDefects::Fix);
    let mut s = r.chain(8);
    s.vertex_mut(5).stat = 2.5;
    let mut a = label_alloc(&mut r.g, &r.env, &mut r.err, 5, None, None).unwrap();
    for (k, v) in [0, 1, 4, 5, 6].into_iter().enumerate() {
        a.lv[k].vno = v;
    }
    a.n_points = 5;
    let l = labelsurf::label_remove_islands_surf(&mut r.g, &mut io!(r), &mut s, &a).unwrap();
    assert_eq!((l.n_points, l.max_points), (3, 4));
    assert_eq!(
        (l.lv[0].vno, l.lv[1].vno, l.lv[2].vno, l.lv[3].vno),
        (4, 5, 6, -1)
    );
    assert_eq!(l.lv[1].stat, 2.5);
    assert!(r.g.notices.is_empty());
}

/// FS-903: a vertex whose cluster number is negative is left out of the summary in both modes
/// (FreeSurfer writes before its list).
#[test]
fn fs903_a_negative_cluster_number() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let mut s = r.chain(3);
        (s.vertex_mut(0).undefval, s.vertex_mut(1).undefval) = (1, -1);
        let (l, n) = sclust::surf_cluster_summary(&mut r.g, &mut io!(r), &s, None, None).unwrap();
        assert_eq!(n, 1);
        assert_eq!(l.unwrap()[0].nmembers, 1);
        assert!(r.noted("FS-903"));
    }
}

/// FS-904: more than 10,000 clusters are renumbered (FreeSurfer's table holds 10,000), noted.
#[test]
fn fs904_more_than_ten_thousand_clusters() {
    let mut r = Run::new(UpstreamDefects::Fix);
    let mut s = r.chain(10_002);
    let list: Vec<SurfClusterSum> = (0..10_002)
        .map(|k| SurfClusterSum {
            clusterno: k + 1,
            pval_clusterwise: if k == 0 { 1.0 } else { 0.01 },
            ..Default::default()
        })
        .collect();
    for v in 0..10_002 {
        s.vertex_mut(v).undefval = v as i32 + 1;
    }
    let (l, n) = sclust::prune_by_cw_pval(&mut r.g, &list, 10_002, 0.05, &mut s);
    assert_eq!((n, l.len()), (10_001, 10_001));
    assert_eq!(s.vertex(0).undefval, 0);
    assert_eq!(s.vertex(10_001).undefval, 10_001);
    assert!(r.noted("FS-904"));
}

/// FS-905: a cluster whose p-value is NaN: its vertices removed by default; with `reproduce` they
/// keep their value and get 0 (FreeSurfer's number is memory never written).
#[test]
fn fs905_a_nan_clusterwise_p_value() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let mut s = r.chain(2);
        (s.vertex_mut(0).undefval, s.vertex_mut(1).undefval) = (1, 2);
        let list = vec![
            SurfClusterSum {
                clusterno: 1,
                pval_clusterwise: f64::NAN,
                ..Default::default()
            },
            SurfClusterSum {
                clusterno: 2,
                pval_clusterwise: 0.01,
                ..Default::default()
            },
        ];
        let (_, n) = sclust::prune_by_cw_pval(&mut r.g, &list, 2, 0.05, &mut s);
        assert_eq!(n, 1);
        assert_eq!((s.vertex(0).undefval, s.vertex(1).undefval), (0, 1));
        let expect = if mode.reproduce() { 1.0 } else { 0.0 };
        assert_eq!(s.vertex(0).val, expect, "{mode:?}");
        assert!(r.noted("FS-905"));
    }
}

/// FS-906: a point set that cannot be created: an error and 1 by default, FreeSurfer's crash
/// (status 139) with `reproduce`.
#[test]
fn fs906_a_point_set_that_cannot_be_created() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let s = r.chain(2);
        let td = tempfile::tempdir().unwrap();
        let res = sclust::save_as_point_set(
            &mut r.g,
            &mut io!(r),
            Some(td.path()),
            "nodir/p.txt",
            &[],
            0,
            &s,
        );
        if mode.reproduce() {
            assert!(matches!(res, Err(Fatal::Exit(139))));
        } else {
            assert_eq!(res.unwrap(), 1);
            assert!(String::from_utf8_lossy(&r.out).contains("could not open nodir/p.txt"));
        }
        assert_eq!(r.g.errno, 2);
        assert!(r.noted("FS-906"));
    }
}

/// FS-907: a growth by distance continued on a surface without the earlier seed's vertex: -1 in
/// both modes (FreeSurfer reads through a pointer into the earlier surface).
#[test]
fn fs907_a_continuation_without_the_seed() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let mut s = r.chain(6);
        let mut l = vec![-2; 6];
        let n = sclust::grow_by_dist(
            &mut r.g,
            &mut r.sg,
            &mut io!(r),
            &mut s,
            5,
            1.5,
            0,
            -1,
            Some(&mut l),
        );
        assert_eq!(n, 2);
        assert_eq!(&l[..2], &[5, 4]);
        let mut small = r.chain(3);
        let mut l = vec![-2; 3];
        let n = sclust::grow_by_dist(
            &mut r.g,
            &mut r.sg,
            &mut io!(r),
            &mut small,
            0,
            1.5,
            0,
            1,
            Some(&mut l),
        );
        assert_eq!(n, -1);
        assert!(r.noted("FS-907"));
    }
}
