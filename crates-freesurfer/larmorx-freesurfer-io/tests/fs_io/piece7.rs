// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.io probe's piece-7 cases (relay 17): Bruker ParaVision studies. `bsp`
//! (`splitParameterValue`), `bd3` (`readBrukerD3proc`), `brc` (`readBrukerReco`), `bacq`
//! (`readBrukerAcqp`; the probe's values and transform start at -7, as a caller that initialises
//! them), `bvx` (`buildVoxToRASTransform` of a given transform on a header of the given sizes),
//! `bkr` (`brukerRead`, the header or the voxels), `bmr` (`mri_read` of a study, its type
//! given). The studies are built here from ParaVision's parameter format (`##$NAME=value`, an
//! array's numbers on the next line), mostly complete, with odd and missing lines, CR LF line
//! ends, overlong values, unsupported types, short and long 2dseq files.

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_io::bruker::{self, BrukerTransform};
use larmorx_freesurfer_io::{identify, mriio};
use larmorx_freesurfer_mri::mri::{MRI_SHORT, mri_alloc_header};

use super::piece4::{file_entry, setup};
use super::piece6::print_mri;
use super::{Gen, Out, Work, dec, enc, hex};

/// The piece-7 commands.
pub const KINDS: [&str; 7] = ["bsp", "bd3", "brc", "bacq", "bvx", "bkr", "bmr"];

// ---------------------------------------------------------------- generation

fn num(g: &mut Gen) -> String {
    match g.below(16) {
        0 => g
            .pick(&[
                "nan", "-nan", "inf", "-0", "1e400", "1e-400", "x", "", "0x10", "3.5e",
            ])
            .into(),
        1 => g.int(-5, 0).to_string(),
        2 => format!("{}.{}", g.int(-9, 9), g.int(0, 99)),
        _ => (g.int(1, 64) as f64 / 8.0).to_string(),
    }
}

fn size_num(g: &mut Gen) -> i64 {
    if g.chance(15) {
        g.pick(&[0, -1, 9])
    } else {
        g.int(1, 5)
    }
}

/// One `##name=value` line (sometimes odd: CR LF, text before `##`, no `=`, parentheses).
fn line(g: &mut Gen, t: &mut Vec<u8>, name: &str, value: &str) {
    if g.chance(40) {
        t.extend_from_slice(b"x ");
    }
    t.extend_from_slice(b"##");
    t.extend_from_slice(name.as_bytes());
    if !g.chance(60) {
        t.push(b'=');
    }
    t.extend_from_slice(value.as_bytes());
    if g.chance(50) {
        t.extend(std::iter::repeat_n(b'v', g.pick(&[130, 300, 600])));
    }
    t.extend_from_slice(if g.chance(15) { b"\r\n" } else { b"\n" });
}

fn next_line(g: &mut Gen, t: &mut Vec<u8>, values: &[String]) {
    t.extend_from_slice(values.join(" ").as_bytes());
    if g.chance(20) {
        t.extend_from_slice(b" ");
    }
    t.push(b'\n');
}

/// Keeps a parameter (every one when `complete`).
fn keep(g: &mut Gen, complete: bool) -> bool {
    complete || !g.chance(5)
}

struct Study {
    reco: Vec<u8>,
    d3proc: Vec<u8>,
    acqp: Vec<u8>,
    /// The voxels' bytes as FreeSurfer reads them (2 per voxel) and the volume's in the default.
    voxels: usize,
}

fn reco(g: &mut Gen, complete: bool, d3: &mut i64) -> (Vec<u8>, bool) {
    let mut t = b"##TITLE=Parameter List\n##ORIGIN=Bruker BioSpin GmbH\n".to_vec();
    let mut int = false;
    if keep(g, complete) {
        line(g, &mut t, "$RECO_mode", "FFT");
    }
    if keep(g, complete) {
        let wt = if complete {
            g.pick(&["_16BIT_SGN_INT", "_16BIT_SGN_INT", "_32BIT_SGN_INT"])
        } else {
            g.pick(&[
                "_16BIT_SGN_INT",
                "_32BIT_SGN_INT",
                "_8BIT_UNSGN_INT",
                "_16BIT_SGN_INX",
                "_32BIT_FLOAT",
            ])
        };
        int = wt.starts_with("_32BIT_SGN");
        line(g, &mut t, "$RECO_wordtype", wt);
    }
    if g.chance(2) {
        let bo = g.pick(&["littleEndian", "littleEndian", "bigEndian"]);
        line(g, &mut t, "$RECO_byte_order", bo);
    }
    if keep(g, complete) {
        line(g, &mut t, "$RECO_transposition", "( 1 )");
        let tr = g.pick(&["0", "0", "1", "2"]).to_string();
        next_line(g, &mut t, &[tr]);
    }
    let dim = if complete {
        g.pick(&[3, 3, 2])
    } else {
        g.pick(&[3, 3, 2, 1, 4])
    };
    for name in ["$RECO_fov", "$RECO_size", "$RECO_ft_size"] {
        if !keep(g, complete) {
            continue;
        }
        line(g, &mut t, name, &format!("( {dim} )"));
        if !complete && g.chance(25) {
            continue; // the array's line missing (or a later parameter's line taken)
        }
        let n = if g.chance(10) && !complete {
            g.int(0, 4)
        } else {
            dim
        };
        let vals: Vec<String> = (0..n)
            .map(|k| {
                if name == "$RECO_fov" {
                    num(g)
                } else if name == "$RECO_size" && k == 2 {
                    d3.to_string()
                } else {
                    size_num(g).to_string()
                }
            })
            .collect();
        next_line(g, &mut t, &vals);
    }
    if g.chance(3) {
        t.extend_from_slice(b"##END=\n");
    }
    if g.chance(10) {
        t.extend_from_slice(b"##$RECO_wordtype=_8BIT_UNSGN_INT\n");
    }
    (t, int)
}

