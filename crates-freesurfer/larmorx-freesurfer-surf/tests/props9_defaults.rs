// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The corrections of queue item 3.1-51's leftovers (relay 23, FS-525 to FS-532): what the
//! default gives against what `reproduce` gives (the probe's `mpf` compares `reproduce` with
//! FreeSurfer's own build; these tests show the defaults).

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{ERROR_BADPARM, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, matrix_alloc};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_UCHAR, Mri, mri_alloc_sequence};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::alloc;
use larmorx_freesurfer_surf::coords::Io;
use larmorx_freesurfer_surf::mris::Mris;
use larmorx_freesurfer_surf::props9::{
    KernelOutput, mri_surface_ras_to_voxel, mris_ar1, mris_arn, mris_smooth_kernel,
};

struct Run {
    g: Globals,
    sg: SurfGlobals,
    env: Environment,
    out: Vec<u8>,
    err: Vec<u8>,
}

impl Run {
    fn new(mode: UpstreamDefects, env: &[&str]) -> Run {
        Run {
            g: Globals::new(mode),
            sg: SurfGlobals::default(),
            env: Environment::from_pairs(
                env.iter()
                    .map(|n| ((*n).to_owned(), "1".to_owned()))
                    .collect::<Vec<(String, String)>>(),
            ),
            out: Vec::new(),
            err: Vec::new(),
        }
    }

    /// A triangle: three vertices, each the other two's neighbour.
    fn triangle(&mut self) -> Mris {
        let mut m = alloc::alloc(&mut self.g, &mut self.out, &mut self.err, &self.sg, 3, 1)
            .expect("a surface");
        m.face_mut(0).v = [0, 1, 2];
        for vno in 0..3usize {
            let t = m.topology_mut(vno);
            t.v = Some((0..3).filter(|&k| k != vno as i32).collect::<Vec<i32>>());
            t.vnum = 2;
        }
        m
    }

    fn volume(&mut self, ty: i32, frames: &[[f32; 3]]) -> Mri {
        let mut v = mri_alloc_sequence(
            &mut self.g,
            &mut self.out,
            &mut self.err,
            3,
            1,
            1,
            ty,
            frames.len() as i32,
        )
        .expect("a volume");
        for (f, vals) in frames.iter().enumerate() {
            for (c, &x) in vals.iter().enumerate() {
                mri_set_vox_val(
                    &mut self.g,
                    &mut self.err,
                    &mut v,
                    c as i32,
                    0,
                    0,
                    f as i32,
                    x,
                    0,
                )
                .expect("a voxel");
            }
        }
        v
    }

    fn io(&mut self) -> (Io<'_>, &mut Globals, &mut SurfGlobals) {
        (
            Io {
                out: &mut self.out,
                err: &mut self.err,
                env: &self.env,
            },
            &mut self.g,
            &mut self.sg,
        )
    }

    fn value(&mut self, m: &Mri, c: i32, f: i32) -> f32 {
        mri_get_vox_val(&mut self.g, &mut self.err, m, c, 0, 0, f, 0).expect("a voxel")
    }
}

/// FS-526: a per-vertex kernel gives a weighted mean by default (here the same weights at every
/// vertex: hop 0 weight 2, hop 1 weight 1), NaN (0/0) with `reproduce`.
#[test]
fn fs526_the_vertex_kernel_is_used() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode, &[]);
        let mut s = r.triangle();
        let src = r.volume(MRI_FLOAT, &[[1.0, 2.0, 3.0]]);
        let kern = r.volume(MRI_FLOAT, &[[2.0, 2.0, 2.0], [1.0, 1.0, 1.0]]);
        let mut shl = None;
        let out = {
            let (mut io, g, _) = r.io();
            mris_smooth_kernel(
                g,
                &mut io,
                &mut s,
                &src,
                None,
                Some(&kern),
                None,
                &mut shl,
                KernelOutput::New,
            )
            .expect("no error")
            .expect("a volume")
        };
        let v0 = r.value(&out, 0, 0);
        if mode == UpstreamDefects::Fix {
            // (2 * 1 + 1 * 2 + 1 * 3) / (2 + 1 + 1)
            assert_eq!(v0, 7.0 / 4.0);
        } else {
            assert!(v0.is_nan());
        }
        assert_eq!(
            s.vertex(2).undefval,
            1,
            "{mode:?}: the hop lists set undefval"
        );
    }
}

