// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mrisurf_vals.cpp`'s marks, rip flags and filters (relay 40 of track D, `valmarks.rs`,
//! `valfilter.rs`): the corrections FS-917, FS-971, FS-972 and FS-973 and the official build's
//! arithmetic the probe's random cases rarely meet, on small surfaces read from binary triangle
//! files.

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_io::label::label_alloc;
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::coords::Io;
use larmorx_freesurfer_surf::mris::Mris;
use larmorx_freesurfer_surf::valfilter::{self, Field};
use larmorx_freesurfer_surf::{readwrite, valmarks};

struct Run {
    g: Globals,
    sg: SurfGlobals,
    env: Environment,
    out: Vec<u8>,
    err: Vec<u8>,
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

impl Run {
    fn new(mode: UpstreamDefects) -> Run {
        Run {
            g: Globals::new(mode),
            sg: SurfGlobals::default(),
            env: Environment::from_pairs(vec![("SURFER_SIDEDOOR".to_owned(), "1".to_owned())]),
            out: Vec::new(),
            err: Vec::new(),
        }
    }

    /// The surface of `v` and `f` written as a binary triangle file and read by `MRISread`.
    fn surface(&mut self, v: &[[f32; 3]], f: &[[i32; 3]]) -> Mris {
        let mut b = vec![0xff, 0xff, 0xfe];
        b.extend_from_slice(b"created by me on Sun Nov 15 06:10:00 2026\n\n");
        b.extend_from_slice(&(v.len() as i32).to_be_bytes());
        b.extend_from_slice(&(f.len() as i32).to_be_bytes());
        for p in v {
            for c in p {
                b.extend_from_slice(&c.to_be_bytes());
            }
        }
        for q in f {
            for x in q {
                b.extend_from_slice(&x.to_be_bytes());
            }
        }
        let td = tempfile::tempdir().expect("a folder");
        std::fs::write(td.path().join("lh.s0"), b).expect("the file");
        let mut io = io!(self);
        readwrite::read(
            &mut self.g,
            &mut self.sg,
            &mut io,
            Some(td.path()),
            "lh.s0",
            true,
        )
        .unwrap()
        .expect("a surface")
    }

    fn noted(&self, id: &str) -> bool {
        self.g.notices.as_slice().iter().any(|n| n.defect.id == id)
    }
}

/// A fan: vertex 0 at the centre of `n` triangles around it (open: a border), the rim at 1..=n.
fn fan(n: usize) -> (Vec<[f32; 3]>, Vec<[i32; 3]>) {
    let mut v = vec![[0.0f32, 0.0, 0.0]];
    for k in 0..=n {
        // the rim on a parabola: no float function differs between platforms
        let x = k as f32 - n as f32 / 2.0;
        v.push([x, 4.0 + x * x / 16.0, 0.0]);
    }
    let f = (1..=n as i32).map(|k| [0, k, k + 1]).collect();
    (v, f)
}

/// FS-917: a label point naming no vertex is skipped in both modes, with the notice; the others
/// are ripped.
#[test]
fn ripping_by_a_label_with_a_point_outside() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let (v, f) = fan(4);
        let mut s = r.surface(&v, &f);
        let mut lab = label_alloc(&mut r.g, &r.env, &mut r.err, 3, None, None).unwrap();
        lab.n_points = 3;
        lab.lv[0].vno = 2;
        lab.lv[1].vno = -1;
        lab.lv[2].vno = 99;
        valmarks::rip_label(&mut r.g, &mut s, &lab);
        assert!(r.noted("FS-917"));
        let rips: Vec<i8> = (0..6).map(|vno| s.vertex(vno).ripflag).collect();
        assert_eq!(rips, vec![0, 0, 1, 0, 0, 0], "{mode:?}");
        let mut out = Vec::new();
        valmarks::rip_not_label(&mut r.g, &mut out, &mut s, &lab);
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "MRISripNotLabel() ripped 5/6 vertices (1 unripped)\n"
        );
        assert!((0..6).all(|vno| s.vertex(vno).ripflag == 1));
        assert_eq!(s.vertex(2).marked, 0);
    }
}

/// A vertex without neighbours keeps its value undivided (GCC skips the division by 1): a
/// signalling NaN stays signalling; with neighbours the division quiets it.
#[test]
fn an_average_without_neighbours_is_not_divided() {
    let mut r = Run::new(UpstreamDefects::Reproduce);
    let (v, f) = fan(2);
    let mut s = r.surface(&v, &f);
    let snan = f32::from_bits(0x7fa0_0001);
    // vertex 0 alone: its neighbours ripped do not make vnum 0, so take one out of the topology
    s.topology_mut(3).vnum = 0;
    s.vertex_mut(3).val = snan;
    s.vertex_mut(0).val = snan;
    valfilter::average_vals(&mut s, 1);
    assert_eq!(s.vertex(3).val.to_bits(), 0x7fa0_0001);
    assert_eq!(s.vertex(0).val.to_bits(), 0x7fe0_0001);
}

/// The medians' middle pair: the lower one is the first operand (its NaN passed on).
#[test]
fn a_median_of_an_even_count() {
    let mut r = Run::new(UpstreamDefects::Reproduce);
    let (v, f) = fan(2);
    let mut s = r.surface(&v, &f);
    // vertex 1: itself and vertices 0 and 2: odd; vertex 0: itself and 1, 2, 3: even
    for (vno, x) in [(0, 4.0f32), (1, 1.0), (2, 2.0), (3, 8.0)] {
        s.vertex_mut(vno).val = x;
    }
    valfilter::median_filter(&mut r.g, &mut s, 1, Field::Val, false);
    assert_eq!(s.vertex(0).val, 3.0);
    let (p, q) = (f32::from_bits(0x7fc0_0001), f32::from_bits(0xffc0_0002));
    for (vno, x) in [(0, p), (1, q), (2, 2.0), (3, 8.0)] {
        s.vertex_mut(vno).val = x;
    }
    valfilter::median_filter(&mut r.g, &mut s, 1, Field::Val, false);
    // NaNs compare equal to everything: the merge sort leaves 0's list as p, q, 2, 8
    assert_eq!(s.vertex(0).val.to_bits(), 0xffc0_0002);
}

