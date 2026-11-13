// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of queue item 3.1-47 part b (relay 11): `sfile`, the surface library's
//! small file readers and writers in an empty work folder (with a subfolder `d`): the case's input
//! files, a surface from `MRISoverAlloc` with its vertices, every face place and the face-normal
//! cache, then 1 to 3 file operations on names relative to the folder, then the surface's state
//! and every file in the folder (name and bytes, sorted): the generator and the port's answers, as
//! `fssurfio.cpp`'s.

use super::*;

use larmorx_freesurfer_surf::{alloc, sfile};

const OPS: [&str; 16] = [
    "sf", "gf", "rt", "wt", "wd", "wa", "wg", "wc", "rb", "ri", "ca", "mm", "wA", "wn", "wv", "wC",
];

/// A name of about `n` bytes whose parts stay under 256 bytes, in a folder that does not exist.
fn long_name(g: &mut Gen, n: usize) -> String {
    let mut s = String::from(if g.chance(2) { "nodir/" } else { "d/" });
    while s.len() + 205 < n {
        s.push_str(&"a".repeat(200));
        s.push('/');
    }
    let tail = if g.chance(2) { "lh.x" } else { "rh.y.w" };
    while s.len() + tail.len() < n {
        s.push('b');
    }
    s.push_str(tail);
    s
}

/// A file name for an operation: in the work folder, in `d`, in a folder that does not exist,
/// with and without a dot, with the dot first or second (FS-129); with seeds other than 1, now and
/// then one of about 1,024 bytes (FS-126, FS-134: kept out of seed 1's platform-independent set,
/// as file systems differ on such names).
fn name(g: &mut Gen, k: usize) -> String {
    if g.1 && g.chance(25) {
        let n = g.int(1015, 1032) as usize;
        return long_name(g, n);
    }
    match g.below(12) {
        0 => format!("d/lh.f{k}"),
        1 => format!("lh.f{k}"),
        2 => "x.white".to_owned(),
        3 => ".white".to_owned(),
        4 => format!("nodir/lh.f{k}"),
        5 => format!("d/f{k}"),
        6 => format!("f{k}"),
        7 => format!("d/rh.f{k}.w"),
        8 => "d/lh.white".to_owned(),
        9 => "rh.pial".to_owned(),
        10 => format!("d/x{k}.vtk"),
        _ => format!("rh.f{k}"),
    }
}

/// `(int) f` of a count read as a `float`, as x86-64 converts it.
fn float_count(bits: u32) -> i64 {
    let f = f64::from(f32::from_bits(bits));
    if f.is_nan() || f >= 2147483648.0 || f <= -2147483649.0 {
        i64::from(i32::MIN)
    } else {
        f.trunc() as i64
    }
}

/// A count FreeSurfer's reader (counts as floats) takes within the surface's face places.
fn face_count_bits(g: &mut Gen, nf: i64, maxf: i64) -> u32 {
    match g.below(6) {
        0 => (nf as f32).to_bits(),
        1 => ((nf - 1).max(0) as f32).to_bits(),
        2 => ((nf + 1).min(maxf) as f32).to_bits(),
        3 => (-1.0f32).to_bits(),
        4 => 0x7fc0_0000,
        _ => {
            let b = g.next() as u32;
            if float_count(b) > maxf {
                0xffc0_0000
            } else {
                b
            }
        }
    }
}

/// A triangle-property file: its counts as the writer writes them (`int`s), as floats (what
/// FreeSurfer's reader takes), or other bits; then the faces' values (one or three floats each),
/// cut now and then (FS-167: FreeSurfer reads memory then).
fn triangle_file(g: &mut Gen, nv: i64, nf: i64, maxf: i64, per: usize) -> Vec<u8> {
    let mut b = Vec::new();
    let entries = match g.below(4) {
        0 => {
            b.extend_from_slice(&(nv as i32).to_be_bytes());
            b.extend_from_slice(&(nf as i32).to_be_bytes());
            nf.max(0) as usize
        }
        1 => {
            let v = if g.chance(5) { nv + 1 } else { nv };
            b.extend_from_slice(&(v as f32).to_bits().to_be_bytes());
            let c = face_count_bits(g, nf, maxf);
            b.extend_from_slice(&c.to_be_bytes());
            float_count(c).clamp(0, maxf) as usize
        }
        2 => {
            b.extend_from_slice(&(nv as i32).to_be_bytes());
            let c = (g.int(-1, maxf + 2) as i32).to_be_bytes();
            b.extend_from_slice(&c);
            g.int(0, maxf) as usize
        }
        _ => {
            b.extend_from_slice(&(nv as f32).to_bits().to_be_bytes());
            b.extend_from_slice(&(nf as f32).to_bits().to_be_bytes());
            nf.max(0) as usize
        }
    };
    for _ in 0..entries * per {
        let heavy = g.chance(4);
        b.extend_from_slice(&smallish(g, heavy).to_be_bytes());
    }
    if g.chance(8) && !b.is_empty() {
        let cut = g.int(1, 5.min(b.len() as i64)) as usize;
        b.truncate(b.len() - cut);
    }
    b
}

