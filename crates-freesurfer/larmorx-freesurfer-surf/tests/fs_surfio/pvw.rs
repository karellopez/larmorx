// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of relay 36 of track D (queue items 3.1-47/48): `pvw`, the patch, value and
//! normal files: `MRISwritePatch` (the binary patch, ASCII, movie.byu, STL), `MRISwriteIntoVolume`
//! (every field, a volume given or not), `MRISwriteWhiteNormals`, `MRISwriteNormals`,
//! `MRISwritePrincipalDirection`, `MRISreadNormals`, `MRISreadValues` and
//! `MRISreadValuesIntoArray` (volumes, curvature files, w files, generated and written), in an empty
//! work folder with `d`; each status and volume; then the surface's vectors and values and every
//! file in the folder.

use super::cfile::curv_file;
use super::crv::{mgh_curv, nii_curv};
use super::sfile::{list_files, utf8};
use super::*;

use larmorx_freesurfer_mri::mri::{Mri, mri_alloc_sequence};
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;
use larmorx_freesurfer_surf::valfile::{self, IntoVolume};
use larmorx_freesurfer_surf::{alloc, sfile as surf_sfile};

/// The last parts of the names the cases write and read.
const LEAVES: [&str; 14] = [
    "patch", "p.asc", "p.geo", "p.stl", "n.mgz", "n.mgh", "n.nii", "n.nii.gz", "v.w", "v", "c",
    "t.mgz", "x.nii", "y.wav",
];

/// A name an operation takes: a last part alone, in `d`, or an odd one (empty, a folder that does
/// not exist, a dot early, about 1,024 bytes when the seed allows). With seed 1 a write takes no
/// name Windows writes otherwise (a last dot, a `*`).
fn name(g: &mut Gen, write: bool) -> String {
    let n = name_drawn(g);
    if write && !g.1 && (n.is_empty() || n.ends_with('.') || n.contains('*')) {
        return "l.patch".to_owned();
    }
    n
}

fn name_drawn(g: &mut Gen) -> String {
    let leaf = g.pick(&LEAVES);
    match g.below(16) {
        0..=6 => leaf.to_owned(),
        7..=10 => format!("d/{leaf}"),
        11 => g
            .pick(&[
                "",
                ".w",
                "x.",
                "nodir/v.w",
                "nodir/n.mgz",
                "d",
                "v.wx",
                "w",
                "x.gz",
                "*.w",
            ])
            .to_owned(),
        12 if g.1 && g.chance(3) => {
            let n = g.pick(&[1018, 1019, 1020, 1021, 1022, 1023, 1024, 1030]);
            let mut s = String::from("d/");
            while s.len() < n {
                s.push(g.int(0x61, 0x7a) as u8 as char);
            }
            if g.chance(2) {
                s.truncate(n - 2);
                s.push_str(".w");
            }
            s
        }
        _ => format!("lh.{leaf}"),
    }
}

/// A w file whose vertex numbers fall outside the surface now and then (FS-621), its count at
/// times beyond its entries, now and then cut.
fn w_values(g: &mut Gen, nv: i64) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&(g.int(-50, 50) as i16).to_be_bytes());
    let num = g.int(0, 7);
    b.extend_from_slice(&(num as u32).to_be_bytes()[1..]);
    let present = if g.chance(6) { g.int(0, num) } else { num };
    for _ in 0..present {
        let k = if g.chance(6) {
            g.pick(&[0xff_ffffu32, 1000, nv as u32, (nv + 1) as u32])
        } else {
            g.int(0, (nv - 1).max(0)) as u32
        };
        b.extend_from_slice(&k.to_be_bytes()[1..]);
        let heavy = g.chance(4);
        b.extend_from_slice(&smallish(g, heavy).to_be_bytes());
    }
    if g.chance(10) && !b.is_empty() {
        let cut = g.int(0, b.len() as i64 - 1) as usize;
        b.truncate(cut);
    }
    b
}

/// An input file for a last part: volumes of 1 to 3 frames (`mgh_curv`, `nii_curv`), curvature
/// files, w files, now and then random bytes (not for a volume name).
fn input_file(g: &mut Gen, leaf: &str, nv: i64) -> Vec<u8> {
    let volume = [".mgz", ".mgh", ".nii", ".nii.gz"]
        .iter()
        .any(|e| leaf.ends_with(e));
    if !volume && g.chance(12) {
        let n = g.int(0, 40) as usize;
        return (0..n).map(|_| g.below(256) as u8).collect();
    }
    if leaf.ends_with(".mgz") {
        mgh_curv(g, nv, true)
    } else if leaf.ends_with(".mgh") {
        mgh_curv(g, nv, false)
    } else if leaf.ends_with(".nii.gz") {
        nii_curv(g, nv, true)
    } else if leaf.ends_with(".nii") {
        nii_curv(g, nv, false)
    } else if leaf.ends_with(".w") || leaf.ends_with(".wav") || g.chance(2) {
        w_values(g, nv)
    } else {
        curv_file(g, nv)
    }
}

