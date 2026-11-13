// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.io's piece 2 (FreeSurfer's tags, FStagsIO, VOL_GEOM's write and read, the binary colour
//! tables): round trips, and FS-177 to FS-193 in both modes.

use std::path::{Path, PathBuf};

use larmorx_core::defects::{
    FS_177, FS_178, FS_179, FS_180, FS_181, FS_182, FS_183, FS_184, FS_185, FS_186, FS_187, FS_188,
    FS_189, FS_190, FS_191, FS_192, FS_193, FS_194, UpstreamDefects,
};
use larmorx_freesurfer_core::cname::CName;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_io::cfile::CFile;
use larmorx_freesurfer_io::colortab_bin;
use larmorx_freesurfer_io::fio;
use larmorx_freesurfer_io::fstagsio::{self, FsTagsIo};
use larmorx_freesurfer_io::tags;
use larmorx_freesurfer_mri::ctab::{ColorTable, ColorTableEntry};
use larmorx_freesurfer_mri::mri::{self, MRI_UCHAR};
use larmorx_freesurfer_mri::vol_geom::VolGeom;
use larmorx_io::znz::ZnzFile;

const MODES: [UpstreamDefects; 2] = [UpstreamDefects::Fix, UpstreamDefects::Reproduce];

fn dir() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("fs io piece2 ~")
        .tempdir()
        .unwrap()
}

fn p(d: &Path, n: &str) -> (PathBuf, Vec<u8>) {
    let f = d.join(n);
    let b = f.to_str().unwrap().as_bytes().to_vec();
    (f, b)
}

fn ct() -> ColorTable {
    let e = |n: &[u8], r| {
        Some(Box::new(ColorTableEntry {
            name: n.to_vec(),
            ri: r,
            gi: 2,
            bi: 3,
            ai: 255,
            ..ColorTableEntry::default()
        }))
    };
    ColorTable {
        entries: vec![
            e(b"Unknown", 0),
            None,
            e(b"Left-Cerebral-White-Matter", 245),
        ],
        fname: b"FreeSurferColorLUT.txt".to_vec(),
        version: 2,
        ..ColorTable::default()
    }
}

#[test]
fn colour_tables_round_trip_in_both_versions_and_streams() {
    let d = dir();
    for version in [1, 2] {
        for znz in [false, true] {
            let mut g = Globals::new(UpstreamDefects::Fix);
            let (f, b) = p(d.path(), "ct.bin");
            let mut t = ct();
            t.version = version;
            if version == 1 {
                // the first version has no empty places (FS-194)
                t.entries.remove(1);
            }
            let (mut o, mut e) = (Vec::new(), Vec::new());
            if znz {
                let mut z = ZnzFile::open(&b, b"wb", true).unwrap();
                colortab_bin::znz_ctab_write_into_binary(
                    &mut g,
                    &mut o,
                    &mut e,
                    Some(&t),
                    Some(&mut z),
                );
                z.close();
                let mut z = ZnzFile::open(&b, b"rb", true).unwrap();
                let r =
                    colortab_bin::znz_ctab_read_from_binary(&mut g, &mut o, &mut e, Some(&mut z))
                        .unwrap();
                check_ct(&r, &t);
            } else {
                let mut fp = CFile::open(None, f.to_str().unwrap(), "wb").unwrap();
                colortab_bin::ctab_write_into_binary(
                    &mut g,
                    &mut o,
                    &mut e,
                    Some(&t),
                    Some(&mut fp),
                );
                fp.close();
                let mut fp = CFile::open(None, f.to_str().unwrap(), "rb").unwrap();
                let r = colortab_bin::ctab_read_from_binary(&mut g, &mut o, &mut e, Some(&mut fp))
                    .unwrap();
                check_ct(&r, &t);
            }
            assert!(g.notices.is_empty(), "{:?}", g.notices.as_slice());
        }
    }
}

fn check_ct(r: &ColorTable, t: &ColorTable) {
    assert_eq!(r.fname, t.fname);
    let names: Vec<_> = r.entries.iter().flatten().map(|e| e.name.clone()).collect();
    let want: Vec<_> = t.entries.iter().flatten().map(|e| e.name.clone()).collect();
    assert_eq!(names, want);
    let last = r.entries.iter().flatten().last().unwrap();
    assert_eq!(
        (last.ri, last.ai, last.rf),
        (245, 255, (245.0f32 as f64 / 255.0) as f32)
    );
}

fn be(v: i32) -> [u8; 4] {
    v.to_be_bytes()
}

