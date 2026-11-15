// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The corrections of relay 33 of track D (FS-609 to FS-613): what the default gives against what
//! `reproduce` gives (the probe's `bvl` compares `reproduce` with FreeSurfer's own build; these
//! tests show the defaults).

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_io::label::{Label, LabelVertex};
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_UCHAR, Mri, mri_alloc};
use larmorx_freesurfer_mri::vol_geom::reinit_cache;
use larmorx_freesurfer_mri::voxel::mri_set_vox_val;
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::alloc;
use larmorx_freesurfer_surf::binarize;
use larmorx_freesurfer_surf::coords::Io;
use larmorx_freesurfer_surf::mris::Mris;
use larmorx_freesurfer_surf::positions::{restore_vertex_positions, save_vertex_positions};
use larmorx_freesurfer_surf::views::{PIAL_VERTICES, WHITE_VERTICES};

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

    /// A volume of type `ty`, `w` x `h` x `d` voxels of 1 mm (LIA) centred at the origin.
    fn volume(&mut self, w: i32, h: i32, d: i32, ty: i32) -> Mri {
        let mut v =
            mri_alloc(&mut self.g, &mut self.out, &mut self.err, w, h, d, ty).expect("a volume");
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

fn label(vnos: &[i32]) -> Label {
    Label {
        max_points: vnos.len() as i32,
        n_points: vnos.len() as i32,
        lv: vnos
            .iter()
            .map(|&vno| LabelVertex {
                vno,
                ..LabelVertex::default()
            })
            .collect(),
        ..Label::default()
    }
}

fn set_values(m: &mut Mris) {
    for vno in 0..3 {
        let v = m.vertex_mut(vno);
        v.curv = 1.0 + vno as f32;
        (v.stat, v.val, v.imag_val, v.val2, v.valbak, v.val2bak) = (2.0, 3.0, 4.0, 5.0, 6.0, 7.0);
    }
}

/// FS-611: label points that name no vertex (-1, or one beyond the surface's) are left out in
/// both modes (FreeSurfer writes outside the vertices), noted.
#[test]
fn fs611_label_points_without_a_vertex_are_left_out() {
    for mode in [UpstreamDefects::Reproduce, UpstreamDefects::Fix] {
        let mut r = Run::new(mode);
        let mut s = r.surface(16, [0.0, 0.0, 0.0]);
        set_values(&mut s);
        binarize::mris_mask_label(&mut r.g, &mut s, &label(&[-1, 1, 3]));
        assert_eq!(s.vertex(1).curv, 0.0);
        assert_eq!(s.vertex(0).curv, 1.0);
        assert_eq!(s.vertex(2).curv, 3.0);
        assert!(r.noted("FS-611"));
    }
}

/// FS-612: a vertex marked before `MRISmaskNotLabel` keeps its values, and a label vertex marked
/// before loses its mark, in both modes (kept), noted.
#[test]
fn fs612_earlier_marks_are_kept_and_noted() {
    for mode in [UpstreamDefects::Reproduce, UpstreamDefects::Fix] {
        let mut r = Run::new(mode);
        let mut s = r.surface(16, [0.0, 0.0, 0.0]);
        set_values(&mut s);
        s.vertex_mut(1).marked = 1;
        s.vertex_mut(0).marked = 1;
        binarize::mris_mask_not_label(&mut r.g, &mut s, &label(&[0]));
        assert_eq!(s.vertex(0).curv, 1.0);
        assert_eq!(s.vertex(0).marked, 0);
        assert_eq!(s.vertex(1).curv, 2.0);
        assert_eq!(s.vertex(1).marked, 1);
        assert_eq!(s.vertex(2).curv, 0.0);
        assert!(r.noted("FS-612"));
    }
}

/// FS-609 and FS-610: a float label volume holding the label 2: by default its values are
/// compared (the box is found); `reproduce` compares one byte of each value as FreeSurfer does
/// (none is 2) and ends with FreeSurfer's message naming 0, the default's names the label.
#[test]
fn fs609_fs610_float_label_volume_and_the_message() {
    for mode in [UpstreamDefects::Reproduce, UpstreamDefects::Fix] {
        let mut r = Run::new(mode);
        let mut v = r.volume(8, 8, 8, MRI_FLOAT);
        mri_set_vox_val(&mut r.g, &mut r.err, &mut v, 3, 4, 5, 0, 2.0, 0).expect("a voxel");
        let mut io = Io {
            out: &mut r.out,
            err: &mut r.err,
            env: &r.env,
        };
        let found = binarize::mri_find_label(&mut r.g, &mut io, &mut v, 2, 1);
        if mode.reproduce() {
            assert!(matches!(found, Err(Fatal::Exit(1))));
        } else {
            let reg = found.expect("the label's box");
            assert_eq!((reg.dx, reg.dy, reg.dz), (2, 2, 2));
        }
        assert!(r.noted("FS-609"));
        let mut u = r.volume(8, 8, 8, MRI_UCHAR);
        r.err.clear();
        let mut io = Io {
            out: &mut r.out,
            err: &mut r.err,
            env: &r.env,
        };
        let e = binarize::mri_find_label(&mut r.g, &mut io, &mut u, 3, 0);
        assert!(matches!(e, Err(Fatal::Exit(1))));
        let text = String::from_utf8_lossy(&r.err).into_owned();
        let named = if mode.reproduce() { 0 } else { 3 };
        assert!(
            text.contains(&format!("No labels {named} could be found in the volume")),
            "{text}"
        );
        assert!(r.noted("FS-610"));
    }
}

/// An octahedron of radius `r` (mm) about the origin, read by `MRISread` from a binary triangle
/// file, with a valid volume geometry of 16^3 voxels of 1 mm (LIA) centred at the origin.
fn octahedron(r: &mut Run, radius: f32) -> Mris {
    let v: [[f32; 3]; 6] = [
        [radius, 0.0, 0.0],
        [-radius, 0.0, 0.0],
        [0.0, radius, 0.0],
        [0.0, -radius, 0.0],
        [0.0, 0.0, radius],
        [0.0, 0.0, -radius],
    ];
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
    b.extend_from_slice(b"created by me on Sun Nov 15 10:00:00 2026\n\n");
    b.extend_from_slice(&6i32.to_be_bytes());
    b.extend_from_slice(&8i32.to_be_bytes());
    for p in &v {
        for c in p {
            b.extend_from_slice(&c.to_be_bytes());
        }
    }
    for t in &f {
        for x in t {
            b.extend_from_slice(&x.to_be_bytes());
        }
    }
    let td = tempfile::tempdir().expect("a folder");
    std::fs::write(td.path().join("lh.oct"), b).expect("the surface file");
    let env = Environment::from_pairs(vec![("SURFER_SIDEDOOR".to_owned(), "1".to_owned())]);
    let mut io = Io {
        out: &mut r.out,
        err: &mut r.err,
        env: &env,
    };
    let mut m = larmorx_freesurfer_surf::readwrite::read(
        &mut r.g,
        &mut r.sg,
        &mut io,
        Some(td.path()),
        "lh.oct",
        true,
    )
    .expect("the read")
    .expect("a surface");
    let vg = &mut m.vg;
    vg.valid = 1;
    (vg.width, vg.height, vg.depth) = (16, 16, 16);
    (vg.xsize, vg.ysize, vg.zsize) = (1.0, 1.0, 1.0);
    (vg.x_r, vg.x_a, vg.x_s) = (-1.0, 0.0, 0.0);
    (vg.y_r, vg.y_a, vg.y_s) = (0.0, 0.0, -1.0);
    (vg.z_r, vg.z_a, vg.z_s) = (0.0, 1.0, 0.0);
    m
}

/// FS-613: a second call of `MRIcomputeLaminarVolumeFractions` after the white positions moved:
/// `reproduce` takes the first call's white interior, the default the new one's (noted in both).
#[test]
fn fs613_each_call_has_its_own_white_interior() {
    let mut results = Vec::new();
    for mode in [UpstreamDefects::Reproduce, UpstreamDefects::Fix] {
        let mut r = Run::new(mode);
        let pial = octahedron(&mut r, 6.0);
        let mut s = octahedron(&mut r, 3.0);
        let v = r.volume(16, 16, 16, MRI_UCHAR);
        let mut io = Io {
            out: &mut r.out,
            err: &mut r.err,
            env: &r.env,
        };
        let (g, sg) = (&mut r.g, &mut r.sg);
        save_vertex_positions(g, &mut io, &mut s, WHITE_VERTICES).expect("white");
        for vno in 0..6 {
            let p = pial.vertex(vno);
            let (x, y, z) = (p.x, p.y, p.z);
            let q = s.vertex_mut(vno);
            (q.x, q.y, q.z) = (x, y, z);
        }
        save_vertex_positions(g, &mut io, &mut s, PIAL_VERTICES).expect("pial");
        restore_vertex_positions(g, &mut io, &mut s, WHITE_VERTICES).expect("white");
        let lvf = |g: &mut Globals, sg: &mut SurfGlobals, io: &mut Io, s: &mut Mris| {
            binarize::mri_compute_laminar_volume_fractions(g, sg, io, s, 1.0, &v, None)
                .expect("the call")
                .expect("a new volume")
        };
        let first = lvf(g, sg, &mut io, &mut s);
        assert!(!g.notices.as_slice().iter().any(|n| n.defect.id == "FS-613"));
        // the white positions moved by 2 mm along x
        for vno in 0..6 {
            s.vertex_mut(vno).x += 2.0;
        }
        save_vertex_positions(g, &mut io, &mut s, WHITE_VERTICES).expect("white");
        let second = lvf(g, sg, &mut io, &mut s);
        assert!(r.noted("FS-613"));
        results.push((first.chunk.clone(), second.chunk.clone()));
    }
    let (rep, fix) = (&results[0], &results[1]);
    assert_eq!(rep.0, fix.0);
    // reproduce: the second call is the first's (its white interior); the default's differs
    assert_eq!(rep.1, rep.0);
    assert_ne!(fix.1, rep.1);
}