fn d3proc(g: &mut Gen, complete: bool) -> (Vec<u8>, [i64; 3]) {
    let mut t = b"##TITLE=Parameter List, ParaVision 6\n".to_vec();
    let s = [size_num(g), size_num(g), size_num(g)];
    if keep(g, complete) {
        let dt = if complete {
            "ip_short"
        } else {
            g.pick(&["ip_short", "ip_int", "ip_float", "ip_short\r"])
        };
        line(g, &mut t, "$DATTYPE", dt);
    }
    let mut depth = s[2];
    for (k, name) in ["$IM_SIX", "$IM_SIY", "$IM_SIZ"].iter().enumerate() {
        if keep(g, complete) {
            line(g, &mut t, name, &s[k].to_string());
        }
    }
    if g.chance(3) {
        let sit = g.int(1, 3);
        line(g, &mut t, "$IM_SIT", &sit.to_string());
        depth = sit;
    }
    if g.chance(3) {
        t.extend_from_slice(b"##END=\n##$IM_SIX=7\n");
    }
    (t, [s[0], s[1], depth])
}

fn acqp(g: &mut Gen, complete: bool) -> Vec<u8> {
    let mut t = b"##TITLE=Parameter List\n".to_vec();
    if g.chance(3) {
        line(g, &mut t, "OWNER", "");
        let n = if complete { 2 } else { g.int(0, 2) };
        for k in 0..n {
            t.extend_from_slice(format!("nmrsu {k}\n").as_bytes());
        }
        if n < 2 && !complete {
            return t; // OWNER at the end of the file
        }
    }
    if keep(g, complete) {
        let d = g.pick(&["3", "3", "2", "( 1 )"]);
        line(g, &mut t, "$ACQ_dim", d);
    }
    if keep(g, complete) {
        let v = num(g);
        line(g, &mut t, "$ACQ_flip_angle", &v);
    }
    for name in [
        "$ACQ_repetition_time",
        "$ACQ_echo_time",
        "$ACQ_inversion_time",
        "$ACQ_read_offset",
        "$ACQ_phase1_offset",
        "$ACQ_slice_offset",
        "$ACQ_grad_matrix",
    ] {
        if !keep(g, complete) {
            continue;
        }
        let grad = name == "$ACQ_grad_matrix";
        line(g, &mut t, name, if grad { "( 1, 3, 3 )" } else { "( 1 )" });
        if !complete && g.chance(30) {
            return t; // its line missing at the end of the file
        }
        let n = if grad {
            if complete { 9 } else { g.int(0, 9) }
        } else {
            1
        };
        let vals: Vec<String> = (0..n)
            .map(|_| {
                if grad && g.chance(2) {
                    g.pick(&["0", "1", "-1", "0.7071067811865476"]).to_string()
                } else {
                    num(g)
                }
            })
            .collect();
        next_line(g, &mut t, &vals);
    }
    t.extend_from_slice(b"##END=\n");
    t
}

fn study(g: &mut Gen, complete: bool) -> Study {
    let (d3, s) = d3proc(g, complete);
    let mut depth3 = s[2];
    let (reco, int) = reco(g, complete, &mut depth3);
    let acqp = acqp(g, complete);
    let n = (s[0].max(0) * s[1].max(0) * s[2].max(0)) as usize;
    Study {
        reco,
        d3proc: d3,
        acqp,
        voxels: n * if int { 4 } else { 2 },
    }
}

