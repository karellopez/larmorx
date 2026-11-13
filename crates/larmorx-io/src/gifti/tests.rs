// SPDX-License-Identifier: Apache-2.0
use larmorx_core::cfmt::{Captured, Stream};
use larmorx_core::defects::Notices;

use super::*;

fn image() -> GiftiImage {
    let lib = Gifti::new();
    let mut c = Captured::default();
    let mut n = Notices::default();
    let mut g = GiftiImage {
        version: Some(b"1.0".to_vec()),
        ..Default::default()
    };
    lib.add_to_meta(
        &mut g.meta,
        b"AnatomicalStructurePrimary",
        b"CortexLeft",
        false,
        &mut c,
        &mut n,
    );
    lib.add_empty_darray(&mut g, 2, &mut c);
    let coords: Vec<f32> = vec![0.5, -1.25, 3.0, 1e-7, 123456.75, -0.0];
    let tri: Vec<i32> = vec![0, 1, 2, 2, 1, 0];
    for (k, enc) in [GIFTI_ENCODING_ASCII, GIFTI_ENCODING_B64GZ]
        .into_iter()
        .enumerate()
    {
        let da = g.darray[k].as_mut().unwrap();
        da.intent = if k == 0 {
            NIFTI_INTENT_POINTSET
        } else {
            NIFTI_INTENT_TRIANGLE
        };
        da.datatype = if k == 0 {
            NIFTI_TYPE_FLOAT32
        } else {
            NIFTI_TYPE_INT32
        };
        da.ind_ord = GIFTI_IND_ORD_ROW_MAJOR;
        da.num_dim = 2;
        da.dims = [2, 3, 0, 0, 0, 0];
        da.encoding = enc;
        da.endian = GIFTI_ENDIAN_LITTLE;
        da.nvals = 6;
        da.nbyper = 4;
        da.data = Some(if k == 0 {
            coords.iter().flat_map(|v| v.to_le_bytes()).collect()
        } else {
            tri.iter().flat_map(|v| v.to_le_bytes()).collect()
        });
    }
    g.darray[0]
        .as_mut()
        .unwrap()
        .coordsys
        .push(Some(CoordSystem {
            dataspace: Some(b"NIFTI_XFORM_UNKNOWN".to_vec()),
            xformspace: Some(b"NIFTI_XFORM_TALAIRACH".to_vec()),
            xform: [
                [1.0, 0.0, 0.0, 0.5],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }));
    g
}

#[test]
fn writes_and_reads_back() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("t x.gii");
    let mut lib = Gifti::new();
    let mut c = Captured::default();
    let mut n = Notices::default();
    let mut g = image();
    assert_eq!(lib.write_image(&mut g, &f, true, &mut c, &mut n), 0);
    let text = std::fs::read_to_string(&f).unwrap();
    assert!(text.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE GIFTI SYSTEM \"http://gifti.projects.nitrc.org/gifti.dtd\">\n<GIFTI Version=\"1.0\"  NumberOfDataArrays=\"2\">\n"));
    assert!(text.contains("<Data>\n         0.500000 -1.250000 3.000000 \n         0.000000 123456.750000 -0.000000 \n      </Data>\n"));
    assert!(text.contains("<Name><![CDATA[gifticlib-version]]></Name>"));
    let back = lib
        .read_image(&f, true, &Gifticlib, &mut c, &mut n)
        .expect("read back");
    assert_eq!(c.text(Stream::Err), "");
    assert_eq!(back.num_da, 2);
    let d0 = back.darray[0].as_ref().unwrap();
    let want: Vec<f32> = vec![0.5, -1.25, 3.0, 0.0, 123456.75, -0.0];
    let got: Vec<f32> = d0
        .data
        .as_ref()
        .unwrap()
        .chunks(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    assert_eq!(
        got.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        want.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
    );
    assert_eq!(d0.coordsys[0].as_ref().unwrap().xform[0][3], 0.5);
    assert_eq!(
        back.darray[1].as_ref().unwrap().data,
        g.darray[1].as_ref().unwrap().data
    );
    assert_eq!(back.compressed, 1);
    assert_eq!(
        lib.get_meta_value(&back.meta, b"AnatomicalStructurePrimary", &mut c, &mut n),
        Some(&Some(b"CortexLeft".to_vec()))
    );
    assert!(lib.valid_gifti_image(Some(&back), true, &Gifticlib, true, &mut c, &mut n));
}

#[test]
fn small_buffers_split_values_as_gifticlib() {
    // a 2,048-byte buffer (one DataArray of a few values) cuts the ASCII numbers across
    // XML_Parse calls; the values must not change
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("big.gii");
    let mut lib = Gifti::new();
    let mut c = Captured::default();
    let mut n = Notices::default();
    let mut g = image();
    let vals: Vec<f32> = (0..3000).map(|i| i as f32 * 0.37 - 400.0).collect();
    let da = g.darray[0].as_mut().unwrap();
    da.dims = [1000, 3, 0, 0, 0, 0];
    da.nvals = 3000;
    da.data = Some(vals.iter().flat_map(|v| v.to_le_bytes()).collect());
    lib.write_image(&mut g, &f, true, &mut c, &mut n);
    let back = lib
        .read_image(&f, true, &Gifticlib, &mut c, &mut n)
        .unwrap();
    let got: Vec<f32> = back.darray[0]
        .as_ref()
        .unwrap()
        .data
        .as_ref()
        .unwrap()
        .chunks(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    let want: Vec<f32> = vals
        .iter()
        .map(|v| larmorx_core::cfmt::f(*v as f64, 6).parse::<f64>().unwrap() as f32)
        .collect();
    assert_eq!(got, want);
}

#[test]
fn long_double_lines() {
    use larmorx_core::cfmt::f80_f;
    use larmorx_core::x87::F80;
    assert_eq!(f80_f(F80::from_f64(0.1), 6), "0.100000");
    assert_eq!(f80_f(F80::from_f64(-2.5e-7), 6), "-0.000000");
    assert_eq!(
        f80_f(F80::from_f64(1e20), 6),
        "100000000000000000000.000000"
    );
    assert_eq!(f80_f(F80::from_f64(0.0000005), 6), "0.000000");
    assert_eq!(f80_f(F80::from_f64(0.0000015), 6), "0.000002");
    assert_eq!(f80_f(F80::INFINITY, 6), "inf");
    assert_eq!(base64_roundtrip(), "QUJD");
}

fn base64_roundtrip() -> String {
    String::from_utf8(write::base64(b"ABC")).unwrap()
}
