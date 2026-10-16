// SPDX-License-Identifier: Apache-2.0
//! ITK's HDF5 transform files written and read back (`larmorx_io::itk_hdf5`).
//!
//! The first tests check that larmorx writes, byte for byte, files that ITK 5.4.5 wrote on
//! HDF5 1.12.1 (the ANTs 2.6.5 oracle, 2026-10-16): given the same transforms, ITK's object time
//! and its version strings, larmorx's file has the length and the FNV-1a hash of ITK's. The
//! oracle's files are not committed; their hashes are (validation: `itk-hdf5`).

use larmorx_core::Precision;
use larmorx_core::defects::{Notices, UpstreamDefects};
use larmorx_io::itk_hdf5::{self, WriteOptions};
use larmorx_io::itk_transform::{ItkTransformParts, Parameters};

/// `affine-centered.mat` of the test data, as ITK stores it (bits of the doubles).
const AFFINE: [u64; 12] = [
    0x3ff0d10080000000,
    0xbfb64236e0000000,
    0xbfb0c1ae00000000,
    0x3fc1d0e160000000,
    0x3fedd060a0000000,
    0xbfa983fc80000000,
    0x3f987bb3e0000000,
    0x3fb8b00ec0000000,
    0x3ff0485600000000,
    0x3ff8000000000000,
    0xc002000000000000,
    0x3fe8000000000000,
];
const CENTER: [u64; 3] = [0x3fd6666660000000, 0xbff3333340000000, 0x4003333340000000];
/// `antsApplyTransforms --float 1 -o Linear[...] -t affine-centered.mat -t euler-centered.mat`.
const COLLAPSED_F32: [u32; 12] = [
    0x3f8218fd, 0xbe6a7922, 0xbdd4dafe, 0x3e926305, 0x3f654421, 0xbe21acf6, 0x3dcad9dc, 0x3e329afa,
    0x3f806d19, 0x40438ede, 0xc029c1e2, 0x4025b82d,
];

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325u64, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
    })
}

fn itk_options(time: u32) -> WriteOptions {
    WriteOptions {
        compression: true,
        time,
        itk_version: "5.4.5".into(),
        hdf_version: "HDF5 library version: 1.12.1".into(),
        os_name: "Linux".into(),
        os_version: "6.8.0-124-generic".into(),
    }
}

fn composite() -> ItkTransformParts {
    ItkTransformParts {
        name: "CompositeTransform_double_3_3".into(),
        parameters: Parameters::F64(Vec::new()),
        fixed_parameters: Vec::new(),
    }
}

fn affine() -> ItkTransformParts {
    ItkTransformParts {
        name: "AffineTransform_double_3_3".into(),
        parameters: Parameters::F64(AFFINE.iter().map(|&b| f64::from_bits(b)).collect()),
        fixed_parameters: CENTER.iter().map(|&b| f64::from_bits(b)).collect(),
    }
}

fn identity() -> ItkTransformParts {
    ItkTransformParts {
        name: "IdentityTransform_double_3_3".into(),
        parameters: Parameters::F64(Vec::new()),
        fixed_parameters: Vec::new(),
    }
}

fn write(parts: &[ItkTransformParts], options: &WriteOptions) -> Vec<u8> {
    let mut notices = Notices::new(UpstreamDefects::Reproduce);
    itk_hdf5::write(parts, options, &mut notices).expect("written")
}

#[test]
fn itk_composite_of_one_affine() {
    // antsApplyTransforms -d 3 -o CompositeTransform[c1.h5] -t affine-centered.mat
    let bytes = write(&[composite(), affine()], &itk_options(1792164947));
    assert_eq!((bytes.len(), fnv1a(&bytes)), (12416, 0x246dd354b2c2e18b));
}

#[test]
fn itk_float_linear() {
    let parts = [ItkTransformParts {
        name: "AffineTransform_float_3_3".into(),
        parameters: Parameters::F32(COLLAPSED_F32.iter().map(|&b| f32::from_bits(b)).collect()),
        fixed_parameters: CENTER.iter().map(|&b| f64::from_bits(b)).collect(),
    }];
    let bytes = write(&parts, &itk_options(1792164947));
    assert_eq!((bytes.len(), fnv1a(&bytes)), (12416, 0x71b8fa5947af369f));
}

