// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's second command of relay 42 of track D: `sbl`, the soap bubbles of
//! `mrisurf_deform.cpp` (`soap.rs`): a surface read by `MRISread` (`d/lh.s0`: vls's open grids,
//! subdivided icosahedra and srd's small meshes), now and then at 2 hops; every vertex's rip flag,
//! mark, current, original and target positions from the case (small offsets from the position
//! read, now and then NaN); `Gdiag`, `Gdiag_no`; then 1 to 3 of the four bubbles with a pass count,
//! `freedists` and `marks` (the case's marks set again); after each every vertex's positions,
//! `tdx tdy tdz`, mark and `marked3`: the generator and the port's answers, as `fssurfio.cpp`'s.

use super::fli::{fv, surface};
use super::mpd::{grid, ico};
use super::srd::mesh;
use super::*;

use larmorx_freesurfer_surf::soap::{self, Positions};
use larmorx_freesurfer_surf::{dist, nbhd, readwrite};

fn offset(g: &mut Gen, heavy: bool) -> f32 {
    if heavy && g.chance(30) {
        f32::NAN
    } else if g.chance(3) {
        0.0
    } else {
        g.int(-16, 16) as f32 / 32.0
    }
}

pub(super) fn sbl_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(5);
    let m = match g.below(10) {
        0..=3 => {
            let (w, h) = (g.int(2, 6), g.int(2, 5));
            grid(g, w, h)
        }
        4 | 5 => mesh(g, heavy),
        6 if g.chance(3) => ico(g, 2, 10.0),
        _ => ico(g, 1, 10.0),
    };
    surface(t, &m);
    let nv = m.v.len() as i64;
    ix(t, if g.chance(4) { 2 } else { 1 });
    let rips = g.chance(3);
    // fixed vertices (mark 1) mostly, the rest to compute; now and then marks of 2 or 3
    let fixed = g.pick(&[2, 3, 4, 8]);
    for _ in 0..nv {
        ix(t, i64::from(rips && g.chance(8)));
        ix(
            t,
            if g.chance(12) {
                g.pick(&[2, 3, -1])
            } else if g.chance(fixed) {
                0
            } else {
                1
            },
        );
        for _ in 0..9 {
            fv(t, offset(g, heavy));
        }
    }
    ix(t, g.pick(&[0, 0, 0, 0x40, 0x1000_0040, 0x1000_0000]));
    ix(
        t,
        if nv > 0 && g.chance(6) {
            g.int(0, nv - 1)
        } else if heavy && g.chance(10) {
            nv + 3
        } else {
            -1
        },
    );
    let nops = g.int(1, 3);
    ix(t, nops);
    for _ in 0..nops {
        match g.below(10) {
            0..=3 => t.push("MRISsoapBubbleVertexPositions".into()),
            4 | 5 => t.push("MRISweightedSoapBubbleVertexPositions".into()),
            6 => t.push("MRISsoapBubbleOrigVertexPositions".into()),
            7 => t.push("MRISsoapBubbleTargetVertexPositions".into()),
            8 => {
                t.push(g.pick(&["freedists", "marks"]).into());
                continue;
            }
            _ => {
                t.push("marks".into());
                continue;
            }
        }
        ix(t, g.pick(&[0, 1, 2, 3, 5, 10, 25]));
    }
}

fn dump(s: &Mris, o: &mut Out) {
    for vno in 0..s.nvertices.max(0) as usize {
        let x = s.vertex(vno);
        for f in [
            x.x, x.y, x.z, x.origx, x.origy, x.origz, x.targx, x.targy, x.targz, x.tdx, x.tdy,
            x.tdz,
        ] {
            o.f(f);
        }
        o.i(i64::from(x.marked));
        o.i(i64::from(x.marked3));
    }
}

pub(super) fn sbl_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let td = tempfile::tempdir().expect("a work folder");
    let dir = td.path();
    std::fs::create_dir(dir.join("d")).expect("the folder d");
    cx.g.progname = "fssurfio".into();
    let bytes = t.h();
    let (nv, nf) = (t.i(), t.i());
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
    let nsize = t.i();
    if nsize > 1 {
        nbhd::set_neighborhood_size(g, sg, &mut io, &mut s, nsize)?;
    }
    let mut marks = Vec::with_capacity(nv as usize);
    for vno in 0..nv as usize {
        let rip = t.i() as i8;
        let mark = t.i() as i16;
        marks.push(mark);
        let off: Vec<f32> = (0..9).map(|_| t.f()).collect();
        let v = s.vertex_mut(vno);
        v.ripflag = rip;
        v.marked = mark;
        v.x += off[0];
        v.y += off[1];
        v.z += off[2];
        v.origx = v.x + off[3];
        v.origy = v.y + off[4];
        v.origz = v.z + off[5];
        v.targx = v.x + off[6];
        v.targy = v.y + off[7];
        v.targz = v.z + off[8];
    }
    g.gdiag = t.i() as u32 as u64;
    g.gdiag_no = t.i();
    let nops = t.i();
    for _ in 0..nops {
        let op = t.next();
        o.s(op);
        match op {
            "MRISsoapBubbleVertexPositions" => {
                let n = t.i();
                o.i(i64::from(soap::soap_bubble_vertex_positions(
                    g, &mut io, &mut s, n,
                )));
            }
            "MRISweightedSoapBubbleVertexPositions" => {
                let n = t.i();
                o.i(i64::from(soap::weighted_soap_bubble_vertex_positions(
                    g, &mut io, &mut s, n,
                )?));
            }
            "MRISsoapBubbleOrigVertexPositions" => {
                let n = t.i();
                o.i(i64::from(soap::soap_bubble_positions(
                    g,
                    &mut io,
                    &mut s,
                    n,
                    Positions::Original,
                )?));
            }
            "MRISsoapBubbleTargetVertexPositions" => {
                let n = t.i();
                o.i(i64::from(soap::soap_bubble_positions(
                    g,
                    &mut io,
                    &mut s,
                    n,
                    Positions::Target,
                )?));
            }
            "freedists" => {
                dist::free_dists_but_not_orig(&mut s);
                dist::free_dist_origs(&mut s);
            }
            "marks" => {
                for (vno, &m) in marks.iter().enumerate() {
                    s.vertex_mut(vno).marked = m;
                }
            }
            _ => panic!("sbl op {op}"),
        }
        dump(&s, o);
    }
    Ok(())
}
