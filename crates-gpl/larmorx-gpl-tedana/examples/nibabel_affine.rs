// SPDX-License-Identifier: GPL-3.0-only
// Part of larmorx-gpl (GNU GPL version 3; see crates-gpl/LICENSE). Copyright 2026 Karel Lopez
// Vilaret.
//! Prints the affine the replica reads from each image named (`data::load_header`: nibabel's
//! `img.affine`, the qform in x87 extended precision), one line per file: the 16 values' bits
//! in hexadecimal, or `ERROR <message>`; for
//! `validation/investigations/t2smap/replica/check_affine.py`, which compares them with
//! nibabel's own.
use larmorx_gpl_tedana::data;

fn main() {
    for path in std::env::args().skip(1) {
        match data::load_header(&path) {
            Ok(h) => {
                let words: Vec<String> = h
                    .affine
                    .rows()
                    .iter()
                    .flat_map(|r| r.iter().map(|v| format!("{:016x}", v.to_bits())))
                    .collect();
                println!("{}", words.join(" "));
            }
            Err(e) => println!("ERROR {}", e.message),
        }
    }
}
