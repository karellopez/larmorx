// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.io probe's piece-4 cases (queue item 3.1-27): the volume format probe. `sty`
//! (`string_to_type`), `tts` (`type_to_string`), `mgvn` (`MRIgetVolumeName`), `mid`
//! (`mri_identify`), `isx` (each format's test), `idf` (the name and stem helpers,
//! `checkBrukerFiles`), `dcm` (`IsDICOM`, `IsSiemensDICOM` and `mri_identify` in sequence, with and
//! without `FS_DICOM_DEBUG`). Each file case makes its files in the fixture's folder `w` first: the
//! formats' headers built here from their layouts (NIfTI, ANALYZE, GE, MINC, NRRD, curvature,
//! Siemens, GE Signa, DICOM in part 10 and bare, Bruker's study folders, COR), right or broken.

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_io::{bruker, dicom_probe, identify, signa};

use super::{Gen, Out, Work, dec, enc, hex, unhex};

/// The piece-4 commands.
pub const KINDS: [&str; 7] = ["sty", "tts", "mgvn", "mid", "isx", "idf", "dcm"];

// ---------------------------------------------------------------- generation

const EXTS: [&str; 64] = [
    "gz",
    "ae",
    "sae",
    "csae",
    "m3z",
    "m3d",
    "cor",
    "minc",
    "mnc",
    "spm",
    "analyze",
    "analyze3d",
    "analyze4d",
    "img",
    "mgh",
    "mgz",
    "gca",
    "signa",
    "ge",
    "genesis",
    "gelx",
    "lx",
    "bshort",
    "bfloat",
    "bhdr",
    "siemens",
    "ima",
    "dicom",
    "siemens_dicom",
    "brik",
    "afni",
    "sdt",
    "varian",
    "otl",
    "outline",
    "gdf",
    "bruker",
    "ximg",
    "nifti1",
    "nii",
    "nrrd",
    "mat",
    "svs",
    "tif",
    "tiff",
    "jpg",
    "jpeg",
    "pgm",
    "ppm",
    "pbm",
    "rgb",
    "curv",
    "gii",
    "vtk",
    "label",
    "annot",
    "npy",
    "hdr",
    "spr",
    "w",
    "MR",
    "x",
    "",
    "dcm",
];

/// An extension, sometimes in another case.
fn ext(g: &mut Gen, portable: bool) -> String {
    let e = g.pick(&EXTS).to_owned();
    if !portable && g.chance(6) {
        return e.to_uppercase();
    }
    e
}

fn put_be16(b: &mut Vec<u8>, at: usize, v: u16) {
    if b.len() < at + 2 {
        b.resize(at + 2, 0);
    }
    b[at..at + 2].copy_from_slice(&v.to_be_bytes());
}

fn put_be32(b: &mut Vec<u8>, at: usize, v: u32) {
    if b.len() < at + 4 {
        b.resize(at + 4, 0);
    }
    b[at..at + 4].copy_from_slice(&v.to_be_bytes());
}

fn put(b: &mut Vec<u8>, at: usize, s: &[u8]) {
    if b.len() < at + s.len() {
        b.resize(at + s.len(), 0);
    }
    b[at..at + s.len()].copy_from_slice(s);
}

/// A Data General float holding `v` (16^(e-64) x fraction): 4 bytes.
fn dg_float(v: f64) -> u32 {
    if v == 0.0 {
        return 0;
    }
    let sign = if v < 0.0 { 0x8000_0000u32 } else { 0 };
    let mut m = v.abs();
    let mut e = 64i32;
    while m >= 1.0 {
        m /= 16.0;
        e += 1;
    }
    while m < 1.0 / 16.0 {
        m *= 16.0;
        e -= 1;
    }
    let frac = (m * 16_777_216.0) as u32 & 0x00ff_ffff;
    sign | ((e as u32 & 0x7f) << 24) | frac
}

