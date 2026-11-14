// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of relay 22: `gwr`, the GIFTI writers (`MRISwriteGIFTI` of every intent,
//! its volume overload, `MRISwriteGIFTICombined`, `mriWriteGifti`, `MRISwrite` of a `.gii` name)
//! on a surface built by `MRISalloc` (coordinates, curvatures, stats, annotations, faces, its name,
//! subject, patch flag, real RAS, volume geometry, centre, group-average area, command lines,
//! Talairach matrix, colour table) and generated volumes, each file printed with its data
//! decompressed (`gii_normal`); `getShapeStatIntentCount` and `MRISreadGiftiAsMRI` on generated
//! GIFTI files and on the written ones.

use super::gii::gii_file;
use super::life::set_vg;
use super::sfile::utf8;
use super::srd::xfm_file;
use super::tri::{coord, printable, vg_tokens, vg_values};
use super::*;

use std::io::Read;

use flate2::read::ZlibDecoder;
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, matrix_alloc};
use larmorx_freesurfer_io::gifti_vol;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::ctab::{ColorTable, ColorTableEntry};
use larmorx_freesurfer_mri::mri::{Mri, mri_alloc_sequence};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_surf::gifti::OverlayInfo;
use larmorx_freesurfer_surf::{alloc, gifti_write, readwrite};

const STATS: [i64; 6] = [2, 3, 5, 16, 24, 1001];

fn intent_w(g: &mut Gen) -> i64 {
    match g.below(12) {
        0 | 1 => 1008,
        2 => 1009,
        3 | 4 => 2005,
        5 | 6 => 1002,
        7 => 2004,
        8 | 9 => g.pick(&STATS),
        10 => g.pick(&[0, 2001, 1007]),
        _ => g.int(2, 24),
    }
}

fn intent_wm(g: &mut Gen) -> i64 {
    match g.below(8) {
        0 => 1008,
        1..=3 => 2005,
        4..=6 => g.pick(&STATS),
        _ => g.pick(&[0, 1002, 2004]),
    }
}

/// A written file's name: in `d` (a surface's name now and then), at the top, or in a folder that
/// does not exist.
fn out_name(g: &mut Gen, k: usize) -> String {
    match g.below(8) {
        0 => format!("nodir/o{k}.gii"),
        1 => format!("o{k}.gii"),
        2 => format!("d/lh.white{k}.gii"),
        3 => format!("d/rh.pial.{k}.gii"),
        _ => format!("d/o{k}.gii"),
    }
}

fn opt_name(t: &mut Vec<String>, g: &mut Gen, names: &[&str]) {
    if g.chance(4) {
        ix(t, 0);
    } else {
        ix(t, 1);
        hx(t, g.pick(names).as_bytes());
    }
}

fn value(g: &mut Gen, heavy: bool) -> String {
    let v = if heavy && g.chance(12) {
        f32::from_bits(g.pick(&[0x7fc0_0000, 0xff80_0000, 0x7f80_0000, 0x8000_0000]))
    } else {
        g.int(-400, 400) as f32 / 8.0
    };
    format!("{:08x}", v.to_bits())
}

