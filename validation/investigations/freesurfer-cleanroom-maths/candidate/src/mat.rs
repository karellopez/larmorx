// SPDX-License-Identifier: Apache-2.0
//! The matrix commands (README.md, "Commands", spec §2 to §4, §8 to §19).

use larmorx_fs_core::maths::affine::{self, AffineVector};
use larmorx_fs_core::maths::dmatrix::{DMatrix, StreamSel};
use larmorx_fs_core::maths::elements::{self, Region};
use larmorx_fs_core::maths::mfiles::CStream;
use larmorx_fs_core::maths::rfields;
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

/// An operation with an output resolved as the probe's `getm_out(a, b)`: `self` is `a`, `other`
/// is `b` (the input copied first, the matrix itself the output); prints the result and O.
fn mout(
    l: &mut Line,
    a: Option<&Matrix>,
    b: Option<&Matrix>,
    o: Out<Option<Matrix>>,
    f: impl FnOnce(Option<&mut Matrix>) -> Result<Returned<Matrix>, Stop>,
) -> Result<(), Stop> {
    match o {
        Out::New | Out::Obj(None) => {
            let r = f(None)?;
            l.putm_res(&r, None);
        }
        Out::First | Out::Second => {
            let src = if matches!(o, Out::First) { a } else { b };
            let mut x = src.cloned();
            let r = f(x.as_mut())?;
            l.putm_res(&r, x.as_ref());
        }
        Out::Obj(Some(mut d)) => {
            let r = f(Some(&mut d))?;
            l.putm_res(&r, Some(&d));
        }
    }
    Ok(())
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
        // -- §8.2
        "midentity" => {
            let n = l.geti();
            let o = l.getm_out(p)?;
            mout(l, None, None, o, |o| p.matrix_identity(n, o))?;
        }
        "mtranspose" | "msquare" | "mdiag" | "mclone" => {
            let a = l.getm(p)?.expect("a matrix");
            if c == "mclone" {
                let m = p.matrix_clone(&a)?;
                l.putm(m.as_ref());
                return Ok(true);
            }
            let o = l.getm_out(p)?;
            mout(l, Some(&a), None, o, |o| match c {
                "mtranspose" => p.matrix_transpose(&a, o),
                "msquare" => p.matrix_square_elts(&a, o),
                _ => p.matrix_diag(&a, o),
            })?;
        }
        "madd" | "msub" => {
            let a = l.getm(p)?.expect("a matrix");
            let b = l.getm(p)?.expect("a matrix");
            let o = l.getm_out(p)?;
            mout(l, Some(&a), Some(&b), o, |o| p.matrix_add_sub(&a, &b, o, c == "msub"))?;
        }
        "mscale" => {
            let a = l.getm(p)?.expect("a matrix");
            let o = l.getm_out(p)?;
            let v = l.getf();
            mout(l, Some(&a), None, o, |o| p.matrix_scalar_mul(&a, v, o))?;
        }
        "mclear" => {
            let mut a = l.getm(p)?.expect("a matrix");
            let r = p.matrix_clear(&mut a);
            l.putm_res(&r, Some(&a));
        }
        "msymmetric" | "miszero" | "mcheck" => {
            let a = l.getm(p)?.expect("a matrix");
            let v = match c {
                "msymmetric" => p.matrix_is_symmetric(&a),
                "miszero" => p.matrix_is_zero(&a),
                _ => p.matrix_check(&a),
            };
            l.puti(i64::from(v));
        }
        "mtrace" => {
            let a = l.getm(p)?.expect("a matrix");
            let v = p.matrix_trace(&a);
            l.putf(v);
        }
        "msum" => {
            let a = l.getm(p)?.expect("a matrix");
            let v = p.matrix_sum_elts(&a);
            l.putd(v);
        }
        "mconst" | "mzero" => {
            let v = if c == "mconst" { l.getf() } else { 0.0 };
            let (rows, cols) = (l.geti(), l.geti());
            let o = l.getm_out(p)?;
            mout(l, None, None, o, |o| p.matrix_const_val(v, rows, cols, o))?;
        }
        "mreshape" => {
            let a = l.getm(p)?.expect("a matrix");
            let o = l.getm_out(p)?;
            let (rows, cols) = (l.geti(), l.geti());
            mout(l, Some(&a), None, o, |o| p.matrix_reshape(&a, o, rows, cols))?;
        }
        "mcolscale" => {
            let a = l.getm(p)?.expect("a matrix");
            let o = l.getm_out(p)?;
            mout(l, Some(&a), None, o, |o| p.matrix_normalize_col_scale(&a, o))?;
        }
        "mnormcol" => {
            let a = l.getm(p)?.expect("a matrix");
            let o = l.getm_out(p)?;
            let s = l.getm_out(p)?;
            let mut s = match s {
                Out::New | Out::Obj(None) => None,
                Out::First | Out::Second => Some(a.clone()),
                Out::Obj(Some(m)) => Some(m),
            };
            mout(l, Some(&a), None, o, |o| p.matrix_normalize_col(&a, o, s.as_mut()))?;
            if s.is_some() {
                l.putm(s.as_ref());
            }
        }
        // -- §8.3
        "mregion" | "mrealregion" | "mimagregion" => {
            let a = l.getm(p)?.expect("a matrix");
            let o = l.getm_out(p)?;
            let v: Vec<i32> = (0..6).map(|_| l.geti()).collect();
            let which = match c {
                "mregion" => Region::Whole,
                "mrealregion" => Region::Real,
                _ => Region::Imag,
            };
            mout(l, Some(&a), None, o, |o| {
                p.matrix_copy_region(which, &a, o, v[0], v[1], v[2], v[3], v[4], v[5])
            })?;
        }
        "mr2c" => {
            let a = l.getm(p)?.expect("a matrix");
            let b = l.getm(p)?.expect("a matrix");
            let o = l.getm_out(p)?;
            mout(l, Some(&a), Some(&b), o, |o| p.matrix_real_to_complex(&a, &b, o))?;
        }
        // -- §8.4, §8.5
        "mrotation" => {
            let n = l.geti();
            let angle = l.getf();
            let which = l.geti();
            let o = l.getm_out(p)?;
            mout(l, None, None, o, |o| p.matrix_rotation(n, angle, which, o))?;
        }
        "vlen" => {
            let a = l.getm(p)?.expect("a matrix");
            let v = p.vector_len(&a);
            l.putf(v);
        }
        "vdot" | "vdist" => {
            let a = l.getm(p)?.expect("a matrix");
            let b = l.getm(p)?.expect("a matrix");
            let v = p.vector_dot_dist(&a, &b, c == "vdist");
            l.putf(v);
        }
        "vcross" => {
            let a = l.getm(p)?.expect("a matrix");
            let b = l.getm(p)?.expect("a matrix");
            let o = l.getm_out(p)?;
            mout(l, Some(&a), Some(&b), o, |o| p.vector_cross_product(&a, &b, o))?;
        }
        "vtriple" => {
            let a = l.getm(p)?.expect("a matrix");
            let b = l.getm(p)?.expect("a matrix");
            let d = l.getm(p)?.expect("a matrix");
            let v = p.vector_triple_product(&a, &b, &d);
            l.putf(v);
        }
        "v3angle" => {
            let a = l.getm(p)?.expect("a matrix");
            let b = l.getm(p)?.expect("a matrix");
            let v = p.vector3_angle(&a, &b);
            l.putd(v);
        }
        "xyznorm" => {
            let (x, y, z) = (l.getf(), l.getf(), l.getf());
            let (v, len) = elements::xyz_normalized_load(x, y, z);
            for k in v {
                l.putf(k);
            }
            l.putf(len);
        }
        "xyzangle" => {
            let n = [l.getf(), l.getf(), l.getf()];
            let (x, y, z, norm) = (l.getf(), l.getf(), l.getf(), l.getf());
            let v = elements::xyz_approx_angle_known_length(n, x, y, z, norm);
            l.putf(v);
        }
        // -- §8.6, §8.7
        "mprint" => {
            let s = stream(l);
            let a = l.getm(p)?;
            let prec = l.geti();
            let to_err = s.map(|s| s == StreamSel::Err);
            let v = p.matrix_print(to_err, a.as_ref(), prec);
            l.puti(i64::from(v));
        }
        "mprintfmt" => {
            let fmt = l.getx();
            let a = l.getm(p)?;
            let v = p.matrix_print_fmt(&fmt, a.as_ref());
            l.puti(i64::from(v));
        }
        "msimilarity" => {
            let a = l.getm(p)?.expect("a matrix");
            let b = l.getm(p)?.expect("a matrix");
            let o = l.getm_out(p)?;
            mout(l, Some(&a), Some(&b), o, |o| p.matrix_similarity_transform(&a, &b, o))?;
        }
        "mgaussian" => {
            let n = l.geti();
            let sd = l.getf();
            let norm = l.geti();
            let o = l.getm_out(p)?;
            mout(l, None, None, o, |o| p.gaussian_matrix(n, sd, norm, o))?;
        }
        "mgaussian2" => {
            let n = l.geti();
            let (s1, s2, w1) = (l.getf(), l.getf(), l.getf());
            let norm = l.geti();
            let o = l.getm_out(p)?;
            mout(l, None, None, o, |o| p.gaussian_matrix2(n, s1, s2, w1, norm, o))?;
        }
        // -- §8.8
        "minverse" => {
            let a = l.getm(p)?;
            let o = l.getm_out(p)?;
            mout(l, a.as_ref(), None, o, |o| p.matrix_inverse(a.as_ref(), o))?;
        }
        "mdet" => {
            let a = l.getm(p)?.expect("a matrix");
            let v = p.matrix_determinant(&a);
            l.putf(v);
        }
        "mqr" => {
            let a = l.getm(p)?;
            let mut q = l.getm(p)?;
            let mut r = l.getm(p)?;
            let v = p.matrix_qr_decomposition(a.as_ref(), q.as_mut(), r.as_mut());
            l.puti(i64::from(v));
            l.putm(q.as_ref());
            l.putm(r.as_ref());
        }
        // -- §9, §10
        "primes" | "pfactors" => {
            let n = l.geti();
            let v = if c == "primes" { p.get_primes(n) } else { p.get_prime_factors(n) };
            match v {
                None => l.out("null"),
                Some(v) => {
                    l.puti(v.len() as i64);
                    for x in v {
                        l.puti(i64::from(x));
                    }
                }
            }
        }
        "pclosest" | "pclosestless" => {
            let (n, q) = (l.geti(), l.geti());
            match p.get_closest_prime_factor(n, q, c == "pclosestless") {
                Some(v) => l.puti(i64::from(v)),
                None => l.out("crash"),
            }
        }
        "gaussdraws" => {
            let seed = l.getl();
            let k = l.geti();
            p.srand48(seed);
            for _ in 0..k.max(0) {
                let v = p.pdf_gaussian();
                l.putd(v);
            }
        }
        "todseed" => {
            let v = p.pdf_tod_seed();
            l.puti(v as i64);
        }
        // -- §14
        "nrvector" => {
            let (nl, nh) = (l.getl(), l.getl());
            let mut v = p.nr_vector(nl, nh)?;
            for i in nl..=nh {
                v.set(i, i as f32);
            }
            for i in nl..=nh {
                l.putf(v.get(i));
            }
        }
        "nrmatrix" => {
            let (r0, r1, c0, c1) = (l.getl(), l.getl(), l.getl(), l.getl());
            let mut m = p.nr_matrix(r0, r1, c0, c1)?;
            for r in r0..=r1 {
                for k in c0..=c1 {
                    m.set(r, k, (r * 100 + k) as f32);
                }
            }
            for r in r0..=r1 {
                for k in c0..=c1 {
                    l.putf(m.get(r, k));
                }
            }
        }
        // -- §17.1 to §17.3
        "oeigen" | "oeigenns" => {
            let n = l.geti();
            let a: Vec<f32> = (0..(n * n).max(0)).map(|_| l.getf()).collect();
            let nu = n.max(0) as usize;
            let e = if c == "oeigen" {
                p.open_eigen_system(&a, nu)
            } else {
                p.open_non_symmetric_eigen_system(&a, nu)
            };
            l.puti(i64::from(e.status));
            for i in 0..nu {
                l.putf(e.values.as_ref().map_or(7.0, |v| v[i]));
            }
            for i in 0..nu * nu {
                l.putf(e.vectors.as_ref().map_or(7.0, |v| v[i]));
            }
        }
        "meigen" | "meigenhelper" => {
            let a = l.getm(p)?.expect("a matrix");
            let sym = if c == "meigenhelper" { l.geti() } else { -1 };
            let o = l.getm_out(p)?;
            let n = if a.rows > 0 { a.rows as usize } else { 1 };
            let mut vals = None;
            mout(l, Some(&a), None, o, |o| {
                let (r, v) = if sym < 0 {
                    p.matrix_eigen_system(&a, o)?
                } else {
                    p.matrix_eigen_helper(&a, o, sym != 0)?
                };
                vals = v;
                Ok(r)
            })?;
            for i in 0..n {
                l.putf(vals.as_ref().and_then(|v| v.get(i).copied()).unwrap_or(7.0));
            }
        }
        // -- §18
        "mreadtxt" | "mreadtxtmissing" => {
            let name = l.getx();
            let path = String::from_utf8_lossy(&name).into_owned();
            if c == "mreadtxt" {
                let bytes = l.getx();
                let _ = std::fs::write(&path, &bytes);
            } else {
                let _ = std::fs::remove_file(&path);
            }
            p.errno = 0;
            let m = p.matrix_read_txt(&name)?;
            l.putm(m.as_ref());
            l.puti(i64::from(p.errno));
            if c == "mreadtxt" {
                let _ = std::fs::remove_file(&path);
            }
        }
        "mreadasc" | "mreadbin" => {
            let bytes = l.getx();
            let o = l.getm_out(p)?;
            let mut s = CStream::new(bytes);
            p.errno = 0;
            match o {
                Out::Obj(Some(mut d)) => {
                    let r = if c == "mreadasc" {
                        p.matrix_ascii_read_from(&mut s, Some(&mut d))?
                    } else {
                        p.matrix_read_from(&mut s, Some(&mut d))?
                    };
                    l.putm_res(&r, Some(&d));
                }
                _ => {
                    let r = if c == "mreadasc" {
                        p.matrix_ascii_read_from(&mut s, None)?
                    } else {
                        p.matrix_read_from(&mut s, None)?
                    };
                    l.putm_res(&r, None);
                }
            }
            l.puti(i64::from(p.errno));
            l.puti(s.pos as i64);
        }
        "mwriteasc" | "mwritebin" => {
            let a = l.getm(p)?.expect("a matrix");
            let b = if c == "mwriteasc" { p.matrix_ascii_write(&a) } else { p.matrix_write_into(&a) };
            l.puti(0);
            l.out(crate::hexbytes(&b));
        }
        "mwritetxt" => {
            let name = l.getx();
            let a = l.getm(p)?.expect("a matrix");
            let path = String::from_utf8_lossy(&name).into_owned();
            let _ = std::fs::remove_file(&path);
            let v = p.matrix_write_txt(&name, &a);
            l.puti(i64::from(v));
            match std::fs::read(&path) {
                Ok(b) => l.out(crate::hexbytes(&b)),
                Err(_) => l.out("nofile"),
            }
            let _ = std::fs::remove_file(&path);
        }
        // -- §19
        "rfname" => {
            let name = l.getx();
            let mut r = p.rf_spec_init(1);
            r.name = Some(name);
            let v = rfields::rf_name_to_code(&mut r);
            l.puti(i64::from(v));
            l.puti(i64::from(r.code));
        }
        "rfinit" => {
            let seed = crate::args::strtoul10(l.tok());
            let mut r = p.rf_spec_init(seed);
            match &r.name {
                Some(n) => l.out(crate::hexbytes(n)),
                None => l.out("null"),
            }
            l.puti(i64::from(r.code));
            l.puti(i64::from(r.nparams));
            l.putd(r.mean);
            l.putd(r.stddev);
            l.puti(r.seed as i64);
            let s2 = crate::args::strtoul10(l.tok());
            let v = p.rf_spec_set_seed(&mut r, s2);
            l.puti(i64::from(v));
            l.puti(r.seed as i64);
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
