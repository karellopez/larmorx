// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_fill -topofix`'s correction (queue item 3.2-34) before the program is assembled (3.2-37,
//! where it is checked against the official program): on a 16^3 volume of the two hemispheres'
//! labels made with integer arithmetic, no edge or corner configuration of either label is left,
//! the other hemisphere's voxels are kept wherever the choice allowed it, and `fix` differs from
//! `reproduce` only by the background components it fills (FS-1287).

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_mri::mri::{MRI_UCHAR, Mri, mri_alloc};
use larmorx_freesurfer_volumes::mri_fill::topofix::{Counters, mri_topofix};

const N: i32 = 16;
const RH: i32 = 127;
const LH: i32 = 255;

fn jitter(x: i32, y: i32, z: i32, n: i32) -> i32 {
    (x * 73 + y * 151 + z * 283 + x * y * 7 + y * z * 13 + x * z * 3) % n
}

fn volume(g: &mut Globals, f: impl Fn(i32, i32, i32) -> u8) -> Mri {
    let (mut o, mut e) = (std::io::sink(), std::io::sink());
    let mut m = mri_alloc(g, &mut o, &mut e, N, N, N, MRI_UCHAR).expect("a volume");
    let c = m.chunk.as_mut().expect("voxels");
    for z in 0..N {
        for y in 0..N {
            for x in 0..N {
                c[((z * N + y) * N + x) as usize] = f(x, y, z);
            }
        }
    }
    m
}

fn labels(x: i32, y: i32, z: i32) -> u8 {
    if !(1..N - 1).contains(&x) || !(1..N - 1).contains(&y) || !(1..N - 1).contains(&z) {
        return 0;
    }
    if jitter(x + 2, y, z + 1, 7) >= 4 {
        return 0;
    }
    if x < N / 2 { RH as u8 } else { LH as u8 }
}

fn at(m: &Mri, x: i32, y: i32, z: i32) -> i32 {
    i32::from(m.chunk.as_ref().expect("voxels")[((z * N + y) * N + x) as usize])
}

/// The edge configurations of `label` left: two voxels of it meeting at an edge with neither of
/// the two voxels joining them of it.
fn edges_left(m: &Mri, label: i32) -> usize {
    let mut n = 0;
    let is = |x: i32, y: i32, z: i32| {
        (0..N).contains(&x) && (0..N).contains(&y) && (0..N).contains(&z) && at(m, x, y, z) == label
    };
    for z in 0..N {
        for y in 0..N {
            for x in 0..N {
                if !is(x, y, z) {
                    continue;
                }
                for (dx, dy, dz) in [
                    (1, 1, 0),
                    (-1, 1, 0),
                    (0, 1, 1),
                    (0, 1, -1),
                    (1, 0, 1),
                    (-1, 0, 1),
                ] {
                    // the two voxels joining them: the steps along each nonzero axis
                    let a = if dx != 0 {
                        (x + dx, y, z)
                    } else {
                        (x, y + dy, z)
                    };
                    let b = if dz != 0 {
                        (x, y, z + dz)
                    } else {
                        (x, y + dy, z)
                    };
                    if is(x + dx, y + dy, z + dz) && !is(a.0, a.1, a.2) && !is(b.0, b.1, b.2) {
                        n += 1;
                    }
                }
            }
        }
    }
    n
}

fn run(mode: UpstreamDefects) -> (Mri, String) {
    let mut g = Globals::new(mode);
    let mut seg = volume(&mut g, labels);
    let orig = volume(&mut g, |x, y, z| (jitter(x, y + 1, z + 2, 12) * 20) as u8);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let mut c = Counters::default();
    mri_topofix(&mut g, &mut out, &mut err, &mut seg, &orig, RH, LH, &mut c).expect("corrected");
    (seg, String::from_utf8_lossy(&err).into_owned())
}

#[test]
fn no_edge_configuration_is_left() {
    let mut g = Globals::new(UpstreamDefects::Reproduce);
    let before = volume(&mut g, labels);
    assert!(edges_left(&before, RH) > 0 && edges_left(&before, LH) > 0);
    for mode in [UpstreamDefects::Reproduce, UpstreamDefects::Fix] {
        let (seg, err) = run(mode);
        assert_eq!(edges_left(&seg, RH), 0, "{err}");
        assert_eq!(edges_left(&seg, LH), 0, "{err}");
        assert!(err.contains("Topological correction completed with"));
    }
}

#[test]
fn fix_differs_only_by_the_filled_background_components() {
    let (rep, rep_err) = run(UpstreamDefects::Reproduce);
    let (fix, _) = run(UpstreamDefects::Fix);
    let mut differ = 0;
    for z in 0..N {
        for y in 0..N {
            for x in 0..N {
                let (a, b) = (at(&rep, x, y, z), at(&fix, x, y, z));
                if a != b {
                    assert_eq!(a, 0, "a voxel other than the background changed");
                    assert!(b == RH || b == LH);
                    differ += 1;
                }
            }
        }
    }
    // the volume has components to fill
    assert!(differ > 0 && rep_err.contains("removing component"));
}
