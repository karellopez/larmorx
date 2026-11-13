// SPDX-License-Identifier: Apache-2.0
//! The matrix commands (README.md, "Commands", spec §2 to §4, §8 to §19).

use larmorx_fs_core::maths::affine::{self, AffineVector};
use larmorx_fs_core::maths::dmatrix::{DMatrix, StreamSel};
use larmorx_fs_core::maths::matrix::Matrix;
use larmorx_fs_core::maths::{Output, Returned};
use larmorx_fs_core::numbers::ListStats;
use larmorx_fs_core::{Process, Stop};

use crate::args::{Line, Out};

/// A product's output resolved (`self`/`other`: the input copied first, the matrix itself the
/// output), then printed as the probe prints it: result, O, then each input that is not O.
fn product(
    p: &mut Process,
    l: &mut Line,
    a: Option<Matrix>,
    b: Option<Matrix>,
    o: Out<Option<Matrix>>,
    float: bool,
) -> Result<(), Stop> {
    let f = if float { Process::matrix_multiply } else { Process::matrix_multiply_d };
    match o {
        Out::New | Out::Obj(None) => {
            let r = f(p, a.as_ref(), b.as_ref(), None)?;
            l.putm_res(&r, None);
            l.putm(a.as_ref());
            l.putm(b.as_ref());
        }
        Out::First => {
            let mut a = a;
            let ac = a.clone();
            let r = f(p, ac.as_ref(), b.as_ref(), a.as_mut())?;
            l.putm_res(&r, a.as_ref());
            l.putm(b.as_ref());
        }
        Out::Second => {
            let mut b = b;
            let bc = b.clone();
            let r = f(p, a.as_ref(), bc.as_ref(), b.as_mut())?;
            l.putm_res(&r, b.as_ref());
            l.putm(a.as_ref());
        }
        Out::Obj(Some(mut d)) => {
            let r = f(p, a.as_ref(), b.as_ref(), Some(&mut d))?;
            l.putm_res(&r, Some(&d));
            l.putm(a.as_ref());
            l.putm(b.as_ref());
        }
    }
    Ok(())
}

/// A double-matrix command with an output (`self`/`other`: the input copied, the matrix itself
/// the output); prints the result and O.
fn dout(
    l: &mut Line,
    a: &DMatrix,
    b: Option<&DMatrix>,
    o: Out<Option<DMatrix>>,
    f: impl FnOnce(&DMatrix, Option<&DMatrix>, Option<&mut DMatrix>) -> Returned<DMatrix>,
) {
    match o {
        Out::New | Out::Obj(None) => {
            let r = f(a, b, None);
            l.putdm_res(&r, None);
        }
        Out::First => {
            let mut x = a.clone();
            let r = f(a, b, Some(&mut x));
            l.putdm_res(&r, Some(&x));
        }
        Out::Second => {
            let mut x = b.cloned().expect("a second matrix");
            let r = f(a, b, Some(&mut x));
            l.putdm_res(&r, Some(&x));
        }
        Out::Obj(Some(mut d)) => {
            let r = f(a, b, Some(&mut d));
            l.putdm_res(&r, Some(&d));
        }
    }
}

fn stream(l: &mut Line) -> Option<StreamSel> {
    match l.tok() {
        "out" => Some(StreamSel::Out),
        "err" => Some(StreamSel::Err),
        "null" => None,
        _ => {
            eprintln!("probe: a stream is out, err or null");
            std::process::exit(3);
        }
    }
}