fn read_ct_file(
    mode: UpstreamDefects,
    bytes: &[u8],
    znz: bool,
) -> (Globals, Option<ColorTable>, Vec<u8>) {
    let d = dir();
    let (f, b) = p(d.path(), "t.bin");
    std::fs::write(&f, bytes).unwrap();
    let mut g = Globals::new(mode);
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let r = if znz {
        let mut z = ZnzFile::open(&b, b"rb", false).unwrap();
        colortab_bin::znz_ctab_read_from_binary(&mut g, &mut o, &mut e, Some(&mut z))
    } else {
        let mut fp = CFile::open(None, f.to_str().unwrap(), "rb").unwrap();
        colortab_bin::ctab_read_from_binary(&mut g, &mut o, &mut e, Some(&mut fp))
    };
    (g, r, e)
}

#[test]
fn colour_table_defects() {
    for mode in MODES {
        // FS-187: a second-version entry name of length 0 on a C stream
        let mut v2 = [be(-2), be(1), be(2)].concat();
        v2.extend_from_slice(b"a\0");
        v2.extend([be(1), be(0), be(0), be(1), be(2), be(3), be(4)].concat());
        let (g, r, _) = read_ct_file(mode, &v2, false);
        assert!(g.notices.contains(&FS_187));
        assert_eq!(r.unwrap().entries[0].as_ref().unwrap().name, b"");
        // FS-189: a structure number beyond the count
        let mut bad = [be(-2), be(1), be(1), [0, 0, 0, 0]].concat();
        bad.truncate(13);
        bad.extend([be(1), be(5)].concat());
        let (g, r, e) = read_ct_file(mode, &bad, true);
        assert!(r.is_none() && g.notices.contains(&FS_189));
        assert!(String::from_utf8_lossy(&e).contains("read entry index 5"));
        // FS-188: a first-version name length of 1,100
        let mut v1 = [be(1), be(1)].concat();
        v1.push(0);
        v1.extend(be(1100));
        v1.extend(vec![b'x'; 1100]);
        v1.extend([be(1), be(2), be(3), be(4)].concat());
        let (g, r, _) = read_ct_file(mode, &v1, true);
        assert!(g.notices.contains(&FS_188) && g.notices.contains(&FS_187));
        assert_eq!(r.unwrap().entries[0].as_ref().unwrap().name.len(), 1023);
        // FS-190: a compressed second-version file name of 1,100 bytes
        let mut long = [be(-2), be(0), be(1100)].concat();
        long.extend(vec![b'n'; 1100]);
        long.extend(be(0));
        let (g, r, _) = read_ct_file(mode, &long, true);
        assert!(g.notices.contains(&FS_190));
        match mode {
            UpstreamDefects::Fix => assert_eq!(r.unwrap().fname.len(), 1023),
            // the name's other bytes read as the number of entries (0x6e6e6e6e: for ever, as
            // zeros at the file's end make every structure 0, a duplicate after the first)
            _ => assert!(r.is_none()),
        }
        // FS-191: a first-version count beyond larmorx's cap
        let (g, r, e) = read_ct_file(mode, &[be(1 << 21), be(0)].concat(), false);
        assert!(r.is_none() && g.notices.contains(&FS_191));
        assert!(String::from_utf8_lossy(&e).contains("could not allocate 2097152 entries"));
        // FS-194: a first-version table with an empty place, written and read back
        let d = dir();
        let (f, _) = p(d.path(), "v1.bin");
        let mut g = Globals::new(mode);
        let mut t = ct();
        t.version = 1;
        let mut fp = CFile::open(None, f.to_str().unwrap(), "wb").unwrap();
        let (mut o, mut e) = (Vec::new(), Vec::new());
        colortab_bin::ctab_write_into_binary(&mut g, &mut o, &mut e, Some(&t), Some(&mut fp));
        fp.close();
        assert!(g.notices.contains(&FS_194));
        let (g, r, _) = read_ct_file(mode, &std::fs::read(&f).unwrap(), false);
        assert!(g.notices.contains(&FS_192));
        assert_eq!(r.is_some(), mode == UpstreamDefects::Reproduce);
        // FS-192: a table the file ends inside
        let (g, r, _) = read_ct_file(mode, &[be(2), be(2)].concat(), true);
        assert!(g.notices.contains(&FS_192));
        assert_eq!(r.is_some(), mode == UpstreamDefects::Reproduce);
    }
}

