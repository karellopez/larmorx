// SPDX-License-Identifier: Apache-2.0
//! Unit cases of the DICOM reader (the probe `validation/probes/fs-dicom` compares it with CTN
//! 3.0.6 and FreeSurfer's copy on generated files; these keep its rules and the digest of a fixed
//! set of files, recorded on Linux x86-64, so every platform is checked against it).

use super::cond::*;
use super::*;
use larmorx_core::cfmt::{Captured, Stream};
use larmorx_core::defects::{Notices, UpstreamDefects};
use std::path::{Path, PathBuf};

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("larmorx-dicom-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn el(o: &mut Vec<u8>, tag: u32, vr: Option<&[u8; 2]>, len: u32, val: &[u8], big: bool) {
    let p16 = |o: &mut Vec<u8>, v: u16| {
        o.extend_from_slice(&if big {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        })
    };
    let p32 = |o: &mut Vec<u8>, v: u32| {
        o.extend_from_slice(&if big {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        })
    };
    p16(o, (tag >> 16) as u16);
    p16(o, tag as u16);
    match vr {
        Some(v) if [b"OB", b"OW", b"UN", b"UT", b"SQ"].contains(&v) => {
            o.extend_from_slice(v);
            o.extend_from_slice(&[0, 0]);
            p32(o, len);
        }
        Some(v) => {
            o.extend_from_slice(v);
            p16(o, len as u16);
        }
        None => p32(o, len),
    }
    o.extend_from_slice(val);
}

struct Run {
    cond: Condition,
    object: Option<DcmObject>,
    stack: CondStack,
    out: Captured,
    notices: Notices,
}

fn open(path: &Path, options: u64, mode: UpstreamDefects, dialect: &dyn Dialect) -> Run {
    let mut r = Run {
        cond: 0,
        object: None,
        stack: CondStack::new(),
        out: Captured::default(),
        notices: Notices::new(mode),
    };
    r.cond = open_file(
        path,
        options,
        &mut r.object,
        dialect,
        &mut r.stack,
        &mut r.out,
        &mut r.notices,
    )
    .unwrap();
    r
}

fn part10(dataset: &[u8], ts: &[u8]) -> Vec<u8> {
    let mut o = vec![0u8; 128];
    o.extend_from_slice(b"DICM");
    let mut meta = Vec::new();
    el(
        &mut meta,
        0x0002_0010,
        Some(b"UI"),
        ts.len() as u32,
        ts,
        false,
    );
    el(
        &mut o,
        0x0002_0000,
        Some(b"UL"),
        4,
        &(meta.len() as u32).to_le_bytes(),
        false,
    );
    o.extend_from_slice(&meta);
    o.extend_from_slice(dataset);
    o
}

#[test]
fn messages_format_as_c() {
    let m = ctn_message(DCM_ELEMENTLENGTHERROR).unwrap();
    let t = c_format(
        m,
        &[
            Arg::Int(0x10),
            Arg::Int(0x20),
            Arg::U32(8),
            Arg::U32(2),
            Arg::Str(b"readFile"),
        ],
    );
    assert_eq!(
        t,
        b"DCM Data Element (0010 0020) longer (8) than remaining length (2) of data in stream or file in readFile"
    );
    assert_eq!(
        c_format(
            b"%8x|%-4d|%04x|%ld",
            &[
                Arg::Long(0x9_0002),
                Arg::Int(-3),
                Arg::Int(7),
                Arg::U32(u32::MAX)
            ]
        ),
        b"   90002|-3  |0007|4294967295"
    );
    // a long is read by %d as its low 32 bits
    assert_eq!(c_format(b"%d", &[Arg::Long(0x1_0000_0000)]), b"0");
}

#[test]
fn vr_codes_compare_as_c_strings() {
    assert_eq!(Vr::from_code(*b"OB"), Some(Vr::Ob));
    assert_eq!(Vr::from_code(*b"  "), Some(Vr::Ctx));
    assert_eq!(Vr::from_code([0, b'X']), None);
    assert_eq!(Vr::from_code([0x1e, 0]), Some(Vr::Dlm));
    assert_eq!(Vr::from_code([0x1e, b'X']), None);
    assert_eq!(Vr::from_code([b'A', 0]), None);
    assert_eq!(Vr::from_code(*b"XX"), None);
}

