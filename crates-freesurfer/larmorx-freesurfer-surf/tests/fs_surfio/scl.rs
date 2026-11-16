// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The probe's command of relay 38 of track D: `scl`, `surfcluster.cpp` whole (`sclust.rs`): a
//! surface read by `MRISread` (`d/lh.s0`: open grids, subdivided icosahedra, srd's small meshes),
//! every vertex's value from the case, the group-average areas and their flags, now and then the
//! total area and preset cluster numbers, `Gdiag_no`, `FS_CLUSTER_USE_AVG_VERTEX_AREA` and
//! `USE_FAST_SURF_SMOOTHER` (and `FREESURFER_SEED`, for the colour table's random colours); then 1 to 5 operations on the case's current cluster list:
//! `sclustMapSurfClusters` (`map`, with a transform of 4 x 4 or 3 x 4 and a FWHM map now and then),
//! `sclustGrowSurfCluster` (`grow`), `sclustSurfaceArea` (`area`), `sclustWeight` (`wt`),
//! `sclustSurfaceMax` (`max`), `sclustSurfaceCentroid` (`cen`), the zeroing and value setters
//! (`zero`, `zeronon`, `tocn`, `cwp`), `sclustCountClusters` (`count`), the two summaries (`sum`,
//! `sumold`), `DumpSurfClusterSum` (`dump`), `SortSurfClusterSum` (`sort`), both renumberings
//! (`remap`, `remap0`), the maxima (`maxa`, `maxc`, `maxw`), p-values set by hand (`pv`),
//! `sclustPruneByCWPval` (`prune`), `sclustAnnot` (`annot`), `sclustGrowByDist` (`gbd`) and
//! `sclustSaveAsPointSet` (`pts`); `G` where FreeSurfer would read or write outside its arrays
//! (FS-903, FS-905); after each the vertices' cluster numbers, values and annotations, the list
//! and `errno`: the generator and the port's answers, as `fssurfio.cpp`'s.

use super::fli::{fv, surface};
use super::mpd::{grid, ico};
use super::srd::mesh;
use super::*;

use larmorx_freesurfer_core::matrix::{MATRIX_REAL, matrix_alloc};
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, Mri, mri_alloc_sequence};
use larmorx_freesurfer_mri::voxel::mri_set_vox_val;
use larmorx_freesurfer_surf::readwrite;
use larmorx_freesurfer_surf::sclust::{self, SurfClusterSum};

fn dv(t: &mut Vec<String>, x: f64) {
    t.push(format!("{:016x}", x.to_bits()));
}

/// A vertex value: the thresholds' neighbourhood mostly, special floats in heavy cases.
fn val(g: &mut Gen, heavy: bool) -> f32 {
    if heavy && g.chance(8) {
        return g.pick(&[
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            -0.0,
            f32::from_bits(0x7fa0_0001),
        ]);
    }
    g.pick(&[
        0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0, 1.5, 2.0, 2.5, 3.0, -0.5, -1.0, -1.5, -2.0, -3.0,
    ])
}

fn small(g: &mut Gen) -> f32 {
    g.int(-16, 16) as f32 / 4.0
}

fn thresholds(t: &mut Vec<String>, g: &mut Gen) {
    fv(t, g.pick(&[0.5, 0.5, 1.0, 0.0, 1.5, -1.0]));
    fv(t, g.pick(&[-1.0, -1.0, 2.0, 0.0, 3.0, 1.5]));
    ix(t, g.pick(&[0, 1, 1, -1, 2]));
}

/// A transform: none (0), 4 x 4 (1) or 3 x 4 (2), its elements.
fn xfm(t: &mut Vec<String>, g: &mut Gen) {
    let kind = g.pick(&[0, 0, 1, 1, 2]);
    ix(t, kind);
    let rows = match kind {
        0 => 0,
        1 => 4,
        _ => 3,
    };
    for r in 0..rows {
        for c in 0..4 {
            fv(t, if r == c && g.chance(2) { 1.0 } else { small(g) });
        }
    }
}

