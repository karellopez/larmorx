// SPDX-License-Identifier: Apache-2.0
//! The minimizers' commands (README.md of the probe folder, "Commands" and "The test costs";
//! spec §13): `powell`, `powell2`, `dfpmin` with the probe's four test costs, their counts and
//! their FNV-1a hash of every call.

// The costs index p[1..=n] as the probe writes them.
#![allow(clippy::needless_range_loop)]

use std::cell::RefCell;

use larmorx_fs_core::process::DIAG_VERBOSE;
use larmorx_fs_core::sse::{add32, div32, mul32, sub32};
use larmorx_fs_core::{Process, Stop};

use super::args::Line;

/// A test cost (`<kind> <n> <f0> <b> <q> <a[1..n]> <c[1..n]>`), its counts and hash.
struct Cost {
    kind: i32,
    n: usize,
    f0: f32,
    b: f32,
    q: f32,
    a: [f32; 33],
    c: [f32; 33],
    hash: u64,
    nf: i64,
    ng: i64,
    ni: i64,
    ns: i64,
    nu: i64,
}

impl Cost {
    fn read(l: &mut Line) -> Result<Cost, Stop> {
        let kind = l.geti();
        let n = l.geti();
        if !(1..=32).contains(&n) {
            eprintln!("candidate: a cost of 1 to 32 parameters");
            return Err(Stop::Exit(2));
        }
        let n = n as usize;
        let mut c = Cost {
            kind,
            n,
            f0: l.getf(),
            b: l.getf(),
            q: l.getf(),
            a: [0.0; 33],
            c: [0.0; 33],
            hash: 0xcbf2_9ce4_8422_2325,
            nf: 0,
            ng: 0,
            ni: 0,
            ns: 0,
            nu: 0,
        };
        for i in 1..=n {
            c.a[i] = l.getf();
        }
        for i in 1..=n {
            c.c[i] = l.getf();
        }
        Ok(c)
    }

