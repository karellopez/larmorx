// SPDX-License-Identifier: Apache-2.0
//! Tests of the RANLUX389 clean room against the spec's reference numbers (`specs/ranlux.md`
//! §2.5, §8, §10), which come from black-box runs of FreeSurfer's own functions, and golden
//! checksums of its own outputs (checked on every platform).

use super::sha256::Sha256;
use super::*;

/// The SHA-256 of the first `count` values of `seed`'s stream, one decimal line each.
fn raw_hash(seed: u64, count: usize) -> String {
    let mut r = Ranlux::new(seed, UpstreamDefects::Fix);
    let mut h = Sha256::new();
    for _ in 0..count {
        h.update(format!("{}\n", r.next_u24()).as_bytes());
    }
    h.hex()
}

#[test]
fn seed_53_first_values() {
    let mut r = Ranlux::new(53, UpstreamDefects::Fix);
    let v: Vec<u32> = (0..28).map(|_| r.next_u24()).collect();
    assert_eq!(
        v[..12],
        [
            2221969, 1586980, 7231467, 13008083, 14055568, 7275409, 14826403, 7390714, 3054659,
            16338312, 6216067, 6833642
        ]
    );
    assert_eq!(v[24..28], [4618358, 3215875, 9571569, 7955534]);
    assert_eq!(
        raw_hash(53, 100_000),
        "b40812f1162500bca3938e3148b47358a9300d4c7be994cd7e03163e9115541d"
    );
}
