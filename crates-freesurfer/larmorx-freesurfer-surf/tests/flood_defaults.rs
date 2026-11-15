// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The corrections of relay 32 of track D (FS-604, FS-605, FS-607, FS-608): what the default gives
//! against what `reproduce` gives (the probe's `fli` compares `reproduce` with FreeSurfer's own
//! build; these tests show the defaults).

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_mri::mri::{MRI_UCHAR, Mri, mri_alloc};
use larmorx_freesurfer_mri::vol_geom::reinit_cache;
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::alloc;
use larmorx_freesurfer_surf::coords::Io;
use larmorx_freesurfer_surf::mris::Mris;
use larmorx_freesurfer_surf::{mriflood, sras};

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

    /// A surface of one triangle with a valid volume geometry of `w`^3 voxels of 1 mm (LIA)
    /// centred at `c`.
    fn surface(&mut self, w: i32, c: [f32; 3]) -> Mris {
        let mut m = alloc::alloc(&mut self.g, &mut self.out, &mut self.err, &self.sg, 3, 1)
            .expect("a surface");
        m.face_mut(0).v = [0, 1, 2];
        for (vno, p) in [[0.0f32, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]]
            .iter()
            .enumerate()
        {
            let v = m.vertex_mut(vno);
            (v.x, v.y, v.z) = (p[0], p[1], p[2]);
            let t = m.topology_mut(vno);
            let nb: Vec<i32> = (0..3).filter(|&x| x != vno as i32).collect();
            t.vnum = 2;
            t.vtotal = 2;
            t.v = Some(nb);
            t.num = 1;
            t.f = Some(vec![0]);
            t.n = Some(vec![vno as u8]);
        }
        let vg = &mut m.vg;
        vg.valid = 1;
        (vg.width, vg.height, vg.depth) = (w, w, w);
        (vg.xsize, vg.ysize, vg.zsize) = (1.0, 1.0, 1.0);
        (vg.x_r, vg.x_a, vg.x_s) = (-1.0, 0.0, 0.0);
        (vg.y_r, vg.y_a, vg.y_s) = (0.0, 0.0, -1.0);
        (vg.z_r, vg.z_a, vg.z_s) = (0.0, 1.0, 0.0);
        (vg.c_r, vg.c_a, vg.c_s) = (c[0], c[1], c[2]);
        m
    }

    /// A `uchar` volume of `w` x `h` x `d` voxels of 1 mm (LIA) centred at the origin.
    fn volume(&mut self, w: i32, h: i32, d: i32) -> Mri {
        let mut v = mri_alloc(
            &mut self.g,
            &mut self.out,
            &mut self.err,
            w,
            h,
            d,
            MRI_UCHAR,
        )
        .expect("a volume");
        let vg = &mut v.vg;
        (vg.x_r, vg.x_a, vg.x_s) = (-1.0, 0.0, 0.0);
        (vg.y_r, vg.y_a, vg.y_s) = (0.0, 0.0, -1.0);
        (vg.z_r, vg.z_a, vg.z_s) = (0.0, 1.0, 0.0);
        reinit_cache(&mut self.g, &mut self.out, &mut self.err, &mut v.vg).expect("the cache");
        v
    }

    fn noted(&self, id: &str) -> bool {
        self.g.notices.as_slice().iter().any(|n| n.defect.id == id)
    }
}

const P: (f64, f64, f64) = (1.25, -2.5, 3.0);

/// FS-604: the cached conversion keeps the first surface's matrix for a second surface whose
/// volume geometry differs, and the first volume's for one of another extent whose header was
/// copied from it (the headers compare equal). By default each call gives the direct conversion's voxel (noted); `reproduce` the
/// first pair's.
#[test]
fn fs604_cached_map_follows_the_surface_and_the_extent() {
    for mode in [UpstreamDefects::Reproduce, UpstreamDefects::Fix] {
        let mut r = Run::new(mode);
        let s1 = r.surface(16, [0.0, 0.0, 0.0]);
        let s2 = r.surface(16, [4.0, -2.0, 1.0]);
        let v1 = r.volume(16, 16, 16);
        // a volume of another extent whose header was copied from the first (MRIcopyHeader keeps
        // the extent): the headers compare equal
        let mut v2 = r.volume(20, 16, 16);
        larmorx_freesurfer_mri::mri::mri_copy_header(
            &mut r.g,
            &mut r.out,
            &mut r.err,
            &v1,
            Some(&mut v2),
        )
        .expect("the header");
        let mut io = Io {
            out: &mut r.out,
            err: &mut r.err,
            env: &r.env,
        };
        let (g, sg) = (&mut r.g, &mut r.sg);
        let direct = |g: &mut Globals, io: &mut Io, s: &Mris, v: &Mri| {
            sras::mris_surface_ras_to_voxel(g, io, s, v, P.0, P.1, P.2).expect("direct")
        };
        let d11 = direct(g, &mut io, &s1, &v1);
        let d21 = direct(g, &mut io, &s2, &v1);
        let d22 = direct(g, &mut io, &s2, &v2);
        assert_ne!(d11, d21);
        assert_ne!(d21, d22);
        let cached = |g: &mut Globals, sg: &mut SurfGlobals, io: &mut Io, s: &Mris, v: &Mri| {
            sras::mris_surface_ras_to_voxel_cached(g, sg, io, s, v, P.0, P.1, P.2).expect("cached")
        };
        assert_eq!(cached(g, sg, &mut io, &s1, &v1), d11);
        let c21 = cached(g, sg, &mut io, &s2, &v1);
        let c22 = cached(g, sg, &mut io, &s2, &v2);
        if mode.reproduce() {
            assert_eq!(c21, d11);
            assert_eq!(c22, d11);
        } else {
            assert_eq!(c21, d21);
            assert_eq!(c22, d22);
            assert!(r.noted("FS-604"));
        }
    }
}

