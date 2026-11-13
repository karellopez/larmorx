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

use larmorx_core::defects::{FS_03, FS_111, UpstreamDefects};
use larmorx_freesurfer_core::diag::{DIAG_SHOW, DIAG_VERBOSE};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::Matrix;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_surf::aaa::{self, MrisStatus, SurfGlobals};
use larmorx_freesurfer_surf::base;
use larmorx_freesurfer_surf::mris::{Face, Mris, Vertex, VertexTopology};
use larmorx_freesurfer_surf::mris_mp::MrisMp;
use larmorx_freesurfer_surf::mrisurf;
use larmorx_freesurfer_surf::surfbase::{self, MrisTempAssigned};
use larmorx_freesurfer_surf::topology;
use larmorx_freesurfer_surf::views::{self, MetricSurface, MpView};
use larmorx_freesurfer_surf::{coords, dist, metric};

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

const COMMANDS: [&str; 23] = [
    "coord", "vnum", "nbr", "modvnum", "norm", "close", "proj", "sign", "status", "sortcmp",
    "angles", "split", "vec3", "view", "fixarea", "addcmd", "vhash", "diff", "temps", "fpv",
    "names", "surf", "metric",
];

/// A byte string's token: `h` and the bytes in hex.
fn hx(t: &mut Vec<String>, b: &[u8]) {
    let mut s = String::from("h");
    for c in b {
        s += &format!("{c:02x}");
    }
    t.push(s);
}

/// Random bytes, `len` of them: printable mostly, a NUL or a high byte now and then.
fn bytes(g: &mut Gen, len: usize) -> Vec<u8> {
    (0..len)
        .map(|_| {
            if g.chance(30) {
                g.pick(&[0u8, 0xff, 0x80, b'\\', b'%'])
            } else {
                g.int(0x21, 0x7e) as u8
            }
        })
        .collect()
}

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

/// A small float: k/4 for k in [-400, 400], or a special one now and then.
fn small(g: &mut Gen) -> u32 {
    if g.chance(10) {
        f32bits(g)
    } else {
        (g.int(-400, 400) as f32 / 4.0).to_bits()
    }
}

fn surf_tokens(t: &mut Vec<String>, g: &mut Gen) {
    ix(t, i64::from(g.chance(3)));
    let nv = g.int(0, 5);
    let nf = if nv == 0 { 0 } else { g.int(0, 3) };
    ix(t, nv);
    ix(t, nf);
    for v in [g.int(0, 9), g.int(0, 9), g.int(0, 3), g.int(0, 3)] {
        ix(t, v);
    }
    let mut vsizes = Vec::new();
    for _ in 0..nv {
        for _ in 0..18 {
            fx(t, g);
        }
        ix(t, i64::from(g.chance(6)));
        let (vnum, v2num, v3num, vtotal) = (g.int(0, 5), g.int(0, 8), g.int(0, 10), g.int(0, 6));
        let nsize_max = g.int(0, 3);
        for v in [vnum, v2num, v3num, vtotal, nsize_max] {
            ix(t, v);
        }
        let num = g.int(0, 4);
        ix(t, num);
        for _ in 0..num {
            ix(t, g.int(-1, nf));
            ix(t, g.int(0, 2));
        }
        let count = match nsize_max {
            1 => vnum,
            2 => v2num,
            3 => v3num,
            _ => 0,
        };
        let vsize = count.max(vtotal);
        vsizes.push(vsize);
        for k in 0..2 {
            // no buffer at all only where growing it cannot keep values FreeSurfer never wrote
            // (the distances' first vtotal: memory realloc leaves undefined)
            if g.chance(4) && (k == 1 || vtotal == 0) {
                ix(t, -1);
                continue;
            }
            let len = vsize + g.int(0, 3);
            ix(t, len);
            ix(t, g.int(0, len));
            ix(t, i64::from(!g.chance(4)));
            for _ in 0..len {
                t.push(format!("{:08x}", small(g)));
            }
        }
    }
    for _ in 0..nf {
        for _ in 0..3 {
            ix(t, g.int(0, nv - 1));
        }
    }
    for _ in 0..9 {
        fx(t, g);
    }
    let nops = g.int(1, 5);
    ix(t, nops);
    let ops: &[&str] = if nv > 0 {
        &[
            "S", "O", "I", "E", "C", "F", "R", "M", "V", "W", "K", "P", "T", "Q", "A", "Z", "Y",
            "X", "H", "G", "N", "L", "U", "B", "D", "J", "Dm", "md", "mo", "gd", "go", "sd", "fd",
            "fo",
        ]
    } else {
        &[
            "E", "F", "R", "K", "T", "Q", "A", "Z", "H", "N", "L", "Dm", "fd", "fo",
        ]
    };
    let f3 = |t: &mut Vec<String>, g: &mut Gen| {
        for _ in 0..3 {
            t.push(format!("{:08x}", small(g)));
        }
    };
    for _ in 0..nops {
        let op = g.pick(ops);
        t.push(op.to_owned());
        match op {
            "S" | "O" => {
                ix(t, g.int(-1, nv));
                f3(t, g);
            }
            "I" => {
                for _ in 0..3 * nv {
                    t.push(format!("{:08x}", small(g)));
                }
            }
            "C" => {
                let n2 = if g.chance(6) { nv + 1 } else { nv };
                ix(t, n2);
                for _ in 0..3 * n2 {
                    t.push(format!("{:08x}", small(g)));
                }
            }
            "R" => ix(t, g.int(0, 1)),
            "M" => {
                ix(t, g.pick(&[4, 4, 0, 3, 7]));
                for _ in 0..3 * nv {
                    t.push(format!("{:08x}", small(g)));
                }
            }
            "V" => {
                ix(t, g.int(0, 3));
                ix(t, g.int(0, 1));
                ix(t, g.pick(&[4, 3, 0, 0, 5]));
            }
            "W" => {
                ix(t, g.int(0, 3));
                ix(t, g.int(0, 1));
            }
            "K" | "J" => {
                if g.chance(2) {
                    t.push(format!("{:016x}", (g.int(-16, 16) as f64 / 8.0).to_bits()));
                } else {
                    dx(t, g);
                }
            }
            "T" | "G" => f3(t, g),
            "Q" => {
                for _ in 0..6 {
                    if g.chance(2) {
                        t.push(format!("{:016x}", (g.int(-16, 16) as f64 / 8.0).to_bits()));
                    } else {
                        dx(t, g);
                    }
                }
            }
            "A" => {
                if g.chance(4) {
                    for _ in 0..3 {
                        t.push("3f800000".to_owned());
                    }
                } else {
                    f3(t, g);
                }
            }
            "Z" => {
                for _ in 0..3 {
                    let a = if g.chance(8) {
                        f32bits(g)
                    } else {
                        (g.int(-700, 700) as f32 / 100.0).to_bits()
                    };
                    t.push(format!("{a:08x}"));
                }
            }
            "X" => {
                let (r, c) = g.pick(&[(4, 4), (4, 4), (4, 4), (3, 4), (4, 3), (0, 0)]);
                ix(t, r);
                ix(t, c);
                for _ in 0..r * c {
                    t.push(format!("{:08x}", small(g)));
                }
            }
            "B" => {
                for _ in 0..2 {
                    t.push(format!("{:08x}", small(g)));
                }
            }
            "D" => t.push(format!("{:08x}", small(g))),
            "md" | "mo" => ix(t, g.int(0, nv - 1)),
            "gd" | "go" => {
                ix(t, g.int(0, nv - 1));
                ix(t, g.int(0, 8));
            }
            "sd" => {
                // a buffer at least the vertex's size in use (a later grow keeps its first
                // vtotal values)
                let vno = g.int(0, nv - 1);
                ix(t, vno);
                let cap = vsizes[vno as usize].max(1) + g.int(0, 3);
                ix(t, cap);
                ix(t, g.int(0, cap));
                for _ in 0..cap {
                    t.push(format!("{:08x}", small(g)));
                }
            }
            _ => {}
        }
    }
}