/// A GE Signa header, plausible or with one field out of range.
fn signa_file(g: &mut Gen) -> Vec<u8> {
    let mut b = vec![0u8; signa::HLENGTH + g.pick(&[0usize, 0, 7, 512])];
    let w = |o: usize| 2 * o;
    let x = g.pick(&[256u16, 256, 256, 512, 512, 3, 128, 2, 0x1ff, 10_240, 0xff80]);
    let y = g.pick(&[256u16, 256, 256, 64, 3, 128, 0x0180]);
    put_be16(&mut b, w(signa::IHDR_START + signa::IHDR_X), x);
    put_be16(&mut b, w(signa::IHDR_START + signa::IHDR_Y), y);
    put_be16(
        &mut b,
        w(signa::SEHDR_START + signa::SEHDR_IALLOC),
        g.pick(&[124u16, 124, 124, 0, 0xffff, 0x0080]),
    );
    let f = |g: &mut Gen, good: f64| -> u32 {
        match g.below(60) {
            0 => g.next() as u32,
            1 => 0,
            2 => dg_float(-good),
            3 => dg_float(good * 1e6),
            _ => dg_float(good),
        }
    };
    let fields = [
        (signa::IHDR_START + signa::IHDR_TR, 2_000_000.0),
        (signa::IHDR_START + signa::IHDR_TI, 900_000.0),
        (signa::IHDR_START + signa::IHDR_TE, 30_000.0),
        (signa::SEHDR_START + signa::SEHDR_FOV, 240_000.0),
        (signa::IHDR_START + signa::IHDR_STRTX, 100_000.0),
        (signa::IHDR_START + signa::IHDR_ENDX, 120_000.0),
        (signa::IHDR_START + signa::IHDR_STRTY, 80_000.0),
        (signa::IHDR_START + signa::IHDR_ENDY, 60_000.0),
        (signa::IHDR_START + signa::IHDR_STRTZ, 40_000.0),
        (signa::IHDR_START + signa::IHDR_ENDZ, 20_000.0),
        (signa::IHDR_START + signa::IHDR_PIXSIZ, 937.5),
    ];
    for (o, good) in fields {
        let v = f(g, good);
        put_be32(&mut b, w(o), v);
    }
    if g.chance(6) {
        b.truncate(g.int(0, signa::HLENGTH as i64) as usize);
    }
    b
}

/// A DICOM element in a transfer syntax: (group, element), the two-letter representation, the
/// value; `explicit`, `big`.
fn element(
    out: &mut Vec<u8>,
    tag: (u16, u16),
    vr: &[u8; 2],
    value: &[u8],
    explicit: bool,
    big: bool,
) {
    let p16 = |o: &mut Vec<u8>, v: u16| {
        o.extend_from_slice(&if big {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        });
    };
    let p32 = |o: &mut Vec<u8>, v: u32| {
        o.extend_from_slice(&if big {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        });
    };
    p16(out, tag.0);
    p16(out, tag.1);
    if explicit {
        out.extend_from_slice(vr);
        if matches!(vr, b"OB" | b"OW" | b"UN" | b"UT" | b"SQ") {
            out.extend_from_slice(&[0, 0]);
            p32(out, value.len() as u32);
        } else {
            p16(out, value.len() as u16);
        }
    } else {
        p32(out, value.len() as u32);
    }
    out.extend_from_slice(value);
}