/// FS-605: the cached conversion from voxels keeps its first call's matrix; by default it follows
/// the surface (noted).
#[test]
fn fs605_cached_inverse_follows_the_surface() {
    for mode in [UpstreamDefects::Reproduce, UpstreamDefects::Fix] {
        let mut r = Run::new(mode);
        let s1 = r.surface(16, [0.0, 0.0, 0.0]);
        let s2 = r.surface(16, [4.0, -2.0, 1.0]);
        let v = r.volume(16, 16, 16);
        let mut io = Io {
            out: &mut r.out,
            err: &mut r.err,
            env: &r.env,
        };
        let (g, sg) = (&mut r.g, &mut r.sg);
        let d1 = sras::mris_surface_ras_from_voxel(g, &mut io, &s1, &v, P.0, P.1, P.2).unwrap();
        let d2 = sras::mris_surface_ras_from_voxel(g, &mut io, &s2, &v, P.0, P.1, P.2).unwrap();
        assert_ne!(d1, d2);
        let c1 = sras::mris_surface_ras_from_voxel_cached(g, sg, &mut io, &s1, &v, P.0, P.1, P.2)
            .unwrap();
        let c2 = sras::mris_surface_ras_from_voxel_cached(g, sg, &mut io, &s2, &v, P.0, P.1, P.2)
            .unwrap();
        assert_eq!(c1, d1);
        if mode.reproduce() {
            assert_eq!(c2, d1);
        } else {
            assert_eq!(c2, d2);
            assert!(r.noted("FS-605"));
        }
    }
}

/// FS-607: a bounding box spanning more than 2^31 voxels: an error by default (status 252,
/// `ERROR_BADPARM`), the wrapped extent with `reproduce` (whose volume cannot be made either).
#[test]
fn fs607_bounding_volume_beyond_int() {
    for mode in [UpstreamDefects::Reproduce, UpstreamDefects::Fix] {
        let mut r = Run::new(mode);
        let mut s = r.surface(16, [0.0, 0.0, 0.0]);
        (s.xlo, s.ylo, s.zlo) = (-1.5e9, -1.0, -1.0);
        (s.xhi, s.yhi, s.zhi) = (1.5e9, 1.0, 1.0);
        let mut io = Io {
            out: &mut r.out,
            err: &mut r.err,
            env: &r.env,
        };
        let got = mriflood::mris_make_bounding_volume(&mut r.g, &mut io, &s, 1.0);
        assert!(got.is_err());
        assert!(r.noted("FS-607"));
        let text = String::from_utf8_lossy(&r.err).into_owned();
        assert_eq!(
            text.contains("more than an int holds"),
            !mode.reproduce(),
            "{text}"
        );
    }
}

/// FS-608: the interior of a surface that does not cross the given volume: FreeSurfer's
/// allocation of a box of negative size ends the program; by default a clear error, status 1
/// both ways.
#[test]
fn fs608_interior_outside_the_volume() {
    for mode in [UpstreamDefects::Reproduce, UpstreamDefects::Fix] {
        let mut r = Run::new(mode);
        let mut s = r.surface(16, [0.0, 0.0, 0.0]);
        for vno in 0..3 {
            s.vertex_mut(vno).x += 100.0;
        }
        let mut v = r.volume(8, 8, 8);
        let mut io = Io {
            out: &mut r.out,
            err: &mut r.err,
            env: &r.env,
        };
        let got =
            mriflood::mris_fill_interior(&mut r.g, &mut r.sg, &mut io, &mut s, 0.0, Some(&mut v));
        assert!(matches!(got, Err(Fatal::Exit(1))), "{got:?}");
        assert!(r.noted("FS-608"));
        let text = String::from_utf8_lossy(&r.err).into_owned();
        assert_eq!(
            text.contains("the surface does not cross the volume"),
            !mode.reproduce(),
            "{text}"
        );
    }
}
