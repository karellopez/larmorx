// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs-gcam probe's cases (`validation/probes/fs-gcam/`, track A's relay 45): a seeded generator
//! of case lines, and the port's answer to each, printed as `fsgcam` prints FreeSurfer's
//! (`<result> | <text>`, the text FreeSurfer's code wrote C-escaped; `EXIT <status> | <text>` when
//! it ended the program). The command `mst`: `MRIStransform` of a surface through a linear
//! transform array or a GCA morph, with the volumes given in each of the five ways.
#![allow(dead_code)]

use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_gca::gcam::gcam_alloc;
use larmorx_freesurfer_gca::surftransform::{Volumes, mris_transform};
use larmorx_freesurfer_gca::transform::{
    GcaLtaIo, Transform, Xform, transform_invert, transform_read,
};
use larmorx_freesurfer_io::mriio::mri_read_volume;
use larmorx_freesurfer_io::streams::Io;
use larmorx_freesurfer_mri::lta::{MORPH_3D_TYPE, lta_alloc};
use larmorx_freesurfer_mri::mri::{MRI_UCHAR, Mri, mri_alloc_header};
use larmorx_freesurfer_mri::vol_geom::{VolGeom, reinit_cache};
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::alloc as salloc;
use larmorx_freesurfer_surf::readwrite;
use larmorx_freesurfer_volume::ltawrite::lta_read;

/// The cases' random numbers.
pub struct Gen(u64);

impl Gen {
    pub fn new(seed: u64) -> Gen {
        Gen(seed)
    }
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn int(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo + 1) as u64) as i64
    }
    fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
        xs[self.below(xs.len() as u64) as usize]
    }
    fn chance(&mut self, k: u64) -> bool {
        self.below(k) == 0
    }
}

const SPECIAL: [f32; 6] = [
    f32::NAN,
    f32::INFINITY,
    f32::NEG_INFINITY,
    1e30,
    -0.0,
    1e-40,
];