/// A DICOM file: part 10 or a bare data set, with or without a Manufacturer element of some
/// representation and value, sometimes cut or changed.
fn dicom_file(g: &mut Gen, portable: bool) -> Vec<u8> {
    let part10 = !g.chance(4);
    // the data set's syntax: implicit little endian, explicit little endian, explicit big endian
    let syntax = g.below(3);
    let (explicit, big) = if part10 {
        match syntax {
            0 => (false, false),
            1 => (true, false),
            _ => (true, true),
        }
    } else {
        (false, g.chance(3))
    };
    let mut ds = Vec::new();
    element(
        &mut ds,
        (0x8, 0x16),
        b"UI",
        b"1.2.840.10008.5.1.4.1.1.4\0",
        explicit,
        big,
    );
    element(&mut ds, (0x8, 0x60), b"CS", b"MR", explicit, big);
    if !g.chance(6) {
        let value: Vec<u8> = g
            .pick(&[
                b"SIEMENS ".as_slice(),
                b"SIEMENS ",
                b"SIEMENS",
                b"SIEMENS\0",
                b"SIEMENS  ",
                b"Siemens ",
                b"GE MEDICAL SYSTEMS",
                b"Philips Medical Systems ",
                b"",
                b"SIEM",
                b"SIEMENS\0x",
            ])
            .to_vec();
        let mut vr = *g.pick(&[
            b"LO", b"LO", b"LO", b"LO", b"SH", b"CS", b"UN", b"OB", b"US", b"UL", b"SQ", b"OT",
            b"UT",
        ]);
        if portable && matches!(&vr, b"OT" | b"UT") {
            vr = *b"LO";
        }
        let value = if &vr == b"SQ" {
            Vec::new()
        } else if matches!(&vr, b"US" | b"UL") && g.chance(2) {
            value.iter().take(2).copied().collect()
        } else {
            value
        };
        element(&mut ds, (0x8, 0x70), &vr, &value, explicit, big);
    }
    element(&mut ds, (0x10, 0x10), b"PN", b"X^Y ", explicit, big);
    if g.chance(3) {
        element(&mut ds, (0x7fe0, 0x10), b"OW", &[1, 2, 3, 4], explicit, big);
    }
    let mut f = Vec::new();
    if part10 {
        f.resize(128, 0);
        f.extend_from_slice(b"DICM");
        let ts: &[u8] = match syntax {
            0 => b"1.2.840.10008.1.2\0",
            1 => b"1.2.840.10008.1.2.1\0",
            _ => b"1.2.840.10008.1.2.2\0",
        };
        let mut meta = Vec::new();
        element(&mut meta, (0x2, 0x1), b"OB", &[0, 1], true, false);
        element(
            &mut meta,
            (0x2, 0x2),
            b"UI",
            b"1.2.840.10008.5.1.4.1.1.4\0",
            true,
            false,
        );
        element(&mut meta, (0x2, 0x10), b"UI", ts, true, false);
        element(
            &mut f,
            (0x2, 0x0),
            b"UL",
            &(meta.len() as u32).to_le_bytes(),
            true,
            false,
        );
        f.extend_from_slice(&meta);
    }
    f.extend_from_slice(&ds);
    match g.below(8) {
        0 => {
            let n = g.int(0, f.len() as i64) as usize;
            f.truncate(n);
        }
        1 => {
            let i = g.below(f.len().max(1) as u64) as usize;
            if i < f.len() {
                f[i] = g.next() as u8;
            }
        }
        _ => {}
    }
    f
}

