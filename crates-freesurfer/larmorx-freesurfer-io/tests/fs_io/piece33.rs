// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Piece 33 (track A relay 41): stacked autoencoders read as volumes. `aed` and `aeh` (piece 6's
//! `mrd` and `mrh`: the type -1 or 30, `.sae`, `.ae`, `.csae`, `.SAE` names, `@sae`) of files built
//! here as `SAEwrite` lays them out: the header line (`whalf nencoders scale type nlevels`: windows
//! of one to seven voxels a side, now and then of one voxel (the reader's check run and dump),
//! negative or past 32 bits (FS-879); 0 to 3 levels, scales ordinary, 0, NaN, huge; the 2-D and
//! focused types; 1 to 3 layers, now and then 0, -1 or more than the file holds; numbers missing:
//! FS-877), the volume geometry (valid or not, small or the training volume's extent, now and then
//! missing), then per layer its seven matrices (real, now and then complex or of an unknown type;
//! the window's inputs or fewer (FS-880, FS-925) or more; vectors of other sizes: `MatrixReadFrom`'s
//! refusal; weights with NaN, infinite, subnormal values) and its kernel size. Frames by the name:
//! none (layer 0), `#0` to `#3` (a later layer through the ones before it, FS-921; past the
//! count: the program's end). Faults: the file cut short, a missing file, a folder, an empty file,
//! a text that is not an autoencoder. The environment bit 2048 sets `FREESURFER_SEED` (the reader's
//! random weights, printed by the one-voxel check).

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;

use super::piece4::file_entry;
use super::{Gen, Out, Work, enc, hex};

pub const KINDS: [&str; 2] = ["aed", "aeh"];

fn be32(d: &mut Vec<u8>, v: i32) {
    d.extend_from_slice(&v.to_be_bytes());
}

fn bed(d: &mut Vec<u8>, v: f64) {
    d.extend_from_slice(&v.to_bits().to_be_bytes());
}

/// A weight: now and then a special number.
fn weight(g: &mut Gen) -> f64 {
    match g.below(60) {
        0 => f64::from_bits(0x7ff8_0000_0000_0000),
        1 => f64::from_bits(0xfff4_0000_0000_0001),
        2 => f64::INFINITY,
        3 => f64::NEG_INFINITY,
        4 => 1e-310,
        5 => -0.0,
        6 => 1e40,
        7 => f64::from_bits(1) * 3.0e8,
        _ => g.int(-40, 40) as f64 / 16.0,
    }
}

/// `MatrixWriteInto`: type, rows, columns, then each element (two for a complex one).
fn matrix(g: &mut Gen, d: &mut Vec<u8>, mtype: i32, rows: i32, cols: i32) {
    be32(d, mtype);
    be32(d, rows);
    be32(d, cols);
    let n = i64::from(rows.max(0)) * i64::from(cols.max(0)) * if mtype == 2 { 2 } else { 1 };
    for _ in 0..n.min(1 << 16) {
        let w = weight(g);
        bed(d, w);
    }
}

