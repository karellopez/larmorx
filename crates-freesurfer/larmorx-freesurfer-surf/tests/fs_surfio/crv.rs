// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of relay 35 of track D (queue items 3.1-47/48): `crv`, the curvature files
//! by name: `MRISwriteCurvature` (every format a name chooses: MGH, MGZ, NIfTI, VTK, text, GIFTI,
//! the new binary format) and `MRISreadCurvatureFile` (those, the old binary format, generated
//! files of each and the files the case's writes made), in an empty work folder with `d` and
//! `d/s`; each status, the volume of values and the overlays; then the surface's values and every
//! file in the folder (a `.gii` one as `gii_normal` prints it).

use super::cfile::{ascii_curv, case_mri_split, curv_file, dump_mri_g, vtk};
use super::gii::gii_file;
use super::gwr::gii_normal;
use super::sfile::{list_files, utf8};
use super::*;

use flate2::Compression;
use flate2::write::GzEncoder;
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;
use larmorx_freesurfer_surf::{alloc, curvfile, sfile as surf_sfile};

/// The last parts of the names the cases write and read: no extension (the new or old binary
/// format), MGH, MGZ, NIfTI, text, VTK, GIFTI.
const LEAVES: [&str; 14] = [
    "curv",
    "sulc",
    "thickness",
    "c.mgz",
    "c.mgh",
    "x.nii",
    "x.nii.gz",
    "a.asc",
    "v.vtk",
    "g.gii",
    "w",
    "t.mgz",
    "y.nii",
    "o.curv",
];

/// A name an operation takes: a last part alone, after a hemisphere, with a folder, or an odd one
/// (empty, a dot early, a folder that does not exist, a wildcard, about 1,024 bytes when the seed
/// allows). With seed 1 (the set every platform runs) a write takes no name whose file Windows
/// names otherwise (a last dot it drops: an empty name or `lh.` gives `<folder>/lh.`; a `*` it
/// refuses): another odd name in its place, the same draws.
fn name(g: &mut Gen, write: bool) -> String {
    let n = name_drawn(g);
    if write && !g.1 && (n.is_empty() || n.ends_with('.') || n.contains('*')) {
        return "l.curv".to_owned();
    }
    n
}

fn name_drawn(g: &mut Gen) -> String {
    let leaf = g.pick(&LEAVES);
    match g.below(20) {
        0..=7 => leaf.to_owned(),
        8..=11 => format!("{}.{leaf}", g.pick(&["lh", "rh"])),
        12..=15 => format!(
            "{}{leaf}",
            g.pick(&["d/lh.", "d/rh.", "d/s/", "./lh.", "d/"])
        ),
        16 => g
            .pick(&[
                "",
                ".curv",
                "l.curv",
                "lh",
                "lh.",
                "nodir/lh.curv",
                "nodir/c.mgz",
                "*.mgz",
                "d/lh.c*.mgz",
                "d/*.nii",
                "both.curv",
                "x.gz",
                ".gii",
                "lh.g",
                "h.curv",
            ])
            .to_owned(),
        17 if g.1 && g.chance(4) => {
            let n = g.pick(&[1010, 1017, 1018, 1019, 1020, 1023, 1030]);
            let mut s = String::from("d/");
            while s.len() < n {
                s.push(g.int(0x61, 0x7a) as u8 as char);
            }
            s
        }
        _ => format!("lh.{leaf}"),
    }
}

/// The file a read of `leaf` finds most often: in `d` after a hemisphere, in `d/s`, or at the top.
fn input_name(g: &mut Gen) -> (String, &'static str) {
    let leaf = g.pick(&LEAVES);
    let n = match g.below(10) {
        0..=4 => format!("d/lh.{leaf}"),
        5 | 6 => format!("d/rh.{leaf}"),
        7 => format!("d/s/{leaf}"),
        8 => format!("lh.{leaf}"),
        _ => format!("d/{leaf}"),
    };
    (n, leaf)
}

fn be16(b: &mut Vec<u8>, v: i16) {
    b.extend_from_slice(&v.to_be_bytes());
}
fn be32(b: &mut Vec<u8>, v: i32) {
    b.extend_from_slice(&v.to_be_bytes());
}
fn le16(b: &mut Vec<u8>, v: i16) {
    b.extend_from_slice(&v.to_le_bytes());
}
fn le32(b: &mut Vec<u8>, v: i32) {
    b.extend_from_slice(&v.to_le_bytes());
}
fn lef(b: &mut Vec<u8>, v: f32) {
    b.extend_from_slice(&v.to_bits().to_le_bytes());
}

