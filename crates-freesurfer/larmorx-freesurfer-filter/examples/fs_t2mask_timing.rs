// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The time each step of the T2 mask (`t2mask.rs`, `MRImask_with_T2_and_aparc_aseg`) takes on a
//! recorded head (relay 34 of track D; `validation/probes/fs-filter/README.md`):
//!
//!     fs_t2mask_timing <volume> <T2> <aparc+aseg> <threshold> <layers>
//!
//! prints the seconds of the reads, of the mask's steps one by one (the same calls in the same
//! order as the function) and of the whole function on fresh copies.

use std::time::Instant;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_filter::cc::get_largest_cc6;
use larmorx_freesurfer_filter::t2mask::mri_mask_with_t2_and_aparc_aseg;
use larmorx_freesurfer_io::identify::MRI_VOLUME_TYPE_UNKNOWN;
use larmorx_freesurfer_io::mriio::mri_read;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_volume::arith::{Pair, Tri, mri_binarize};
use larmorx_freesurfer_volume::mriset::{mri_and, mri_dilate, mri_not, mri_open_n};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(
        args.len(),
        6,
        "usage: fs_t2mask_timing <volume> <T2> <aparc+aseg> <threshold> <layers>"
    );
    let mut g = Globals::new(UpstreamDefects::Reproduce);
    let env = Environment::default();
    let (mut out, mut err) = (std::io::sink(), std::io::stderr());
    let t = Instant::now();
    let mut read = |g: &mut Globals, p: &str| -> Mri {
        mri_read(
            g,
            &mut out,
            &mut err,
            &env,
            None,
            p,
            MRI_VOLUME_TYPE_UNKNOWN,
            true,
            -1,
            -1,
        )
        .expect("read")
        .expect("a volume")
    };
    let src = read(&mut g, &args[1]);
    let t2 = read(&mut g, &args[2]);
    let ap0 = read(&mut g, &args[3]);
    println!("reads {:.3} s", t.elapsed().as_secs_f64());
    let th: f32 = args[4].parse().expect("threshold");
    let mm: i32 = args[5].parse().expect("layers");
    let (mut o, mut e) = (std::io::sink(), std::io::sink());
    let step = |name: &str, t: Instant| println!("{name} {:.3} s", t.elapsed().as_secs_f64());

    let t = Instant::now();
    let mut mask = mri_binarize(&mut g, &mut o, &mut e, Pair::New(&t2), th, 255.0, 0.0)
        .unwrap()
        .unwrap_new();
    step("binarize T2", t);
    let t = Instant::now();
    let _bright = mri_copy(&mut g, &mut o, &mut e, &mask, None).unwrap();
    step("copy", t);
    let mut ap = mri_copy(&mut g, &mut o, &mut e, &ap0, None)
        .unwrap()
        .unwrap_new();
    let t = Instant::now();
    mri_binarize(&mut g, &mut o, &mut e, Pair::Same(&mut ap), 1.0, 0.0, 255.0).unwrap();
    step("binarize aparc+aseg", t);
    let t = Instant::now();
    mri_dilate(&mut g, &mut o, &mut e, Pair::Same(&mut ap)).unwrap();
    step("dilate", t);
    let t = Instant::now();
    mri_not(&mut g, &mut o, &mut e, Pair::Same(&mut ap)).unwrap();
    step("not", t);
    let t = Instant::now();
    get_largest_cc6(&mut g, &mut o, &mut e, &mut ap).unwrap();
    step("largest CC6", t);
    let t = Instant::now();
    mri_and(&mut g, &mut o, &mut e, Tri::First(&mut mask, &ap), 1).unwrap();
    step("and", t);
    let t = Instant::now();
    mri_open_n(&mut g, &mut o, &mut e, Pair::Same(&mut mask), 3).unwrap();
    step("open 3", t);
    let t = Instant::now();
    mri_not(&mut g, &mut o, &mut e, Pair::Same(&mut mask)).unwrap();
    step("not", t);

    for k in 0..2 {
        let mut s = mri_copy(&mut g, &mut o, &mut e, &src, None)
            .unwrap()
            .unwrap_new();
        let mut a = mri_copy(&mut g, &mut o, &mut e, &ap0, None)
            .unwrap()
            .unwrap_new();
        let t = Instant::now();
        mri_mask_with_t2_and_aparc_aseg(
            &mut g,
            &mut o,
            &mut e,
            &env,
            Pair::Same(&mut s),
            &t2,
            Some(&mut a),
            th,
            mm,
        )
        .unwrap();
        step(&format!("whole function ({k})"), t);
    }
}