/// `%.15e` as C writes it.
fn e15(x: f64) -> String {
    let s = format!("{x:.15e}");
    let (m, e) = s.split_once('e').unwrap();
    let e: i32 = e.parse().unwrap();
    format!("{m}e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
}

/// `writeVolGeom`'s text block (the first `lines` of its eight lines).
fn vol_geom(g: &mut Gen, d: &mut Vec<u8>, lines: usize, ext: (i32, i32, i32)) {
    let valid = g.pick(&[1, 1, 1, 0]);
    let mut t = Vec::new();
    t.push(if valid == 0 {
        "valid = 0  # volume info invalid".to_owned()
    } else {
        "valid = 1  # volume info valid".to_owned()
    });
    t.push(format!(
        "filename = {}",
        g.pick(&["/x/y/orig.mgz", "norm.mgz", ""])
    ));
    t.push(format!("volume = {} {} {}", ext.0, ext.1, ext.2));
    let sz = |g: &mut Gen| g.pick(&[1.0, 2.0, 0.5, 1.5]);
    let (a, b, c) = (sz(g), sz(g), sz(g));
    t.push(format!("voxelsize = {} {} {}", e15(a), e15(b), e15(c)));
    let rows: [[f64; 3]; 3] = if g.chance(3) {
        [[0.0, 0.0, -1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
    } else {
        [[-1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]]
    };
    for (k, r) in ["xras  ", "yras  ", "zras  "].iter().zip(rows) {
        t.push(format!("{k} = {} {} {}", e15(r[0]), e15(r[1]), e15(r[2])));
    }
    let cr = |g: &mut Gen| g.int(-40, 40) as f64 / 4.0;
    let (x, y, z) = (cr(g), cr(g), cr(g));
    t.push(format!("cras   = {} {} {}", e15(x), e15(y), e15(z)));
    for l in t.iter().take(lines) {
        d.extend_from_slice(l.as_bytes());
        d.push(b'\n');
    }
}

/// One layer's matrices and kernel size: `ninputs` inputs, `nhidden` hidden units, `noutputs`
/// outputs; now and then a matrix of another type or size.
fn layer(g: &mut Gen, d: &mut Vec<u8>, ninputs: i32, nhidden: i32, noutputs: i32, ksize: i32) {
    fn odd(g: &mut Gen, v: i32) -> i32 {
        if g.chance(25) {
            v + g.pick(&[-1, 1, 2])
        } else {
            v
        }
    }
    let t_ih = match g.below(30) {
        0 => 2,
        1 => 7,
        _ => 1,
    };
    matrix(g, d, t_ih, nhidden, ninputs);
    let c = if g.chance(25) { 2 } else { 1 };
    let t_ob = if g.chance(40) { 2 } else { 1 };
    matrix(g, d, t_ob, noutputs, c);
    let r = odd(g, nhidden);
    matrix(g, d, 1, r, 1);
    let r = odd(g, noutputs);
    matrix(g, d, 1, r, nhidden);
    let r = odd(g, ninputs);
    matrix(g, d, 1, r, 1);
    let r = odd(g, nhidden);
    matrix(g, d, 1, r, 1);
    let r = odd(g, noutputs);
    matrix(g, d, 1, r, 1);
    be32(d, ksize);
}

/// Whether the header's own layer (`SAEalloc`: `(2 whalf + 1)^2` voxels a level, times `2 whalf +
/// 1` in 3-D, in 32-bit arithmetic; `nint(scale n)` hidden units) is small (at most 2,000,000
/// weights) or refused by the allocator at once (a size below 0, or more than 2^31 - 1 floats).
fn header_fits(whalf: i64, nlevels: i64, stype: i32, scale: &str) -> bool {
    let w = (whalf as i32).wrapping_mul(2).wrapping_add(1);
    let mut n = w.wrapping_mul(w).wrapping_mul(nlevels as i32);
    if stype & 8 == 0 {
        n = n.wrapping_mul(w);
    }
    let sc: f64 = scale.parse().unwrap_or(f64::NAN);
    let x = sc * f64::from(n);
    let x = if x < 0.0 { x - 0.5 } else { x + 0.5 };
    let h = if x.is_nan() || x >= 2147483648.0 || x <= -2147483649.0 {
        i32::MIN
    } else {
        x as i32
    };
    let out = if stype & 2 != 0 { 1 } else { i64::from(n) };
    if n < 0 || h < 0 || out < 0 {
        return true;
    }
    let weights = i64::from(h) * i64::from(n) + out * i64::from(h);
    i64::from(h) * i64::from(n) > i64::from(i32::MAX) - 2
        || out * i64::from(h) > i64::from(i32::MAX) - 2
        || weights <= 2_000_000
}

/// An autoencoder file, and whether it holds a one-voxel window (its check prints the random
/// weights).
fn sae_bytes(g: &mut Gen) -> (Vec<u8>, bool) {
    let whalf: i64 = match g.below(40) {
        0..=4 => 0,
        5 => g.pick(&[-1, -2]),
        6 => g.pick(&[1 << 30, 50_000, 1000]),
        7..=10 => 2,
        11 | 12 => 3,
        _ => 1,
    };
    let nlevels: i64 = match g.below(40) {
        0 => g.pick(&[0, -1]),
        1..=8 => 2,
        9 | 10 => 3,
        _ => 1,
    };
    let stype = g.pick(&[1, 1, 1, 9, 9, 2, 10, 0, 8]);
    let nenc: i64 = match g.below(16) {
        0 => g.pick(&[0, -1]),
        1 => 5,
        2..=5 => 2,
        6 | 7 => 3,
        _ => 1,
    };
    let scale = match g.below(60) {
        0 => "nan".to_owned(),
        1 => "inf".to_owned(),
        2 => "0.000000".to_owned(),
        3 => "1e10".to_owned(),
        // a one-voxel window: at least one hidden unit, mostly (its check prints them)
        _ if whalf == 0 && !g.chance(8) => format!("{:.6}", g.pick(&[1.0, 2.0, 0.5, 3.0])),
        _ => format!("{:.6}", g.pick(&[0.5, 0.25, 1.0, 0.1, 0.75])),
    };
    // FreeSurfer's reader allocates and fills the header's layer first: keep it small, or one
    // its allocator refuses at once (a negative size or more numbers than an int holds), never
    // one the probe's memory limit decides
    let whalf = if header_fits(whalf, nlevels, stype, &scale) {
        whalf
    } else {
        1
    };
    let mut d = Vec::new();
    let header = match g.below(50) {
        0 => format!("{whalf} {nenc} {scale}\n"),
        1 => format!("{whalf} {nenc}\n"),
        2 => format!("{whalf} {nenc} {scale} x {nlevels}\n"),
        3 => format!("  {whalf}\t{nenc} {scale} {stype} {nlevels} trailing\n"),
        _ => format!("{whalf} {nenc} {scale} {stype} {nlevels}\n"),
    };
    d.extend_from_slice(header.as_bytes());
    // the layers as the file holds them (the header's window, or another)
    let wsize = (2 * whalf + 1).clamp(0, 9) as i32;
    let depth = if stype & 8 != 0 { 1 } else { wsize };
    let levels = nlevels.clamp(0, 3) as i32;
    let mut ninputs = wsize * wsize * depth * levels;
    if g.chance(12) {
        ninputs = (ninputs - g.int(1, 3) as i32).max(0);
    } else if g.chance(20) {
        ninputs += 2;
    }
    let mut ext = (wsize.max(1), wsize.max(1), depth.max(1));
    if g.chance(4) {
        ext = g.pick(&[(7, 7, 1), (3, 3, 3), (9, 9, 9), (256, 256, 256)]);
    }
    let vlines = if g.chance(14) { g.below(8) as usize } else { 8 };
    vol_geom(g, &mut d, vlines, ext);
    let file_layers = match g.below(20) {
        0 => 0,
        1 => (nenc.max(1) - 1).max(0),
        _ => nenc.clamp(1, 3),
    };
    let mut nhidden = g.pick(&[1, 2, 2, 3, 4, 0]);
    let noutputs = if stype & 2 != 0 { 1 } else { ninputs };
    let ks = g.pick(&[1, 2, 2, 3]);
    layer(g, &mut d, ninputs, nhidden, noutputs, ks);
    for _ in 1..file_layers {
        let ksize = g.pick(&[1, 2, 2, 3, 0, -1]);
        let prev = nhidden;
        let mut nin = prev * ksize.max(0) * ksize.max(0);
        if g.chance(10) {
            nin = (nin - 1).max(0);
        }
        nhidden = g.pick(&[1, 2, 2, 3]);
        let nout = if g.chance(2) { 1 } else { nin };
        layer(g, &mut d, nin, nhidden, nout, ksize);
    }
    (d, whalf == 0)
}

fn read_case(g: &mut Gen, entries: &mut Vec<String>, env: &mut i32) -> (String, i64) {
    let (name, ty) = g.pick(&[
        ("w/a.sae", -1i64),
        ("w/a.sae", -1),
        ("w/a.ae", -1),
        ("w/a.csae", -1),
        ("w/a.SAE", -1),
        ("w/a.sae", 30),
        ("w/a.dat@sae", -1),
    ]);
    let file = name.split('@').next().unwrap_or(name);
    let mut one_voxel = false;
    match g.below(40) {
        0 => entries.push(format!("D{}", hex(file.as_bytes()))),
        1 => {}
        2 => entries.push(file_entry(file, b"not an autoencoder at all\n")),
        3 => entries.push(file_entry(file, b"")),
        4..=7 => {
            let (mut b, ov) = sae_bytes(g);
            one_voxel = ov;
            let cut = g.below(b.len() as u64 + 1) as usize;
            b.truncate(cut);
            entries.push(file_entry(file, &b));
        }
        _ => {
            let (b, ov) = sae_bytes(g);
            one_voxel = ov;
            entries.push(file_entry(file, &b));
        }
    }
    // FREESURFER_SEED for most one-voxel windows (their check prints random weights)
    if (one_voxel && !g.chance(5)) || g.chance(10) {
        *env |= 2048;
    }
    let frame: Option<i32> = match g.below(12) {
        0..=5 => None,
        6 | 7 => Some(1),
        8 => Some(2),
        9 => Some(g.pick(&[3, 5])),
        _ => Some(0),
    };
    let name = match frame {
        None => name.to_owned(),
        Some(f) => format!("{name}#{f}"),
    };
    (name, ty)
}

pub fn generate(g: &mut Gen, kind: &str, _portable: bool) -> Vec<String> {
    let mut t = vec![kind.to_owned()];
    let mut entries = Vec::new();
    let mut env = 0;
    let (name, ty) = read_case(g, &mut entries, &mut env);
    t.push(g.pick(&["0", "0", "0", "40"]).into()); // Gdiag
    t.push(if g.chance(12) { "0" } else { "1" }.into()); // MRIIO_Strip_Pound
    t.push(env.to_string()); // piece 6's environment bits (2048: FREESURFER_SEED)
    t.push(ty.to_string());
    t.push(enc(Some(name.as_bytes())));
    t.push(if entries.is_empty() {
        "-".into()
    } else {
        entries.join(",")
    });
    t
}

/// The port's answer to a piece-33 case: piece 6's `mrd`/`mrh`.
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
    // the process's name, which randomNumber prints without a seed
    g.comm = "fsio".into();
    let mut t: Vec<&str> = vec![if tk[0] == "aeh" { "mrh" } else { "mrd" }];
    t.extend_from_slice(&tk[1..]);
    let mut text = Vec::new();
    let r = super::piece6::run_env(&t, g, &mut text, err, o, w, extra, &[]);
    let _ = out.write_all(&text);
    let dumped =
        text.windows(4).any(|x| x == b"f(I=") || text.windows(9).any(|x| x == b"\nv_input\n");
    for n in g.notices.as_slice() {
        // FreeSurfer crashes, writes outside its memory or reads outside the volume's voxels
        if matches!(
            n.defect.id,
            "FS-878" | "FS-922" | "FS-923" | "FS-924" | "FS-926" | "FS-53"
        ) || (n.defect.id == "FS-921" && n.detail.ends_with("refused"))
        {
            extra.push("SKIP");
        }
        // memory contents: the header's missing numbers, weights past the matrix, the clock's
        // random weights printed
        if n.defect.id == "FS-877"
            || n.defect.id == "FS-925"
            || (n.defect.id == "FS-04" && dumped && g.notices.reproduce())
        {
            extra.push("VARY");
        }
    }
    r
}
