// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of relay 39 of track D: `mht`, the surface hash table (`mht.rs`: the face
//! and vertex tables, the face tests), `mrisMarkIntersections`, `MRISfillHoles` and
//! `MRISmarkEdge`'s hole filling (`props10.rs`, `vals.rs`): a surface read by `MRISread`
//! (`d/lh.s0`: folded grids, crumpled icosahedra, srd's small meshes, two sheets crossing, a large
//! triangle over a grid (FS-911), a dense grid under a triangle (FS-912), now and then moved
//! beyond the table (FS-910) or given a coordinate that is not finite or far (FS-909, FS-913)),
//! `Gdiag_no`, faces and vertices ripped, marks and original positions now and then; then 1 to 5
//! operations: the tables (`ft`, `ftd`, `vt`) and their buckets in the box of the vertices'
//! voxels, `MHTdoesFaceIntersect` for every face (`dfi`), `MHTisVectorFilled` (`ivf`),
//! `MHTaddAllFaces` and `MHTremoveAllFaces` (`add`, `rem`), a vertex moved by up to 3 mm (`mv`),
//! `MHTtestIsMRISselfIntersecting` (`tis`), `mrisMarkIntersections` (`mi`), `MRISfillHoles`
//! (`fh`), `MRISmarkEdge` (`me`), `MHTfree` (`free`); after each `errno`: the generator and the
//! port's answers, as `fssurfio.cpp`'s.

use super::fli::{fv, surface};
use super::mpd::{grid, ico};
use super::srd::{Mesh, mesh};
use super::*;

use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, Mri, mri_alloc_sequence};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_surf::mht::{self, Mht, MhtFnoUsage, TABLE_CENTER, TABLE_SIZE};
use larmorx_freesurfer_surf::views::vertex_coord2xyz_float;
use larmorx_freesurfer_surf::{props10, readwrite, vals};

fn dv(t: &mut Vec<String>, x: f64) {
    t.push(format!("{:016x}", x.to_bits()));
}

/// Two `w` by `h` grids crossing: one in the plane z = 0 (heights `k/16`), one turned into the
/// plane y = `h/2` and moved.
fn sheets(g: &mut Gen) -> Mesh {
    let (w, h) = (g.int(3, 5), g.int(3, 5));
    let a = grid(g, w, h);
    let b = grid(g, w, h);
    let off = a.v.len() as i64;
    let mut v = a.v;
    let sx = g.int(-4, 4) as f32 / 4.0;
    for p in b.v {
        v.push([
            p[0] + sx,
            (h as f32 - 1.0) / 2.0 + p[2],
            p[1] - (h as f32 - 1.0) / 2.0,
        ]);
    }
    let mut f = a.f;
    f.extend(b.f.iter().map(|q| [q[0] + off, q[1] + off, q[2] + off]));
    Mesh { v, f }
}

/// Some vertices moved by up to 3 mm in each coordinate (quarters), so faces cross.
fn crumple(g: &mut Gen, m: &mut Mesh, k: u64) {
    for p in &mut m.v {
        if g.chance(k) {
            for c in p.iter_mut() {
                *c += g.int(-12, 12) as f32 / 4.0;
            }
        }
    }
}

/// A triangle of `side` mm in the plane z = 0 from the origin, with a small grid crossing it.
fn big_triangle(g: &mut Gen, side: f32) -> Mesh {
    let mut m = grid(g, 4, 4);
    for p in &mut m.v {
        p[2] = p[2] * 4.0 - 0.25;
        p[0] += 2.0;
        p[1] += 2.0;
    }
    let off = m.v.len() as i64;
    m.v.extend_from_slice(&[[0.0, 0.0, 0.0], [side, 0.0, 0.0], [0.0, side, 0.0]]);
    m.f.push([off, off + 1, off + 2]);
    m
}

