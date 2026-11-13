// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `cscan::Scanf` against glibc 2.35's `sscanf` on track E's 9,008 calls at the end of the input
//! (ignored: the table is made on a Linux machine by
//! `validation/probes/fs-maths/scanf_errno_cases.py <out>`, then
//! `ESCANF=<out> cargo test -p larmorx-freesurfer-core --test scanf_errno_e -- --ignored`).
//! 2026-11-13, track A relay 17: the return value and `errno` identical on all 9,008.

use larmorx_freesurfer_core::cscan::{Bytes, Scanf};

fn unhex(h: &str) -> Vec<u8> {
    if h == "-" {
        return Vec::new();
    }
    (0..h.len() / 2)
        .map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

#[test]
#[ignore = "needs the table made by validation/probes/fs-maths/scanf_errno_cases.py (ESCANF)"]
fn track_e_calls_as_glibc() {
    let path = std::env::var("ESCANF").expect("ESCANF: the table's path");
    let table = std::fs::read_to_string(path).expect("the table");
    let mut bad = Vec::new();
    let mut n = 0;
    for line in table.lines() {
        let t: Vec<&str> = line.split(' ').collect();
        let (inp, fmt) = (unhex(t[0]), unhex(t[1]));
        let before: i32 = t[2].parse().unwrap();
        let want = (t[3].parse::<i32>().unwrap(), t[4].parse::<i32>().unwrap());
        let mut b = Bytes::new(&inp);
        let mut sc = Scanf::new(&mut b);
        let mut e = before;
        let mut i = 0;
        while i < fmt.len() {
            if fmt[i] == b'%' {
                match fmt[i + 1] {
                    b'f' => {
                        sc.f(&mut e, true);
                    }
                    b'd' => {
                        sc.d(&mut e, true);
                    }
                    _ => {
                        sc.s(&mut e, usize::MAX, true);
                    }
                }
                i += 2;
            } else {
                sc.text(&mut e, &fmt[i..i + 1]);
                i += 1;
            }
        }
        let got = (sc.finish(&mut e), e);
        if got != want {
            bad.push(format!("{line}: got {got:?}"));
        }
        n += 1;
    }
    assert_eq!(n, 9008);
    assert!(bad.is_empty(), "{} differ:\n{}", bad.len(), bad.join("\n"));
}