#[test]
fn the_stack_keeps_256_bytes_and_overflows_at_98() {
    let mut s = CondStack::new();
    let mut out = Captured::default();
    let mut n = Notices::new(UpstreamDefects::Fix);
    let long = vec![b'x'; 300];
    s.push(
        DCM_FILEOPENFAILED,
        ctn_message(DCM_FILEOPENFAILED),
        &[Arg::Str(&long), Arg::Str(b"f")],
        &mut out,
        &mut n,
    );
    assert_eq!(s.entries()[0].1.len(), 256);
    assert!(n.is_empty());
    for i in 1..98 {
        s.push(
            DCM_NULLOBJECT,
            ctn_message(DCM_NULLOBJECT),
            &[Arg::Str(format!("r{i}").as_bytes())],
            &mut out,
            &mut n,
        );
    }
    assert_eq!(s.entries().len(), 98);
    assert!(out.text(Stream::Err).is_empty());
    s.push(
        DCM_NULLOBJECT,
        ctn_message(DCM_NULLOBJECT),
        &[Arg::Str(b"last")],
        &mut out,
        &mut n,
    );
    let err = out.text(Stream::Err);
    assert!(
        err.ends_with("   50092 NULL object passed to routine last\nCONDITION Stack overflow\n")
    );
    assert_eq!(err.lines().count(), 100);
    // the top is entry 0 again: the first condition pushed
    assert_eq!(s.entries().len(), 1);
    assert_eq!(s.entries()[0].0, DCM_FILEOPENFAILED);
    let (c, code, text) = s.top(10);
    assert_eq!((c, code), (DCM_FILEOPENFAILED, DCM_FILEOPENFAILED));
    assert_eq!(text, b"DCM faile");
    assert_eq!(s.pop(false), DCM_FILEOPENFAILED);
    assert_eq!(s.pop(false), COND_NORMAL);
    // a message of 1,024 bytes or more overflows CTN's buffer
    let huge = vec![b'y'; 1100];
    s.push(
        DCM_FILEOPENFAILED,
        ctn_message(DCM_FILEOPENFAILED),
        &[Arg::Str(&huge), Arg::Str(b"f")],
        &mut out,
        &mut n,
    );
    assert!(n.contains(&larmorx_core::defects::FS_90));
}