/// FS-530: fewer than one hop is refused by default (`None`), crashes with `reproduce`.
#[test]
fn fs530_hops_below_one() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode, &[]);
        let mut s = r.triangle();
        let src = r.volume(MRI_FLOAT, &[[1.0, 2.0, 3.0], [2.0, 1.0, 0.5]]);
        let res = {
            let (mut io, g, _) = r.io();
            mris_arn(g, &mut io, &mut s, &src, None, None, 0)
        };
        match mode {
            UpstreamDefects::Fix => assert!(matches!(res, Ok(None))),
            _ => assert!(matches!(res, Err(Fatal::Exit(139)))),
        }
    }
}

/// FS-525: a volume of bytes is read by its values by default; with `reproduce` as FreeSurfer reads
/// it (floats over its bytes, past them FS-534).
#[test]
fn fs525_a_byte_volume() {
    let mut vals = Vec::new();
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode, &[]);
        let s = r.triangle();
        let src = r.volume(MRI_UCHAR, &[[1.0, 2.0, 3.0], [2.0, 1.0, 4.0]]);
        let flt = r.volume(MRI_FLOAT, &[[1.0, 2.0, 3.0], [2.0, 1.0, 4.0]]);
        let (a, b) = {
            let (mut io, g, _) = r.io();
            let a = mris_ar1(g, &mut io, &s, &src, None, None).expect("no error");
            let b = mris_ar1(g, &mut io, &s, &flt, None, None).expect("no error");
            (a.expect("a volume"), b.expect("a volume"))
        };
        let (x, y) = (r.value(&a, 0, 0), r.value(&b, 0, 0));
        if mode == UpstreamDefects::Fix {
            assert_eq!(x, y, "the byte volume's values as the float volume's");
        }
        assert!(
            r.g.notices
                .as_slice()
                .iter()
                .any(|n| n.defect.id == "FS-525")
        );
        vals.push(x);
    }
    assert_ne!(vals[0].to_bits(), vals[1].to_bits());
}

/// FS-531 and FS-532: with both ways the default answers (the two agree), `reproduce` crashes; with
/// no matrix the default refuses, `reproduce` crashes.
#[test]
fn fs531_fs532_surface_ras_to_voxel() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = Run::new(mode, &["FREESURFER_mriSurfaceRASToVoxel_both"]);
        {
            let (mut io, g, sg) = r.io();
            let res = mri_surface_ras_to_voxel(g, sg, &mut io, 1.0, 2.0, 3.0);
            match mode {
                UpstreamDefects::Fix => {
                    assert!(matches!(res, Ok((e, None)) if e == ERROR_BADPARM))
                }
                _ => assert!(matches!(res, Err(Fatal::Exit(139)))),
            }
        }
        let mut m = matrix_alloc(&mut r.g, &mut r.out, &mut r.err, 4, 4, MATRIX_REAL)
            .expect("no error")
            .expect("a matrix");
        for k in 1..=4 {
            m.set_relt(k, k, 2.0);
        }
        r.sg.voxel_from_sras = Some(m);
        let (mut io, g, sg) = r.io();
        let res = mri_surface_ras_to_voxel(g, sg, &mut io, 1.0, 2.0, 3.0);
        match mode {
            UpstreamDefects::Fix => {
                assert!(matches!(res, Ok((e, Some(v))) if e == NO_ERROR && v == (2.0, 4.0, 6.0)))
            }
            _ => assert!(matches!(res, Err(Fatal::Exit(139)))),
        }
    }
}
