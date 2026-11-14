// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The histogram translations' own tests: the defects' two modes and the C arithmetic's
//! details (the official build's values are checked by the probe, `validation/probes/fs-maths/`).

use super::*;
use larmorx_core::defects::UpstreamDefects;

fn g(mode: UpstreamDefects) -> Globals {
    Globals::new(mode)
}

fn hist(bins: &[f32], counts: &[f32]) -> Histogram {
    let mut h = Histogram::zeros(bins.len() as i32);
    h.bins.copy_from_slice(bins);
    h.counts.copy_from_slice(counts);
    h.bin_size = if bins.len() > 1 {
        bins[1] - bins[0]
    } else {
        1.0
    };
    h.min = bins.first().copied().unwrap_or(0.0);
    h.max = bins.last().copied().unwrap_or(0.0);
    h
}

#[test]
fn clear_counts_fs18() {
    let h0 = hist(&[0.0, 1.0, 2.0], &[3.0, 4.0, 5.0]);
    let mut gr = g(UpstreamDefects::Reproduce);
    let mut h = h0.clone();
    histo_clear_counts_in_place(&mut gr, &mut h);
    assert_eq!(h.counts, vec![0.0, 4.0, 5.0]);
    let fresh = histo_clear_counts(&mut gr, &h0, None).unwrap_new();
    assert_eq!(fresh.bins, vec![0.0, 0.0, 0.0]);
    let mut gf = g(UpstreamDefects::Fix);
    let mut h = h0.clone();
    histo_clear_counts_in_place(&mut gf, &mut h);
    assert_eq!(h.counts, vec![0.0; 3]);
    let fresh = histo_clear_counts(&mut gf, &h0, None).unwrap_new();
    assert_eq!(fresh.bins, h0.bins);
    assert!(gf.notices.contains(&FS_18) && gr.notices.contains(&FS_18));
}

#[test]
fn add_sample_fs19_fs20() {
    // a sample exactly at a bin: lost by FreeSurfer, counted once by the default
    let mut h = hist(&[0.0, 1.0, 2.0, 3.0], &[0.0; 4]);
    let mut gr = g(UpstreamDefects::Reproduce);
    histo_add_sample(&mut gr, &mut h, 2.0, 0.0, 0.0);
    assert_eq!(h.counts, vec![0.0; 4]);
    let mut gf = g(UpstreamDefects::Fix);
    histo_add_sample(&mut gf, &mut h, 2.0, 0.0, 0.0);
    assert_eq!(h.counts, vec![0.0, 0.0, 1.0, 0.0]);
    histo_add_sample(&mut gf, &mut h, 0.25, 0.0, 0.0);
    assert_eq!(h.counts, vec![0.75, 0.25, 1.0, 0.0]);
    // above the range with a lower intensity beyond the bin count: FreeSurfer's first bin
    let mut h = hist(&[5000.0, 5001.0, 5002.0], &[0.0; 3]);
    histo_add_sample(&mut gr, &mut h, 6000.0, 0.0, 0.0);
    assert_eq!(h.counts, vec![1.0, 0.0, 0.0]);
    histo_add_sample(&mut gf, &mut h, 6000.0, 0.0, 0.0);
    assert_eq!(h.counts, vec![1.0, 0.0, 1.0]);
    // below a negative lower intensity: dropped by FreeSurfer
    let mut h = hist(&[-100.0, -99.0, -98.0], &[0.0; 3]);
    histo_add_sample(&mut gr, &mut h, -101.5, 0.0, 0.0);
    assert_eq!(h.counts, vec![0.0; 3]);
    histo_add_sample(&mut gf, &mut h, -101.5, 0.0, 0.0);
    assert_eq!(h.counts, vec![1.0, 0.0, 0.0]);
    // NaN: the first bin in FreeSurfer, nowhere by default
    histo_add_sample(&mut gr, &mut h, f32::NAN, 0.0, 0.0);
    assert_eq!(h.counts[0], 2.0);
    histo_add_sample(&mut gf, &mut h, f32::NAN, 0.0, 0.0);
    assert_eq!(h.counts[0], 2.0);
    assert!(gf.notices.contains(&FS_19) && gf.notices.contains(&FS_20));
}

