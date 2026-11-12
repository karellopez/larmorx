// SPDX-License-Identifier: Apache-2.0
// C entry points to ITK 4.13.2's VNL and netlib routines that FreeSurfer 8.2.0 calls (the unit
// oracle's build of ITK, GCC 8.4.0 with FreeSurfer's ITK flags; this file compiled by build.sh with
// FreeSurfer's own C++ flags, as numerics.cpp is), for the Rust driver of validation/probes/fs-vnl.
// Written for this probe. Matrices are row-major arrays, as FreeSurfer's MATRIX holds them.
#include <cstdio>
#include <complex>
#include <iostream>

#include <vnl/vnl_cost_function.h>
#include <vnl/vnl_det.h>
#include <vnl/vnl_inverse.h>
#include <vnl/vnl_matrix.h>
#include <vnl/vnl_matrix_fixed.h>
#include <vnl/vnl_vector.h>
#include <vnl/algo/vnl_bracket_minimum.h>
#include <vnl/algo/vnl_brent.h>
#include <vnl/algo/vnl_brent_minimizer.h>
#include <vnl/algo/vnl_cholesky.h>
#include <vnl/algo/vnl_determinant.h>
#include <vnl/algo/vnl_fft_1d.h>
#include <vnl/algo/vnl_matrix_inverse.h>
#include <vnl/algo/vnl_netlib.h>
#include <vnl/algo/vnl_powell.h>
#include <vnl/algo/vnl_qr.h>
#include <vnl/algo/vnl_real_eigensystem.h>
#include <vnl/algo/vnl_svd.h>
#include <vnl/algo/vnl_symmetric_eigensystem.h>

typedef double (*fv_cost)(void *ctx, const double *x, int n);

namespace {
struct Callback : public vnl_cost_function {
  fv_cost fn;
  void *ctx;
  Callback(int n, fv_cost f, void *c) : vnl_cost_function(n), fn(f), ctx(c) {}
  double f(vnl_vector<double> const &x) { return fn(ctx, x.data_block(), (int)x.size()); }
};

template <unsigned N, class T> T det_fixed(const T *m) { return vnl_det(vnl_matrix_fixed<T, N, N>(m)); }
template <unsigned N, class T> void inverse_fixed(const T *m, T *out) {
  vnl_inverse(vnl_matrix_fixed<T, N, N>(m)).copy_out(out);
}
}  // namespace

