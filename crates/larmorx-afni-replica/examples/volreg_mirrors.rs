// SPDX-License-Identifier: Apache-2.0
//! The rule-D measurement of the blur's row-end mirrors in 3dvolreg (development tool,
//! `docs/findings/afni-volreg.md`): runs the replica twice on a NIfTI series with fMRIPrep's
//! call (`-Fourier -twopass -zpad 4`, `-maxdisp`; or the default heptic run with `heptic`),
//! once with AFNI's mirrors (`BMIRROR` along x and y, `RMIRROR` along z, one point for
//! `fir_blury` when nx >= 512) and once mirrored about the end points on every axis, and
//! prints how far the motion parameters, the maximum displacements and the registered volumes
//! move.
//!
//! ```text
//! cargo run --release -p larmorx-afni-replica --example volreg_mirrors -- <series.nii.gz> [heptic]
//! ```

use larmorx_afni_replica::dataset;
use larmorx_afni_replica::environ::Settings;
use larmorx_afni_replica::shift::Method;
use larmorx_afni_replica::volreg::{self, Base, Edging, VolregParams, VolregResult};
use larmorx_core::defects::{Notices, UpstreamDefects};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let heptic = a.get(1).map(String::as_str) == Some("heptic");
    let mode = UpstreamDefects::Reproduce;
    let settings = Settings::empty();
    let mut msgs = Vec::new();
    let mut notices = Notices::new(mode);
    let ds = dataset::read(&a[0], 0, &settings, &mut msgs, &mut notices).expect("a dataset");
    let n = ds.nxyz;
    let mm = ((0.1 * n[0].min(n[1]).min(n[2]) as f64 + 0.499) as i32).max(1);
    let run = |whole: bool| -> VolregResult {
        let p = VolregParams {
            base: Base::Index(0),
            weight: None,
            wtinp: false,
            resam: if heptic {
                Method::Heptic
            } else {
                Method::Fourier
            },
            final_mode: None,
            clipit: true,
            zpad: 4,
            twopass: !heptic,
            twoblur: 2.0,
            sinit: true,
            coarse_del: if heptic { 10 } else { 10.min(mm) },
            coarse_num: 2,
            coarse_rot: true,
            edging: Edging::Percent(5.0),
            maxite: 23,
            dxy: 0.01,
            dph: 0.02,
            del: 0.70,
            maxdisp: true,
            verbose: 0,
            floatize: true,
            firfac: 0.0,
            firold: false,
            matvec: None,
            whole_point_mirrors: whole,
            mode,
        };
        let mut log = Vec::new();
        let mut nt = Notices::new(mode);
        volreg::volreg(&ds, &p, false, &mut log, &mut nt).expect("a registration")
    };
    let afni = run(false);
    let whole = run(true);
    let maxd = |x: &[f32], y: &[f32]| {
        x.iter()
            .zip(y)
            .map(|(a, b)| f64::from((a - b).abs()))
            .fold(0.0, f64::max)
    };
    let rot = maxd(&afni.roll, &whole.roll)
        .max(maxd(&afni.pitch, &whole.pitch))
        .max(maxd(&afni.yaw, &whole.yaw));
    let sh = maxd(&afni.dx, &whole.dx)
        .max(maxd(&afni.dy, &whole.dy))
        .max(maxd(&afni.dz, &whole.dz));
    let md = match (&afni.maxdisp, &whole.maxdisp) {
        (Some(x), Some(y)) => maxd(&x.dmax, &y.dmax),
        _ => f64::NAN,
    };
    let vol = |r: &VolregResult| match &r.output.as_ref().expect("an output").data {
        dataset::BrickData::Float(v) => v.clone(),
        _ => Vec::new(),
    };
    let (va, vw) = (vol(&afni), vol(&whole));
    let top = va.iter().map(|x| f64::from(x.abs())).fold(0.0, f64::max);
    let ndiff = va.iter().zip(&vw).filter(|(a, b)| a != b).count();
    println!(
        "{}: {}x{}x{}x{} ({}): max |d| roll/pitch/yaw {rot:.3e} deg, shifts {sh:.3e} mm, \
         max displacement {md:.3e} mm; registered volumes: {ndiff} of {} voxels differ, max |d| \
         {:.3e} ({:.3e} of the largest value)",
        a[0],
        n[0],
        n[1],
        n[2],
        ds.nvals,
        if heptic {
            "-heptic"
        } else {
            "-Fourier -twopass -zpad 4"
        },
        va.len(),
        maxd(&va, &vw),
        maxd(&va, &vw) / top
    );
}