fn fx(v: f32) -> String {
    format!("{:08x}", v.to_bits())
}
fn dx(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

/// A number of `lo..=hi` in steps of `1/d`, a special value now and then.
fn num(g: &mut Gen, lo: i64, hi: i64, d: i64) -> f32 {
    if g.chance(60) {
        return g.pick(&SPECIAL);
    }
    g.int(lo * d, hi * d) as f32 / d as f32
}

/// A geometry's tokens (`set_geom`): its extent from `extents` (one of them `0` now and then
/// when `odd`), coronal most of the time; voxel sizes, cosines and centres odd now and then.
fn geom(g: &mut Gen, t: &mut Vec<String>, extents: &[i64], odd: bool) {
    t.push(i64::from(!g.chance(5)).to_string());
    for _ in 0..3 {
        let e = if odd && g.chance(40) {
            g.pick(&[0, -1])
        } else {
            g.pick(extents)
        };
        t.push(e.to_string());
    }
    for _ in 0..3 {
        let s = if g.chance(6) {
            if odd && g.chance(12) {
                g.pick(&SPECIAL)
            } else if odd && g.chance(8) {
                g.pick(&[0.0, -1.0])
            } else {
                g.pick(&[0.5, 2.0, 1.25, 0.9375, 3.0])
            }
        } else {
            1.0
        };
        t.push(fx(s));
    }
    t.push(g.int(0, 1).to_string());
    let dircos: [f32; 9] = if odd && g.chance(12) {
        // an odd set of cosines (now and then singular or not finite)
        let c = [
            -1.0,
            0.0,
            1.0,
            0.6,
            0.8,
            -0.6,
            -0.8,
            0.707_106_77,
            0.1,
            0.995,
        ];
        let mut d = [0.0f32; 9];
        for v in &mut d {
            *v = if g.chance(80) {
                g.pick(&SPECIAL)
            } else {
                g.pick(&c)
            };
        }
        d
    } else if g.chance(5) {
        // sagittal, axial, a rotation about the vertical axis
        g.pick(&[
            [0.0, 1.0, 0.0, 0.0, 0.0, -1.0, -1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            [-0.8, 0.6, 0.0, 0.0, 0.0, -1.0, 0.6, 0.8, 0.0],
        ])
    } else {
        [-1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0]
    };
    for v in dircos {
        t.push(fx(v));
    }
    for _ in 0..3 {
        let c = if g.chance(3) { 0.0 } else { num(g, -40, 40, 8) };
        t.push(fx(c));
    }
    t.push(g.pick(&["-", "-", "orig.mgz", "/x/y/T1.mgz"]).to_owned());
}

/// A coronal geometry of the given extent (voxel sizes 1, its centre from the generator).
fn coronal_geom(g: &mut Gen, t: &mut Vec<String>, w: i64, h: i64, d: i64) {
    t.push("1".into());
    for e in [w, h, d] {
        t.push(e.to_string());
    }
    for _ in 0..3 {
        t.push(fx(1.0));
    }
    t.push("1".into());
    for v in [-1.0f32, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0] {
        t.push(fx(v));
    }
    for _ in 0..3 {
        t.push(fx(num(g, -20, 20, 4)));
    }
    t.push(g.pick(&["-", "atlas.mgz"]).to_owned());
}

/// A 4 x 4 matrix near an affine (an identity, a shift, a scale, a rotation), the last row
/// `0 0 0 1` most of the time.
fn matrix(g: &mut Gen, t: &mut Vec<String>) {
    let mut m = [[0.0f32; 4]; 4];
    for (i, row) in m.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    match g.below(5) {
        0 => {}
        1 => {
            for row in m.iter_mut().take(3) {
                row[3] = num(g, -20, 20, 4);
            }
        }
        2 => {
            for (i, row) in m.iter_mut().enumerate().take(3) {
                row[i] = g.pick(&[0.5, 2.0, -1.0, 1.1, 0.0]);
                row[3] = num(g, -10, 10, 2);
            }
        }
        3 => {
            let (c, s) = g.pick(&[(0.6f32, 0.8f32), (0.8, -0.6), (0.0, 1.0)]);
            m[0][0] = c;
            m[0][1] = -s;
            m[1][0] = s;
            m[1][1] = c;
            for row in m.iter_mut().take(3) {
                row[3] = num(g, -10, 10, 4);
            }
        }
        _ => {
            for row in m.iter_mut().take(3) {
                for v in row.iter_mut() {
                    *v = num(g, -3, 3, 4);
                }
            }
        }
    }
    if g.chance(15) {
        for v in m[3].iter_mut() {
            *v = num(g, -2, 2, 2);
        }
    }
    for row in m {
        for v in row {
            t.push(fx(v));
        }
    }
}

/// One `mst` case.
fn mst(g: &mut Gen) -> String {
    let mut t: Vec<String> = vec!["mst".into()];
    t.push(
        g.pick(&["0", "0", "0", "0", "40", "10000000", "10000040"])
            .to_owned(),
    );
    t.push(g.pick(&["-1", "0", "0", "0", "1"]).to_owned());
    let vols = g.pick(&["nn", "nn", "nn", "ss", "ss", "gn", "ng", "gg"]);
    t.push(vols.to_owned());
    let morph = g.chance(3);
    // the morph's grid of nodes and its spacing; its atlas (the vertices' volume when none is
    // given) covers the grid most of the time, so that most vertices fall inside it
    let mut ext = || if g.chance(5) { 1 } else { g.int(2, 4) };
    let (gw, gh, gd) = (ext(), ext(), ext());
    let sp = if g.chance(30) {
        g.pick(&[0, -1])
    } else {
        g.pick(&[1, 1, 2, 3])
    };
    let s1 = sp.max(1);
    let (aw, ah, ad) = (gw * s1, gh * s1, gd * s1);
    let nv = if g.chance(25) { 0 } else { g.int(1, 8) };
    t.push(nv.to_string());
    t.push(i64::from(g.chance(5)).to_string());
    geom(g, &mut t, &[1, 2, 4, 8, 256], true);
    for _ in 0..nv {
        if morph {
            // tkregister RAS of a coronal volume: x along the columns (width), y along the
            // slices (depth), z along the rows (height), each in [-n/2, n/2]
            // (the column is n/2 - x, the slice y + n/2, the row n/2 - z: inside [0, n))
            for (n, lo, hi) in [(aw, 1, 0), (ad, 0, 1), (ah, 1, 0)] {
                let v = if g.chance(8) {
                    num(g, -n - 2, n + 2, 4)
                } else {
                    g.int(-2 * n + lo, 2 * n - hi) as f32 / 4.0
                };
                t.push(fx(v));
            }
        } else {
            for _ in 0..3 {
                t.push(fx(num(g, -60, 60, 8)));
            }
        }
        t.push(i64::from(g.chance(6)).to_string());
    }
    if morph {
        t.push("M".into());
        t.push(gw.to_string());
        t.push(gh.to_string());
        t.push(gd.to_string());
        t.push(sp.to_string());
        t.push(g.pick(&[2, 2, 1, 1, 0]).to_string());
        let ext = [1, 2, 3, 4, 6, 8, 12];
        if g.chance(5) {
            geom(g, &mut t, &ext, true);
        } else {
            coronal_geom(g, &mut t, aw.max(1), ah.max(1), ad.max(1));
        }
        if g.chance(4) {
            geom(g, &mut t, &ext, true);
        } else {
            coronal_geom(g, &mut t, aw.max(1), ah.max(1), ad.max(1));
        }
        for x in 0..gw {
            for y in 0..gh {
                for z in 0..gd {
                    for c in [x, y, z] {
                        let v = if g.chance(120) {
                            f64::from(g.pick(&SPECIAL))
                        } else {
                            (c * s1) as f64 + g.int(-8, 8) as f64 / 4.0
                        };
                        t.push(dx(v));
                    }
                    let inv = if g.chance(40) { g.pick(&[1, 2, 2]) } else { 0 };
                    t.push(inv.to_string());
                }
            }
        }
    } else {
        t.push("L".into());
        let nx = if g.chance(20) { g.pick(&[0, 2]) } else { 1 };
        t.push(nx.to_string());
        let ty = if g.chance(12) {
            g.pick(&[2, 21, 14, 10, 12, 15])
        } else if g.chance(80) {
            MORPH_3D_TYPE
        } else {
            g.pick(&[0, 1, 1])
        };
        t.push(ty.to_string());
        matrix(g, &mut t);
        let ext = [1, 2, 4, 8, 16, 256];
        geom(g, &mut t, &ext, true);
        geom(g, &mut t, &ext, true);
    }
    let nvols = match vols {
        "gn" | "ng" | "ss" => 1,
        "gg" => 2,
        _ => 0,
    };
    for _ in 0..nvols {
        if morph && g.chance(2) {
            coronal_geom(g, &mut t, aw.max(1), ah.max(1), ad.max(1));
        } else {
            let odd = g.chance(3);
            geom(g, &mut t, &[1, 2, 3, 4, 8, 12], odd);
        }
    }
    t.join(" ")
}

/// `n` case lines of seed `seed`.
pub fn generate(seed: u64, n: usize) -> Vec<String> {
    let mut g = Gen::new(seed);
    (0..n).map(|_| mst(&mut g)).collect()
}

struct Shared(Rc<RefCell<Vec<u8>>>);

impl Write for Shared {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct Toks<'a> {
    t: Vec<&'a str>,
    i: usize,
}

impl<'a> Toks<'a> {
    fn next(&mut self) -> &'a str {
        let s = self.t[self.i];
        self.i += 1;
        s
    }
    fn i(&mut self) -> i32 {
        self.next().parse().expect("an int")
    }
    fn f(&mut self) -> f32 {
        f32::from_bits(u32::from_str_radix(self.next(), 16).expect("a float"))
    }
    fn d(&mut self) -> f64 {
        f64::from_bits(u64::from_str_radix(self.next(), 16).expect("a double"))
    }
    fn s(&mut self) -> &'a str {
        let s = self.next();
        if s == "-" { "" } else { s }
    }
}