/// A file entry (`F<hex name>:<size>[+<offset>:<hex>]...`) from its bytes: zero runs left out.
fn file_entry(name: &str, b: &[u8]) -> String {
    let mut s = format!("F{}:{}", hex(name.as_bytes()), b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == 0 {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < b.len() && (b[j] != 0 || (j + 1 < b.len() && b[j + 1] != 0)) {
            j += 1;
        }
        s.push_str(&format!("+{i}:{}", hex(&b[i..j])));
        i = j;
    }
    s
}

/// The content kinds (`kind_content`).
const KINDS_OF_CONTENT: [&str; 12] = [
    "nii", "analyze", "genesis", "gelx", "mnc", "nrrd", "curv", "siemens", "signa", "dicom",
    "dicom", "bytes",
];

/// A format's content of `kind` (right, or broken in the ways the tests check).
fn kind_content(g: &mut Gen, kind: &str, portable: bool) -> Vec<u8> {
    match kind {
        "nii" | "nifti1" => {
            let mut b = vec![0u8; 352];
            put(
                &mut b,
                344,
                g.pick(&[b"n+1\0".as_slice(), b"ni1\0", b"n+2\0"]),
            );
            if g.chance(8) {
                b.truncate(g.int(340, 348) as usize);
            }
            b
        }
        "analyze" => analyze_header(g),
        "genesis" => {
            let mut b = g
                .pick(&[b"IMGF".as_slice(), b"IMGF", b"IMG", b"FGMI"])
                .to_vec();
            b.resize(g.int(b.len() as i64, 40) as usize, 0);
            b
        }
        "gelx" => {
            let mut b = vec![0u8; 3232 + g.int(0, 8) as usize];
            put(&mut b, 3228, b"IMGF");
            if g.chance(4) {
                b.truncate(g.int(3224, 3231) as usize);
            }
            b
        }
        "mnc" => g
            .pick(&[b"CDF\x01".as_slice(), b"CDF\x02", b"CD", b"CDFx", b"cdf"])
            .to_vec(),
        "nrrd" => g
            .pick(&[b"NRRD0004\n".as_slice(), b"NRRD", b"NRR", b"nrrd"])
            .to_vec(),
        "curv" => g
            .pick(&[
                b"\xff\xff\xff\0\0\0\x01".as_slice(),
                b"\xff\xff\xff",
                b"\xff\xff",
                b"\xff\xff\xfe",
                b"\xff",
            ])
            .to_vec(),
        "siemens" => {
            let mut b = vec![0u8; 5842];
            put(&mut b, 5790, g.pick(&[b"SL".as_slice(), b"SL", b"Sl"]));
            put(&mut b, 5802, g.pick(&[b"SP".as_slice(), b"SP", b"S\0"]));
            put(
                &mut b,
                5838,
                g.pick(&[b"FoV".as_slice(), b"FoV", b"Fov", b"Fo\0"]),
            );
            if g.chance(4) {
                b.truncate(g.int(5780, 5842) as usize);
            }
            b
        }
        "signa" => signa_file(g),
        "dicom" => dicom_file(g, portable),
        _ => {
            let n = g.int(0, 64) as usize;
            (0..n).map(|_| g.next() as u8).collect()
        }
    }
}

/// An ANALYZE header: its length in its first int (big endian), 16,384 extents, `r`; or broken.
fn analyze_header(g: &mut Gen) -> Vec<u8> {
    let mut b = vec![0u8; g.pick(&[348usize, 348, 348, 352, 300])];
    put_be32(&mut b, 0, g.pick(&[348u32, 348, 348, 352, 0]));
    put_be32(&mut b, 32, g.pick(&[16384u32, 16384, 16384, 0]));
    if b.len() > 38 {
        b[38] = g.pick(b"rrrx");
    }
    b.truncate(b.len().min(352));
    b
}

/// The test a name's extension leads `mri_identify` to (for the content of its file).
fn kind_of_ext(e: &str) -> &'static str {
    match e.to_ascii_lowercase().as_str() {
        "nii" => "nii",
        "img" | "hdr" | "analyze" | "spm" | "nifti1" => "nifti1",
        "signa" => "signa",
        "ge" | "genesis" | "mr" => "genesis",
        "gelx" | "lx" => "gelx",
        "mnc" | "minc" => "mnc",
        "nrrd" => "nrrd",
        "curv" => "curv",
        "ima" | "siemens" => "siemens",
        "bruker" => "bruker",
        "cor" => "cor",
        "sdt" | "varian" => "sdt",
        "dicom" | "siemens_dicom" | "dcm" => "dicom",
        _ => "bytes",
    }
}