fn input_name(g: &mut Gen) -> (String, &'static str) {
    let leaf = g.pick(&LEAVES);
    let n = match g.below(8) {
        0..=3 => leaf.to_owned(),
        4 | 5 => format!("d/{leaf}"),
        _ => format!("lh.{leaf}"),
    };
    (n, leaf)
}

pub(super) fn pvw_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(3);
    ix(t, if g.chance(4) { g.int(1, 3) } else { 0 });
    let mut envs = 0;
    if g.chance(6) {
        envs |= 1;
    }
    if g.chance(8) {
        envs |= 4;
    }
    if g.chance(4) {
        envs |= 8;
    }
    ix(t, envs);
    if envs & 4 != 0 {
        hx(t, g.pick(&["0", "1", "2", "-1", "x", "3"]).as_bytes());
    }
    ix(t, if g.chance(8) { g.int(-2, 2) } else { -1 });
    let nv = if g.chance(12) { 0 } else { g.int(1, 6) };
    let nf = if nv == 0 { 0 } else { g.int(0, 3) };
    let nin = g.int(0, 3);
    ix(t, nin);
    let mut known: Vec<String> = Vec::new();
    for _ in 0..nin {
        let (n, leaf) = input_name(g);
        let b = input_file(g, leaf, nv);
        hx(t, n.as_bytes());
        hx(t, &b);
        known.push(n);
    }
    ix(t, nv);
    ix(t, nf);
    hx(
        t,
        g.pick(&["d/lh.white", "lh.white", "d/rh.pial", ""])
            .as_bytes(),
    );
    for _ in 0..nv {
        for k in 0..22 {
            let x = if heavy && g.chance(10) {
                g.pick(&SPECIAL32)
            } else if k == 6 {
                // the annotation: an int's bits
                g.pick(&[0u32, 1, 0x00ff_00ff, 0xffff_ffff, 12345])
            } else {
                (g.int(-800, 800) as f32 / 8.0).to_bits()
            };
            t.push(format!("{x:08x}"));
        }
        ix(t, i64::from(g.chance(4)));
        ix(t, i64::from(g.chance(4)));
    }
    for _ in 0..nf {
        for _ in 0..3 {
            ix(t, g.int(0, nv - 1));
        }
        ix(t, i64::from(g.chance(5)));
    }
    let nops = g.int(1, 4);
    ix(t, nops);
    for _ in 0..nops {
        let op = g.pick(&["p", "p", "i", "i", "n", "N", "e", "R", "v", "v", "a", "a"]);
        t.push(op.to_owned());
        let read = matches!(op, "R" | "v" | "a");
        let pick_known = |g: &mut Gen, known: &[String]| -> String {
            if read && !known.is_empty() && !g.chance(3) {
                known[g.below(known.len() as u64) as usize].clone()
            } else {
                name(g, !read)
            }
        };
        match op {
            "i" => {
                ix(t, g.pick(&[1, 2, 3, 5, 6, 7, 8, 10, 1, 3, 0, 4, 9, 11, -1]));
                if g.chance(2) {
                    ix(t, -1);
                } else {
                    ix(t, g.pick(&[3, 3, 0, 1, 4]));
                    let (w, h, d, f) = match g.below(5) {
                        0 => (1, 1, 1, nv),
                        1 => (nv + 1, 1, 1, 1),
                        2 => (1, 2, 1, 1),
                        _ => (nv, 1, 1, g.int(1, 2)),
                    };
                    for e in [w, h, d, f] {
                        ix(t, e.max(1));
                    }
                }
            }
            "e" => {
                ix(t, g.pick(&[1, 2, 0]));
                let n = pick_known(g, &known);
                hx(t, n.as_bytes());
                known.push(n);
            }
            "a" => {
                if g.chance(12) {
                    ix(t, 0);
                } else {
                    ix(t, 1);
                    let n = pick_known(g, &known);
                    hx(t, n.as_bytes());
                }
                ix(t, if g.chance(5) { g.int(-1, 8) } else { nv });
            }
            _ => {
                let n = pick_known(g, &known);
                hx(t, n.as_bytes());
                if !read {
                    known.push(n);
                }
            }
        }
    }
}

fn dump_volume(g: &mut Globals, m: &Mri, o: &mut Out) -> Result<(), Fatal> {
    for e in [m.width, m.height, m.depth, m.nframes, m.mtype] {
        o.i(i64::from(e));
    }
    let mut sink = Vec::new();
    for f in 0..m.nframes {
        for s in 0..m.depth {
            for r in 0..m.height {
                for c in 0..m.width {
                    o.f(mri_get_vox_val(g, &mut sink, m, c, r, s, f, 0)?);
                }
            }
        }
    }
    Ok(())
}

