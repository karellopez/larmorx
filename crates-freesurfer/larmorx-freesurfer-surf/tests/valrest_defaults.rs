// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mrisurf_vals.cpp`'s functions of one program each (relay 41 of track D, `valrest.rs`): the
//! corrections FS-978 to FS-985 and the paths the probe's random cases leave to the official
//! build (its `G` cases) or rarely meet, on small surfaces read from binary triangle files.

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_io::annotation;
use larmorx_freesurfer_mri::ctab::{ColorTable, ColorTableEntry};
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, Mri, mri_alloc};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::coords::Io;
use larmorx_freesurfer_surf::mris::Mris;
use larmorx_freesurfer_surf::readwrite;
use larmorx_freesurfer_surf::valrest::{self, ValsVp};

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
        b.extend_from_slice(b"created by me on Tue Nov 17 09:30:00 2026\n\n");
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

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.out).into_owned()
    }

    /// A volume of one voxel per vertex, every voxel `x`.
    fn measure(&mut self, n: i32, x: f32) -> Mri {
        let mut m = mri_alloc(&mut self.g, &mut self.out, &mut self.err, n, 1, 1, MRI_FLOAT).unwrap();
        for c in 0..n {
            mri_set_vox_val(&mut self.g, &mut self.err, &mut m, c, 0, 0, 0, x, 0).unwrap();
        }
        m
    }
}

/// A fan: vertex 0 at the centre of `n` triangles around it (open: a border), the rim at 1..=n.
fn fan(n: usize) -> (Vec<[f32; 3]>, Vec<[i32; 3]>) {
    let mut v = vec![[0.0f32, 0.0, 0.0]];
    for k in 0..=n {
        let x = k as f32 - n as f32 / 2.0;
        v.push([x, 4.0 + x * x / 16.0, 0.0]);
    }
    let f = (1..=n as i32).map(|k| [0, k, k + 1]).collect();
    (v, f)
}

/// A strip of `len` squares, two vertices wide (`2 (len + 1)` vertices), each square two
/// triangles; optionally a lone triangle beside it (a part of the surface of its own).
fn strip(len: i32, lone: bool) -> (Vec<[f32; 3]>, Vec<[i32; 3]>) {
    let mut v = Vec::new();
    for i in 0..=len {
        v.push([i as f32, 0.0, 0.0]);
        v.push([i as f32, 1.0, 0.0]);
    }
    let mut f = Vec::new();
    for i in 0..len {
        let (a, b, c, d) = (2 * i, 2 * i + 1, 2 * i + 2, 2 * i + 3);
        f.push([a, c, b]);
        f.push([b, c, d]);
    }
    if lone {
        let n = v.len() as i32;
        v.push([0.0, 5.0, 0.0]);
        v.push([1.0, 5.0, 0.0]);
        v.push([0.0, 6.0, 0.0]);
        f.push([n, n + 1, n + 2]);
    }
    (v, f)
}

/// A colour table of the given colours (entries named `label<k>`; `None` an empty place).
fn ctab(colours: &[Option<(i32, i32, i32)>]) -> ColorTable {
    let mut ct = ColorTable {
        fname: b"user.ctab".to_vec(),
        version: 2,
        ..ColorTable::default()
    };
    for (k, c) in colours.iter().enumerate() {
        ct.entries.push(c.map(|(ri, gi, bi)| {
            Box::new(ColorTableEntry {
                name: format!("label{k}").into_bytes(),
                ri,
                gi,
                bi,
                ai: 0,
                rf: ri as f32 / 255.0,
                gf: gi as f32 / 255.0,
                bf: bi as f32 / 255.0,
                af: 0.0,
                tissue_type: 0,
                count: 0,
            })
        }));
    }
    ct
}

fn annot((r, g, b): (i32, i32, i32)) -> i32 {
    r | (g << 8) | (b << 16)
}

const A: (i32, i32, i32) = (10, 20, 30);
const B: (i32, i32, i32) = (40, 50, 60);
const C: (i32, i32, i32) = (70, 80, 90);

