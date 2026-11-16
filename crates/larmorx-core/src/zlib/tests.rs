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

/// The digest of the 96 recipes of `cases.rs` (every call's status and counts, and the bytes),
/// recorded from zlib 1.2.11 itself (Ubuntu 22.04's `libz.so.1`: the probe's `testset`), and
/// from the port on Linux x64; every platform must give it.
const RECORDED: u64 = 0x0029_45f5_2e85_d35d;

#[test]
fn recipes_give_zlibs_bytes() {
    let mut all = Vec::new();
    for i in 0..cases::N {
        all.extend_from_slice(&cases::port_digest(i).to_le_bytes());
    }
    let d = cases::digest(&[], &all);
    assert_eq!(d, RECORDED, "digest {d:#018x}");
}

fn bytes(n: usize, seed: u32) -> Vec<u8> {
    (0..n as u32)
        .map(|i| ((i.wrapping_mul(seed) >> 3) % 7) as u8 + b'a')
        .collect()
}

/// FS-809: deflatePrime while compressed bytes wait behind some handed out: noted in both
/// modes; with `fix` the bits go after the waiting bytes, with `reproduce` where zlib puts them.
#[test]
fn prime_with_waiting_bytes_is_noted() {
    use crate::defects::{FS_809, UpstreamDefects};
    let data = bytes(5000, 2_654_435_761);
    let mut outs = Vec::new();
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut d = Deflate::init2(6, Z_DEFLATED, -15, 8, 0)
            .unwrap()
            .with_upstream_defects(mode);
        let mut out = vec![0u8; 3];
        let (r, _, made) = d.deflate(&data, &mut out, Z_SYNC_FLUSH);
        assert_eq!((r, made), (Z_OK, 3));
        assert_eq!(d.prime(12, 0x5a5), Z_OK);
        assert!(d.notices.contains(&FS_809), "{mode:?}");
        let mut rest = vec![0u8; 20_000];
        let (r, _, made) = d.deflate(&[], &mut rest, Z_FINISH);
        assert_eq!(r, Z_STREAM_END);
        outs.push(rest[..made].to_vec());
    }
    assert_ne!(outs[0], outs[1]);
}

/// FS-810: bits that would not fit in the pending buffer are refused (zlib writes past it).
#[test]
fn prime_past_the_buffer_is_refused() {
    use crate::defects::{FS_810, UpstreamDefects};
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut d = Deflate::init2(0, Z_DEFLATED, -9, 1, 0)
            .unwrap()
            .with_upstream_defects(mode);
        let data = bytes(600, 40_503);
        let mut out = vec![0u8; 6];
        let _ = d.deflate(&data, &mut out, Z_SYNC_FLUSH);
        let mut refused = false;
        for _ in 0..400 {
            if d.prime(16, 0x1234) == Z_BUF_ERROR {
                refused = true;
                break;
            }
        }
        assert!(refused && d.notices.contains(&FS_810), "{mode:?}");
    }
}

/// FS-811: a switch from storing to compressing with more bytes to hash than lie before the
/// position after the window's slide: zlib crashes; larmorx hashes the bytes there are.
#[test]
fn stored_window_slide_then_compressing_is_limited() {
    use crate::defects::FS_811;
    let mut d = Deflate::init2(0, Z_DEFLATED, -9, 8, 0).unwrap();
    let mut big = vec![0u8; 100_000];
    let (r, used, _) = d.deflate(&bytes(600, 7), &mut big, Z_NO_FLUSH);
    assert_eq!((r, used), (Z_OK, 600));
    let mut small = [0u8; 10];
    let (_, used, _) = d.deflate(&bytes(500, 11), &mut small, Z_NO_FLUSH);
    assert_eq!(used, 500);
    let (r, _, _) = d.params(1, 0, &[], &mut big);
    assert_eq!(r, Z_OK);
    let (r, used, _) = d.deflate(&bytes(100, 13), &mut big, Z_NO_FLUSH);
    assert_eq!((r, used), (Z_OK, 100));
    assert!(d.notices.contains(&FS_811));
}