#[derive(Default)]
struct Out(String);

impl Out {
    fn i(&mut self, v: i64) {
        self.0 += &format!("{v} ");
    }
    fn f(&mut self, v: f32) {
        self.0 += &format!("{:08x} ", v.to_bits());
    }
    fn s(&mut self, v: &[u8]) {
        if v.is_empty() {
            self.0 += "-";
        } else {
            self.0 += &String::from_utf8_lossy(v);
        }
        self.0 += " ";
    }
    fn geom(&mut self, v: &VolGeom) {
        self.i(i64::from(v.valid));
        self.i(i64::from(v.width));
        self.i(i64::from(v.height));
        self.i(i64::from(v.depth));
        for x in [v.xsize, v.ysize, v.zsize] {
            self.f(x);
        }
        self.i(i64::from(v.ras_good_flag));
        for x in [
            v.x_r, v.x_a, v.x_s, v.y_r, v.y_a, v.y_s, v.z_r, v.z_a, v.z_s, v.c_r, v.c_a, v.c_s,
        ] {
            self.f(x);
        }
        self.s(v.fname.c_bytes());
    }
}

fn set_geom(t: &mut Toks, v: &mut VolGeom) {
    v.valid = t.i();
    v.width = t.i();
    v.height = t.i();
    v.depth = t.i();
    v.xsize = t.f();
    v.ysize = t.f();
    v.zsize = t.f();
    v.ras_good_flag = t.i();
    v.x_r = t.f();
    v.x_a = t.f();
    v.x_s = t.f();
    v.y_r = t.f();
    v.y_a = t.f();
    v.y_s = t.f();
    v.z_r = t.f();
    v.z_a = t.f();
    v.z_s = t.f();
    v.c_r = t.f();
    v.c_a = t.f();
    v.c_s = t.f();
    let f = t.s();
    v.fname.strcpy_bytes(f.as_bytes());
}