/// A triangle over a dense 76 x 76 grid of 0.25 mm (11,250 faces near it: FS-912), the
/// triangle first.
fn dense(g: &mut Gen) -> Mesh {
    let n = 76i64;
    let mut v = vec![[-1.0, -1.0, 0.0], [40.0, -1.0, 0.0], [-1.0, 40.0, 0.0]];
    for j in 0..n {
        for i in 0..n {
            v.push([i as f32 * 0.25, j as f32 * 0.25, g.int(-4, 4) as f32 / 64.0]);
        }
    }
    let mut f = vec![[0, 1, 2]];
    for j in 0..n - 1 {
        for i in 0..n - 1 {
            let (a, b, c, d) = (
                3 + j * n + i,
                3 + j * n + i + 1,
                3 + (j + 1) * n + i,
                3 + (j + 1) * n + i + 1,
            );
            f.push([a, b, d]);
            f.push([a, d, c]);
        }
    }
    Mesh { v, f }
}

fn res(g: &mut Gen, heavy: bool) -> f32 {
    if heavy && g.chance(6) {
        return g.pick(&[0.0, -1.0, f32::NAN, -0.5]);
    }
    g.pick(&[1.0, 1.0, 1.0, 0.5, 2.0, 0.25])
}

fn which(g: &mut Gen, orig: bool) -> i64 {
    if g.chance(12) {
        return g.pick(&[1, 99, 3, 2]);
    }
    if orig && g.chance(3) { 0 } else { 4 }
}

fn small(g: &mut Gen, heavy: bool) -> f32 {
    if heavy && g.chance(12) {
        return g.pick(&[f32::NAN, f32::INFINITY, 2e9]);
    }
    g.int(-12, 12) as f32 / 4.0
}

