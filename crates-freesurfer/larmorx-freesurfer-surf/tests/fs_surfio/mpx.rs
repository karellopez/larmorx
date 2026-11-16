// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of relay 37 of track D: `mpx`, group (e) of the READY list (`props11.rs`):
//! a surface read by `MRISread` (`d/lh.s0`: subdivided icosahedra, open grids, srd's small meshes)
//! with some vertices ripped, its radius, canonical positions given or not, per-vertex overrides
//! of the current, original, white positions, normals and tangent axes, `Gdiag`'s write and
//! verbose bits now and then, the surface's volume geometry and one volume of the case, a
//! Talairach transform and its matrix; then 1 to 4 operations: the move to Talairach space in
//! place or into a clone (`tal`, `MRIStalairachTransform`), one of the seven vertex-to-voxel
//! conversions (`v2v`), the normal-dot distribution (`ndd`), the principal-curvature
//! distributions (`pcd`, a volume FreeSurfer would read outside written `G` on both sides) and
//! Ashburner's sampled triangle energy (`ash`); at the end, with `Gdiag`'s bits, the work
//! folder's files by name, length and FNV-1a hash: the generator and the port's answers, as
//! `fssurfio.cpp`'s.

use super::fli::{fv, geom, surface};
use super::mpd::{grid, ico};
use super::srd::mesh;
use super::*;

use larmorx_freesurfer_core::diag::{DIAG_VERBOSE, DIAG_WRITE};
use larmorx_freesurfer_core::histo::histo_alloc;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_UCHAR, mri_alloc};
use larmorx_freesurfer_mri::vol_geom::reinit_cache;
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;
use larmorx_freesurfer_surf::{lta, props11, readwrite};

fn small(t: &mut Vec<String>, g: &mut Gen, heavy: bool) {
    if heavy && g.chance(6) {
        t.push(format!("{:08x}", smallish(g, heavy)));
    } else {
        t.push(format!("{:08x}", (g.int(-16, 16) as f32 / 8.0).to_bits()));
    }
}