/// A small float, mostly: k/4 for k in [-400, 400] (a special one now and then), or any float;
/// with `heavy` a NaN (of both signs, with payloads) or an infinity one time in four, so that two
/// NaNs meet in one operation often enough to show the official build's operand order.
fn smallish(g: &mut Gen, heavy: bool) -> u32 {
    if heavy && g.chance(4) {
        return g.pick(&[
            0x7fc0_0000,
            0xffc0_0000,
            0x7fa0_0001,
            0xff80_0123,
            0x7fc0_0042,
            0xffd0_0007,
            0x7f80_0000,
            0xff80_0000,
        ]);
    }
    if g.chance(8) { f32bits(g) } else { small(g) }
}

/// `metric`'s case: the surface (15 coordinate sets, curvatures, areas, rip flags, neighbour
/// lists, distances with their storage, faces), then 1 to 6 operations.
fn metric_tokens(t: &mut Vec<String>, g: &mut Gen) {
    let heavy = g.chance(3);
    ix(t, i64::from(g.chance(3)));
    ix(t, g.int(0, 3));
    ix(t, i64::from(g.chance(4)));
    let nv = g.int(0, 6);
    let nf = if nv == 0 { 0 } else { g.int(0, 4) };
    ix(t, nv);
    ix(t, nf);
    for _ in 0..2 {
        // mostly a valid status (10 kinds), now and then one outside the list
        ix(
            t,
            if g.chance(12) {
                g.pick(&[10, -1])
            } else {
                g.int(0, 9)
            },
        );
    }
    ix(t, g.int(0, 3));
    for _ in 0..2 {
        ix(t, g.pick(&[0, 1, 2, 3, 1, 2, 1, -1, 4]));
    }
    ix(t, g.pick(&[1, 2, 3, 1, 1, 0, 4]));
    ix(t, i64::from(g.chance(4)));
    for _ in 0..4 {
        t.push(format!("{:08x}", smallish(g, heavy)));
    }
    let mut vsizes = Vec::new();
    for _ in 0..nv {
        for _ in 0..45 {
            t.push(format!("{:08x}", smallish(g, heavy)));
        }
        for _ in 0..6 {
            t.push(format!("{:08x}", smallish(g, heavy)));
        }
        ix(t, i64::from(g.chance(6)));
        let hi = nv.min(5);
        let (vnum, v2num, v3num, vtotal) = (
            g.int(0, hi),
            g.int(0, hi + 2),
            g.int(0, hi + 3),
            g.int(0, hi + 1),
        );
        let nsize_max = g.int(0, 3);
        for v in [vnum, v2num, v3num, vtotal, nsize_max] {
            ix(t, v);
        }
        let len = vnum.max(v2num).max(v3num).max(vtotal);
        ix(t, len);
        for _ in 0..len {
            ix(t, g.int(0, nv - 1));
        }
        let count = match nsize_max {
            1 => vnum,
            2 => v2num,
            3 => v3num,
            _ => 0,
        };
        let vsize = count.max(vtotal);
        vsizes.push(vsize);
        for _ in 0..2 {
            if g.chance(5) {
                ix(t, -1);
                continue;
            }
            let blen = (vsize + g.int(-1, 3)).max(0);
            ix(t, blen);
            ix(t, if g.chance(3) { blen } else { g.int(0, blen) });
            ix(t, i64::from(!g.chance(4)));
            for _ in 0..blen {
                t.push(format!("{:08x}", small(g)));
            }
        }
    }
    for _ in 0..nf {
        for _ in 0..3 {
            ix(t, g.int(0, nv - 1));
        }
        t.push(format!("{:08x}", small(g)));
        ix(t, i64::from(g.chance(5)));
    }
    let nops = g.int(1, 6);
    ix(t, nops);
    let mut ops: Vec<&str> = vec![
        "av", "sm", "cs", "co", "sc", "kd", "ko", "vd", "vo", "vn", "pe", "ca", "cv", "zn", "ex",
        "cc", "c2", "rc", "sk", "sh", "zm", "nc", "nv", "cp", "sH", "mm", "sx", "lx", "px", "lb",
    ];
    if nf > 0 {
        ops.extend(["ar", "lo", "lt"]);
    }
    // FS-114's case (no vertex) seldom: the official build never returns from it (the probe's
    // alarm ends it after 5 seconds)
    if nv == 0 && !g.chance(10) {
        ops.retain(|o| *o != "cs");
    }
    for _ in 0..nops {
        let op = g.pick(&ops);
        t.push(op.to_owned());
        match op {
            "av" | "sh" => ix(t, g.int(-1, 4)),
            "sm" => {
                ix(t, g.int(-1, 4));
                let s = if g.chance(2) {
                    g.pick(&[0.5f32, 1.0, 1.5, 0.25]).to_bits()
                } else {
                    smallish(g, heavy)
                };
                t.push(format!("{s:08x}"));
            }
            "sc" => t.push(format!("{:08x}", smallish(g, heavy))),
            "ar" => {
                ix(t, g.int(0, nf - 1));
                ix(t, g.int(0, 2));
            }
            "lo" => ix(t, g.int(0, nf - 1)),
            "lt" => {
                ix(t, g.int(0, nf - 1));
                ix(t, g.pick(&[4, 9, 7, 0, 3]));
            }
            "ex" => ix(t, g.pick(&[4, 10, 9, 11, 7, 5, 6, 3, 0, 8, 2, 1, 12, -1])),
            "sk" => {
                t.push(format!("{:08x}", smallish(g, heavy)));
                t.push(format!("{:08x}", smallish(g, heavy)));
            }
            "nc" => ix(t, g.pick(&[-1, 0, 1, 1, 2])),
            "cp" => {
                if g.chance(3) {
                    dx(t, g);
                } else {
                    t.push(format!("{:016x}", (g.int(0, 400) as f64 / 8.0).to_bits()));
                }
            }
            _ => {}
        }
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
            "addcmd" => {
                let n0 = if g.chance(5) {
                    g.int(1000, 1003)
                } else if g.chance(4) {
                    g.int(990, 999)
                } else {
                    g.int(0, 999)
                };
                ix(&mut t, n0);
                let len = g.int(0, 40) as usize;
                let b = bytes(&mut g, len);
                hx(&mut t, &b);
            }
            "vhash" => {
                ix(&mut t, g.int(0, 1));
                let (vnum, v2num, v3num) = (g.int(-3, 12), g.int(-3, 12), g.int(-3, 12));
                let vtotal = g.int(0, 12);
                let nsize_max = g.pick(&[0, 1, 2, 3, 4, 255]);
                let num = g.int(0, 8);
                for v in [
                    vnum,
                    v2num,
                    v3num,
                    vtotal,
                    g.int(-5, 5),
                    nsize_max,
                    g.int(0, 3),
                    num,
                ] {
                    ix(&mut t, v);
                }
                let count = match nsize_max {
                    1 => vnum,
                    2 => v2num,
                    3 => v3num,
                    _ => 0,
                };
                let vsize = count.max(vtotal);
                let nv = if g.chance(5) { -1 } else { vsize + g.int(0, 2) };
                ix(&mut t, nv);
                for _ in 0..nv.max(0) {
                    let x = if g.chance(8) {
                        g.next() as i32 as i64
                    } else {
                        g.int(-5, 1000)
                    };
                    ix(&mut t, x);
                }
                let nf = if g.chance(5) { -1 } else { num + g.int(0, 2) };
                ix(&mut t, nf);
                for _ in 0..nf.max(0) {
                    let x = if g.chance(8) {
                        g.next() as i32 as i64
                    } else {
                        g.int(-1, 2000)
                    };
                    ix(&mut t, x);
                    ix(&mut t, g.int(0, 255));
                }
                for _ in 0..2 {
                    let nd = if g.chance(4) { -1 } else { vsize + g.int(0, 2) };
                    ix(&mut t, nd);
                    for _ in 0..nd.max(0) {
                        fx(&mut t, &mut g);
                    }
                }
                coords(&mut t, &mut g);
                fx(&mut t, &mut g);
                fx(&mut t, &mut g);
                ix(&mut t, g.int(-1, 1));
                ix(&mut t, g.int(0, 3));
            }
            "diff" => {
                ix(&mut t, if g.chance(2) { -1 } else { g.int(0, 5) });
                let nv1 = g.int(1, 5);
                let nv2 = if g.chance(8) { g.int(1, 5) } else { nv1 };
                let nf1 = g.int(0, 3);
                let nf2 = if g.chance(8) { g.int(0, 3) } else { nf1 };
                for v in [nv1, nv2, nf1, nf2] {
                    ix(&mut t, v);
                }
                for _ in 0..nv1.min(nv2) {
                    let vnum = g.int(0, 6);
                    let rip = i64::from(g.chance(6));
                    let a: [u32; 3] = [f32bits(&mut g), f32bits(&mut g), f32bits(&mut g)];
                    ix(&mut t, vnum);
                    ix(&mut t, rip);
                    for x in a {
                        t.push(format!("{x:08x}"));
                    }
                    fx(&mut t, &mut g);
                    ix(&mut t, if g.chance(8) { g.int(0, 6) } else { vnum });
                    ix(&mut t, if g.chance(8) { 1 - rip } else { rip });
                    for x in a {
                        let b = if g.chance(3) {
                            x
                        } else if g.chance(2) {
                            (f32::from_bits(x) + g.int(-8, 8) as f32 / 16.0).to_bits()
                        } else {
                            f32bits(&mut g)
                        };
                        t.push(format!("{b:08x}"));
                    }
                    fx(&mut t, &mut g);
                }
                ix(&mut t, g.int(-1, 3));
                if g.chance(3) {
                    t.push(format!(
                        "{:016x}",
                        g.pick(&[0.0f64, 0.25, 0.5, 1.0]).to_bits()
                    ));
                } else {
                    dx(&mut t, &mut g);
                }
                ix(&mut t, g.int(0, 1));
            }
            "temps" => {
                ix(&mut t, g.int(0, 7));
                let n = g.int(1, 6);
                ix(&mut t, n);
                for _ in 0..n {
                    t.push(g.pick(&["A", "C", "R"]).to_owned());
                    ix(&mut t, g.int(0, 2));
                }
            }
            "fpv" => {
                ix(&mut t, g.int(-3, 50));
                ix(&mut t, g.int(-2, 3));
                ix(&mut t, g.int(0, 4));
            }
            "surf" => surf_tokens(&mut t, &mut g),
            "metric" => metric_tokens(&mut t, &mut g),
            "names" => {
                let n = g.int(1, 5);
                ix(&mut t, n);
                for _ in 0..n {
                    let op = g.pick(&["I", "S", "O"]);
                    t.push(op.to_owned());
                    // long names printable only (a NUL would end them early)
                    let b = if op == "I" && g.chance(3) {
                        let len = g.pick(&[1019, 1020, 1021, 1022, 1023, 1024, 1025, 1500, 2100]);
                        (0..len).map(|_| g.int(0x21, 0x7e) as u8).collect()
                    } else {
                        let len = g.int(0, 30) as usize;
                        bytes(&mut g, len)
                    };
                    hx(&mut t, &b);
                }
            }
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
    /// A byte token (`h` and hex).
    fn h(&mut self) -> Vec<u8> {
        let t = self.next().strip_prefix('h').expect("a byte token");
        (0..t.len() / 2)
            .map(|i| u8::from_str_radix(&t[2 * i..2 * i + 2], 16).expect("hex"))
            .collect()
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
    /// A C string as the probe prints it: `NULL`, or `h` and its bytes before a NUL in hex.
    fn h(&mut self, v: Option<&[u8]>) {
        match v {
            None => self.0 += "NULL ",
            Some(b) => {
                self.0 += "h";
                for c in b.iter().take_while(|&&c| c != 0) {
                    self.0 += &format!("{c:02x}");
                }
                self.0 += " ";
            }
        }
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
        dist_storage: Some(vec![None; nv]),
        dist_orig_storage: Some(vec![None; nv]),
        nsize: 1,
        max_nsize: 1,
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

fn dump_surface(m: &Mris, o: &mut Out) {
    for vno in 0..m.nvertices as usize {
        let v = m.vertex(vno);
        for x in [v.x, v.y, v.z, v.origx, v.origy, v.origz, v.cx, v.cy, v.cz] {
            o.f(x);
        }
        for k in 0..2 {
            let (d, cap, st) = if k == 1 {
                (
                    &v.dist_orig,
                    v.dist_orig_capacity,
                    &m.dist_orig_storage.as_ref().unwrap()[vno],
                )
            } else {
                (
                    &v.dist,
                    v.dist_capacity,
                    &m.dist_storage.as_ref().unwrap()[vno],
                )
            };
            o.i(i64::from(d.is_some()));
            o.i(i64::from(d.is_some() || st.is_some()));
            o.i(i64::from(cap));
            if let Some(d) = d {
                for x in &d[..cap as usize] {
                    o.f(*x);
                }
            }
        }
        let vt = m.topology(vno);
        for k in 0..usize::from(vt.num) {
            o.i(i64::from(vt.n.as_ref().unwrap()[k]));
        }
    }
    for fno in 0..m.nfaces as usize {
        for k in 0..3 {
            o.i(i64::from(m.face(fno).v[k]));
        }
    }
    o.i(i64::from(m.dist_alloced_flags));
    o.i(i64::from(m.dist_nsize));
    o.i(i64::from(m.origxyz_status.0));
    o.i(i64::from(m.status.0));
    for x in [
        m.xlo, m.ylo, m.zlo, m.xhi, m.yhi, m.zhi, m.xctr, m.yctr, m.zctr,
    ] {
        o.f(x);
    }
}

fn surf_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let env = if t.i() != 0 {
        Environment::from_pairs([("FREESURFER_reportLogicErrors", "1")])
    } else {
        Environment::from_pairs::<&str, &str>([])
    };
    let (nv, nf) = (t.i(), t.i());
    let mut m = mris_alloc(nv as usize, nf as usize);
    m.status = MrisStatus(t.i());
    m.origxyz_status = MrisStatus(t.i());
    m.dist_alloced_flags = t.i() as i8;
    m.dist_nsize = t.i() as i8;
    for vno in 0..nv as usize {
        {
            let v = m.vertex_mut(vno);
            let mut f = || t.f();
            (v.x, v.y, v.z) = (f(), f(), f());
            (v.origx, v.origy, v.origz) = (f(), f(), f());
            (v.cx, v.cy, v.cz) = (f(), f(), f());
            (v.tx, v.ty, v.tz) = (f(), f(), f());
            (v.nx, v.ny, v.nz) = (f(), f(), f());
            (v.dx, v.dy, v.dz) = (f(), f(), f());
            v.ripflag = t.i() as i8;
        }
        {
            let vt = m.topology_mut(vno);
            vt.vnum = t.i() as i16;
            vt.v2num = t.i() as i16;
            vt.v3num = t.i() as i16;
            vt.vtotal = t.i() as i16;
            vt.nsizeMax = t.i() as u8;
            let num = t.i();
            vt.num = num as u8;
            let (mut f, mut n) = (Vec::new(), Vec::new());
            for _ in 0..num {
                f.push(t.i());
                n.push(t.i() as u8);
            }
            vt.f = Some(f);
            vt.n = Some(n);
        }
        for k in 0..2 {
            let len = t.i();
            if len < 0 {
                continue;
            }
            let (cap, valid) = (t.i(), t.i() != 0);
            let buf: Vec<f32> = (0..len).map(|_| t.f()).collect();
            let v = m.vertex_mut(vno);
            let (d, c) = if k == 1 {
                (&mut v.dist_orig, &mut v.dist_orig_capacity)
            } else {
                (&mut v.dist, &mut v.dist_capacity)
            };
            if valid {
                *d = Some(buf);
                *c = cap;
            } else {
                let st = if k == 1 {
                    &mut m.dist_orig_storage
                } else {
                    &mut m.dist_storage
                };
                st.as_mut().unwrap()[vno] = Some(buf);
            }
        }
    }
    for fno in 0..nf as usize {
        for k in 0..3 {
            m.face_mut(fno).v[k] = t.i();
        }
    }
    (m.xlo, m.ylo, m.zlo, m.xhi, m.yhi, m.zhi) = (t.f(), t.f(), t.f(), t.f(), t.f(), t.f());
    (m.xctr, m.yctr, m.zctr) = (t.f(), t.f(), t.f());
    let nops = t.i();
    let g = &mut cx.g;
    let mut io = coords::Io {
        out: &mut cx.a,
        err: &mut cx.b,
        env: &env,
    };
    let nvu = nv as usize;
    let three = |t: &mut Toks, n: usize| -> Vec<f32> { (0..3 * n).map(|_| t.f()).collect() };
    for _ in 0..nops {
        match t.next() {
            "S" => {
                let vno = t.i();
                let (x, y, z) = (t.f(), t.f(), t.f());
                coords::set_xyz(g, &mut io, &mut m, vno, x, y, z, "probe.cpp", 7)?;
            }
            "O" => {
                let vno = t.i();
                let (x, y, z) = (t.f(), t.f(), t.f());
                coords::set_original_xyz(g, &mut io, &mut m, vno, x, y, z, "probe.cpp", 8)?;
            }
            "I" => {
                let a = three(t, nvu);
                coords::import_xyz(
                    g,
                    &mut io,
                    &mut m,
                    &a[..nvu],
                    &a[nvu..2 * nvu],
                    &a[2 * nvu..],
                )?;
            }
            "E" => {
                let (px, py, pz) = coords::export_xyz(g, &mut io, &m)?;
                for i in 0..nvu {
                    o.f(px[i]);
                    o.f(py[i]);
                    o.f(pz[i]);
                }
            }
            "C" => {
                let n2 = t.i() as usize;
                let mut b = mris_alloc(n2, 0);
                for i in 0..n2 {
                    let v = b.vertex_mut(i);
                    (v.x, v.y, v.z) = (t.f(), t.f(), t.f());
                }
                coords::copy_xyz(g, &mut io, &mut m, &b)?;
            }
            "F" => coords::set_original_xyz_from_xyz(&mut m),
            "R" => {
                let (avg, sd) = coords::avg_vertex_radius(&m, t.i() != 0);
                o.d(avg);
                o.d(sd.unwrap_or(0.0));
            }
            "M" => {
                let which = t.i();
                let a = three(t, nvu);
                let loc = [&a[..nvu], &a[nvu..2 * nvu], &a[2 * nvu..]];
                let r = coords::import_vertex_coords(g, &mut io, &mut m, loc, which)?;
                o.i(i64::from(r));
            }
            "V" => {
                let (d, r, w) = (t.i(), t.i(), t.i());
                o.i(i64::from(coords::reverse_coords(
                    g, &mut io, &mut m, d, r, w,
                )?));
            }
            "W" => {
                let (w, r) = (t.i(), t.i());
                o.i(i64::from(coords::reverse(g, &mut io, &mut m, w, r)?));
            }
            "K" => {
                let s = t.d();
                o.i(i64::from(coords::scale(g, &mut m, s)));
            }
            "P" => o.i(i64::from(coords::flip_patch(&mut m))),
            "T" => {
                let (a, b, c) = (t.f(), t.f(), t.f());
                o.i(i64::from(coords::translate(&mut m, a, b, c)));
            }
            "Q" => {
                let w: Vec<f64> = (0..6).map(|_| t.d()).collect();
                coords::scale_then_translate(g, &mut m, w[0], w[1], w[2], w[3], w[4], w[5]);
            }
            "A" => {
                let (a, b, c) = (t.f(), t.f(), t.f());
                o.i(i64::from(coords::anisotropic_scale(&mut m, a, b, c)));
            }
            "Z" => {
                let (a, b, c) = (t.f(), t.f(), t.f());
                coords::rotate(g, &mut m, a, b, c);
            }
            "Y" => return Err(coords::rotate_mp(g, &mut io)),
            "X" => {
                let (r, c) = (t.i(), t.i());
                let mat = (r > 0).then(|| {
                    let mut mm = Matrix::real(r, c);
                    for i in 1..=r {
                        for j in 1..=c {
                            mm.set_relt(i, j, t.f());
                        }
                    }
                    mm
                });
                o.i(i64::from(coords::matrix_multiply(
                    g,
                    &mut io,
                    &mut m,
                    mat.as_ref(),
                )?));
            }
            "H" => o.i(i64::from(coords::spherical_coords(&mut m))),
            "G" => {
                let (a, b, c) = (t.f(), t.f(), t.f());
                coords::move_origin(&mut m, a, b, c);
            }
            "N" => coords::center(&mut m),
            "L" => {
                let (a, b, c) = coords::center_cog2(&mut m);
                o.d(a);
                o.d(b);
                o.d(c);
            }
            "U" => coords::center_cog(&mut m),
            "B" => {
                let (a, b) = (t.f(), t.f());
                coords::blend_xyz_and_txyz(&mut m, a, b);
            }
            "D" => {
                let a = t.f();
                coords::blend_xyz_and_nxyz(&mut m, a);
            }
            "J" => {
                let dt = t.d();
                coords::translate_along_vertex_dxdydz(&mut m, dt);
            }
            "Dm" => o.i(i64::from(coords::compute_surface_dimensions(&mut m))),
            "md" => dist::make_dist(&mut m, t.i() as usize),
            "mo" => dist::make_dist_orig(&mut m, t.i() as usize),
            "gd" => {
                let (vno, min) = (t.i() as usize, t.i());
                dist::grow_dist(&mut m, vno, min);
            }
            "go" => {
                let (vno, min) = (t.i() as usize, t.i());
                dist::grow_dist_orig(&mut m, vno, min);
            }
            "sd" => {
                let (vno, cap, ncap) = (t.i() as usize, t.i(), t.i());
                let mut p = dist::steal_dist_store(&mut m, vno, cap);
                for x in p.iter_mut() {
                    *x = t.f();
                }
                dist::set_dist(g, io.out, io.err, &mut m, vno, p, ncap)?;
            }
            "fd" => dist::free_dists_but_not_orig(&mut m),
            "fo" => dist::free_dist_origs(&mut m),
            op => panic!("surf op {op}"),
        }
    }
    o.0 += "| ";
    dump_surface(&m, o);
    Ok(())
}

/// The probe's `G` test: whether an operation's reads and writes stay inside what the case
/// allocated (FreeSurfer reads and writes the distances unchecked).
fn metric_guard(m: &Mris, op: &str) -> bool {
    let vnum_at = |vt: &VertexTopology, n: i8| -> i32 {
        i32::from(match n {
            1 => vt.vnum,
            2 => vt.v2num,
            3 => vt.v3num,
            _ => 0,
        })
    };
    for vno in 0..m.nvertices.max(0) as usize {
        let v = m.vertex(vno);
        let vt = m.topology(vno);
        if v.ripflag != 0 {
            continue;
        }
        let vtotal = i32::from(vt.vtotal);
        match op {
            "co" if vtotal > 0 && (v.dist_orig.is_none() || v.dist_orig_capacity < vtotal) => {
                return false;
            }
            "sc" => {
                let vs = topology::vertex_vsize(m, vno);
                if vs > 0 && (v.dist.is_none() || v.dist_capacity < vs) {
                    return false;
                }
            }
            "kd" if (1..=3).contains(&m.dist_nsize)
                && v.dist.is_some()
                && v.dist_capacity < vnum_at(vt, m.dist_nsize) =>
            {
                return false;
            }
            "ko" if (1..=3).contains(&m.dist_orig_nsize)
                && v.dist_orig.is_some()
                && v.dist_orig_capacity < vnum_at(vt, m.dist_orig_nsize) =>
            {
                return false;
            }
            "pe" if vtotal > 0
                && ((v.dist.is_some() && v.dist_capacity < vtotal)
                    || (v.dist_orig.is_some() && v.dist_orig_capacity < vtotal)) =>
            {
                return false;
            }
            _ => {}
        }
    }
    true
}

fn dump_metric(m: &Mris, o: &mut Out) {
    for vno in 0..m.nvertices.max(0) as usize {
        let v = m.vertex(vno);
        for x in [
            v.x, v.y, v.z, v.origx, v.origy, v.origz, v.curv, v.H, v.val2, v.d, v.area, v.origarea,
        ] {
            o.f(x);
        }
        for k in 0..2 {
            let (d, cap, st) = if k == 1 {
                (
                    &v.dist_orig,
                    v.dist_orig_capacity,
                    &m.dist_orig_storage.as_ref().unwrap()[vno],
                )
            } else {
                (
                    &v.dist,
                    v.dist_capacity,
                    &m.dist_storage.as_ref().unwrap()[vno],
                )
            };
            o.i(i64::from(d.is_some()));
            o.i(i64::from(d.is_some() || st.is_some()));
            o.i(i64::from(cap));
            if let Some(d) = d {
                for x in &d[..cap as usize] {
                    o.f(*x);
                }
            }
        }
    }
    for fno in 0..m.nfaces.max(0) as usize {
        o.f(m.face(fno).area);
    }
    for x in [
        i64::from(m.dist_alloced_flags),
        i64::from(m.dist_nsize),
        i64::from(m.dist_orig_nsize),
        // the enum's storage: an unsigned int
        i64::from(m.status.0 as u32),
        i64::from(m.origxyz_status.0 as u32),
    ] {
        o.i(x);
    }
    for x in [
        m.max_curv, m.min_curv, m.xlo, m.ylo, m.zlo, m.xhi, m.yhi, m.zhi, m.xctr, m.yctr, m.zctr,
    ] {
        o.f(x);
    }
}

fn metric_case(cx: &mut Ctx, t: &mut Toks, o: &mut Out) -> Result<(), Fatal> {
    let mut pairs: Vec<(&str, &str)> = Vec::new();
    if t.i() != 0 {
        pairs.push(("FREESURFER_reportLogicErrors", "1"));
    }
    let fixes = t.i();
    if fixes & 1 != 0 {
        pairs.push(("FREESURFER_fix_MRISsmoothSurface1", "1"));
    }
    if fixes & 2 != 0 {
        pairs.push(("FREESURFER_fix_missing_dist_orig", "1"));
    }
    let env = Environment::from_pairs(pairs);
    if t.i() != 0 {
        cx.g.gdiag |= DIAG_SHOW | DIAG_VERBOSE;
    }
    let (nv, nf) = (t.i(), t.i());
    let mut m = mris_alloc(nv as usize, nf as usize);
    m.status = MrisStatus(t.i());
    m.origxyz_status = MrisStatus(t.i());
    m.dist_alloced_flags = t.i() as i8;
    m.dist_nsize = t.i() as i8;
    m.dist_orig_nsize = t.i() as i8;
    m.nsize = t.i() as i8;
    m.patch = t.i();
    (m.total_area, m.orig_area, m.max_curv, m.min_curv) = (t.f(), t.f(), t.f(), t.f());
    for vno in 0..nv as usize {
        {
            let v = m.vertex_mut(vno);
            set_coords(t, v);
            (v.curv, v.H, v.val2, v.d, v.area, v.origarea) =
                (t.f(), t.f(), t.f(), t.f(), t.f(), t.f());
            v.ripflag = t.i() as i8;
        }
        {
            let vt = m.topology_mut(vno);
            vt.vnum = t.i() as i16;
            vt.v2num = t.i() as i16;
            vt.v3num = t.i() as i16;
            vt.vtotal = t.i() as i16;
            vt.nsizeMax = t.i() as u8;
            let len = t.i();
            vt.v = Some((0..len).map(|_| t.i()).collect());
        }
        for k in 0..2 {
            let len = t.i();
            if len < 0 {
                continue;
            }
            let (cap, valid) = (t.i(), t.i() != 0);
            let buf: Vec<f32> = (0..len).map(|_| t.f()).collect();
            let v = m.vertex_mut(vno);
            let (d, c) = if k == 1 {
                (&mut v.dist_orig, &mut v.dist_orig_capacity)
            } else {
                (&mut v.dist, &mut v.dist_capacity)
            };
            if valid {
                *d = Some(buf);
                *c = cap;
            } else {
                let st = if k == 1 {
                    &mut m.dist_orig_storage
                } else {
                    &mut m.dist_storage
                };
                st.as_mut().unwrap()[vno] = Some(buf);
            }
        }
    }
    for fno in 0..nf as usize {
        let f = m.face_mut(fno);
        for k in 0..3 {
            f.v[k] = t.i();
        }
        f.area = t.f();
        f.ripflag = t.i() as i8;
    }
    let nops = t.i();
    let g = &mut cx.g;
    let mut io = coords::Io {
        out: &mut cx.a,
        err: &mut cx.b,
        env: &env,
    };
    let nvu = nv as usize;
    let mut saved: Option<metric::SavedXyz> = None;
    for _ in 0..nops {
        let op = t.next();
        if matches!(op, "co" | "sc" | "kd" | "ko" | "pe") && !metric_guard(&m, op) {
            o.s("G");
            if op == "sc" {
                t.f();
            }
            continue;
        }
        match op {
            "av" => {
                let n = t.i();
                o.i(i64::from(metric::average_vertex_positions(
                    g, &mut io, &mut m, n,
                )?));
            }
            "sm" => {
                let (n, st) = (t.i(), t.f());
                metric::smooth_surface_positions(g, &mut io, &mut m, n, st)?;
                // the stub's MRIScomputeMetricProperties
                o.s("MP");
            }
            "cs" => metric::center_sphere(g, &mut io, &mut m)?,
            "co" => o.i(i64::from(metric::clear_orig_distances(&mut m))),
            "sc" => {
                let s = t.f();
                o.i(i64::from(metric::scale_distances(&mut m, s)));
            }
            "kd" => o.i(i64::from(metric::check_dist(g, &mut io, &mut m)?)),
            "ko" => o.i(i64::from(metric::check_dist_orig(g, &mut io, &mut m)?)),
            "vd" => o.i(i64::from(metric::compute_vertex_distances(
                g, &mut io, &mut m,
            )?)),
            "vo" => o.i(i64::from(metric::compute_original_vertex_distances(
                g, &mut io, &mut m,
            )?)),
            "vn" => metric::compute_original_vertex_distances_if_necessary(g, &mut io, &mut m)?,
            "pe" => o.d(metric::percent_distance_error(g, &mut io, &m)?),
            "ca" => o.i(i64::from(metric::clear_orig_area(&mut m))),
            "cv" => metric::clear_orig_area_and_val2(&mut m),
            "ar" => {
                let (fac, n) = (t.i() as usize, t.i() as usize);
                o.f(metric::compute_area(&m, fac, n));
            }
            "zn" => o.i(i64::from(metric::zero_negative_areas(&mut m))),
            "lo" | "lt" => {
                let fno = t.i() as usize;
                let u = if op == "lo" {
                    metric::load_orig_triangle_vertices(&m, fno)
                } else {
                    let which = t.i();
                    metric::load_triangle_vertices(&m, fno, which)
                };
                o.i(0);
                for row in u {
                    for x in row {
                        o.d(x);
                    }
                }
            }
            "ex" => {
                let which = t.i();
                let mut loc = [vec![-7.0f32; nvu], vec![-7.0f32; nvu], vec![-7.0f32; nvu]];
                let r = metric::extract_vertex_coords(g, &mut io, &m, &mut loc, which)?;
                o.i(i64::from(r));
                for a in &loc {
                    for x in a {
                        o.f(*x);
                    }
                }
            }
            "cc" => o.i(i64::from(metric::clear_curvature(&mut m))),
            "c2" => metric::clear_curv_and_val2(&mut m),
            "rc" => o.i(i64::from(metric::rectify_curvature(&mut m))),
            "sk" => {
                let (a, b) = (t.f(), t.f());
                o.i(i64::from(metric::scale_curvatures(&mut m, a, b)));
            }
            "sh" => {
                let n = t.i();
                o.i(i64::from(metric::smooth_curvatures(&mut m, n)));
            }
            "zm" => o.i(i64::from(metric::zero_mean_curvature(g, &mut io, &mut m))),
            "nc" => {
                let w = t.i();
                if w == metric::NORM_MEDIAN && nv == 0 {
                    o.s("G");
                    continue;
                }
                o.i(i64::from(metric::normalize_curvature(&mut io, &mut m, w)));
            }
            "nv" => o.i(i64::from(metric::normalize_curvature_variance(
                g, &mut io, &mut m,
            ))),
            "cp" => {
                let d = t.d();
                o.i(i64::from(metric::count_compressed(&m, d)));
            }
            "sH" => o.i(i64::from(metric::store_mean_curvature(&mut m))),
            "mm" => o.i(i64::from(metric::compute_curvature_min_max(&mut m))),
            "sx" => saved = Some(metric::save_xyz(&m)),
            "lx" => match &saved {
                Some(sv) => metric::load_xyz(g, &mut io, &mut m, sv)?,
                None => o.s("N"),
            },
            "px" => {
                if saved.is_some() {
                    metric::pop_xyz(g, &mut io, &mut m, &mut saved)?;
                } else {
                    o.s("N");
                }
                o.i(i64::from(saved.is_some()));
            }
            "lb" => {
                let b = mris_alloc(nvu + 1, 0);
                let sb = metric::save_xyz(&b);
                metric::load_xyz(g, &mut io, &mut m, &sb)?;
            }
            op => panic!("metric op {op}"),
        }
    }
    o.0 += "| ";
    dump_metric(&m, o);
    Ok(())
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
        "addcmd" => {
            let mut m = mris_alloc(1, 0);
            let n0 = t.i();
            let line = t.h();
            m.ncmds = n0;
            let r = mrisurf::add_command_line(&mut cx.g, &mut cx.b, &mut m, &line);
            o.i(i64::from(r));
            if n0 < aaa::MAX_CMDS as i32 {
                o.i(i64::from(m.ncmds));
                o.h(m.cmdlines[n0 as usize].as_deref());
            }
        }
        "vhash" => {
            let mut m = mris_alloc(2, 0);
            let vno = t.i() as usize;
            {
                let vt = m.topology_mut(vno);
                vt.vnum = t.i() as i16;
                vt.v2num = t.i() as i16;
                vt.v3num = t.i() as i16;
                vt.vtotal = t.i() as i16;
                vt.nsizeMaxClock = t.i() as i16;
                vt.nsizeMax = t.i() as u8;
                vt.nsizeCur = t.i() as u8;
                vt.num = t.i() as u8;
                let nv = t.i();
                vt.v = (nv >= 0).then(|| (0..nv).map(|_| t.i()).collect());
                let nf = t.i();
                if nf >= 0 {
                    let (mut f, mut n) = (Vec::new(), Vec::new());
                    for _ in 0..nf {
                        f.push(t.i());
                        n.push(t.i() as u8);
                    }
                    vt.f = Some(f);
                    vt.n = Some(n);
                }
            }
            let v = m.vertex_mut(vno);
            for k in 0..2 {
                let nd = t.i();
                let d = (nd >= 0).then(|| (0..nd).map(|_| t.f()).collect());
                if k == 0 {
                    v.dist = d;
                } else {
                    v.dist_orig = d;
                }
            }
            set_coords(t, v);
            v.val = t.f();
            v.curv = t.f();
            v.ripflag = t.i() as i8;
            v.marked = t.i() as i16;
            o.i(i64::from(topology::vertex_vsize(&m, vno)));
            // the hash, an unsigned long, printed in decimal
            o.0 += &format!("{} ", mrisurf::vertex_hash(&m, vno).hash);
        }
        "diff" => {
            cx.g.gdiag_no = t.i();
            let (nv1, nv2, nf1, nf2) = (t.i(), t.i(), t.i(), t.i());
            let mut a = mris_alloc(nv1 as usize, nf1 as usize);
            let mut b = mris_alloc(nv2 as usize, nf2 as usize);
            let nv = nv1.min(nv2) as usize;
            for k in 0..nv {
                for m in [&mut a, &mut b] {
                    m.topology_mut(k).vnum = t.i() as i16;
                    let v = m.vertex_mut(k);
                    v.ripflag = t.i() as i8;
                    v.x = t.f();
                    v.y = t.f();
                    v.z = t.f();
                    v.val = t.f();
                }
            }
            let ndiffmin = t.i();
            let rmsthresh = t.d();
            let verbosity = t.i();
            let r =
                mrisurf::diff_simple(&cx.g, &mut cx.a, &mut a, &b, ndiffmin, rmsthresh, verbosity);
            o.i(i64::from(r));
            for k in 0..nv {
                o.f(a.vertex(k).val);
            }
        }
        "temps" => {
            let mut m = mris_alloc(1, 0);
            m.tempsAssigned = t.i();
            let n = t.i();
            for _ in 0..n {
                let op = t.next();
                let temp = match t.i() {
                    0 => MrisTempAssigned::VertexMarked,
                    1 => MrisTempAssigned::VertexMarked2,
                    2 => MrisTempAssigned::End,
                    x => panic!("temp {x}"),
                };
                match op {
                    "A" => {
                        let r =
                            surfbase::acquire_temp(&mut cx.g, &mut cx.a, &mut cx.b, &mut m, temp)?;
                        o.i(i64::from(r));
                    }
                    "C" => surfbase::check_acquired_temp(
                        &mut cx.g, &mut cx.a, &mut cx.b, &m, temp, 123,
                    )?,
                    "R" => {
                        surfbase::release_temp(&mut cx.g, &mut cx.a, &mut cx.b, &mut m, temp, 123)?
                    }
                    _ => panic!("temps op"),
                }
                o.i(i64::from(m.tempsAssigned));
            }
        }
        "fpv" => {
            let mut m = mris_alloc(1, 0);
            m.nvertices = t.i();
            m.nverticesFrozen = t.i();
            let n = t.i();
            let mut ps = Vec::new();
            for _ in 0..n {
                ps.push(surfbase::make_float_per_vertex(&mut m));
                o.i(i64::from(ps.last().expect("one").is_some()));
                o.i(i64::from(m.nverticesFrozen));
            }
            for p in &mut ps {
                surfbase::free_float_per_vertex(&mut m, p);
                o.i(i64::from(p.is_some()));
                o.i(i64::from(m.nverticesFrozen));
            }
        }
        "surf" => surf_case(cx, t, o)?,
        "metric" => metric_case(cx, t, o)?,
        "names" => {
            let n = t.i();
            for _ in 0..n {
                let op = t.next();
                let name = t.h();
                let r = match op {
                    "I" => {
                        surfbase::set_inflated_file_name(&mut cx.g, &mut cx.sg, &mut cx.b, &name)?
                    }
                    "S" => surfbase::set_sulc_file_name(&mut cx.sg, &name),
                    "O" => surfbase::set_original_file_name(&mut cx.sg, &name),
                    _ => panic!("names op"),
                };
                o.i(i64::from(r));
            }
            for k in 0..3 {
                o.h(cx.sg.surface_names[k].as_deref());
                o.h(cx.sg.curvature_names[k].as_deref());
            }
            o.d(cx.sg.neg_area_k);
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
        // FS-111 reproduced: the official build's buffer check aborts (signal 6)
        Err(Fatal::Exit(134)) if mode.reproduce() && cx.g.notices.contains(&FS_111) => {
            "CRASH 6".to_owned()
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
