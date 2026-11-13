// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The corrections of queue item 3.1-51's leftovers, second group (relay 24, FS-535 to FS-538):
//! what the default gives against what `reproduce` gives (the probe's `mpg` compares `reproduce`
//! with FreeSurfer's own build; these tests show the defaults).

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::alloc;
use larmorx_freesurfer_surf::coords::Io;
use larmorx_freesurfer_surf::mris::Mris;
use larmorx_freesurfer_surf::props10;

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
            env: Environment::from_pairs(Vec::<(String, String)>::new()),
            out: Vec::new(),
            err: Vec::new(),
        }
    }

    /// A surface of the given faces, each vertex's neighbours and faces in order.
    fn surface(&mut self, coords: &[[f32; 3]], faces: &[[i32; 3]]) -> Mris {
        let (nv, nf) = (coords.len(), faces.len());
        let mut m = alloc::alloc(
            &mut self.g,
            &mut self.out,
            &mut self.err,
            &self.sg,
            nv as i32,
            nf as i32,
        )
        .expect("a surface");
        for (f, v) in faces.iter().enumerate() {
            m.face_mut(f).v = *v;
        }
        for (vno, p) in coords.iter().enumerate() {
            let v = m.vertex_mut(vno);
            (v.x, v.y, v.z) = (p[0], p[1], p[2]);
            let mut nb: Vec<i32> = Vec::new();
            let mut fs: Vec<i32> = Vec::new();
            for (f, fv) in faces.iter().enumerate() {
                if fv.contains(&(vno as i32)) {
                    fs.push(f as i32);
                    for &x in fv {
                        if x != vno as i32 && !nb.contains(&x) {
                            nb.push(x);
                        }
                    }
                }
            }
            let t = m.topology_mut(vno);
            t.vnum = nb.len() as i16;
            t.vtotal = nb.len() as i16;
            t.v = Some(nb);
            t.num = fs.len() as u8;
            t.f = Some(fs);
        }
        m
    }

    fn io(&mut self) -> Io<'_> {
        Io {
            out: &mut self.out,
            err: &mut self.err,
            env: &self.env,
        }
    }
}

const TETRA: [[f32; 3]; 4] = [
    [1.0, 1.0, 1.0],
    [1.0, -1.0, -1.0],
    [-1.0, 1.0, -1.0],
    [-1.0, -1.0, 1.0],
];
const TETRA_FACES: [[i32; 3]; 4] = [[0, 1, 2], [0, 3, 1], [0, 2, 3], [1, 3, 2]];

/// FS-535: on an open surface (a triangle: every edge has one face) the edge table stops; the
/// corner table then crashes FreeSurfer (`reproduce`: the crash's status) and is refused by
/// default (no corners, status 1).
#[test]
fn fs535_corners_after_a_stopped_edge_table() {
    for mode in [UpstreamDefects::Reproduce, UpstreamDefects::Fix] {
        let mut r = Run::new(mode);
        let mut s = r.surface(&TETRA[..3], &TETRA_FACES[..1]);
        let mut io = Io {
            out: &mut r.out,
            err: &mut r.err,
            env: &r.env,
        };
        assert_eq!(props10::mris_edges(&mut io, &mut s), 1);
        let got = props10::mris_corners(&mut r.g, &mut io, &mut s);
        let text = String::from_utf8_lossy(&r.out).into_owned();
        assert!(text.starts_with("ERROR: MRISedges(): not enough faces 0 1 k=1\n"), "{text}");
        if mode.reproduce() {
            assert!(matches!(got, Err(Fatal::Exit(139))));
        } else {
            assert!(matches!(got, Ok(1)));
            assert!(s.corners.is_none() && s.ncorners == 0);
        }
        assert!(r.g.notices.as_slice().iter().any(|n| n.defect.id == "FS-535"));
    }
}

/// The closed tetrahedron's tables: 6 edges, 12 corners, every vertex with three edge numbers.
#[test]
fn tetrahedron_tables() {
    let mut r = Run::new(UpstreamDefects::Fix);
    let mut s = r.surface(&TETRA, &TETRA_FACES);
    assert_eq!(props10::mris_count_edges(&s), 6);
    let mut io = r.io();
    assert_eq!(props10::mris_edges(&mut io, &mut s), 0);
    let mut g = Globals::new(UpstreamDefects::Fix);
    assert!(matches!(props10::mris_corners(&mut g, &mut io, &mut s), Ok(0)));
    assert_eq!(s.ncorners, 12);
    for vno in 0..4 {
        assert_eq!(s.topology(vno).e.as_ref().map(Vec::len), Some(3));
    }
    for c in s.corners.as_ref().unwrap() {
        assert!(c.edgedir.iter().all(|d| d.abs() == 1));
    }
}

