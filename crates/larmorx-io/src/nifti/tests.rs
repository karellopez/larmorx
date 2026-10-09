//! Round trips through files: every type, version, byte order and file layout.

use std::path::{Path, PathBuf};

use larmorx_core::ndarray::{ArrayD, IxDyn, ShapeBuilder};
use proptest::prelude::*;

use super::*;

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn ramp<T: Element>(shape: &[usize], f: impl Fn(usize) -> T) -> ArrayD<T> {
    let n = shape.iter().product();
    ArrayD::from_shape_vec(IxDyn(shape).f(), (0..n).map(f).collect()).unwrap()
}

fn oblique() -> Affine {
    let (c, s) = (0.25f64.cos(), 0.25f64.sin());
    Affine::from_linear(
        [
            [2.0 * c, 0.0, -3.0 * s],
            [0.0, 2.0, 0.0],
            [2.0 * s, 0.0, 3.0 * c],
        ],
        [-80.5, 100.25, -60.0],
    )
}

fn names(dir: &Path) -> Vec<PathBuf> {
    ["a.nii", "b.nii.gz", "c.hdr", "d.img.gz", "E.HDR"]
        .iter()
        .map(|n| dir.join(n))
        .collect()
}

fn check_round_trip<T: Element>(data: ArrayD<T>) {
    let dir = tmp();
    for version in [NiftiVersion::V1, NiftiVersion::V2] {
        for order in [ByteOrder::Little, ByteOrder::Big] {
            for path in names(dir.path()) {
                let mut hdr =
                    header_for_image(None, Some(version), data.shape(), T::DATA_TYPE, &oblique())
                        .unwrap();
                hdr.byte_order = order;
                write(
                    &path,
                    &hdr,
                    data.view(),
                    &WriteOptions {
                        compression_level: 6,
                        n_threads: 3,
                    },
                )
                .unwrap();
                let img = read(&path, &ReadOptions::default()).unwrap();
                assert!(img.fixes.is_empty(), "{:?}", img.fixes);
                assert!(!img.scaled);
                assert_eq!(
                    img.data.as_typed::<T>(),
                    Some(&data),
                    "{path:?} {version:?} {order:?}"
                );
                assert_eq!(img.header.version, version);
                assert_eq!(img.header.byte_order, order);
                assert_eq!(
                    img.header.is_single_file(),
                    path.to_string_lossy().contains(".nii")
                );
                assert!(
                    img.affine().unwrap().allclose(&oblique(), 0.0, 1e-4),
                    "{:?}",
                    img.affine()
                );
                // The header as stored equals the header as written.
                let stored = read_header(&path).unwrap();
                assert_eq!(stored.to_bytes(), {
                    let mut h = hdr.clone();
                    h.set_data_type(T::DATA_TYPE);
                    h.set_shape(data.shape()).unwrap();
                    h.magic = stored.magic;
                    h.vox_offset = stored.vox_offset;
                    h.to_bytes()
                });
            }
        }
    }
}

#[test]
fn every_type_round_trips() {
    let shape = [3, 4, 5, 2];
    check_round_trip(ramp(&shape, |i| i as u8));
    check_round_trip(ramp(&shape, |i| i as i8 - 60));
    check_round_trip(ramp(&shape, |i| i as u16 * 500));
    check_round_trip(ramp(&shape, |i| (i as i16).wrapping_mul(-300)));
    check_round_trip(ramp(&shape, |i| i as u32 * 70_000));
    check_round_trip(ramp(&shape, |i| i as i32 * -70_000));
    check_round_trip(ramp(&shape, |i| (i as u64) << 40));
    check_round_trip(ramp(&shape, |i| -(i as i64) << 40));
    check_round_trip(ramp(&shape, |i| i as f32 * 0.37 - 5.0));
    check_round_trip(ramp(&shape, |i| i as f64 * 1e-3 - 0.0));
    check_round_trip(ramp(&shape, |i| Complex::new(i as f32, -(i as f32))));
    check_round_trip(ramp(&shape, |i| Complex::new(i as f64 * 0.5, 1.0)));
}

#[test]
fn non_fortran_arrays_are_written_in_fortran_order() {
    let dir = tmp();
    let path = dir.path().join("c.nii");
    let c_order =
        ArrayD::from_shape_vec(IxDyn(&[2, 3, 4]), (0..24).map(|v| v as f32).collect()).unwrap();
    let hdr = header_for_image(
        None,
        None,
        c_order.shape(),
        DataType::F32,
        &Affine::IDENTITY,
    )
    .unwrap();
    write(&path, &hdr, c_order.view(), &WriteOptions::default()).unwrap();
    let back = read(&path, &ReadOptions::default()).unwrap();
    assert_eq!(back.data.as_typed::<f32>().unwrap(), &c_order);
    // A strided view (every other element along axis 2).
    let view = c_order.slice_each_axis(|ax| {
        if ax.axis.index() == 2 {
            larmorx_core::ndarray::Slice::new(0, None, 2)
        } else {
            larmorx_core::ndarray::Slice::from(..)
        }
    });
    write(&path, &hdr, view.clone(), &WriteOptions::default()).unwrap();
    assert_eq!(
        read(&path, &ReadOptions::default())
            .unwrap()
            .data
            .as_typed::<f32>()
            .unwrap(),
        &view.to_owned()
    );
}

