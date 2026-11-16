// SPDX-License-Identifier: Apache-2.0
//! Unit cases of `DCM_GetElement` and `DCM_GetElementValue` (the probe `validation/probes/fs-dicom`
//! compares them with CTN 3.0.6 and FreeSurfer's copy on generated files, in `reproduce`; these keep
//! the rules of both modes and the digest of a fixed set of requests, recorded on Linux x86-64, so
//! every platform is checked against it).

use super::cond::*;
use super::tests::{Run, el, open, part10, scratch};
use super::*;
use larmorx_core::cfmt::{Captured, Stream};
use larmorx_core::defects::{FS_91, FS_951, FS_952, FS_953, Notices, UpstreamDefects};
use std::path::Path;

const PIXEL: Tag = TAG_PIXEL_DATA;
const TS_LE: &[u8] = b"1.2.840.10008.1.2.1\0";
const TS_BE: &[u8] = b"1.2.840.10008.1.2.2\0";

fn get(r: &mut Run, tag: Tag) -> Result<Element, Condition> {
    get_element(
        r.object.as_ref(),
        tag,
        &Ctn,
        &mut r.stack,
        &mut r.out,
        &mut r.notices,
    )
}

fn value(r: &mut Run, e: &Element, ctx: &mut u32) -> ValueRead {
    get_element_value(
        r.object.as_ref(),
        e,
        ctx,
        &Ctn,
        &mut r.stack,
        &mut r.out,
        &mut r.notices,
    )
}

fn top(r: &Run) -> String {
    String::from_utf8_lossy(&r.stack.entries().last().unwrap().1).into_owned()
}

/// A part 10 file in explicit VR of the given order: Manufacturer, Bits Allocated, a sequence of
/// one item, pixel data of 12 bytes (or encapsulated: two fragments).
fn sample(big: bool, bits: u16, encapsulated: bool) -> Vec<u8> {
    let mut o = Vec::new();
    el(&mut o, 0x0008_0018, Some(b"UI"), 4, b"1.2\0", big);
    el(&mut o, 0x0008_0070, Some(b"LO"), 8, b"SIEMENS ", big);
    let mut item = Vec::new();
    el(&mut item, 0x0008_0100, Some(b"SH"), 4, b"AB12", big);
    let mut body = Vec::new();
    el(&mut body, 0xfffe_e000, None, item.len() as u32, &item, big);
    el(
        &mut o,
        0x0008_1140,
        Some(b"SQ"),
        body.len() as u32,
        &body,
        big,
    );
    let b = if big {
        bits.to_be_bytes()
    } else {
        bits.to_le_bytes()
    };
    el(&mut o, 0x0028_0100, Some(b"US"), 2, &b, big);
    if encapsulated {
        let mut px = Vec::new();
        el(&mut px, 0xfffe_e000, None, 0, &[], big);
        el(&mut px, 0xfffe_e000, None, 6, &[9, 8, 7, 6, 5, 4], big);
        el(&mut px, 0xfffe_e0dd, None, 0, &[], big);
        el(&mut o, PIXEL, Some(b"OB"), 0xffff_ffff, &px, big);
    } else {
        let px: Vec<u8> = (1..=12).collect();
        let vr = if bits == 8 { b"OB" } else { b"OW" };
        el(&mut o, PIXEL, Some(vr), 12, &px, big);
    }
    part10(&o, if big { TS_BE } else { TS_LE })
}

fn opened(d: &Path, name: &str, bytes: &[u8], mode: UpstreamDefects) -> Run {
    let f = d.join(name);
    std::fs::write(&f, bytes).unwrap();
    let r = open(&f, opt::PART10FILE, mode, &Ctn);
    assert_eq!(r.cond, DCM_NORMAL);
    r
}