/// A w file: the latency (2 bytes), a count, then vertex numbers (3 bytes) and values; the count
/// at times beyond the entries the file holds, or the file cut (FS-167).
fn w_file(g: &mut Gen, nv: i64) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&(g.int(-50, 50) as i16).to_be_bytes());
    let num = g.int(0, 8);
    b.extend_from_slice(&(num as u32).to_be_bytes()[1..]);
    let present = if g.chance(6) { g.int(0, num) } else { num };
    for _ in 0..present {
        let k = if g.chance(8) {
            g.pick(&[0xff_ffffu32, 1000, nv as u32])
        } else {
            g.int(0, (nv + 1).max(0)) as u32
        };
        b.extend_from_slice(&k.to_be_bytes()[1..]);
        let heavy = g.chance(4);
        b.extend_from_slice(&smallish(g, heavy).to_be_bytes());
    }
    if g.chance(10) && !b.is_empty() {
        b.truncate(b.len() - 1);
    }
    b
}

/// Whether an input file of this name can be made: in the work folder or in `d`.
fn creatable(n: &str) -> bool {
    match n.rfind('/') {
        None => true,
        Some(i) => &n[..i] == "d",
    }
}

/// `FileNamePath` and the hemisphere's two letters of a surface name, as the triangle-property
/// functions take them (`None` without a dot; empty for a dot first or second, as `reproduce`).
fn triangle_paths(name: &str) -> Option<(String, String)> {
    let dot = name.rfind('.')?;
    let hemi = if dot < 2 {
        String::new()
    } else {
        name[dot - 2..dot].to_owned()
    };
    let path = match name.rfind('/') {
        Some(i) => name[..i].to_owned(),
        None => ".".to_owned(),
    };
    Some((path, hemi))
}

pub(super) fn sfile_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(3);
    let mut envs = 0;
    if g.chance(4) {
        envs |= 1;
    }
    if g.chance(4) {
        envs |= 2;
    }
    ix(t, envs);
    if envs & 2 != 0 {
        let v = g.pick(&["3", "-2", "x", " 12", "99999999999", "", "7x", "0"]);
        hx(t, v.as_bytes());
    }
    ix(t, g.int(0, 3));
    ix(t, i64::from(!g.chance(4)));
    let nv = g.int(0, 7);
    let nf = if nv >= 3 { g.int(0, 6) } else { 0 };
    let maxv = nv + g.pick(&[0, 0, 1, 3]);
    let maxf = nf + g.pick(&[0, 1, 1, 2]);
    // the operations first: the input files follow from them
    let nops = g.int(1, 3);
    let mut ops: Vec<String> = Vec::new();
    let mut inputs: Vec<(String, Vec<u8>)> = Vec::new();
    for k in 0..nops as usize {
        let op = g.pick(&OPS);
        ops.push(op.to_owned());
        match op {
            "sf" => ops.push(g.int(-2, 5).to_string()),
            "gf" | "ca" | "mm" => {}
            _ => {
                let n = name(g, k);
                if op == "rt"
                    && !g.chance(4)
                    && let Some((path, hemi)) = triangle_paths(&n)
                    && (path == "." || path == "d")
                {
                    inputs.push((
                        format!("{path}/{hemi}.triangle_area"),
                        triangle_file(g, nv, nf, maxf, 1),
                    ));
                    if !g.chance(4) {
                        inputs.push((
                            format!("{path}/{hemi}.triangle_angle"),
                            triangle_file(g, nv, nf, maxf, 3),
                        ));
                    }
                }
                if (op == "rb" || op == "ri") && !g.chance(5) && creatable(&n) {
                    inputs.push((n.clone(), w_file(g, nv)));
                }
                if op == "wC" && g.chance(3) && creatable(&n) {
                    inputs.push((n.clone(), b"x\n".to_vec()));
                }
                let mut tok = Vec::new();
                hx(&mut tok, n.as_bytes());
                ops.extend(tok);
            }
        }
    }
    inputs.sort();
    inputs.dedup_by(|a, b| a.0 == b.0);
    ix(t, inputs.len() as i64);
    for (n, b) in &inputs {
        hx(t, n.as_bytes());
        hx(t, b);
    }
    for v in [maxv, maxf, nv, nf] {
        ix(t, v);
    }
    ix(t, g.int(0, 2));
    let fname = g.pick(&[
        "d/lh.white",
        "lh.white",
        "d/rh.pial",
        "",
        "nodir/lh.x",
        "d/sub/lh.w",
    ]);
    hx(t, fname.as_bytes());
    let mut faces: Vec<[i64; 3]> = Vec::new();
    for _ in 0..maxf {
        if nv >= 3 {
            let a = g.int(0, nv - 1);
            let mut b = g.int(0, nv - 1);
            while b == a {
                b = g.int(0, nv - 1);
            }
            let mut c = g.int(0, nv - 1);
            while c == a || c == b {
                c = g.int(0, nv - 1);
            }
            faces.push([a, b, c]);
        } else {
            faces.push([0, 0, 0]);
        }
    }
    for vno in 0..nv {
        for _ in 0..13 {
            t.push(format!("{:08x}", smallish(g, heavy)));
        }
        ix(t, g.int(-3, 20));
        ix(t, g.int(-1, 30));
        ix(t, i64::from(g.chance(6)));
        let mine: Vec<(usize, usize)> = faces[..nf as usize]
            .iter()
            .enumerate()
            .flat_map(|(f, tri)| {
                tri.iter()
                    .enumerate()
                    .filter(move |(_, w)| **w == vno)
                    .map(move |(c, _)| (f, c))
            })
            .collect();
        ix(t, mine.len() as i64);
        for (f, _) in &mine {
            ix(t, *f as i64);
        }
        for (_, c) in &mine {
            ix(t, *c as i64);
        }
    }
    for f in &faces {
        for &v in f {
            ix(t, v);
        }
        ix(t, i64::from(g.chance(8)));
        for _ in 0..11 {
            t.push(format!("{:08x}", smallish(g, heavy)));
        }
        ix(t, if g.chance(5) { g.int(1, 3) } else { 0 });
    }
    t.push(format!("{:08x}", smallish(g, heavy)));
    ix(t, nops);
    t.extend(ops);
}