extern "C" {
void fv_flush() {
  std::cout.flush();
  std::cerr.flush();
  fflush(stdout);
  fflush(stderr);
}

int fv_snprintf(char *buf, int len, const char *fmt, double v) { return snprintf(buf, len, fmt, v); }

// --- Small matrices (numerics.cpp's OpenLUMatrixInverse, OpenMatrixDeterminant, MatrixSqrt) -----
float fv_det_fixed_f32(int n, const float *m) {
  switch (n) {
    case 2: return det_fixed<2>(m);
    case 3: return det_fixed<3>(m);
    default: return det_fixed<4>(m);
  }
}
double fv_det_fixed_f64(int n, const double *m) {
  switch (n) {
    case 2: return det_fixed<2>(m);
    case 3: return det_fixed<3>(m);
    default: return det_fixed<4>(m);
  }
}
void fv_inverse_fixed_f32(int n, const float *m, float *out) {
  switch (n) {
    case 2: inverse_fixed<2>(m, out); break;
    case 3: inverse_fixed<3>(m, out); break;
    default: inverse_fixed<4>(m, out); break;
  }
}
void fv_inverse_fixed_f64(int n, const double *m, double *out) {
  switch (n) {
    case 2: inverse_fixed<2>(m, out); break;
    case 3: inverse_fixed<3>(m, out); break;
    default: inverse_fixed<4>(m, out); break;
  }
}
float fv_determinant_f32(int n, const float *m, int balance) {
  return vnl_determinant<float>(vnl_matrix<float>(m, n, n), balance != 0);
}
double fv_determinant_f64(int n, const double *m, int balance) {
  return vnl_determinant<double>(vnl_matrix<double>(m, n, n), balance != 0);
}

// --- QR (OpenLUMatrixInverse above 4 x 4, OpenQRdecomposition, mri_robust_register) -------------
#define QR_FUNCS(T, S)                                                                         \
  T fv_qr_##S(int r, int c, const T *m, T *q, T *rr) {                                         \
    vnl_qr<T> qr(vnl_matrix<T>(m, r, c));                                                      \
    if (q) qr.Q().copy_out(q);                                                                 \
    if (rr) qr.R().copy_out(rr);                                                               \
    return qr.determinant();                                                                   \
  }                                                                                            \
  void fv_qr_inverse_##S(int n, const T *m, T *out) {                                          \
    vnl_qr<T> qr(vnl_matrix<T>(m, n, n));                                                      \
    qr.inverse().copy_out(out);                                                                \
  }                                                                                            \
  void fv_qr_solve_##S(int r, int c, const T *m, const T *b, T *x) {                           \
    vnl_qr<T> qr(vnl_matrix<T>(m, r, c));                                                      \
    qr.solve(vnl_vector<T>(b, r)).copy_out(x);                                                 \
  }                                                                                            \
  void fv_qr_qtb_##S(int r, int c, const T *m, const T *b, T *x) {                             \
    vnl_qr<T> qr(vnl_matrix<T>(m, r, c));                                                      \
    qr.QtB(vnl_vector<T>(b, r)).copy_out(x);                                                   \
  }
QR_FUNCS(float, f32)
QR_FUNCS(double, f64)

// --- SVD (OpenSvdcmp) and the SVD inverse (vnl_matrix_inverse<double>) ------------------------
int fv_svd_f32(int r, int c, const float *m, float *u, float *w, float *v) {
  vnl_svd<float> svd(vnl_matrix<float>(m, r, c));
  if (!svd.valid()) return 0;
  svd.U().copy_out(u);
  svd.W().diagonal().copy_out(w);
  svd.V().copy_out(v);
  return 1;
}
void fv_matrix_inverse_f64(int n, const double *m, double *out) {
  vnl_matrix<double> inv = vnl_matrix_inverse<double>(vnl_matrix<double>(m, n, n));
  inv.copy_out(out);
}

// --- Eigensystems (OpenEigenSystem, OpenNonSymmetricEigenSystem) -------------------------------
void fv_symeig_f32(int n, const float *m, float *vec, float *val) {
  vnl_symmetric_eigensystem<float> es(vnl_matrix<float>(m, n, n));
  es.V.copy_out(vec);
  for (int i = 0; i < n; ++i) val[i] = es.get_eigenvalue(i);
}
void fv_symeig_f64(int n, const double *m, double *vec, double *val) {
  vnl_symmetric_eigensystem<double> es(vnl_matrix<double>(m, n, n));
  es.V.copy_out(vec);
  for (int i = 0; i < n; ++i) val[i] = es.get_eigenvalue(i);
}
// vreal's columns of a complex pair are never written by VNL (uninitialised memory).
void fv_realeig(int n, const double *m, double *vreal, double *vre, double *vim, double *dre, double *dim) {
  vnl_real_eigensystem es(vnl_matrix<double>(m, n, n));
  es.Vreal.copy_out(vreal);
  for (int r = 0; r < n; ++r)
    for (int c = 0; c < n; ++c) {
      vre[r * n + c] = es.V(r, c).real();
      vim[r * n + c] = es.V(r, c).imag();
    }
  for (int i = 0; i < n; ++i) {
    dre[i] = es.D(i, i).real();
    dim[i] = es.D(i, i).imag();
  }
}

