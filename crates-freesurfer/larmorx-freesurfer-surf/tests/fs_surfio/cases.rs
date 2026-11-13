// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.surfio probe's cases (`validation/probes/fs-surfio/`): a seeded generator of case lines,
//! and the port's answer to each, printed as `fssurfio` prints FreeSurfer's (`<result> | <text>`,
//! the text FreeSurfer's code wrote C-escaped; `EXIT <status> | <text>` when it ended the program,
//! `CRASH 11 | <text>` where FreeSurfer's failed assertion faults on purpose, FS-03 reproduced).
#![allow(dead_code, clippy::needless_range_loop)]

use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;
use std::sync::Arc;

use larmorx_core::defects::{FS_03, UpstreamDefects};
use larmorx_freesurfer_core::diag::DIAG_VERBOSE;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_surf::aaa::{self, MrisStatus, SurfGlobals};
use larmorx_freesurfer_surf::base;
use larmorx_freesurfer_surf::mris::{Face, Mris, Vertex, VertexTopology};
use larmorx_freesurfer_surf::mris_mp::MrisMp;
use larmorx_freesurfer_surf::topology;
use larmorx_freesurfer_surf::views::{self, MetricSurface, MpView};

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

/// Special floats: NaNs of both signs with payloads, infinities, zeros, subnormals, extremes.
const SPECIAL32: [u32; 12] = [
    0x7fc0_0000,
    0xffc0_0000,
    0x7fa0_0001,
    0xff80_0123,
    0x7f80_0000,
    0xff80_0000,
    0x0000_0000,
    0x8000_0000,
    0x0000_0001,
    0x807f_ffff,
    0x7f7f_ffff,
    0x3400_0000,
];

fn f32bits(g: &mut Gen) -> u32 {
    if g.chance(12) {
        return g.pick(&SPECIAL32);
    }
    if g.chance(10) {
        return g.next() as u32;
    }
    (g.int(-4000, 4000) as f32 / 16.0).to_bits()
}

fn f64bits(g: &mut Gen) -> u64 {
    if g.chance(12) {
        return f64::from(f32::from_bits(g.pick(&SPECIAL32))).to_bits();
    }
    if g.chance(10) {
        return g.next();
    }
    (g.int(-40000, 40000) as f64 / 64.0).to_bits()
}

fn fx(t: &mut Vec<String>, g: &mut Gen) {
    t.push(format!("{:08x}", f32bits(g)));
}
fn dx(t: &mut Vec<String>, g: &mut Gen) {
    t.push(format!("{:016x}", f64bits(g)));
}
fn ix(t: &mut Vec<String>, v: i64) {
    t.push(v.to_string());
}

const COMMANDS: [&str; 15] = [
    "coord", "vnum", "nbr", "modvnum", "norm", "close", "proj", "sign", "status", "sortcmp",
    "angles", "split", "vec3", "view", "fixarea",
];

fn coords(t: &mut Vec<String>, g: &mut Gen) {
    for _ in 0..45 {
        fx(t, g);
    }
}

fn view_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let nv = g.int(1, 3);
    let nf = g.int(1, 2);
    ix(t, nv);
    ix(t, nf);
    let small = |g: &mut Gen| g.int(-3, 3);
    // the MRIS
    for _ in 0..nv {
        for _ in 0..4 {
            fx(t, g);
        }
        ix(t, g.int(0, 9));
        coords(t, g);
        for _ in 0..3 {
            fx(t, g);
        }
        ix(t, g.int(-1, 9));
        for _ in 0..3 {
            ix(t, small(g));
        }
    }
    for _ in 0..nf {
        for _ in 0..3 {
            ix(t, g.int(0, nv - 1));
        }
        for _ in 0..7 {
            fx(t, g);
        }
        ix(t, small(g));
    }
    surface_tokens(t, g, true);
    // the MRIS_MP
    for _ in 0..nv {
        for _ in 0..2 {
            fx(t, g);
        }
        ix(t, g.int(0, 9));
        for _ in 0..27 {
            fx(t, g);
        }
        ix(t, g.int(-1, 9));
        for _ in 0..3 {
            ix(t, small(g));
        }
    }
    for _ in 0..nf {
        for _ in 0..3 {
            ix(t, g.int(0, nv - 1));
        }
        for _ in 0..7 {
            fx(t, g);
        }
        ix(t, small(g));
    }
    surface_tokens(t, g, true);
    // the writes, through the MRIS views then the MRIS_MP ones
    for _ in 0..2 {
        for _ in 0..24 {
            fx(t, g);
        }
        ix(t, g.int(-1, 9));
        for _ in 0..3 {
            ix(t, small(g));
        }
        ix(t, g.int(0, nv - 1));
        for _ in 0..7 {
            fx(t, g);
        }
        ix(t, small(g));
        surface_tokens(t, g, false);
    }
}