/// A map of one float per vertex, or none.
fn map(t: &mut Vec<String>, g: &mut Gen, nv: i64, heavy: bool) {
    let on = g.chance(3);
    ix(t, i64::from(on));
    if on {
        for _ in 0..nv {
            let x = if heavy && g.chance(10) {
                g.pick(&[f32::NAN, -1.0, f32::INFINITY])
            } else {
                g.pick(&[0.0, 1.0, 2.0, 4.0, 0.5, 3.0])
            };
            fv(t, x);
        }
    }
}

pub(super) fn scl_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(5);
    let m = match g.below(10) {
        0..=2 => {
            let (w, h) = (g.int(2, 6), g.int(2, 5));
            grid(g, w, h)
        }
        3 => mesh(g, heavy),
        4 => ico(g, 2, 10.0),
        _ => {
            let r = g.pick(&[1.0, 10.0, 100.0]);
            ico(g, 1, r)
        }
    };
    surface(t, &m);
    let nv = m.v.len() as i64;
    ix(t, g.pick(&[-1, -1, 0, 1, 2]));
    t.push(g.pick(&["-", "-", "-", "1", "0", "x", "2"]).into());
    t.push(g.pick(&["-", "-", "0", "1"]).into());
    for _ in 0..nv {
        fv(t, val(g, heavy));
    }
    ix(t, i64::from(g.chance(4)));
    fv(t, g.pick(&[0.0, 0.0, 100.0, 1000.0, -5.0]));
    let total = g.chance(5);
    ix(t, i64::from(total));
    if total {
        fv(t, g.pick(&[0.0, 50.0, 500.0, -1.0]));
    }
    let gav = g.chance(3);
    ix(t, i64::from(gav));
    if gav {
        for _ in 0..nv {
            fv(t, g.pick(&[0.5, 1.0, 2.0, 0.0]));
        }
    }
    let undef = g.chance(6);
    ix(t, i64::from(undef));
    if undef {
        for _ in 0..nv {
            ix(t, g.pick(&[0, 0, 1, 2, 3, -1]));
        }
    }
    let nops = g.int(1, 5);
    ix(t, nops);
    for _ in 0..nops {
        let mut k = g.below(26);
        if nv == 0 && matches!(k, 2 | 3 | 22 | 23) {
            k = 0;
        }
        match k {
            0 | 1 => {
                t.push("map".into());
                thresholds(t, g);
                fv(t, g.pick(&[0.0, 0.0, 0.0, 0.5, 2.0, 10.0, 50.0]));
                xfm(t, g);
                map(t, g, nv, heavy);
            }
            2 | 3 => {
                t.push("grow".into());
                ix(t, g.pick(&[1, 1, 2, 5, 0, -2]));
                ix(t, g.int(0, nv - 1));
                thresholds(t, g);
            }
            4 => {
                t.push("area".into());
                ix(t, g.int(-1, 4));
            }
            5 => {
                t.push("wt".into());
                ix(t, g.int(0, 3));
                map(t, g, nv, heavy);
                ix(t, g.int(0, 1));
            }
            6 => {
                t.push("max".into());
                ix(t, g.int(0, 4));
            }
            7 => {
                t.push("cen".into());
                ix(t, g.int(0, 4));
            }
            8 => {
                t.push("zero".into());
                ix(t, g.int(0, 3));
            }
            9 => t.push("zeronon".into()),
            10 => t.push("tocn".into()),
            11 => t.push("cwp".into()),
            12 => t.push("count".into()),
            13 | 14 => {
                t.push("sum".into());
                xfm(t, g);
                map(t, g, nv, heavy);
            }
            15 => {
                t.push("sumold".into());
                xfm(t, g);
            }
            16 => t.push("dump".into()),
            17 => t.push("sort".into()),
            18 => t.push(g.pick(&["remap", "remap", "remap0"]).into()),
            19 => {
                let w = g.pick(&["maxa", "maxc", "maxw"]);
                t.push(w.into());
                if w == "maxw" {
                    ix(t, g.pick(&[0, 1, -1, 2]));
                }
            }
            20 => {
                t.push("pv".into());
                for _ in 0..6 {
                    let p = if heavy && g.chance(6) {
                        g.pick(&[f64::NAN, -0.5, 0.0, -0.0])
                    } else {
                        g.pick(&[0.0, 0.01, 0.05, 0.1, 0.5, 1.0, 1e-30, 0.002])
                    };
                    dv(t, p);
                }
            }
            21 => {
                t.push("prune".into());
                dv(t, g.pick(&[0.05, 0.1, 0.5, 0.0, 1.0]));
            }
            22 => {
                t.push("gbd".into());
                ix(t, g.int(0, nv - 1));
                dv(t, g.pick(&[0.5, 1.0, 2.0, 5.0, 20.0, 1000.0]));
                ix(t, g.pick(&[0, 1, 1, 2]));
                ix(t, if g.chance(5) { g.int(0, nv - 1) } else { -1 });
            }
            23 => {
                t.push("annot".into());
                ix(t, g.int(0, 4));
            }
            _ => {
                t.push("pts".into());
                t.push(g.pick(&["p.txt", "d/p.txt", "nodir/p.txt"]).into());
            }
        }
    }
}

