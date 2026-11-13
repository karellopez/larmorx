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
