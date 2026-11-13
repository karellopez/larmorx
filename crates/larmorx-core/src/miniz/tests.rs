// SPDX-License-Identifier: Apache-2.0
use super::streams::{self, Bits, Rng, Tok};
use super::*;
use crate::defects::UpstreamDefects::{Fix, Reproduce};

/// A random valid stream of 1 to 4 blocks: (raw deflate, the bytes it holds).
pub(super) fn random_stream(rng: &mut Rng, max_len: usize) -> (Vec<u8>, Vec<u8>) {
    let mut b = Bits::default();
    let mut data = Vec::new();
    let blocks = 1 + rng.below(4);
    for k in 0..blocks {
        let last = k + 1 == blocks;
        let n = rng.below(max_len as u64 + 1) as usize;
        let alphabet = [2, 4, 16, 256][rng.below(4) as usize];
        match rng.below(3) {
            0 => {
                let bytes: Vec<u8> = (0..n.min(65535)).map(|_| rng.below(256) as u8).collect();
                streams::stored(&mut b, &bytes, last);
                data.extend_from_slice(&bytes);
            }
            1 => {
                let (toks, out) = streams::random_tokens(rng, &data, n, alphabet, 32768);
                streams::fixed(&mut b, &toks, last);
                data.extend_from_slice(&out);
            }
            _ => {
                let (toks, out) = streams::random_tokens(rng, &data, n, alphabet, 32768);
                let (ll, dl) = streams::lengths_for(rng, &toks);
                let rle = rng.below(2) == 1;
                streams::dynamic(&mut b, rng, &toks, &ll, &dl, rle, last);
                data.extend_from_slice(&out);
            }
        }
    }
    (b.finish(), data)
}

/// Inflates `input` as `how` feeds it: 0 one MZ_FINISH call, 1 MZ_SYNC_FLUSH with the whole
/// input and output in pieces of `piece`, 2 the input byte by byte (MZ_NO_FLUSH) then
/// MZ_FINISH. Returns (last code, output, total_in, adler, notices' IDs).
fn run(
    input: &[u8],
    zlib: bool,
    how: u32,
    room: usize,
    piece: usize,
    mode: UpstreamDefects,
) -> (i32, Vec<u8>, u64, u64, Vec<&'static str>) {
    let mut s = InflateStream::new(mode);
    assert_eq!(s.init2(if zlib { 15 } else { -15 }), MZ_OK);
    let mut out = vec![0u8; room];
    let mut op = 0;
    let mut ip = 0;
    let mut ret;
    match how {
        0 => {
            let r = s.inflate(input, &mut out, MZ_FINISH);
            op = r.produced;
            ret = r.ret;
        }
        1 => loop {
            let end = (op + piece).min(room);
            let r = s.inflate(&input[ip..], &mut out[op..end], MZ_SYNC_FLUSH);
            ip += r.consumed;
            op += r.produced;
            ret = r.ret;
            if ret != MZ_OK || (r.consumed == 0 && r.produced == 0) {
                break;
            }
        },
        _ => loop {
            let (inp, flush) = if ip < input.len() {
                (&input[ip..ip + 1], MZ_NO_FLUSH)
            } else {
                (&input[ip..], MZ_FINISH)
            };
            let r = s.inflate(inp, &mut out[op..], flush);
            ip += r.consumed;
            op += r.produced;
            ret = r.ret;
            if ret != MZ_OK || flush == MZ_FINISH || (r.consumed == 0 && r.produced == 0) {
                break;
            }
        },
    }
    out.truncate(op);
    let ids = s.notices.as_slice().iter().map(|n| n.defect.id).collect();
    (ret, out, s.total_in, s.adler, ids)
}

fn adler(data: &[u8]) -> u64 {
    adler32_update(1, data) as u64
}

