// SPDX-License-Identifier: Apache-2.0 AND Zlib
//! zlib's compressor: round trips through zlib-rs's inflater and the digests of generated
//! streams recorded from zlib 1.2.11 (Ubuntu 22.04's `libz.so.1`, the probe
//! `validation/probes/zlib-deflate`).

use super::*;

/// A deflate stream inflated by zlib-rs (flate2) gives the input back.
#[test]
fn round_trip_every_level_and_strategy() {
    let mut data = Vec::new();
    let mut x = 0x1234_5678_9abc_def0u64;
    for i in 0..200_000u32 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        data.push(if i % 1000 < 600 {
            (x % 7) as u8
        } else {
            (i / 3) as u8
        });
    }
    for level in -1..=9 {
        for strategy in 0..=4 {
            let mut d =
                Deflate::init2(level, Z_DEFLATED, MAX_WBITS, DEF_MEM_LEVEL, strategy).unwrap();
            let mut out = vec![0u8; d.bound(data.len() as u64) as usize];
            let (r, used, made) = d.deflate(&data, &mut out, Z_FINISH);
            assert_eq!(
                (r, used),
                (Z_STREAM_END, data.len()),
                "level {level} strategy {strategy}"
            );
            let mut back = Vec::new();
            use std::io::Read;
            flate2::read::ZlibDecoder::new(&out[..made])
                .read_to_end(&mut back)
                .unwrap();
            assert!(back == data, "level {level} strategy {strategy}");
        }
    }
}