fn gz(b: Vec<u8>) -> Vec<u8> {
    let mut e = GzEncoder::new(Vec::new(), Compression::default());
    e.write_all(&b).expect("gzip in memory");
    e.finish().expect("gzip in memory")
}

/// The extent of a volume of `nv` values: mostly `nv` x 1 x 1, now and then along another axis,
/// split, or one voxel more or fewer.
fn extent(g: &mut Gen, nv: i64) -> (i32, i32, i32) {
    let nv = nv.max(1) as i32;
    match g.below(12) {
        0 => (1, nv, 1),
        1 => (1, 1, nv),
        2 if nv % 2 == 0 => (2, nv / 2, 1),
        3 => (nv + 1, 1, 1),
        4 if nv > 1 => (nv - 1, 1, 1),
        _ => (nv, 1, 1),
    }
}

/// A voxel value of type `ty` (the MGH codes: 0 bytes, 1 int, 3 float, 4 short), its bits as a
/// big-endian `int`: floats with NaNs and infinities now and then.
fn voxel(g: &mut Gen, ty: i32, heavy: bool) -> i32 {
    match ty {
        0 => g.int(0, 255) as i32,
        1 => g.pick(&[0, 1, -7, 300, i32::MAX, i32::MIN, 12345]),
        4 => g.int(-32768, 32767) as i32,
        _ => smallish(g, heavy) as i32,
    }
}

/// An MGH volume of one voxel per vertex (mostly): version 1, the extent, frames, type, the RAS
/// flag and geometry, the voxels; now and then cut.
pub(super) fn mgh_curv(g: &mut Gen, nv: i64, compressed: bool) -> Vec<u8> {
    let heavy = g.chance(3);
    let mut b = Vec::new();
    be32(&mut b, 1);
    let (w, h, d) = extent(g, nv);
    let nf = g.pick(&[1, 1, 1, 2, 3]);
    let ty = g.pick(&[3, 3, 3, 0, 1, 4]);
    for v in [w, h, d, nf, ty, 0] {
        be32(&mut b, v);
    }
    let flag: i16 = g.pick(&[1, 0]);
    be16(&mut b, flag);
    if flag != 0 {
        for v in [
            1.0f32, 1.0, 1.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0,
        ] {
            b.extend_from_slice(&v.to_bits().to_be_bytes());
        }
    }
    b.resize(284, 0);
    for _ in 0..(w * h * d * nf) {
        let v = voxel(g, ty, heavy);
        match ty {
            0 => b.push(v as u8),
            4 => be16(&mut b, v as i16),
            _ => be32(&mut b, v),
        }
    }
    if g.chance(15) {
        let cut = g.int(0, b.len() as i64) as usize;
        b.truncate(cut);
    }
    if compressed { gz(b) } else { b }
}

/// A NIfTI-1 single file of one voxel per vertex (mostly): the header (the extent, frames, the
/// data type: bytes, shorts, ints or floats, a scale now and then, the qform code), no extension,
/// the voxels in little-endian order; now and then cut.
pub(super) fn nii_curv(g: &mut Gen, nv: i64, compressed: bool) -> Vec<u8> {
    let heavy = g.chance(3);
    let (w, h, d) = extent(g, nv);
    let nf = g.pick(&[1, 1, 1, 2]);
    let (dt, bitpix, ty) = g.pick(&[(16, 32, 3), (16, 32, 3), (2, 8, 0), (4, 16, 4), (8, 32, 1)]);
    let mut b = Vec::new();
    le32(&mut b, 348);
    b.resize(40, 0);
    let dims = [if nf > 1 { 4 } else { 3 }, w, h, d, nf, 1, 1, 1];
    for v in dims {
        le16(&mut b, v as i16);
    }
    b.resize(70, 0);
    le16(&mut b, dt);
    le16(&mut b, bitpix);
    b.resize(76, 0);
    for v in [1.0f32, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0] {
        lef(&mut b, v);
    }
    lef(&mut b, 352.0);
    lef(&mut b, g.pick(&[0.0f32, 0.0, 1.0, 2.0]));
    lef(&mut b, g.pick(&[0.0f32, 0.0, 0.5]));
    b.resize(123, 0);
    b.push(10);
    b.resize(252, 0);
    le16(&mut b, g.pick(&[0, 1]));
    le16(&mut b, 0);
    b.resize(344, 0);
    b.extend_from_slice(b"n+1\0");
    b.extend_from_slice(&[0, 0, 0, 0]);
    for _ in 0..(w * h * d * nf) {
        let v = voxel(g, ty, heavy);
        match ty {
            0 => b.push(v as u8),
            4 => le16(&mut b, v as i16),
            _ => le32(&mut b, v),
        }
    }
    if g.chance(15) {
        let cut = g.int(0, b.len() as i64) as usize;
        b.truncate(cut);
    }
    if compressed { gz(b) } else { b }
}