pub(super) fn gwr_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(4);
    ix(t, g.int(0, 3));
    hx(
        t,
        g.pick(&["Thu Nov 13 20:55:19 2026", "", "probe date"])
            .as_bytes(),
    );
    // the input files: GIFTI files (or random bytes), a subject's talairach.xfm
    let nin = g.int(0, 2);
    let xfm = g.chance(3);
    ix(t, nin + i64::from(xfm));
    let mut inputs = Vec::new();
    for k in 0..nin {
        let b = if g.chance(20) {
            (0..g.int(0, 40)).map(|_| g.below(256) as u8).collect()
        } else {
            gii_file(g, heavy).0
        };
        let name = format!("d/in{k}.gii");
        hx(t, name.as_bytes());
        hx(t, &b);
        inputs.push(name);
    }
    if xfm {
        hx(t, b"mri/transforms/talairach.xfm");
        hx(t, &xfm_file(g));
    }
    // the surface
    let nv = if g.chance(12) { 0 } else { g.int(1, 24) };
    let nf = if nv == 0 { 0 } else { g.int(0, 2 * nv) };
    ix(t, nv);
    ix(t, nf);
    // the colour table's colours, for annotations that find them
    let colours: Vec<(i64, i64, i64)> = (0..6)
        .map(|_| (g.int(0, 255), g.int(0, 255), g.int(0, 255)))
        .collect();
    for _ in 0..nv {
        for _ in 0..3 {
            t.push(format!("{:08x}", coord(g, heavy).to_bits()));
        }
        t.push(value(g, heavy));
        t.push(value(g, heavy));
        let annot = if g.chance(4) {
            g.int(-1, 0xff_ffff)
        } else {
            let (r, gg, b) = g.pick(&colours);
            r | (gg << 8) | (b << 16)
        };
        ix(t, annot);
    }
    for _ in 0..nf {
        for _ in 0..3 {
            let v = if heavy && g.chance(10) {
                g.pick(&[-1, nv, 1 << 20])
            } else {
                g.int(0, nv - 1)
            };
            ix(t, v);
        }
    }
    let fname = g.pick(&[
        "lh.white",
        "/s/surf/rh.pial",
        "lh.inflated",
        "lh.orig",
        "rh.sphere.reg",
        "lh.smoothwm",
        "lh.graymid",
        "lh.pial-outer-smoothed",
        "rh.qsphere.nofix",
        "x",
        "",
    ]);
    hx(t, fname.as_bytes());
    hx(t, g.pick(&["", "", "bert", "sub-01"]).as_bytes());
    ix(t, i64::from(g.chance(6)));
    ix(t, i64::from(g.chance(2)));
    vg_tokens(t, &vg_values(g));
    for _ in 0..3 {
        t.push(value(g, heavy));
    }
    let area: f32 = match g.below(6) {
        0 | 1 => 0.0,
        2 => 1e-8,
        3 => f32::from_bits(0x7fc0_0000),
        _ => g.int(1, 90000) as f32 / 16.0,
    };
    t.push(format!("{:08x}", area.to_bits()));
    let nl = if g.chance(2) { 0 } else { g.int(1, 3) };
    ix(t, nl);
    for _ in 0..nl {
        let b = if heavy && g.chance(4) {
            printable(g, 1000, 1100)
        } else {
            printable(g, 0, 30)
        };
        hx(t, &b);
    }
    if g.chance(3) {
        ix(t, 1);
        let (rows, cols) = if g.chance(6) { (3, 4) } else { (4, 4) };
        ix(t, rows);
        ix(t, cols);
        for _ in 0..rows * cols {
            t.push(format!("{:08x}", (g.int(-16, 16) as f32 / 4.0).to_bits()));
        }
    } else {
        ix(t, 0);
    }
    if g.chance(4) {
        ix(t, -1);
    } else {
        let n = g.int(0, 6);
        ix(t, n);
        for k in 0..n {
            let present = !g.chance(6);
            ix(t, i64::from(present));
            if present {
                let name = if g.chance(8) {
                    String::new()
                } else {
                    format!("label{k}")
                };
                hx(t, name.as_bytes());
                let (r, gg, b) = colours[k as usize % colours.len()];
                for x in [r, gg, b] {
                    ix(t, x);
                }
                ix(t, g.int(0, 1) * 255);
            }
        }
    }
    // the volumes
    let nvol = g.int(1, 3);
    ix(t, nvol);
    for _ in 0..nvol {
        let ty = g.pick(&[3, 3, 0, 1, 4]);
        let w = if g.chance(4) { g.int(1, 30) } else { nv.max(1) };
        let (h, d) = if g.chance(4) {
            (g.int(1, 3), g.int(1, 2))
        } else {
            (1, 1)
        };
        let fr = g.pick(&[1, 1, 1, 2, 3]);
        for x in [ty, w, h, d, fr] {
            ix(t, x);
        }
        t.push(format!("{:08x}", (g.int(0, 4000) as f32 / 8.0).to_bits()));
        for _ in 0..w * h * d * fr {
            t.push(value(g, heavy));
        }
    }
    let nops = g.int(1, 4);
    ix(t, nops);
    let mut written: Vec<String> = Vec::new();
    for k in 0..nops as usize {
        let op = match g.below(10) {
            0..=2 => "w",
            3 => "wm",
            4 => "c",
            5 => "mw",
            6 => "sw",
            7 => "n",
            _ => "r",
        };
        t.push(op.to_owned());
        let curv = [
            "lh.thickness",
            "/s/surf/rh.curv",
            "lh.sulc.pial",
            "x.area.volume",
            "a",
            "",
        ];
        match op {
            "w" => {
                ix(t, intent_w(g));
                let n = out_name(g, k);
                hx(t, n.as_bytes());
                opt_name(t, g, &curv);
                written.push(n);
            }
            "wm" => {
                ix(t, intent_wm(g));
                ix(t, g.int(0, nvol - 1));
                let n = out_name(g, k);
                hx(t, n.as_bytes());
                opt_name(t, g, &curv);
                opt_name(t, g, &["Thickness", "", "SulcalDepth"]);
                written.push(n);
            }
            "c" => {
                let n = out_name(g, k);
                hx(t, n.as_bytes());
                let nov = g.int(0, 3);
                ix(t, nov);
                for _ in 0..nov {
                    let ty = match g.below(6) {
                        0 | 1 => g.int(100, 104),
                        2 => 200,
                        3 => g.int(201, 224),
                        _ => g.pick(&[100, 202, -1, 999]),
                    };
                    ix(t, ty);
                    ix(t, g.pick(&[0, 3, 5, 24, 1001]));
                    ix(
                        t,
                        if heavy && g.chance(6) {
                            -1
                        } else {
                            g.int(0, nvol - 1)
                        },
                    );
                    opt_name(t, g, &curv);
                    hx(t, g.pick(&["", "Thickness", "CurvatureRadial"]).as_bytes());
                }
                written.push(n);
            }
            "mw" => {
                ix(t, g.int(0, nvol - 1));
                let n = out_name(g, k);
                hx(t, n.as_bytes());
                written.push(n);
            }
            "sw" => {
                let n = format!("d/{}s{k}.gii", g.pick(&["lh.", "rh.", ""]));
                hx(t, n.as_bytes());
                written.push(n);
            }
            _ => {
                let mut names: Vec<String> = inputs.clone();
                names.extend(written.iter().cloned());
                let n = if names.is_empty() || g.chance(10) {
                    "none.gii".to_owned()
                } else {
                    names[g.below(names.len() as u64) as usize].clone()
                };
                hx(t, n.as_bytes());
                if op == "r" {
                    ix(t, i64::from(!g.chance(4)));
                }
            }
        }
    }
}