    fn mix(&mut self, v: u32) {
        for k in 0..4 {
            self.hash ^= u64::from((v >> (8 * k)) & 0xff);
            self.hash = self.hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn mixp(&mut self, p: &[f32], from: usize) {
        for i in from..=self.n {
            self.mix(p[i].to_bits());
        }
    }

    fn value(&mut self, p: &[f32]) -> f32 {
        let n = self.n;
        let mut s = self.f0;
        if self.kind == 1 {
            for i in 1..n {
                let t = sub32(p[i + 1], mul32(p[i], p[i]));
                let u = sub32(1.0, p[i]);
                s = add32(add32(s, mul32(mul32(100.0, t), t)), mul32(u, u));
            }
        } else {
            for i in 1..=n {
                let d = sub32(p[i], self.c[i]);
                s = if self.kind == 3 {
                    add32(s, mul32(self.a[i], d.abs()))
                } else {
                    add32(s, mul32(mul32(self.a[i], d), d))
                };
            }
            if self.kind != 3 {
                for i in 1..n {
                    s = add32(
                        s,
                        mul32(
                            self.b,
                            mul32(sub32(p[i], self.c[i]), sub32(p[i + 1], self.c[i + 1])),
                        ),
                    );
                }
            }
            if self.kind == 2 {
                s = div32(mul32(s, self.q).floor(), self.q);
            }
        }
        self.nf += 1;
        self.mix(1);
        self.mixp(p, 1);
        self.mix(s.to_bits());
        s
    }

    fn gradient(&mut self, p: &[f32], g: &mut [f32]) {
        let n = self.n;
        for gi in g.iter_mut().take(n + 1).skip(1) {
            *gi = 0.0;
        }
        if self.kind == 1 {
            for i in 1..n {
                let t = sub32(p[i + 1], mul32(p[i], p[i]));
                let u = sub32(1.0, p[i]);
                g[i] = add32(g[i], sub32(mul32(mul32(-400.0, t), p[i]), mul32(2.0, u)));
                g[i + 1] = add32(g[i + 1], mul32(200.0, t));
            }
        } else {
            for i in 1..=n {
                let d = sub32(p[i], self.c[i]);
                let mut gi = if self.kind == 3 {
                    let sg = if d > 0.0 {
                        1.0
                    } else if d < 0.0 {
                        -1.0
                    } else {
                        0.0
                    };
                    mul32(self.a[i], sg)
                } else {
                    mul32(mul32(2.0, self.a[i]), d)
                };
                if self.kind != 3 && i > 1 {
                    gi = add32(gi, mul32(self.b, sub32(p[i - 1], self.c[i - 1])));
                }
                if self.kind != 3 && i < n {
                    gi = add32(gi, mul32(self.b, sub32(p[i + 1], self.c[i + 1])));
                }
                g[i] = gi;
            }
        }
        self.ng += 1;
        self.mix(2);
        self.mixp(p, 1);
        self.mixp(g, 1);
    }

    fn put(&self, l: &mut Line) {
        l.puti(self.nf);
        l.puti(self.ng);
        l.puti(self.ni);
        l.puti(self.ns);
        l.puti(self.nu);
        l.out(format!("{:016x}", self.hash));
    }
}

/// One minimizer command; `false` when `c` is not one.
pub fn call(p: &mut Process, l: &mut Line, c: &str) -> Result<bool, Stop> {
    match c {
        "powell" | "powell2" => {
            let cost = RefCell::new(Cost::read(l)?);
            let n = cost.borrow().n;
            let mut x = vec![0.0f32; n + 1];
            for v in x.iter_mut().skip(1) {
                *v = l.getf();
            }
            let dirs = l.geti() != 0;
            let mut xi = if dirs {
                let mut m = vec![0.0f32; n * n];
                for v in m.iter_mut() {
                    *v = l.getf();
                }
                Some(m)
            } else {
                None
            };
            let mut iter = -7i32;
            let mut fret = -7.0f32;
            let mut func = |q: &[f32]| cost.borrow_mut().value(q);
            let mut on_iteration = |q: &[f32], nparms: i32| {
                let mut c = cost.borrow_mut();
                c.ni += 1;
                c.mix(3);
                c.mix(nparms as u32);
                c.mixp(q, 0);
            };
            if c == "powell" {
                let tol = l.getf();
                let cb = l.geti() != 0;
                p.open_powell(
                    &mut x,
                    xi.as_deref_mut(),
                    n as i32,
                    tol,
                    &mut iter,
                    &mut fret,
                    &mut func,
                    if cb { Some(&mut on_iteration) } else { None },
                )?;
            } else {
                let (ftol, lin) = (l.getf(), l.getf());
                let (maxit, gno) = (l.geti(), l.geti());
                let cb = l.geti() != 0;
                p.gdiag_no = gno;
                let r = p.open_powell2(
                    &mut x,
                    xi.as_deref_mut(),
                    n as i32,
                    ftol,
                    lin,
                    maxit,
                    &mut iter,
                    &mut fret,
                    &mut func,
                    if cb { Some(&mut on_iteration) } else { None },
                );
                l.puti(i64::from(r));
            }
            l.puti(i64::from(iter));
            l.putf(fret);
            for &v in x.iter().skip(1) {
                l.putf(v);
            }
            if let Some(m) = &xi {
                for &v in m {
                    l.putf(v);
                }
            }
            cost.borrow().put(l);
        }
        "dfpmin" => {
            let cost = RefCell::new(Cost::read(l)?);
            let n = cost.borrow().n;
            let mut x = vec![0.0f32; n + 1];
            for v in x.iter_mut().skip(1) {
                *v = l.getf();
            }
            let (tol, mut fret) = (l.getf(), l.getf());
            let (verbose, stepf, userf) = (l.geti(), l.geti(), l.geti());
            if verbose != 0 {
                p.gdiag |= DIAG_VERBOSE;
            }
            let mut iter = -7i32;
            let mut func = |q: &[f32]| cost.borrow_mut().value(q);
            let mut dfunc = |q: &[f32], g: &mut [f32]| cost.borrow_mut().gradient(q, g);
            let mut on_step = |itno: i32, sse: f32, q: &[f32]| {
                let mut c = cost.borrow_mut();
                c.ns += 1;
                c.mix(4);
                c.mix(itno as u32);
                c.mix(sse.to_bits());
                c.mixp(q, 1);
            };
            let mut on_user = |q: &[f32]| {
                let mut c = cost.borrow_mut();
                c.nu += 1;
                c.mix(5);
                c.mixp(q, 1);
            };
            let r = p.open_dfp_min(
                &mut x,
                n as i32,
                tol,
                &mut iter,
                &mut fret,
                &mut func,
                &mut dfunc,
                if stepf != 0 { Some(&mut on_step) } else { None },
                if userf != 0 { Some(&mut on_user) } else { None },
            )?;
            l.puti(i64::from(r));
            l.puti(i64::from(iter));
            l.putf(fret);
            for &v in x.iter().skip(1) {
                l.putf(v);
            }
            cost.borrow().put(l);
        }
        _ => return Ok(false),
    }
    Ok(true)
}
