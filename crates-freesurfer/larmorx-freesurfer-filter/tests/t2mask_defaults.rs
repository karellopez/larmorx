// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The T2 mask (`MRImask_with_T2_and_aparc_aseg`, `utils/mri2.cpp`): a head-like volume (a dark
//! border two voxels thick, a bright rim, a medium interior) and the defects FS-615 to FS-618.

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_filter::t2mask::mri_mask_with_t2_and_aparc_aseg;
use larmorx_freesurfer_mri::mri::{MRI_UCHAR, Mri, mri_alloc};
use larmorx_freesurfer_volume::arith::Pair;

const N: i32 = 9;

fn noted(g: &Globals, id: &str) -> bool {
    g.notices.as_slice().iter().any(|n| n.defect.id == id)
}

fn depth(x: i32, y: i32, z: i32) -> i32 {
    [x, N - 1 - x, y, N - 1 - y, z, N - 1 - z]
        .into_iter()
        .min()
        .unwrap()
}

fn vol(g: &mut Globals, mut val: impl FnMut(i32, i32, i32) -> u8) -> Mri {
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let mut m = mri_alloc(g, &mut o, &mut e, N, N, N, MRI_UCHAR).unwrap();
    let c = m.chunk.as_mut().unwrap();
    for z in 0..N {
        for y in 0..N {
            for x in 0..N {
                c[((z * N + y) * N + x) as usize] = val(x, y, z);
            }
        }
    }
    m
}

/// The T2: 0 at depths 0 and 1, 200 at depth 2, 100 inside.
fn t2(g: &mut Globals) -> Mri {
    vol(g, |x, y, z| match depth(x, y, z) {
        0 | 1 => 0,
        2 => 200,
        _ => 100,
    })
}

fn at(m: &Mri, x: i32, y: i32, z: i32) -> u8 {
    m.chunk.as_ref().unwrap()[((z * N + y) * N + x) as usize]
}

/// The voxels of `m` set to 0 are exactly those at depth 2 or less (the dark border and the
/// bright rim), the others keep `keep`.
fn masked_to_interior(m: &Mri, keep: u8) -> bool {
    (0..N).all(|z| {
        (0..N).all(|y| {
            (0..N).all(|x| {
                let want = if depth(x, y, z) <= 2 { 0 } else { keep };
                at(m, x, y, z) == want
            })
        })
    })
}

#[allow(clippy::type_complexity)]
fn call(
    mode: UpstreamDefects,
    f: impl FnOnce(
        &mut Globals,
        &mut Vec<u8>,
        &mut Vec<u8>,
        &Mri,
    ) -> Result<Ret<Mri>, larmorx_freesurfer_core::log::Fatal>,
) -> (
    Globals,
    Result<Ret<Mri>, larmorx_freesurfer_core::log::Fatal>,
    String,
    String,
) {
    let mut g = Globals::new(mode);
    let t = t2(&mut g);
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let r = f(&mut g, &mut o, &mut e, &t);
    let (o, e) = (
        String::from_utf8_lossy(&o).into_owned(),
        String::from_utf8_lossy(&e).into_owned(),
    );
    (g, r, o, e)
}

/// FS-615: without an aparc+aseg the pass erases the 98 bright rim voxels next to the dark border
/// (its test of the aparc+aseg skipped); with `reproduce`, FreeSurfer's crash (an error).
#[test]
fn without_an_aparc_aseg_the_rim_is_erased() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut src = None;
        let (g, r, o, e) = call(mode, |g, o, e, t| {
            let mut s = vol(g, |_, _, _| 7);
            let r = mri_mask_with_t2_and_aparc_aseg(
                g,
                o,
                e,
                &Environment::default(),
                Pair::Same(&mut s),
                t,
                None,
                150.0,
                1,
            );
            src = Some(s);
            r
        });
        assert!(noted(&g, "FS-615"));
        if mode == UpstreamDefects::Fix {
            assert!(matches!(r, Ok(Ret::Given)));
            assert_eq!(o, "98 T2-bright exterior voxels removed\n");
            assert!(masked_to_interior(src.as_ref().unwrap(), 7));
        } else {
            assert!(r.is_err(), "{o}");
            assert!(e.contains("FreeSurfer crashes here"), "{e}");
        }
    }
}

/// With an aparc+aseg whose brain is the interior (depth 3 and more): its dilation reaches the
/// rim, so no rim voxel is erased; the aparc+aseg is changed in place (1 on the exterior, 0 on
/// the dilated brain). The same in both modes.
#[test]
fn an_aparc_aseg_keeps_the_rim_and_is_changed_in_place() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut g = Globals::new(mode);
        let t = t2(&mut g);
        let mut s = vol(&mut g, |_, _, _| 7);
        let mut ap = vol(&mut g, |x, y, z| if depth(x, y, z) >= 3 { 41 } else { 0 });
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let r = mri_mask_with_t2_and_aparc_aseg(
            &mut g,
            &mut o,
            &mut e,
            &Environment::default(),
            Pair::Same(&mut s),
            &t,
            Some(&mut ap),
            150.0,
            5,
        )
        .unwrap();
        assert!(matches!(r, Ret::Given));
        assert!(g.notices.as_slice().is_empty());
        let o = String::from_utf8_lossy(&o);
        assert_eq!(o.matches(" T2-bright exterior voxels removed\n").count(), 5);
        assert!(o.starts_with("0 T2-bright"), "{o}");
        for z in 0..N {
            for y in 0..N {
                for x in 0..N {
                    let want = u8::from(depth(x, y, z) < 2);
                    assert_eq!(at(&ap, x, y, z), want, "({x}, {y}, {z})");
                }
            }
        }
    }
}