/// The surface fields: 10 floats, 3 doubles, 3 floats, (radius), status, origxyz_status, patch,
/// (noscale).
fn surface_tokens(t: &mut Vec<String>, g: &mut Gen, with_ro: bool) {
    for _ in 0..10 {
        fx(t, g);
    }
    for _ in 0..3 {
        dx(t, g);
    }
    for _ in 0..3 {
        fx(t, g);
    }
    if with_ro {
        dx(t, g);
    }
    ix(t, g.int(0, 9));
    ix(t, g.int(0, 9));
    ix(t, g.int(-1, 2));
    if with_ro {
        ix(t, g.int(0, 1));
    }
}

/// `n` case lines from `seed`, the commands in turn.
pub fn generate(seed: u64, n: usize) -> Vec<String> {
    let mut g = Gen::new(seed);
    let mut lines = Vec::with_capacity(n);
    for k in 0..n {
        let cmd = COMMANDS[k % COMMANDS.len()];
        let mut t: Vec<String> = vec![cmd.to_owned()];
        match cmd {
            "coord" => {
                ix(&mut t, g.int(-1, 15));
                coords(&mut t, &mut g);
            }
            "vnum" => {
                ix(&mut t, g.int(0, 4));
                for _ in 0..3 {
                    ix(&mut t, g.int(-5, 40));
                }
            }
            "nbr" => {
                let nv = g.int(0, 4);
                let vno1 = if g.chance(8) {
                    g.int(-1, 5)
                } else {
                    g.int(0, nv.max(1) - 1)
                };
                let hasv = i64::from(!g.chance(5));
                let vnum = if hasv == 1 { g.int(0, 6) } else { g.int(-2, 4) };
                ix(&mut t, nv);
                ix(&mut t, vno1);
                ix(&mut t, g.int(-1, 6));
                ix(&mut t, hasv);
                ix(&mut t, vnum);
                if hasv == 1 {
                    for _ in 0..vnum {
                        ix(&mut t, g.int(-1, 6));
                    }
                }
            }
            "modvnum" => {
                ix(&mut t, i64::from(g.chance(2)));
                ix(&mut t, g.int(0, 2));
                let start = if g.chance(6) {
                    g.pick(&[32767, -32768, 32760, -1])
                } else {
                    g.int(0, 40)
                };
                ix(&mut t, start);
                let n = g.int(1, 6);
                ix(&mut t, n);
                for _ in 0..n {
                    t.push(g.pick(&["S", "C", "A", "D"]).to_owned());
                    let a = if g.chance(8) {
                        g.pick(&[32767, -32768, 20000])
                    } else {
                        g.int(-5, 12)
                    };
                    ix(&mut t, a);
                }
            }
            "norm" | "close" | "proj" | "sortcmp" => {
                let k = match cmd {
                    "norm" => 3,
                    "proj" => 4,
                    _ => 2,
                };
                for _ in 0..k {
                    fx(&mut t, &mut g);
                }
            }
            "sign" => {
                fx(&mut t, &mut g);
                dx(&mut t, &mut g);
            }
            "status" => {
                ix(&mut t, i64::from(g.chance(2)));
                for _ in 0..2 {
                    let s = if g.chance(8) {
                        g.pick(&[-1, 10, 11, 99])
                    } else {
                        g.int(0, 9)
                    };
                    ix(&mut t, s);
                }
            }
            "angles" => {
                for _ in 0..3 {
                    fx(&mut t, &mut g);
                }
                for k in 0..3 {
                    // often equal to the first array's, element by element
                    if g.chance(2) {
                        let v = t[1 + k].clone();
                        t.push(v);
                    } else {
                        fx(&mut t, &mut g);
                    }
                }
            }
            "split" => {
                for _ in 0..2 {
                    let v = if g.chance(10) {
                        g.int(-50, -1)
                    } else {
                        g.int(0, 2_000_000)
                    };
                    ix(&mut t, v);
                }
            }
            "vec3" => {
                for _ in 0..6 {
                    dx(&mut t, &mut g);
                }
                // the scale never NaN: where two NaNs meet, the operand order is the inlining
                // call site's code (the probe's own compilation here), checked with each caller
                let s = loop {
                    let b = f64bits(&mut g);
                    if !f64::from_bits(b).is_nan() {
                        break b;
                    }
                };
                t.push(format!("{s:016x}"));
            }
            "view" => view_tokens(&mut t, &mut g),
            "fixarea" => ix(&mut t, g.int(-2, 3)),
            _ => unreachable!(),
        }
        lines.push(t.join(" "));
    }
    lines
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
    /// `strtol(s, NULL, 10)` cut to `int`.
    fn i(&mut self) -> i32 {
        self.next().parse::<i64>().expect("int") as i32
    }
    fn f(&mut self) -> f32 {
        f32::from_bits(u32::from_str_radix(self.next(), 16).expect("float"))
    }
    fn d(&mut self) -> f64 {
        f64::from_bits(u64::from_str_radix(self.next(), 16).expect("double"))
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
    fn d(&mut self, v: f64) {
        self.0 += &format!("{:016x} ", v.to_bits());
    }
    fn s(&mut self, v: &str) {
        self.0 += v;
        self.0 += " ";
    }
}

/// Standard output and error, one stream (the probe sends both to one file).
#[derive(Clone, Default)]
struct Shared(Rc<RefCell<Vec<u8>>>);

impl Write for Shared {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct Ctx {
    g: Globals,
    sg: SurfGlobals,
    a: Shared,
    b: Shared,
}

/// `set_coords`'s 45 floats.
fn set_coords(t: &mut Toks, v: &mut Vertex) {
    let mut f = || t.f();
    (v.x, v.y, v.z) = (f(), f(), f());
    (v.origx, v.origy, v.origz) = (f(), f(), f());
    (v.tx, v.ty, v.tz) = (f(), f(), f());
    (v.cx, v.cy, v.cz) = (f(), f(), f());
    (v.infx, v.infy, v.infz) = (f(), f(), f());
    (v.pialx, v.pialy, v.pialz) = (f(), f(), f());
    (v.t2x, v.t2y, v.t2z) = (f(), f(), f());
    (v.fx, v.fy, v.fz) = (f(), f(), f());
    (v.whitex, v.whitey, v.whitez) = (f(), f(), f());
    (v.nx, v.ny, v.nz) = (f(), f(), f());
    (v.pnx, v.pny, v.pnz) = (f(), f(), f());
    (v.wnx, v.wny, v.wnz) = (f(), f(), f());
    (v.targx, v.targy, v.targz) = (f(), f(), f());
    (v.l4x, v.l4y, v.l4z) = (f(), f(), f());
    (v.dx, v.dy, v.dz) = (f(), f(), f());
}

/// An `MRIS` of `nv` vertices and `nf` faces, as the probe's `MRISalloc` gives one for these
/// cases (only the fields the cases read and write matter).
fn mris_alloc(nv: usize, nf: usize) -> Mris {
    Mris {
        nvertices: nv as i32,
        nfaces: nf as i32,
        vertices: Some(vec![Vertex::default(); nv]),
        vertices_topology: Some(vec![VertexTopology::default(); nv]),
        faces: Some(vec![Face::default(); nf]),
        ..Mris::default()
    }
}

fn read_view(s: &dyn MetricSurface, nv: usize, nf: usize, ndist: usize, o: &mut Out) {
    for vno in 0..nv {
        for i in 0..ndist {
            o.f(s.v_dist(vno, i));
            o.f(s.v_dist_orig(vno, i));
        }
        o.i(i64::from(s.v_dist_capacity(vno)));
        for v in [
            s.v_x(vno),
            s.v_y(vno),
            s.v_z(vno),
            s.v_origx(vno),
            s.v_origy(vno),
            s.v_origz(vno),
            s.v_nx(vno),
            s.v_ny(vno),
            s.v_nz(vno),
            s.v_wnx(vno),
            s.v_wny(vno),
            s.v_wnz(vno),
            s.v_dx(vno),
            s.v_dy(vno),
            s.v_dz(vno),
            s.v_curv(vno),
            s.v_cx(vno),
            s.v_cy(vno),
            s.v_cz(vno),
            s.v_pialx(vno),
            s.v_pialy(vno),
            s.v_pialz(vno),
            s.v_whitex(vno),
            s.v_whitey(vno),
            s.v_whitez(vno),
            s.v_area(vno),
            s.v_origarea(vno),
        ] {
            o.f(v);
        }
        o.i(i64::from(s.v_fno(vno)));
        o.i(i64::from(s.v_neg(vno)));
        o.i(i64::from(s.v_border(vno)));
        o.i(i64::from(s.v_ripflag(vno)));
    }
    for fno in 0..nf {
        for i in 0..3 {
            o.i(i64::from(s.f_v(fno, i)));
        }
        o.f(s.f_area(fno));
        let (a, b) = (s.f_angle(fno), s.f_orig_angle(fno));
        for i in 0..3 {
            o.f(a[i]);
            o.f(b[i]);
        }
        o.i(i64::from(s.f_ripflag(fno)));
    }
    o.i(i64::from(s.nvertices()));
    o.i(i64::from(s.nfaces()));
    for v in [
        s.xctr(),
        s.yctr(),
        s.zctr(),
        s.xlo(),
        s.ylo(),
        s.zlo(),
        s.xhi(),
        s.yhi(),
        s.zhi(),
        s.total_area(),
    ] {
        o.f(v);
    }
    o.d(s.avg_vertex_area());
    o.d(s.avg_vertex_dist());
    o.d(s.std_vertex_dist());
    o.f(s.orig_area());
    o.f(s.neg_area());
    o.f(s.neg_orig_area());
    o.d(s.radius());
    o.i(i64::from(s.status().0));
    o.i(i64::from(s.origxyz_status().0));
    o.i(i64::from(s.patch()));
    o.i(i64::from(s.noscale()));
}

fn write_view(s: &mut dyn MetricSurface, t: &mut Toks) {
    let w: [fn(&mut dyn MetricSurface, usize, f32); 24] = [
        |s, v, x| s.set_v_x(v, x),
        |s, v, x| s.set_v_y(v, x),
        |s, v, x| s.set_v_z(v, x),
        |s, v, x| s.set_v_nx(v, x),
        |s, v, x| s.set_v_ny(v, x),
        |s, v, x| s.set_v_nz(v, x),
        |s, v, x| s.set_v_wnx(v, x),
        |s, v, x| s.set_v_wny(v, x),
        |s, v, x| s.set_v_wnz(v, x),
        |s, v, x| s.set_v_dx(v, x),
        |s, v, x| s.set_v_dy(v, x),
        |s, v, x| s.set_v_dz(v, x),
        |s, v, x| s.set_v_curv(v, x),
        |s, v, x| s.set_v_cx(v, x),
        |s, v, x| s.set_v_cy(v, x),
        |s, v, x| s.set_v_cz(v, x),
        |s, v, x| s.set_v_pialx(v, x),
        |s, v, x| s.set_v_pialy(v, x),
        |s, v, x| s.set_v_pialz(v, x),
        |s, v, x| s.set_v_whitex(v, x),
        |s, v, x| s.set_v_whitey(v, x),
        |s, v, x| s.set_v_whitez(v, x),
        |s, v, x| s.set_v_area(v, x),
        |s, v, x| s.set_v_origarea(v, x),
    ];
    for f in w {
        let x = t.f();
        f(s, 0, x);
    }
    s.set_v_fno(0, t.i());
    s.set_v_neg(0, t.i() as i8);
    s.set_v_border(0, t.i() as i8);
    s.set_v_ripflag(0, t.i() as i8);
    let k = t.i();
    s.set_f_v(0, 0, k);
    s.set_f_area(0, t.f());
    let (mut a, mut b) = ([0f32; 3], [0f32; 3]);
    for i in 0..3 {
        a[i] = t.f();
        b[i] = t.f();
    }
    s.set_f_angle(0, a);
    s.set_f_orig_angle(0, b);
    s.set_f_ripflag(0, t.i() as i8);
    let sw: [fn(&mut dyn MetricSurface, f32); 10] = [
        |s, x| s.set_xctr(x),
        |s, x| s.set_yctr(x),
        |s, x| s.set_zctr(x),
        |s, x| s.set_xlo(x),
        |s, x| s.set_ylo(x),
        |s, x| s.set_zlo(x),
        |s, x| s.set_xhi(x),
        |s, x| s.set_yhi(x),
        |s, x| s.set_zhi(x),
        |s, x| s.set_total_area(x),
    ];
    for f in sw {
        let x = t.f();
        f(s, x);
    }
    s.set_avg_vertex_area(t.d());
    s.set_avg_vertex_dist(t.d());
    s.set_std_vertex_dist(t.d());
    s.set_orig_area(t.f());
    s.set_neg_area(t.f());
    s.set_neg_orig_area(t.f());
    s.set_status(MrisStatus(t.i()));
    s.set_origxyz_status(MrisStatus(t.i()));
    s.set_patch(t.i());
}

fn view_case(t: &mut Toks, o: &mut Out) {
    let nv = t.i() as usize;
    let nf = t.i() as usize;
    let ndist = 2;
    let mut m = mris_alloc(nv, nf);
    for vno in 0..nv {
        let v = m.vertex_mut(vno);
        let (mut d, mut dor) = (vec![0f32; ndist], vec![0f32; ndist]);
        for i in 0..ndist {
            d[i] = t.f();
            dor[i] = t.f();
        }
        v.dist = Some(d);
        v.dist_orig = Some(dor);
        v.dist_capacity = t.i();
        set_coords(t, v);
        v.curv = t.f();
        v.area = t.f();
        v.origarea = t.f();
        v.fno = t.i();
        v.neg = t.i() as i8;
        v.border = t.i() as i8;
        v.ripflag = t.i() as i8;
    }
    for fno in 0..nf {
        let f = m.face_mut(fno);
        for i in 0..3 {
            f.v[i] = t.i();
        }
        f.area = t.f();
        for i in 0..3 {
            f.angle[i] = t.f();
            f.orig_angle[i] = t.f();
        }
        f.ripflag = t.i() as i8;
    }
    (m.xctr, m.yctr, m.zctr, m.xlo, m.ylo) = (t.f(), t.f(), t.f(), t.f(), t.f());
    (m.zlo, m.xhi, m.yhi, m.zhi, m.total_area) = (t.f(), t.f(), t.f(), t.f(), t.f());
    (m.avg_vertex_area, m.avg_vertex_dist, m.std_vertex_dist) = (t.d(), t.d(), t.d());
    (m.orig_area, m.neg_area, m.neg_orig_area, m.radius) = (t.f(), t.f(), t.f(), t.d());
    m.status = MrisStatus(t.i());
    m.origxyz_status = MrisStatus(t.i());
    m.patch = t.i();
    m.noscale = t.i();
    let mut mp = MrisMp {
        nvertices: nv as i32,
        nfaces: nf as i32,
        ..MrisMp::default()
    };
    let mut v_dist = vec![None; nv];
    let mut cap = vec![0i32; nv];
    let mut vf: Vec<Vec<f32>> = vec![vec![0f32; nv]; 27];
    let (mut fno_, mut neg, mut border, mut rip) =
        (vec![0i32; nv], vec![0i8; nv], vec![0i8; nv], vec![0i8; nv]);
    for vno in 0..nv {
        let mut d = vec![0f32; ndist];
        for x in d.iter_mut() {
            *x = t.f();
        }
        v_dist[vno] = Some(d);
        cap[vno] = t.i();
        for a in vf.iter_mut() {
            a[vno] = t.f();
        }
        fno_[vno] = t.i();
        neg[vno] = t.i() as i8;
        border[vno] = t.i() as i8;
        rip[vno] = t.i() as i8;
    }
    let mut it = vf.into_iter().map(Some);
    let mut nxt = || it.next().unwrap();
    let sh = |v: Option<Vec<f32>>| v.map(Arc::new);
    mp.v_x = nxt();
    mp.v_y = nxt();
    mp.v_z = nxt();
    mp.v_origx = nxt();
    mp.v_origy = nxt();
    mp.v_origz = nxt();
    mp.v_nx = nxt();
    mp.v_ny = nxt();
    mp.v_nz = nxt();
    mp.v_wnx = sh(nxt());
    mp.v_wny = sh(nxt());
    mp.v_wnz = sh(nxt());
    mp.v_dx = nxt();
    mp.v_dy = nxt();
    mp.v_dz = nxt();
    mp.v_curv = nxt();
    mp.v_cx = nxt();
    mp.v_cy = nxt();
    mp.v_cz = nxt();
    mp.v_pialx = sh(nxt());
    mp.v_pialy = sh(nxt());
    mp.v_pialz = sh(nxt());
    mp.v_whitex = sh(nxt());
    mp.v_whitey = sh(nxt());
    mp.v_whitez = sh(nxt());
    mp.v_area = nxt();
    mp.v_origarea = nxt();
    mp.v_dist = Some(v_dist);
    mp.v_dist_capacity = Some(cap);
    mp.v_fno = Some(fno_);
    mp.v_neg = Some(neg);
    mp.v_border = Some(border);
    mp.v_ripflag = Some(Arc::new(rip));
    let (mut f_v, mut f_area, mut f_angle, mut f_orig, mut f_rip) = (
        vec![[0i32; 3]; nf],
        vec![0f32; nf],
        vec![[0f32; 3]; nf],
        vec![[0f32; 3]; nf],
        vec![0i8; nf],
    );
    for fno in 0..nf {
        for i in 0..3 {
            f_v[fno][i] = t.i();
        }
        f_area[fno] = t.f();
        for i in 0..3 {
            f_angle[fno][i] = t.f();
            f_orig[fno][i] = t.f();
        }
        f_rip[fno] = t.i() as i8;
    }
    mp.f_v = Some(f_v);
    mp.f_area = Some(f_area);
    mp.f_angle = Some(f_angle);
    mp.f_orig_angle = Some(Arc::new(f_orig));
    mp.f_ripflag = Some(Arc::new(f_rip));
    (mp.xctr, mp.yctr, mp.zctr, mp.xlo, mp.ylo) = (t.f(), t.f(), t.f(), t.f(), t.f());
    (mp.zlo, mp.xhi, mp.yhi, mp.zhi, mp.total_area) = (t.f(), t.f(), t.f(), t.f(), t.f());
    (mp.avg_vertex_area, mp.avg_vertex_dist, mp.std_vertex_dist) = (t.d(), t.d(), t.d());
    (mp.orig_area, mp.neg_area, mp.neg_orig_area, mp.radius) = (t.f(), t.f(), t.f(), t.d());
    mp.status = MrisStatus(t.i());
    mp.origxyz_status = MrisStatus(t.i());
    mp.patch = t.i();
    mp.noscale = t.i();

    read_view(&m, nv, nf, ndist, o);
    read_view(
        &MpView {
            mp: &mut mp,
            mris: &mut m,
        },
        nv,
        nf,
        ndist,
        o,
    );
    o.0 += "| ";
    write_view(&mut m, t);
    write_view(
        &mut MpView {
            mp: &mut mp,
            mris: &mut m,
        },
        t,
    );
    read_view(&m, nv, nf, ndist, o);
    read_view(
        &MpView {
            mp: &mut mp,
            mris: &mut m,
        },
        nv,
        nf,
        ndist,
        o,
    );
    o.i(i64::from(mp.status.0));
    o.i(i64::from(mp.patch));
}

fn case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let cmd = t.next();
    match cmd {
        "coord" => {
            let which = t.i();
            let mut v = Vertex::default();
            set_coords(t, &mut v);
            let (x, y, z) = views::vertex_coord2xyz_float(&mut cx.g, &mut cx.b, &v, which)?;
            o.f(x);
            o.f(y);
            o.f(z);
            let (a, b, c) = views::vertex_coord2xyz_double(&mut cx.g, &mut cx.b, &v, which)?;
            o.d(a);
            o.d(b);
            o.d(c);
            // the MRIS views' which_coords at every level
            for _ in 0..3 {
                let (a, b, c) = views::vertex_coord2xyz_float(&mut cx.g, &mut cx.b, &v, which)?;
                o.f(a);
                o.f(b);
                o.f(c);
            }
        }
        "vnum" => {
            let i = t.i();
            let vt = VertexTopology {
                vnum: t.i() as i16,
                v2num: t.i() as i16,
                v3num: t.i() as i16,
                ..VertexTopology::default()
            };
            let r = topology::vertex_vnum(&mut cx.g, &mut cx.a, &mut cx.b, &vt, i)?;
            o.i(i64::from(r));
        }
        "nbr" => {
            let (nv, vno1, vno2, hasv, vnum) = (t.i(), t.i(), t.i(), t.i(), t.i());
            let mut m = mris_alloc(nv.max(1) as usize, 0);
            m.nvertices = nv;
            if 0 <= vno1 && vno1 < nv.max(1) {
                let vt = m.topology_mut(vno1 as usize);
                vt.v = None;
                if hasv != 0 {
                    vt.v = Some((0..vnum).map(|_| t.i()).collect());
                }
                vt.vnum = vnum as i16;
            }
            let k =
                topology::vertex_neighbor_index(&mut cx.g, &mut cx.a, &mut cx.b, &m, vno1, vno2)?;
            o.i(i64::from(k));
            let b =
                topology::vertices_are_neighbors(&mut cx.g, &mut cx.a, &mut cx.b, &m, vno1, vno2)?;
            o.i(i64::from(b));
            if hasv != 0 || vnum <= 0 {
                o.i(i64::from(topology::vertex_neighbor(
                    &m,
                    vno1 as usize,
                    vno2,
                )));
            }
        }
        "modvnum" => {
            if t.i() != 0 {
                cx.g.gdiag |= DIAG_VERBOSE;
            }
            let mut m = mris_alloc(3, 0);
            let vno = t.i() as usize;
            m.topology_mut(vno).vnum = t.i() as i16;
            let n = t.i();
            for _ in 0..n {
                let op = t.next();
                let a = t.i() as i16;
                let (g, sg, e) = (&cx.g, &mut cx.sg, &mut cx.b);
                let r = match op {
                    "S" => topology::set_vnum(g, sg, e, &mut m, vno, a),
                    "C" => topology::clear_vnum(g, sg, e, &mut m, vno),
                    "A" => topology::vnum_add(g, sg, e, &mut m, vno, a),
                    "D" => topology::add_vnum(g, sg, e, &mut m, vno, a),
                    _ => panic!("modvnum op"),
                };
                o.i(i64::from(r));
            }
            o.i(i64::from(m.topology(vno).vnum));
        }
        "norm" => {
            let mut v = [t.f(), t.f(), t.f()];
            let d = base::mris_normalize(&mut v);
            o.f(d);
            o.f(v[0]);
            o.f(v[1]);
            o.f(v[2]);
        }
        "close" => {
            let (a, b) = (t.f(), t.f());
            o.i(i64::from(base::close_enough(a, b)));
        }
        "proj" => {
            let (cx_, cy, cz, r) = (t.f(), t.f(), t.f(), t.f());
            let (x, y, z) = base::project_point_onto_sphere(cx_, cy, cz, r);
            o.i(0);
            o.f(x);
            o.f(y);
            o.f(z);
        }
        "sign" => {
            let (f, d) = (t.f(), t.d());
            o.d(base::sign_f32(f));
            o.d(base::sign_f64(d));
        }
        "status" => {
            if t.i() != 0 {
                cx.g.gdiag |= DIAG_VERBOSE;
            }
            let (s1, s2) = (MrisStatus(t.i()), MrisStatus(t.i()));
            let (g, a, b) = (&mut cx.g, &mut cx.a, &mut cx.b);
            o.s(aaa::status_text(g, a, b, s1)?);
            let f = aaa::status_distance_formula(g, a, b, s1)?;
            o.i(i64::from(f == aaa::DistanceFormula::Formula1));
            o.i(i64::from(aaa::are_compatible(g, a, b, s1, s2)?));
            aaa::check_orig_xyz_compatible(g, a, b, s1, s2)?;
            o.0 += aaa::status_text(g, a, b, s2)?;
        }
        "sortcmp" => {
            let (a, b) = (t.f(), t.f());
            o.i(i64::from(aaa::mris_sort_compare_float(a, b)));
        }
        "angles" => {
            let mut a = [t.f(), t.f(), t.f()];
            let b = [t.f(), t.f(), t.f()];
            o.i(i64::from(aaa::cmp_angles_per_triangle(&a, &b)));
            aaa::copy_angles_per_triangle(&mut a, &b);
            o.f(a[0]);
            o.f(a[1]);
            o.f(a[2]);
            o.i(i64::from(aaa::cmp_angles_per_triangle(&a, &b)));
        }
        "split" => {
            let (a, b) = (t.i(), t.i());
            o.i(i64::from(aaa::which_face_split(&mut cx.g, a, b)));
        }
        "vec3" => {
            let u = base::Vec3::new(t.d(), t.d(), t.d());
            let v = base::Vec3::new(t.d(), t.d(), t.d());
            let s = t.d();
            let (p, q, r, w, c) = (u + v, u - v, u.scale(s), v.scale(s), base::cross(u, v));
            for x in p.0.iter().chain(q.0.iter()) {
                o.d(*x);
            }
            o.d(u.inner(v));
            for x in r.0.iter().chain(w.0.iter()) {
                o.d(*x);
            }
            o.f(base::dot(u, v));
            for x in c.0 {
                o.d(x);
            }
        }
        "view" => view_case(t, o),
        "fixarea" => {
            o.i(i64::from(aaa::get_fix_vertex_area_value(&cx.sg)));
            let v = t.i();
            o.i(i64::from(aaa::set_fix_vertex_area_value(&mut cx.sg, v)));
            o.i(i64::from(aaa::get_fix_vertex_area_value(&cx.sg)));
        }
        c => panic!("unknown command {c}"),
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
    let shared = Shared::default();
    let mut cx = Ctx {
        g: Globals::new(mode),
        sg: SurfGlobals::default(),
        a: shared.clone(),
        b: shared.clone(),
    };
    let mut t = Toks {
        t: line.split_whitespace().collect(),
        i: 0,
    };
    let mut o = Out::default();
    let res = case(&mut cx, &mut t, &mut o);
    let text = escape(&shared.0.borrow());
    let result = match res {
        Ok(()) => o.0.trim_end().to_owned(),
        // FS-03 reproduced: FreeSurfer's failed assertion faults on purpose (signal 11)
        Err(Fatal::Exit(139)) if mode.reproduce() && cx.g.notices.contains(&FS_03) => {
            "CRASH 11".to_owned()
        }
        Err(f) => format!("EXIT {}", f.status()),
    };
    let mut ids: Vec<&'static str> =
        cx.g.notices
            .as_slice()
            .iter()
            .map(|n| n.defect.id)
            .collect();
    ids.sort_unstable();
    ids.dedup();
    (format!("{result} | {text}"), ids)
}