pub(super) fn pvw_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let td = tempfile::tempdir().expect("a work folder");
    let dir = td.path();
    cx.work = Some(dir.to_string_lossy().into_owned());
    std::fs::create_dir(dir.join("d")).expect("a folder");
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
    if envs & 4 != 0 {
        let v = t.h();
        let v = utf8(v[..v.iter().position(|&c| c == 0).unwrap_or(v.len())].to_vec());
        pairs.push(("MRIS_READVAL_FRAME".into(), v));
    }
    if envs & 8 != 0 {
        pairs.push(("FS_LEAVE_NANS".into(), "1".into()));
    }
    pairs.push(("SURFER_SIDEDOOR".into(), "1".into()));
    pairs.push(("USER".into(), "U".into()));
    let env = Environment::from_pairs(pairs);
    let rf = t.i();
    if rf != -1 {
        surf_sfile::set_read_frame(&mut cx.sg, rf);
    }
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
    m.fname = utf8(t.h());
    for vno in 0..nv.max(0) as usize {
        let v = m.vertex_mut(vno);
        (v.x, v.y, v.z, v.val, v.curv, v.area) = (t.f(), t.f(), t.f(), t.f(), t.f(), t.f());
        v.annotation = t.f().to_bits() as i32;
        (v.dx, v.dy, v.dz, v.nx, v.ny, v.nz) = (t.f(), t.f(), t.f(), t.f(), t.f(), t.f());
        (v.wnx, v.wny, v.wnz) = (t.f(), t.f(), t.f());
        (v.e1x, v.e1y, v.e1z, v.e2x, v.e2y, v.e2z) = (t.f(), t.f(), t.f(), t.f(), t.f(), t.f());
        v.ripflag = t.i() as i8;
        v.border = t.i() as i8;
    }
    for fno in 0..nf.max(0) as usize {
        for k in 0..3 {
            m.face_mut(fno).v[k] = t.i();
        }
        m.face_mut(fno).ripflag = t.i() as _;
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
        match op {
            "p" => {
                let n = utf8(t.h());
                o.i(i64::from(valfile::write_patch(g, &mut io, d, &mut m, &n)));
            }
            "i" => {
                let which = t.i();
                let ty = t.i();
                let mut given = if ty < 0 {
                    None
                } else {
                    let (w, h, dd, f) = (t.i(), t.i(), t.i(), t.i());
                    Some(mri_alloc_sequence(g, io.out, io.err, w, h, dd, ty, f)?)
                };
                let r = valfile::write_into_volume(g, &mut io, &m, given.as_mut(), which)?;
                match r {
                    IntoVolume::Null => {
                        o.s("NULL");
                        if let Some(v) = &given {
                            dump_volume(g, v, o)?;
                        }
                    }
                    IntoVolume::Given => {
                        o.s("GIVEN");
                        dump_volume(g, given.as_ref().expect("a volume given"), o)?;
                    }
                    IntoVolume::New(v) => {
                        o.s("NEW");
                        dump_volume(g, &v, o)?;
                    }
                }
            }
            "n" => {
                let n = utf8(t.h());
                o.i(i64::from(valfile::write_white_normals(
                    g, &mut io, d, &m, &n,
                )?));
            }
            "N" => {
                let n = utf8(t.h());
                o.i(i64::from(valfile::write_normals(g, &mut io, d, &m, &n)?));
            }
            "e" => {
                let k = t.i();
                let n = utf8(t.h());
                let r = valfile::write_principal_direction(g, &mut io, d, &m, k, &n)?;
                o.i(i64::from(r));
            }
            "R" => {
                let n = utf8(t.h());
                o.i(i64::from(valfile::read_normals(g, &mut io, d, &mut m, &n)?));
            }
            "v" => {
                let n = utf8(t.h());
                let r = valfile::read_values(g, &cx.sg, &mut io, d, &mut m, &n)?;
                o.i(i64::from(r));
            }
            "a" => {
                let n = (t.i() != 0).then(|| utf8(t.h()));
                let size = t.i();
                let (r, arr) =
                    valfile::read_values_into_array(g, &cx.sg, &mut io, d, n.as_deref(), size)?;
                o.i(i64::from(r));
                match arr {
                    Some(a) if r == 0 => {
                        for x in a {
                            o.f(x);
                        }
                    }
                    _ => o.s("NOARRAY"),
                }
            }
            _ => panic!("pvw op {op}"),
        }
    }
    o.0 += "| ";
    for vno in 0..nv.max(0) as usize {
        let v = m.vertex(vno);
        for x in [v.x, v.y, v.z, v.nx, v.ny, v.nz, v.val] {
            o.f(x);
        }
    }
    o.0 += "| ";
    let mut files = Vec::new();
    list_files(dir, "", &mut files);
    files.sort();
    for n in files {
        o.h(Some(n.as_bytes()));
        match std::fs::read(dir.join(&n)) {
            Ok(b) => {
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