/// A name in `w` and the files of a case: (the name as probed, the setup entries). `hint`: the
/// test whose file to make (an `isx` case's), or `None` (`mri_identify`'s: by the extension, or
/// any).
fn file_case(g: &mut Gen, portable: bool, hint: Option<&str>) -> (String, Vec<String>) {
    let mut entries = Vec::new();
    let sub = if g.chance(5) {
        entries.push(format!("D{}", hex(b"w/a.d")));
        "w/a.d/"
    } else {
        "w/"
    };
    let stem = g.pick(&["s", "brain", "x.y", "COR-001", "v"]).to_owned();
    let mut e = ext(g, portable);
    // ANALYZE's test reads `<name>.hdr` of a name without a dot
    let bare = hint == Some("analyze") && !g.chance(4) || e.is_empty();
    if bare {
        e = String::new();
    }
    let stem = if bare && stem.contains('.') {
        "s".to_owned()
    } else {
        stem
    };
    // (a name ending in a dot is another name on Windows: not in the portable cases)
    let mut name = if bare || (e.is_empty() && (portable || g.chance(2))) {
        format!("{sub}{stem}")
    } else {
        format!("{sub}{stem}.{e}")
    };
    if !bare && g.chance(10) {
        name.push_str(g.pick(&[".gz", ".mgh.gz", ".nii.gz", ".mnc.gz"]));
    }
    let kind = match hint {
        Some(h) if !g.chance(4) => h.to_owned(),
        Some(_) => g.pick(&KINDS_OF_CONTENT).to_owned(),
        None if g.chance(2) => kind_of_ext(&e).to_owned(),
        None => g.pick(&KINDS_OF_CONTENT).to_owned(),
    };
    // the file itself: a format's content, a folder (a COR or Bruker study), or nothing
    let folder = match kind.as_str() {
        "bruker" | "cor" => !g.chance(4),
        _ => g.chance(10),
    };
    let nothing = g.chance(10);
    if nothing {
        // no file of that name
    } else if folder {
        entries.push(format!("D{}", hex(name.as_bytes())));
        if kind == "bruker" || g.chance(2) {
            let parts = [
                "method",
                "acqp",
                "pdata",
                "pdata/1",
                "pdata/1/2dseq",
                "pdata/1/d3proc",
                "pdata/1/reco",
            ];
            let upto = if kind == "bruker" && g.chance(2) {
                parts.len()
            } else {
                g.int(1, parts.len() as i64) as usize
            };
            for p in &parts[..upto] {
                let n = format!("{name}/{p}");
                if matches!(*p, "pdata" | "pdata/1") {
                    entries.push(format!("D{}", hex(n.as_bytes())));
                } else {
                    entries.push(file_entry(&n, b"x"));
                }
            }
        }
    } else if !(bare && kind == "analyze") {
        let b = kind_content(g, &kind, portable);
        entries.push(file_entry(&name, &b));
    }
    // companions: a header pair's .hdr, Varian's .spr
    let base = name
        .rfind('.')
        .map_or(name.as_str(), |i| &name[..i])
        .to_owned();
    let hdr_kind = matches!(kind.as_str(), "nifti1" | "analyze");
    if hdr_kind || g.chance(4) {
        let b = match (kind.as_str(), g.below(3)) {
            ("analyze", _) | (_, 1) => analyze_header(g),
            ("nifti1", _) | (_, 0) => kind_content(g, "nifti1", portable),
            _ => vec![0u8; 352],
        };
        let h = if bare || g.chance(2) {
            format!("{base}.hdr")
        } else {
            format!("{name}.hdr")
        };
        if h != name {
            entries.push(file_entry(&h, &b));
        }
    }
    if kind == "sdt" || g.chance(6) {
        let s = format!("{base}.spr");
        if s != name && !g.chance(5) {
            entries.push(file_entry(&s, b"x"));
        }
    }
    (name, entries)
}