/// FS-978: the program's table, set before from another colour table that lacks B, ignores the
/// rim's annotation B, so FreeSurfer leaves the centre at A; larmorx indexes through the surface's
/// own colour table, so the centre takes its neighbours' B.
#[test]
fn mode_filter_through_the_surfaces_own_table() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let (v, f) = fan(5);
        let mut s = r.surface(&v, &f);
        let other = ctab(&[Some((0, 0, 0)), Some(A), Some(C)]);
        annotation::set_atable_from_ctable(&mut r.g, &mut r.out, &mut r.err, &mut r.sg.atable, Some(&other));
        s.ct = Some(ctab(&[Some((0, 0, 0)), Some(A), Some(B)]));
        s.vertex_mut(0).annotation = annot(A);
        for k in 1..s.nvertices as usize {
            s.vertex_mut(k).annotation = annot(B);
        }
        let mut io = io!(r);
        valrest::mode_filter_annotations(&mut r.g, &mut r.sg, &mut io, None, &mut s, 10).unwrap();
        let want = if mode.reproduce() { annot(A) } else { annot(B) };
        assert_eq!(s.vertex(0).annotation, want, "{mode:?}");
        assert_eq!(r.noted("FS-978"), !mode.reproduce());
        assert!(r.text().contains("filter iterations complete (10 requested, 0 changed)"), "{}", r.text());
    }
}

/// FS-978 and FS-979: no table in the program and no default table file: FreeSurfer sizes its
/// counts at one entry, then sets the table from the surface's colour table and counts past them;
/// its result keeps every known annotation and clears the others (`reproduce`); larmorx filters.
#[test]
fn mode_filter_without_a_table() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let (v, f) = fan(5);
        let mut s = r.surface(&v, &f);
        s.ct = Some(ctab(&[Some((0, 0, 0)), Some(A), Some(B)]));
        s.vertex_mut(0).annotation = annot(A);
        for k in 1..s.nvertices as usize {
            s.vertex_mut(k).annotation = annot(B);
        }
        s.vertex_mut(1).annotation = annot(C);
        let td = tempfile::tempdir().expect("a folder without a table file");
        let mut io = io!(r);
        valrest::mode_filter_annotations(&mut r.g, &mut r.sg, &mut io, Some(td.path()), &mut s, 1)
            .unwrap();
        if mode.reproduce() {
            assert!(r.noted("FS-979"));
            assert_eq!(s.vertex(0).annotation, annot(A));
            assert_eq!(s.vertex(1).annotation, 0, "C is not in the table");
            assert!(String::from_utf8_lossy(&r.err).contains("could not open translation file ./Simple_surface_labels2009.txt"));
        } else {
            assert!(r.noted("FS-978") && !r.noted("FS-979"));
            assert_eq!(s.vertex(0).annotation, annot(B));
        }
        assert!(r.text().contains("set_atable_from_ctable(): 3 entries"));
    }
}

/// A table index of 2^31 - 2 costs no memory (only the indices met are counted); one of 2^31 - 1
/// is FreeSurfer's failed allocation of `2^31` entries (`-2147483648` ints).
#[test]
fn mode_filter_large_indices() {
    for (index, fails) in [(2_147_483_646, false), (2_147_483_647, true)] {
        let mut r = Run::new(UpstreamDefects::Fix);
        let (v, f) = fan(5);
        let mut s = r.surface(&v, &f);
        let td = tempfile::tempdir().expect("a folder");
        let (ra, ga, ba) = A;
        let (rb, gb, bb) = B;
        std::fs::write(
            td.path().join("Simple_surface_labels2009.txt"),
            format!("1 a {ra} {ga} {ba} 0\n{index} b {rb} {gb} {bb} 0\n"),
        )
        .unwrap();
        s.vertex_mut(0).annotation = annot(A);
        for k in 1..s.nvertices as usize {
            s.vertex_mut(k).annotation = annot(B);
        }
        let mut io = io!(r);
        let res =
            valrest::mode_filter_annotations(&mut r.g, &mut r.sg, &mut io, Some(td.path()), &mut s, 2);
        if fails {
            assert!(res.is_err());
            assert!(
                String::from_utf8_lossy(&r.err)
                    .contains("MRISmodeFilterVals: could not allocate histo array of -2147483648 ints"),
                "{}",
                String::from_utf8_lossy(&r.err)
            );
        } else {
            res.unwrap();
            assert_eq!(s.vertex(0).annotation, annot(B));
        }
    }
}