/// FS-972: a value that rounds above the table's last entry: `reproduce` drops it (FreeSurfer
/// counts it outside its table), the default counts it.
#[test]
fn a_mode_past_the_table() {
    for (mode, want) in [
        (UpstreamDefects::Reproduce, 0.0f32),
        (UpstreamDefects::Fix, 3.0),
    ] {
        let mut r = Run::new(mode);
        let (v, f) = fan(3);
        let mut s = r.surface(&v, &f);
        // the largest value 2.6 truncates to 2; vertex 0's neighbours 1..=4: three of 2.6 (3), a 0
        for (vno, x) in [(0, 0.0f32), (1, 2.6), (2, 2.6), (3, 2.6), (4, 0.0)] {
            s.vertex_mut(vno).val = x;
        }
        let mut io = io!(r);
        valfilter::mode_filter_vals(&mut r.g, &mut io, &mut s, 1).unwrap();
        assert!(r.noted("FS-972"));
        // reproduce: histo[0] = 1, nothing at 1 or 2: 0; fix: 3 counted three times
        assert_eq!(s.vertex(0).val, want, "{mode:?}");
        assert!(String::from_utf8_lossy(&r.out).starts_with("iter 0: "));
    }
}

/// A table of a negative size: `calloc`'s failure, as `ErrorExit` reports it (status 254).
#[test]
fn a_mode_table_of_a_negative_size() {
    let mut r = Run::new(UpstreamDefects::Fix);
    let (v, f) = fan(2);
    let mut s = r.surface(&v, &f);
    s.vertex_mut(3).val = 1e10;
    let mut io = io!(r);
    let e = valfilter::mode_filter_vals(&mut r.g, &mut io, &mut s, 1).unwrap_err();
    assert_eq!(e.status(), 254);
    let err = String::from_utf8_lossy(&r.err).into_owned();
    assert!(err.contains("Cannot allocate memory"), "{err}");
    assert!(
        err.contains("MRISmodeFilterVals: could not allocate histo array of -2147483647 ints"),
        "{err}"
    );
}

/// FS-973: a NaN next to a lasting zero: FreeSurfer's filter never ends; here it stops at the
/// first pass that changes nothing, in both modes.
#[test]
fn the_zero_mode_filter_ends_with_a_nan() {
    for mode in [UpstreamDefects::Reproduce, UpstreamDefects::Fix] {
        let mut r = Run::new(mode);
        let (v, f) = fan(3);
        let mut s = r.surface(&v, &f);
        for vno in 0..5 {
            s.vertex_mut(vno).val = 0.0;
        }
        s.vertex_mut(0).val = f32::NAN;
        let mut io = io!(r);
        valfilter::mode_filter_zero_vals(&mut r.g, &mut io, &mut s).unwrap();
        let out = String::from_utf8_lossy(&r.out).into_owned();
        assert!(out.lines().count() < 10, "{mode:?}: {out}");
        if mode == UpstreamDefects::Reproduce {
            assert!(r.noted("FS-973"));
        } else {
            // nint(NaN) is 0 by default (FS-12): the NaN is processed as a zero and settles
            assert!(r.noted("FS-12"));
            assert!(!s.vertex(0).val.is_nan());
        }
    }
}

/// FS-971: a vertex with more than 1,023 unripped neighbours in use: every value sorted, noted
/// (a surface's reader refuses so many 1-hop neighbours: here 1,100 in use by hand, the fan's
/// 201 rim vertices over and over).
#[test]
fn a_median_of_more_than_the_list() {
    let mut r = Run::new(UpstreamDefects::Reproduce);
    let (v, f) = fan(200);
    let mut s = r.surface(&v, &f);
    for vno in 0..202 {
        s.vertex_mut(vno).d = vno as f32;
    }
    let vt = s.topology_mut(0);
    vt.v = Some((0..1100).map(|k| 1 + (k % 201)).collect());
    vt.vtotal = 1100;
    valfilter::median_filter_d(&mut r.g, &mut s, 1, 1);
    assert!(r.noted("FS-971"));
    // vertex 0 (0) and 1,100 values of 1..=201: 1,101 values, the middle one the 551st
    let mut all: Vec<f32> = std::iter::once(0.0)
        .chain((0..1100).map(|k| (1 + k % 201) as f32))
        .collect();
    all.sort_by(f32::total_cmp);
    assert_eq!(s.vertex(0).d, all[550]);
}

/// `detectContrast` without an unripped vertex: `0 / 0`, SSE's NaN, printed `-nan`.
#[test]
fn the_contrast_of_no_vertex() {
    let mut r = Run::new(UpstreamDefects::Reproduce);
    let (v, f) = fan(2);
    let mut s = r.surface(&v, &f);
    for vno in 0..4 {
        s.vertex_mut(vno).ripflag = 1;
    }
    let mut err = Vec::new();
    assert_eq!(valmarks::detect_contrast(&mut err, &s), -1);
    assert_eq!(
        String::from_utf8(err).unwrap(),
        "Detecting contrast direction: inverted [ WM (-nan) < GM (-nan) ]\n"
    );
}