#[test]
fn scaling_modes() {
    let dir = tmp();
    let path = dir.path().join("s.nii.gz");
    let data = ramp(&[4, 3, 2], |i| i as i16 - 10);
    let mut hdr =
        header_for_image(None, None, data.shape(), DataType::I16, &Affine::IDENTITY).unwrap();
    hdr.scl_slope = 0.5;
    hdr.scl_inter = 3.0;
    write(&path, &hdr, data.view(), &WriteOptions::default()).unwrap();
    let expect = |v: i16| f64::from(v) * 0.5 + 3.0;
    let auto = read(&path, &ReadOptions::default()).unwrap();
    assert!(auto.scaled);
    assert_eq!(auto.data.as_typed::<f64>().unwrap(), &data.mapv(expect));
    let raw = read(
        &path,
        &ReadOptions {
            scaling: Scaling::Raw,
            n_threads: 1,
        },
    )
    .unwrap();
    assert_eq!(raw.data.as_typed::<i16>().unwrap(), &data);
    let f32s = read(
        &path,
        &ReadOptions {
            scaling: Scaling::F32,
            n_threads: 2,
        },
    )
    .unwrap();
    assert_eq!(
        f32s.data.as_typed::<f32>().unwrap(),
        &data.mapv(|v| expect(v) as f32)
    );

    // Unscaled: Auto keeps the stored type, F64 converts.
    hdr.scl_slope = 1.0;
    hdr.scl_inter = 0.0;
    write(&path, &hdr, data.view(), &WriteOptions::default()).unwrap();
    assert_eq!(
        read(&path, &ReadOptions::default())
            .unwrap()
            .data
            .data_type(),
        DataType::I16
    );
    let f64s = read(
        &path,
        &ReadOptions {
            scaling: Scaling::F64,
            n_threads: 1,
        },
    )
    .unwrap();
    assert_eq!(f64s.data.as_typed::<f64>().unwrap(), &data.mapv(f64::from));
}

#[test]
fn negative_zero_survives_a_zero_intercept() {
    let dir = tmp();
    let path = dir.path().join("z.nii");
    let data = ArrayD::from_shape_vec(IxDyn(&[2]).f(), vec![-0.0f32, 1.0]).unwrap();
    let mut hdr = header_for_image(None, None, &[2], DataType::F32, &Affine::IDENTITY).unwrap();
    hdr.scl_slope = 2.0;
    hdr.scl_inter = 0.0;
    write(&path, &hdr, data.view(), &WriteOptions::default()).unwrap();
    let back = read(&path, &ReadOptions::default()).unwrap();
    let v = back.data.as_typed::<f64>().unwrap();
    assert!(v[[0]] == 0.0 && v[[0]].is_sign_negative());
}

#[test]
fn extensions_round_trip() {
    let dir = tmp();
    for name in ["x.nii", "x.nii.gz", "y.hdr"] {
        let path = dir.path().join(name);
        let data = ramp(&[2, 2, 2], |i| i as u8);
        let mut hdr =
            header_for_image(None, None, data.shape(), DataType::U8, &Affine::IDENTITY).unwrap();
        hdr.extensions = vec![
            Extension::new(6, b"comment".to_vec()),
            Extension::new(4, b"<AFNI_attributes/>".to_vec()),
        ];
        write(&path, &hdr, data.view(), &WriteOptions::default()).unwrap();
        let img = read(&path, &ReadOptions::default()).unwrap();
        assert_eq!(img.header.extensions.len(), 2);
        assert_eq!(img.header.extensions[0].trimmed_content(), b"comment");
        assert_eq!(img.header.extensions[1].code, 4);
        assert_eq!(img.data.as_typed::<u8>(), Some(&data));
        if name.contains(".nii") {
            assert_eq!(img.header.vox_offset, (352 + 16 + 32) as f64);
        }
    }
}