fn dump_list(l: &[SurfClusterSum], n: i32, o: &mut Out) {
    o.i(i64::from(n));
    for c in l.iter().take(n.max(0) as usize) {
        o.i(i64::from(c.clusterno));
        o.i(i64::from(c.nmembers));
        for x in [c.area, c.weightvtx, c.weightarea, c.maxval] {
            o.f(x);
        }
        o.i(i64::from(c.vtxmaxval));
        for x in [
            c.x, c.y, c.z, c.xxfm, c.yxfm, c.zxfm, c.cx, c.cy, c.cz, c.cxxfm, c.cyxfm, c.czxfm,
        ] {
            o.f(x);
        }
        for x in [
            c.pval_clusterwise,
            c.pval_clusterwise_low,
            c.pval_clusterwise_hi,
        ] {
            o.d(x);
        }
    }
}

fn read_xfm(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    t: &mut Toks,
) -> Result<Option<Matrix>, Fatal> {
    let rows = match t.i() {
        0 => return Ok(None),
        1 => 4,
        _ => 3,
    };
    let mut m = matrix_alloc(g, out, err, rows, 4, MATRIX_REAL)?.expect("a matrix");
    for r in 1..=rows {
        for c in 1..=4 {
            m.set_relt(r, c, t.f());
        }
    }
    Ok(Some(m))
}

fn read_map(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    t: &mut Toks,
    nv: i32,
) -> Result<Option<Mri>, Fatal> {
    if t.i() == 0 {
        return Ok(None);
    }
    let mut m = mri_alloc_sequence(g, out, err, nv, 1, 1, MRI_FLOAT, 1)?;
    for v in 0..nv {
        let x = t.f();
        mri_set_vox_val(g, err, &mut m, v, 0, 0, 0, x, 0)?;
    }
    Ok(Some(m))
}

