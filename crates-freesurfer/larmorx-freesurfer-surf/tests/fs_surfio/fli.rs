// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of relay 32 of track D: `fli`, `mrisurf_mri.cpp`'s surface-to-voxel maps
//! (`sras.rs`) and `mriflood.cpp`'s surface functions (`mriflood.rs`): one or two surfaces read by
//! `MRISread` (`d/lh.s0`, `d/lh.s1`: subdivided icosahedra moved off the centre, the second often
//! the first shrunk; open grids, srd's small meshes; now and then a face's second corner moved
//! onto its first), each with a volume geometry of the case (valid or not; LIA, RAS, LPS, a
//! permutation, an oblique rotation, now and then a singular one), `Gdiag`'s show and verbose bits
//! and `Gdiag_no` now and then, 1 to 3 volumes (`uchar` mostly; the surface's geometry, the same
//! with another extent, or their own; zeros or a pattern), then 1 to 4 operations: the four
//! conversions (`r2v`, `v2r`, `r2vc`, `v2rc`), a map made, used, loaded again for another pair and
//! used (`map`), the shell (`shl`), the ribbon (`rib`), the bounding volume (`bbv`), the interior
//! (`fil`) and the bounding volume filled (`bbf`), each printing its numbers or volume: the
//! generator and the port's answers, as `fssurfio.cpp`'s.

use super::gwr::gii_normal;
use super::mpd::{grid, ico};
use super::sfile::list_files;
use super::srd::{Mesh, be4, mesh};
use super::*;

use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_mri::mri::{Mri, mri_alloc};
use larmorx_freesurfer_mri::vol_geom::reinit_cache;
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_surf::{curvfile, mriflood, readwrite, sras, valfile, vpos};