/// FS-980 and FS-981: a field outside the values is skipped in both modes; a vertex without values
/// ends `reproduce` as FreeSurfer crashes and is skipped by default; the others are set.
#[test]
fn value_setters() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let (v, f) = fan(3);
        let mut s = r.surface(&v, &f);
        for k in 0..s.nvertices as usize {
            s.vertex_mut(k).curv = k as f32 + 0.5;
            let n = if k == 2 { 1 } else { 2 };
            valrest::set_vals_vp(
                s.vertex_mut(k),
                ValsVp {
                    vals: vec![-1.0; n],
                    orig_vals: vec![-2.0; n],
                    nvals: n as i32,
                },
            );
        }
        valrest::set_curvatures_to_values(&mut r.g, &mut s, 1).unwrap();
        assert!(r.noted("FS-980"));
        let vals = |s: &Mris, k: usize| valrest::vals_vp(s.vertex(k)).unwrap().lock().unwrap().clone();
        assert_eq!(vals(&s, 0).vals, vec![-1.0, 0.5]);
        assert_eq!(vals(&s, 2).vals, vec![-1.0], "vertex 2 holds one value: skipped");
        valrest::set_orig_values_to_curvatures(&mut r.g, &mut s, 0).unwrap();
        assert_eq!(s.vertex(3).curv, -2.0);
        s.vertex_mut(1).vp = Default::default();
        let res = valrest::set_values_to_curvatures(&mut r.g, &mut s, 0);
        assert!(r.noted("FS-981"));
        if mode.reproduce() {
            assert!(matches!(res, Err(Fatal::Exit(139))));
        } else {
            res.unwrap();
            assert_eq!(s.vertex(0).curv, -1.0);
            assert_eq!(s.vertex(1).curv, -2.0, "vertex 1 without values keeps its curvature");
        }
    }
}

/// FS-982: on a strip of 102 vertices with a measure of 1 per vertex, a target of 40 needs about 40
/// vertices; FreeSurfer's count over every pass reaches 102 first and reports the target as not
/// possible, larmorx finds the disk.
#[test]
fn dilation_counts_each_pass() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let (v, f) = strip(50, false);
        let mut s = r.surface(&v, &f);
        let n = s.nvertices;
        let m = r.measure(n, 1.0);
        let mut io = io!(r);
        let mask =
            valrest::dilate_vertex_to_sum(&mut r.g, &mut r.sg, &mut io, 0, Some(&mut s), Some(&m), 40.0)
                .unwrap();
        if mode.reproduce() {
            assert!(mask.is_none());
            assert!(r.text().contains("not possible to achieve target sum 40"), "{}", r.text());
        } else {
            let mask = mask.expect("a mask");
            let set = (0..n)
                .filter(|&c| mri_get_vox_val(&mut r.g, &mut r.err, &mask, c, 0, 0, 0, 0).unwrap() != 0.0)
                .count();
            assert!((40..=44).contains(&set), "{set} vertices");
            assert!(r.noted("FS-982"));
        }
    }
}

/// FS-982: the vertex's part of the surface (a lone triangle) cannot reach the target: larmorx
/// stops at the first pass that adds no vertex; FreeSurfer goes on until its running count reaches
/// the surface's vertex count; both report the target as not possible.
#[test]
fn dilation_of_a_part_used_up() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let (v, f) = strip(30, true);
        let mut s = r.surface(&v, &f);
        let n = s.nvertices;
        let m = r.measure(n, 1.0);
        let mut io = io!(r);
        let mask = valrest::dilate_vertex_to_sum(
            &mut r.g,
            &mut r.sg,
            &mut io,
            n - 1,
            Some(&mut s),
            Some(&m),
            10.0,
        )
        .unwrap();
        assert!(mask.is_none());
        let t = r.text();
        assert!(t.contains("not possible to achieve target sum 10"), "{t}");
        let hops = t.lines().filter(|l| l.starts_with("hop ")).count();
        if mode.reproduce() {
            assert!(hops > 10, "{hops} passes");
        } else {
            assert_eq!(hops, 2, "{t}");
            assert!(r.noted("FS-982"));
        }
    }
}