fn b64val(c: u8) -> Option<u32> {
    Some(match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => return None,
    } as u32)
}

fn find(s: &[u8], pat: &[u8], from: usize) -> Option<usize> {
    if from > s.len() {
        return None;
    }
    s[from..]
        .windows(pat.len())
        .position(|w| w == pat)
        .map(|p| p + from)
}

/// The probe's `gii_normal`: the `UserName` value `U`, each `<Data>` payload Base64-decoded and
/// inflated, `Z` and its bytes in hex (`B` and the payload when it does not inflate).
fn gii_normal(f: &[u8]) -> Vec<u8> {
    let mut s = f.to_vec();
    if let Some(u) = find(&s, b"<![CDATA[UserName]]></Name>", 0)
        && let Some(v) = find(&s, b"<Value><![CDATA[", u)
    {
        let v = v + 16;
        if let Some(e) = find(&s, b"]]>", v) {
            s.splice(v..e, b"U".iter().copied());
        }
    }
    let mut r = Vec::new();
    let mut p = 0;
    loop {
        let a = find(&s, b"<Data>", p);
        let b = a.and_then(|a| find(&s, b"</Data>", a + 6));
        let (Some(a), Some(b)) = (a, b) else {
            r.extend_from_slice(&s[p..]);
            break;
        };
        let a = a + 6;
        r.extend_from_slice(&s[p..a]);
        let pay = &s[a..b];
        let mut bin = Vec::new();
        let (mut acc, mut nb) = (0u32, 0);
        for &c in pay {
            let Some(v) = b64val(c) else { continue };
            acc = (acc << 6) | v;
            nb += 6;
            if nb >= 8 {
                nb -= 8;
                bin.push(((acc >> nb) & 0xff) as u8);
            }
        }
        let mut inf = Vec::new();
        let ok = ZlibDecoder::new(&bin[..]).read_to_end(&mut inf).is_ok();
        if ok {
            r.push(b'Z');
            for c in inf {
                r.extend_from_slice(format!("{c:02x}").as_bytes());
            }
        } else {
            r.push(b'B');
            r.extend_from_slice(pay);
        }
        p = b;
    }
    r
}

