// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Piece 23 (track A relay 31): image files read and written as volumes. `ixd` and `ixh` (piece
//! 6's `mrd` and `mrh`, the type -1 or 21) of SGI images (`.rgb`: both byte orders, uncompressed
//! and run-length, 1- and 2-byte pixels, one to four planes, of one to three dimensions, rows of
//! 16,384 pixels and more; then one fault: a wrong magic number, an unknown type, dimension or
//! pixel size, a cut header, tables or rows, run-length rows of length -1, longer than the buffer
//! or running past the width), MATLAB files (piece 5's), HIPS names (any other extension, or a
//! name with `@<image type>`), PGM, PPM and PBM files, JPEG files (since relay 39: track C's
//! generated files, now and then damaged), TIFF files (since relay 40: track C's generated files of
//! one to three pages, now and then damaged, and the port's own writes read back),
//! missing files and folders, with and without `#` frames; `ixw` (piece 17's `vwr`, `how` 31 or
//! `MRIwrite` by the name) of volumes of every type as SGI, MATLAB (the JPEG writer), PGM, PPM,
//! PBM, HIPS, TIFF and JPEG names, `#` and folder names, volumes 65,536 pixels wide or tall; and
//! of ITK warps (`how` 41: `itkMorphWrite`).

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_io::jpeg::cases as jpeg_cases;
use larmorx_io::tiff::cases as tiff_cases;

use super::piece4::file_entry;
use super::piece12::fhex;
use super::{Gen, Out, Work, enc, hex};

pub const KINDS: [&str; 3] = ["ixd", "ixh", "ixw"];

fn dir_entry(name: &str) -> String {
    format!("D{}", hex(name.as_bytes()))
}

fn put16(b: &mut Vec<u8>, v: u16, be: bool) {
    b.extend_from_slice(&if be { v.to_be_bytes() } else { v.to_le_bytes() });
}

fn put32(b: &mut Vec<u8>, v: u32, be: bool) {
    b.extend_from_slice(&if be { v.to_be_bytes() } else { v.to_le_bytes() });
}

/// One run-length row of `vals` (1 or 2 bytes a unit): runs of repeats and of copies, a 0 at the
/// end (now and then a fault).
fn rle_row(g: &mut Gen, vals: &[u16], unit: usize, be: bool) -> Vec<u8> {
    let mut units: Vec<u16> = Vec::new();
    let mut i = 0;
    while i < vals.len() {
        let mut j = i + 1;
        while j < vals.len() && vals[j] == vals[i] && j - i < 126 {
            j += 1;
        }
        if j - i >= 2 {
            units.push((j - i) as u16);
            units.push(vals[i]);
        } else {
            let k = (vals.len() - i).min(g.int(1, 4) as usize);
            units.push(0x80 | k as u16);
            units.extend_from_slice(&vals[i..i + k]);
            j = i + k;
        }
        i = j;
    }
    match g.below(14) {
        0 => {}                                // no final 0: the expansion runs on
        1 => units.extend_from_slice(&[5, 7]), // a run past the width
        _ => units.push(0),
    }
    let mut b = Vec::new();
    for u in units {
        if unit == 1 {
            b.push(u as u8);
        } else {
            put16(&mut b, u, be);
        }
    }
    b
}