fn study_case(g: &mut Gen) -> (String, String) {
    let complete = !g.chance(4);
    let st = study(g, complete);
    let mut entries = vec![];
    for d in ["w/s", "w/s/pdata", "w/s/pdata/1"] {
        entries.push(format!("D{}", hex(d.as_bytes())));
    }
    let mut files: Vec<(&str, Vec<u8>)> = vec![
        ("w/s/method", b"##$Method=RARE\n".to_vec()),
        ("w/s/acqp", st.acqp),
        ("w/s/pdata/1/d3proc", st.d3proc),
        ("w/s/pdata/1/reco", st.reco),
    ];
    let len = match g.below(12) {
        0 => st.voxels / 2,
        1 => st.voxels + 7,
        2 => st.voxels / 2 + 1,
        _ => st.voxels,
    };
    let data: Vec<u8> = (0..len).map(|_| g.below(256) as u8).collect();
    files.push(("w/s/pdata/1/2dseq", data));
    if g.chance(12) {
        let k = g.int(0, files.len() as i64 - 1) as usize;
        files.remove(k);
    }
    for (n, b) in &files {
        entries.push(file_entry(n, b));
    }
    let name = match g.below(14) {
        0 => "w/none",
        1 => "w/s/acqp",
        _ => "w/s",
    };
    (name.to_owned(), entries.join(","))
}

fn file_case(g: &mut Gen, text: Vec<u8>) -> (String, String) {
    let name = if g.chance(15) { "w/none" } else { "w/p" };
    (name.to_owned(), file_entry("w/p", &text))
}

fn dhex(g: &mut Gen) -> String {
    let v: f64 = match g.below(14) {
        0 => f64::NAN,
        1 => f64::INFINITY,
        2 => -0.0,
        3 => 0.0,
        _ => g.int(-24, 24) as f64 / 8.0,
    };
    format!("{:016x}", v.to_bits())
}

pub fn generate(g: &mut Gen, kind: &str, _portable: bool) -> Vec<String> {
    let mut t = vec![kind.to_owned()];
    match kind {
        "bsp" => {
            let mut l = Vec::new();
            let name = g.pick(&["$RECO_fov", "END", "", "$A=B", "OWNER"]);
            let value = g.pick(&["( 3 )", "ip_short", "", "a(b)c", "x\ny", "=="]);
            line(g, &mut l, name, value);
            if g.chance(6) {
                l.retain(|&c| c != b'#');
            }
            if g.chance(10) {
                l.insert(0, b'#');
            }
            t.push(hex(&l));
        }
        "bd3" | "brc" | "bacq" => {
            let complete = g.chance(3);
            let text = match kind {
                "bd3" => d3proc(g, complete).0,
                "brc" => {
                    let mut d = 3;
                    reco(g, complete, &mut d).0
                }
                _ => acqp(g, complete),
            };
            let (name, spec) = file_case(g, text);
            t.push(enc(Some(name.as_bytes())));
            t.push(spec);
        }
        "bvx" => {
            for _ in 0..3 {
                t.push(g.int(0, 6).to_string());
            }
            t.push(g.pick(&[0, 0, 1, 2]).to_string());
            t.push(g.pick(&[3, 3, 2, 1]).to_string());
            for _ in 0..18 {
                t.push(dhex(g));
            }
        }
        _ => {
            if kind == "bkr" {
                t.push(if g.chance(10) { "40" } else { "0" }.into());
                t.push(if g.chance(4) { "0" } else { "1" }.into());
            }
            let (name, spec) = study_case(g);
            t.push(enc(Some(name.as_bytes())));
            t.push(spec);
        }
    }
    t
}

// ---------------------------------------------------------------- the port's answers

fn dh(o: &mut Out, v: Option<f64>) {
    match v {
        Some(v) => o.x(&v.to_bits().to_be_bytes(), 8),
        None => o.x(&[0; 8], 0),
    }
}
fn fh(o: &mut Out, v: f32) {
    o.x(&v.to_bits().to_be_bytes(), 4);
}
fn oi(o: &mut Out, v: Option<i32>) {
    match v {
        Some(v) => o.i(i64::from(v)),
        None => o.0.push_str("?? "),
    }
}
fn ostr(o: &mut Out, s: &[u8]) {
    if s.is_empty() {
        o.0.push_str("x ");
    } else {
        o.0.push('x');
        o.bytes(s);
    }
}

fn print_bt(o: &mut Out, t: &BrukerTransform) {
    oi(o, t.transposition);
    for v in t.fov {
        dh(o, v);
    }
    for v in t.size.iter().chain(&t.ft_size) {
        oi(o, *v);
    }
    for v in t.vox_size.iter().chain(&t.offset) {
        dh(o, *v);
    }
    for v in [t.read_offset, t.phase1_offset, t.slice_offset] {
        dh(o, v);
    }
    for v in t.grad_matrix {
        dh(o, v);
    }
    oi(o, t.mtype);
    oi(o, t.dim);
}