/// An input file for a last part: by its extension, now and then random bytes (not for a volume:
/// a volume header of random bytes makes both sides allocate what the machine's memory decides,
/// as FS-354's counts do; the port's `MRIalloc` then builds every frame it is asked for, relay
/// 35's finding for fs.mri).
fn input_file(g: &mut Gen, leaf: &str, nv: i64, faces: &[[i64; 3]]) -> Vec<u8> {
    let volume = [".mgz", ".mgh", ".nii", ".nii.gz"]
        .iter()
        .any(|e| leaf.ends_with(e));
    if !volume && g.chance(12) {
        let n = g.int(0, 60) as usize;
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
    } else if leaf.ends_with(".asc") {
        ascii_curv(g, nv)
    } else if leaf.ends_with(".vtk") {
        vtk(g, nv, faces)
    } else if leaf.ends_with(".gii") {
        let heavy = g.chance(4);
        gii_file(g, heavy).0
    } else {
        curv_file(g, nv)
    }
}

pub(super) fn crv_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(3);
    ix(t, if g.chance(4) { g.int(1, 3) } else { 0 });
    let mut envs = 0;
    if g.chance(6) {
        envs |= 1;
    }
    if g.chance(6) {
        envs |= 2;
    }
    if g.chance(8) {
        envs |= 4;
    }
    if g.chance(4) {
        envs |= 8;
    }
    ix(t, envs);
    if envs & 2 != 0 {
        hx(t, g.pick(&[".gii", ".gii", "x", ""]).as_bytes());
    }
    if envs & 4 != 0 {
        hx(t, g.pick(&["0", "1", "2", "-1", "x", "3"]).as_bytes());
    }
    hx(
        t,
        g.pick(&["Sat Nov 15 16:00:00 2026", "", "probe date"])
            .as_bytes(),
    );
    ix(t, if g.chance(8) { g.int(-2, 2) } else { -1 });
    let nv = if g.chance(12) { 0 } else { g.int(1, 6) };
    let nf = if nv == 0 { 0 } else { g.int(0, 3) };
    let faces: Vec<[i64; 3]> = (0..nf)
        .map(|_| [g.int(0, nv - 1), g.int(0, nv - 1), g.int(0, nv - 1)])
        .collect();
    let nin = g.int(0, 3);
    ix(t, nin);
    // the names a read may take to find a file: the inputs' (whole, or their last part where the
    // surface's folder and hemisphere lead to it) and the written ones
    let mut known: Vec<String> = Vec::new();
    for _ in 0..nin {
        let (n, leaf) = input_name(g);
        let b = input_file(g, leaf, nv, &faces);
        hx(t, n.as_bytes());
        hx(t, &b);
        known.push(n.clone());
        if n.starts_with("d/lh.") {
            known.push(leaf.to_owned());
            known.push(format!("lh.{leaf}"));
        }
    }
    ix(t, nv);
    ix(t, nf);
    // mostly a left surface in `d` (the inputs' names lead there)
    if g.chance(2) {
        ix(t, 0);
        hx(t, b"d/lh.white");
    } else {
        ix(t, g.pick(&[0, 0, 0, 1, 1, 3, 2]));
        hx(
            t,
            g.pick(&[
                "d/lh.white",
                "d/rh.pial",
                "d/s/lh.inflated",
                "white",
                "",
                "nodir/lh.white",
            ])
            .as_bytes(),
        );
    }
    hx(t, g.pick(&["", "bert", "s01"]).as_bytes());
    for _ in 0..nv {
        for _ in 0..3 {
            let x = if heavy && g.chance(10) {
                g.pick(&SPECIAL32)
            } else {
                (g.int(-800, 800) as f32 / 8.0).to_bits()
            };
            t.push(format!("{x:08x}"));
        }
        t.push(format!("{:08x}", smallish(g, heavy)));
        t.push(format!("{:08x}", smallish(g, heavy)));
    }
    for f in &faces {
        for &v in f {
            ix(t, v);
        }
    }
    let nops = g.int(1, 4);
    ix(t, nops);
    let mut last: Option<String> = None;
    for _ in 0..nops {
        let reread = last.is_some() && g.chance(2);
        let op = if reread || g.chance(2) { "r" } else { "w" };
        t.push(op.to_owned());
        let n = match (&last, reread) {
            (Some(n), true) => n.clone(),
            _ if op == "r" && !known.is_empty() && !g.chance(3) => {
                known[g.below(known.len() as u64) as usize].clone()
            }
            _ => name(g, op == "w"),
        };
        if op == "w" {
            known.push(n.clone());
        }
        hx(t, n.as_bytes());
        if op == "w" {
            if g.chance(6) {
                ix(t, 1);
                hx(t, g.pick(&["lh.curv", "x.mgz", ""]).as_bytes());
            } else {
                ix(t, 0);
            }
            last = Some(n);
        } else {
            ix(
                t,
                if g.chance(3) {
                    g.pick(&[3, 3, 0, 4, 1])
                } else {
                    -1
                },
            );
            ix(t, i64::from(g.chance(4)));
            last = None;
        }
    }
}

