// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-MNI
use larmorx_core::cfmt::{Captured, Stream};

use super::*;

fn read(text: &str) -> (Result<GeneralTransform, ()>, String) {
    let mut c = Captured::default();
    let r = input_transform_from_bytes(text.as_bytes(), None, &Mni, &mut c);
    (r, c.text(Stream::Err))
}

fn linear(t: &GeneralTransform) -> (Transform, Transform) {
    let mut q = larmorx_core::cfmt::Silent;
    (
        *t.linear_transform(&mut q).unwrap(),
        *t.inverse_linear_transform(&mut q).unwrap(),
    )
}

const T1: &str = "MNI Transform File\n% Single linear transformation.\n\nTransform_Type = Linear;\nLinear_Transform =\n 1 0 0 10\n 0 1 0 20\n 0 0 1 30;\n";

#[test]
fn reads_a_linear_transform_and_its_inverse() {
    let (r, msg) = read(T1);
    let t = r.unwrap();
    assert_eq!(msg, "");
    let (f, i) = linear(&t);
    assert_eq!(f[0][3], 10.0);
    assert_eq!(f[2][3], 30.0);
    assert_eq!(i[1][3], -20.0);
    assert_eq!(f[3], [0.0, 0.0, 0.0, 1.0]);
}

#[test]
fn invert_flag_swaps_the_matrices() {
    let (r, _) = read(
        "MNI Transform File\nTransform_Type = Linear;\nInvert_Flag = True;\nLinear_Transform =\n 2 0 0 1\n 0 4 0 2\n 0 0 8 3;\n",
    );
    let t = r.unwrap();
    assert!(!t.inverse_flag);
    let (f, i) = linear(&t);
    assert_eq!(f[0][0], 0.5);
    assert_eq!(i[0][0], 2.0);
}

#[test]
fn two_linear_transforms_are_multiplied() {
    let (r, _) = read(
        "MNI Transform File\nTransform_Type = Linear;\nLinear_Transform =\n 1 0 0 1\n 0 1 0 2\n 0 0 1 3;\nTransform_Type = Linear;\nLinear_Transform =\n 2 0 0 1\n 0 2 0 2\n 0 0 2 3;\n",
    );
    let t = r.unwrap();
    assert_eq!(t.transform_type(), TransformType::Linear);
    let (f, _) = linear(&t);
    assert_eq!(f[0], [2.0, 0.0, 0.0, 3.0]);
}

#[test]
fn a_tab_does_not_end_a_number() {
    let (r, _) = read(
        "MNI Transform File\nTransform_Type = Linear;\nLinear_Transform =\n 1\t0 0 1\n 0 1 0 2\n 0 0 1 3 4;\n",
    );
    let (f, _) = linear(&r.unwrap());
    assert_eq!(f[0], [1.0, 0.0, 1.0, 0.0]);
    assert_eq!(f[2], [0.0, 1.0, 3.0, 4.0]);
}

#[test]
fn messages_of_the_failures() {
    let cases: &[(&str, &str)] = &[
        ("", "input_transform(): could not read header in file.\n"),
        ("Bad header\n", "input_transform(): invalid header in file.\n"),
        ("MNI Transform File\n", "input_transform: no transform present.\n"),
        (
            "MNI Transform File\nTransform_Type = Affine;\n",
            "input_transform(): invalid transform type.\ninput_transform: error reading transform.\n",
        ),
        (
            "MNI Transform File\nTransform_Type = Linear;\nLinear_Transform =\n 1 0 0 1\n 0 1 x 2\n 0 0 1 3;\n",
            "input_transform(): error reading transform elem [2,3]\ninput_transform: error reading transform.\n",
        ),
        (
            "MNI Transform File\nTransform_Type = Linear;\nLinear_Transform =\n 1 0 0 1\n 0 1 0 2\n 0 0 1 3\n",
            "Expected ';', found end of file.\ninput_transform: error reading transform.\n",
        ),
        (
            "MNI Transform File\nTransform_Type = Linear;\nInvert_Flag = Maybe;\n",
            "Expected True or False after Invert_Flag =\ninput_transform: error reading transform.\n",
        ),
    ];
    for (text, want) in cases {
        let (r, msg) = read(text);
        assert!(r.is_err(), "{text:?}");
        assert_eq!(&msg, want, "{text:?}");
    }
}

#[test]
fn a_singular_matrix_gives_identities() {
    let (r, msg) = read(
        "MNI Transform File\nTransform_Type = Linear;\nLinear_Transform =\n 1 0 0 1\n 0 1 0 2\n 0 0 0 3;\n",
    );
    assert_eq!(msg, "");
    let (f, i) = linear(&r.unwrap());
    assert_eq!(f, make_identity_transform());
    assert_eq!(i, make_identity_transform());
}

#[test]
fn thin_plate_splines_read_and_write_back() {
    let text = "MNI Transform File\n\nTransform_Type = Thin_Plate_Spline_Transform;\nNumber_Dimensions = 3;\nPoints =\n 1 2 3;\nDisplacements =\n 0 0 0\n 1 1 1\n 2 2 2\n 3 3 3\n 4 4 4;\n";
    let (r, msg) = read(text);
    assert_eq!(msg, "");
    let t = r.unwrap();
    assert_eq!(output_transform_to_string(&t, None), text);
}

#[test]
fn a_spline_list_at_the_end_of_the_file_stops() {
    // volume_io reads forever here (FS-72).
    let (r, msg) = read(
        "MNI Transform File\nTransform_Type = Thin_Plate_Spline_Transform;\nNumber_Dimensions = 3;\nPoints = 1 2 3 ",
    );
    assert!(r.is_err());
    assert_eq!(
        msg,
        "Expected ';', found end of file.\ninput_transform: error reading transform.\n"
    );
}

#[test]
fn the_writer_formats_with_15_digits() {
    let mut t = make_identity_transform();
    t[0][3] = 0.1;
    t[1][1] = 1.0 / 3.0;
    let g = GeneralTransform::linear(Some(&t), &Mni, &mut larmorx_core::cfmt::Silent);
    let s = output_transform_to_string(&g, Some("a\nb"));
    assert_eq!(
        s,
        "MNI Transform File\n%a\n%b\n\nTransform_Type = Linear;\nLinear_Transform =\n 1 0 0 0.1\n 0 0.333333333333333 0 0\n 0 0 1 0;\n"
    );
}

#[test]
fn expansion_of_names() {
    assert_eq!(expand_filename(b"a\\$b"), b"a$b".to_vec());
    assert_eq!(expand_filename(b"$LARMORX_SURELY_UNSET_9/x"), b"$LARMORX_SURELY_UNSET_9/x".to_vec());
    assert_eq!(extract_directory(b"d/e/f.xfm"), b"d/e/".to_vec());
    assert_eq!(extract_directory(b"f.xfm"), b".".to_vec());
}

#[test]
fn gaussian_inverse() {
    let m = vec![
        vec![2.0, 1.0, 0.0, 3.0],
        vec![0.5, 4.0, 1.0, -2.0],
        vec![1.0, 0.0, 3.0, 1.0],
        vec![0.0, 0.0, 0.0, 1.0],
    ];
    let inv = invert_square_matrix(&m).unwrap();
    for i in 0..4 {
        for j in 0..4 {
            let mut s = 0.0;
            for k in 0..4 {
                s += m[i][k] * inv[k][j];
            }
            let want = if i == j { 1.0 } else { 0.0 };
            assert!((s - want).abs() < 1e-12);
        }
    }
    assert!(invert_square_matrix(&[vec![1.0, 2.0], vec![2.0, 4.0]]).is_none());
}