// --- Cholesky (sc_linalg_cholesky_decomp) --------------------------------------------------------
// out: the upper triangle (n*n), the lower triangle (n*n); returns rank_deficiency(). mode: 0 quiet,
// 1 verbose, 2 estimate_condition (rcond written). want: 1 solve(b) into x, 2 determinant, 4 inverse.
long fv_cholesky(int n, const double *m, int mode, double *upper, double *lower, double *rcond,
                 int want, const double *b, double *x, double *det, double *inv) {
  vnl_cholesky ch(vnl_matrix<double>(m, n, n), (vnl_cholesky::Operation)mode);
  ch.upper_triangle().copy_out(upper);
  ch.lower_triangle().copy_out(lower);
  if (mode == 2) *rcond = ch.rcond();
  if (want & 1) ch.solve(vnl_vector<double>(b, n)).copy_out(x);
  if (want & 2) *det = ch.determinant();
  if (want & 4) {
    vnl_matrix<double> i = ch.inverse();
    if (i.rows() == (unsigned)n) i.copy_out(inv);
  }
  return ch.rank_deficiency();
}

// --- 1-D minimization (fs_powell's vnl_brent; vnl_powell's vnl_brent_minimizer) ------------------
void fv_bracket(fv_cost f, void *ctx, double a, double b, double *out6) {
  Callback cb(1, f, ctx);
  double c, fa, fb, fc;
  vnl_bracket_minimum(cb, a, b, c, fa, fb, fc);
  out6[0] = a; out6[1] = b; out6[2] = c; out6[3] = fa; out6[4] = fb; out6[5] = fc;
}
// which: 0 minimize_given_bounds(ax, bx, cx), 1 ..._and_one_f(ax, bx, cx, fb),
// 2 ..._and_all_f(ax, bx, cx, fa, fb, fc), 3 minimize(ax). Returns x; *fmin = f_at_last_minimum().
double fv_brent_minimizer(fv_cost f, void *ctx, int which, double xtol, const double *args, double *fmin) {
  Callback cb(1, f, ctx);
  vnl_brent_minimizer bm(cb);
  if (xtol > 0) bm.set_x_tolerance(xtol);
  double x = 0;
  switch (which) {
    case 0: x = bm.minimize_given_bounds(args[0], args[1], args[2]); break;
    case 1: x = bm.minimize_given_bounds_and_one_f(args[0], args[1], args[2], args[3]); break;
    case 2: x = bm.minimize_given_bounds_and_all_f(args[0], args[1], args[2], args[3], args[4], args[5]); break;
    default: x = bm.minimize(args[0]); break;
  }
  *fmin = bm.f_at_last_minimum();
  return x;
}
// The deprecated vnl_brent as fs_powell uses it: bracket_minimum(&ax, &xx, &bx) then
// minimize_given_bounds(bx, xx, ax, tol, &xx). io: ax, xx, bx in; out: ax, xx, bx, then xmin, fret.
void fv_vnl_brent_powell_step(fv_cost f, void *ctx, double tol, double *io5) {
  Callback cb(1, f, ctx);
  vnl_brent brent(&cb);
  double ax = io5[0], xx = io5[1], bx = io5[2];
  brent.bracket_minimum(&ax, &xx, &bx);
  io5[0] = ax; io5[1] = xx; io5[2] = bx;
  double fret = brent.minimize_given_bounds(bx, xx, ax, tol, &xx);
  io5[3] = xx;
  io5[4] = fret;
}
double fv_vnl_brent_minimize(fv_cost f, void *ctx, double x) {
  Callback cb(1, f, ctx);
  vnl_brent brent(&cb);
  return brent.minimize(x);
}
// vnl_powell::minimize. Returns the return code; counters through the pointers.
int fv_powell(fv_cost f, void *ctx, int n, double *p, double linmin_xtol, double initial_step, double ftol,
              int maxfev, int verbose, int *iters, long *evals, double *start_err, double *end_err) {
  Callback cb(n, f, ctx);
  vnl_powell pw(&cb);
  pw.set_linmin_xtol(linmin_xtol);
  pw.set_initial_step(initial_step);
  if (ftol > 0) pw.set_f_tolerance(ftol);
  pw.set_max_function_evals(maxfev);
  pw.set_verbose(verbose != 0);
  vnl_vector<double> x(p, n);
  int code = pw.minimize(x);
  x.copy_out(p);
  *iters = pw.get_num_iterations();
  *evals = pw.get_num_evaluations();
  *start_err = pw.get_start_error();
  *end_err = pw.get_end_error();
  return code;
}