#[test]
fn smooth_in_place_reads_its_own_output() {
    let h0 = hist(
        &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
        &[0.0, 0.0, 9.0, 1.0, 0.0, 3.0],
    );
    let mut gl = g(UpstreamDefects::Fix);
    let other = histo_smooth(&mut gl, &h0, None, 1.0).unwrap_new();
    let mut inplace = h0.clone();
    histo_smooth_in_place(&mut gl, &mut inplace, 1.0);
    assert_ne!(other.counts, inplace.counts);
    assert_eq!(other.counts[0], inplace.counts[0]);
    let total: f32 = other.counts.iter().sum();
    assert!((total - 13.0).abs() < 2.0);
}

#[test]
fn searches_and_fs22() {
    let h = hist(&[0.0, 1.0, 2.0, 3.0, 4.0], &[1.0, 5.0, 2.0, 7.0, 1.0]);
    assert_eq!(histo_find_last_peak(&h, 3, 0.0), 3);
    assert_eq!(histo_find_first_peak(&h, 3, 0.0), 1);
    assert_eq!(histo_find_highest_peak_in_region(&h, 0, 99), 3);
    let mut gl = g(UpstreamDefects::Fix);
    assert_eq!(histo_find_next_valley(&mut gl, &h, 1), 3);
    assert_eq!(histo_find_previous_valley(&mut gl, &h, 3), 1);
    assert!(!gl.notices.contains(&FS_22));
    // the window of HISTOfindNextPeak reaches the bin after the last
    let h2 = hist(&[0.0, 1.0, 2.0, 3.0, 4.0], &[1.0, 5.0, 2.0, 7.0, 9.0]);
    assert_eq!(histo_find_next_peak(&mut gl, &h2, 3, 1), 4);
    assert!(gl.notices.contains(&FS_22));
    assert_eq!(histo_find_valley(&mut gl, &h, 3, 0, 4), 0);
}

#[test]
fn robust_gaussian_fit_one_bin_fs24() {
    let mut gl = g(UpstreamDefects::Fix);
    let mut e = Vec::new();
    let mut h = hist(&[5.0], &[10.0]);
    let (status, _, sigma) = histo_robust_gaussian_fit(&mut gl, &mut e, None, &mut h, 0.5).unwrap();
    assert_eq!(status, 0);
    assert!(sigma.is_some());
    assert!(gl.notices.contains(&FS_24));
}

#[test]
fn copy_into_smaller_fs21_and_make_pdf_fs23() {
    let src = hist(&[0.0, 1.0, 2.0], &[1.0, 2.0, 3.0]);
    let mut gl = g(UpstreamDefects::Fix);
    let mut small = Histogram::zeros(1);
    histo_copy(&mut gl, &src, Some(&mut small));
    assert_eq!(small.counts, src.counts);
    assert_eq!(small.max_bins, 1);
    assert!(gl.notices.contains(&FS_21));
    let mut other = hist(&[0.0, 1.0, 2.0], &[1.0, 1.0, 2.0]);
    histo_make_pdf(&mut gl, &src, Some(&mut other));
    assert_eq!(other.counts[2], 0.5);
    let mut gr = g(UpstreamDefects::Reproduce);
    let mut other = hist(&[0.0, 1.0, 2.0], &[1.0, 1.0, 2.0]);
    histo_make_pdf(&mut gr, &src, Some(&mut other));
    assert_eq!(other.counts[2], 0.5);
    assert_eq!(other.counts[0], 0.25);
}

#[test]
fn abs_of_an_infinite_range_fs50() {
    let h = hist(&[0.0, f32::INFINITY, 2.0], &[1.0, 2.0, 3.0]);
    for (mode, status) in [
        (UpstreamDefects::Reproduce, 254u8),
        (UpstreamDefects::Fix, 252),
    ] {
        let mut gm = g(mode);
        let mut err = Vec::new();
        let r = histo_abs(&mut gm, &mut err, &h, None);
        assert_eq!(r.err().map(Fatal::status), Some(status));
        assert_eq!(gm.notices.contains(&FS_50), mode == UpstreamDefects::Fix);
        let text = String::from_utf8_lossy(&err).into_owned();
        if mode == UpstreamDefects::Fix {
            assert!(
                text.contains("HISTOabs: absolute values from 0.000000 to inf"),
                "{text}"
            );
        } else {
            assert!(
                text.starts_with(
                    "error: Cannot allocate memory\nerror: HISTOalloc(-2147483648): could not allocate histogram"
                ),
                "{text}"
            );
        }
    }
}

