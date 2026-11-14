// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Piece 21 (track A relay 29): old Siemens Numaris `.ima` series. `imd` and `imh` (`mri_read`
//! with voxels and `MRIreadHeader`, as piece 6's `mrd` and `mrh`, the type -1 or 6): a complete
//! header (every field FreeSurfer reads, big-endian, written on a Sun) with its sibling files
//! `<study>-<series>-<n>.ima` beside it, structural (one slice per file) and mosaic series, plus
//! scouts, bad slice directions, bad matrix sizes, non-divisible counts, short headers (FreeSurfer
//! aborts, FS-688: skipped), name forms that do not parse, and missing or unreadable neighbours.

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;

use super::piece4::file_entry;
use super::{Gen, Out, Work, enc};

pub const KINDS: [&str; 2] = ["imd", "imh"];

fn put(b: &mut Vec<u8>, at: usize, v: &[u8]) {
    if b.len() < at + v.len() {
        b.resize(at + v.len(), 0);
    }
    b[at..at + v.len()].copy_from_slice(v);
}

/// A complete `.ima` header (6,144 bytes) followed by `rows * cols` big-endian shorts.
#[allow(clippy::too_many_arguments)]
fn ima_file(
    g: &mut Gen,
    rows: i16,
    cols: i16,
    brms: i32,
    nsl: i32,
    avg: i32,
    seq: &[u8],
    slice_dir: i32,
    pix_r: f64,
    pix_c: f64,
) -> Vec<u8> {
    let mut b = vec![0u8; 6144];
    let pd = |b: &mut Vec<u8>, at: usize, v: f64| put(b, at, &v.to_be_bytes());
    let pi = |b: &mut Vec<u8>, at: usize, v: i32| put(b, at, &v.to_be_bytes());
    let ph = |b: &mut Vec<u8>, at: usize, v: i16| put(b, at, &v.to_be_bytes());
    pd(&mut b, 1544, g.pick(&[1.5, 3.0, 5.0])); // slice thickness
    pd(&mut b, 1560, g.int(100, 3000) as f64); // tr
    pd(&mut b, 1568, g.int(5, 90) as f64); // te
    pd(&mut b, 1576, g.int(0, 900) as f64); // ti
    pi(&mut b, 1584, avg); // number of averages
    pd(&mut b, 2112, 90.0); // flip angle (degrees)
    pi(&mut b, 2864, brms); // base raw matrix size
    put(&mut b, 3009, &seq[..seq.len().min(65)]); // sequence file name
    pd(&mut b, 3744, g.pick(&[256.0, 240.0, 200.0])); // fov height
    pd(&mut b, 3752, g.pick(&[256.0, 240.0])); // fov width (FreeSurfer ignores it)
    pi(&mut b, 3760, slice_dir); // slice direction
    for (k, at) in [3768usize, 3776, 3784].iter().enumerate() {
        pd(&mut b, *at, g.int(-20, 20) as f64 + k as f64); // image centre sag/cor/tra
    }
    // normal, column, row direction cosines (triples)
    pd(&mut b, 3792, 0.0);
    pd(&mut b, 3800, 0.0);
    pd(&mut b, 3808, 1.0);
    pd(&mut b, 3832, 1.0);
    pd(&mut b, 3840, 0.0);
    pd(&mut b, 3848, 0.0);
    pd(&mut b, 3856, 0.0);
    pd(&mut b, 3864, 1.0);
    pd(&mut b, 3872, 0.0);
    pi(&mut b, 4004, nsl); // number of slices nominal
    pd(&mut b, 4136, g.pick(&[0.0, 0.25, 0.5])); // distance factor
    ph(&mut b, 4994, rows);
    ph(&mut b, 4996, cols);
    pd(&mut b, 5000, pix_r);
    pd(&mut b, 5008, pix_c);
    let n = (rows.max(0) as usize) * (cols.max(0) as usize);
    for k in 0..n {
        b.extend_from_slice(&(((k as i32 * 37) % 3000 - 1500) as i16).to_be_bytes());
    }
    b
}

