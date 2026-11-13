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

/// FNV-1a 64 of bytes.
fn fnv(b: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &x in b {
        h = (h ^ x as u64).wrapping_mul(0x100000001b3);
    }
    h
}

/// Seeded bytes (SplitMix64; plain integer arithmetic only).
fn bytes(seed: u64, n: usize) -> Vec<u8> {
    let mut s = seed;
    (0..n)
        .map(|_| {
            s = s.wrapping_add(0x9E3779B97F4A7C15);
            let mut z = s;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
            (z ^ (z >> 31)) as u8
        })
        .collect()
}

/// The writer's bytes for one array of every data type in every encoding written into the
/// file itself (any bit pattern: NaNs, infinities, subnormals; long doubles as x87 values),
/// and the messages at verbosity 7: digests recorded on Linux x86-64 from the build whose
/// differential probe against gifticlib passed (validation/probes/fs-gifti); every platform
/// must give them (the compressed bytes are zlib-rs's, the same everywhere).
#[test]
fn writer_digests_every_type_and_encoding() {
    let types = [
        NIFTI_TYPE_UINT8,
        NIFTI_TYPE_INT16,
        NIFTI_TYPE_INT32,
        NIFTI_TYPE_FLOAT32,
        NIFTI_TYPE_COMPLEX64,
        NIFTI_TYPE_FLOAT64,
        NIFTI_TYPE_RGB24,
        NIFTI_TYPE_INT8,
        NIFTI_TYPE_UINT16,
        NIFTI_TYPE_UINT32,
        NIFTI_TYPE_INT64,
        NIFTI_TYPE_UINT64,
        NIFTI_TYPE_FLOAT128,
        NIFTI_TYPE_COMPLEX128,
        NIFTI_TYPE_COMPLEX256,
    ];
    let mut all = Vec::new();
    for (k, &t) in types.iter().enumerate() {
        for enc in [
            GIFTI_ENCODING_ASCII,
            GIFTI_ENCODING_B64BIN,
            GIFTI_ENCODING_B64GZ,
        ] {
            let mut lib = Gifti::new();
            lib.set_verb(7);
            lib.xml_set_verb(7);
            let mut c = Captured::default();
            let mut n = Notices::default();
            let (nbyper, _) = lib.datatype_sizes(t, true, &mut c).unwrap();
            let mut g = GiftiImage {
                version: Some(b"1.0".to_vec()),
                ..Default::default()
            };
            lib.add_empty_darray(&mut g, 1, &mut c);
            let da = g.darray[0].as_mut().unwrap();
            da.intent = NIFTI_INTENT_NONE;
            da.datatype = t;
            da.ind_ord = GIFTI_IND_ORD_ROW_MAJOR;
            da.num_dim = 2;
            da.dims = [3, 2, 0, 0, 0, 0];
            da.encoding = enc;
            da.endian = GIFTI_ENDIAN_LITTLE;
            da.nvals = 6;
            da.nbyper = nbyper;
            let mut data = bytes(k as u64 * 7 + enc as u64, 6 * nbyper as usize);
            if t == NIFTI_TYPE_FLOAT128 || t == NIFTI_TYPE_COMPLEX256 {
                for v in data.chunks_mut(16) {
                    v[10..].fill(0); // an x87 value in 16 bytes
                }
            }
            da.data = Some(data);
            let out = lib.write_to_bytes(&g, true, &mut c, &mut n);
            let h = fnv(&out) ^ fnv(c.text(Stream::Err).as_bytes()).rotate_left(1);
            all.push(format!("{t}/{enc}={h:016x}"));
        }
    }
    let joined = all.join(" ");
    assert_eq!(fnv(joined.as_bytes()), 9001464466038773048, "{joined}");
}