#[test]
fn negative_counts_fail_with_enomem() {
    // The official messages (the open maths probe's `halloc -7` and `hrealloc <H> -3`, observed
    // 2026-11-14): the C library's ENOMEM text first, the program's end with status 254.
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        // FS-01: the message's NUL byte only with reproduce.
        let nul: &[u8] = if mode == UpstreamDefects::Reproduce {
            b"\0\n"
        } else {
            b"\n"
        };
        let mut gm = g(mode);
        let mut err = Vec::new();
        let r = histo_alloc(&mut gm, &mut err, -7);
        assert_eq!(r.err().map(Fatal::status), Some(254));
        assert_eq!(gm.errno, 12);
        assert_eq!(
            err,
            [
                &b"error: Cannot allocate memory\nerror: HISTOalloc(-7): could not allocate histogram"[..],
                nul
            ]
            .concat()
        );
        let mut gm = g(mode);
        let mut err = Vec::new();
        let mut h = hist(&[0.0, 1.0], &[1.0, 1.0]);
        let r = histo_realloc(&mut gm, &mut err, Some(&mut h), -3);
        assert_eq!(r.err().map(Fatal::status), Some(254));
        assert_eq!(
            err,
            [
                &b"error: Cannot allocate memory\nerror: HISTOrealloc(-3): could not allocate histogram"[..],
                nul
            ]
            .concat()
        );
        let mut gm = g(mode);
        let mut err = Vec::new();
        let r = histo_realloc(&mut gm, &mut err, None, -1);
        assert!(r.is_err());
        assert!(err.starts_with(b"error: Cannot allocate memory\nerror: HISTOalloc(-1)"));
        // A count of 0 allocates (glibc's calloc of 0 bytes succeeds): no message.
        let mut gm = g(mode);
        let mut err = Vec::new();
        assert_eq!(
            histo_alloc(&mut gm, &mut err, 0).map(|h| h.nbins).ok(),
            Some(0)
        );
        assert!(err.is_empty() && gm.errno == 0);
    }
}

#[test]
fn robust_fit_with_an_infinite_bin_size_fs51() {
    for mode in [UpstreamDefects::Reproduce, UpstreamDefects::Fix] {
        let mut h = hist(&[0.0, 1.0, 2.0, 3.0], &[1.0, 5.0, 2.0, 1.0]);
        h.bin_size = f32::INFINITY;
        let mut gm = g(mode);
        let mut err = Vec::new();
        let (status, offset, sigma) =
            histo_robust_gaussian_fit(&mut gm, &mut err, None, &mut h, 0.5).unwrap();
        assert_eq!((status, sigma), (ERROR_BADPARM, None));
        assert!(offset.is_some());
        assert!(gm.notices.contains(&FS_51));
    }
}

#[test]
fn add_operand_orders() {
    // two NaNs meet: the official build keeps h1's in blocks of four and one bin at a time, h2's in
    // the first two bins after the last block
    let (p, q) = (f32::from_bits(0x7fc0_0001), f32::from_bits(0xffc0_0002));
    let h1 = hist(&[0.0, 1.0, 2.0, 3.0, 4.0, 5.0], &[p; 6]);
    let h2 = hist(&[0.0, 1.0, 2.0, 3.0, 4.0, 5.0], &[q; 6]);
    let mut gm = g(UpstreamDefects::Reproduce);
    let sum = histo_add(&mut gm, &h1, Some(&h2), None).unwrap_new();
    let bits: Vec<u32> = sum.counts.iter().map(|c| c.to_bits()).collect();
    assert_eq!(
        bits,
        [
            0x7fc0_0001,
            0x7fc0_0001,
            0x7fc0_0001,
            0x7fc0_0001,
            0xffc0_0002,
            0xffc0_0002
        ]
    );
    let mut a = h1.clone();
    histo_add_into_first(&mut gm, &mut a, &h2);
    assert!(a.counts.iter().all(|c| c.to_bits() == 0x7fc0_0001));
    let mut b = h2.clone();
    histo_add_into_second(&mut gm, &h1, &mut b);
    assert!(b.counts.iter().all(|c| c.to_bits() == 0x7fc0_0001));
    // inf - inf is the default NaN (sign set) on every platform
    let i = hist(&[0.0], &[f32::INFINITY]);
    let d = histo_subtract(&mut gm, &i, &i, None).unwrap_new();
    assert_eq!(d.counts[0].to_bits(), 0xffc0_0000);
}
