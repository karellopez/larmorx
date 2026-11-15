// SPDX-License-Identifier: Apache-2.0
//! The NRRD reader on hand-made files (the probe `validation/probes/fs-nrrd-minc` compares it
//! with NrrdIO on generated ones).

use super::*;
use larmorx_core::defects::{Notices, UpstreamDefects};

fn write(dir: &std::path::Path, name: &str, bytes: &[u8]) -> Vec<u8> {
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    p.to_string_lossy().into_owned().into_bytes()
}

fn read(name: &[u8], mode: UpstreamDefects) -> (Result<Nrrd, LoadError>, Vec<u8>, Notices) {
    let mut biff = Biff::new();
    let mut n = Notices::new(mode);
    let r = load(name, &mut biff, &mut n);
    let msg = biff.get_done(NRRD).unwrap_or_default();
    (r, msg, n)
}

#[test]
fn raw_float_with_space() {
    let dir = tempfile::tempdir().unwrap();
    let mut f =
        b"NRRD0004\n# a comment\ntype: float\ndimension: 3\nspace: left-posterior-superior\n\
sizes: 2 1 1\nspace directions: (1,0,0) (0,2,0) (0,0,3)\nkinds: domain domain domain\n\
endian: big\nencoding: raw\nspace origin: (-1.5,2,3e1)\nkey:=va\\nlue\n\n"
            .to_vec();
    f.extend_from_slice(&1.5f32.to_be_bytes());
    f.extend_from_slice(&(-2.0f32).to_be_bytes());
    let name = write(dir.path(), "a.nrrd", &f);
    let (r, msg, n) = read(&name, UpstreamDefects::Fix);
    let nrrd = r.unwrap();
    assert!(msg.is_empty());
    assert!(n.is_empty());
    assert_eq!(nrrd.dim, 3);
    assert_eq!(nrrd.type_, types::FLOAT);
    assert_eq!(nrrd.space, spaces::LPS);
    assert_eq!(nrrd.space_dim, 3);
    assert_eq!(nrrd.space_origin[..3], [-1.5, 2.0, 30.0]);
    assert_eq!(nrrd.axis[1].space_direction[..3], [0.0, 2.0, 0.0]);
    assert_eq!(
        nrrd.axis[0].space_direction[3].to_bits(),
        0x7fff_ffff_e000_0000
    );
    assert_eq!(nrrd.cmt, vec![b"a comment".to_vec()]);
    assert_eq!(nrrd.kvp, vec![(b"key".to_vec(), b"va\nlue".to_vec())]);
    let d = nrrd.data.unwrap();
    assert_eq!(&d[..4], &1.5f32.to_le_bytes());
    assert_eq!(&d[4..], &(-2.0f32).to_le_bytes());
}

#[test]
fn ascii_hex_and_detached() {
    let dir = tempfile::tempdir().unwrap();
    let a = write(
        dir.path(),
        "a.nrrd",
        b"NRRD0001\ntype: short\ndimension: 1\nsizes: 3\nencoding: ascii\n\n1 -2\n70000\n",
    );
    let d = read(&a, UpstreamDefects::Fix).0.unwrap().data.unwrap();
    assert_eq!(d, [1, 0, 0xfe, 0xff, 0x70, 0x11]);
    let h = write(
        dir.path(),
        "h.nrrd",
        b"NRRD0004\ntype: ushort\ndimension: 1\nsizes: 2\nendian: big\nencoding: hex\n\n01 02\nA0b0\n",
    );
    let d = read(&h, UpstreamDefects::Fix).0.unwrap().data.unwrap();
    assert_eq!(d, [0x02, 0x01, 0xb0, 0xa0]);
    write(dir.path(), "s0.raw", &[7]);
    write(dir.path(), "s1.raw", &[9]);
    let t = write(
        dir.path(),
        "t.nhdr",
        b"NRRD0004\ntype: uchar\ndimension: 2\nsizes: 1 2\nencoding: raw\ndata file: s%d.raw 0 1 1\n",
    );
    let d = read(&t, UpstreamDefects::Fix).0.unwrap().data.unwrap();
    assert_eq!(d, [7, 9]);
    let l = write(
        dir.path(),
        "l.nhdr",
        b"NRRD0004\ntype: uchar\ndimension: 2\nsizes: 1 2\nencoding: raw\ndata file: LIST\ns1.raw\ns0.raw\n",
    );
    let d = read(&l, UpstreamDefects::Fix).0.unwrap().data.unwrap();
    assert_eq!(d, [9, 7]);
}