// --- netlib's L-BFGS (fs_lbfgs: OpenDFPMin) -------------------------------------------------------
void *fv_lbfgs_new() {
  v3p_netlib_lbfgs_global_t *g = new v3p_netlib_lbfgs_global_t();
  v3p_netlib_lbfgs_init(g);
  return g;
}
void fv_lbfgs_free(void *g) { delete static_cast<v3p_netlib_lbfgs_global_t *>(g); }
void fv_lbfgs_set(void *gv, double gtol, double stpinit, long lp) {
  v3p_netlib_lbfgs_global_t *g = static_cast<v3p_netlib_lbfgs_global_t *>(gv);
  g->gtol = gtol;
  g->stpinit = stpinit;
  g->lp = lp;
}
// The struct's fields, in its order: 14 integers (mp, lp, info, infoc, nfev, maxfev, inmc, iscn,
// iycn, iter, nfun, ispt, iypt, bound), point, finish, stage1, brackt, and the doubles.
void fv_lbfgs_state(void *gv, long *ints, double *dbls) {
  v3p_netlib_lbfgs_global_t *g = static_cast<v3p_netlib_lbfgs_global_t *>(gv);
  long iv[] = {g->mp, g->lp, g->private_info, g->private_infoc, g->private_nfev, g->private_maxfev,
               g->private_inmc, g->private_iscn, g->private_iycn, g->private_iter, g->private_nfun,
               g->private_ispt, g->private_iypt, g->private_bound, g->private_point, g->private_finish,
               g->private_stage1, g->private_brackt};
  for (int i = 0; i < 18; ++i) ints[i] = iv[i];
  double dv[] = {g->gtol, g->stpmin, g->stpmax, g->stpinit, g->private_stp, g->private_stp1,
                 g->private_beta, g->private_ftol, g->private_gnorm, g->private_xnorm, g->private_dg,
                 g->private_fm, g->private_fx, g->private_fy, g->private_dgm, g->private_dgx,
                 g->private_dgy, g->private_fxm, g->private_fym, g->private_stx, g->private_sty,
                 g->private_dgxm, g->private_dgym, g->private_finit, g->private_width,
                 g->private_stmin, g->private_stmax, g->private_width1, g->private_ftest1,
                 g->private_dginit, g->private_dgtest};
  for (int i = 0; i < 31; ++i) dbls[i] = dv[i];
}
void fv_lbfgs(long n, long m, double *x, double f, double *g, long diagco, double *diag, long *iprint,
              double eps, double xtol, double *w, long *iflag, void *gl) {
  v3p_netlib_lbfgs_(&n, &m, x, &f, g, &diagco, diag, iprint, &eps, &xtol, w, iflag,
                    static_cast<v3p_netlib_lbfgs_global_t *>(gl));
}

// --- vnl_fft_1d<double> (N4's histogram sharpening) ----------------------------------------------
void fv_fft(int n, int forward, double *reim) {
  vnl_vector<std::complex<double> > v(n);
  for (int i = 0; i < n; ++i) v[i] = std::complex<double>(reim[2 * i], reim[2 * i + 1]);
  vnl_fft_1d<double> fft(n);
  if (forward) fft.fwd_transform(v); else fft.bwd_transform(v);
  for (int i = 0; i < n; ++i) {
    reim[2 * i] = v[i].real();
    reim[2 * i + 1] = v[i].imag();
  }
}
}