/// The fixture's own names and ones outside `w` (no files made).
fn plain_name(g: &mut Gen, portable: bool) -> String {
    if portable {
        g.pick(&["d", "f", "nowhere", "I.001", "x.MR", "q.IMA", "r.BRIK", "w"])
            .to_owned()
    } else {
        g.pick(&[
            "d", "f", "nowhere", "I.001", "x.MR", "q.IMA", "r.BRIK", "w", "nr", "ro", "ln", "dln",
            "bad", "f/", "d/", "f/x", ".gz", "a.gz", "", "x.", "..",
        ])
        .to_owned()
    }
}

fn spec(entries: &[String]) -> String {
    if entries.is_empty() {
        "-".into()
    } else {
        entries.join(",")
    }
}

/// One piece-4 case line's tokens.
pub fn generate(g: &mut Gen, kind: &str, portable: bool) -> Vec<String> {
    let mut t: Vec<String> = vec![kind.into()];
    match kind {
        "sty" => {
            let mut e = ext(g, false);
            if g.chance(8) {
                e = g
                    .pick(&["Nii", "MgZ", "COR", "x.nii", "gz ", "a", "BRIK"])
                    .to_owned();
            }
            if !portable && g.chance(200) {
                e = "m".repeat(1030);
            }
            t.push(enc(Some(e.as_bytes())));
        }
        "tts" => t.push(g.int(-3, 37).to_string()),
        "mgvn" => {
            t.push(g.pick(&[1i64, 1, 0]).to_string());
            let mut s = g
                .pick(&[
                    "a.mgz",
                    "b/c.nii",
                    "x",
                    "",
                    "d.mgz@mgh",
                    "e.nii#1,2",
                    "f@g#h",
                    "#@",
                    "x#y@z",
                    "a@b@c#d#e",
                ])
                .to_owned();
            if g.chance(3) {
                s.push_str(g.pick(&["@nii", "#3", "@", "#"]));
            }
            t.push(enc(Some(s.as_bytes())));
        }
        "mid" | "isx" | "idf" | "dcm" => {
            if kind == "isx" {
                t.push(
                    g.pick(&[
                        "cor", "brik", "siemens", "genesis", "gelx", "analyze", "mnc", "mgh",
                        "bshort", "bfloat", "bhdr", "sdt", "gdf", "otl", "ximg", "nifti1", "nii",
                        "nrrd", "curv", "signa", "bruker",
                    ])
                    .into(),
                );
            }
            if kind == "idf" {
                t.push(
                    g.pick(&[
                        "stem", "ext", "name", "type", "bstem", "bfirst", "bprecs", "bprec", "bruk",
                    ])
                    .into(),
                );
            }
            if kind == "dcm" {
                t.push(
                    g.pick(&["d", "dd", "s", "ds", "sd", "ss", "m", "dm", "sm", "dsm"])
                        .into(),
                );
            }
            if kind == "mid" || kind == "dcm" {
                t.push(i64::from(g.chance(4)).to_string());
            }
            let (name, entries) = if kind == "idf" && g.chance(2) {
                stem_case(g)
            } else if kind == "dcm" {
                let n = format!(
                    "w/{}",
                    g.pick(&["i.dcm", "i", "i.dicom", "i.siemens_dicom", "i.img"])
                );
                let b = dicom_file(g, portable);
                (n.clone(), vec![file_entry(&n, &b)])
            } else if g.chance(8) {
                (plain_name(g, portable), Vec::new())
            } else {
                // mri_identify reaches ANALYZE's test only for a name without a dot
                let hint = if kind == "isx" {
                    Some(t[1].clone())
                } else {
                    (kind == "mid" && g.chance(10)).then(|| "analyze".to_owned())
                };
                file_case(g, portable, hint.as_deref())
            };
            t.push(enc(Some(name.as_bytes())));
            t.push(spec(&entries));
        }
        _ => unreachable!("{kind}"),
    }
    t
}