#[test]
fn a_part10_file_switches_to_its_transfer_syntax() {
    let d = scratch("p10");
    let mut ds = Vec::new();
    el(&mut ds, 0x0008_0018, None, 4, b"1.2\0", false);
    el(&mut ds, 0x0028_0100, None, 2, &16u16.to_le_bytes(), false);
    el(&mut ds, 0x7fe0_0010, None, 4, &[1, 2, 3, 4], false);
    let f = d.join("a.dcm");
    std::fs::write(&f, part10(&ds, b"1.2.840.10008.1.2\0")).unwrap();
    let r = open(
        &f,
        opt::PART10FILE | opt::ACCEPTVRMISMATCH,
        UpstreamDefects::Fix,
        &Ctn,
    );
    assert_eq!(r.cond, DCM_NORMAL);
    let o = r.object.unwrap();
    assert!(o.preamble_flag);
    assert_eq!(o.data_options, opt::ORDERLITTLEENDIAN);
    assert_eq!(o.pixel_bits_allocated, 16);
    assert_eq!(
        o.groups.iter().map(|g| g.group).collect::<Vec<_>>(),
        [2, 8, 0x28, 0x7fe0]
    );
    let px = &o.groups[3].elements[0];
    assert_eq!(
        (px.representation, &px.data, px.data_offset),
        (Vr::Ow, &Data::OnDisk, 200)
    );
    assert!(o.fd.is_some());
    // the group length is recomputed from the elements
    let gl = &o.groups[0].elements[0];
    assert_eq!(gl.data, Data::Bytes(26u32.to_le_bytes().to_vec()));
    assert!(r.stack.entries().is_empty());
    // the same file as a bare data set fails: its preamble's zeros are (0000,0000) elements of
    // length 0, the second out of order
    let r = open(
        &f,
        opt::ORDERLITTLEENDIAN | opt::ACCEPTVRMISMATCH,
        UpstreamDefects::Fix,
        &Ctn,
    );
    assert_eq!(r.cond, DCM_FILEOPENFAILED);
    assert!(r.object.is_some());
    assert_eq!(
        r.stack.entries()[0].1,
        b"DCM group/element out of order (0000 0000) in checkAttributeOrder"
    );
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn the_format_probe_sequence_on_a_nifti_header() {
    let d = scratch("nii");
    let mut h = vec![0u8; 352];
    h[0..4].copy_from_slice(&348i32.to_le_bytes());
    h[344..348].copy_from_slice(b"n+1\0");
    let f = d.join("x.nii");
    std::fs::write(&f, &h).unwrap();
    let name = f.to_string_lossy().into_owned();
    let mut stack = CondStack::new();
    let mut out = Captured::default();
    let mut n = Notices::new(UpstreamDefects::Fix);
    let mut object = None;
    let mut conds = Vec::new();
    for o in [
        opt::PART10FILE,
        opt::ORDERLITTLEENDIAN,
        opt::ORDERBIGENDIAN,
        opt::FORMATCONVERSION,
    ] {
        let c = open_file(
            &f,
            o | opt::ACCEPTVRMISMATCH,
            &mut object,
            &Ctn,
            &mut stack,
            &mut out,
            &mut n,
        )
        .unwrap();
        conds.push(c);
        close_object(&mut object, &Ctn, &mut stack, &mut out, &mut n);
    }
    assert_eq!(
        conds,
        [0, 1, 2, 3].map(|i| if i == 3 {
            DCM_ILLEGALOPTION
        } else {
            DCM_FILEOPENFAILED
        })
    );
    let texts: Vec<String> = stack
        .entries()
        .iter()
        .map(|(_, t)| String::from_utf8_lossy(t).into_owned())
        .collect();
    // 348 read as a tag is (015c,0000) (little endian) or (5c01,0000) (big): a group length of
    // length 0, then the zeros make (0000,0000), out of order; the object stays open
    let order = "DCM group/element out of order (0000 0000) in checkAttributeOrder".to_owned();
    let failed = format!("DCM failed to open file: {name} in DCM_OpenFile");
    assert_eq!(
        texts,
        [
            failed.clone(),
            order.clone(),
            failed.clone(),
            order,
            failed,
            "DCM illegal stream option: Byte order".to_owned(),
            "NULL object passed to routine DCM_CloseObject".to_owned(),
        ]
    );
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn an_undefined_length_that_is_no_sequence() {
    let d = scratch("fs91");
    let mut o = Vec::new();
    el(&mut o, 0x0008_0018, None, 4, b"1.2\0", false);
    // Patient's Name of undefined length holding an item: read as a sequence (fix), or as CTN
    // does: the value never read, the item's tags read as elements (reproduce)
    let mut item = Vec::new();
    el(&mut item, 0x0010_0020, None, 2, b"42", false);
    el(&mut o, 0x0010_0010, None, 0xffff_ffff, &[], false);
    el(&mut o, 0xfffe_e000, None, item.len() as u32, &item, false);
    el(&mut o, 0xfffe_e0dd, None, 0, &[], false);
    el(&mut o, 0x0020_0010, None, 2, b"7 ", false);
    let f = d.join("u.dcm");
    std::fs::write(&f, &o).unwrap();
    let r = open(&f, opt::ORDERLITTLEENDIAN, UpstreamDefects::Fix, &Ctn);
    assert_eq!(r.cond, DCM_NORMAL);
    assert!(r.notices.contains(&larmorx_core::defects::FS_91));
    let ob = r.object.unwrap();
    let pn = &ob.groups[1].elements[0];
    assert_eq!(pn.representation, Vr::Sq);
    assert!(matches!(&pn.data, Data::Sequence(items) if items.len() == 1));
    assert_eq!(ob.groups.last().unwrap().group, 0x0020);
    let r = open(&f, opt::ORDERLITTLEENDIAN, UpstreamDefects::Reproduce, &Ctn);
    assert_eq!(r.cond, DCM_NORMAL);
    let ob = r.object.unwrap();
    assert_eq!(ob.groups[1].elements[0].data, Data::Unread);
    // the item tag ends the reading at its delimiter: (0020,0010) never read
    assert_eq!(
        ob.groups.iter().map(|g| g.group).collect::<Vec<_>>(),
        [0x0008, 0x0010, 0xfffe]
    );
    assert_eq!(r.stack.errno, EINVAL);
    std::fs::remove_dir_all(&d).ok();
}

struct PairSwap;
impl Dialect for PairSwap {
    fn swap_4byte_in_pairs(&self) -> bool {
        true
    }
}

#[test]
fn big_endian_values() {
    let d = scratch("be");
    let mut o = Vec::new();
    el(&mut o, 0x0008_0018, None, 4, b"1.2\0", true);
    el(&mut o, 0x0018_0050, None, 8, &2.5f64.to_be_bytes(), true);
    el(
        &mut o,
        0x0018_6020,
        None,
        4,
        &0x0102_0304u32.to_be_bytes(),
        true,
    );
    el(&mut o, 0x0028_0010, None, 2, &0x0506u16.to_be_bytes(), true);
    let f = d.join("b.dcm");
    std::fs::write(&f, &o).unwrap();
    let value = |r: &Run, g: usize, e: usize| match &r.object.as_ref().unwrap().groups[g].elements
        [e]
        .data
    {
        Data::Bytes(b) => b.clone(),
        _ => panic!(),
    };
    // (0018,0050) is DS in CTN's dictionary: the FD bytes are kept as read
    let r = open(&f, opt::ORDERBIGENDIAN, UpstreamDefects::Fix, &Ctn);
    assert_eq!(r.cond, DCM_NORMAL);
    assert_eq!(value(&r, 1, 0), 2.5f64.to_be_bytes());
    // (0018,6020) Reference Pixel X0 is SL, (0028,0010) Rows US
    assert_eq!(value(&r, 1, 1), 0x0102_0304u32.to_le_bytes());
    assert_eq!(value(&r, 2, 0), 0x0506u16.to_le_bytes());
    let r = open(
        &f,
        opt::ORDERBIGENDIAN,
        UpstreamDefects::Reproduce,
        &PairSwap,
    );
    assert_eq!(value(&r, 1, 1), [0x02, 0x01, 0x04, 0x03]);
    assert!(r.notices.contains(&larmorx_core::defects::FS_94));
    let r = open(&f, opt::ORDERBIGENDIAN, UpstreamDefects::Fix, &PairSwap);
    assert_eq!(value(&r, 1, 1), 0x0102_0304u32.to_le_bytes());
    // an FD element: (0018,602c) Physical Delta X
    let mut o = Vec::new();
    el(&mut o, 0x0008_0018, None, 4, b"1.2\0", true);
    el(&mut o, 0x0018_602c, None, 8, &1000.0f64.to_be_bytes(), true);
    std::fs::write(&f, &o).unwrap();
    let r = open(&f, opt::ORDERBIGENDIAN, UpstreamDefects::Fix, &Ctn);
    assert_eq!(value(&r, 1, 0), 1000.0f64.to_le_bytes());
    assert!(r.notices.contains(&larmorx_core::defects::FS_96));
    let r = open(&f, opt::ORDERBIGENDIAN, UpstreamDefects::Reproduce, &Ctn);
    assert_eq!(value(&r, 1, 0), 1000.0f64.to_be_bytes());
    std::fs::remove_dir_all(&d).ok();
}

fn nested(depth: usize) -> Vec<u8> {
    let mut inner = Vec::new();
    el(&mut inner, 0x0008_0018, None, 4, b"1.2\0", false);
    for _ in 0..depth {
        let mut item = Vec::new();
        el(&mut item, 0xfffe_e000, None, 0xffff_ffff, &inner, false);
        el(&mut item, 0xfffe_e00d, None, 0, &[], false);
        el(&mut item, 0xfffe_e0dd, None, 0, &[], false);
        let mut sq = Vec::new();
        el(&mut sq, 0x0008_1140, None, 0xffff_ffff, &item, false);
        inner = sq;
    }
    inner
}

#[test]
fn sequences_nest_to_the_limit() {
    let d = scratch("nest");
    let f = d.join("n.dcm");
    std::fs::write(&f, nested(read::MAX_NESTING as usize)).unwrap();
    let r = open(&f, opt::ORDERLITTLEENDIAN, UpstreamDefects::Fix, &Ctn);
    assert_eq!(r.cond, DCM_NORMAL);
    assert!(r.notices.is_empty());
    std::fs::write(&f, nested(read::MAX_NESTING as usize + 1)).unwrap();
    let r = open(&f, opt::ORDERLITTLEENDIAN, UpstreamDefects::Fix, &Ctn);
    assert_eq!(r.cond, DCM_FILEOPENFAILED);
    assert!(r.notices.contains(&larmorx_core::defects::FS_97));
    assert_eq!(
        r.stack.entries()[0].1,
        b"DCM Failed to read stream in readSequence"
    );
    std::fs::remove_dir_all(&d).ok();
}

/// FNV-1a over a rendering of everything the reader gives.
struct Digest(u64);
impl Digest {
    fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 = (self.0 ^ x as u64).wrapping_mul(0x100_0000_01b3);
        }
    }
    fn u64(&mut self, v: u64) {
        self.bytes(&v.to_le_bytes());
    }
    fn object(&mut self, o: &DcmObject) {
        for v in [
            o.object_type as u64,
            o.object_size,
            o.offset,
            o.pixel_bits_allocated as u64,
            o.pixel_representation as u64,
        ] {
            self.u64(v);
        }
        for v in [
            o.fd.is_some() as u64,
            o.preamble_flag as u64,
            o.data_options,
            o.meta_header_length,
            o.long_vr_attributes as u64,
        ] {
            self.u64(v);
        }
        self.bytes(&o.preamble);
        self.bytes(&o.waveform_data_vr);
        for g in &o.groups {
            self.u64(g.group as u64);
            self.u64(g.base_length);
            for e in &g.elements {
                self.u64(e.tag as u64);
                self.u64(e.representation as u64);
                self.u64(e.multiplicity);
                self.u64(e.length as u64);
                self.bytes(e.description.as_bytes());
                for v in [
                    e.data_offset as u64,
                    e.original_data_length,
                    e.padded_data_length,
                    e.allocated_data_length,
                ] {
                    self.u64(v);
                }
                match &e.data {
                    Data::Bytes(b) => self.bytes(b),
                    Data::OnDisk => self.u64(1),
                    Data::Unread => self.u64(2),
                    Data::Sequence(items) => {
                        for it in items {
                            self.object(it);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn digest_of_generated_files() {
    // 300 files: part 10 and bare data sets, both byte orders, sequences, pixel data, cuts and
    // flipped bytes; each opened as the format probe does (four options, a close after each)
    let d = scratch("digest");
    let mut seed = 0x2026_1113u64;
    let mut next = move || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 33) as u32
    };
    let tags: Vec<(u32, u32)> = dict::entries()
        .filter(|e| e.tag >> 16 >= 8 && e.tag >> 16 < 0x7fe0)
        .map(|e| (e.tag, e.vr))
        .collect();
    let mut h = Digest(0xcbf2_9ce4_8422_2325);
    for i in 0..300 {
        let big = i % 5 == 3;
        let mut ds = Vec::new();
        let mut picked: Vec<(u32, u32)> = (0..next() % 25)
            .map(|_| tags[next() as usize % tags.len()])
            .collect();
        picked.sort();
        picked.dedup_by_key(|x| x.0);
        for (t, vr) in picked {
            if vr == Vr::Sq as u32 {
                let mut item = Vec::new();
                el(&mut item, 0x0008_0100, None, 4, b"AB12", big);
                let mut body = Vec::new();
                el(&mut body, 0xfffe_e000, None, item.len() as u32, &item, big);
                el(&mut ds, t, None, body.len() as u32, &body, big);
            } else {
                let n = 2 * (next() % 9) as usize;
                let v: Vec<u8> = (0..n).map(|_| next() as u8).collect();
                el(&mut ds, t, None, n as u32, &v, big);
            }
        }
        if i % 3 == 0 {
            el(&mut ds, 0x7fe0_0010, None, 8, &[9; 8], big);
        }
        let mut f = if i % 2 == 0 {
            part10(&ds, b"1.2.840.10008.1.2\0")
        } else {
            ds
        };
        match i % 7 {
            0 if !f.is_empty() => {
                let n = next() as usize % f.len();
                f.truncate(n);
            }
            1 if !f.is_empty() => {
                let n = next() as usize % f.len();
                f[n] ^= 0x41;
            }
            _ => {}
        }
        let p = d.join(format!("f{i}.dcm"));
        std::fs::write(&p, &f).unwrap();
        let name = p.to_string_lossy().into_owned();
        let mut stack = CondStack::new();
        let mut out = Captured::default();
        let mut notices = Notices::new(UpstreamDefects::Fix);
        let mut object = None;
        for o in [
            opt::PART10FILE,
            opt::ORDERLITTLEENDIAN,
            opt::ORDERBIGENDIAN,
            opt::FORMATCONVERSION,
        ] {
            let c = open_file(
                &p,
                o | opt::ACCEPTVRMISMATCH,
                &mut object,
                &Ctn,
                &mut stack,
                &mut out,
                &mut notices,
            )
            .unwrap();
            h.u64(c);
            if let Some(ob) = &object {
                h.object(ob);
            }
            h.u64(close_object(
                &mut object,
                &Ctn,
                &mut stack,
                &mut out,
                &mut notices,
            ));
        }
        for (c, t) in stack.entries() {
            h.u64(*c);
            h.bytes(
                String::from_utf8_lossy(t)
                    .replace(&name, "<name>")
                    .as_bytes(),
            );
        }
        h.bytes(out.text(Stream::Out).as_bytes());
        h.bytes(out.text(Stream::Err).replace(&name, "<name>").as_bytes());
        for n in notices.as_slice() {
            h.bytes(n.defect.id.as_bytes());
        }
    }
    std::fs::remove_dir_all(&d).ok();
    assert_eq!(
        h.0, 0x3028_c185_b5ef_abd5,
        "digest of the generated files: {:#018x}",
        h.0
    );
}