pub fn generate(g: &mut Gen, kind: &str, _portable: bool) -> Vec<String> {
    let mut t = vec![kind.to_owned()];
    let mut entries = Vec::new();

    // the study/series numbers and the separator
    let sep = if g.chance(4) { '_' } else { '-' };
    let study = g.int(1, 20);
    let series = g.int(1, 9);
    let ext = if g.chance(8) { "IMA" } else { "ima" };
    let nm = |n: i64| format!("w/{study}{sep}{series}{sep}{n}.{ext}");

    // a mosaic or a structural series
    let mosaic = g.chance(3);
    let brms = g.pick(&[2, 4]);
    let (rows, cols): (i16, i16) = if mosaic {
        let mr = g.pick(&[1, 2]);
        let mc = g.pick(&[1, 2]);
        ((brms * mr) as i16, (brms * mc) as i16)
    } else {
        (brms as i16, brms as i16)
    };
    let nsl = if mosaic {
        g.int(1, 6) as i32
    } else if g.chance(10) {
        g.pick(&[0, 1])
    } else {
        1
    };
    let avg = if mosaic { g.pick(&[1, 2]) } else { 1 };
    let seq: Vec<u8> = if g.chance(12) {
        b"scout_64".to_vec()
    } else {
        g.pick(&[&b"fl3d"[..], b"ep2d", b"se"]).to_vec()
    };
    let slice_dir = if g.chance(12) {
        g.pick(&[0, 7, -1])
    } else {
        g.int(1, 6) as i32
    };
    let bad_matrix = !mosaic && g.chance(14);
    let base = if bad_matrix {
        brms + g.pick(&[1, -1])
    } else {
        brms
    };
    let pix_r = g.pick(&[0.9375, 1.0, 2.0]);
    let pix_c = g.pick(&[0.9375, 1.0, 1.25]);

    // the number of files in the series
    let n_files = if mosaic {
        // files must divide into whole volumes: mosaics_per_volume * frames
        let mpv = {
            let dangling = nsl.rem_euclid(brms * brms).max(0);
            let full = (nsl - dangling) / (brms * brms).max(1);
            (full + i32::from(dangling != 0)).max(1)
        };
        (mpv * g.int(1, 2) as i32).max(1) as i64
    } else {
        g.int(1, 4)
    };
    let first = g.int(1, 5);
    let given = first + g.below(n_files as u64) as i64;

    // a short header: FreeSurfer aborts in imaLoadFileInfo (FS-688, skipped)
    let short = g.chance(8);
    let cut = if short {
        Some(g.int(0, 4997) as usize)
    } else if g.chance(20) {
        // a header between 4,998 and 6,144: imaLoadFileInfo completes, siemensRead reads some
        // geometry past the end (FS-689, memory)
        Some(g.int(4998, 6143) as usize)
    } else {
        None
    };

    for n in first..first + n_files {
        let mut f = ima_file(g, rows, cols, base, nsl, avg, &seq, slice_dir, pix_r, pix_c);
        if let Some(c) = cut {
            f.truncate(c.min(f.len()));
        }
        entries.push(file_entry(&nm(n), &f));
    }
    let _ = given;

    // the name actually read: the given file, a sibling, a name that does not parse, or a missing one
    let name = if g.chance(16) {
        g.pick(&["w/x.ima", "w/1.ima", "w/a_b.ima"]).to_owned()
    } else if g.chance(18) {
        format!("w/{study}{sep}{series}{sep}{given}.bad")
    } else if g.chance(20) {
        nm(first + n_files + 2) // a number with no file
    } else {
        nm(given)
    };
    let ty = if g.chance(5) { -1 } else { 6 };

    t.push("0".into()); // Gdiag
    t.push(if g.chance(12) { "0" } else { "1" }.into()); // MRIIO_Strip_Pound
    t.push("0".into()); // set_env6 bits (none used by siemensRead)
    t.push(ty.to_string());
    t.push(enc(Some(name.as_bytes())));
    t.push(entries.join(","));
    t
}

/// The port's answer to a piece-21 case: piece 6's `mrd`/`mrh`.
pub fn run(
    tk: &[&str],
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
    w: &Work,
    extra: &mut Vec<&'static str>,
) -> Result<(), u8> {
    g.progname = "fsio".into();
    let mut t: Vec<&str> = vec![if tk[0] == "imd" { "mrd" } else { "mrh" }];
    t.extend_from_slice(&tk[1..]);
    let r = super::piece6::run_env(&t, g, out, err, o, w, extra, &[]);
    for n in g.notices.as_slice() {
        match n.defect.id {
            // FreeSurfer aborts in imaLoadFileInfo (a stack address freed): not comparable
            "FS-688" => extra.push("SKIP"),
            // FreeSurfer writes past the raw volume (memory corruption or a crash): not comparable
            "FS-692" => extra.push("SKIP"),
            // geometry fields read past the end of the header are memory contents
            "FS-689" => extra.push("VARY"),
            _ => {}
        }
    }
    r
}