fn dump_gii_file(dir: &std::path::Path, name: &str, o: &mut Out) {
    let Ok(b) = std::fs::read(dir.join(name)) else {
        o.s("NOFILE");
        return;
    };
    let n = gii_normal(&b);
    o.0 += "x";
    for c in n {
        o.0 += &format!("{c:02x}");
    }
    o.0 += " ";
}

fn dump_ctab(ct: Option<&ColorTable>, o: &mut Out) {
    let Some(ct) = ct else {
        o.s("NULL");
        return;
    };
    o.i(i64::from(ct.nentries()));
    o.i(i64::from(ct.version));
    o.h(Some(&ct.fname));
    for e in &ct.entries {
        let Some(e) = e else {
            o.s("-");
            continue;
        };
        o.h(Some(&e.name));
        for x in [e.ri, e.gi, e.bi, e.ai] {
            o.i(i64::from(x));
        }
        for x in [e.rf, e.gf, e.bf, e.af] {
            o.f(x);
        }
    }
}

fn dump_mri(cx: &mut Ctx, m: &Mri, values: bool, o: &mut Out) -> Result<(), Fatal> {
    for x in [
        m.width, m.height, m.depth, m.nframes, m.mtype, m.intent, m.version,
    ] {
        o.i(i64::from(x));
    }
    o.f(m.tr);
    dump_ctab(m.ct.as_ref(), o);
    if !values {
        return Ok(());
    }
    let mut sink = Vec::new();
    for f in 0..m.nframes {
        for s in 0..m.depth {
            for r in 0..m.height {
                for c in 0..m.width {
                    o.f(mri_get_vox_val(&mut cx.g, &mut sink, m, c, r, s, f, 0)?);
                }
            }
        }
    }
    Ok(())
}

fn opt(t: &mut Toks) -> Option<Vec<u8>> {
    (t.i() != 0).then(|| t.h())
}

