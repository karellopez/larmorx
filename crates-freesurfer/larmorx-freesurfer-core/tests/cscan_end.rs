// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `cscan::Scanf` against glibc 2.35's `sscanf` at and near the end of the input: the 1,060
//! `sscanf` lines of `validation/investigations/glibc-stdio-probe/scanf_end.c`'s output (kept in
//! `tests/data/glibc_scanf_end.txt`: 20 formats on 53 inputs, the return value and `errno`, 5
//! before each call). The `fscanf` lines are checked on fs.io's `CFile` (its unit tests).

use larmorx_freesurfer_core::cscan::{Bytes, Scanf};

#[path = "scanf_end/formats.rs"]
mod formats;
use formats::format;

fn unhex(h: &str) -> Vec<u8> {
    if h == "-" {
        return Vec::new();
    }
    (0..h.len() / 2)
        .map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

#[test]
fn sscanf_at_the_end_of_the_input_as_glibc() {
    let table = include_str!("data/glibc_scanf_end.txt");
    let mut n = 0;
    let mut bad = Vec::new();
    for line in table.lines() {
        let t: Vec<&str> = line.split(' ').collect();
        if t[0] != "s" {
            continue;
        }
        let id: u32 = t[1].parse().unwrap();
        let input = unhex(t[2]);
        let want = (t[3].parse::<i32>().unwrap(), t[4].parse::<i32>().unwrap());
        let mut b = Bytes::new(&input);
        let mut sc = Scanf::new(&mut b);
        let mut e = 5;
        format(id, &mut sc, &mut e);
        let got = (sc.finish(&mut e), e);
        if got != want {
            bad.push(format!("{line}: got {got:?}"));
        }
        n += 1;
    }
    assert_eq!(n, 1060);
    assert!(bad.is_empty(), "{} differ:\n{}", bad.len(), bad.join("\n"));
}