/// `get_volume`: `MRIallocHeader` of the geometry's extent, its other fields, `MRIreInitCache`.
fn get_volume(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    t: &mut Toks,
) -> Result<Mri, Fatal> {
    let mut vg = VolGeom::default();
    set_geom(t, &mut vg);
    let mut m = mri_alloc_header(g, out, err, vg.width, vg.height, vg.depth, MRI_UCHAR, 1)?;
    let d = &mut m.vg;
    (d.xsize, d.ysize, d.zsize) = (vg.xsize, vg.ysize, vg.zsize);
    d.ras_good_flag = vg.ras_good_flag;
    (d.x_r, d.x_a, d.x_s) = (vg.x_r, vg.x_a, vg.x_s);
    (d.y_r, d.y_a, d.y_s) = (vg.y_r, vg.y_a, vg.y_s);
    (d.z_r, d.z_a, d.z_s) = (vg.z_r, vg.z_a, vg.z_s);
    (d.c_r, d.c_a, d.c_s) = (vg.c_r, vg.c_a, vg.c_s);
    d.fname.strcpy(&vg.fname);
    reinit_cache(g, out, err, &mut m.vg)?;
    Ok(m)
}

fn fnv(mut h: u64, b: &[u8]) -> u64 {
    for &c in b {
        h ^= u64::from(c);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// `msf <surface> <transform> nn|ss [<volume>]` (real files): `mri_normalize`'s call
/// (`TransformRead`, `TransformInvert(transform, NULL)`, `MRIStransform(mris, NULL, transform,
/// NULL)`) or `mris_sphere`'s (`LTAread`, `MRIread`, `MRIStransform(mris, mri, &transform, mri)`).
fn files(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    t: &mut Toks,
    o: &mut Out,
) -> Result<(), Fatal> {
    let env = Environment::from_pairs(vec![
        ("FREESURFER_HOME", "/nonexistent-freesurfer-home"),
        ("FS_LICENSE", "/usr/local/freesurfer/license.txt"),
    ]);
    larmorx_freesurfer_gca::register_io();
    let (sf, xf, vols) = (t.next(), t.next(), t.next());
    let mut sg = SurfGlobals::default();
    let mris = {
        let mut io = Io {
            out: &mut *out,
            err: &mut *err,
            env: &env,
        };
        readwrite::read(g, &mut sg, &mut io, None, sf, true)?
    };
    let Some(mut mris) = mris else {
        o.i(-1);
        return Ok(());
    };
    let mut vol = None;
    let mut transform = if vols == "nn" {
        let Some(mut tr) = transform_read(g, out, err, &env, None, xf)? else {
            o.i(-2);
            return Ok(());
        };
        o.i(i64::from(transform_invert(
            g, out, err, &env, None, &mut tr, None,
        )?));
        tr
    } else {
        let mut lio = GcaLtaIo { env: &env };
        let Some(lta) = lta_read(g, out, err, None, xf, &mut lio)? else {
            o.i(-2);
            return Ok(());
        };
        let Some(m) = mri_read_volume(g, out, err, &env, None, t.next())? else {
            o.i(-3);
            return Ok(());
        };
        vol = Some(m);
        Transform::new(lta.ltype, Xform::Lta(lta))
    };
    let volumes = match vol.as_mut() {
        Some(m) => Volumes::Same(m),
        None => Volumes::Separate(None, None),
    };
    let ret = mris_transform(g, out, err, &env, &mut mris, volumes, &mut transform)?;
    o.i(i64::from(ret));
    o.i(i64::from(mris.nvertices));
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for vno in 0..mris.nvertices.max(0) as usize {
        let v = mris.vertex(vno);
        for c in [v.x, v.y, v.z] {
            h = fnv(h, &c.to_le_bytes());
        }
    }
    o.0 += &format!("{h:016x} ");
    for v in [
        mris.xlo, mris.xhi, mris.ylo, mris.yhi, mris.zlo, mris.zhi, mris.xctr, mris.yctr, mris.zctr,
    ] {
        o.f(v);
    }
    o.geom(&mris.vg);
    Ok(())
}

fn case(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    t: &mut Toks,
    o: &mut Out,
) -> Result<(), Fatal> {
    let cmd = t.next();
    if cmd == "msf" {
        return files(g, out, err, t, o);
    }
    assert_eq!(cmd, "mst", "unknown command");
    g.gdiag = u64::from_str_radix(t.next(), 16).expect("gdiag");
    g.gdiag_no = t.i();
    let vols = t.next();
    let nv = t.i();
    let real = t.i();
    let sg = SurfGlobals::default();
    let mut mris = salloc::alloc(g, out, err, &sg, nv, 0)?;
    mris.useRealRAS = real;
    set_geom(t, &mut mris.vg);
    for vno in 0..nv.max(0) as usize {
        let (x, y, z, rip) = (t.f(), t.f(), t.f(), t.i());
        let v = mris.vertex_mut(vno);
        (v.x, v.y, v.z) = (x, y, z);
        v.ripflag = rip as i8;
    }
    let kind = t.next();
    let mut transform = if kind == "L" {
        let nx = t.i();
        let mut lta = lta_alloc(g, out, err, nx, None)?;
        lta.ltype = t.i();
        if nx > 0 {
            let lt = &mut lta.xforms[0];
            lt.ltype = lta.ltype;
            let m = lt.m_l.as_mut().expect("LTAalloc's matrix");
            for r in 1..=4 {
                for c in 1..=4 {
                    m.set_relt(r, c, t.f());
                }
            }
            set_geom(t, &mut lt.src);
            set_geom(t, &mut lt.dst);
        } else {
            for _ in 0..16 {
                t.f();
            }
            let mut tmp = VolGeom::default();
            set_geom(t, &mut tmp);
            set_geom(t, &mut tmp);
        }
        Transform::new(lta.ltype, Xform::Lta(lta))
    } else {
        let (w, h, d) = (t.i(), t.i(), t.i());
        let mut gcam = gcam_alloc(g, err, w, h, d)?;
        gcam.spacing = t.i();
        gcam.mtype = t.i();
        set_geom(t, &mut gcam.image);
        set_geom(t, &mut gcam.atlas);
        for x in 0..w {
            for y in 0..h {
                for z in 0..d {
                    let n = gcam.node_mut(x, y, z);
                    n.x = t.d();
                    n.y = t.d();
                    n.z = t.d();
                    n.invalid = t.i() as i8;
                }
            }
        }
        Transform::new(MORPH_3D_TYPE, Xform::Morph(Box::new(gcam)))
    };
    let env = Environment::default();
    let ret = match vols {
        "nn" => mris_transform(
            g,
            out,
            err,
            &env,
            &mut mris,
            Volumes::Separate(None, None),
            &mut transform,
        )?,
        "gn" => {
            let mut a = get_volume(g, out, err, t)?;
            mris_transform(
                g,
                out,
                err,
                &env,
                &mut mris,
                Volumes::Separate(Some(&mut a), None),
                &mut transform,
            )?
        }
        "ng" => {
            let mut b = get_volume(g, out, err, t)?;
            mris_transform(
                g,
                out,
                err,
                &env,
                &mut mris,
                Volumes::Separate(None, Some(&mut b)),
                &mut transform,
            )?
        }
        "gg" => {
            let mut a = get_volume(g, out, err, t)?;
            let mut b = get_volume(g, out, err, t)?;
            mris_transform(
                g,
                out,
                err,
                &env,
                &mut mris,
                Volumes::Separate(Some(&mut a), Some(&mut b)),
                &mut transform,
            )?
        }
        "ss" => {
            let mut a = get_volume(g, out, err, t)?;
            mris_transform(
                g,
                out,
                err,
                &env,
                &mut mris,
                Volumes::Same(&mut a),
                &mut transform,
            )?
        }
        v => panic!("volumes {v}"),
    };
    o.i(i64::from(ret));
    for vno in 0..nv.max(0) as usize {
        let v = mris.vertex(vno);
        o.f(v.x);
        o.f(v.y);
        o.f(v.z);
    }
    for v in [
        mris.xlo, mris.xhi, mris.ylo, mris.yhi, mris.zlo, mris.zhi, mris.xctr, mris.yctr, mris.zctr,
    ] {
        o.f(v);
    }
    o.geom(&mris.vg);
    if let Xform::Morph(gcam) = &transform.xform {
        o.i(i64::from(gcam.mtype));
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for x in 0..gcam.width {
            for y in 0..gcam.height {
                for z in 0..gcam.depth {
                    let n = gcam.node(x, y, z);
                    for p in [n.x, n.y, n.z, n.origx, n.origy, n.origz] {
                        h = fnv(h, &p.to_le_bytes());
                    }
                    h = fnv(h, &[n.invalid as u8]);
                    for a in [n.orig_area, n.orig_area1, n.orig_area2] {
                        h = fnv(h, &a.to_le_bytes());
                    }
                }
            }
        }
        o.0 += &format!("{h:016x} ");
        o.geom(&gcam.image);
    }
    Ok(())
}

fn escape(b: &[u8]) -> String {
    let mut s = String::new();
    for &c in b {
        if !(0x20..=0x7e).contains(&c) || c == b'\\' {
            s += &format!("\\x{c:02x}");
        } else {
            s.push(c as char);
        }
    }
    s
}

/// The port's line for a case line, and the defects it noted.
pub fn run_line(line: &str, mode: UpstreamDefects) -> (String, Vec<&'static str>) {
    let shared = Rc::new(RefCell::new(Vec::new()));
    let (mut a, mut b) = (Shared(shared.clone()), Shared(shared.clone()));
    let mut g = Globals::new(mode);
    let mut t = Toks {
        t: line.split_whitespace().collect(),
        i: 0,
    };
    let mut o = Out::default();
    let res = case(&mut g, &mut a, &mut b, &mut t, &mut o);
    let text = escape(&shared.borrow());
    let result = match res {
        Ok(()) => o.0.trim_end().to_owned(),
        Err(f) => format!("EXIT {}", f.status()),
    };
    let mut ids: Vec<&'static str> = g.notices.as_slice().iter().map(|n| n.defect.id).collect();
    ids.sort_unstable();
    ids.dedup();
    (format!("{result} | {text}"), ids)
}