pub(super) fn crv_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let td = tempfile::tempdir().expect("a work folder");
    let dir = td.path();
    cx.work = Some(dir.to_string_lossy().into_owned());
    for d in ["d", "d/s"] {
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
    let value = |t: &mut Toks| -> String {
        let v = t.h();
        utf8(v[..v.iter().position(|&c| c == 0).unwrap_or(v.len())].to_vec())
    };
    let mut pairs: Vec<(String, String)> = Vec::new();
    if envs & 1 != 0 {
        pairs.push(("FS_POSIX".into(), "1".into()));
    }
    if envs & 2 != 0 {
        pairs.push(("FS_GII".into(), value(t)));
    }
    if envs & 4 != 0 {
        pairs.push(("MRIS_READVAL_FRAME".into(), value(t)));
    }
    if envs & 8 != 0 {
        pairs.push(("FS_LEAVE_NANS".into(), "1".into()));
    }
    pairs.push((
        "FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING".into(),
        value(t),
    ));
    pairs.push(("SURFER_SIDEDOOR".into(), "1".into()));
    // a user name on every platform (`gwr`'s): both sides print a GIFTI file's `UserName` as `U`
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
    m.hemisphere = t.i();
    let cut = |b: Vec<u8>| -> String {
        let b: Vec<u8> = b.into_iter().take_while(|&c| c != 0).take(1023).collect();
        utf8(b)
    };
    m.fname = cut(t.h());
    m.subject_name = cut(t.h());
    for vno in 0..nv.max(0) as usize {
        let v = m.vertex_mut(vno);
        (v.x, v.y, v.z, v.curv, v.val) = (t.f(), t.f(), t.f(), t.f(), t.f());
    }
    for fno in 0..nf.max(0) as usize {
        for k in 0..3 {
            m.face_mut(fno).v[k] = t.i();
        }
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
            "w" => {
                let n = utf8(t.h());
                let cf = (t.i() != 0).then(|| String::from_utf8_lossy(&t.h()).into_owned());
                let r = curvfile::write_curvature(g, &mut io, d, &mut m, &n, cf.as_deref())?;
                o.i(i64::from(r));
            }
            "r" => {
                let n = utf8(t.h());
                let mut mri = case_mri_split(g, &mut io, t, nv)?;
                let ov = t.i() != 0;
                let mut ovs = Vec::new();
                let r = curvfile::read_curvature_file(
                    g,
                    &mut cx.sg,
                    &mut io,
                    d,
                    &mut m,
                    &n,
                    mri.as_mut(),
                    ov.then_some(&mut ovs),
                )?;
                o.i(i64::from(r));
                dump_mri_g(g, &mut io, o, mri)?;
                if ov {
                    o.i(ovs.len() as i64);
                    for v in &ovs {
                        o.h(v.foverlay.as_deref());
                        o.i(i64::from(v.otype));
                        o.i(i64::from(v.gifti_intent));
                        o.h(Some(&v.shapedatatype));
                        o.i(i64::from(v.format));
                        o.i(i64::from(v.n_vals_per_vertex));
                        match &v.overlaymri {
                            None => o.s("NULL"),
                            Some(x) => {
                                for e in [x.width, x.height, x.depth, x.nframes] {
                                    o.i(i64::from(e));
                                }
                                for c in 0..x.width {
                                    let mut sink = Vec::new();
                                    o.f(mri_get_vox_val(g, &mut sink, x, c, 0, 0, 0, 0)?);
                                }
                            }
                        }
                    }
                }
            }
            _ => panic!("crv op {op}"),
        }
    }
    o.0 += "| ";
    for vno in 0..nv.max(0) as usize {
        let v = m.vertex(vno);
        for x in [v.x, v.y, v.z, v.curv, v.val] {
            o.f(x);
        }
    }
    o.f(m.max_curv);
    o.f(m.min_curv);
    o.i(i64::from(m.nvertices));
    o.i(i64::from(m.nfaces));
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