#[test]
fn valid_streams_every_feeding() {
    let mut rng = Rng(7);
    for case in 0..300 {
        let (raw, data) = random_stream(&mut rng, if case % 10 == 0 { 40_000 } else { 600 });
        for zlib in [false, true] {
            let input = if zlib {
                streams::zlib(&raw, &data)
            } else {
                raw.clone()
            };
            for (how, piece) in [(0, 0), (1, 1), (1, 7), (1, 70_000), (2, 0)] {
                let (ret, out, total_in, ad, ids) =
                    run(&input, zlib, how, data.len() + 10, piece, Fix);
                assert_eq!(
                    ret, MZ_STREAM_END,
                    "case {case} zlib {zlib} how {how}/{piece}"
                );
                assert_eq!(out, data, "case {case} zlib {zlib} how {how}/{piece}");
                assert!(ids.is_empty());
                assert_eq!(ad, adler(&data));
                if zlib {
                    assert_eq!(total_in, input.len() as u64);
                }
            }
        }
    }
}

#[test]
fn uncompress_and_api() {
    let mut rng = Rng(11);
    let (raw, data) = random_stream(&mut rng, 3000);
    let z = streams::zlib(&raw, &data);
    let mut dest = vec![0u8; data.len()];
    let mut len = data.len() as u64;
    let (ret, notes) = uncompress(&mut dest, &mut len, &z, Fix);
    assert_eq!((ret, len), (MZ_OK, data.len() as u64));
    assert_eq!(dest, data);
    assert!(notes.is_empty());
    // too little room: MZ_BUF_ERROR; the adler-32 wrong: MZ_DATA_ERROR
    let mut small = vec![0u8; data.len() - 1];
    let mut len = small.len() as u64;
    assert_eq!(uncompress(&mut small, &mut len, &z, Fix).0, MZ_BUF_ERROR);
    let mut bad = z.clone();
    *bad.last_mut().unwrap() ^= 1;
    let mut len = dest.len() as u64;
    assert_eq!(uncompress(&mut dest, &mut len, &bad, Fix).0, MZ_DATA_ERROR);
    let mut s = InflateStream::new(Fix);
    assert_eq!(s.inflate(&z, &mut dest, MZ_FINISH).ret, MZ_STREAM_ERROR);
    assert_eq!(s.init2(14), MZ_PARAM_ERROR);
    assert_eq!(s.init(), MZ_OK);
    assert_eq!(s.inflate(&z, &mut dest, MZ_FULL_FLUSH).ret, MZ_STREAM_ERROR);
    assert_eq!(s.inflate(&z, &mut dest, MZ_BLOCK).ret, MZ_STREAM_ERROR);
    assert_eq!(
        s.inflate(&z[..0], &mut dest, MZ_SYNC_FLUSH).ret,
        MZ_BUF_ERROR
    );
    assert_eq!(s.inflate(&z, &mut dest, MZ_FINISH).ret, MZ_STREAM_END);
    assert_eq!(s.inflate(&z, &mut dest, MZ_SYNC_FLUSH).ret, MZ_STREAM_ERROR);
    assert_eq!(s.end(), MZ_OK);
    assert_eq!(error_text(MZ_BUF_ERROR), Some("buf error"));
    assert_eq!(error_text(MZ_PARAM_ERROR), Some("parameter error"));
    assert_eq!(error_text(7), None);
    // a zlib header asking for a 64 KiB window, or a preset dictionary: refused
    let mut z2 = z.clone();
    z2[0] = 0x88;
    z2[1] = 0x1c;
    assert_eq!(run(&z2, true, 1, 8000, 9000, Fix).0, MZ_DATA_ERROR);
    let mut z3 = z.clone();
    z3[1] = 0x20; // FDICT, (0x78 * 256 + 0x20) % 31 == 0
    assert_eq!(run(&z3, true, 1, 8000, 9000, Fix).0, MZ_DATA_ERROR);
}