/// An SGI image file.
fn sgi_file(g: &mut Gen) -> Vec<u8> {
    // big-endian is SGI's order and FreeSurfer's (FS-720 makes those cases VARY); one in three
    // little-endian
    let be = !g.chance(3);
    let wide = g.chance(30);
    let rle = !wide && g.chance(5);
    let mut bpp: u16 = if wide || g.chance(5) { 2 } else { 1 };
    if g.chance(25) {
        bpp = g.pick(&[0u16, 3]);
    }
    let mut itype = if rle { 0x0100 | bpp } else { bpp };
    if g.chance(30) {
        itype = 0x0200 | bpp;
    }
    let (x, y) = if wide {
        (g.pick(&[16384u16, 20000, 32767, 32768, 40000]), 1)
    } else {
        (
            if g.chance(20) { 0 } else { g.int(1, 5) as u16 },
            if g.chance(20) { 0 } else { g.int(1, 4) as u16 },
        )
    };
    let z = if wide {
        1
    } else if g.chance(20) {
        g.pick(&[0u16, 4])
    } else {
        g.pick(&[1u16, 1, 1, 3, 3, 2])
    };
    let mut dim = if z > 1 {
        3
    } else if y > 1 {
        2
    } else {
        g.pick(&[1u16, 2])
    };
    if g.chance(12) {
        dim = g.pick(&[0u16, 3, 4]);
    }
    let magic = if g.chance(25) {
        g.pick(&[0x1234u16, 0xda01])
    } else {
        0o732
    };
    let mut h = Vec::new();
    put16(&mut h, magic, be);
    for v in [itype, dim, x, y, z] {
        put16(&mut h, v, be);
    }
    for _ in 0..3 {
        put32(&mut h, g.below(300) as u32, be);
    }
    let mut name = [0u8; 80];
    name[..7].copy_from_slice(b"no name");
    h.extend_from_slice(&name);
    // colormap, then the library's own fields (FreeSurfer's reader overwrites them)
    while h.len() < 176 {
        h.push(g.below(256) as u8);
    }
    let mut d = h;
    d.resize(512, 0);
    let (nx, ny, nz) = (usize::from(x), usize::from(y), usize::from(z));
    let unit = usize::from(bpp.max(1));
    // (the same draws as before relay 31's lint fix: no chance drawn for 1-byte pixels)
    let px = |g: &mut Gen| -> u16 {
        if unit == 1 || g.chance(3) {
            g.below(256) as u16
        } else {
            g.below(65536) as u16
        }
    };
    if rle {
        let mut rows = Vec::new();
        for _ in 0..ny * nz {
            let vals: Vec<u16> = (0..nx)
                .map(|_| if g.chance(2) { 9 } else { px(g) })
                .collect();
            rows.push(rle_row(g, &vals, unit, be));
        }
        let mut start = 512 + 8 * rows.len() as u32;
        let mut starts = Vec::new();
        let mut sizes = Vec::new();
        for r in &rows {
            starts.push(start);
            let mut s = r.len() as u32;
            if g.chance(20) {
                s = g.pick(&[u32::MAX, 0x0001_0000, 4000]);
            }
            sizes.push(s);
            start += r.len() as u32;
        }
        for s in starts {
            put32(&mut d, s, be);
        }
        for s in sizes {
            put32(&mut d, s, be);
        }
        for r in rows {
            d.extend_from_slice(&r);
        }
    } else {
        for _ in 0..nx * ny * nz {
            let v = px(g);
            if unit == 1 {
                d.push(v as u8);
            } else {
                put16(&mut d, v, be);
            }
        }
    }
    match g.below(12) {
        0 => {
            let cut = g.below(d.len() as u64 + 1) as usize;
            d.truncate(cut);
        }
        1 => d.truncate(g.int(0, 175) as usize),
        2 => d.extend_from_slice(&[1, 2, 3]),
        _ => {}
    }
    d
}