/// The direction cosines (`x_r x_a x_s y_r y_a y_s z_r z_a z_s`) the cases pick from.
pub(super) const FRAMES: [[f32; 9]; 6] = [
    [-1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0],
    [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
    [-1.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 1.0],
    [0.0, -1.0, 0.0, 0.0, 0.0, -1.0, 1.0, 0.0, 0.0],
    [0.6, 0.8, 0.0, -0.8, 0.6, 0.0, 0.0, 0.0, 1.0],
    [1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0],
];

pub(super) fn fv(t: &mut Vec<String>, x: f32) {
    t.push(format!("{:08x}", x.to_bits()));
}

/// A geometry's 15 floats: voxel sizes, direction cosines, centre.
pub(super) fn geom(t: &mut Vec<String>, g: &mut Gen, heavy: bool) -> [f32; 15] {
    let mut v = [0f32; 15];
    let size = |g: &mut Gen| {
        if heavy && g.chance(30) {
            0.0
        } else {
            g.pick(&[1.0, 1.0, 1.0, 0.5, 2.0, 1.5, 0.75])
        }
    };
    let iso = g.chance(2);
    let s0 = size(g);
    v[0] = s0;
    v[1] = if iso { s0 } else { size(g) };
    v[2] = if iso { s0 } else { size(g) };
    let fr = if g.chance(40) {
        FRAMES[5]
    } else {
        FRAMES[g.below(5) as usize]
    };
    v[3..12].copy_from_slice(&fr);
    for k in 12..15 {
        v[k] = if g.chance(3) {
            0.0
        } else {
            g.int(-64, 64) as f32 / 8.0
        };
    }
    if heavy && g.chance(30) {
        v[12 + g.below(3) as usize] = f32::from_bits(g.pick(&[0x7fc0_0000, 0x7f80_0000]));
    }
    for x in v {
        fv(t, x);
    }
    v
}

/// A surface file (binary triangle, no tags) of `m`.
fn surface_bytes(m: &Mesh) -> Vec<u8> {
    let mut b = vec![0xff, 0xff, 0xfe];
    b.extend_from_slice(b"created by me on Sun Nov 15 06:10:00 2026\n\n");
    be4(&mut b, m.v.len() as i64);
    be4(&mut b, m.f.len() as i64);
    for p in &m.v {
        for c in p {
            b.extend_from_slice(&c.to_be_bytes());
        }
    }
    for f in &m.f {
        for &x in f {
            be4(&mut b, x);
        }
    }
    b
}

pub(super) fn surface(t: &mut Vec<String>, m: &Mesh) {
    hx(t, &surface_bytes(m));
    ix(t, m.v.len() as i64);
    ix(t, m.f.len() as i64);
}

pub(super) fn fli_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(5);
    let mut m = match g.below(12) {
        0 => {
            let (w, h) = (g.int(2, 5), g.int(2, 5));
            grid(g, w, h)
        }
        1 => mesh(g, heavy),
        2 | 3 => {
            let r = g.pick(&[3.0, 4.5, 6.0]);
            ico(g, 2, r)
        }
        _ => {
            let r = g.pick(&[2.0, 3.0, 4.5, 6.0]);
            ico(g, 1, r)
        }
    };
    // srd's meshes can reach 10,000 mm: scaled into 8 mm, so a face spans few voxels (the
    // official probe's alarm is 5 seconds)
    let big =
        m.v.iter()
            .flatten()
            .fold(0f32, |a, &c| if c.abs() > a { c.abs() } else { a });
    if big.is_finite() && big > 8.0 {
        for c in m.v.iter_mut().flatten() {
            *c = *c / big * 8.0;
        }
    }
    let off: [f32; 3] = [0, 1, 2].map(|_| {
        if g.chance(2) {
            0.0
        } else {
            g.int(-12, 12) as f32 / 4.0
        }
    });
    for p in &mut m.v {
        for k in 0..3 {
            p[k] += off[k];
        }
    }
    // FS-606: a face's second corner on its first now and then
    if g.chance(8) && !m.f.is_empty() {
        let f = m.f[g.below(m.f.len() as u64) as usize];
        let p = m.v[f[0] as usize];
        m.v[f[1] as usize] = p;
    }
    let two = g.chance(2);
    ix(t, i64::from(two));
    surface(t, &m);
    if two {
        let inner = if g.chance(4) {
            match g.below(3) {
                0 => grid(g, 3, 3),
                _ => {
                    let r = g.pick(&[2.0, 3.0]);
                    ico(g, 1, r)
                }
            }
        } else {
            let k = g.pick(&[0.5f32, 0.6, 0.75]);
            Mesh {
                v: m.v
                    .iter()
                    .map(|p| [0, 1, 2].map(|i| (p[i] - off[i]) * k + off[i]))
                    .collect(),
                f: m.f.clone(),
            }
        };
        surface(t, &inner);
    }
    // the surfaces' volume geometries
    let dims = [0, 1, 2].map(|_| g.int(8, 20));
    let mut first: Option<[f32; 15]> = None;
    for s in 0..1 + usize::from(two) {
        let valid = !g.chance(5);
        ix(t, i64::from(valid));
        let same = s == 1 && g.chance(2);
        let d = if same {
            dims
        } else {
            [0, 1, 2].map(|_| g.int(8, 20))
        };
        let d = if s == 0 { dims } else { d };
        for x in d {
            ix(t, x);
        }
        if same && let Some(v) = first {
            for x in v {
                fv(t, x);
            }
        } else {
            let v = geom(t, g, heavy);
            first.get_or_insert(v);
        }
    }
    let gdiag = if g.chance(8) { 0x1000_0040u64 } else { 0 };
    ix(t, gdiag as i64);
    ix(t, if g.chance(8) { 1 } else { -1 });
    // volumes
    let nvol = g.int(1, 3);
    ix(t, nvol);
    let mut vdims = Vec::new();
    for _ in 0..nvol {
        ix(t, g.pick(&[0, 0, 0, 0, 1, 3, 4]));
        let kind = g.below(4);
        let d = if kind == 2 {
            [0, 1, 2].map(|_| g.int(4, 20))
        } else {
            dims
        };
        for x in d {
            ix(t, x);
        }
        vdims.push(d);
        match (kind, first) {
            (0 | 2, Some(v)) => {
                for x in v {
                    fv(t, x);
                }
            }
            _ => {
                geom(t, g, heavy);
            }
        }
        ix(t, if g.chance(3) { g.int(1, 3) } else { 0 });
    }
    let nsurf = 1 + i64::from(two);
    let pt = |t: &mut Vec<String>, g: &mut Gen| {
        for _ in 0..3 {
            let x = if heavy && g.chance(20) {
                f64::from(f32::from_bits(g.pick(&SPECIAL32)))
            } else {
                g.int(-160, 160) as f64 / 16.0
            };
            t.push(format!("{:016x}", x.to_bits()));
        }
    };
    // a destination: none, or another volume (of the source's extent four times in five, when
    // there is one; MRIintersect refuses other extents)
    let other = |g: &mut Gen, v: i64| -> i64 {
        if nvol == 1 || g.chance(3) {
            return -1;
        }
        let same: Vec<i64> = (0..nvol)
            .filter(|&d| d != v && vdims[d as usize] == vdims[v as usize])
            .collect();
        if !same.is_empty() && !g.chance(5) {
            return same[g.below(same.len() as u64) as usize];
        }
        let mut d = g.int(0, nvol - 1);
        if d == v {
            d = (d + 1) % nvol;
        }
        d
    };
    let res = |t: &mut Vec<String>, g: &mut Gen| {
        let r = if heavy && g.chance(10) {
            -1.0f64
        } else {
            g.pick(&[1.0, 1.0, 0.5, 2.0, 1.5, 0.75])
        };
        t.push(format!("{:016x}", r.to_bits()));
    };
    let nops = g.int(1, 4);
    ix(t, nops);
    for _ in 0..nops {
        let s = g.int(0, nsurf - 1);
        let v = g.int(0, nvol - 1);
        match g.below(12) {
            0 | 1 => {
                t.push(g.pick(&["r2v", "v2r", "r2vc", "v2rc"]).into());
                ix(t, s);
                ix(t, v);
                pt(t, g);
            }
            2 => {
                t.push("map".into());
                for _ in 0..2 {
                    ix(t, g.int(0, nsurf - 1));
                    ix(t, g.int(0, nvol - 1));
                    let n = g.int(1, 3);
                    ix(t, n);
                    for _ in 0..n {
                        pt(t, g);
                    }
                }
            }
            3 | 4 => {
                t.push("shl".into());
                ix(t, s);
                ix(t, v);
                let d = other(g, v);
                ix(t, d);
                ix(t, i64::from(!g.chance(4)));
            }
            5 | 6 => {
                t.push("rib".into());
                ix(t, nsurf - 1);
                ix(t, 0);
                ix(t, v);
                let d = other(g, v);
                ix(t, d);
            }
            7 => {
                t.push("bbv".into());
                ix(t, s);
                res(t, g);
            }
            8 | 9 => {
                t.push("fil".into());
                ix(t, s);
                res(t, g);
                ix(t, if g.chance(2) { -1 } else { v });
            }
            _ => {
                t.push("bbf".into());
                ix(t, s);
                res(t, g);
            }
        }
    }
}

