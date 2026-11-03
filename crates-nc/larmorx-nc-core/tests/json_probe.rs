// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret. Distributed under the FSL Licence (see LICENSE).
//! The replica's side of `validation/probes/nlohmann-to-chars/probe.cc`: for each line
//! `<bits> <text>` of the file `JSON_PROBE` names, the text `json::to_chars` gives the same
//! double; prints the number of lines that differ (and the first ones). Run with `--ignored`.

use larmorx_nc_core::json;

#[test]
#[ignore]
fn json_probe() {
    let path = std::env::var("JSON_PROBE").expect("JSON_PROBE");
    let text = std::fs::read_to_string(path).unwrap();
    let (mut n, mut bad) = (0usize, 0usize);
    for line in text.lines() {
        let Some((bits, want)) = line.split_once(' ') else {
            continue;
        };
        let v = f64::from_bits(u64::from_str_radix(bits, 16).unwrap());
        let got = json::to_chars(v);
        n += 1;
        if got != want {
            bad += 1;
            if bad <= 10 {
                eprintln!("{bits}: nlohmann {want}, replica {got}");
            }
        }
    }
    eprintln!("{n} lines, {bad} differ");
    assert!(n > 1_000_000);
    assert_eq!(bad, 0);
}
