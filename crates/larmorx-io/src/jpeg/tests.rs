// SPDX-License-Identifier: Apache-2.0
//! The JPEG port's unit tests: messages, the encoder's bytes, round trips, FS-813, and a digest
//! of many generated files decoded (recorded on Linux x86-64 after the differential probe found
//! them identical to IJG 6b's results; the same on every platform).

use super::cases::{self, Recipe};
use super::*;
use larmorx_core::cfmt::{Captured, Stream};
use larmorx_core::defects::{Notices, UpstreamDefects};

fn decode(data: &[u8], mode: UpstreamDefects) -> (Result<Header, Fatal>, Vec<u8>, String) {
    let mut cap = Captured::default();
    let mut rgb = Vec::new();
    let mut notices = Notices::new(mode);
    let opts = Options {
        upstream_defects: mode,
        max_alloc: None,
    };
    let r = decode_rgb(data, &mut cap, opts, &mut notices, &mut |row| {
        rgb.extend_from_slice(row)
    });
    (r, rgb, cap.text(Stream::Err))
}

struct Fnv(u64);
impl Fnv {
    fn add(&mut self, b: &[u8]) {
        for &x in b {
            self.0 ^= x as u64;
            self.0 = self.0.wrapping_mul(0x100_0000_01b3);
        }
    }
}

/// The bytes the probe compares for one decoded file: the status, standard error, the image.
pub(crate) fn outcome_bytes(data: &[u8], mode: UpstreamDefects) -> Vec<u8> {
    let (r, rgb, err) = decode(data, mode);
    let mut v = Vec::new();
    match r {
        Ok(h) => {
            v.extend_from_slice(format!("ok {} {}\n", h.image_width, h.image_height).as_bytes())
        }
        Err(_) => v.extend_from_slice(b"exit 1\n"),
    }
    v.extend_from_slice(err.as_bytes());
    v.extend_from_slice(b"--\n");
    v.extend_from_slice(&rgb);
    v
}

#[test]
fn messages_are_formatted_as_c_does() {
    assert_eq!(
        msg::format(msg::JERR_NO_SOI, &[0, 1]),
        "Not a JPEG file: starts with 0x00 0x01"
    );
    assert_eq!(
        msg::format(msg::JWRN_EXTRANEOUS_DATA, &[-1, 0xd9]),
        "Corrupt JPEG data: 4294967295 extraneous bytes before marker 0xd9"
    );
    assert_eq!(
        msg::format(msg::JWRN_JFIF_MAJOR, &[2, 5]),
        "Warning: unknown JFIF revision number 2.05"
    );
    assert_eq!(
        msg::format(msg::JERR_IMAGE_TOO_BIG, &[65500]),
        "Maximum supported image dimension is 65500 pixels"
    );
    assert_eq!(msg::format(500, &[]), "Bogus message code 500");
}

#[test]
fn empty_and_foreign_files() {
    let (r, rgb, err) = decode(b"", UpstreamDefects::Fix);
    assert_eq!(r.unwrap_err().message, "Empty input file");
    assert!(rgb.is_empty());
    assert_eq!(err, "Empty input file\n");
    let (r, _, err) = decode(b"GIF89a", UpstreamDefects::Fix);
    assert!(r.is_err());
    assert_eq!(err, "Not a JPEG file: starts with 0x47 0x49\n");
    // SOI then nothing: the first warning only, then the image is missing
    let (r, _, err) = decode(&[0xFF, 0xD8], UpstreamDefects::Fix);
    assert!(r.is_err());
    assert_eq!(
        err,
        "Premature end of JPEG file\nJPEG datastream contains no image\n"
    );
}

#[test]
fn encoder_writes_ijg_markers() {
    let img = [7u8];
    let mut out = Vec::new();
    let mut cap = Captured::default();
    encode_gray(
        1,
        1,
        &mut |_, row| row.copy_from_slice(&img),
        &mut out,
        &mut cap,
    )
    .unwrap();
    assert_eq!(&out[..4], &[0xFF, 0xD8, 0xFF, 0xE0]);
    assert_eq!(&out[6..11], b"JFIF\0");
    // DQT 0 at quality 75: the luminance table halved (16 -> 8)
    assert_eq!(&out[20..26], &[0xFF, 0xDB, 0, 67, 0, 8]);
    assert_eq!(&out[out.len() - 2..], &[0xFF, 0xD9]);
    let mut cap = Captured::default();
    let e = encode_gray(0, 5, &mut |_, _| {}, &mut out, &mut cap).unwrap_err();
    assert_eq!(e.message, "Empty JPEG image (DNL not supported)");
    let e = encode_gray(70000, 5, &mut |_, _| {}, &mut out, &mut cap).unwrap_err();
    assert_eq!(
        e.message,
        "Maximum supported image dimension is 65500 pixels"
    );
}

