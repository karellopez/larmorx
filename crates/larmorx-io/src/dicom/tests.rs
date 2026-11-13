// SPDX-License-Identifier: Apache-2.0
//! Unit cases of the DICOM reader (the probe `validation/probes/fs-dicom` compares it with CTN).

use super::cond::*;
use super::*;

#[test]
fn messages_format_as_c() {
    let m = ctn_message(DCM_ELEMENTLENGTHERROR).unwrap();
    let t = c_format(
        m,
        &[Arg::Int(0x10), Arg::Int(0x20), Arg::U32(8), Arg::U32(2), Arg::Str(b"readFile")],
    );
    assert_eq!(
        t,
        b"DCM Data Element (0010 0020) longer (8) than remaining length (2) of data in stream or file in readFile"
    );
    assert_eq!(c_format(b"%8x|%-4d|%04x|%ld", &[Arg::Long(0x9_0002), Arg::Int(-3), Arg::Int(7), Arg::U32(u32::MAX)]),
        b"   90002|-3  |0007|4294967295");
}

#[test]
fn vr_codes_compare_as_c_strings() {
    assert_eq!(Vr::from_code(*b"OB"), Some(Vr::Ob));
    assert_eq!(Vr::from_code(*b"  "), Some(Vr::Ctx));
    assert_eq!(Vr::from_code([0, b'X']), Some(Vr::Dlm));
    assert_eq!(Vr::from_code([b'A', 0]), None);
    assert_eq!(Vr::from_code(*b"XX"), None);
}
