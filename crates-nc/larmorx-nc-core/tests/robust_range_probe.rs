// SPDX-License-Identifier: LicenseRef-FSL
//! FSL-187 in the shared robust range (`find_thresholds`, the robust limits of BET, FLIRT,
//! fslmaths' `-thrp`/`-thrP`/`-uthrp`/`-uthrP`/`-inm`... and fslstats' `-r`, `-e`, `-E`): on which
//! inputs does a value land more than 2^31 bins above a pass's histogram range (FSL counts it
//! in the first bin), and does it move the range? Rule D's measurement
//! (`validation/investigations/fsl-maths/README.md`, "The robust range's bins").
//!
//! `LARMORX_ROBUST_RANGE_INPUTS=<file of NIfTI paths, one per line> cargo test --release -p
//! larmorx-nc-core --test robust_range_probe -- --ignored --nocapture`: every image as float
//! (BET's and FLIRT's reading; a 4D image's first volume, as `read_volume`), the range FSL
//! computes and the corrected one; one line per image (`overflow`, `differs`) and the totals.
//! Without the variable only the synthetic triggers run.

use larmorx_nc_core::settings::Settings;
use larmorx_nc_core::volops::robust_range;
use larmorx_nc_core::volume::{Volume, read_volume};

/// The smallest synthetic triggers found: `n` values uniform on [0, 1) (a fixed ramp) and one
/// value `far` above them, so that the passes narrow the range to the ramp's and the far value
/// leaves `int`'s range.
fn ramp_with_outlier(n: usize, far: f32) -> Volume<f32> {
    let mut v = Volume::<f32>::new(n as i64 + 1, 1, 1, 1);
    for k in 0..n {
        v.data[k] = k as f32 / n as f32;
    }
    v.data[n] = far;
    v
}

#[test]
#[ignore = "a measurement: LARMORX_ROBUST_RANGE_INPUTS=<list of NIfTI paths> ... -- --ignored"]
fn robust_range_on_inputs() {
    // the synthetic triggers
    for (n, far) in [
        (1000usize, 1e12f32),
        (100, 1e12),
        (50, 1e12),
        (1000, 1e9),
        (1000, 1e7),
    ] {
        let v = ramp_with_outlier(n, far);
        let r = robust_range(&v, None).unwrap();
        println!(
            "synthetic ramp n={n} far={far:e}: FSL {:?}, corrected {:?}, overflow {}, differs {}",
            r.fsl,
            r.fixed,
            r.overflow,
            r.differs()
        );
    }
    // the smallest ramp whose range moves (one far value), and the nearest far value that moves
    // a 1000-value ramp's
    let smallest = (2..400usize).find(|&n| {
        robust_range(&ramp_with_outlier(n, 1e12), None)
            .unwrap()
            .differs()
    });
    println!("smallest ramp with one value at 1e12 whose range moves: {smallest:?} values + 1");
    let nearest = (0..60).map(|k| 10f32.powf(k as f32 / 4.0)).find(|&far| {
        robust_range(&ramp_with_outlier(1000, far), None)
            .unwrap()
            .differs()
    });
    println!(
        "nearest far value (steps of 10^0.25) that moves a 1000-value ramp's range: {nearest:?}"
    );
    let Ok(list) = std::env::var("LARMORX_ROBUST_RANGE_INPUTS") else {
        return;
    };
    let settings = Settings::new();
    let (mut n, mut overflow, mut differ) = (0, 0, 0);
    for path in std::fs::read_to_string(list).unwrap().lines() {
        let path = path.trim();
        if path.is_empty() {
            continue;
        }
        let v = match read_volume::<f32>(path, &settings) {
            Ok((v, _)) => v,
            Err(e) => {
                println!("{path}: unreadable ({})", e.what);
                continue;
            }
        };
        let r = robust_range(&v, None).unwrap();
        let over = r.overflow;
        n += 1;
        overflow += usize::from(over);
        differ += usize::from(r.differs());
        if over {
            println!(
                "{path}: overflow, FSL {:?}, corrected {:?}, differs {}",
                r.fsl,
                r.fixed,
                r.differs()
            );
        }
    }
    println!("{n} images: {overflow} reach the overflow, {differ} change their robust range");
}