/// The port's answer to a piece-7 case (`Err`: the program's exit status).
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
        // FreeSurfer writes past its buffers or crashes there
        if matches!(n.defect.id, "FS-259" | "FS-403") {
            extra.push("SKIP");
        }
        // FreeSurfer's values from memory never written
        if n.defect.id == "FS-401" {
            extra.push("VARY");
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
    let fatal = |f: larmorx_freesurfer_core::log::Fatal| f.status();
    let dir = Some(w.fix.as_path());
    let mut i = 1;
    let mut next = || {
        let s = tk[i];
        i += 1;
        s
    };
    match tk[0] {
        "bsp" => {
            let raw = super::unhex(next());
            let mut l = raw[..raw.iter().position(|&c| c == 0).unwrap_or(raw.len())].to_vec();
            match bruker::split_parameter_value(g, &mut l) {
                None => o.i(0),
                Some((p, v)) => {
                    o.i(1);
                    ostr(o, &p);
                    ostr(o, &v);
                    ostr(o, &l);
                }
            }
        }
        "bd3" | "brc" | "bacq" => {
            let name = String::from_utf8(dec(next()).unwrap()).unwrap();
            setup(w, next());
            g.errno = 0;
            let mut t = BrukerTransform::filled(-7.0, -7);
            if tk[0] == "bd3" {
                let mut size = [Some(-7); 3];
                let mut ty = Some(-7);
                let r = bruker::read_bruker_d3proc(g, err, dir, &name, &mut size, &mut ty);
                o.i(i64::from(r));
                for v in size {
                    oi(o, v);
                }
                oi(o, ty);
                o.i(-7);
            } else if tk[0] == "brc" {
                let r = bruker::read_bruker_reco(g, out, err, dir, &name, &mut t);
                o.i(i64::from(r));
                print_bt(o, &t);
            } else {
                let mut times = [Some(-7.0); 3];
                let mut flip = Some(-7.0);
                let r = bruker::read_bruker_acqp(
                    g, out, err, dir, &name, &mut times, &mut flip, &mut t,
                );
                o.i(i64::from(r));
                for v in times {
                    dh(o, v);
                }
                dh(o, flip);
                print_bt(o, &t);
            }
            o.i(i64::from(g.errno));
        }
        "bvx" => {
            let (wd, ht, dp): (i32, i32, i32) = (
                next().parse().unwrap(),
                next().parse().unwrap(),
                next().parse().unwrap(),
            );
            let mut t = BrukerTransform::filled(-7.0, -7);
            t.transposition = Some(next().parse().unwrap());
            t.dim = Some(next().parse().unwrap());
            let mut d = || Some(f64::from_bits(u64::from_str_radix(next(), 16).unwrap()));
            for k in 0..3 {
                t.vox_size[k] = d();
            }
            for k in 0..3 {
                t.offset[k] = d();
            }
            t.read_offset = d();
            t.phase1_offset = d();
            t.slice_offset = d();
            for k in 0..9 {
                t.grad_matrix[k] = d();
            }
            let mut m = mri_alloc_header(g, out, err, wd, ht, dp, MRI_SHORT, 1).map_err(fatal)?;
            bruker::build_vox_to_ras_transform(g, out, err, &mut m, &t).map_err(fatal)?;
            let v = &m.vg;
            for x in [
                v.c_r, v.c_a, v.c_s, v.x_r, v.x_a, v.x_s, v.y_r, v.y_a, v.y_s, v.z_r, v.z_a, v.z_s,
                v.xsize, v.ysize, v.zsize,
            ] {
                fh(o, x);
            }
            o.i(i64::from(v.ras_good_flag));
        }
        _ => {
            let (gd, rv) = if tk[0] == "bkr" {
                (u64::from_str_radix(next(), 16).unwrap(), next() != "0")
            } else {
                (0, true)
            };
            g.gdiag = gd;
            let name = String::from_utf8(dec(next()).unwrap()).unwrap();
            setup(w, next());
            g.errno = 0;
            let m = if tk[0] == "bkr" {
                bruker::bruker_read(g, out, err, dir, &name, rv)
            } else {
                let env = Environment::from_pairs(Vec::<(String, String)>::new());
                mriio::mri_read(
                    g,
                    out,
                    err,
                    &env,
                    dir,
                    &name,
                    identify::BRUKER_FILE,
                    true,
                    -1,
                    -1,
                )
            }
            .map_err(fatal)?;
            print_mri(o, w, m.as_ref());
            o.i(i64::from(g.errno));
        }
    }
    Ok(())
}