#[test]
fn tags_and_their_defects() {
    let d = dir();
    for mode in MODES {
        let mut g = Globals::new(mode);
        let mut e = Vec::new();
        let (f, b) = p(d.path(), "tags");
        // FS-177: a negative length
        std::fs::write(&f, [1u8, 2]).unwrap();
        let mut fp = CFile::open(None, f.to_str().unwrap(), "rb").unwrap();
        let r = tags::tag_skip(&mut g, &mut e, &mut fp, 3, -1);
        assert_eq!(r.err().map(|x| x.status()), Some(254));
        let shown = if mode == UpstreamDefects::Reproduce {
            "4294967295"
        } else {
            "-1"
        };
        assert!(String::from_utf8_lossy(&e).contains(&format!("calloc {shown} bytes!")));
        assert!(g.notices.contains(&FS_177));
        // FS-178: a length beyond the file
        let mut fp = CFile::open(None, f.to_str().unwrap(), "rb").unwrap();
        assert_eq!(tags::tag_skip(&mut g, &mut e, &mut fp, 3, 100).unwrap(), 2);
        assert!(g.notices.contains(&FS_178));
        // FS-181: a 3 x 3 matrix written as a tag; read back as zeros where it has none
        let m3 = {
            let (mut a, mut c) = (Vec::new(), Vec::new());
            larmorx_freesurfer_core::matrix::matrix_alloc(
                &mut g,
                &mut a,
                &mut c,
                3,
                3,
                larmorx_freesurfer_core::matrix::MATRIX_REAL,
            )
            .unwrap()
            .unwrap()
        };
        let mut fp = CFile::open(None, f.to_str().unwrap(), "wb").unwrap();
        tags::tag_write_matrix(&mut g, &mut fp, &m3, 23);
        fp.close();
        assert!(g.notices.contains(&FS_181));
        let mut fp = CFile::open(None, f.to_str().unwrap(), "rb").unwrap();
        let mut len = 0;
        assert_eq!(tags::tag_read_start(&mut g, &mut e, &mut fp, &mut len), 23);
        assert_eq!(len, 6 + 16 * 11);
        let m = tags::tag_read_matrix(&mut g, &mut fp);
        assert_eq!(m.relt(4, 4), 0.0);
        // FS-179 and FS-180: a long first word, no NUL in the 1,600 bytes
        let mut z = ZnzFile::open(&b, b"wb", true).unwrap();
        let mut here = 0;
        tags::znz_tag_write_start(&mut g, &mut z, 23, &mut here, 1600);
        let mut text = vec![b'W'; 120];
        text.extend(b" 1 2 3");
        text.resize(1600, b'9');
        fio::fs_znzwrite(&mut z, &text, 1, 1600);
        z.close();
        let mut z = ZnzFile::open(&b, b"rb", true).unwrap();
        let m = tags::znz_read_matrix(&mut g, &mut e, &mut z);
        assert_eq!((m.relt(1, 1), m.relt(1, 2)), (1.0, 2.0));
        assert!(g.notices.contains(&FS_179) && g.notices.contains(&FS_180));
        // FS-182
        let line = tags::tag_make_command_line_string(&mut g, &[b"mri_segment", b"x"], 8);
        assert_eq!(line, b"mri_segment x ");
        assert!(g.notices.contains(&FS_182));
    }
}

fn geometry(name: &[u8]) -> VolGeom {
    let mut v = VolGeom {
        valid: 1,
        width: 4,
        height: 5,
        depth: 6,
        ..VolGeom::default()
    };
    v.fname = CName::new(name);
    v
}

#[test]
fn geometries_round_trip_and_fs_185_186() {
    let d = dir();
    for mode in MODES {
        for nifti in [false, true] {
            let mut g = Globals::new(mode);
            let mut e = Vec::new();
            let (_, b) = p(d.path(), "vg");
            let mut z = ZnzFile::open(&b, b"wb", true).unwrap();
            fstagsio::vol_geom_write(&mut z, &geometry(b"ab"), nifti, true);
            z.close();
            // a reader that held a longer name
            let mut v = geometry(b"/old/longer/name.mgz");
            let mut z = ZnzFile::open(&b, b"rb", true).unwrap();
            fstagsio::vol_geom_read(&mut g, &mut e, &mut z, &mut v, nifti, true);
            assert_eq!((v.width, v.height, v.depth), (4, 5, 6));
            // an .mgz's 512 bytes carry the name's NUL; the extension's name does not
            let kept = nifti && mode == UpstreamDefects::Reproduce;
            assert_eq!(
                v.fname.c_bytes(),
                if kept {
                    &b"abld/longer/name.mgz"[..]
                } else {
                    b"ab"
                }
            );
            assert_eq!(g.notices.contains(&FS_186), nifti);
        }
        // FS-185: a name length of 20,000,000 in the extension (the file holds 600 bytes)
        let mut g = Globals::new(mode);
        let mut e = Vec::new();
        let (f, b) = p(d.path(), "vg2");
        let mut bytes = vec![0u8; 76];
        bytes.extend(20_000_000i32.to_be_bytes());
        bytes.extend(vec![b'z'; 600]);
        std::fs::write(&f, &bytes).unwrap();
        let mut z = ZnzFile::open(&b, b"rb", false).unwrap();
        let mut v = VolGeom::default();
        fstagsio::vol_geom_read(&mut g, &mut e, &mut z, &mut v, true, true);
        assert_eq!(v.fname.len(), 512);
        assert!(g.notices.contains(&FS_185));
    }
}