#[test]
fn messages_newest_first() {
    let dir = tempfile::tempdir().unwrap();
    let a = write(
        dir.path(),
        "a.nrrd",
        b"NRRD0004\ntype: floaty\ndimension: 1\n",
    );
    let (r, msg, _) = read(&a, UpstreamDefects::Fix);
    assert_eq!(r, Err(LoadError::Failed { code: 1 }));
    let text = String::from_utf8(msg).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines[0].starts_with("[nrrd] nrrdLoad: trouble reading \""));
    assert_eq!(lines[1], "[nrrd] nrrdRead: trouble");
    assert_eq!(lines[2], "[nrrd] _nrrdRead: trouble reading NRRD file");
    assert_eq!(
        lines[3],
        "[nrrd] _nrrdFormatNRRD_read: trouble parsing type info \"floaty\""
    );
    assert_eq!(
        lines[4],
        "[nrrd] _nrrdReadNrrdParse_type: couldn't parse type \"floaty\""
    );
    let missing = b"/nonexistent-dir-larmorx/x.nrrd";
    let (r, msg, _) = read(missing, UpstreamDefects::Fix);
    assert_eq!(r, Err(LoadError::Failed { code: 2 }));
    assert_eq!(
        msg,
        b"[nrrd] nrrdLoad: fopen(\"/nonexistent-dir-larmorx/x.nrrd\",\"rb\") failed: No such file or directory\n"
    );
}

#[test]
fn defects_both_ways() {
    let dir = tempfile::tempdir().unwrap();
    // FS-518: the space names that run together
    let a = write(
        dir.path(),
        "a.nrrd",
        b"NRRD0004\ntype: uchar\ndimension: 1\nsizes: 1\nspace: 3D-left-handed\nencoding: raw\n\n\x05",
    );
    let (r, _, n) = read(&a, UpstreamDefects::Fix);
    assert_eq!(r.unwrap().space, 10);
    assert_eq!(n.as_slice()[0].defect.id, "FS-518");
    assert!(read(&a, UpstreamDefects::Reproduce).0.is_err());
    // FS-520: 3-gradient
    let g = write(
        dir.path(),
        "g.nrrd",
        b"NRRD0004\ntype: uchar\ndimension: 1\nsizes: 3\nkinds: 3-gradient\nencoding: raw\n\n123",
    );
    assert!(read(&g, UpstreamDefects::Fix).0.is_ok());
    assert_eq!(
        read(&g, UpstreamDefects::Reproduce).0,
        Err(LoadError::Exit {
            status: 1,
            message: b"nrrdKindSize: PANIC: nrrdKind 20 not implemented!\n".to_vec()
        })
    );
    // FS-763: a last line without its newline
    write(dir.path(), "x.raw", &[3]);
    let h = write(
        dir.path(),
        "h.nhdr",
        b"NRRD0004\ntype: uchar\ndimension: 1\nsizes: 1\nencoding: raw\ndata file: x.raw",
    );
    assert_eq!(read(&h, UpstreamDefects::Fix).0.unwrap().data.unwrap(), [3]);
    let (r, msg, _) = read(&h, UpstreamDefects::Reproduce);
    assert!(r.is_err());
    assert!(
        String::from_utf8_lossy(&msg).contains("hit end of header, but no \"data file\" given")
    );
    // FS-761: an unprintable character in a comment
    let c = write(
        dir.path(),
        "c.nrrd",
        b"NRRD0004\n# caf\xc3\xa9 \x01 x\ntype: uchar\ndimension: 1\nsizes: 1\nencoding: raw\n\n\x05",
    );
    assert_eq!(
        read(&c, UpstreamDefects::Fix).0.unwrap().cmt,
        vec![b"caf x".to_vec()]
    );
    assert!(read(&c, UpstreamDefects::Reproduce).0.is_err());
}

#[test]
fn biff_keys_and_spaces() {
    let mut b = Biff::new();
    b.add("zz", b"one\ttwo\n\n");
    b.add("aa", b"first");
    b.add("zz", b"three");
    assert_eq!(b.get("zz").unwrap(), b"[zz] three\n[zz] one two\n");
    assert_eq!(b.check("aa"), 1);
    assert!(b.get_done("aa").is_some());
    assert_eq!(b.get("aa"), None);
}

#[test]
fn strings() {
    let mut s = b"a\\\\b\\nc\\".to_vec();
    unescape(&mut s);
    assert_eq!(s, b"a\\b\nc\\");
    let mut t = Strtok::new(b"1  2 ", b" \t");
    assert_eq!(t.next_token(), Some(&b"1"[..]));
    assert_eq!(t.next_token(), Some(&b"2"[..]));
    assert_eq!(t.next_token(), None);
    let mut t = Strtok::new(b"1  ", b" \t");
    assert_eq!(t.next_token(), Some(&b"1"[..]));
    assert_eq!(t.next_token(), Some(&b""[..]));
    assert_eq!(t.next_token(), None);
}
