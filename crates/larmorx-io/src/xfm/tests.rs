// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-MNI
use larmorx_core::cfmt::{Captured, Stream};

use super::*;

fn read(text: &str) -> (Result<GeneralTransform, ()>, String) {
    let mut c = Captured::default();
    let mut n = larmorx_core::defects::Notices::default();
    let r = input_transform_from_bytes(text.as_bytes(), None, &Mni, &mut c, &mut n);
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

#[test]
fn files_compressed_defaults_and_dialects() {
    use std::io::Write as _;
    let dir = tempfile::tempdir().unwrap();
    let mut c = Captured::default();
    let mut n = larmorx_core::defects::Notices::default();
    // `.xfm` added to a name without an extension
    std::fs::write(dir.path().join("t1.xfm"), T1).unwrap();
    let name = dir.path().join("t1");
    let t = input_transform_file(name.as_os_str().as_encoded_bytes(), &Mni, &mut c, &mut n).unwrap();
    assert_eq!(linear(&t).0[1][3], 20.0);
    // a gzipped file, and a missing name whose compressed file exists
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::new(6));
    e.write_all(T1.as_bytes()).unwrap();
    std::fs::write(dir.path().join("z.xfm.gz"), e.finish().unwrap()).unwrap();
    for name in ["z.xfm.gz", "z.xfm"] {
        let p = dir.path().join(name);
        let t = input_transform_file(p.as_os_str().as_encoded_bytes(), &Mni, &mut c, &mut n).unwrap();
        assert_eq!(linear(&t).0[2][3], 30.0);
    }
    // a file that is not gzip data under a .gz name
    std::fs::write(dir.path().join("bad.xfm.gz"), b"not gzip").unwrap();
    let p = dir.path().join("bad.xfm.gz");
    let mut c = Captured::default();
    assert!(input_transform_file(p.as_os_str().as_encoded_bytes(), &Mni, &mut c, &mut n).is_err());
    assert!(c.text(Stream::Err).starts_with("Error uncompressing "));
    // a missing file
    let mut c = Captured::default();
    let p = dir.path().join("none.xfm");
    assert!(input_transform_file(p.as_os_str().as_encoded_bytes(), &Mni, &mut c, &mut n).is_err());
    assert_eq!(
        c.text(Stream::Err),
        format!(
            "Error:  could not open file \"{}\".  \nSystem message: No such file or directory\n",
            p.display()
        )
    );
    // the writer adds `.xfm`, and what it writes reads back
    let w = dir.path().join("out");
    output_transform_file(w.as_os_str().as_encoded_bytes(), Some(b"c"), &t, &Mni, &mut c, &mut n).unwrap();
    let back = std::fs::read_to_string(dir.path().join("out.xfm")).unwrap();
    assert!(back.starts_with("MNI Transform File\n%c\n\nTransform_Type = Linear;\n"));
}

#[test]
fn a_dialect_changes_messages_and_types() {
    struct LinearOnly;
    impl Dialect for LinearOnly {
        fn transform_type(&self, name: &[u8], streams: &mut dyn larmorx_core::cfmt::Streams) -> Option<TransformType> {
            if name == b"Linear" {
                return Some(TransformType::Linear);
            }
            streams.err(&format!("no {}\n", String::from_utf8_lossy(name)));
            None
        }
        fn nul_ends_token(&self) -> bool {
            true
        }
    }
    let mut c = Captured::default();
    let mut n = larmorx_core::defects::Notices::default();
    let r = input_transform_from_bytes(
        b"MNI Transform File\nTransform_Type = Grid_Transform;\n",
        None,
        &LinearOnly,
        &mut c,
        &mut n,
    );
    assert!(r.is_err());
    assert_eq!(c.text(Stream::Err), "no Grid_Transform\ninput_transform: error reading transform.\n");
    // a NUL in a number: volume_io skips it, the dialect ends the token there
    let text = b"MNI Transform File\nTransform_Type = Linear;\nLinear_Transform =\n 1 0 0 1\x002\n 0 1 0 2\n 0 0 1 3;\n";
    let a = input_transform_from_bytes(text, None, &Mni, &mut Captured::default(), &mut n).unwrap();
    let b = input_transform_from_bytes(text, None, &LinearOnly, &mut Captured::default(), &mut n).unwrap();
    assert_eq!(linear(&a).0[0][3], 12.0);
    assert_eq!(linear(&b).0[0][3], 1.0);
}

#[test]
fn defects_71_to_75_are_noted() {
    let mut c = Captured::default();
    let mut n = larmorx_core::defects::Notices::default();
    let tps = |rest: &str| format!("MNI Transform File\nTransform_Type = Thin_Plate_Spline_Transform;\nNumber_Dimensions = {rest}");
    let _ = input_transform_from_bytes(tps("0;\nPoints = 1;\n").as_bytes(), None, &Mni, &mut c, &mut n);
    assert!(n.contains(&larmorx_core::defects::FS_73));
    let _ = input_transform_from_bytes(tps("3;\nPoints = ;\nDisplacements = 1 2 3 4 5 6 7 8 9 10 11 12;\n").as_bytes(), None, &Mni, &mut c, &mut n);
    assert!(n.contains(&larmorx_core::defects::FS_74));
    let _ = input_transform_from_bytes(tps("3;\nPoints = 1 2 3 \n").as_bytes(), None, &Mni, &mut c, &mut n);
    assert!(n.contains(&larmorx_core::defects::FS_72));
    // a `~user` whose home directory is empty, at the start of a name
    struct EmptyHome;
    impl Dialect for EmptyHome {
        fn user_home_directory(&self, _user: &[u8]) -> Option<Vec<u8>> {
            Some(Vec::new())
        }
    }
    let mut n2 = larmorx_core::defects::Notices::default();
    let e = expand_filename_in(b"~nobody/x", &EmptyHome, &mut n2);
    assert_eq!(e, b"/x".to_vec());
    assert!(n2.contains(&larmorx_core::defects::FS_75));
}