#[test]
fn fstagsio_frames_round_trip_and_lengths() {
    let d = dir();
    for nifti in [false, true] {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let mut m = mri::mri_alloc_header(&mut g, &mut o, &mut e, 1, 1, 1, MRI_UCHAR, 2).unwrap();
        for (i, f) in m.frames.iter_mut().enumerate() {
            f.ftype = 1;
            f.te = 2.5;
            f.label = 7 + i as i32;
            f.name = CName::new(b"frame");
            f.thresh = 0.5;
            f.bvalue = 1000.0;
            f.d4_flat = 3;
        }
        let (_, b) = p(d.path(), "frames");
        let mut z = ZnzFile::open(&b, b"wb", true).unwrap();
        FsTagsIo::new(&mut z, nifti)
            .write_mri_frames(&mut g, &mut o, &mut e, &m)
            .unwrap();
        z.close();
        let mut back =
            mri::mri_alloc_header(&mut g, &mut o, &mut e, 1, 1, 1, MRI_UCHAR, 2).unwrap();
        let mut z = ZnzFile::open(&b, b"rb", true).unwrap();
        let mut io = FsTagsIo::new(&mut z, nifti);
        let mut len = 0;
        assert_eq!(
            io.read_tagid_len(&mut g, &mut e, &mut len, 0),
            tags::TAG_MRI_FRAME
        );
        assert_eq!(len, fstagsio::getlen_mri_frames(&m, nifti, false));
        io.read_mri_frames(&mut g, &mut o, &mut e, &mut back, len);
        assert!(io.fp.eof() || io.fp.tell() == 12 + len);
        for (a, b) in m.frames.iter().zip(&back.frames) {
            assert_eq!(
                (a.label, a.thresh, a.name.c_bytes()),
                (b.label, b.thresh, b.name.c_bytes())
            );
            if !nifti {
                assert_eq!((a.te, a.bvalue, a.d4_flat), (b.te, b.bvalue, b.d4_flat));
            }
        }
        assert!(g.notices.is_empty(), "{:?}", g.notices.as_slice());
    }
}

#[test]
fn fstagsio_length_defects() {
    for mode in MODES {
        let mut g = Globals::new(mode);
        let wrapped = fstagsio::getlen_gcamorph_labels(&mut g, 1024, 1024, 1024, 4, true, false);
        assert_eq!(
            wrapped,
            if mode == UpstreamDefects::Reproduce {
                0
            } else {
                1 << 32
            }
        );
        assert!(g.notices.contains(&FS_184));
        let long = vec![b'a'; 600];
        let n = fstagsio::getlen_gcamorph_geom(&mut g, &long, b"b", true, false, true);
        assert_eq!(
            n,
            if mode == UpstreamDefects::Reproduce {
                160 + 601
            } else {
                160 + 513
            }
        );
        assert!(g.notices.contains(&FS_193));
        // FS-183: a scan-parameter tag shorter than its numbers
        let d = dir();
        let (f, b) = p(d.path(), "scan");
        std::fs::write(&f, vec![0u8; 40]).unwrap();
        let mut z = ZnzFile::open(&b, b"rb", false).unwrap();
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let mut m = mri::mri_alloc_header(&mut g, &mut o, &mut e, 1, 1, 1, MRI_UCHAR, 1).unwrap();
        FsTagsIo::new(&mut z, true).read_scan_parameters(&mut g, &mut o, &mut e, &mut m, 10);
        assert_eq!(m.pedir.as_deref(), Some(&b""[..]));
        assert!(g.notices.contains(&FS_183));
    }
}