/// A volume as the probe prints it: its extent, type, frames, voxel sizes, direction cosines,
/// centre and `ras_good_flag`, then its voxels (frame, slice, row, column) as runs `<bits>x<n>`,
/// and `E`.
pub(super) fn fli_vol(g: &mut Globals, err: &mut dyn Write, m: Option<&Mri>, o: &mut Out) {
    let Some(m) = m else {
        o.s("NULL");
        return;
    };
    let v = &m.vg;
    for x in [v.width, v.height, v.depth, m.mtype, m.nframes] {
        o.i(i64::from(x));
    }
    for x in [
        v.xsize, v.ysize, v.zsize, v.x_r, v.x_a, v.x_s, v.y_r, v.y_a, v.y_s, v.z_r, v.z_a, v.z_s,
        v.c_r, v.c_a, v.c_s,
    ] {
        o.f(x);
    }
    o.i(i64::from(v.ras_good_flag));
    let mut run: Option<(u32, u64)> = None;
    for f in 0..m.nframes {
        for s in 0..v.depth {
            for r in 0..v.height {
                for c in 0..v.width {
                    let x = mri_get_vox_val(g, err, m, c, r, s, f, 0)
                        .unwrap_or(0.0)
                        .to_bits();
                    run = match run {
                        Some((b, n)) if b == x => Some((b, n + 1)),
                        Some((b, n)) => {
                            o.s(&format!("{b:08x}x{n}"));
                            Some((x, 1))
                        }
                        None => Some((x, 1)),
                    };
                }
            }
        }
    }
    if let Some((b, n)) = run {
        o.s(&format!("{b:08x}x{n}"));
    }
    o.s("E");
}