fn read_case(g: &mut Gen, entries: &mut Vec<String>) -> (String, i64) {
    let k = g.below(20);
    let (name, ty): (String, i64) = match k {
        0..=8 => {
            let name = g.pick(&["w/i.rgb", "w/I.RGB", "w/i.rgb@rgb", "w/i.Rgb"]);
            let file = name.split('@').next().unwrap_or(name);
            match g.below(20) {
                0 => entries.push(dir_entry(file)),
                1 => {}
                _ => entries.push(file_entry(file, &sgi_file(g))),
            }
            (name.into(), if g.chance(2) { 21 } else { -1 })
        }
        9..=12 => {
            let name = g.pick(&["w/m.mat", "w/m.MAT", "w/m.mat@tif"]);
            let file = name.split('@').next().unwrap_or(name);
            if !g.chance(15) {
                let b = super::piece5::mat_file(g, true, false);
                entries.push(file_entry(file, &b));
            }
            (name.into(), if g.chance(2) { 21 } else { -1 })
        }
        13 | 14 => {
            // HIPS: any other extension with the image type, or a name with @<image type>
            let name = g.pick(&["w/h.x", "w/h", "w/h.dat@rgb", "w/h.x@mat", "w/d.x"]);
            let file = name.split('@').next().unwrap_or(name);
            if file == "w/d.x" {
                entries.push(dir_entry(file));
            } else if !g.chance(8) {
                entries.push(file_entry(file, b"\x01\x02hips"));
            }
            (name.into(), if name.contains('@') { -1 } else { 21 })
        }
        15 | 16 => {
            let name = g.pick(&["w/p.pgm", "w/p.ppm", "w/p.pbm", "w/p.PGM"]);
            if g.chance(2) {
                entries.push(file_entry(name, b"P5\n1 1\n255\n\x07"));
            }
            (name.into(), if g.chance(2) { 21 } else { -1 })
        }
        _ => {
            let name = g.pick(&["w/t.tif", "w/t.tiff", "w/t.svs", "w/j.jpg", "w/j.jpeg"]);
            if name.contains(".jp") && !g.chance(6) {
                // relay 39: JPEG files of track C's test encoder (IJG 6b decodes them), now and
                // then damaged
                let seed = g.below(1 << 62);
                let mut f = jpeg_cases::make(&jpeg_cases::Recipe::random(seed, 24));
                if g.chance(4) {
                    jpeg_cases::damage(&mut f, seed, g.int(1, 4) as u32);
                }
                entries.push(file_entry(name, &f));
            } else if name.contains(".t") || name.contains(".s") {
                // relay 40: TIFF files of track C's generator (libtiff 3.6.1 reads them), of
                // at most 96 KiB
                if !g.chance(12) {
                    let seed = g.below(1 << 40);
                    let mut f = Vec::new();
                    for i in 0..64 {
                        f = tiff_cases::make(seed, i);
                        if f.len() <= 96 << 10 {
                            break;
                        }
                    }
                    entries.push(file_entry(name, &f));
                } else if !g.chance(3) {
                    entries.push(file_entry(name, b"II*\x00\x08\x00\x00\x00\xff\xd8\xff"));
                }
            } else if !g.chance(3) {
                entries.push(file_entry(name, b"II*\x00\x08\x00\x00\x00\xff\xd8\xff"));
            }
            (name.into(), if g.chance(2) { 21 } else { -1 })
        }
    };
    let tiff = name.contains(".t") || name.contains(".s");
    let name = if g.chance(if tiff { 4 } else { 10 }) {
        format!(
            "{name}#{}",
            g.pick(&["0", "1", "0:1", "x", "-2", "2", "1:2", "0:2", "5", "0:7"])
        )
    } else {
        name
    };
    (name, ty)
}

/// A plain volume in `read_mri14`'s fields: the given shape and type, the voxels (`None`: zeros),
/// coronal axes.
fn plain_volume(t: &mut Vec<String>, w: i64, h: i64, d: i64, ty: i32, nf: i64, vox: Option<&[u8]>) {
    for v in [w, h, d, i64::from(ty), nf, 1] {
        t.push(v.to_string());
    }
    let geom = [
        1.0f32, 1.0, 1.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0,
    ];
    for v in geom {
        t.push(fhex(v));
    }
    t.push("1".into());
    for _ in 0..4 {
        t.push(fhex(0.0));
    }
    t.push(format!("{:016x}", 0f64.to_bits()));
    t.push(fhex(0.0));
    t.push("1".into());
    t.push(match vox {
        Some(b) if !b.is_empty() => hex(b),
        _ => "-".into(),
    });
    t.push(enc(None));
    t.push(enc(None));
    t.push("0".into()); // no command lines
    t.push("0".into());
    t.push(fhex(0.0));
    for _ in 0..2 {
        t.push("-1".into());
        t.push("0".into());
    }
    t.push("-1".into()); // no warp
    t.push("0".into()); // no colour table
    t.push("-1".into()); // no b-values
}