/// FNV-1a (64 bits) of `b`.
pub(super) fn fnv64(b: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &c in b {
        h ^= u64::from(c);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

pub(super) fn mpx_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(5);
    let r = g.pick(&[1.0f32, 10.0, 100.0]);
    let m = match g.below(10) {
        0 | 1 => {
            let (w, h) = (g.int(2, 5), g.int(2, 5));
            grid(g, w, h)
        }
        2 | 3 => mesh(g, heavy),
        4 => ico(g, 2, r),
        _ => ico(g, 1, r),
    };
    surface(t, &m);
    let nv = m.v.len() as i64;
    let nrip = if g.chance(4) && nv > 0 {
        g.int(1, 3)
    } else {
        0
    };
    ix(t, nrip);
    for _ in 0..nrip {
        ix(t, g.int(0, nv - 1));
    }
    let radius: f64 = if heavy && g.chance(8) {
        f64::from(f32::from_bits(g.pick(&[
            0x7fc0_0000,
            0x0000_0000,
            0x7f80_0000,
        ])))
    } else {
        f64::from(g.pick(&[r, r, 1.0, 50.0, 0.5]))
    };
    t.push(format!("{:016x}", radius.to_bits()));
    // the canonical positions: none (zeros) or each vertex's scaled, now and then disturbed
    let canon = !g.chance(4);
    ix(t, i64::from(canon));
    if canon {
        let s = g.pick(&[1.0f32, 0.5, 2.0, 1.0]);
        for p in &m.v {
            for &c in p {
                let mut x = c * s;
                if g.chance(6) {
                    x += g.int(-4, 4) as f32 / 16.0;
                }
                if heavy && g.chance(40) {
                    x = f32::from_bits(smallish(g, true));
                }
                fv(t, x);
            }
        }
    }
    // per-vertex overrides: p current, o original, w white, n normal, e tangent axes
    let nover = if nv > 0 { g.int(0, 4) } else { 0 };
    ix(t, nover);
    for _ in 0..nover {
        ix(t, g.int(0, nv - 1));
        let k = g.pick(&["p", "o", "w", "n", "e"]);
        t.push(k.to_owned());
        for _ in 0..if k == "e" { 6 } else { 3 } {
            small(t, g, heavy);
        }
    }
    ix(t, i64::from(g.chance(6)));
    // the surface's volume geometry: kept as read, or the case's
    let svg = g.chance(2);
    ix(t, i64::from(svg));
    if svg {
        ix(t, i64::from(!g.chance(5)));
        for _ in 0..3 {
            ix(t, g.int(4, 12));
        }
        geom(t, g, heavy);
    }
    // one volume for the conversions
    ix(t, g.pick(&[MRI_UCHAR, MRI_FLOAT]).into());
    for _ in 0..3 {
        ix(t, g.int(2, 6));
    }
    geom(t, g, heavy);
    // the Talairach transform and its matrix
    let lta = g.chance(3);
    ix(t, i64::from(!lta));
    let mat = !g.chance(5);
    ix(t, i64::from(mat));
    if mat {
        for r in 0..4 {
            for c in 0..4 {
                let x = if r == 3 {
                    if c == 3 { 1.0 } else { 0.0 }
                } else if heavy && g.chance(12) {
                    f32::from_bits(smallish(g, true))
                } else if c == 3 {
                    g.int(-32, 32) as f32 / 4.0
                } else if r == c {
                    g.pick(&[1.0f32, 1.0, 0.5, 1.25, -1.0])
                } else if g.chance(2) {
                    0.0
                } else {
                    g.int(-4, 4) as f32 / 8.0
                };
                fv(t, x);
            }
        }
    }
    let nops = g.int(1, 4);
    ix(t, nops);
    for _ in 0..nops {
        match g.below(12) {
            0 | 1 => {
                t.push("tal".into());
                ix(t, g.int(0, 1));
            }
            2..=4 if nv > 0 => {
                t.push("v2v".into());
                let k = g.int(0, 6);
                ix(t, k);
                ix(t, g.int(0, nv - 1));
                if k == 4 {
                    ix(t, if g.chance(12) { 14 } else { g.int(0, 11) });
                }
            }
            5 | 6 => {
                t.push("ndd".into());
                ix(
                    t,
                    if g.chance(5) {
                        g.pick(&[0, 1, 2, 3, 10, 50])
                    } else {
                        100
                    },
                );
            }
            7 | 8 => {
                t.push("pcd".into());
                for _ in 0..2 {
                    ix(
                        t,
                        if g.chance(6) {
                            g.pick(&[1, 7, 50])
                        } else {
                            100
                        },
                    );
                }
                let (w, h, d, ty) = match g.below(12) {
                    0 => (120, 100, 1, MRI_FLOAT),
                    1 => (100, 130, 2, MRI_FLOAT),
                    2 => (101, 103, 1, MRI_FLOAT),
                    3 => (50, 100, 1, MRI_FLOAT),
                    4 => (100, 100, 1, MRI_UCHAR),
                    _ => (100, 100, 1, MRI_FLOAT),
                };
                for x in [w, h, d, ty] {
                    ix(t, i64::from(x));
                }
            }
            _ if nv > 0 => {
                t.push("ash".into());
                ix(t, g.int(0, nv - 1));
                let lambda = if heavy && g.chance(6) {
                    f32::from_bits(smallish(g, true))
                } else {
                    g.pick(&[1.0f32, 0.5, 2.0, 0.0])
                };
                fv(t, lambda);
                for _ in 0..3 {
                    small(t, g, heavy);
                }
            }
            _ => t.push("ndd".into()),
        }
        if t.last().map(String::as_str) == Some("ndd") {
            ix(t, 100);
        }
    }
}

fn dump_surface(s: &Mris, o: &mut Out) {
    for vno in 0..s.nvertices.max(0) as usize {
        let v = s.vertex(vno);
        for x in [v.x, v.y, v.z] {
            o.f(x);
        }
    }
    for x in [
        s.xlo, s.ylo, s.zlo, s.xhi, s.yhi, s.zhi, s.xctr, s.yctr, s.zctr,
    ] {
        o.f(x);
    }
    o.i(i64::from(s.dist_alloced_flags));
}

fn dump_histo(h: &larmorx_freesurfer_core::histo::Histogram, o: &mut Out) {
    o.i(i64::from(h.nbins));
    o.f(h.bin_size);
    for b in 0..h.nbins.max(0) as usize {
        o.f(h.bins[b]);
        o.f(h.counts[b]);
    }
}

pub(super) fn mpx_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let td = tempfile::tempdir().expect("a work folder");
    let dir = td.path();
    std::fs::create_dir(dir.join("d")).expect("the folder d");
    cx.g.progname = "fssurfio".into();
    let bytes = t.h();
    let (nv, nf) = (t.i(), t.i());
    let rips: Vec<i32> = (0..t.i()).map(|_| t.i()).collect();
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
    for vno in rips {
        s.vertex_mut(vno as usize).ripflag = 1;
    }
    s.radius = t.d();
    if t.i() != 0 {
        for vno in 0..nv as usize {
            let v = s.vertex_mut(vno);
            (v.cx, v.cy, v.cz) = (t.f(), t.f(), t.f());
        }
    }
    for _ in 0..t.i() {
        let vno = t.i() as usize;
        let k = t.next();
        let v = s.vertex_mut(vno);
        match k {
            "p" => (v.x, v.y, v.z) = (t.f(), t.f(), t.f()),
            "o" => (v.origx, v.origy, v.origz) = (t.f(), t.f(), t.f()),
            "w" => (v.whitex, v.whitey, v.whitez) = (t.f(), t.f(), t.f()),
            "n" => (v.nx, v.ny, v.nz) = (t.f(), t.f(), t.f()),
            "e" => {
                (v.e1x, v.e1y, v.e1z) = (t.f(), t.f(), t.f());
                (v.e2x, v.e2y, v.e2z) = (t.f(), t.f(), t.f());
            }
            _ => panic!("mpx override {k}"),
        }
    }
    let gd = t.i() != 0;
    if t.i() != 0 {
        let vg = &mut s.vg;
        vg.valid = t.i();
        (vg.width, vg.height, vg.depth) = (t.i(), t.i(), t.i());
        (vg.xsize, vg.ysize, vg.zsize) = (t.f(), t.f(), t.f());
        (vg.x_r, vg.x_a, vg.x_s) = (t.f(), t.f(), t.f());
        (vg.y_r, vg.y_a, vg.y_s) = (t.f(), t.f(), t.f());
        (vg.z_r, vg.z_a, vg.z_s) = (t.f(), t.f(), t.f());
        (vg.c_r, vg.c_a, vg.c_s) = (t.f(), t.f(), t.f());
    }
    let ty = t.i();
    let (w, h, dd) = (t.i(), t.i(), t.i());
    let mut vol = mri_alloc(g, io.out, io.err, w, h, dd, ty)?;
    {
        let vg = &mut vol.vg;
        (vg.xsize, vg.ysize, vg.zsize) = (t.f(), t.f(), t.f());
        (vg.x_r, vg.x_a, vg.x_s) = (t.f(), t.f(), t.f());
        (vg.y_r, vg.y_a, vg.y_s) = (t.f(), t.f(), t.f());
        (vg.z_r, vg.z_a, vg.z_s) = (t.f(), t.f(), t.f());
        (vg.c_r, vg.c_a, vg.c_s) = (t.f(), t.f(), t.f());
    }
    reinit_cache(g, io.out, io.err, &mut vol.vg)?;
    if t.i() != 0 {
        s.lta = Some(lta::lta_alloc(g, io.out, io.err, 1, None)?);
    }
    if t.i() != 0 {
        let mut m = Matrix::real(4, 4);
        for r in 1..=4 {
            for c in 1..=4 {
                m.set_relt(r, c, t.f());
            }
        }
        s.SRASToTalSRAS_ = Some(m);
    }
    if gd {
        g.gdiag |= DIAG_WRITE | DIAG_VERBOSE;
    }
    let nops = t.i();
    for _ in 0..nops {
        let op = t.next();
        o.s(op);
        match op {
            "tal" => {
                if t.i() == 0 {
                    let r = props11::talairach_transform_in_place(g, &mut io, &mut s)?;
                    if r != 0 {
                        o.s("NULL");
                        continue;
                    }
                    o.i(1);
                    dump_surface(&s, o);
                } else {
                    match props11::talairach_transform(g, sg, &mut io, &s, None)? {
                        Ret::New(c) => {
                            o.i(1);
                            dump_surface(&c, o);
                            o.i(i64::from(c.lta.is_some()));
                        }
                        _ => o.s("NULL"),
                    }
                }
            }
            "v2v" => {
                let (k, vno) = (t.i(), t.i() as usize);
                let v = s.vertex(vno);
                let r = match k {
                    0 => props11::vertex_to_voxel(g, sg, &mut io, &s, v, &vol)?,
                    1 => props11::vertex_to_voxel_not_cached(g, &mut io, &s, v, &vol)?,
                    2 => props11::vertex_normal_to_voxel(g, sg, &mut io, &s, v, &vol)?,
                    3 => {
                        let (code, p) =
                            props11::vertex_normal_to_voxel_scaled(g, sg, &mut io, &s, v, &vol)?;
                        o.i(i64::from(code));
                        p.unwrap_or([-7.0; 3])
                    }
                    4 => {
                        let which = t.i();
                        props11::vertex_coord_to_voxel(g, sg, &mut io, &s, v, &vol, which)?
                    }
                    5 => props11::orig_vertex_to_voxel(g, sg, &mut io, &s, v, &vol)?,
                    _ => props11::white_vertex_to_voxel(g, sg, &mut io, &s, v, &vol)?,
                };
                r.iter().for_each(|&x| o.d(x));
            }
            "ndd" => {
                let n = t.i();
                let mut hd = histo_alloc(g, io.err, n)?;
                let r =
                    props11::compute_normal_dot_distribution(g, sg, &mut io, d, &mut s, &mut hd)?;
                o.i(i64::from(r));
                dump_histo(&hd, o);
                for vno in 0..nv as usize {
                    let v = s.vertex(vno);
                    for x in [v.x, v.y, v.z, v.tx, v.ty, v.tz, v.nx, v.ny, v.nz, v.area] {
                        o.f(x);
                    }
                }
                o.f(s.total_area);
            }
            "pcd" => {
                let (n1, n2) = (t.i(), t.i());
                let (w, h, dd, ty) = (t.i(), t.i(), t.i(), t.i());
                if w < 100 || h < 100 || ty != MRI_FLOAT {
                    o.s("G");
                    continue;
                }
                let mut h1 = histo_alloc(g, io.err, n1)?;
                let mut h2 = histo_alloc(g, io.err, n2)?;
                let mut m = mri_alloc(g, io.out, io.err, w, h, dd, ty)?;
                let r = props11::compute_principal_curvature_distributions(
                    g, sg, &mut io, d, &mut s, &mut h1, &mut h2, &mut m,
                )?;
                o.i(i64::from(r));
                dump_histo(&h1, o);
                dump_histo(&h2, o);
                for x in [m.vg.xsize, m.vg.ysize, m.xstart, m.ystart] {
                    o.f(x);
                }
                let mut b = Vec::new();
                for z in 0..dd {
                    for y in 0..h {
                        for x in 0..w {
                            let v = mri_get_vox_val(g, io.err, &m, x, y, z, 0, 0)?;
                            b.extend_from_slice(&v.to_le_bytes());
                        }
                    }
                }
                o.s(&format!("{:016x}", fnv64(&b)));
                for vno in 0..nv as usize {
                    let v = s.vertex(vno);
                    o.f(v.k1);
                    o.f(v.k2);
                }
            }
            "ash" => {
                let vno = t.i() as usize;
                let lambda = t.f();
                let (a, b, c) = (t.f(), t.f(), t.f());
                let r = props11::sample_ashburner_triangle_energy(
                    g, &mut io, &mut s, vno, lambda, a, b, c,
                )?;
                o.f(r);
                let v = s.vertex(vno);
                for x in [v.x, v.y, v.z] {
                    o.f(x);
                }
            }
            _ => panic!("mpx op {op}"),
        }
    }
    if gd {
        o.0 += "| ";
        let mut files = Vec::new();
        super::sfile::list_files(dir, "", &mut files);
        files.sort();
        for n in files {
            o.h(Some(n.as_bytes()));
            match std::fs::read(dir.join(&n)) {
                Ok(b) => {
                    o.i(b.len() as i64);
                    o.s(&format!("{:016x}", fnv64(&b)));
                }
                Err(_) => o.s("NOFILE"),
            }
        }
    }
    Ok(())
}