/// `vols[a]` and `vols[b]` (`a != b`), the second mutable.
pub(super) fn pair(vols: &mut [Mri], a: usize, b: usize) -> (&Mri, &mut Mri) {
    if a < b {
        let (l, r) = vols.split_at_mut(b);
        (&l[a], &mut r[0])
    } else {
        let (l, r) = vols.split_at_mut(a);
        (&r[0], &mut l[b])
    }
}

pub(super) fn ret_vol(
    g: &mut Globals,
    err: &mut dyn Write,
    r: Ret<Mri>,
    given: Option<&Mri>,
    o: &mut Out,
) {
    match r {
        Ret::New(m) => {
            o.s("N");
            fli_vol(g, err, Some(&m), o);
        }
        Ret::Given => {
            o.s("G");
            fli_vol(g, err, given, o);
        }
        Ret::Null => o.s("NULL"),
    }
}

pub(super) fn fli_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let td = tempfile::tempdir().expect("a work folder");
    let dir = td.path();
    std::fs::create_dir(dir.join("d")).expect("the folder d");
    cx.g.progname = "fssurfio".into();
    let two = t.i() != 0;
    let mut files = Vec::new();
    for k in 0..1 + usize::from(two) {
        let bytes = t.h();
        let (nv, nf) = (t.i(), t.i());
        let name = format!("d/lh.s{k}");
        std::fs::write(dir.join(&name), bytes).expect("the surface file");
        files.push((name, nv, nf));
    }
    let env = Environment::from_pairs(vec![("SURFER_SIDEDOOR".to_owned(), "1".to_owned())]);
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
    let mut surfs = Vec::new();
    for (name, nv, nf) in &files {
        let Some(s) = readwrite::read(g, sg, &mut io, d, name, true)? else {
            o.s("NULL");
            return Ok(());
        };
        if *nv != s.nvertices || *nf != s.nfaces {
            o.i(i64::from(s.nvertices));
            o.i(i64::from(s.nfaces));
            o.s("COUNTS");
            return Ok(());
        }
        surfs.push(s);
    }
    for s in surfs.iter_mut() {
        let vg = &mut s.vg;
        vg.valid = t.i();
        (vg.width, vg.height, vg.depth) = (t.i(), t.i(), t.i());
        (vg.xsize, vg.ysize, vg.zsize) = (t.f(), t.f(), t.f());
        (vg.x_r, vg.x_a, vg.x_s) = (t.f(), t.f(), t.f());
        (vg.y_r, vg.y_a, vg.y_s) = (t.f(), t.f(), t.f());
        (vg.z_r, vg.z_a, vg.z_s) = (t.f(), t.f(), t.f());
        (vg.c_r, vg.c_a, vg.c_s) = (t.f(), t.f(), t.f());
    }
    g.gdiag = t.i() as u32 as u64;
    g.gdiag_no = t.i();
    let nvol = t.i();
    let mut vols: Vec<Mri> = Vec::new();
    for _ in 0..nvol {
        let ty = t.i();
        let (w, h, dd) = (t.i(), t.i(), t.i());
        let mut v = mri_alloc(g, io.out, io.err, w, h, dd, ty)?;
        let vg = &mut v.vg;
        (vg.xsize, vg.ysize, vg.zsize) = (t.f(), t.f(), t.f());
        (vg.x_r, vg.x_a, vg.x_s) = (t.f(), t.f(), t.f());
        (vg.y_r, vg.y_a, vg.y_s) = (t.f(), t.f(), t.f());
        (vg.z_r, vg.z_a, vg.z_s) = (t.f(), t.f(), t.f());
        (vg.c_r, vg.c_a, vg.c_s) = (t.f(), t.f(), t.f());
        reinit_cache(g, io.out, io.err, &mut v.vg)?;
        let fill = t.i();
        if fill != 0 {
            for s in 0..dd {
                for r in 0..h {
                    for c in 0..w {
                        let x = ((c + 2 * r + 3 * s + fill) % 4) as f32;
                        mri_set_vox_val(g, io.err, &mut v, c, r, s, 0, x, 0)?;
                    }
                }
            }
        }
        vols.push(v);
    }
    let nops = t.i();
    for _ in 0..nops {
        let op = t.next();
        o.s(op);
        match op {
            "r2v" | "v2r" | "r2vc" | "v2rc" => {
                let (s, v) = (t.i() as usize, t.i() as usize);
                let (a, b, c) = (t.d(), t.d(), t.d());
                let (m, vol) = (&surfs[s], &vols[v]);
                let p = match op {
                    "r2v" => sras::mris_surface_ras_to_voxel(g, &mut io, m, vol, a, b, c)?,
                    "v2r" => sras::mris_surface_ras_from_voxel(g, &mut io, m, vol, a, b, c)?,
                    "r2vc" => {
                        sras::mris_surface_ras_to_voxel_cached(g, sg, &mut io, m, vol, a, b, c)?
                    }
                    _ => sras::mris_surface_ras_from_voxel_cached(g, sg, &mut io, m, vol, a, b, c)?,
                };
                p.iter().for_each(|&x| o.d(x));
            }
            "map" => {
                let (s, v) = (t.i() as usize, t.i() as usize);
                let mut map = sras::mris_make_ras2voxel_map(g, &mut io, &vols[v], &surfs[s])?;
                for k in 0..2 {
                    if k == 1 {
                        let (s, v) = (t.i() as usize, t.i() as usize);
                        sras::mris_load_ras2voxel_map(g, &mut io, &mut map, &vols[v], &surfs[s])?;
                    }
                    let n = t.i();
                    for _ in 0..n {
                        let (a, b, c) = (t.d(), t.d(), t.d());
                        let p = sras::mris_use_ras2voxel_map(g, &mut io, &mut map, a, b, c)?;
                        p.iter().for_each(|&x| o.d(x));
                    }
                }
                sras::mris_free_ras2voxel_map(map);
            }
            "shl" => {
                let (s, v, di, clear) = (t.i() as usize, t.i() as usize, t.i(), t.i());
                if di >= 0 {
                    let (src, dst) = pair(&mut vols, v, di as usize);
                    let r = mriflood::mris_shell(g, sg, &mut io, src, &surfs[s], Some(dst), clear)?;
                    let given = &vols[di as usize];
                    ret_vol(g, io.err, r, Some(given), o);
                } else {
                    let r = mriflood::mris_shell(g, sg, &mut io, &vols[v], &surfs[s], None, clear)?;
                    ret_vol(g, io.err, r, None, o);
                }
            }
            "rib" => {
                let (si, so_, v, di) = (t.i() as usize, t.i() as usize, t.i() as usize, t.i());
                if di >= 0 {
                    let (src, dst) = pair(&mut vols, v, di as usize);
                    let r = mriflood::mris_ribbon(
                        g,
                        sg,
                        &mut io,
                        &surfs[si],
                        &surfs[so_],
                        src,
                        Some(dst),
                    )?;
                    let given = &vols[di as usize];
                    ret_vol(g, io.err, r, Some(given), o);
                } else {
                    let r = mriflood::mris_ribbon(
                        g,
                        sg,
                        &mut io,
                        &surfs[si],
                        &surfs[so_],
                        &vols[v],
                        None,
                    )?;
                    ret_vol(g, io.err, r, None, o);
                }
            }
            "bbv" => {
                let (s, res) = (t.i() as usize, t.d());
                let r = mriflood::mris_make_bounding_volume(g, &mut io, &surfs[s], res)?;
                fli_vol(g, io.err, Some(&r), o);
            }
            "fil" => {
                let (s, res, di) = (t.i() as usize, t.d(), t.i());
                if di >= 0 {
                    let dst = &mut vols[di as usize];
                    let r = mriflood::mris_fill_interior(
                        g,
                        sg,
                        &mut io,
                        &mut surfs[s],
                        res,
                        Some(dst),
                    )?;
                    let given = &vols[di as usize];
                    ret_vol(g, io.err, r, Some(given), o);
                } else {
                    let r = mriflood::mris_fill_interior(g, sg, &mut io, &mut surfs[s], res, None)?;
                    ret_vol(g, io.err, r, None, o);
                }
            }
            "bbf" => {
                let (s, res) = (t.i() as usize, t.d());
                let mut b = mriflood::mris_make_bounding_volume(g, &mut io, &surfs[s], res)?;
                let r =
                    mriflood::mris_fill_interior(g, sg, &mut io, &mut surfs[s], res, Some(&mut b))?;
                ret_vol(g, io.err, r, Some(&b), o);
            }
            _ => panic!("fli op {op}"),
        }
    }
    Ok(())
}