pub(super) fn scl_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let td = tempfile::tempdir().expect("a work folder");
    let dir = td.path();
    std::fs::create_dir(dir.join("d")).expect("the folder d");
    cx.g.progname = "fssurfio".into();
    let bytes = t.h();
    let (nv, nf) = (t.i(), t.i());
    let gdiag_no = t.i();
    let (avg, ufss) = (t.next(), t.next());
    let mut pairs = vec![
        ("SURFER_SIDEDOOR".to_owned(), "1".to_owned()),
        ("FREESURFER_SEED".to_owned(), "1234".to_owned()),
    ];
    if avg != "-" {
        pairs.push(("FS_CLUSTER_USE_AVG_VERTEX_AREA".to_owned(), avg.to_owned()));
    }
    if ufss != "-" {
        pairs.push(("USE_FAST_SURF_SMOOTHER".to_owned(), ufss.to_owned()));
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
    g.gdiag_no = -1;
    let Some(mut s) = readwrite::read(g, sg, &mut io, d, "d/lh.s0", true)? else {
        o.s("NULL");
        return Ok(());
    };
    g.gdiag_no = gdiag_no;
    if nv != s.nvertices || nf != s.nfaces {
        o.i(i64::from(s.nvertices));
        o.i(i64::from(s.nfaces));
        o.s("COUNTS");
        return Ok(());
    }
    for v in 0..nv as usize {
        s.vertex_mut(v).val = t.f();
    }
    s.group_avg_vtxarea_loaded = t.i();
    s.group_avg_surface_area = t.f();
    if t.i() != 0 {
        s.total_area = t.f();
    }
    if t.i() != 0 {
        for v in 0..nv as usize {
            s.vertex_mut(v).group_avg_area = t.f();
        }
    }
    if t.i() != 0 {
        for v in 0..nv as usize {
            s.vertex_mut(v).undefval = t.i();
        }
    }
    let mut list: Vec<SurfClusterSum> = Vec::new();
    let mut nlist = 0i32;
    let nops = t.i();
    for _ in 0..nops {
        let op = t.next();
        o.s(op);
        g.errno = 0;
        let (mut outside, mut beyond) = (false, false);
        for v in 0..nv as usize {
            let c = s.vertex(v).undefval;
            outside |= c < 0;
            beyond |= c > nlist;
        }
        let nanp = list
            .iter()
            .take(nlist as usize)
            .any(|c| c.pval_clusterwise.is_nan());
        match op {
            "map" => {
                let (thmin, thmax, thsign, minarea) = (t.f(), t.f(), t.i(), t.f());
                let tm = read_xfm(g, io.out, io.err, t)?;
                let fw = read_map(g, io.out, io.err, t, nv)?;
                let (r, n) = sclust::map_surf_clusters(
                    g,
                    &mut io,
                    &mut s,
                    thmin,
                    thmax,
                    thsign,
                    minarea,
                    tm.as_ref(),
                    fw.as_ref(),
                )?;
                o.i(i64::from(n));
                match r {
                    Some(r) => {
                        list = r;
                        nlist = n;
                    }
                    None => o.s("NULL"),
                }
            }
            "grow" => {
                let (c, seed) = (t.i(), t.i());
                let (thmin, thmax, thsign) = (t.f(), t.f(), t.i());
                o.i(i64::from(sclust::grow_surf_cluster(
                    g, &mut io, c, seed, &mut s, thmin, thmax, thsign,
                )));
            }
            "area" => {
                let (a, n) = sclust::surface_area(t.i(), &s);
                o.f(a);
                o.i(i64::from(n));
            }
            "wt" => {
                let c = t.i();
                let m = read_map(g, io.out, io.err, t, nv)?;
                let ua = t.i() != 0;
                o.f(sclust::weight(g, io.err, c, &s, m.as_ref(), ua)?);
            }
            "max" => {
                let mut vm = -7;
                o.f(sclust::surface_max(t.i(), &s, &mut vm));
                o.i(i64::from(vm));
            }
            "cen" => {
                let c = sclust::surface_centroid(t.i(), &s);
                o.i(0);
                for x in c {
                    o.d(x);
                }
            }
            "zero" => o.f(sclust::zero_surface_cluster_no(t.i(), &mut s)),
            "zeronon" => o.f(sclust::zero_surface_non_clusters(&mut s)),
            "tocn" => o.f(sclust::set_surface_val_to_cluster_no(&mut s)),
            "cwp" => {
                if outside || beyond {
                    o.s("G");
                    continue;
                }
                o.f(sclust::set_surface_val_to_cwp(
                    g,
                    &mut s,
                    &list[..nlist as usize],
                ));
            }
            "count" => o.f(sclust::count_clusters(&s)),
            "sum" | "sumold" => {
                let tm = read_xfm(g, io.out, io.err, t)?;
                let fw = if op == "sum" {
                    read_map(g, io.out, io.err, t, nv)?
                } else {
                    None
                };
                if outside {
                    o.s("G");
                    continue;
                }
                let (r, n) = if op == "sum" {
                    sclust::surf_cluster_summary(g, &mut io, &s, tm.as_ref(), fw.as_ref())?
                } else {
                    sclust::surf_cluster_summary_old(g, &mut io, &s, tm.as_ref())?
                };
                o.i(i64::from(n));
                match r {
                    Some(r) => {
                        list = r;
                        nlist = n;
                    }
                    None => o.s("NULL"),
                }
            }
            "dump" => o.i(i64::from(sclust::dump_surf_cluster_sum(
                io.out, &list, nlist,
            ))),
            "sort" => list = sclust::sort_surf_cluster_sum(&list, nlist),
            "remap" | "remap0" => {
                let bad = list
                    .iter()
                    .take(nlist as usize)
                    .any(|c| c.clusterno < 1 || c.clusterno > nlist);
                if op == "remap" && (bad || beyond) {
                    o.s("G");
                    continue;
                }
                let r = if op == "remap" {
                    sclust::remap(g, &mut s, nlist, &mut list)
                } else {
                    sclust::remap0(g, &mut io, &mut s, nlist, &list)
                };
                o.i(i64::from(r));
            }
            "maxa" => o.d(sclust::max_cluster_area(&list, nlist)),
            "maxc" => o.i(i64::from(sclust::max_cluster_count(&list, nlist))),
            "maxw" => o.f(sclust::max_cluster_weight_vtx(&list, nlist, t.i())),
            "pv" => {
                for c in 0..6 {
                    let p = t.d();
                    if c < nlist as usize {
                        list[c].pval_clusterwise = p;
                    }
                }
            }
            "prune" => {
                let th = t.d();
                if outside || beyond || nanp {
                    o.s("G");
                    continue;
                }
                let (r, n) = sclust::prune_by_cw_pval(g, &list, nlist, th, &mut s);
                o.i(i64::from(n));
                list = r;
                nlist = n;
            }
            "annot" => {
                let n = t.i();
                if outside {
                    o.s("G");
                    continue;
                }
                o.i(i64::from(sclust::annot(g, &mut io, &mut s, n)?));
                let ct = s.ct.as_ref().expect("a colour table");
                o.i(i64::from(ct.nentries()));
                for e in &ct.entries {
                    let e = e.as_ref().expect("an entry");
                    o.h(Some(&e.name));
                    for x in [e.ri, e.gi, e.bi, e.ai] {
                        o.i(i64::from(x));
                    }
                }
            }
            "gbd" => {
                let (seed, dd, shape, vtxno) = (t.i(), t.d(), t.i(), t.i());
                let mut l = vec![-2i32; (nv as usize).max(1)];
                let r = sclust::grow_by_dist(
                    g,
                    sg,
                    &mut io,
                    &mut s,
                    seed,
                    dd,
                    shape,
                    vtxno,
                    Some(&mut l),
                );
                o.i(i64::from(r));
                for x in l.iter().take(nv as usize) {
                    o.i(i64::from(*x));
                }
            }
            "pts" => {
                let name = t.next();
                o.i(i64::from(sclust::save_as_point_set(
                    g, &mut io, d, name, &list, nlist, &s,
                )?));
                match std::fs::read(dir.join(name)) {
                    Ok(b) => {
                        let mut x = String::from("x");
                        for c in b {
                            x += &format!("{c:02x}");
                        }
                        o.s(&x);
                    }
                    Err(_) => o.s("NOFILE"),
                }
            }
            _ => panic!("scl op {op}"),
        }
        for v in 0..nv as usize {
            let x = s.vertex(v);
            o.i(i64::from(x.undefval));
            o.f(x.val);
            o.i(i64::from(x.annotation));
        }
        dump_list(&list, nlist, o);
        o.i(i64::from(g.errno));
    }
    Ok(())
}