/// FS-616: a destination other than the source gets what the call in place gives the source;
/// with `reproduce`, what it gives the destination's own voxels (FreeSurfer's call).
#[test]
fn a_destination_of_its_own_gets_the_source() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let run = |p: &mut dyn FnMut(&mut Globals, &mut Mri) -> Ret<Mri>| {
            let mut g = Globals::new(mode);
            let mut ap = vol(&mut g, |x, y, z| if depth(x, y, z) >= 4 { 2 } else { 0 });
            let r = p(&mut g, &mut ap);
            (g, r)
        };
        let t = t2(&mut Globals::new(mode));
        let s = vol(&mut Globals::new(mode), |x, _, _| 7 + x as u8);
        let mut d = vol(&mut Globals::new(mode), |_, y, _| 30 + y as u8);
        let (g, r) = run(&mut |g, ap| {
            let (mut o, mut e) = (Vec::new(), Vec::new());
            let env = Environment::default();
            let p = Pair::Given(&s, &mut d);
            mri_mask_with_t2_and_aparc_aseg(g, &mut o, &mut e, &env, p, &t, Some(ap), 150.0, 1)
                .unwrap()
        });
        assert!(matches!(r, Ret::Given));
        assert!(noted(&g, "FS-616"));
        let mut want = if mode == UpstreamDefects::Fix {
            vol(&mut Globals::new(mode), |x, _, _| 7 + x as u8)
        } else {
            vol(&mut Globals::new(mode), |_, y, _| 30 + y as u8)
        };
        let (g2, _) = run(&mut |g, ap| {
            let (mut o, mut e) = (Vec::new(), Vec::new());
            let env = Environment::default();
            let p = Pair::Same(&mut want);
            mri_mask_with_t2_and_aparc_aseg(g, &mut o, &mut e, &env, p, &t, Some(ap), 150.0, 1)
                .unwrap()
        });
        assert!(g2.notices.as_slice().is_empty());
        assert_eq!(d.chunk, want.chunk);
        assert!(d.chunk.as_ref().unwrap().iter().any(|&v| v != 0));
    }
}

/// FS-617: no pass: `MRIfree: null pointer` only with `reproduce`.
#[test]
fn no_pass_prints_no_error() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut g = Globals::new(mode);
        let t = t2(&mut g);
        let mut s = vol(&mut g, |_, _, _| 7);
        let mut ap = vol(&mut g, |x, y, z| if depth(x, y, z) >= 3 { 2 } else { 0 });
        let (mut o, mut e) = (Vec::new(), Vec::new());
        mri_mask_with_t2_and_aparc_aseg(
            &mut g,
            &mut o,
            &mut e,
            &Environment::default(),
            Pair::Same(&mut s),
            &t,
            Some(&mut ap),
            150.0,
            0,
        )
        .unwrap();
        assert!(noted(&g, "FS-617"));
        let e = String::from_utf8_lossy(&e);
        assert_eq!(
            e.contains("MRIfree: null pointer"),
            mode == UpstreamDefects::Reproduce,
            "{e}"
        );
    }
}

/// FS-618: no destination: a new volume, the source masked (with `reproduce`, FreeSurfer's crash
/// at the end, after its messages: an error).
#[test]
fn no_destination_gives_a_new_volume() {
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (g, r, o, e) = call(mode, |g, o, e, t| {
            let s = vol(g, |_, _, _| 7);
            let mut ap = vol(g, |x, y, z| if depth(x, y, z) >= 4 { 2 } else { 0 });
            mri_mask_with_t2_and_aparc_aseg(
                g,
                o,
                e,
                &Environment::default(),
                Pair::New(&s),
                t,
                Some(&mut ap),
                150.0,
                1,
            )
        });
        assert!(noted(&g, "FS-618"));
        assert!(o.ends_with(" T2-bright exterior voxels removed\n"), "{o}");
        if mode == UpstreamDefects::Fix {
            match r {
                Ok(Ret::New(m)) => {
                    let c = m.chunk.as_ref().unwrap();
                    assert!(c.iter().all(|&v| v == 0 || v == 7));
                }
                _ => panic!("a new volume"),
            }
        } else {
            assert!(r.is_err());
            assert!(
                e.contains("no destination (FreeSurfer crashes here)"),
                "{e}"
            );
        }
    }
}