pub(super) fn mht_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(5);
    let mut big = false;
    let mut m = match g.below(14) {
        0..=2 => {
            let (w, h) = (g.int(2, 6), g.int(2, 5));
            let mut m = grid(g, w, h);
            crumple(g, &mut m, 3);
            m
        }
        3 | 4 => {
            let mut m = mesh(g, heavy);
            let far =
                m.v.iter()
                    .flatten()
                    .fold(0f32, |a, &c| if c.abs() > a { c.abs() } else { a });
            if far.is_finite() && far > 12.0 {
                for c in m.v.iter_mut().flatten() {
                    *c = *c / far * 12.0;
                }
            }
            m
        }
        5 | 6 => {
            let r = g.pick(&[3.0, 5.0, 8.0]);
            let mut m = ico(g, 1, r);
            crumple(g, &mut m, 4);
            m
        }
        7 => {
            let r = g.pick(&[4.0, 6.0]);
            let mut m = ico(g, 2, r);
            if g.chance(2) {
                crumple(g, &mut m, 10);
            }
            m
        }
        8..=10 => sheets(g),
        11 => {
            let (w, h) = (g.int(2, 6), g.int(2, 5));
            grid(g, w, h)
        }
        12 => {
            big = true;
            let side = g.pick(&[150.0, 20.0, 160.0]);
            big_triangle(g, side)
        }
        _ => {
            if heavy && g.chance(4) {
                big = true;
                dense(g)
            } else {
                sheets(g)
            }
        }
    };
    if g.chance(10) {
        // FS-910: beyond the table along one axis at 1 mm
        let k = g.below(3) as usize;
        let d = g.pick(&[1001.5f32, -1000.75, 1600.0]);
        for p in &mut m.v {
            p[k] += d;
        }
    }
    if heavy && !big && g.chance(4) && !m.v.is_empty() {
        let k = g.below(m.v.len() as u64) as usize;
        m.v[k][g.below(3) as usize] = g.pick(&[f32::NAN, f32::INFINITY, 1e12, -3e9]);
    }
    surface(t, &m);
    let (nv, nf) = (m.v.len() as i64, m.f.len() as i64);
    ix(t, g.pick(&[-1, -1, -1, -1, 2]));
    let nfrip = if g.chance(5) && nf > 0 && !big {
        g.int(1, 3)
    } else {
        0
    };
    ix(t, nfrip);
    for _ in 0..nfrip {
        ix(t, g.int(0, nf - 1));
    }
    let nvrip = if g.chance(6) && nv > 0 && !big {
        g.int(1, 2)
    } else {
        0
    };
    ix(t, nvrip);
    for _ in 0..nvrip {
        ix(t, g.int(0, nv - 1));
    }
    let marks = g.chance(3) && !big;
    ix(t, i64::from(marks));
    if marks {
        for _ in 0..nv {
            ix(t, g.pick(&[0, 0, 1, 2]));
        }
    }
    let orig = g.chance(4) && !big;
    ix(t, i64::from(orig));
    if orig {
        let s = g.pick(&[0.5f32, 2.0, 1.0]);
        for p in &m.v {
            for c in p {
                fv(t, c * s + 0.25);
            }
        }
    }
    let nops = if big { g.int(1, 2) } else { g.int(1, 5) };
    ix(t, nops);
    for _ in 0..nops {
        let k = if big {
            g.pick(&[0, 1, 2, 7, 8])
        } else {
            g.below(19)
        };
        match k {
            0 | 1 => {
                t.push("ft".into());
                ix(t, which(g, orig));
                // a large face at a fine resolution: FreeSurfer's quadratic list search passes the
                // official probe's alarm
                fv(
                    t,
                    if big {
                        g.pick(&[1.0, 2.0])
                    } else {
                        res(g, heavy)
                    },
                );
            }
            2 | 3 => t.push("ftd".into()),
            4 => {
                t.push("vt".into());
                ix(t, which(g, orig));
                fv(t, res(g, heavy));
            }
            5 | 6 => t.push("dfi".into()),
            7 | 8 => {
                t.push("mi".into());
                ix(t, g.pick(&[0, 1, 1]));
            }
            9 => {
                t.push("ivf".into());
                ix(t, g.int(0, nv - 1));
                for _ in 0..3 {
                    fv(t, small(g, heavy));
                }
            }
            10 | 11 => {
                t.push(g.pick(&["add", "rem"]).into());
                ix(t, g.int(0, nv - 1));
            }
            12 => {
                t.push("mv".into());
                ix(t, g.int(0, nv - 1));
                for _ in 0..3 {
                    fv(t, small(g, heavy));
                }
            }
            13 => {
                t.push("tis".into());
                fv(t, res(g, heavy));
            }
            14 | 15 => {
                t.push("fh".into());
                let field = g.pick(&[
                    "marked", "marked", "val", "curv", "undefval", "nosuch", "-", "-",
                ]);
                t.push(field.into());
                let usemri = if field == "-" {
                    !g.chance(8)
                } else {
                    g.chance(10)
                };
                ix(t, i64::from(usemri));
                dv(
                    t,
                    if heavy && g.chance(6) {
                        f64::NAN
                    } else {
                        g.pick(&[0.5, 0.5, 1.0, 0.0])
                    },
                );
                if usemri {
                    for _ in 0..nv {
                        fv(t, g.pick(&[0.0, 0.0, 1.0, 1.0, 0.5, 2.0, -1.0]));
                    }
                }
            }
            16 | 17 => {
                t.push("me".into());
                ix(t, g.pick(&[0, 1, 2, 2, 2, 5]));
                dv(t, g.pick(&[100.0, 30.0, 0.5, 1.5, 150.0, 60.0]));
                ix(t, g.pick(&[0, 1, 1]));
            }
            _ => t.push("free".into()),
        }
    }
}

/// `x / res + 1000`, truncated, `None` when the conversion is undefined (as the C++ probe's).
fn vox(x: f32, res: f32) -> Option<i32> {
    let w = f64::from(x) / f64::from(res) + f64::from(TABLE_CENTER);
    (w > -2_147_483_649.0 && w < 2_147_483_648.0).then_some(w as i32)
}

