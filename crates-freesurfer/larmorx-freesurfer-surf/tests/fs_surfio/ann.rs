// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of relay 36 of track D (queue items 3.1-47/48): `ann`, the annotation
//! files: `MRISwriteAnnotation` (an annotation file with or without its colour table, an MGH or
//! NIfTI segmentation, GIFTI with either intent) and `MRISreadAnnotation` (those files read back,
//! generated annotation files, cut or with an odd count, vertex or tag, MGH segmentations without a
//! colour table, a caller's colour table or none), in a work folder with `s/surf` and `s/label`;
//! each status; then the vertices' annotations, the surface's colour table and every file.

use super::crv::mgh_curv;
use super::gwr::gii_normal;
use super::sfile::{list_files, utf8};
use super::*;

use larmorx_core::defects::FS_625;
use larmorx_freesurfer_mri::ctab::{ColorTable, ColorTableEntry};
use larmorx_freesurfer_surf::{alloc, annot};

/// The annotation names the cases write and read (in `s/label`, after the hemisphere).
const LEAVES: [&str; 8] = [
    "aparc.annot",
    "a2009s.annot",
    "aparc",
    "seg.mgz",
    "seg.nii",
    "lab.gii",
    "x.mgh",
    "lh.aparc.annot",
];

fn be32(b: &mut Vec<u8>, v: i64) {
    b.extend_from_slice(&(v as i32).to_be_bytes());
}

/// An annotation file of `nv` vertices: the count (now and then another), the entries (now and
/// then a vertex out of range), now and then a tag with a short random tail or a cut file.
fn annot_file(g: &mut Gen, nv: i64, colours: &[(i64, i64, i64)]) -> Vec<u8> {
    let mut b = Vec::new();
    let n = if g.chance(8) { g.int(0, nv + 2) } else { nv };
    be32(&mut b, n);
    for k in 0..n {
        let vno = if g.chance(12) {
            g.pick(&[-1, nv, 1000])
        } else if g.chance(3) {
            g.int(0, (nv - 1).max(0))
        } else {
            k
        };
        be32(&mut b, vno);
        let (r, gg, bb) = g.pick(colours);
        be32(
            &mut b,
            if g.chance(6) {
                g.int(-1, 0xff_ffff)
            } else {
                r | (gg << 8) | (bb << 16)
            },
        );
    }
    match g.below(6) {
        0 => {
            be32(&mut b, g.pick(&[1, 2, 0]));
            let tail = g.int(0, 12) as usize;
            for _ in 0..tail {
                b.push(g.below(256) as u8);
            }
        }
        1 => {
            be32(&mut b, 1);
            be32(&mut b, g.pick(&[-3, -2, 0, 5]));
        }
        2 if !b.is_empty() => {
            let cut = g.int(0, b.len() as i64 - 1) as usize;
            b.truncate(cut);
        }
        _ => {}
    }
    b
}

fn ctab_tokens(t: &mut Vec<String>, g: &mut Gen, colours: &[(i64, i64, i64)]) {
    let nct = if g.chance(4) { -1 } else { g.int(0, 6) };
    ix(t, nct);
    for k in 0..nct.max(0) {
        if g.chance(8) {
            ix(t, 0);
            continue;
        }
        ix(t, 1);
        hx(t, format!("label{k}").as_bytes());
        let (r, gg, b) = colours[k as usize % colours.len()];
        for v in [r, gg, b, g.pick(&[0, 0, 255])] {
            ix(t, v);
        }
    }
}

pub(super) fn ann_tokens(t: &mut Vec<String>, g: &mut Gen) {
    ix(t, if g.chance(4) { g.int(1, 3) } else { 0 });
    let mut envs = 0;
    if g.chance(8) {
        envs |= 1;
    }
    if g.chance(8) {
        envs |= 2;
    }
    ix(t, envs);
    let nv = if g.chance(12) { 0 } else { g.int(1, 8) };
    let colours: Vec<(i64, i64, i64)> = (0..6)
        .map(|_| (g.int(0, 255), g.int(0, 255), g.int(0, 255)))
        .collect();
    let nin = g.int(0, 3);
    ix(t, nin);
    let mut known: Vec<String> = Vec::new();
    for _ in 0..nin {
        let leaf = g.pick(&LEAVES);
        let n = format!("s/label/lh.{leaf}");
        let b = if leaf.ends_with(".mgz") || leaf.ends_with(".mgh") {
            mgh_curv(g, nv, leaf.ends_with(".mgz"))
        } else if g.chance(10) {
            let n = g.int(0, 30) as usize;
            (0..n).map(|_| g.below(256) as u8).collect()
        } else {
            annot_file(g, nv, &colours)
        };
        hx(t, n.as_bytes());
        hx(t, &b);
        known.push(leaf.to_owned());
    }
    ix(t, nv);
    ix(t, g.pick(&[0, 0, 0, 1]));
    hx(
        t,
        g.pick(&[
            "s/surf/lh.white",
            "s/surf/rh.white",
            "s/surf/lh.white",
            "white",
        ])
        .as_bytes(),
    );
    for _ in 0..nv {
        let a = if g.chance(5) {
            g.int(-1, 0xff_ffff)
        } else {
            let (r, gg, b) = g.pick(&colours);
            r | (gg << 8) | (b << 16)
        };
        ix(t, a);
        ix(t, i64::from(g.chance(6)));
    }
    // the surface's colour table and the caller's
    ctab_tokens(t, g, &colours);
    ctab_tokens(t, g, &colours);
    let nops = g.int(1, 4);
    ix(t, nops);
    for _ in 0..nops {
        let op = if g.chance(2) { "w" } else { "r" };
        t.push(op.to_owned());
        let n = if op == "r" && !known.is_empty() && !g.chance(4) {
            known[g.below(known.len() as u64) as usize].clone()
        } else if g.chance(10) {
            g.pick(&["", "s/label/none.annot", "nodir/x.annot", "x.wav", "lh"])
                .to_owned()
        } else {
            g.pick(&LEAVES).to_owned()
        };
        if op == "w" {
            known.push(n.clone());
        }
        hx(t, n.as_bytes());
        if op == "w" {
            ix(t, i64::from(g.chance(2)));
        } else {
            ix(t, g.pick(&[-1, -1, 0, 1]));
            ix(t, i64::from(g.chance(3)));
        }
    }
}