/// A write case: piece 17's `vwr` fields, `how` 31 (`MRIwriteType` as an image) or 1 (`MRIwrite`
/// by the name).
fn write_case(g: &mut Gen, mut t: Vec<String>) -> Vec<String> {
    if g.chance(5) {
        return itk_case(g, t);
    }
    let name = g.pick(&[
        "w/o.rgb",
        "w/o.rgb",
        "w/o.RGB",
        "w/o.mat",
        "w/o.pgm",
        "w/o.ppm",
        "w/o.pbm",
        "w/o.x",
        "w/o",
        "w/o.tif",
        "w/o.tif",
        "w/o.tiff",
        "w/o.jpg",
        "w/no/o.rgb",
        "w/o.rgb#2",
        "w/x#1.rgb",
        "w/d",
        "w/d.rgb#1",
    ]);
    let mut entries = Vec::new();
    if name == "w/d" {
        entries.push(dir_entry("w/d"));
    }
    if name == "w/d.rgb#1" {
        entries.push(dir_entry("w/d.rgb"));
    }
    t.push("N".into());
    t.push("N".into());
    t.push("0".into());
    t.push("N".into());
    t.push(g.pick(&["0", "0", "10000040"]).into());
    t.push(if entries.is_empty() {
        "-".into()
    } else {
        entries.join(",")
    });
    t.push("0".into());
    let by_name = g.chance(6) && name.ends_with(".rgb");
    t.push(if by_name { "1".into() } else { "31".into() });
    let (nf, alloc) = match g.below(40) {
        0 => {
            plain_volume(&mut t, 65536, 1, 1, 0, 1, None);
            (1, true)
        }
        1 => {
            plain_volume(&mut t, 1, 65536, 1, 0, 1, None);
            (1, true)
        }
        2..=26 => {
            // the types images take: bytes, 32-bit integers, floats
            let ty = g.pick(&[0i32, 0, 1, 3]);
            let w = if g.chance(15) { 0 } else { g.int(1, 6) };
            let h = if g.chance(15) { 0 } else { g.int(1, 4) };
            let d = if g.chance(10) { 2 } else { 1 };
            let nf = if g.chance(10) { g.pick(&[0i64, 2]) } else { 1 };
            let size = if ty == 0 { 1 } else { 4 };
            let n = (w * h * d * nf) as usize * size;
            let vox: Vec<u8> = (0..n).map(|_| g.below(256) as u8).collect();
            plain_volume(&mut t, w, h, d, ty, nf, Some(&vox));
            (nf, true)
        }
        _ => {
            let (_, _, _, _, nf, alloc) = super::piece14::volume14(g, &mut t, true, false);
            (nf, alloc)
        }
    };
    t.push("-1".into());
    t.push(g.pick(&[-1i64, 0, 3]).to_string());
    t.push(enc(Some(name.as_bytes())));
    let back = alloc && nf > 0 && (g.chance(3) || name.contains(".tif") && g.chance(2));
    t.push(i64::from(back).to_string());
    t
}

/// An ITK warp write: piece 17's `vwr` fields, `how` 41 (`MRIwriteType` as `ITK_MORPH`): float
/// volumes of three frames (now and then another type, frame count or a width of 32,768), plain
/// or compressed names (a final `z`), a missing folder.
fn itk_case(g: &mut Gen, mut t: Vec<String>) -> Vec<String> {
    let name = g.pick(&["w/w.nii", "w/w.nii.gz", "w/w.mgz", "w/no/w.nii", "w/w"]);
    for v in ["N", "N", "0", "N"] {
        t.push(v.into());
    }
    t.push(g.pick(&["0", "0", "10000040"]).into());
    t.push("-".into());
    t.push("0".into());
    t.push("41".into());
    match g.below(12) {
        0 => plain_volume(&mut t, 32768, 1, 1, 3, 3, None),
        1..=7 => {
            let ty = if g.chance(8) {
                g.pick(&[0i32, 1, 4])
            } else {
                3
            };
            let nf = if g.chance(8) { g.pick(&[1i64, 2]) } else { 3 };
            let (w, h, d) = (g.int(1, 4), g.int(1, 3), g.int(1, 2));
            let size = if ty == 0 {
                1
            } else if ty == 4 {
                2
            } else {
                4
            };
            let n = (w * h * d * nf) as usize * size;
            let vox: Vec<u8> = (0..n).map(|_| g.below(256) as u8).collect();
            plain_volume(&mut t, w, h, d, ty, nf, Some(&vox));
        }
        _ => {
            super::piece14::volume14(g, &mut t, false, false);
        }
    }
    t.push("-1".into());
    t.push(g.pick(&[-1i64, 0, 3]).to_string());
    t.push(enc(Some(name.as_bytes())));
    t.push("0".into());
    t
}