/// FS-538: the edge measurement without the faces' normals: FreeSurfer prints its two error lines
/// and crashes (`reproduce`); the default computes the missing normals first (the regular
/// tetrahedron's hinges: dot -1/3, angle acos(-1/3) in degrees).
#[test]
fn fs538_edge_metric_without_normals() {
    for mode in [UpstreamDefects::Reproduce, UpstreamDefects::Fix] {
        let mut r = Run::new(mode);
        let mut s = r.surface(&TETRA, &TETRA_FACES);
        let sg = SurfGlobals::default();
        let mut io = Io {
            out: &mut r.out,
            err: &mut r.err,
            env: &r.env,
        };
        props10::mris_edges(&mut io, &mut s);
        let got = props10::mris_edge_metric(&mut r.g, &sg, &mut io, &mut s);
        if mode.reproduce() {
            assert!(matches!(got, Err(Fatal::Exit(139))));
            let text = String::from_utf8_lossy(&r.out).into_owned();
            assert!(text.contains("ERROR: MRISedgeMetricEdge(): edgeno=0 DoGrad=0\n"), "{text}");
        } else {
            assert!(matches!(got, Ok(0)));
            for e in s.edges.as_ref().unwrap() {
                assert!((e.dot + 1.0 / 3.0).abs() < 1e-12, "{}", e.dot);
                assert!((e.angle - 109.471_220_634_490_7).abs() < 1e-9, "{}", e.angle);
            }
        }
        assert!(r.g.notices.as_slice().iter().any(|n| n.defect.id == "FS-538"));
    }
}

/// FS-537's mechanism: the corner measurement reads the edges' unit vectors as they are: on a
/// fresh edge table (no edge measurement yet) every corner reads 90 degrees in both modes; the
/// default's correction is in the corner statistics (the probe's `mpg` cases note it).
#[test]
fn fs537_corners_read_the_edges_unit_vectors() {
    let mut r = Run::new(UpstreamDefects::Fix);
    let mut s = r.surface(&TETRA, &TETRA_FACES);
    let mut io = r.io();
    let mut g = Globals::new(UpstreamDefects::Fix);
    props10::mris_corner_metric(&mut g, &mut io, &mut s).unwrap();
    assert!(s.corners.as_ref().unwrap().iter().all(|c| c.angle == 90.0));
    let sg = SurfGlobals::default();
    props10::mris_face_metric(&sg, &mut io, &mut s);
    props10::mris_edge_metric(&mut g, &sg, &mut io, &mut s).unwrap();
    props10::mris_corner_metric(&mut g, &mut io, &mut s).unwrap();
    for c in s.corners.as_ref().unwrap() {
        assert!((c.angle - 60.0).abs() < 1e-9, "{}", c.angle);
    }
}

/// glibc's default NaN for an arc cosine beyond 1 (a corner's dot product just above 1): the
/// corner angle's NaN has its sign set, as FreeSurfer's.
#[test]
fn corner_angle_beyond_one_is_glibcs_nan() {
    let mut r = Run::new(UpstreamDefects::Fix);
    let mut s = r.surface(&TETRA, &TETRA_FACES);
    let mut io = r.io();
    let mut g = Globals::new(UpstreamDefects::Fix);
    props10::mris_corners(&mut g, &mut io, &mut s).unwrap();
    let (e0, e1) = {
        let c = &s.corners.as_ref().unwrap()[0];
        (c.edgeno[0] as usize, c.edgeno[1] as usize)
    };
    let (d0, d1) = {
        let c = &s.corners.as_ref().unwrap()[0];
        (f64::from(c.edgedir[0]), f64::from(c.edgedir[1]))
    };
    let edges = s.edges.as_mut().unwrap();
    edges[e0].u = [d0, 0.0, 0.0];
    edges[e1].u = [d1 * (1.0 + f64::EPSILON), 0.0, 0.0];
    let dot = props10::mris_corner_metric_corner(&mut s, 0);
    assert!(dot > 1.0);
    let a = s.corners.as_ref().unwrap()[0].angle;
    assert_eq!(a.to_bits(), 0xfff8_0000_0000_0000);
}
