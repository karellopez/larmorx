// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The projections of `mrisurf_project.cpp` (relay 43 of track D, `project.rs`) on an icosahedron
//! read from a binary triangle file: the sphere, the cylinder and the two ellipsoids put the
//! vertices where they belong, the status follows, and the orthographic ellipsoid's failure goes on
//! radially with glibc's `EDOM` in its message.

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_surf::aaa::{MrisStatus, SurfGlobals};
use larmorx_freesurfer_surf::coords::Io;
use larmorx_freesurfer_surf::mris::Mris;
use larmorx_freesurfer_surf::{project, readwrite};

struct Run {
    g: Globals,
    sg: SurfGlobals,
    env: Environment,
    out: Vec<u8>,
    err: Vec<u8>,
}

impl Run {
    fn new() -> Run {
        Run {
            g: Globals::new(UpstreamDefects::Fix),
            sg: SurfGlobals::default(),
            env: Environment::from_pairs(vec![("SURFER_SIDEDOOR".to_owned(), "1".to_owned())]),
            out: Vec::new(),
            err: Vec::new(),
        }
    }

    /// An octahedron of radius `r` centred at `o`, written as a binary triangle file and read.
    fn octahedron(&mut self, r: f32, o: [f32; 3]) -> Mris {
        let v: Vec<[f32; 3]> = [
            [r, 0.0, 0.0],
            [-r, 0.0, 0.0],
            [0.0, r, 0.0],
            [0.0, -r, 0.0],
            [0.0, 0.0, r],
            [0.0, 0.0, -r],
        ]
        .iter()
        // turned by 30 degrees about z (no vertex on the y axis), then moved
        .map(|p| {
            let (c, s) = (0.866_025_4f32, 0.5f32);
            [
                p[0] * c - p[1] * s + o[0],
                p[0] * s + p[1] * c + o[1],
                p[2] + o[2],
            ]
        })
        .collect();
        let f: [[i32; 3]; 8] = [
            [0, 2, 4],
            [2, 1, 4],
            [1, 3, 4],
            [3, 0, 4],
            [2, 0, 5],
            [1, 2, 5],
            [3, 1, 5],
            [0, 3, 5],
        ];
        let mut b = vec![0xff, 0xff, 0xfe];
        b.extend_from_slice(b"created by me on Tue Nov 17 19:00:00 2026\n\n");
        b.extend_from_slice(&(v.len() as i32).to_be_bytes());
        b.extend_from_slice(&(f.len() as i32).to_be_bytes());
        for p in &v {
            for c in p {
                b.extend_from_slice(&c.to_be_bytes());
            }
        }
        for q in &f {
            for x in q {
                b.extend_from_slice(&x.to_be_bytes());
            }
        }
        let td = tempfile::tempdir().expect("a folder");
        std::fs::write(td.path().join("lh.s0"), b).expect("the file");
        let mut io = Io {
            out: &mut self.out,
            err: &mut self.err,
            env: &self.env,
        };
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
}

fn norm(v: [f32; 3]) -> f64 {
    v.iter()
        .map(|&c| f64::from(c) * f64::from(c))
        .sum::<f64>()
        .sqrt()
}

/// The sphere: every vertex at the radius after centring (100 for a radius of 0), the status a
/// sphere, the point projection at 100 mm.
#[test]
fn onto_a_sphere() {
    let mut r = Run::new();
    let mut s = r.octahedron(7.0, [3.0, -2.0, 1.0]);
    let mut io = Io {
        out: &mut r.out,
        err: &mut r.err,
        env: &r.env,
    };
    assert_eq!(
        project::project_onto_sphere(&mut r.g, &mut io, &mut s, 0.0),
        0
    );
    assert_eq!(s.status, MrisStatus::SPHERE);
    assert_eq!(s.radius, 100.0);
    for v in 0..6 {
        let p = s.vertex(v);
        assert!((norm([p.x, p.y, p.z]) - 100.0).abs() < 1e-3, "{v}");
    }
    let (x, y, z) = project::spherical_project_xyz(3.0, 4.0, 12.0);
    assert!((norm([x, y, z]) - 100.0).abs() < 1e-4);
    assert!(project::project_onto_sphere_src_dst(&mut r.g, &mut io, &mut s, false, 1.0).is_err());
}

/// The cylinder (radius 5 about the y axis) and the two ellipsoids (axes 30, 20, 10): the moved
/// vertices on them; the copy's source unchanged.
#[test]
fn onto_a_cylinder_and_ellipsoids() {
    let mut r = Run::new();
    let mut s = r.octahedron(7.0, [0.0, 0.0, 0.0]);
    let mut io = Io {
        out: &mut r.out,
        err: &mut r.err,
        env: &r.env,
    };
    project::project_onto_cylinder(&mut r.g, &mut r.sg, &mut io, None, &mut s, 5.0).unwrap();
    for v in 0..6 {
        let p = s.vertex(v);
        assert!(
            (f64::from(p.x).hypot(f64::from(p.z)) - 5.0).abs() < 1e-5,
            "{v} {:?}",
            (0..6)
                .map(|v| (s.vertex(v).x, s.vertex(v).y, s.vertex(v).z))
                .collect::<Vec<_>>()
        );
    }
    let mut r = Run::new();
    let mut s = r.octahedron(7.0, [0.0, 0.0, 0.0]);
    let mut io = Io {
        out: &mut r.out,
        err: &mut r.err,
        env: &r.env,
    };
    let copy = project::project_onto_ellipsoid(
        &mut r.g, &mut r.sg, &mut io, &mut s, true, 30.0, 20.0, 10.0,
    )
    .unwrap()
    .expect("a copy");
    assert_eq!(copy.status, MrisStatus::ELLIPSOID);
    let on = |p: [f32; 3]| {
        (f64::from(p[0]) / 30.0).powi(2)
            + (f64::from(p[1]) / 20.0).powi(2)
            + (f64::from(p[2]) / 10.0).powi(2)
    };
    for v in 0..6 {
        let p = copy.vertex(v);
        assert!((on([p.x, p.y, p.z]) - 1.0).abs() < 1e-4, "{v}");
        // the source is unchanged
        assert!((norm([s.vertex(v).x, s.vertex(v).y, s.vertex(v).z]) - 7.0).abs() < 1e-5);
    }
    project::radial_project_onto_ellipsoid(
        &mut r.g, &mut r.sg, &mut io, &mut s, false, 30.0, 20.0, 10.0,
    )
    .unwrap();
    for v in 0..6 {
        let p = s.vertex(v);
        assert!((on([p.x, p.y, p.z]) - 1.0).abs() < 1e-4, "{v}");
    }
}

/// The orthographic ellipsoid fails where its root is not real (a point far outside a flat
/// ellipsoid): glibc's `EDOM` is printed before the message and the radial projection does the rest.
#[test]
fn an_ellipsoid_that_fails_goes_on_radially() {
    let mut r = Run::new();
    let mut s = r.octahedron(200.0, [0.0, 0.0, 0.0]);
    let mut io = Io {
        out: &mut r.out,
        err: &mut r.err,
        env: &r.env,
    };
    project::project_onto_ellipsoid(
        &mut r.g, &mut r.sg, &mut io, &mut s, false, 30.0, 100.0, 80.0,
    )
    .unwrap();
    let e = String::from_utf8_lossy(&r.err);
    assert!(
        e.contains("cannot be projected on ell"),
        "{e} {:?}",
        (0..6)
            .map(|v| (s.vertex(v).x, s.vertex(v).y, s.vertex(v).z))
            .collect::<Vec<_>>()
    );
    assert!(e.contains("Numerical argument out of domain"), "{e}");
    for v in 0..6 {
        let p = s.vertex(v);
        assert!(p.x.is_finite() && p.y.is_finite() && p.z.is_finite(), "{v}");
    }
}
