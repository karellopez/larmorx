// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The port's side of `validation/probes/fs-normalize/bench.cpp`: where `mri_normalize`'s time
//! goes, stage by stage, with FreeSurfer's default behaviour reproduced (`reproduce`). The same
//! stages and digests as the official bench, plus the steps inside the outlier removal
//! (`wmo`'s `+` lines: the port's own split of the one official stage).
//!
//!     fs_normalize_bench bobm <volume> <atlas> <transform> <whalf>
//!     fs_normalize_bench wmo <volume> <aseg> <intensity_below>

use std::io::sink;
use std::time::Instant;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_filter::fastmarch::{DTRANS_MODE_SIGNED, mri_distance_transform};
use larmorx_freesurfer_gca::read::gca_read;
use larmorx_freesurfer_gca::transform::transform_read;
use larmorx_freesurfer_io::mriio::mri_read_volume;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, Mri};
use larmorx_freesurfer_volume::arith::{Pair, mri_binarize, mri_clone, mri_scalar_mul};
use larmorx_freesurfer_volume::deriv::mri_non_max_suppress;
use larmorx_freesurfer_volume::mriset::mri_copy_label;
use larmorx_freesurfer_volume::resize::mri_change_type;
use larmorx_freesurfer_volumes::mri_normalize::helpers::{self as h, Run};

fn fnv(m: &Mri) -> u64 {
    let mut x: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in m.chunk.as_ref().expect("voxels") {
        x ^= u64::from(b);
        x = x.wrapping_mul(0x0100_0000_01b3);
    }
    x
}

struct Clock(Instant);

impl Clock {
    fn stage(&mut self, name: &str) {
        let t = Instant::now();
        println!("{name} {:.3}", (t - self.0).as_secs_f64());
        self.0 = t;
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut g = Globals::new(UpstreamDefects::Reproduce);
    let env = Environment::default();
    let (mut o, mut e) = (sink(), sink());
    let mut c = Clock(Instant::now());
    match (a.get(1).map(String::as_str), a.len()) {
        (Some("bobm"), 6) => {
            let src = mri_read_volume(&mut g, &mut o, &mut e, &env, None, &a[2]).unwrap();
            c.stage("MRIread");
            let atlas = gca_read(&mut g, &mut o, &mut e, None, &a[3]).unwrap();
            c.stage("GCAread");
            let xf = transform_read(&mut g, &mut o, &mut e, &env, None, &a[4]).unwrap();
            c.stage("TransformRead");
            let (Some(src), Some(atlas), Some(mut xf)) = (src, atlas, xf) else {
                std::process::exit(1);
            };
            let whalf: i32 = a[5].parse().expect("whalf");
            let m = h::build_outside_of_brain_mask(
                &mut g, &mut o, &mut e, &env, &src, &atlas, &mut xf, whalf,
            )
            .unwrap();
            c.stage("build_outside_of_brain_mask");
            println!("digest {:016x}", fnv(&m));
        }
        (Some("wmo"), 5) => {
            let src = mri_read_volume(&mut g, &mut o, &mut e, &env, None, &a[2]).unwrap();
            c.stage("MRIread");
            let aseg = mri_read_volume(&mut g, &mut o, &mut e, &env, None, &a[3]).unwrap();
            c.stage("MRIread-aseg");
            let (Some(src), Some(aseg)) = (src, aseg) else {
                std::process::exit(1);
            };
            let src = if src.mtype == MRI_FLOAT {
                src
            } else {
                mri_change_type(&mut g, &mut o, &mut e, &env, &src, MRI_FLOAT, 0.0, 255.0, 1)
                    .unwrap()
                    .expect("a float volume")
            };
            c.stage("MRIchangeType");
            let mut ctrl = mri_clone(&mut g, &mut o, &mut e, &aseg, None)
                .unwrap()
                .unwrap_new();
            for label in [2, 41] {
                mri_copy_label(&mut g, &mut e, &aseg, &mut ctrl, label).unwrap();
            }
            c.stage("MRIclone+MRIcopyLabel");
            // the outlier removal's first steps, timed one by one (results dropped)
            let bin = mri_binarize(&mut g, &mut o, &mut e, Pair::New(&ctrl), 1.0, 0.0, 1.0)
                .unwrap()
                .unwrap_new();
            c.stage("+ MRIbinarize");
            let mut dist = mri_distance_transform(
                &mut g,
                &mut o,
                &mut e,
                &env,
                &bin,
                None,
                1,
                -1.0,
                DTRANS_MODE_SIGNED,
                None,
            )
            .unwrap()
            .unwrap_new();
            c.stage("+ MRIdistanceTransform");
            mri_scalar_mul(&mut g, &mut o, &mut e, Pair::Same(&mut dist), -1.0).unwrap();
            c.stage("+ MRIscalarMul");
            let sup = mri_non_max_suppress(&mut g, &mut o, &mut e, &dist, None, 0.0, 1).unwrap();
            c.stage("+ MRInonMaxSuppress");
            drop((bin, dist, sup));
            c.0 = Instant::now();
            let ib: i32 = a[4].parse().expect("intensity_below");
            let mut r = Run {
                out: &mut o,
                err: &mut e,
                env: &env,
                dir: None,
            };
            h::mri_remove_wm_outliers_and_retain_medial_surface(&mut g, &mut r, &src, &mut ctrl, ib)
                .unwrap();
            c.stage("MRIremoveWMOutliersAndRetainMedialSurface");
            println!("digest {:016x}", fnv(&ctrl));
        }
        _ => {
            eprintln!(
                "usage: fs_normalize_bench bobm <volume> <atlas> <transform> <whalf> | wmo <volume> <aseg> <ib>"
            );
            std::process::exit(2);
        }
    }
}
