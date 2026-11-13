// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `MRIalloc` of a width of -2147483648 (track C's relay 16 report: FreeSurfer's
//! `MRIdownsampleN` with a NaN factor under `reproduce`, measured on the official build by the
//! fs-volume probe's `dnsn`): the chunk cannot be allocated; each slice's buffer, `width * height`
//! in 32-bit arithmetic, wraps to 0 bytes for an even height and is given, so the index tables
//! fail: `MRI::Shape::Shape(): volume size must be greater than 0 in every dimension`, then the
//! error `could not allocate -2147482648 elt index array`, status 254. An odd height wraps to
//! -2147483648 bytes, which calloc refuses: `could not allocate memory for slice 1 out of 1`,
//! status 1.

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_mri::mri::{MRI_UCHAR, Mri};

fn alloc(h: i32) -> (u8, String) {
    let mut g = Globals::new(UpstreamDefects::Reproduce);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let r = Mri::new(
        &mut g,
        &mut out,
        &mut err,
        &[i32::MIN, h, 1],
        MRI_UCHAR,
        true,
    );
    let status = match r {
        Ok(_) => 0,
        Err(f) => f.status(),
    };
    out.extend_from_slice(&err);
    (status, String::from_utf8_lossy(&out).into_owned())
}

#[test]
fn a_width_of_int_min_fails_in_the_index_tables_or_the_slices() {
    let (status, text) = alloc(4);
    assert_eq!(status, 254, "{text}");
    assert!(
        text.contains("volume size must be greater than 0 in every dimension"),
        "{text}"
    );
    assert!(
        text.contains("could not allocate -2147482648 elt index array"),
        "{text}"
    );
    assert!(
        !text.contains("could not allocate memory for slice"),
        "{text}"
    );
    let (status, text) = alloc(3);
    assert_eq!(status, 1, "{text}");
    assert!(
        text.contains("could not allocate memory for slice 1 out of 1"),
        "{text}"
    );
}
