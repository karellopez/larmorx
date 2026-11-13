// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of relay 23: `mpf`, queue item 3.1-51's leftovers (`props9.rs`): a surface
//! read by `MRISread` (`d/lh.s0`: subdivided icosahedra, open grids, srd's small meshes) with some
//! vertices ripped, volumes of one voxel per vertex (mostly `MRI_FLOAT`, sometimes another type or
//! a mismatched size), then 1 to 3 operations: the vertex-to-voxel table (`lut`), the spatial AR(1)
//! (`ar1`), the autocorrelation by hops (`arn`), the kernel smoothing (`skn`, its hop lists kept
//! between calls), the hop lists (`hop`) and the surface-RAS-to-voxel helper (`sras`), each
//! printing its volume (or `NULL`) and the vertices' `undefval`: the generator and the port's
//! answers, as `fssurfio.cpp`'s.

use super::mpd::{grid, ico};
use super::srd::{be4, mesh};
use super::*;

use larmorx_freesurfer_core::matrix::{MATRIX_REAL, matrix_alloc};
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::{Mri, mri_alloc_sequence};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_surf::props9::{self, KernelOutput, SurfHopList};
use larmorx_freesurfer_surf::readwrite;

/// A voxel value: 0 often (vertices of no variance), `k/8`, or a heavy float now and then.
fn val(t: &mut Vec<String>, g: &mut Gen, heavy: bool) {
    if g.chance(5) {
        t.push(format!("{:08x}", 0f32.to_bits()));
    } else if heavy && g.chance(6) {
        t.push(format!("{:08x}", smallish(g, heavy)));
    } else {
        t.push(format!("{:08x}", (g.int(-16, 16) as f32 / 8.0).to_bits()));
    }
}

pub(super) fn mpf_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(5);
    let r = g.pick(&[1.0, 10.0, 100.0]);
    let m = match g.below(10) {
        0 | 1 => {
            let (w, h) = (g.int(2, 6), g.int(2, 6));
            grid(g, w, h)
        }
        2 | 3 => mesh(g, heavy),
        4 => ico(g, 2, r),
        _ => ico(g, 1, r),
    };
    let mut b = vec![0xff, 0xff, 0xfe];
    b.extend_from_slice(b"created by me on Fri Nov 14 10:55:19 2026\n\n");
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
    hx(t, &b);
    let (nv, nf) = (m.v.len() as i64, m.f.len() as i64);
    ix(t, nv);
    ix(t, nf);
    // ripped vertices
    let nrip = if g.chance(3) && nv > 0 {
        g.int(1, 3)
    } else {
        0
    };
    ix(t, nrip);
    for _ in 0..nrip {
        ix(t, g.int(0, nv - 1));
    }
    // FREESURFER_mriSurfaceRASToVoxel_both (1) and _old (2)
    ix(t, if g.chance(3) { g.int(1, 3) } else { 0 });
    // volumes: (type, width, height, depth, frames)
    let nvol = g.int(1, 3);
    ix(t, nvol);
    let mut dims = Vec::new();
    for _ in 0..nvol {
        let ty = if g.chance(6) { g.pick(&[0, 1, 4]) } else { 3 };
        let n = nv.max(1);
        let (w, h, d) = if g.chance(8) {
            (n + g.pick(&[1, -1]), 1, 1)
        } else {
            match g.below(4) {
                0 => (1, n, 1),
                1 => (1, 1, n),
                2 if n % 2 == 0 => (n / 2, 2, 1),
                _ => (n, 1, 1),
            }
        };
        let fr = g.int(1, 4);
        for x in [ty, w, h, d, fr] {
            ix(t, x);
        }
        for _ in 0..w * h * d * fr {
            val(t, g, heavy);
        }
        dims.push((w * h * d, fr));
    }
    let pick_vol = |g: &mut Gen| g.int(0, nvol - 1);
    let mask = |g: &mut Gen| if g.chance(3) { g.int(0, nvol - 1) } else { -1 };
    // the hop lists' hops an earlier `skn` made (none yet)
    let mut cached: Option<i64> = None;
    let nops = g.int(1, 3);
    ix(t, nops);
    for _ in 0..nops {
        match g.below(12) {
            0 => {
                t.push("lut".into());
                ix(t, pick_vol(g));
            }
            1 | 2 => {
                t.push("ar1".into());
                ix(t, pick_vol(g));
                ix(t, mask(g));
            }
            3 | 4 => {
                t.push("arn".into());
                ix(t, pick_vol(g));
                ix(t, mask(g));
                ix(
                    t,
                    if g.chance(8) {
                        g.pick(&[0, -1])
                    } else {
                        g.int(1, 4)
                    },
                );
            }
            5..=8 => {
                t.push("skn".into());
                let vi = pick_vol(g);
                ix(t, vi);
                ix(t, mask(g));
                // kernel: 0 none, 1 a per-vertex volume, 2 a global matrix, 3 both
                let kind = if g.chance(10) {
                    g.pick(&[0, 3])
                } else {
                    g.int(1, 2)
                };
                ix(t, kind);
                let ki = pick_vol(g);
                ix(t, ki);
                let (rows, cols) = (g.int(1, 4), g.int(1, 2));
                ix(t, rows);
                ix(t, cols);
                for _ in 0..rows * cols {
                    val(t, g, heavy);
                }
                let nhops = match kind {
                    1 => dims[ki as usize].1,
                    2 => rows,
                    _ => 0,
                };
                // reuse the hop lists only when they have the hops this kernel reads (FS-533)
                let reuse = cached.is_some_and(|c| nhops <= c) && g.chance(2);
                ix(t, i64::from(reuse));
                if !reuse {
                    cached = None;
                }
                // output: 0 new, 1 a volume of the case (the input itself refused), 2 the input
                let ok = if g.chance(4) { g.int(1, 2) } else { 0 };
                ix(t, ok);
                ix(t, pick_vol(g));
                let builds = kind != 3 && dims[vi as usize].0 == nv && nhops >= 1;
                if builds && cached.is_none() && ok != 2 {
                    cached = Some(nhops);
                }
            }
            9 | 10 => {
                t.push("hop".into());
                ix(t, if nv > 0 { g.int(0, nv - 1) } else { 0 });
                ix(t, if g.chance(10) { 0 } else { g.int(1, 5) });
            }
            _ => {
                t.push("sras".into());
                if g.chance(6) {
                    ix(t, 0);
                } else {
                    ix(t, 1);
                    let (rows, cols) = if g.chance(2) {
                        (4, 4)
                    } else {
                        (g.int(3, 5), g.pick(&[3, 4, 4, 5]))
                    };
                    ix(t, rows);
                    ix(t, cols);
                    for _ in 0..rows * cols {
                        val(t, g, heavy);
                    }
                }
                let np = g.int(1, 3);
                ix(t, np);
                for _ in 0..3 * np {
                    if g.chance(4) {
                        dx(t, g);
                    } else {
                        t.push(format!("{:016x}", (g.int(-64, 64) as f64 / 4.0).to_bits()));
                    }
                }
            }
        }
    }
}