/// `flr` (`fssurfio.cpp`, relay 32 of track D): a recorded head's result as its extent, type,
/// frames, geometry, the count of voxels that are not 0 and the FNV-1a of its voxels' bytes.
fn flr_put(g: &mut Globals, err: &mut dyn Write, m: Option<&Mri>, o: &mut Out) {
    let Some(m) = m else {
        o.s("NULL");
        return;
    };
    let v = &m.vg;
    for x in [v.width, v.height, v.depth, m.mtype, m.nframes] {
        o.i(i64::from(x));
    }
    for x in [
        v.xsize, v.ysize, v.zsize, v.x_r, v.x_a, v.x_s, v.y_r, v.y_a, v.y_s, v.z_r, v.z_a, v.z_s,
        v.c_r, v.c_a, v.c_s,
    ] {
        o.f(x);
    }
    let mut nz = 0i64;
    for f in 0..m.nframes {
        for s in 0..v.depth {
            for r in 0..v.height {
                for c in 0..v.width {
                    let x = mri_get_vox_val(g, err, m, c, r, s, f, 0).unwrap_or(0.0);
                    nz += i64::from(x != 0.0);
                }
            }
        }
    }
    o.i(nz);
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in m.chunk.as_deref().unwrap_or(&[]) {
        h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    o.s(&format!("{h:016x}"));
}

/// `flr fil <surface> <volume>`, `flr rib <inner> <outer> <volume>`, `flr bbf <surface>
/// <resolution>`: the recorded heads' calls (`real.py`), as `fssurfio.cpp`'s.
pub(super) fn flr_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    cx.g.progname = "fssurfio".into();
    // relay 35 of track D: a GIFTI file's `Date` (`flr crv`) the same on both sides
    let env = Environment::from_pairs(vec![
        ("SURFER_SIDEDOOR".to_owned(), "1".to_owned()),
        (
            "FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING".to_owned(),
            "probe date".to_owned(),
        ),
    ]);
    let (mut so, mut se) = (cx.a.clone(), cx.b.clone());
    let mut io = coords::Io {
        out: &mut so,
        err: &mut se,
        env: &env,
    };
    let g = &mut cx.g;
    let sg = &mut cx.sg;
    let op = t.next();
    o.s(op);
    let surf = |g: &mut Globals, sg: &mut SurfGlobals, io: &mut coords::Io, p: &str| {
        readwrite::read(g, sg, io, None, p, true).map(|s| s.expect("the surface"))
    };
    let vol = |g: &mut Globals, io: &mut coords::Io, p: &str| {
        larmorx_freesurfer_io::mriio::mri_read(
            g,
            io.out,
            io.err,
            io.env,
            None,
            p,
            larmorx_freesurfer_io::identify::MRI_VOLUME_TYPE_UNKNOWN,
            true,
            -1,
            -1,
        )
        .map(|v| v.expect("the volume"))
    };
    match op {
        "fil" => {
            let mut s = surf(g, sg, &mut io, t.next())?;
            let v = vol(g, &mut io, t.next())?;
            let mut d =
                larmorx_freesurfer_mri::copy::mri_copy(g, io.out, io.err, &v, None)?.unwrap_new();
            larmorx_freesurfer_mri::mri::mri_copy_header(g, io.out, io.err, &v, Some(&mut d))?;
            mriflood::mris_fill_interior(g, sg, &mut io, &mut s, 0.0, Some(&mut d))?;
            flr_put(g, io.err, Some(&d), o);
        }
        "rib" => {
            let inner = surf(g, sg, &mut io, t.next())?;
            let outer = surf(g, sg, &mut io, t.next())?;
            let v = vol(g, &mut io, t.next())?;
            let r = mriflood::mris_ribbon(g, sg, &mut io, &inner, &outer, &v, None)?;
            flr_put(g, io.err, r.into_new().as_ref(), o);
        }
        "bbf" => {
            let mut s = surf(g, sg, &mut io, t.next())?;
            let res = t.d();
            let mut b = mriflood::mris_make_bounding_volume(g, &mut io, &s, res)?;
            mriflood::mris_fill_interior(g, sg, &mut io, &mut s, res, Some(&mut b))?;
            flr_put(g, io.err, Some(&b), o);
        }
        "crv" => {
            // relay 35 of track D: `fssurfio.cpp`'s `flr crv` (a recorded curvature file read,
            // written in five formats into a work folder, read back)
            let mut s = surf(g, sg, &mut io, t.next())?;
            let cf = t.next().to_owned();
            let td = tempfile::tempdir().expect("a work folder");
            let dir = td.path();
            let fnv = |b: &[u8], o: &mut Out| {
                let mut h: u64 = 0xcbf2_9ce4_8422_2325;
                for &c in b {
                    h = (h ^ u64::from(c)).wrapping_mul(0x0000_0100_0000_01b3);
                }
                o.s(&format!("{h:016x}"));
            };
            let curv_fnv = |s: &Mris, o: &mut Out| {
                let mut b = Vec::new();
                for v in 0..s.nvertices.max(0) as usize {
                    b.extend_from_slice(&s.vertex(v).curv.to_bits().to_le_bytes());
                }
                fnv(&b, o);
                o.f(s.max_curv);
                o.f(s.min_curv);
            };
            let r = curvfile::read_curvature_file(g, sg, &mut io, None, &mut s, &cf, None, None)?;
            o.i(i64::from(r));
            curv_fnv(&s, o);
            // five stems: a GIFTI file beside a file of its stem would stop the read-back
            for nm in ["./o", "./m.mgz", "./n.nii.gz", "./g.gii", "./v.vtk"] {
                let r = curvfile::write_curvature(g, &mut io, Some(dir), &mut s, nm, None)?;
                o.i(i64::from(r));
            }
            let mut files = Vec::new();
            list_files(dir, "", &mut files);
            files.sort();
            let slurp = |n: &str| std::fs::read(dir.join(n)).unwrap_or_default();
            for n in &files {
                o.h(Some(n.as_bytes()));
                let b = slurp(n);
                if n.ends_with(".gii") {
                    fnv(&gii_normal(&b), o);
                } else {
                    fnv(&b, o);
                }
            }
            let written = files
                .iter()
                .filter(|n| n.ends_with(".o"))
                .map(|n| slurp(n))
                .next_back()
                .unwrap_or_default();
            o.i(i64::from(written == std::fs::read(&cf).unwrap_or_default()));
            for n in &files {
                if !n.ends_with(".o") && !n.contains(".mgz") {
                    continue;
                }
                for v in 0..s.nvertices.max(0) as usize {
                    s.vertex_mut(v).curv = 0.0;
                }
                let r = curvfile::read_curvature_file(
                    g,
                    sg,
                    &mut io,
                    Some(dir),
                    &mut s,
                    &format!("./{n}"),
                    None,
                    None,
                )?;
                o.i(i64::from(r));
                curv_fnv(&s, o);
            }
        }
        "pvw" => {
            // relay 36 of track D: `fssurfio.cpp`'s `flr pvw` (a recorded per-vertex file read as
            // values; the patch, the normals, the values into a volume; read back)
            let mut s = surf(g, sg, &mut io, t.next())?;
            let cf = t.next().to_owned();
            let td = tempfile::tempdir().expect("a work folder");
            let dir = td.path();
            let fnv = |b: &[u8], o: &mut Out| {
                let mut h: u64 = 0xcbf2_9ce4_8422_2325;
                for &c in b {
                    h = (h ^ u64::from(c)).wrapping_mul(0x0000_0100_0000_01b3);
                }
                o.s(&format!("{h:016x}"));
            };
            let field_fnv = |s: &Mris, normals: bool, o: &mut Out| {
                let mut b = Vec::new();
                for v in 0..s.nvertices.max(0) as usize {
                    let x = s.vertex(v);
                    if normals {
                        for f in [x.nx, x.ny, x.nz] {
                            b.extend_from_slice(&f.to_bits().to_le_bytes());
                        }
                    } else {
                        b.extend_from_slice(&x.val.to_bits().to_le_bytes());
                    }
                }
                fnv(&b, o);
            };
            let r = valfile::read_values(g, sg, &mut io, None, &mut s, &cf)?;
            o.i(i64::from(r));
            field_fnv(&s, false, o);
            o.i(i64::from(valfile::write_patch(
                g,
                &mut io,
                Some(dir),
                &mut s,
                "./p.patch",
            )));
            let r = valfile::write_normals(g, &mut io, Some(dir), &s, "./n.mgz")?;
            o.i(i64::from(r));
            let mut given = larmorx_freesurfer_mri::mri::mri_alloc(
                g,
                io.out,
                io.err,
                s.nvertices,
                1,
                1,
                larmorx_freesurfer_mri::mri::MRI_FLOAT,
            )?;
            let r =
                valfile::write_into_volume(g, &mut io, &s, Some(&mut given), valfile::VERTEX_VALS)?;
            o.i(i64::from(matches!(r, valfile::IntoVolume::Given)));
            let mut b = Vec::new();
            for v in 0..s.nvertices {
                let x = larmorx_freesurfer_mri::voxel::mri_get_vox_val(
                    g, io.err, &given, v, 0, 0, 0, 0,
                )?;
                b.extend_from_slice(&x.to_bits().to_le_bytes());
            }
            fnv(&b, o);
            for v in 0..s.nvertices.max(0) as usize {
                let x = s.vertex_mut(v);
                (x.nx, x.ny, x.nz) = (0.0, 0.0, 0.0);
            }
            o.i(i64::from(valfile::read_normals(
                g,
                &mut io,
                Some(dir),
                &mut s,
                "./n.mgz",
            )?));
            field_fnv(&s, true, o);
            let r = valfile::read_values(g, sg, &mut io, Some(dir), &mut s, "./n.mgz")?;
            o.i(i64::from(r));
            field_fnv(&s, false, o);
            let mut files = Vec::new();
            list_files(dir, "", &mut files);
            files.sort();
            for n in &files {
                o.h(Some(n.as_bytes()));
                fnv(&std::fs::read(dir.join(n)).unwrap_or_default(), o);
            }
        }
        "vps" => {
            // relay 36 of track D: `fssurfio.cpp`'s `flr vps` (a recorded surface's other
            // files' positions: canonical, original properties, pial, white, current)
            let sn = t.next().to_owned();
            let mut s = surf(g, sg, &mut io, &sn)?;
            let (sph, swm, pial) = (t.next(), t.next(), t.next());
            o.i(i64::from(vpos::read_canonical_coordinates(
                g, sg, &mut io, None, &mut s, sph,
            )?));
            let r = vpos::read_original_properties(g, sg, &mut io, None, &mut s, Some(swm))?;
            o.i(i64::from(r));
            let r = vpos::read_pial_coordinates(g, sg, &mut io, None, &mut s, Some(pial))?;
            o.i(i64::from(r));
            let r = vpos::read_white_coordinates(g, sg, &mut io, None, &mut s, Some(&sn))?;
            o.i(i64::from(r));
            o.i(i64::from(vpos::read_vertex_positions(
                g, sg, &mut io, None, &mut s, pial,
            )?));
            for set in 0..8 {
                let mut h: u64 = 0xcbf2_9ce4_8422_2325;
                for v in 0..s.nvertices.max(0) as usize {
                    let x = s.vertex(v);
                    let f = match set {
                        0 => [x.x, x.y, x.z],
                        1 => [x.cx, x.cy, x.cz],
                        2 => [x.origx, x.origy, x.origz],
                        3 => [x.pialx, x.pialy, x.pialz],
                        4 => [x.whitex, x.whitey, x.whitez],
                        5 => [x.theta, x.phi, 0.0],
                        6 => [x.area, x.origarea, 0.0],
                        _ => [x.nx, x.ny, x.nz],
                    };
                    for c in f {
                        for b in c.to_bits().to_le_bytes() {
                            h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
                        }
                    }
                }
                o.s(&format!("{h:016x}"));
            }
            o.i(i64::from(s.status.0));
            o.f(s.total_area);
            o.f(s.orig_area);
            o.d(s.radius);
        }
        _ => panic!("flr op {op}"),
    }
    Ok(())
}