/// A stem helper's case: a stem or name and some of the stem's files.
fn stem_case(g: &mut Gen) -> (String, Vec<String>) {
    let stem = g.pick(&["w/s", "w/a.b", "w/s_000", "w/x"]);
    let mut entries = Vec::new();
    for suffix in [
        ".nii",
        ".nii.gz",
        ".mgh",
        ".mgz",
        ".img",
        ".bhdr",
        "_000.bfloat",
        "_000.bshort",
        ".w",
    ] {
        if g.chance(4) {
            entries.push(file_entry(&format!("{stem}{suffix}"), b"x"));
        }
    }
    let name = match g.below(4) {
        0 => stem.to_owned(),
        1 => format!("{stem}.bhdr"),
        _ => format!(
            "{stem}{}",
            g.pick(&[
                ".nii",
                ".nii.gz",
                ".mgh",
                ".mgz",
                ".img",
                ".bhdr",
                "_000.bfloat",
                "_000.bshort",
                ".w",
                "nii",
                "mgz",
                "x.gz",
                "",
                ".hdr"
            ])
        ),
    };
    (name, entries)
}

// ---------------------------------------------------------------- the port's answers

struct Toks<'a> {
    t: &'a [&'a str],
    i: usize,
}

impl<'a> Toks<'a> {
    fn s(&mut self) -> &'a str {
        let s = self.t[self.i];
        self.i += 1;
        s
    }
    fn i(&mut self) -> i64 {
        self.s().parse().unwrap()
    }
    fn name(&mut self) -> String {
        String::from_utf8(dec(self.s()).unwrap_or_default()).unwrap()
    }
}

/// The case's files in `<fix>/w` (made afresh), as `fsio`'s `setup` makes them.
fn setup(w: &Work, spec: &str) {
    let root = w.fix.join("w");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    if spec == "-" {
        return;
    }
    for e in spec.split(',').filter(|e| !e.is_empty()) {
        if let Some(n) = e.strip_prefix('D') {
            let n = String::from_utf8(unhex(n)).unwrap();
            std::fs::create_dir(w.fix.join(n)).unwrap();
            continue;
        }
        let e = &e[1..];
        let (n, rest) = e.split_once(':').unwrap();
        let mut parts = rest.split('+');
        let size: usize = parts.next().unwrap().parse().unwrap();
        let mut b = vec![0u8; size];
        for p in parts {
            let (off, h) = p.split_once(':').unwrap();
            let off: usize = off.parse().unwrap();
            for (k, &c) in unhex(h).iter().enumerate() {
                if off + k < b.len() {
                    b[off + k] = c;
                }
            }
        }
        let n = String::from_utf8(unhex(n)).unwrap();
        std::fs::write(w.fix.join(n), b).unwrap();
    }
}

/// The port's answer to a piece-4 case (`Err`: the program's exit status).
pub fn run(
    tk: &[&str],
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
    w: &Work,
    extra: &mut Vec<&'static str>,
) -> Result<(), u8> {
    let r = answer(tk, g, out, err, o, w);
    for n in g.notices.as_slice() {
        if matches!(n.defect.id, "FS-221" | "FS-227" | "FS-228") {
            extra.push("SKIP");
        }
    }
    r
}

