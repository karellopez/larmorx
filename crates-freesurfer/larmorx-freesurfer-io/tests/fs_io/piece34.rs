// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Piece 34 (track A relay 45): `gro` (`MRIreadGeRoi`, `mri_convert --roi`'s GE ROI series) of a
//! series the case makes in the fixture's folder `w`: a named file (one of the slices, another
//! file of the series' name, a name whose three digits run on, a missing file, a folder, a name
//! without three digits) whose bytes 8 to 16 give the width and height (0 to 5, now and then 0,
//! -1, a long row or a volume past 16 GiB: FS-947), cut now and then before them (FS-944); 0 to
//! 5 slices (now and then 12, or 1,002 with the slices 1,000 and 1,001: four-digit names), each
//! present or not, its pixel offset at byte 4 (in the header, past the end, negative, 0), its rows
//! whole or cut; slice files cut before their offset (FS-945 for the first read, FS-946 after).
//! The volume printed as piece 6's, then `errno`. `SKIP` for FS-944, FS-945 and FS-947 (memory
//! contents; the machine's memory).

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_io::mriio_ge::mri_read_ge_roi;

use super::piece4::file_entry;
use super::piece6::print_mri;
use super::{Gen, Out, Work, dec, enc, hex};

pub const KINDS: [&str; 1] = ["gro"];

/// The series' name parts in `w` (a folder entry first when the prefix has one).
const NAMES: [(&str, &str); 7] = [
    ("w/roi", ".img"),
    ("w/I.", ""),
    ("w/s12e", "_x"),
    ("w/d4/ge", ".MR"),
    ("w/7a", "b9"),
    ("w/x", ""),
    ("w/p.q", ".r1"),
];

fn header(g: &mut Gen, w: i32, h: i32, offset: i32) -> Vec<u8> {
    let mut b = vec![0u8; 16];
    for v in b.iter_mut().take(4) {
        *v = g.below(256) as u8;
    }
    b[4..8].copy_from_slice(&offset.to_be_bytes());
    b[8..12].copy_from_slice(&w.to_be_bytes());
    b[12..16].copy_from_slice(&h.to_be_bytes());
    b
}

/// One slice file: its header, then rows of shorts at `offset` (each short `0` to `0xffff`), cut
/// now and then; for a slice of more than 64 KiB the header alone (relay 45: a generator that
/// filled a 65,536 x 65,536 slice was ended by its 20 GiB memory box).
fn slice_file(g: &mut Gen, w: i32, h: i32) -> Vec<u8> {
    let data = (w.max(0) as usize) * (h.max(0) as usize) * 2;
    if data > 1 << 16 {
        // a volume of huge rows (FS-947's): the header alone, its first row cut
        return header(g, w, h, 16);
    }
    let offset = match g.below(14) {
        0 => g.pick(&[-4, i32::MIN, -1]),
        1 => 0,
        2 => g.int(16, 24) as i32 + data as i32 + 4,
        3 => g.int(2, 10) as i32,
        _ => g.pick(&[16, 16, 20, 32]),
    };
    let mut f = header(g, w, h, offset);
    let at = offset.max(0) as usize;
    if f.len() < at {
        f.resize(at, 0);
    }
    if offset >= 0 && f.len() < at + data {
        f.resize(at + data, 0);
    }
    let start = if offset >= 0 { at } else { 16 };
    for k in 0..data.min(f.len().saturating_sub(start)) {
        f[start + k] = g.below(256) as u8;
    }
    match g.below(12) {
        // cut before the offset or inside it (FS-945, FS-946)
        0 => f.truncate(g.int(0, 7) as usize),
        // cut inside the rows
        1 if data > 0 => f.truncate(at + g.below(data as u64) as usize),
        _ => {}
    }
    f
}

