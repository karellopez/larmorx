// SPDX-License-Identifier: LicenseRef-FSL
// Part of larmorx-nc (FSL Licence terms; see LICENSE). Copyright 2026 Karel Lopez Vilaret.
//! The counterpart of `validation/probes/fsl-flirt-costs/cost_probe.cc`: one of the core's cost
//! functions on a reference and a test volume, for the matrices of `$COST_PROBE_MATS` (16 numbers
//! per line, row by row), each printed as its bits and value, to compare with FSL's own code.
//!
//! `COST_PROBE_REF=<ref.nii> COST_PROBE_TEST=<test.nii> COST_PROBE_COST=labeldiff_smoothed
//! COST_PROBE_MATS=<file> cargo test -p larmorx-nc-flirt --test cost_probe -- --ignored --nocapture`

use larmorx_nc_core::arma::Mat;
use larmorx_nc_core::costfns;
use larmorx_nc_core::settings::Settings;
use larmorx_nc_core::volume::read_volume;

#[test]
#[ignore = "a probe: run with the COST_PROBE_* variables"]
fn cost_probe() {
    let var = |n: &str| std::env::var(n).unwrap_or_else(|_| panic!("{n} is not set"));
    let settings = Settings::new();
    let (r, _) = read_volume::<f32>(&var("COST_PROBE_REF"), &settings).unwrap();
    let (t, _) = read_volume::<f32>(&var("COST_PROBE_TEST"), &settings).unwrap();
    let smooth: f32 = std::env::var("COST_PROBE_SMOOTH")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0);
    let cost = var("COST_PROBE_COST");
    let text = std::fs::read_to_string(var("COST_PROBE_MATS")).unwrap();
    let nums: Vec<f64> = text
        .split_ascii_whitespace()
        .map(|w| w.parse().unwrap())
        .collect();
    for m in nums.chunks(16) {
        let aff = Mat::from_rows(4, 4, m);
        let v = match cost.as_str() {
            "labeldiff" => costfns::p_labeldiff(&r, &t, &aff),
            "labeldiff_smoothed" => costfns::p_labeldiff_weighted(&r, &t, None, &aff, smooth),
            "leastsq_smoothed" => {
                costfns::p_leastsquares_weighted(&r, &t, None, &aff, smooth, false)
            }
            other => panic!("unknown cost {other}"),
        };
        println!("{:08x} {v:e}", v.to_bits());
    }
}