/// FS-983: a target no pass is needed for: FreeSurfer builds its mask from 0 hops and crashes
/// (`reproduce`, with FS-530); larmorx reports it.
#[test]
fn dilation_target_not_above_zero() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let (v, f) = fan(3);
        let mut s = r.surface(&v, &f);
        let n = s.nvertices;
        let m = r.measure(n, 1.0);
        let mut io = io!(r);
        let res =
            valrest::dilate_vertex_to_sum(&mut r.g, &mut r.sg, &mut io, 0, Some(&mut s), Some(&m), 0.0);
        assert!(r.noted("FS-983"));
        if mode.reproduce() {
            assert!(matches!(res, Err(Fatal::Exit(139))));
            assert!(r.noted("FS-530"));
        } else {
            assert!(res.unwrap().is_none());
            assert!(r.text().contains("ERROR: MRISdilateVertexToSum(): targetSum = 0, must be > 0\n"));
        }
    }
}

/// FS-984 and FS-985: without a colour table the index and the annotation FreeSurfer would read
/// from memory are taken as not found in both modes.
#[test]
fn colour_table_lookups_without_a_table() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let (v, f) = fan(3);
        let mut s = r.surface(&v, &f);
        valrest::rip_unknown(&mut r.g, &mut r.err, &mut s);
        assert!(r.noted("FS-984"));
        assert!((0..s.nvertices as usize).all(|k| s.vertex(k).ripflag == 1));
        assert!(String::from_utf8_lossy(&r.err).contains("CTABfindAnnotation: ct was NULL"));
        let mut io = io!(r);
        assert!(valrest::annotation_to_label(&mut r.g, &mut io, &s, 1).unwrap().is_none());
        assert!(r.noted("FS-985"));
        s.ct = Some(ctab(&[Some(A), None]));
        for k in 0..s.nvertices as usize {
            s.vertex_mut(k).annotation = annot(A);
            s.vertex_mut(k).ripflag = 0;
        }
        let mask = r.measure(s.nvertices, 1.0);
        let mut io = io!(r);
        let out = valrest::dilate_confined(&mut r.g, &mut io, &mut s, &mask, 0, 1, 1).unwrap();
        assert_eq!(out.vg.width, s.nvertices);
        assert_eq!(s.vertex(0).annotation, annot(A), "an empty place: no new annotation");
    }
}

/// The rip flags' dilation and erosion and the faces' and borders' bookkeeping.
#[test]
fn rips_and_faces() {
    let mut r = Run::new(UpstreamDefects::Fix);
    let (v, f) = fan(4);
    let mut s = r.surface(&v, &f);
    s.vertex_mut(0).ripflag = 1;
    valrest::dilate_ripped(&mut s, 1);
    assert!((0..s.nvertices as usize).all(|k| s.vertex(k).ripflag == 1));
    assert!((0..s.nfaces as usize).all(|k| s.face(k).ripflag == 1));
    assert!((0..s.nvertices as usize).all(|k| s.vertex(k).border == 1));
    s.vertex_mut(5).ripflag = 0;
    valrest::erode_ripped(&mut s, 1);
    // the centre and rim vertex 4 are next to vertex 5
    assert_eq!(s.vertex(0).ripflag, 0);
    assert_eq!(s.vertex(4).ripflag, 0);
    assert_eq!(s.vertex(2).ripflag, 1);
    assert_eq!(s.face(0).ripflag, 1, "face 0 (0 1 2) holds ripped vertices");
}

/// `MRISclearWhichAndVal2` for a field it does not know fails FreeSurfer's assertion.
#[test]
fn clear_which_unknown_field() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode);
        let (v, f) = fan(3);
        let mut s = r.surface(&v, &f);
        let mut io = io!(r);
        let res = valrest::clear_which_and_val2(&mut r.g, &mut io, &mut s, 5);
        assert!(res.is_err());
        assert!(r.text().contains("utils/mrisurf_vals.cpp:4137 false"));
    }
}