fn gen_gro(g: &mut Gen, t: &mut Vec<String>) {
    let (pre, post) = g.pick(&NAMES);
    let (w, h): (i32, i32) = match g.below(30) {
        0 => (0, g.int(0, 3) as i32),
        1 => (-1, 2),
        2 => (g.pick(&[300, 1000]), 1),
        3 => (65536, 65536),
        _ => (g.int(1, 5) as i32, g.int(1, 5) as i32),
    };
    let n_slices = match g.below(25) {
        0 => 12,
        1 => 1002,
        2 => 0,
        _ => g.int(1, 5),
    };
    let mut files: Vec<String> = Vec::new();
    if pre.contains("/d4/") {
        files.push(format!("D{}", hex(b"w/d4")));
    }
    let numbers: Vec<i64> = if n_slices == 1002 {
        vec![1000, 1001, g.int(0, 3)]
    } else {
        (0..n_slices).collect()
    };
    let mut made = Vec::new();
    for &i in &numbers {
        if !g.chance(4) {
            let b = slice_file(g, w, h);
            let name = format!("{pre}{i:03}{post}");
            files.push(file_entry(&name, &b));
            made.push(name);
        }
    }
    // the named file
    let own = |g: &mut Gen, files: &mut Vec<String>, n: String, cut: bool| {
        let mut b = header(g, w, h, 16);
        if cut && g.chance(2) {
            b.truncate(g.int(0, 15) as usize);
        }
        files.push(file_entry(&n, &b));
        n
    };
    let name = match g.below(12) {
        // a missing file
        0 => format!("{pre}{:03}{post}", g.int(1100, 1999)),
        // no three digits in the base name (the file is there)
        1 => {
            let n = format!("{pre}{}{post}", g.pick(&["12", "1", "1x23", ""]));
            own(g, &mut files, n, false)
        }
        2 => {
            // a folder of the series' name
            let n = format!("{pre}{:03}{post}", g.int(500, 999));
            files.push(format!("D{}", hex(n.as_bytes())));
            n
        }
        // digits that run on: the series' name is cut after the first three
        3 => {
            let n = format!("{pre}{:04}{post}", g.int(0, 30));
            own(g, &mut files, n, false)
        }
        // a named file of its own, cut before its width or height now and then (FS-944)
        4 | 5 => {
            let n = format!("{pre}{:03}{post}", g.int(600, 999));
            own(g, &mut files, n, true)
        }
        _ => match made.first() {
            Some(n) => n.clone(),
            None => {
                let n = format!("{pre}{:03}{post}", g.int(600, 999));
                own(g, &mut files, n, false)
            }
        },
    };
    t.push(enc(Some(name.as_bytes())));
    t.push(n_slices.to_string());
    t.push(if files.is_empty() {
        "-".into()
    } else {
        files.join(",")
    });
}

/// A piece-34 case's tokens.
pub fn generate(g: &mut Gen, kind: &str, _portable: bool) -> Vec<String> {
    let mut t: Vec<String> = vec![kind.to_owned()];
    match kind {
        "gro" => gen_gro(g, &mut t),
        k => panic!("piece 34: {k}"),
    }
    t
}

fn has(g: &Globals, id: &str) -> bool {
    g.notices.as_slice().iter().any(|x| x.defect.id == id)
}

/// What the port's side writes for a volume past fsio's memory limit (not run; `SKIP`).
const NOT_RUN: &[u8] = b"fs_io_probe: a GE ROI volume past fsio's limits, not run\n";

pub fn run(
    tk: &[&str],
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
    w: &Work,
    extra: &mut Vec<&'static str>,
) -> Result<(), u8> {
    let fatal = |f: larmorx_freesurfer_core::log::Fatal| f.status();
    match tk[0] {
        "gro" => {
            let name = String::from_utf8(dec(tk[1]).unwrap_or_default()).unwrap();
            let n_slices: i32 = tk[2].parse().unwrap();
            super::piece4::setup(w, tk[3]);
            // a volume or (with a negative extent) a slice buffer past fsio's 1 GiB of address space
            // (a volume within 16 GiB): not run
            if let Ok(b) = std::fs::read(w.fix.join(&name))
                && b.len() >= 16
            {
                let wd = i32::from_be_bytes(b[8..12].try_into().unwrap());
                let ht = i32::from_be_bytes(b[12..16].try_into().unwrap());
                let bytes = i128::from(wd) * i128::from(ht) * i128::from(n_slices) * 2;
                // a negative extent: no chunk, and each slice's buffer is the extents' product
                // in 32-bit arithmetic (wrapping), widened, times 2
                let slice = (i64::from(wd.wrapping_mul(ht)) as u64).wrapping_mul(2);
                let big = |b: u64| b > (1 << 28) && b <= (1 << 47);
                if (wd > 0 && ht > 0 && n_slices > 0 && bytes > (1 << 28) && bytes <= (1 << 34))
                    || ((wd < 0 || ht < 0) && n_slices > 0 && big(slice))
                {
                    let _ = out.write_all(NOT_RUN);
                    extra.push("SKIP");
                    return Ok(());
                }
            }
            g.errno = 0;
            let r = mri_read_ge_roi(g, out, err, Some(w.fix.as_path()), &name, n_slices);
            if has(g, "FS-944") || has(g, "FS-945") || has(g, "FS-947") {
                // memory contents in FreeSurfer, or its outcome depends on the machine's memory
                extra.push("SKIP");
            }
            let r = r.map_err(fatal)?;
            print_mri(o, w, r.as_ref());
            o.i(i64::from(g.errno));
        }
        k => panic!("piece 34: {k}"),
    }
    Ok(())
}