#[test]
fn gray_round_trip_is_close() {
    for (w, h) in [(1u32, 1u32), (8, 8), (13, 9), (40, 33)] {
        // a smooth image
        let img: Vec<u8> = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x * 2 + y * 3) as u8 + 20))
            .collect();
        let mut out = Vec::new();
        let mut cap = Captured::default();
        encode_gray(
            w,
            h,
            &mut |r, row| row.copy_from_slice(&img[(r * w) as usize..((r + 1) * w) as usize]),
            &mut out,
            &mut cap,
        )
        .unwrap();
        let (r, rgb, err) = decode(&out, UpstreamDefects::Fix);
        let hd = r.unwrap();
        assert_eq!((hd.image_width, hd.image_height), (w, h));
        assert!(err.is_empty(), "{err}");
        let mut worst = 0;
        for (i, &g) in img.iter().enumerate() {
            assert_eq!(rgb[3 * i], rgb[3 * i + 1]);
            assert_eq!(rgb[3 * i], rgb[3 * i + 2]);
            worst = worst.max((rgb[3 * i] as i32 - g as i32).abs());
        }
        assert!(worst <= 24, "{w}x{h}: {worst}");
    }
}

#[test]
fn fs813_a_component_twice_in_a_scan() {
    // a three-component baseline file whose scan names component 1 twice
    let mut r = Recipe::random(11, 16);
    r.comps = vec![(1, 1, 1, 0), (2, 1, 1, 1), (3, 1, 1, 1)];
    r.progressive = false;
    r.interleaved = true;
    r.damage = 0;
    r.restart = 0;
    let mut f = cases::make(&r);
    let sos = f.windows(2).position(|w| w == [0xFF, 0xDA]).unwrap();
    // the SOS's component list: n, then (id, tables) pairs
    assert_eq!(f[sos + 4], 3);
    f[sos + 7] = 1;
    let (res, _, err) = decode(&f, UpstreamDefects::Fix);
    assert!(res.is_err());
    assert_eq!(err, "Invalid component ID 1 in SOS\n");
    let mut notices = Notices::new(UpstreamDefects::Fix);
    let mut cap = Captured::default();
    let _ = decode_rgb(&f, &mut cap, Options::default(), &mut notices, &mut |_| {});
    assert!(notices.contains(&larmorx_core::defects::FS_813));
    let (res, rgb, _) = decode(&f, UpstreamDefects::Reproduce);
    assert!(res.is_ok());
    assert_eq!(rgb.len(), (3 * r.width * r.height) as usize);
}

/// The recorded digest of the generated files (`generated_files_digest`).
const DIGEST: u64 = 0xd318_18c2_50d5_b758;

#[test]
fn generated_files_digest() {
    let mut h = Fnv(0xcbf2_9ce4_8422_2325);
    for seed in 0..400u64 {
        let f = cases::make(&Recipe::random(seed, 40));
        h.add(&outcome_bytes(&f, UpstreamDefects::Fix));
        h.add(&outcome_bytes(&f, UpstreamDefects::Reproduce));
    }
    for seed in 0..40u64 {
        let w = 1 + (seed as u32 * 7) % 37;
        let ht = 1 + (seed as u32 * 5) % 29;
        let img = cases::gray_image(seed, w, ht);
        let mut out = Vec::new();
        let mut cap = Captured::default();
        encode_gray(
            w,
            ht,
            &mut |r, row| row.copy_from_slice(&img[(r * w) as usize..((r + 1) * w) as usize]),
            &mut out,
            &mut cap,
        )
        .unwrap();
        h.add(&out);
    }
    assert_eq!(h.0, DIGEST, "{:#018x}", h.0);
}