fn dump(
    g: &mut Globals,
    err: &mut dyn Write,
    s: &Mris,
    mht: Option<&Mht>,
    o: &mut Out,
) -> Result<(), Fatal> {
    let Some(mht) = mht else {
        o.s("NOMHT");
        return Ok(());
    };
    o.i(i64::from(mht.which()));
    o.f(mht.vres());
    o.i(if mht.fno_usage() == MhtFnoUsage::Face {
        0
    } else {
        1
    });
    let (mut lo, mut hi) = ([TABLE_SIZE; 3], [-1i32; 3]);
    for v in 0..s.nvertices.max(0) as usize {
        let (x, y, z) = vertex_coord2xyz_float(g, err, s.vertex(v), mht.which())?;
        for (k, c) in [x, y, z].into_iter().enumerate() {
            let Some(x) = vox(c, mht.vres()) else {
                continue;
            };
            let x = x.clamp(0, TABLE_SIZE - 1);
            lo[k] = lo[k].min(x);
            hi[k] = hi[k].max(x);
        }
    }
    let mut vol = 1i64;
    for k in 0..3 {
        if hi[k] < 0 {
            o.s("EMPTYBOX");
            return Ok(());
        }
        lo[k] = (lo[k] - 1).max(0);
        hi[k] = (hi[k] + 1).min(TABLE_SIZE - 1);
        vol *= i64::from(hi[k] - lo[k] + 1);
    }
    for k in 0..3 {
        o.i(i64::from(lo[k]));
        o.i(i64::from(hi[k]));
    }
    if vol > 400_000 {
        o.s("BOX");
        o.i(vol);
        return Ok(());
    }
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                if let Some(b) = mht.bucket(x, y, z) {
                    o.i(i64::from(x));
                    o.i(i64::from(y));
                    o.i(i64::from(z));
                    o.i(b.len() as i64);
                    for &f in b {
                        o.i(i64::from(f));
                    }
                }
            }
        }
    }
    o.s(";");
    Ok(())
}

fn vertices(s: &Mris, o: &mut Out) {
    for v in 0..s.nvertices.max(0) as usize {
        let x = s.vertex(v);
        o.i(i64::from(x.marked));
        o.f(x.val);
        o.i(i64::from(x.undefval));
    }
}