pub fn generate(g: &mut Gen, kind: &str, _portable: bool) -> Vec<String> {
    let t = vec![kind.to_owned()];
    if kind == "ixw" {
        return write_case(g, t);
    }
    let mut t = t;
    let mut entries = Vec::new();
    let (name, ty) = read_case(g, &mut entries);
    let tiff = name.contains(".t") || name.contains(".s");
    // Gdiag (hexadecimal: DIAG_WRITE, DIAG_VERBOSE for the TIFF reader's lines)
    t.push(
        if tiff && g.chance(6) {
            "10000000"
        } else if g.chance(10) {
            "8"
        } else {
            "0"
        }
        .into(),
    );
    t.push(if g.chance(8) { "0" } else { "1" }.into()); // MRIIO_Strip_Pound
    // piece 6's environment bits (1024: FS_TIFF_DEBUG)
    t.push(if tiff && g.chance(8) { "1024" } else { "0" }.into());
    t.push(ty.to_string());
    t.push(enc(Some(name.as_bytes())));
    t.push(if entries.is_empty() {
        "-".into()
    } else {
        entries.join(",")
    });
    t
}

/// The port's answer to a piece-23 case: piece 6's `mrd`/`mrh`, or piece 17's `vwr`.
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
    let r = if tk[0] == "ixw" {
        let mut t: Vec<&str> = vec!["vwr"];
        t.extend_from_slice(&tk[1..]);
        super::piece17::run(&t, g, out, err, o, w, extra)
    } else {
        let mut t: Vec<&str> = vec![if tk[0] == "ixh" { "mrh" } else { "mrd" }];
        t.extend_from_slice(&tk[1..]);
        super::piece6::run_env(&t, g, out, err, o, w, extra, &[])
    };
    // MatReadHeader's message with DIAG_SHOW prints addresses (piece 5's rule), when a write
    // case reads a MATLAB name back
    let mat_name = enc(Some(b"w/o.mat"));
    if tk[0] == "ixw" && tk.get(5) == Some(&"10000040") && tk.contains(&mat_name.as_str()) {
        extra.push("VARY");
    }
    // FS_TIFF_DEBUG's lines print FreeSurfer's heap addresses
    if tk[0] != "ixw" && tk.get(3) == Some(&"1024") {
        extra.push("VARY");
    }
    for n in g.notices.as_slice() {
        let id = n.defect.id;
        // the MATLAB reader's own rules (piece 5)
        if let Some(c) = larmorx_freesurfer_io::matfile::probe_class(id, &n.detail) {
            extra.push(c);
        }
        // FreeSurfer writes or reads past its buffers, crashes on a negative matrix size or
        // divides by zero (its outcome depends on the heap)
        // (FS-290, FS-58, FS-465: fs.volume's reads outside a volume or an image, as its probe
        // has them)
        if matches!(
            id,
            "FS-719"
                | "FS-724"
                | "FS-726"
                | "FS-729"
                | "FS-229"
                | "FS-13"
                | "FS-290"
                | "FS-58"
                | "FS-465"
        ) {
            extra.push("SKIP");
        }
        // FreeSurfer's pixels from memory contents; the 8 bytes it writes past an SGI header
        // (harmless, or a crash in the allocator, as the heap lies)
        if matches!(id, "FS-727" | "FS-720") {
            extra.push("VARY");
        }
        // relay 40: TIFF images: FreeSurfer's stack contents, writes and reads past its buffers,
        // its crashes and hangs on tiles
        if matches!(id, "FS-863" | "FS-865" | "FS-871" | "FS-873") {
            extra.push("SKIP");
        }
        // the upper bytes of FreeSurfer's int (measured: see the findings, Part 35)
        if id == "FS-867" {
            extra.push("VARY");
        }
    }
    r
}