fn read_ctab(t: &mut Toks) -> Option<ColorTable> {
    let nct = t.i();
    if nct < 0 {
        return None;
    }
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
        let name: Vec<u8> = t.h().into_iter().take_while(|&c| c != 0).collect();
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
    Some(ct)
}

fn dump_ctab(ct: Option<&ColorTable>, o: &mut Out) {
    let Some(ct) = ct else {
        o.s("NULL");
        return;
    };
    o.i(i64::from(ct.nentries()));
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
    }
}

pub(super) fn ann_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let td = tempfile::tempdir().expect("a work folder");
    let dir = td.path();
    cx.work = Some(dir.to_string_lossy().into_owned());
    for d in ["s", "s/surf", "s/label"] {
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
    let envs = t.i();
    let mut pairs: Vec<(String, String)> = Vec::new();
    if envs & 1 != 0 {
        pairs.push(("FS_POSIX".into(), "1".into()));
    }
    if envs & 2 != 0 {
        pairs.push(("FS_GII".into(), ".gii".into()));
    }
    pairs.push((
        "FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING".into(),
        "probe date".into(),
    ));
    pairs.push(("SURFER_SIDEDOOR".into(), "1".into()));
    pairs.push(("USER".into(), "U".into()));
    let env = Environment::from_pairs(pairs);
    let nin = t.i();
    for _ in 0..nin {
        let (n, b) = (utf8(t.h()), t.h());
        std::fs::write(dir.join(&n), b).expect("an input file");
    }
    let nv = t.i();
    let mut m = {
        let (mut out, mut err) = (cx.a.clone(), cx.b.clone());
        alloc::alloc(&mut cx.g, &mut out, &mut err, &cx.sg, nv, 0)?
    };
    m.hemisphere = t.i();
    m.fname = utf8(t.h());
    for vno in 0..nv.max(0) as usize {
        let v = m.vertex_mut(vno);
        v.annotation = t.i();
        v.ripflag = t.i() as i8;
    }
    m.ct = read_ctab(t);
    let user = read_ctab(t);
    let nops = t.i();
    let d = Some(dir);
    // FS-625: FreeSurfer frees the caller's colour table in a segmentation read given it; a later
    // use of that table (passed again, or the surface's table taken from it) reads freed memory
    let (mut freed, mut ct_is_user) = (false, false);
    for _ in 0..nops {
        let op = t.next();
        o.s(op);
        let (mut out, mut err) = (cx.a.clone(), cx.b.clone());
        let mut io = coords::Io {
            out: &mut out,
            err: &mut err,
            env: &env,
        };
        let (g, sg) = (&mut cx.g, &mut cx.sg);
        let n = utf8(t.h());
        let r = match op {
            "w" => {
                let writect = t.i() != 0;
                annot::write_annotation(g, sg, &mut io, d, &mut m, &n, writect)?
            }
            "r" => {
                let danum = t.i();
                let ct = (t.i() != 0).then_some(user.as_ref()).flatten();
                if ct.is_some() && freed {
                    cx.extra.push("FS-625u");
                }
                let r = annot::read_annotation(g, sg, &mut io, d, &mut m, &n, danum, ct)?;
                let seg = [".mgz", ".mgh", ".nii", ".nii.gz"]
                    .iter()
                    .any(|e| n.ends_with(e));
                if ct.is_some() {
                    freed |= g.notices.contains(&FS_625);
                    // the surface's table is the caller's itself after a read of another kind
                    ct_is_user = r == 0 && !seg;
                } else if r == 0 {
                    ct_is_user = false;
                }
                r
            }
            _ => panic!("ann op {op}"),
        };
        o.i(i64::from(r));
    }
    if freed && ct_is_user {
        cx.extra.push("FS-625u");
    }
    o.0 += "| ";
    for vno in 0..nv.max(0) as usize {
        o.i(i64::from(m.vertex(vno).annotation));
    }
    dump_ctab(m.ct.as_ref(), o);
    o.0 += "| ";
    let mut files = Vec::new();
    list_files(dir, "", &mut files);
    files.sort();
    for n in files {
        o.h(Some(n.as_bytes()));
        match std::fs::read(dir.join(&n)) {
            Ok(b) => {
                let b = if n.ends_with(".gii") {
                    gii_normal(&b)
                } else {
                    b
                };
                let mut s = String::from("x");
                for c in b {
                    s += &format!("{c:02x}");
                }
                o.s(&s);
            }
            Err(_) => o.s("NOFILE"),
        }
    }
    Ok(())
}
