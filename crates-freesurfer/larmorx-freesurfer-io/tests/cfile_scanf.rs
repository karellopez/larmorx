// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `cscan::Scanf` on fs.io's `CFile` against glibc 2.35's `fscanf` at and near the end of a file:
//! the 1,060 `fscanf` lines of `validation/investigations/glibc-stdio-probe/scanf_end.c`'s output
//! (fs.core's `tests/data/glibc_scanf_end.txt`): the return value, `errno` (5 before each call),
//! the position after the call and the end-of-file flag.

use larmorx_freesurfer_core::cscan::Scanf;
use larmorx_freesurfer_io::cfile::CFile;

#[path = "../../larmorx-freesurfer-core/tests/scanf_end/formats.rs"]
mod formats;

fn unhex(h: &str) -> Vec<u8> {
    if h == "-" {
        return Vec::new();
    }
    (0..h.len() / 2)
        .map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

#[test]
fn fscanf_at_the_end_of_a_file_as_glibc() {
    let table = include_str!("../../larmorx-freesurfer-core/tests/data/glibc_scanf_end.txt");
    let mut n = 0;
    let mut bad = Vec::new();
    for line in table.lines() {
        let t: Vec<&str> = line.split(' ').collect();
        if t[0] != "f" {
            continue;
        }
        let id: u32 = t[1].parse().unwrap();
        let want: Vec<i64> = t[3..7].iter().map(|x| x.parse().unwrap()).collect();
        let mut fp = CFile::from_bytes(unhex(t[2]));
        let mut e = 5;
        let r = {
            let mut sc = Scanf::new(&mut fp);
            formats::format(id, &mut sc, &mut e);
            sc.finish(&mut e)
        };
        let got = vec![i64::from(r), i64::from(e), fp.tell(), i64::from(fp.feof())];
        if got != want {
            bad.push(format!("{line}: got {got:?}"));
        }
        n += 1;
    }
    assert_eq!(n, 1060);
    assert!(bad.is_empty(), "{} differ:\n{}", bad.len(), bad.join("\n"));
}