#[test]
fn itk_dense_groups() {
    // -o CompositeTransform[...] of 9 and 37 affines: TransformGroup turns dense at its
    // ninth link (fractal heap, v2 B-tree, condensed header); 38 links fill the heap's first
    // direct block.
    let mut parts = vec![composite()];
    parts.extend(std::iter::repeat_with(affine).take(9));
    let bytes = write(&parts, &itk_options(1792169571));
    assert_eq!((bytes.len(), fnv1a(&bytes)), (37815, 0xdb0d852b3527382b));
    let mut parts = vec![composite()];
    parts.extend(std::iter::repeat_with(affine).take(37));
    let bytes = write(&parts, &itk_options(1792171730));
    assert_eq!((bytes.len(), fnv1a(&bytes)), (129731, 0x2952648c5d2849fd));
}

#[test]
fn itk15_identity_with_compression() {
    // CompositeTransformUtil --assemble comp.h5 affine-centered.mat identity.txt: ITK fails
    // and leaves the file written so far.
    let parts = [composite(), affine(), identity()];
    let options = itk_options(1792169651);
    let mut notices = Notices::new(UpstreamDefects::Reproduce);
    let failure = itk_hdf5::write(&parts, &options, &mut notices).expect_err("ITK fails");
    assert!(failure.ants_report.contains("H5Pset_chunk failed"));
    assert!(failure.ants_report.contains("Line: 462"));
    let partial = failure.partial.expect("the file ITK leaves");
    assert_eq!(
        (partial.len(), fnv1a(&partial)),
        (12416, 0xf0b78e510bfcb258)
    );
    // By default the empty parameters are stored uncompressed, and the file reads back.
    let mut notices = Notices::new(UpstreamDefects::Fix);
    let bytes = itk_hdf5::write(&parts, &options, &mut notices).expect("written");
    assert_eq!(notices.as_slice().len(), 1);
    assert_eq!(notices.as_slice()[0].defect.id, "ITK-15");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fixed.h5");
    std::fs::write(&path, bytes).unwrap();
    let mut notices = Notices::new(UpstreamDefects::Fix);
    let back = itk_hdf5::read(&path, Precision::Double, &mut notices).expect("read back");
    assert_eq!(back, parts.to_vec());
}

#[test]
fn round_trip_and_determinism() {
    let field = ItkTransformParts {
        name: "DisplacementFieldTransform_float_3_3".into(),
        parameters: Parameters::F32((0..3 * 4 * 5 * 6).map(|i| i as f32 / 8.0).collect()),
        fixed_parameters: vec![
            4.0, 5.0, 6.0, -1.5, 2.0, 0.25, 2.0, 2.0, 3.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
            1.0,
        ],
    };
    let parts = vec![composite(), affine(), field];
    for compression in [true, false] {
        let options = WriteOptions {
            compression,
            ..WriteOptions::default()
        };
        let a = write(&parts, &options);
        let b = write(&parts, &options);
        assert_eq!(a, b, "the same transforms give the same bytes");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.h5");
        std::fs::write(&path, a).unwrap();
        let mut notices = Notices::new(UpstreamDefects::Fix);
        assert_eq!(
            itk_hdf5::read(&path, Precision::Double, &mut notices).unwrap(),
            parts
        );
        assert!(notices.is_empty());
    }
}

#[test]
fn composite_after_the_first_is_refused() {
    let mut notices = Notices::new(UpstreamDefects::Fix);
    let failure = itk_hdf5::write(
        &[affine(), composite()],
        &WriteOptions::default(),
        &mut notices,
    )
    .expect_err("refused");
    assert!(
        failure
            .ants_report
            .contains("Composite Transform can only be 1st transform in a file")
    );
    assert!(failure.partial.is_none());
}

#[test]
fn many_transforms_beyond_the_first_heap_block() {
    // 38 and more transforms: HDF5 would give the heap an indirect block; larmorx keeps the
    // links as link messages. Every transform reads back, in order.
    let mut parts = vec![composite()];
    for i in 0..45 {
        let mut a = affine();
        if let Parameters::F64(p) = &mut a.parameters {
            p[9] = f64::from(i);
        }
        parts.push(a);
    }
    let bytes = write(&parts, &WriteOptions::default());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("many.h5");
    std::fs::write(&path, bytes).unwrap();
    let mut notices = Notices::new(UpstreamDefects::Fix);
    assert_eq!(
        itk_hdf5::read(&path, Precision::Double, &mut notices).unwrap(),
        parts
    );
}