/// Every regular file under `dir`, by its relative name ('/' between parts), sorted.
fn list_files(dir: &std::path::Path, rel: &str, files: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        let r = if rel.is_empty() {
            n.clone()
        } else {
            format!("{rel}/{n}")
        };
        let Ok(ft) = e.file_type() else {
            continue;
        };
        if ft.is_dir() {
            list_files(&e.path(), &r, files);
        } else if ft.is_file() {
            files.push(r);
        }
    }
}

fn utf8(b: Vec<u8>) -> String {
    String::from_utf8(b).expect("a probe name is UTF-8")
}

pub(super) fn sfile_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let td = tempfile::tempdir().expect("a work folder");
    let dir = td.path();
    std::fs::create_dir(dir.join("d")).expect("the folder d");
    let envs = t.i();
    let mut pairs: Vec<(String, String)> = Vec::new();
    if envs & 1 != 0 {
        pairs.push(("FS_POSIX".into(), "1".into()));
    }
    if envs & 2 != 0 {
        let v = t.h();
        let v = &v[..v.iter().position(|&c| c == 0).unwrap_or(v.len())];
        pairs.push(("MRIS_READVAL_FRAME".into(), utf8(v.to_vec())));
    }
    let env = Environment::from_pairs(pairs);
    let gd = t.i();
    if gd & 1 != 0 {
        cx.g.gdiag |= DIAG_SHOW;
    }
    if gd & 2 != 0 {
        cx.g.gdiag |= DIAG_VERBOSE;
    }
    aaa::set_fix_vertex_area_value(&mut cx.sg, t.i());
    let nin = t.i();
    for _ in 0..nin {
        let (n, b) = (utf8(t.h()), t.h());
        std::fs::write(dir.join(&n), b).expect("an input file");
    }
    let (maxv, maxf, nv, nf) = (t.i(), t.i(), t.i(), t.i());
    let mut m = alloc::over_alloc(&mut cx.g, &mut cx.a, &mut cx.b, &cx.sg, maxv, maxf, nv, nf)?;
    m.hemisphere = t.i();
    let fname = t.h();
    let fname = &fname[..fname.iter().position(|&c| c == 0).unwrap_or(fname.len())];
    m.fname = utf8(fname[..fname.len().min(1023)].to_vec());
    for vno in 0..nv as usize {
        let mut f = [0.0f32; 13];
        for x in &mut f {
            *x = t.f();
        }
        let v = m.vertex_mut(vno);
        (v.x, v.y, v.z, v.nx, v.ny, v.nz) = (f[0], f[1], f[2], f[3], f[4], f[5]);
        (v.d, v.curv, v.val, v.valbak, v.imag_val) = (f[6], f[7], f[8], f[9], f[10]);
        (v.area, v.origarea) = (f[11], f[12]);
        v.marked = t.i() as i16;
        v.annotation = t.i();
        v.ripflag = t.i() as i8;
        let num = t.i();
        let fs: Vec<i32> = (0..num).map(|_| t.i()).collect();
        let ns: Vec<u8> = (0..num).map(|_| t.i() as u8).collect();
        let vt = m.topology_mut(vno);
        vt.num = num as u8;
        vt.f = Some(fs);
        vt.n = Some(ns);
    }
    for fno in 0..maxf as usize {
        let v = [t.i(), t.i(), t.i()];
        let rip = t.i() as i8;
        let mut f = [0.0f32; 11];
        for x in &mut f {
            *x = t.f();
        }
        let face = m.face_mut(fno);
        face.v = v;
        face.ripflag = rip;
        face.area = f[0];
        face.angle = [f[1], f[2], f[3]];
        face.orig_angle = [f[4], f[5], f[6]];
        let e = &mut m.faceNormCacheEntries.as_mut().unwrap()[fno];
        (e.nx, e.ny, e.nz, e.orig_area) = (f[7], f[8], f[9], f[10]);
        m.faceNormDeferredEntries.as_mut().unwrap()[fno].deferred = t.i() as i8;
    }
    m.orig_area = t.f();
    let nops = t.i();
    let g = &mut cx.g;
    let sg = &mut cx.sg;
    let mut io = coords::Io {
        out: &mut cx.a,
        err: &mut cx.b,
        env: &env,
    };
    let d = Some(dir);
    for _ in 0..nops {
        let op = t.next();
        match op {
            "sf" => sfile::set_read_frame(sg, t.i()),
            "gf" => o.i(i64::from(sfile::get_read_frame(g, sg, &io))),
            "ca" => o.i(i64::from(sfile::copy_marks_to_annotation(&mut m))),
            "mm" if maxv == 0 => o.s("G"),
            "mm" => o.i(i64::from(sfile::max_marked(g, &m))),
            _ => {
                let n = utf8(t.h());
                let r = match op {
                    "rt" | "wt" if n.len() >= 1024 || (op == "wt" && nf > 0 && nf == maxf) => {
                        o.s("G");
                        continue;
                    }
                    "rt" => sfile::read_triangle_properties(g, &mut io, d, &mut m, &n)?,
                    "wt" => sfile::write_triangle_properties(g, sg, &mut io, d, &mut m, &n)?,
                    "wd" => sfile::write_dists(g, &mut io, d, &m, &n)?,
                    "wa" => sfile::write_area_error(g, &mut io, d, &mut m, &n)?,
                    "wg" => sfile::write_angle_error(g, &mut io, d, &m, &n)?,
                    "wc" => sfile::write_curvature_to_wfile(g, &mut io, d, &m, &n)?,
                    "rb" => sfile::read_values_bak(g, &mut io, d, &mut m, &n),
                    "ri" => sfile::read_imag_values(g, &mut io, d, &mut m, &n),
                    "wA" => sfile::write_ascii(g, &mut io, d, &m, &n),
                    "wn" => sfile::write_normals_ascii(g, &mut io, d, &m, &n),
                    "wv" => sfile::write_vtk(g, &mut io, d, &m, &n),
                    "wC" => sfile::write_curv_vtk(g, &mut io, d, &m, &n),
                    _ => panic!("sfile op {op}"),
                };
                o.i(i64::from(r));
            }
        }
    }
    o.0 += "| ";
    for vno in 0..nv as usize {
        let v = m.vertex(vno);
        for x in [v.valbak, v.imag_val, v.area, v.origarea, v.nx, v.nz] {
            o.f(x);
        }
        o.i(i64::from(v.annotation));
    }
    for fno in 0..nf as usize {
        let f = m.face(fno).clone();
        o.f(f.area);
        for x in f.angle {
            o.f(x);
        }
        for x in f.orig_angle {
            o.f(x);
        }
        o.f(m.faceNormCacheEntries.as_ref().unwrap()[fno].orig_area);
        o.i(i64::from(
            m.faceNormDeferredEntries.as_ref().unwrap()[fno].deferred,
        ));
    }
    o.f(m.orig_area);
    o.f(m.total_area);
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