#[test]
fn header_for_image_keeps_or_replaces_xforms_like_nibabel() {
    let shape = [4, 5, 6];
    let mut template = header_for_image(None, None, &shape, DataType::F32, &oblique()).unwrap();
    template.set_qform(&oblique(), xform::SCANNER_ANAT).unwrap();
    template.set_sform(&oblique(), xform::MNI_152);
    template.pixdim[4] = 2.5;
    // Same affine (up to float32 storage): codes kept.
    let same = header_for_image(
        Some(&template),
        None,
        &[4, 5, 6, 10],
        DataType::I16,
        &oblique(),
    )
    .unwrap();
    assert_eq!(
        (same.qform_code, same.sform_code, same.pixdim[4]),
        (1, 4, 2.5)
    );
    assert_eq!((same.scl_slope, same.scl_inter), (1.0, 0.0));
    // A different affine: sform aligned, qform unknown.
    let moved = Affine::from_zooms([1.0, 1.0, 1.0], [5.0, 5.0, 5.0]);
    let other = header_for_image(Some(&template), None, &shape, DataType::F32, &moved).unwrap();
    assert_eq!(
        (other.qform_code, other.sform_code),
        (xform::UNKNOWN, xform::ALIGNED_ANAT)
    );
    assert_eq!(other.best_affine().unwrap(), moved);
    // A fresh header is what nibabel writes.
    let fresh = header_for_image(None, None, &shape, DataType::F32, &moved).unwrap();
    assert_eq!(
        (fresh.qform_code, fresh.sform_code, fresh.version),
        (0, 2, NiftiVersion::V1)
    );
    // Shapes too large for NIfTI-1 select NIfTI-2.
    let big = header_for_image(None, None, &[40_000, 2, 1], DataType::U8, &moved).unwrap();
    assert_eq!(big.version, NiftiVersion::V2);
}

#[test]
fn malformed_files_are_errors_not_panics() {
    let dir = tmp();
    let path = dir.path().join("m.nii");
    let data = ramp(&[4, 4, 4], |i| i as i16);
    let hdr = header_for_image(None, None, data.shape(), DataType::I16, &Affine::IDENTITY).unwrap();
    write(&path, &hdr, data.view(), &WriteOptions::default()).unwrap();
    let good = std::fs::read(&path).unwrap();
    for cut in [0, 3, 100, 351, 352, good.len() - 1] {
        std::fs::write(&path, &good[..cut]).unwrap();
        assert!(
            read(&path, &ReadOptions::default()).is_err(),
            "truncated at {cut}"
        );
    }
    let mut bad_magic = good.clone();
    bad_magic[344..348].copy_from_slice(b"xx1\0");
    std::fs::write(&path, &bad_magic).unwrap();
    assert!(matches!(
        read(&path, &ReadOptions::default()),
        Err(Error::Header {
            source: HeaderError::BadMagic(_),
            ..
        })
    ));
    assert!(matches!(
        read(dir.path().join("x.mgz"), &ReadOptions::default()),
        Err(Error::InvalidArgument(_))
    ));
    assert!(matches!(
        read(dir.path().join("missing.nii"), &ReadOptions::default()),
        Err(Error::Io { .. })
    ));
}

#[test]
fn load_fixes_are_reported() {
    let dir = tmp();
    let path = dir.path().join("f.nii");
    let data = ramp(&[2, 2, 2], |i| i as f32);
    let mut hdr =
        header_for_image(None, None, data.shape(), DataType::F32, &Affine::IDENTITY).unwrap();
    write(&path, &hdr, data.view(), &WriteOptions::default()).unwrap();
    // Corrupt qfac and an xform code on disk.
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[76..80].copy_from_slice(&0f32.to_le_bytes());
    bytes[252..254].copy_from_slice(&42i16.to_le_bytes());
    std::fs::write(&path, &bytes).unwrap();
    let img = read(&path, &ReadOptions::default()).unwrap();
    assert_eq!(img.fixes.len(), 2, "{:?}", img.fixes);
    assert_eq!((img.header.pixdim[0], img.header.qform_code), (1.0, 0));
    hdr.pixdim[0] = 0.0;
    assert_eq!(read_header(&path).unwrap().pixdim[0], 0.0); // read_header does not fix
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, .. ProptestConfig::default() })]

    #[test]
    fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..1200), seed_header in any::<bool>()) {
        let dir = tmp();
        let path = dir.path().join("fuzz.nii");
        let mut content = bytes;
        if seed_header && content.len() >= 4 {
            content[..4].copy_from_slice(&348i32.to_le_bytes());
        }
        std::fs::write(&path, &content).unwrap();
        let _ = read(&path, &ReadOptions::default());
        let _ = read_header(&path);
    }

    #[test]
    fn header_fields_survive_nifti2(dims in proptest::collection::vec(1usize..50, 1..=7), p in -1e6f64..1e6) {
        let mut h = NiftiHeader::new(NiftiVersion::V2, &dims, DataType::F64).unwrap();
        h.intent_p1 = p;
        h.toffset = -p;
        h.slice_end = 9;
        let back = NiftiHeader::parse(&h.to_bytes()).unwrap();
        prop_assert_eq!(back.to_bytes(), h.to_bytes());
        prop_assert_eq!(back.toffset.to_bits(), h.toffset.to_bits());
    }
}