pub fn call(p: &mut Process, l: &mut Line, c: &str) -> Result<bool, Stop> {
    match c {
        // -- §2
        "malloc" => {
            let (r, k, t) = (l.geti(), l.geti(), l.geti());
            let m = p.matrix_alloc(r, k, t)?;
            l.putm(m.as_ref());
        }
        "malloc2" => {
            let (r, k, t) = (l.geti(), l.geti(), l.geti());
            let mut m = p.matrix_alloc2(r, k, t)?;
            l.putm(m.as_ref());
            if let Some(x) = m.as_ref() {
                l.puti(i64::from(x.in_buf));
                let v = p.matrix_free(&mut m);
                l.puti(i64::from(v));
                l.out(if m.is_some() { "set" } else { "none" });
            }
        }
        "mfree" => {
            let mut m = l.getm(p)?;
            let v = p.matrix_free(&mut m);
            l.puti(i64::from(v));
            l.out(if m.is_some() { "set" } else { "none" });
        }
        "mcopy" => {
            let a = l.getm(p)?;
            let o = l.getm_out(p)?;
            match o {
                Out::New | Out::Obj(None) => {
                    let r = p.matrix_copy(a.as_ref(), Output::New)?;
                    l.putm_res(&r, None);
                }
                Out::First | Out::Second => {
                    let r = p.matrix_copy(a.as_ref(), Output::First)?;
                    l.putm_res(&r, a.as_ref());
                }
                Out::Obj(Some(mut d)) => {
                    let r = p.matrix_copy(a.as_ref(), Output::Given(&mut d))?;
                    l.putm_res(&r, Some(&d));
                }
            }
        }
        "mmuld" | "mmul" => {
            let a = l.getm(p)?;
            let b = l.getm(p)?;
            let o = l.getm_out(p)?;
            product(p, l, a, b, o, c == "mmul")?;
        }
        // -- §3
        "affine" => {
            let a = l.getm(p)?.expect("a matrix");
            let b = l.getm(p)?.expect("a matrix");
            let am = p.set_affine_matrix(&a)?;
            let bm = p.set_affine_matrix(&b)?;
            let (x, y, z) = (l.getf(), l.getf(), l.getf());
            let mut v = AffineVector::new(x, y, z);
            for i in 0..4 {
                l.putf(v.0[i]);
            }
            v.0[3] = l.getf();
            let w = affine::affine_mv(&am, &v);
            for i in 0..4 {
                l.putf(w.0[i]);
            }
            let (gx, gy, gz) = w.get();
            l.putf(gx);
            l.putf(gy);
            l.putf(gz);
            let (ix, iy, iz) = p.affine_vector_floor(&w);
            l.puti(i64::from(ix));
            l.puti(i64::from(iy));
            l.puti(i64::from(iz));
            let m = affine::affine_mm(&am, &bm);
            for i in 0..16 {
                l.putf(m.0[i]);
            }
            let mut g = p.matrix_alloc(4, 4, 1)?.expect("a matrix");
            p.get_affine_matrix(&mut g, &m)?;
            l.putm(Some(&g));
        }
        "affineset" => {
            let a = l.getm(p)?.expect("a matrix");
            let am = p.set_affine_matrix(&a)?;
            for i in 0..16 {
                l.putf(am.0[i]);
            }
        }
        "affineget" => {
            let mut a = l.getm(p)?.expect("a matrix");
            let mut am = affine::AffineMatrix::default();
            for i in 0..16 {
                am.0[i] = l.getf();
            }
            p.get_affine_matrix(&mut a, &am)?;
            l.putm(Some(&a));
        }
        // -- §4
        "dalloc" => {
            let (r, k, t) = (l.geti(), l.geti(), l.geti());
            let m = p.dmatrix_alloc(r, k, t)?;
            l.putdm(m.as_ref());
        }
        "dcheck" => {
            let a = l.getdm(p)?;
            let b = l.getdm(p)?;
            let kind = l.geti();
            let s = stream(l);
            let pre = l.getx();
            let v = p.dmatrix_check_dims(a.as_ref(), b.as_ref(), kind, s, &pre);
            l.puti(i64::from(v));
        }
        "daddmul" => {
            let a = l.getdm(p)?.expect("a matrix");
            let b = l.getdm(p)?;
            let o = l.getdm_out(p)?;
            let (v1, v2) = (l.getd(), l.getd());
            dout(l, &a, b.as_ref(), o, |a, b, o| p.dmatrix_add_mul(a, b, v1, v2, o));
        }
        "dadd" | "dsub" => {
            let a = l.getdm(p)?.expect("a matrix");
            let b = l.getdm(p)?.expect("a matrix");
            let o = l.getdm_out(p)?;
            let sub = c == "dsub";
            dout(l, &a, Some(&b), o, |a, b, o| {
                let b = b.expect("b");
                if sub { p.dmatrix_subtract(a, b, o) } else { p.dmatrix_add(a, b, o) }
            });
        }
        "dscale" => {
            let a = l.getdm(p)?.expect("a matrix");
            let o = l.getdm_out(p)?;
            let v = l.getd();
            dout(l, &a, None, o, |a, _, o| p.dmatrix_scalar_mul(a, v, o));
        }
        "dcopy" => {
            let a = l.getdm(p)?;
            let o = l.getdm_out(p)?;
            match a {
                None => {
                    let r = p.dmatrix_copy(None, None);
                    l.putdm_res(&r, None);
                }
                Some(a) => dout(l, &a, None, o, |a, _, o| p.dmatrix_copy(Some(a), o)),
            }
        }
        "dtranspose" => {
            let mut a = l.getdm(p)?.expect("a matrix");
            let o = l.getdm_out(p)?;
            match o {
                Out::New | Out::Obj(None) => {
                    let r = p.dmatrix_transpose(&a, Output::New);
                    l.putdm_res(&r, None);
                }
                Out::First | Out::Second => {
                    let r = p.dmatrix_transpose(&a, Output::First);
                    let _ = &mut a;
                    l.putdm_res(&r, Some(&a));
                }
                Out::Obj(Some(mut d)) => {
                    let r = p.dmatrix_transpose(&a, Output::Given(&mut d));
                    l.putdm_res(&r, Some(&d));
                }
            }
        }
        "dprint" => {
            let fmt = l.getx();
            let a = l.getdm(p)?;
            let v = if fmt.is_empty() {
                p.dmatrix_print(a.as_ref())
            } else {
                p.dmatrix_print_fmt(&fmt, a.as_ref())
            };
            l.puti(i64::from(v));
        }
        "dmul" => {
            let a = l.getdm(p)?.expect("a matrix");
            let b = l.getdm(p)?.expect("a matrix");
            let o = l.getdm_out(p)?;
            dout(l, &a, Some(&b), o, |a, b, o| p.dmatrix_multiply(a, b.expect("b"), o));
        }
        "ddot" => {
            let a = l.getdm(p)?.expect("a matrix");
            let b = l.getdm(p)?.expect("a matrix");
            let v = p.dvector_dot(&a, &b);
            l.putd(v);
        }
        "dcmpf" => {
            let f = l.getm(p)?;
            let d = l.getdm(p)?;
            let v = p.dmatrix_compare_fmatrix(f.as_ref(), d.as_ref());
            l.putd(v);
        }
        "dcopyf" => {
            let f = l.getm(p)?;
            let o = l.getdm_out(p)?;
            match o {
                Out::Obj(Some(mut d)) => {
                    let r = p.dmatrix_copy_fmatrix(f.as_ref(), Some(&mut d));
                    l.putdm_res(&r, Some(&d));
                }
                _ => {
                    let r = p.dmatrix_copy_fmatrix(f.as_ref(), None);
                    l.putdm_res(&r, None);
                }
            }
        }
        "dconst" | "dzero" => {
            let v = if c == "dconst" { l.getd() } else { 0.0 };
            let (rows, cols) = (l.geti(), l.geti());
            let o = l.getdm_out(p)?;
            match o {
                Out::Obj(Some(mut d)) => {
                    let r = p.dmatrix_const_val(v, rows, cols, Some(&mut d))?;
                    l.putdm_res(&r, Some(&d));
                }
                _ => {
                    let r = p.dmatrix_const_val(v, rows, cols, None)?;
                    l.putdm_res(&r, None);
                }
            }
        }
        "dmaxabs" => {
            let a = l.getdm(p)?;
            let v = p.dmatrix_max_abs(a.as_ref());
            l.putd(v);
        }
        "dliststats" => {
            let n = l.geti();
            let v: Vec<f64> = (0..n.max(0)).map(|_| l.getd()).collect();
            match p.dlist_stats(Some(&v), n) {
                ListStats::Stats(s) => {
                    for x in s {
                        l.putd(x);
                    }
                }
                ListStats::None => l.out("null"),
            }
        }
        _ => return Ok(false),
    }
    Ok(true)
}