fn dump_vol(g: &mut Globals, err: &mut dyn Write, m: Option<&Mri>, o: &mut Out) {
    let Some(m) = m else {
        o.s("NULL");
        return;
    };
    o.i(i64::from(m.nframes));
    for f in 0..m.nframes {
        for s in 0..m.vg.depth {
            for r in 0..m.vg.height {
                for c in 0..m.vg.width {
                    let x = mri_get_vox_val(g, err, m, c, r, s, f, 0).unwrap_or(0.0);
                    o.f(x);
                }
            }
        }
    }
}

fn dump_undef(s: &Mris, o: &mut Out) {
    for vno in 0..s.nvertices as usize {
        o.i(i64::from(s.vertex(vno).undefval));
    }
}

pub(super) fn mpf_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let td = tempfile::tempdir().expect("a work folder");
    let dir = td.path();
    std::fs::create_dir(dir.join("d")).expect("the folder d");
    cx.g.progname = "fssurfio".into();
    let bytes = t.h();
    let (nv, nf) = (t.i(), t.i());
    let rips: Vec<i32> = (0..t.i()).map(|_| t.i()).collect();
    let flags = t.i();
    let mut pairs = vec![("SURFER_SIDEDOOR".to_owned(), "1".to_owned())];
    if flags & 1 != 0 {
        pairs.push((
            "FREESURFER_mriSurfaceRASToVoxel_both".to_owned(),
            "1".to_owned(),
        ));
    }
    if flags & 2 != 0 {
        pairs.push((
            "FREESURFER_mriSurfaceRASToVoxel_old".to_owned(),
            "1".to_owned(),
        ));
    }
    let env = Environment::from_pairs(pairs);
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
    let nvol = t.i();
    let mut vols: Vec<Mri> = Vec::new();
    for _ in 0..nvol {
        let (ty, w, h, dd, fr) = (t.i(), t.i(), t.i(), t.i(), t.i());
        let mut v = mri_alloc_sequence(g, io.out, io.err, w, h, dd, ty, fr)?;
        for f in 0..fr {
            for z in 0..dd {
                for r in 0..h {
                    for c in 0..w {
                        let x = t.f();
                        mri_set_vox_val(g, io.err, &mut v, c, r, z, f, x, 0)?;
                    }
                }
            }
        }
        vols.push(v);
    }
    let mut shl: Option<Vec<SurfHopList>> = None;
    let nops = t.i();
    for _ in 0..nops {
        let op = t.next();
        o.s(op);
        match op {
            "lut" => {
                let vi = t.i() as usize;
                match props9::mris_crs_lut(g, &mut io, &s, &vols[vi]) {
                    None => o.s("NULL"),
                    Some(l) => {
                        for k in 0..l.c.len() {
                            o.i(i64::from(l.c[k]));
                            o.i(i64::from(l.r[k]));
                            o.i(i64::from(l.s[k]));
                        }
                        o.i(i64::from(props9::mris_crs_lut_free(l)));
                    }
                }
            }
            "ar1" => {
                let (vi, mi) = (t.i() as usize, t.i());
                let mask = (mi >= 0).then(|| &vols[mi as usize]);
                let r = props9::mris_ar1(g, &mut io, &s, &vols[vi], mask, None)?;
                dump_vol(g, io.err, r.as_ref(), o);
            }
            "arn" => {
                let (vi, mi, n) = (t.i() as usize, t.i(), t.i());
                let mask = (mi >= 0).then(|| &vols[mi as usize]);
                let r = props9::mris_arn(g, &mut io, &mut s, &vols[vi], mask, None, n)?;
                dump_vol(g, io.err, r.as_ref(), o);
                dump_undef(&s, o);
            }
            "skn" => {
                let (vi, mi, kind, ki) = (t.i() as usize, t.i(), t.i(), t.i() as usize);
                let (rows, cols) = (t.i(), t.i());
                let mut gk =
                    matrix_alloc(g, io.out, io.err, rows, cols, MATRIX_REAL)?.expect("a matrix");
                for rr in 1..=rows {
                    for cc in 1..=cols {
                        gk.set_relt(rr, cc, t.f());
                    }
                }
                let reuse = t.i() != 0;
                let (ok, oi) = (t.i(), t.i() as usize);
                if !reuse {
                    shl = None;
                }
                let mask = (mi >= 0).then(|| &vols[mi as usize]);
                let mrikern = (kind & 1 != 0).then(|| &vols[ki]);
                let globkern = (kind & 2 != 0).then_some(&gk);
                let out = match ok {
                    0 => KernelOutput::New,
                    1 if oi != vi => {
                        // the volume itself, as FreeSurfer passes it (a copy taken here, put back after)
                        let (mut so, mut se) = (Vec::new(), Vec::new());
                        let copy = mri_copy(g, &mut so, &mut se, &vols[oi], None)?
                            .into_new()
                            .expect("a copy");
                        KernelOutput::Given(Box::new(copy))
                    }
                    _ => KernelOutput::Src,
                };
                let r = props9::mris_smooth_kernel(
                    g, &mut io, &mut s, &vols[vi], mask, mrikern, globkern, &mut shl, out,
                )?;
                dump_vol(g, io.err, r.as_ref(), o);
                dump_undef(&s, o);
                if let Some(r) = r
                    && ok == 1
                    && oi != vi
                {
                    vols[oi] = r;
                }
            }
            "hop" => {
                let (vtx, n) = (t.i(), t.i());
                match props9::set_surf_hop_list(g, &mut io, vtx, &mut s, n)? {
                    None => o.s("NULL"),
                    Some(l) => {
                        o.i(i64::from(l.cvtx));
                        o.i(i64::from(l.nhops));
                        for hop in 0..l.vtxlist.len() {
                            o.i(i64::from(l.nperhop(hop)));
                            for &x in &l.vtxlist[hop] {
                                o.i(i64::from(x));
                            }
                        }
                        o.i(i64::from(props9::surf_hop_list_free(l)));
                    }
                }
                dump_undef(&s, o);
            }
            "sras" => {
                sg.voxel_from_sras = None;
                if t.i() != 0 {
                    let (rows, cols) = (t.i(), t.i());
                    let mut m = matrix_alloc(g, io.out, io.err, rows, cols, MATRIX_REAL)?
                        .expect("a matrix");
                    for rr in 1..=rows {
                        for cc in 1..=cols {
                            m.set_relt(rr, cc, t.f());
                        }
                    }
                    sg.voxel_from_sras = Some(m);
                }
                let np = t.i();
                for _ in 0..np {
                    let (x, y, z) = (t.d(), t.d(), t.d());
                    let (ret, v) = props9::mri_surface_ras_to_voxel(g, sg, &mut io, x, y, z)?;
                    o.i(i64::from(ret));
                    match v {
                        Some((a, b, c)) => {
                            o.d(a);
                            o.d(b);
                            o.d(c);
                        }
                        None => o.s("UNSET"),
                    }
                }
            }
            _ => panic!("mpf op {op}"),
        }
    }
    Ok(())
}