fn answer(
    tk: &[&str],
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
    w: &Work,
) -> Result<(), u8> {
    let mut t = Toks { t: &tk[1..], i: 0 };
    let fatal = |f: Fatal| f.status();
    let dir = Some(w.fix.as_path());
    match tk[0] {
        "sty" => {
            let s = t.name();
            o.i(i64::from(identify::string_to_type(g, &s)));
        }
        "tts" => {
            let v = t.i() as i32;
            o.s(Some(identify::type_to_string(v)));
        }
        "mgvn" => {
            g.mriio_strip_pound = t.i() as i32;
            let s = t.name();
            let r = identify::mri_get_volume_name(g, &s);
            o.s(Some(&r));
        }
        kind => {
            let f = if kind == "mid" { "" } else { t.s() };
            let dbg = if kind == "mid" || kind == "dcm" {
                t.i() != 0
            } else {
                false
            };
            let name = t.name();
            setup(w, t.s());
            let env = if dbg {
                Environment::from_pairs([("FS_DICOM_DEBUG", "1")])
            } else {
                Environment::default()
            };
            g.errno = 0;
            match kind {
                "mid" => {
                    let r = identify::mri_identify(g, dir, &env, out, err, &name).map_err(fatal)?;
                    o.i(i64::from(r));
                }
                "dcm" => {
                    for k in f.chars() {
                        let r = match k {
                            'd' => dicom_probe::is_dicom(g, dir, &env, out, err, &name),
                            's' => dicom_probe::is_siemens_dicom(g, dir, &env, out, err, &name)
                                .map_err(fatal)?,
                            _ => identify::mri_identify(g, dir, &env, out, err, &name)
                                .map_err(fatal)?,
                        };
                        o.i(i64::from(r));
                    }
                }
                "isx" => {
                    let n = name.as_str();
                    let r = match f {
                        "cor" => identify::is_cor(g, dir, n),
                        "brik" => identify::is_brik(g, n),
                        "siemens" => identify::is_siemens(g, dir, n),
                        "genesis" => identify::is_genesis(g, dir, n),
                        "gelx" => identify::is_ge_lx(g, dir, n),
                        "analyze" => identify::is_analyze(g, dir, n),
                        "mnc" => identify::is_mnc(g, dir, n),
                        "mgh" => identify::is_mgh(g, n),
                        "bshort" => identify::is_bshort(n),
                        "bfloat" => identify::is_bfloat(n),
                        "bhdr" => identify::is_bhdr(n),
                        "sdt" => identify::is_sdt(g, dir, n),
                        "gdf" => identify::is_gdf(n),
                        "otl" => identify::is_otl(n),
                        "ximg" => identify::is_ximg(n),
                        "nifti1" => identify::is_nifti1(g, dir, n),
                        "nii" => identify::is_nii(g, dir, n),
                        "nrrd" => identify::is_nrrd(g, dir, n),
                        "curv" => identify::id_is_curv(g, dir, n),
                        "signa" => signa::is_signa(g, dir, n),
                        "bruker" => bruker::is_bruker(g, dir, n),
                        other => panic!("test {other}"),
                    };
                    o.i(i64::from(r));
                }
                _ => {
                    let n = name.as_str();
                    match f {
                        "stem" => o.s(identify::id_stem_from_name(n).as_deref()),
                        "ext" => o.s(identify::id_extension_from_name(g, n).as_deref()),
                        "name" => o.s(identify::id_name_from_stem(g, dir, n).as_deref()),
                        "type" => o.i(i64::from(identify::id_type_from_stem(g, dir, n))),
                        "bstem" => o.s(identify::bhdr_stem(n).as_deref()),
                        "bfirst" => o.s(identify::bhdr_firstslicefname(g, dir, n).as_deref()),
                        "bprecs" => o.s(identify::bhdr_precisionstring(g, dir, n).as_deref()),
                        "bprec" => o.i(i64::from(identify::bhdr_precision(g, dir, n))),
                        "bruk" => {
                            let mut files = bruker::BrukerFiles::default();
                            let r =
                                bruker::check_bruker_files(g, dir, out, err, n, &mut files, true);
                            o.i(i64::from(r));
                            for s in [
                                &files.method,
                                &files.acqp,
                                &files.data,
                                &files.d3proc,
                                &files.reco,
                            ] {
                                o.s(Some(s));
                            }
                        }
                        other => panic!("helper {other}"),
                    }
                }
            }
            o.i(i64::from(g.errno));
            if kind == "dcm" || kind == "mid" {
                dicom_probe::dump_conditions(g, err);
            }
        }
    }
    Ok(())
}