#[test]
fn fs281_truncated_with_no_more_input() {
    let mut b = Bits::default();
    streams::fixed(
        &mut b,
        &[Tok::Lit(b'a'), Tok::Lit(b'b'), Tok::Match(100, 2)],
        true,
    );
    let raw = b.finish();
    let cut = &raw[..raw.len() - 1];
    let (ret, _, _, _, ids) = run(cut, false, 0, 200, 0, Fix);
    assert_eq!((ret, ids), (MZ_DATA_ERROR, vec!["FS-281"]));
    let (_, out, _, _, ids) = run(cut, false, 0, 200, 0, Reproduce);
    assert_eq!(ids, vec!["FS-281"]);
    assert!(out.starts_with(b"ab"));
    // with more input to come, the same cut only waits for it (a call with no input then says
    // it cannot go on: MZ_BUF_ERROR)
    let (ret, out, _, _, ids) = run(cut, false, 1, 200, 200, Fix);
    assert_eq!(ret, MZ_BUF_ERROR);
    assert!(ids.is_empty() && out.starts_with(b"ab"));
}

#[test]
fn fs282_undefined_symbols() {
    let lit = |s: &[u8]| s.iter().map(|&x| Tok::Lit(x)).collect::<Vec<_>>();
    let mut toks = lit(b"abc");
    toks.push(Tok::Raw(286, 0, Some((0, 0))));
    toks.extend(lit(b"z"));
    let mut b = Bits::default();
    streams::fixed(&mut b, &toks, true);
    let raw = b.finish();
    for how in [0, 1] {
        let (ret, _, _, _, ids) = run(&raw, false, how, 100, 100, Fix);
        assert_eq!((ret, ids), (MZ_DATA_ERROR, vec!["FS-282"]));
        let (ret, out, _, _, ids) = run(&raw, false, how, 100, 100, Reproduce);
        assert_eq!(
            (ret, &out[..], ids),
            (MZ_STREAM_END, &b"abccccz"[..], vec!["FS-282"])
        );
    }
    // a distance symbol of 30: the byte under the cursor (zero: never written)
    let mut toks = lit(b"abc");
    toks.push(Tok::Raw(257, 0, Some((30, 0))));
    let mut b = Bits::default();
    streams::fixed(&mut b, &toks, true);
    let raw = b.finish();
    let (ret, out, _, _, ids) = run(&raw, false, 0, 100, 100, Reproduce);
    assert_eq!(
        (ret, &out[..], ids),
        (MZ_STREAM_END, &b"abc\0\0\0"[..], vec!["FS-282"])
    );
    let (ret, out, _, _, ids) = run(&raw, false, 1, 100, 100, Reproduce);
    assert_eq!((ret, &out[..]), (MZ_STREAM_END, &b"abc\0\0\0"[..]));
    assert_eq!(ids, vec!["FS-282", "FS-284"]);
    // a length of 0 within 3 bytes of the output's end: never reproduced
    let mut toks = lit(b"abc");
    toks.push(Tok::Raw(287, 0, Some((0, 0))));
    let mut b = Bits::default();
    streams::fixed(&mut b, &toks, true);
    let (ret, _, _, _, ids) = run(&b.finish(), false, 0, 4, 0, Reproduce);
    assert_eq!((ret, ids), (MZ_DATA_ERROR, vec!["FS-282"]));
}