#[test]
fn an_element_and_its_value_whole_and_in_pieces() {
    let d = scratch("el-basic");
    let mut r = opened(&d, "a.dcm", &sample(false, 16, false), UpstreamDefects::Fix);
    let e = get(&mut r, 0x0008_0070).unwrap();
    assert_eq!(
        (e.vr(), e.length, e.multiplicity, e.description.as_str()),
        (Some(Vr::Lo), 8, 1, "ID Manufacturer")
    );
    let mut ctx = 0;
    let v = value(&mut r, &e, &mut ctx);
    assert_eq!(
        (v.cond, v.rtn_length, v.data.as_slice(), ctx),
        (DCM_NORMAL, Some(8), &b"SIEMENS "[..], 8)
    );
    // at the end: nothing more, and complete
    let v = value(&mut r, &e, &mut ctx);
    assert_eq!(
        (v.cond, v.rtn_length, v.data.len()),
        (DCM_NORMAL, Some(0), 0)
    );
    // in pieces of 3: incomplete (no condition pushed) until the end
    let piece = Element {
        length: 3,
        ..e.clone()
    };
    let mut ctx = 0;
    let mut got = Vec::new();
    let mut conds = Vec::new();
    for _ in 0..3 {
        let v = value(&mut r, &piece, &mut ctx);
        conds.push(v.cond);
        got.extend_from_slice(&v.data);
    }
    assert_eq!(conds, [DCM_GETINCOMPLETE, DCM_GETINCOMPLETE, DCM_NORMAL]);
    assert_eq!(got, b"SIEMENS ");
    assert!(r.stack.entries().is_empty());
    // a context past the value
    let mut ctx = 9;
    let v = value(&mut r, &e, &mut ctx);
    assert_eq!((v.cond, v.rtn_length), (DCM_ILLEGALCONTEXT, None));
    assert_eq!(top(&r), "DCM Illegal context value in DCM_GetElementValue");
    // pixel data left in the file, read through the object's descriptor
    let px = get(&mut r, PIXEL).unwrap();
    assert_eq!((px.vr(), px.length), (Some(Vr::Ow), 12));
    let mut ctx = 0;
    let v = value(&mut r, &px, &mut ctx);
    assert_eq!(v.data, (1..=12).collect::<Vec<u8>>());
    assert_eq!(v.cond, DCM_NORMAL);
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn the_conditions_and_their_messages() {
    let d = scratch("el-cond");
    let mut r = opened(&d, "a.dcm", &sample(false, 16, false), UpstreamDefects::Fix);
    assert_eq!(get(&mut r, 0x0008_0072), Err(DCM_ELEMENTNOTFOUND));
    assert_eq!(
        top(&r),
        "Requested element (8 72) not found in DCM_GetElementVM"
    );
    assert_eq!(get(&mut r, 0x0009_0010), Err(DCM_ELEMENTNOTFOUND));
    let missing = Element {
        tag: 0x0010_0010,
        length: 4,
        ..Element::default()
    };
    let v = value(&mut r, &missing, &mut 0);
    assert_eq!(v.cond, DCM_ELEMENTNOTFOUND);
    assert_eq!(
        top(&r),
        "Requested element (10 10) not found in DCM_GetElementValue"
    );
    // a request that says SQ, and the sequence itself
    let sq = get(&mut r, 0x0008_1140).unwrap();
    assert_eq!(sq.vr(), Some(Vr::Sq));
    let v = value(&mut r, &sq, &mut 0);
    assert_eq!(v.cond, DCM_CANNOTGETSEQUENCEVALUE);
    assert_eq!(
        top(&r),
        "DCM Cannot retrieve value of element with SQ representation (00081140) in (DCM_GetElementValue)"
    );
    // no object
    let mut stack = CondStack::new();
    let mut out = Captured::default();
    let mut n = Notices::new(UpstreamDefects::Fix);
    assert_eq!(
        get_element(None, PIXEL, &Ctn, &mut stack, &mut out, &mut n),
        Err(DCM_NULLOBJECT)
    );
    let v = get_element_value(None, &sq, &mut 0, &Ctn, &mut stack, &mut out, &mut n);
    assert_eq!(v.cond, DCM_NULLOBJECT);
    let texts: Vec<_> = stack.entries().iter().map(|(_, t)| t.clone()).collect();
    assert_eq!(
        texts,
        [
            &b"NULL object passed to routine DCM_GetElementVM"[..],
            b"NULL object passed to routine DCM_GetElementValue"
        ]
    );
    // an illegal object
    let mut o = r.object.clone().unwrap();
    o.key_type = "something else".into();
    let v = get_element_value(Some(&o), &sq, &mut 0, &Ctn, &mut stack, &mut out, &mut n);
    assert_eq!(v.cond, DCM_ILLEGALOBJECT);
    assert!(out.text(Stream::Err).is_empty());
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn a_group_that_holds_no_element_shows_the_group_twice() {
    // FS-894 (kept): a group whose only element is its group length, read with NOGROUPLENGTH
    let d = scratch("el-empty");
    let mut o = Vec::new();
    el(&mut o, 0x0008_0000, None, 4, &4u32.to_le_bytes(), false);
    el(&mut o, 0x0010_0010, None, 4, b"DOE ", false);
    let f = d.join("g.dcm");
    std::fs::write(&f, &o).unwrap();
    let mut r = open(
        &f,
        opt::ORDERLITTLEENDIAN | opt::NOGROUPLENGTH,
        UpstreamDefects::Fix,
        &Ctn,
    );
    assert_eq!(r.cond, DCM_NORMAL);
    assert_eq!(get(&mut r, 0x0008_0070), Err(DCM_ELEMENTNOTFOUND));
    assert_eq!(
        top(&r),
        "Requested element (8 70) not found in DCM_GetElementVM"
    );
    let e = Element {
        tag: 0x0008_0070,
        length: 8,
        ..Element::default()
    };
    assert_eq!(value(&mut r, &e, &mut 0).cond, DCM_ELEMENTNOTFOUND);
    assert_eq!(
        top(&r),
        "Requested element (8 8) not found in DCM_GetElementValue"
    );
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn big_endian_pixel_data_are_swapped_as_read() {
    let d = scratch("el-be");
    let mut r = opened(&d, "w.dcm", &sample(true, 16, false), UpstreamDefects::Fix);
    let px = get(&mut r, PIXEL).unwrap();
    let mut ctx = 0;
    let v = value(&mut r, &px, &mut ctx);
    assert_eq!(v.data, [2, 1, 4, 3, 6, 5, 8, 7, 10, 9, 12, 11]);
    // a piece starting at an odd offset is swapped from its own start
    let mut ctx = 1;
    let v = value(
        &mut r,
        &Element {
            length: 4,
            ..px.clone()
        },
        &mut ctx,
    );
    assert_eq!(v.data, [3, 2, 5, 4]);
    // FS-953: an odd count, the whole pairs swapped and the last byte kept
    let mut ctx = 0;
    let v = value(
        &mut r,
        &Element {
            length: 5,
            ..px.clone()
        },
        &mut ctx,
    );
    assert_eq!(v.data, [2, 1, 4, 3, 5]);
    assert!(r.notices.contains(&FS_953));
    // 8-bit pixel data are left as they are
    let mut r = opened(&d, "b.dcm", &sample(true, 8, false), UpstreamDefects::Fix);
    let px = get(&mut r, PIXEL).unwrap();
    assert_eq!(px.vr(), Some(Vr::Ob));
    let v = value(&mut r, &px, &mut 0);
    assert_eq!(v.data, (1..=12).collect::<Vec<u8>>());
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn encapsulated_pixel_data_asked_for_whole() {
    // FS-952: 4,294,967,295 bytes read from the file: the read fails; CTN takes it for complete
    let d = scratch("el-enc");
    for (mode, cond) in [
        (UpstreamDefects::Fix, DCM_FILEACCESSERROR),
        (UpstreamDefects::Reproduce, DCM_NORMAL),
    ] {
        let mut r = opened(&d, "e.dcm", &sample(false, 16, true), mode);
        let px = get(&mut r, PIXEL).unwrap();
        assert_eq!(px.length, UNSPECIFIED_LENGTH);
        let mut ctx = 0;
        let v = value(&mut r, &px, &mut ctx);
        assert_eq!(
            (v.cond, v.rtn_length, v.data.len()),
            (cond, Some(u32::MAX), 0)
        );
        assert!(r.notices.contains(&FS_952));
        assert_eq!(r.stack.errno, EFAULT);
        // the fragments' bytes as they are in the file, from the pixel data's value on
        let mut ctx = 0;
        let v = value(
            &mut r,
            &Element {
                length: 10,
                ..px.clone()
            },
            &mut ctx,
        );
        assert_eq!(v.data, [0xfe, 0xff, 0x00, 0xe0, 0, 0, 0, 0, 0xfe, 0xff]);
        assert_eq!(v.cond, DCM_GETINCOMPLETE);
        // past the file's end, and a count of 2 GiB or more: the read fails
        let v = value(
            &mut r,
            &Element {
                length: 0x7fff_ffff,
                ..px.clone()
            },
            &mut 0,
        );
        assert_eq!((v.cond, v.data.len()), (DCM_FILEACCESSERROR, 30));
        let v = value(
            &mut r,
            &Element {
                length: 0x8000_0000,
                ..px.clone()
            },
            &mut 0,
        );
        assert_eq!((v.cond, v.data.len()), (DCM_FILEACCESSERROR, 0));
    }
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn a_sequence_asked_for_as_another_representation() {
    // FS-951: refused (fix); CTN copies its list's memory (reproduce: those bytes not given)
    let d = scratch("el-sq");
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let mut r = opened(&d, "s.dcm", &sample(false, 16, false), mode);
        let sq = get(&mut r, 0x0008_1140).unwrap();
        let ask = Element {
            representation: Vr::Ob as u32,
            length: 6,
            ..sq.clone()
        };
        let mut ctx = 0;
        let v = value(&mut r, &ask, &mut ctx);
        assert!(r.notices.contains(&FS_951));
        if mode == UpstreamDefects::Fix {
            assert_eq!(
                (v.cond, v.rtn_length, ctx),
                (DCM_CANNOTGETSEQUENCEVALUE, None, 0)
            );
        } else {
            assert_eq!(
                (v.cond, v.rtn_length, v.data.len(), v.unknown, ctx),
                (DCM_GETINCOMPLETE, Some(6), 0, 6, 6)
            );
        }
    }
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn a_value_never_read_and_a_closed_descriptor() {
    let d = scratch("el-unread");
    // FS-91 in reproduce: the value of undefined length kept unread
    let mut o = Vec::new();
    el(&mut o, 0x0008_0018, None, 4, b"1.2\0", false);
    let mut item = Vec::new();
    el(&mut item, 0x0010_0020, None, 2, b"42", false);
    el(&mut o, 0x0010_0010, None, 0xffff_ffff, &[], false);
    el(&mut o, 0xfffe_e000, None, item.len() as u32, &item, false);
    el(&mut o, 0xfffe_e0dd, None, 0, &[], false);
    let f = d.join("u.dcm");
    std::fs::write(&f, &o).unwrap();
    let mut r = open(&f, opt::ORDERLITTLEENDIAN, UpstreamDefects::Reproduce, &Ctn);
    assert_eq!(r.cond, DCM_NORMAL);
    let pn = get(&mut r, 0x0010_0010).unwrap();
    let v = value(&mut r, &Element { length: 4, ..pn }, &mut 0);
    assert_eq!((v.cond, v.data.len(), v.unknown), (DCM_GETINCOMPLETE, 0, 4));
    assert!(r.notices.contains(&FS_91));
    // a failure after the pixel data keeps the object and closes its descriptor
    let mut o = Vec::new();
    el(&mut o, 0x0008_0018, Some(b"UI"), 4, b"1.2\0", false);
    el(&mut o, PIXEL, Some(b"OW"), 4, &[1, 2, 3, 4], false);
    o.extend_from_slice(&[0xe0, 0x7f, 0x20, 0x00, b'Z', b'Z', 2, 0, 0, 0]);
    let f = d.join("k.dcm");
    std::fs::write(&f, part10(&o, TS_LE)).unwrap();
    let mut r = open(&f, opt::PART10FILE, UpstreamDefects::Fix, &Ctn);
    assert_eq!(r.cond, DCM_FILEOPENFAILED);
    let px = get(&mut r, PIXEL).unwrap();
    for length in [4, 0] {
        let v = value(
            &mut r,
            &Element {
                length,
                ..px.clone()
            },
            &mut 0,
        );
        assert_eq!((v.cond, v.rtn_length), (DCM_FILEACCESSERROR, Some(length)));
        assert_eq!(r.stack.errno, EBADF);
    }
    let t = top(&r);
    assert!(
        t.starts_with("DCM failed to access file: ") && t.ends_with("k.dcm in DCM_GetElementValue")
    );
    std::fs::remove_dir_all(&d).ok();
}

/// FNV-1a.
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
}

#[test]
fn digest_of_element_requests() {
    // 240 files (part 10 in both byte orders and bare data sets, sequences, native and
    // encapsulated pixel data, cuts and flipped bytes), opened as FreeSurfer's reader opens them
    // (part 10, then little endian), every element asked for whole, in pieces of 3, past its end
    // and as another representation, in both modes
    let d = scratch("el-digest");
    let mut seed = 0x2026_1116u64;
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
    for i in 0..240 {
        let big = i % 4 == 1;
        let explicit = i % 2 == 0;
        let mut ds = Vec::new();
        let mut picked: Vec<(u32, u32)> = (0..next() % 16)
            .map(|_| tags[next() as usize % tags.len()])
            .collect();
        picked.push((0x0028_0100, Vr::Us as u32));
        picked.sort();
        picked.dedup_by_key(|x| x.0);
        let code = |vr: u32| -> [u8; 2] {
            let c = Vr::from_number(vr)
                .and_then(|v| Vr::code_table().iter().find(|(x, _)| *x == v))
                .map(|(_, c)| *c)
                .unwrap_or(b"UN");
            if c.len() == 2 { [c[0], c[1]] } else { *b"UN" }
        };
        for (t, vr) in picked {
            let c = code(vr);
            let vr_opt = explicit.then_some(&c);
            if vr == Vr::Sq as u32 {
                let mut item = Vec::new();
                el(
                    &mut item,
                    0x0008_0100,
                    explicit.then_some(b"SH"),
                    4,
                    b"AB12",
                    big,
                );
                let mut body = Vec::new();
                el(&mut body, 0xfffe_e000, None, item.len() as u32, &item, big);
                el(&mut ds, t, vr_opt, body.len() as u32, &body, big);
            } else if t == 0x0028_0100 {
                let bits: u16 = if next() % 2 == 0 { 16 } else { 8 };
                let b = if big {
                    bits.to_be_bytes()
                } else {
                    bits.to_le_bytes()
                };
                el(&mut ds, t, vr_opt, 2, &b, big);
            } else {
                let n = 2 * (next() % 9) as usize;
                let v: Vec<u8> = (0..n).map(|_| next() as u8).collect();
                el(&mut ds, t, vr_opt, n as u32, &v, big);
            }
        }
        match i % 3 {
            0 => {
                let v: Vec<u8> = (0..10).map(|_| next() as u8).collect();
                el(&mut ds, PIXEL, explicit.then_some(b"OW"), 10, &v, big);
            }
            1 => {
                let mut px = Vec::new();
                el(&mut px, 0xfffe_e000, None, 4, &[1, 2, 3, 4], big);
                el(&mut px, 0xfffe_e0dd, None, 0, &[], big);
                el(
                    &mut ds,
                    PIXEL,
                    explicit.then_some(b"OB"),
                    u32::MAX,
                    &px,
                    big,
                );
            }
            _ => {}
        }
        let mut f = if explicit {
            part10(&ds, if big { TS_BE } else { TS_LE })
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
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let mut stack = CondStack::new();
            let mut out = Captured::default();
            let mut notices = Notices::new(mode);
            let mut object = None;
            for (k, o) in [opt::PART10FILE, opt::ORDERLITTLEENDIAN]
                .into_iter()
                .enumerate()
            {
                if k > 0 {
                    let _ = close_object(&mut object, &Ctn, &mut stack, &mut out, &mut notices);
                }
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
                if c == DCM_NORMAL {
                    break;
                }
            }
            let mut asked: Vec<Tag> = object
                .iter()
                .flat_map(|o| {
                    o.groups
                        .iter()
                        .flat_map(|g| g.elements.iter().map(|e| e.tag))
                })
                .collect();
            asked.extend_from_slice(&[0x0008_0070, 0x0009_1001]);
            for t in asked {
                let e =
                    match get_element(object.as_ref(), t, &Ctn, &mut stack, &mut out, &mut notices)
                    {
                        Ok(e) => e,
                        Err(c) => {
                            h.u64(c);
                            Element {
                                tag: t,
                                length: 4,
                                ..Element::default()
                            }
                        }
                    };
                h.u64(e.representation as u64);
                h.u64(e.length as u64);
                h.u64(e.multiplicity);
                h.bytes(e.description.as_bytes());
                let asks = [
                    (e.clone(), 0u32),
                    (
                        Element {
                            length: 3,
                            ..e.clone()
                        },
                        0,
                    ),
                    (
                        Element {
                            length: 3,
                            ..e.clone()
                        },
                        3,
                    ),
                    (e.clone(), e.length.wrapping_add(1)),
                    (
                        Element {
                            representation: Vr::Ob as u32,
                            length: 5,
                            ..e.clone()
                        },
                        0,
                    ),
                ];
                for (ask, start) in asks {
                    let mut ctx = start;
                    let v = get_element_value(
                        object.as_ref(),
                        &ask,
                        &mut ctx,
                        &Ctn,
                        &mut stack,
                        &mut out,
                        &mut notices,
                    );
                    h.u64(v.cond);
                    h.u64(v.rtn_length.map_or(u64::MAX, |l| l as u64));
                    h.bytes(&v.data);
                    h.u64(v.unknown as u64);
                    h.u64(ctx as u64);
                }
            }
            let _ = close_object(&mut object, &Ctn, &mut stack, &mut out, &mut notices);
            for (c, t) in stack.entries() {
                h.u64(*c);
                h.bytes(
                    String::from_utf8_lossy(t)
                        .replace(&name, "<name>")
                        .as_bytes(),
                );
            }
            h.bytes(out.text(Stream::Err).replace(&name, "<name>").as_bytes());
            for n in notices.as_slice() {
                h.bytes(n.defect.id.as_bytes());
            }
        }
    }
    std::fs::remove_dir_all(&d).ok();
    assert_eq!(
        h.0, 0x1319_78a0_9fff_2d6e,
        "digest of the element requests: {:#018x}",
        h.0
    );
}