/// FS-812: deflateGetDictionary past the window's end gives zeros there.
#[test]
fn dictionary_past_the_window_is_zeros() {
    use crate::defects::FS_812;
    let mut d = Deflate::init2(6, Z_DEFLATED, -9, 8, 0).unwrap();
    let mut big = vec![0u8; 100_000];
    let _ = d.deflate(&bytes(300, 17), &mut big, Z_NO_FLUSH);
    assert_eq!(d.reset_keep(), Z_OK);
    let (r, _, _) = d.params(0, 0, &[], &mut big);
    assert_eq!(r, Z_OK);
    let mut small = [0u8; 10];
    let _ = d.deflate(&bytes(1000, 19), &mut small, Z_NO_FLUSH);
    let dict = d.get_dictionary();
    assert_eq!(dict.len(), 512);
    assert!(d.notices.contains(&FS_812));
}

/// zlib's own decompressor on streams of every level and window, called with small slices of
/// input and output: the same bytes as the input, `Z_STREAM_END` at the end, the check value.
#[test]
fn inflate_round_trips_in_small_steps() {
    for (k, &(level, wbits)) in [(6, 15), (1, 9), (9, 12), (0, 15), (6, -15), (3, -9)]
        .iter()
        .enumerate()
    {
        let data: Vec<u8> = (0..40_000u32)
            .map(|i| (((i * 7 + k as u32) % 251) ^ (i >> 5)) as u8)
            .collect();
        let mut d = Deflate::init2(level, Z_DEFLATED, wbits, 8, Z_DEFAULT_STRATEGY).unwrap();
        let mut z = vec![0u8; compress_bound(data.len() as u64) as usize + 64];
        let (r, _, n) = d.deflate(&data, &mut z, Z_FINISH);
        assert_eq!(r, Z_STREAM_END);
        z.truncate(n);
        let mut inf = Inflate::init2(wbits).unwrap();
        let mut out = vec![0u8; data.len()];
        let (mut i, mut o) = (0usize, 0usize);
        let mut step = 1usize;
        loop {
            let ie = (i + step).min(z.len());
            let oe = (o + step * 3 / 2 + 1).min(out.len());
            let (r, used, wrote) = inf.inflate(&z[i..ie], &mut out[o..oe], Z_PARTIAL_FLUSH);
            i += used;
            o += wrote;
            step = step % 700 + 37;
            if r == Z_STREAM_END {
                break;
            }
            assert!(r == Z_OK || r == Z_BUF_ERROR, "{r}");
        }
        assert_eq!(o, data.len());
        assert_eq!(out, data);
        assert_eq!(i, z.len());
        if wbits > 0 {
            assert_eq!(inf.adler, adler32(1, Some(&data)));
        }
    }
}

/// The messages of damaged streams and `inflateSync`'s search for a flush point.
#[test]
fn inflate_messages_and_sync() {
    let mut inf = Inflate::init().unwrap();
    let mut out = [0u8; 16];
    assert_eq!(
        inf.inflate(&[0x78, 0x9d], &mut out, Z_NO_FLUSH).0,
        Z_DATA_ERROR
    );
    assert_eq!(inf.msg, Some("incorrect header check"));
    let mut inf = Inflate::init().unwrap();
    // A zlib header, then a block of type 3.
    assert_eq!(
        inf.inflate(&[0x78, 0x9c, 0x07], &mut out, Z_NO_FLUSH).0,
        Z_DATA_ERROR
    );
    assert_eq!(inf.msg, Some("invalid block type"));
    // inflateSync finds 00 00 ff ff and resets (msg cleared), ready for a new block.
    let (r, used) = inf.sync(&[1, 0, 0, 0xff, 0xff, 9]);
    assert_eq!((r, used), (Z_OK, 5));
    assert_eq!(inf.msg, None);
    // No input and no progress: Z_BUF_ERROR, the message unchanged.
    let mut inf = Inflate::init().unwrap();
    assert_eq!(inf.inflate(&[], &mut out, Z_PARTIAL_FLUSH).0, Z_BUF_ERROR);
    assert_eq!(inf.msg, None);
}