#[test]
fn fs283_pattern_without_a_code() {
    let mut rng = Rng(3);
    let mut toks: Vec<Tok> = b"abcd".iter().map(|&x| Tok::Lit(x)).collect();
    toks.push(Tok::Raw(257, 0, None));
    toks.push(Tok::Bits(1, 1));
    let (ll, _) = streams::lengths_for(&mut rng, &toks);
    let mut b = Bits::default();
    streams::dynamic(&mut b, &mut rng, &toks, &ll, &[1], false, true);
    let raw = b.finish();
    let (ret, _, _, _, ids) = run(&raw, false, 1, 100, 100, Fix);
    assert_eq!((ret, ids), (MZ_DATA_ERROR, vec!["FS-283"]));
    let (_, out, _, _, ids) = run(&raw, false, 1, 100, 100, Reproduce);
    assert_eq!(ids[0], "FS-283");
    assert!(out.starts_with(b"abcdddd"));
    // the single code's own pattern is a distance of 1, as deflate allows
    let mut toks: Vec<Tok> = b"ab".iter().map(|&x| Tok::Lit(x)).collect();
    toks.push(Tok::Match(5, 1));
    let (ll, _) = streams::lengths_for(&mut rng, &toks);
    let mut b = Bits::default();
    streams::dynamic(&mut b, &mut rng, &toks, &ll, &[1], true, true);
    let (ret, out, _, _, ids) = run(&b.finish(), false, 1, 100, 100, Fix);
    assert_eq!((ret, &out[..]), (MZ_STREAM_END, &b"abbbbbb"[..]));
    assert!(ids.is_empty());
}

#[test]
fn fs284_farther_back_than_the_output() {
    let mut b = Bits::default();
    streams::fixed(
        &mut b,
        &[Tok::Lit(b'a'), Tok::Match(3, 5), Tok::Lit(b'q')],
        true,
    );
    let raw = b.finish();
    let (ret, _, _, _, ids) = run(&raw, false, 1, 100, 100, Fix);
    assert_eq!((ret, ids), (MZ_DATA_ERROR, vec!["FS-284"]));
    let (ret, out, _, _, ids) = run(&raw, false, 1, 100, 100, Reproduce);
    assert_eq!(
        (ret, &out[..], ids),
        (MZ_STREAM_END, &b"a\0\0\0q"[..], vec!["FS-284"])
    );
    // MZ_FINISH at once: miniz's own check refuses it, in both modes, without a notice
    for mode in [Fix, Reproduce] {
        let (ret, _, _, _, ids) = run(&raw, false, 0, 100, 100, mode);
        assert_eq!((ret, ids.len()), (MZ_DATA_ERROR, 0));
    }
}

/// The observables of many streams (valid, cut, flipped, random) fed four ways in both modes,
/// hashed (FNV-1a 64): recorded on Linux x86-64 after the probe `validation/probes/fs-misc`
/// found them identical to miniz's.
#[test]
fn digest_of_generated_streams() {
    let mut rng = Rng(2026);
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let mut eat = |h: &mut u64, bytes: &[u8]| {
        for &x in bytes {
            *h = (*h ^ x as u64).wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    for case in 0..400 {
        let (raw, data) = random_stream(&mut rng, 900);
        let zlib = case % 2 == 1;
        let mut input = if zlib {
            streams::zlib(&raw, &data)
        } else {
            raw
        };
        match case % 5 {
            1 => input.truncate(rng.below(input.len() as u64 + 1) as usize),
            2 => {
                for _ in 0..1 + rng.below(4) {
                    let i = rng.below(input.len() as u64) as usize;
                    input[i] ^= 1 << rng.below(8);
                }
            }
            3 => input = (0..rng.below(300)).map(|_| rng.below(256) as u8).collect(),
            _ => {}
        }
        for (how, piece) in [(0, 0), (1, 1), (1, 300), (2, 0)] {
            for mode in [Fix, Reproduce] {
                let room = data.len() + rng.below(3) as usize;
                let (ret, out, total_in, ad, ids) = run(&input, zlib, how, room, piece, mode);
                eat(&mut h, &ret.to_le_bytes());
                eat(&mut h, &(out.len() as u64).to_le_bytes());
                eat(&mut h, &out);
                eat(&mut h, &total_in.to_le_bytes());
                eat(&mut h, &ad.to_le_bytes());
                eat(&mut h, ids.join(",").as_bytes());
            }
        }
    }
    assert_eq!(h, DIGEST, "{h:#018x}");
}

const DIGEST: u64 = 0x01bd_8f8d_e15f_f052;