pub(super) fn mht_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let td = tempfile::tempdir().expect("a work folder");
    let dir = td.path();
    std::fs::create_dir(dir.join("d")).expect("the folder d");
    cx.g.progname = "fssurfio".into();
    let bytes = t.h();
    let (nv, nf) = (t.i(), t.i());
    let gdiag_no = t.i();
    let env = Environment::from_pairs(vec![("SURFER_SIDEDOOR".to_owned(), "1".to_owned())]);
    std::fs::write(dir.join("d/lh.s0"), bytes).expect("the surface file");
    let d = Some(dir);
    let (mut so, mut se) = (cx.a.clone(), cx.b.clone());
    let mut io = coords::Io {
        out: &mut so,
        err: &mut se,
        env: &env,
    };
    let g = &mut cx.g;
    let sg = &mut cx.sg;
    g.errno = 0;
    g.gdiag_no = -1;
    let Some(mut s) = readwrite::read(g, sg, &mut io, d, "d/lh.s0", true)? else {
        o.s("NULL");
        return Ok(());
    };
    if nv != s.nvertices || nf != s.nfaces {
        o.i(i64::from(s.nvertices));
        o.i(i64::from(s.nfaces));
        o.s("COUNTS");
        return Ok(());
    }
    g.gdiag_no = gdiag_no;
    for _ in 0..t.i() {
        s.face_mut(t.i() as usize).ripflag = 1;
    }
    for _ in 0..t.i() {
        s.vertex_mut(t.i() as usize).ripflag = 1;
    }
    if t.i() != 0 {
        for v in 0..nv as usize {
            s.vertex_mut(v).marked = t.i() as i16;
        }
    }
    if t.i() != 0 {
        for v in 0..nv as usize {
            let x = s.vertex_mut(v);
            (x.origx, x.origy, x.origz) = (t.f(), t.f(), t.f());
        }
    }
    let mut table: Option<Mht> = None;
    let nops = t.i();
    for _ in 0..nops {
        let op = t.next();
        o.s(op);
        g.errno = 0;
        match op {
            "ft" | "vt" => {
                let (which, r) = (t.i(), t.f());
                table = Some(if op == "ft" {
                    Mht::create_face_table_resolution(g, io.err, &s, which, r)?
                } else {
                    Mht::create_vertex_table_resolution(g, io.err, &s, which, r)?
                });
                dump(g, io.err, &s, table.as_ref(), o)?;
            }
            "ftd" => {
                table = Some(Mht::create_face_table(g, io.err, &s)?);
                dump(g, io.err, &s, table.as_ref(), o)?;
            }
            "dfi" => {
                let Some(m) = &table else {
                    o.s("G");
                    continue;
                };
                for f in 0..nf {
                    o.i(i64::from(m.does_face_intersect(g, io.err, &s, f)?));
                }
            }
            "ivf" => {
                let vno = t.i();
                let (dx, dy, dz) = (t.f(), t.f(), t.f());
                let Some(m) = &table else {
                    o.s("G");
                    continue;
                };
                o.i(i64::from(
                    m.is_vector_filled(g, io.err, &s, vno, dx, dy, dz)?,
                ));
            }
            "add" | "rem" => {
                let vno = t.i();
                let Some(m) = &mut table else {
                    o.s("G");
                    continue;
                };
                let r = if op == "add" {
                    m.add_all_faces(g, io.err, &s, vno)?
                } else {
                    m.remove_all_faces(g, io.err, &s, vno)?
                };
                o.i(i64::from(r));
                dump(g, io.err, &s, table.as_ref(), o)?;
            }
            "mv" => {
                let vno = t.i() as usize;
                let (dx, dy, dz) = (t.f(), t.f(), t.f());
                let x = s.vertex_mut(vno);
                x.x = sse::add(x.x, dx);
                x.y = sse::add(x.y, dy);
                x.z = sse::add(x.z, dz);
            }
            "tis" => {
                let r = t.f();
                o.i(i64::from(mht::test_is_mris_self_intersecting(
                    g, io.err, &s, r,
                )?));
            }
            "mi" => {
                let fill = t.i();
                o.i(i64::from(props10::mris_mark_intersections(
                    g, &mut io, &mut s, fill,
                )?));
                vertices(&s, o);
            }
            "fh" => {
                let field = t.next();
                let usemri = t.i() != 0;
                let thresh = t.d();
                let mut m: Option<Mri> = None;
                if usemri {
                    let mut x = mri_alloc_sequence(g, io.out, io.err, nv, 1, 1, MRI_FLOAT, 1)?;
                    for v in 0..nv {
                        let val = t.f();
                        mri_set_vox_val(g, io.err, &mut x, v, 0, 0, 0, val, 0)?;
                    }
                    m = Some(x);
                }
                let name = (field != "-").then_some(field);
                let r = vals::fill_holes(g, &mut io, &mut s, name, m.as_mut(), thresh)?;
                o.i(i64::from(r));
                vertices(&s, o);
                if let Some(m) = &m {
                    for v in 0..nv {
                        o.f(mri_get_vox_val(g, io.err, m, v, 0, 0, 0, 0)?);
                    }
                }
            }
            "me" => {
                let (id, th, fill) = (t.i(), t.d(), t.i());
                let r = props10::mris_mark_edge(g, sg, &mut io, &mut s, None, id, th, fill)?;
                o.i(i64::from(r));
                vertices(&s, o);
            }
            "free" => {
                table = None;
                o.s("FREED");
            }
            _ => panic!("mht op {op}"),
        }
        o.i(i64::from(g.errno));
    }
    Ok(())
}