pub(super) fn gwr_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let td = tempfile::tempdir().expect("a work folder");
    let dir = td.path();
    cx.work = Some(dir.to_string_lossy().into_owned());
    for d in ["d", "mri", "mri/transforms"] {
        std::fs::create_dir(dir.join(d)).expect("a folder");
    }
    cx.g.progname = "fssurfio".into();
    let gd = t.i();
    if gd & 1 != 0 {
        cx.g.gdiag |= DIAG_SHOW;
    }
    if gd & 2 != 0 {
        cx.g.gdiag |= DIAG_VERBOSE;
    }
    let date = utf8(t.h());
    let env = Environment::from_pairs(vec![
        (
            "FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING".to_owned(),
            date,
        ),
        ("SURFER_SIDEDOOR".to_owned(), "1".to_owned()),
        // a user name on every platform: FreeSurfer's run (getpwuid on Linux) writes a
        // `UserName`, which both sides print as `U`; where the port has no password database
        // (Windows, macOS) it takes `USER`, which Windows' runners do not set
        ("USER".to_owned(), "U".to_owned()),
    ]);
    let nin = t.i();
    for _ in 0..nin {
        let (n, b) = (utf8(t.h()), t.h());
        std::fs::write(dir.join(&n), b).expect("an input file");
    }
    let (nv, nf) = (t.i(), t.i());
    let mut m = {
        let (mut out, mut err) = (cx.a.clone(), cx.b.clone());
        alloc::alloc(&mut cx.g, &mut out, &mut err, &cx.sg, nv, nf)?
    };
    for vno in 0..nv.max(0) as usize {
        let v = m.vertex_mut(vno);
        (v.x, v.y, v.z, v.curv, v.stat) = (t.f(), t.f(), t.f(), t.f(), t.f());
        v.annotation = t.i();
    }
    for fno in 0..nf.max(0) as usize {
        for k in 0..3 {
            m.face_mut(fno).v[k] = t.i();
        }
    }
    let cut = |b: Vec<u8>| -> String {
        let b: Vec<u8> = b.into_iter().take_while(|&c| c != 0).take(1023).collect();
        utf8(b)
    };
    m.fname = cut(t.h());
    m.subject_name = cut(t.h());
    m.patch = t.i();
    m.useRealRAS = t.i();
    set_vg(t, &mut m.vg);
    (m.xctr, m.yctr, m.zctr) = (t.f(), t.f(), t.f());
    m.group_avg_surface_area = t.f();
    let nl = t.i();
    for i in 0..nl as usize {
        m.cmdlines[i] = Some(t.h());
    }
    m.ncmds = nl;
    if t.i() != 0 {
        let (rows, cols) = (t.i(), t.i());
        let (mut out, mut err) = (cx.a.clone(), cx.b.clone());
        let mut mt = matrix_alloc(&mut cx.g, &mut out, &mut err, rows, cols, MATRIX_REAL)?
            .expect("a matrix");
        for r in 1..=rows {
            for c in 1..=cols {
                *mt.relt_mut(r, c) = t.f();
            }
        }
        m.SRASToTalSRAS_ = Some(mt);
    }
    let nct = t.i();
    if nct >= 0 {
        let mut ct = ColorTable {
            fname: b"user.ctab".to_vec(),
            version: 2,
            ..ColorTable::default()
        };
        for _ in 0..nct {
            if t.i() == 0 {
                ct.entries.push(None);
                continue;
            }
            let name: Vec<u8> = t
                .h()
                .into_iter()
                .take_while(|&c| c != 0)
                .take(1023)
                .collect();
            let (ri, gi, bi, ai) = (t.i(), t.i(), t.i(), t.i());
            ct.entries.push(Some(Box::new(ColorTableEntry {
                name,
                ri,
                gi,
                bi,
                ai,
                rf: ri as f32 / 255.0,
                gf: gi as f32 / 255.0,
                bf: bi as f32 / 255.0,
                af: ai as f32 / 255.0,
                tissue_type: 0,
                count: 0,
            })));
        }
        m.ct = Some(ct);
    }
    let nvol = t.i();
    let mut vols: Vec<Mri> = Vec::new();
    for _ in 0..nvol {
        let (ty, w, h, d, fr) = (t.i(), t.i(), t.i(), t.i(), t.i());
        let (mut out, mut err) = (cx.a.clone(), cx.b.clone());
        let mut v = mri_alloc_sequence(&mut cx.g, &mut out, &mut err, w, h, d, ty, fr)?;
        v.tr = t.f();
        for f in 0..fr {
            for s in 0..d {
                for r in 0..h {
                    for c in 0..w {
                        let x = t.f();
                        mri_set_vox_val(&mut cx.g, &mut err, &mut v, c, r, s, f, x, 0)?;
                    }
                }
            }
        }
        vols.push(v);
    }
    let nops = t.i();
    let d = Some(dir);
    for _ in 0..nops {
        let op = t.next();
        o.s(op);
        let (mut out, mut err) = (cx.a.clone(), cx.b.clone());
        let mut io = coords::Io {
            out: &mut out,
            err: &mut err,
            env: &env,
        };
        let g = &mut cx.g;
        let name_of = |b: Vec<u8>| -> String { utf8(b) };
        let lossy = |b: Option<Vec<u8>>| b.map(|b| String::from_utf8_lossy(&b).into_owned());
        match op {
            "w" => {
                let intent = t.i();
                let n = name_of(t.h());
                let cf = lossy(opt(t));
                let r = gifti_write::mris_write_gifti(
                    g,
                    &mut io,
                    d,
                    &mut m,
                    intent,
                    &n,
                    cf.as_deref(),
                )?;
                o.i(i64::from(r));
                dump_gii_file(dir, &n, o);
            }
            "wm" => {
                let (intent, vol) = (t.i(), t.i());
                let n = name_of(t.h());
                let cf = opt(t);
                let dt = opt(t);
                let r = gifti_write::mris_write_gifti_mri(
                    g,
                    &mut io,
                    d,
                    &mut m,
                    &vols[vol as usize],
                    intent,
                    &n,
                    cf.as_deref(),
                    dt.as_deref(),
                )?;
                o.i(i64::from(r));
                dump_gii_file(dir, &n, o);
            }
            "c" => {
                let n = name_of(t.h());
                let nov = t.i();
                let mut ov = Vec::new();
                for _ in 0..nov {
                    let (ty, gi, vol) = (t.i(), t.i(), t.i());
                    // the overlay's own copy of the volume (MRIcopy: the same voxels)
                    let copy = if vol >= 0 {
                        let (mut so, mut se) = (Vec::new(), Vec::new());
                        mri_copy(g, &mut so, &mut se, &vols[vol as usize], None)?.into_new()
                    } else {
                        None
                    };
                    let foverlay = opt(t);
                    let sd: Vec<u8> = t
                        .h()
                        .into_iter()
                        .take_while(|&c| c != 0)
                        .take(255)
                        .collect();
                    ov.push(OverlayInfo {
                        foverlay,
                        otype: ty,
                        gifti_intent: gi,
                        shapedatatype: sd,
                        format: larmorx_freesurfer_surf::gifti::GIFTI_FILE,
                        overlaymri: copy,
                        n_vals_per_vertex: 1,
                    });
                }
                let r = gifti_write::mris_write_gifti_combined(g, &mut io, d, &mut m, &ov, &n)?;
                o.i(i64::from(r));
                dump_gii_file(dir, &n, o);
            }
            "mw" => {
                let vol = t.i();
                let n = name_of(t.h());
                let r = gifti_vol::mri_write_gifti(g, &mut io, d, &vols[vol as usize], &n)?;
                o.i(i64::from(r));
                dump_gii_file(dir, &n, o);
            }
            "sw" => {
                let n = name_of(t.h());
                let r = readwrite::write(g, &mut io, d, &mut m, &n)?;
                o.i(i64::from(r));
                dump_gii_file(dir, &n, o);
            }
            "n" => {
                let n = name_of(t.h());
                let (mut a, mut b) = (7, 7);
                let r =
                    gifti_write::get_shape_stat_intent_count(g, &mut io, d, &n, &mut a, &mut b)?;
                o.i(i64::from(r));
                o.i(i64::from(a));
                o.i(i64::from(b));
            }
            "r" => {
                let n = name_of(t.h());
                let rv = t.i() != 0;
                let r = gifti_vol::mris_read_gifti_as_mri(g, &mut io, d, &n, rv)?;
                match r {
                    None => o.s("NULL"),
                    Some(v) => dump_mri(cx, &v, rv, o)?,
                }
            }
            _ => panic!("gwr op {op}"),
        }
    }
    Ok(())
}
