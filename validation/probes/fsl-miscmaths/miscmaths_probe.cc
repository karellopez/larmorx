// SPDX-License-Identifier: Apache-2.0
// miscmaths_probe: runs FSL 6.0.7.23's own sparse matrices (SpMat<T>, SparseBFMatrix<T>,
// FullBFMatrix), the solvers of SpMat::SolveForx (CG, BiCG) and the optimisers of nonlin
// (Levenberg-Marquardt, scaled conjugate gradients; libfsl-miscmaths) on the inputs of one case
// directory, and prints every result as the hexadecimal bits of its value, one line per value, in
// the order and format of the replica's side (crates-nc/larmorx-nc-core/tests/miscmaths_probe.rs).
// See README.md.
//
// Usage: miscmaths_probe <case dir>
// <case dir>/A.csc, B.csc: u32 m, u32 n, u32 nnz, irp[nnz] (u32), jcp[n+1] (u32), val[nnz] (f64)
// D.bin: u32 rows, u32 cols, col-major f64; x.bin, b.bin: n f64; cf.bin: c[n], d[n], q (f64);
// params.txt: lm_maxiter scg_maxiter.
#include <cstdio>
#include <cstring>
#include <fstream>
#include <iostream>
#include <memory>
#include <string>
#include <vector>
#include "armawrap/newmat.h"
#include "miscmaths/bfmatrix.h"
#include "miscmaths/nonlin.h"
#include "miscmaths/SpMat.h"
#include "utils/threading.h"

using namespace std;
using namespace MISCMATHS;

static vector<char> slurp(const string& f)
{
  ifstream in(f, ios::binary);
  return vector<char>((istreambuf_iterator<char>(in)), istreambuf_iterator<char>());
}

struct Csc { unsigned int m, n, nnz; vector<unsigned int> irp, jcp; vector<double> val; };

static Csc read_csc(const string& f)
{
  vector<char> b = slurp(f);
  Csc c; const char* p = b.data();
  memcpy(&c.m, p, 4); memcpy(&c.n, p + 4, 4); memcpy(&c.nnz, p + 8, 4); p += 12;
  c.irp.resize(c.nnz); memcpy(c.irp.data(), p, 4 * c.nnz); p += 4 * c.nnz;
  c.jcp.resize(c.n + 1); memcpy(c.jcp.data(), p, 4 * (c.n + 1)); p += 4 * (c.n + 1);
  c.val.resize(c.nnz); memcpy(c.val.data(), p, 8 * c.nnz);
  return c;
}

static NEWMAT::ColumnVector read_vec(const string& f, int n, int off = 0)
{
  vector<char> b = slurp(f);
  NEWMAT::ColumnVector v(n);
  memcpy(v.Store(), b.data() + 8 * off, 8 * n);
  return v;
}

static void pd(const char* tag, double v)
{
  unsigned long long b; memcpy(&b, &v, 8);
  printf("%s %016llx\n", tag, b);
}

static void pvec(const char* tag, const NEWMAT::ColumnVector& v)
{
  printf("%s n=%d\n", tag, int(v.Nrows()));
  for (int i = 1; i <= v.Nrows(); i++) pd(tag, v(i));
}

template<class T>
static void psp(const char* tag, const SpMat<T>& sp)
{
  printf("%s m=%u n=%u nz=%u\n", tag, sp.Nrows(), sp.Ncols(), sp.NZ());
  for (unsigned int c = 1; c <= sp.Ncols(); c++) {
    for (typename SpMat<T>::ColumnIterator it = sp.begin(c); it != sp.end(c); ++it) {
      unsigned long long b; double v = double(*it); memcpy(&b, &v, 8);
      printf("%s %u %u %016llx\n", tag, c - 1, it.Row() - 1, b);
    }
  }
}

static void pbf(const char* tag, const BFMatrix& m)
{
  if (const SparseBFMatrix<float>* s = dynamic_cast<const SparseBFMatrix<float>*>(&m)) psp(tag, s->AsSpMat());
  else if (const SparseBFMatrix<double>* s = dynamic_cast<const SparseBFMatrix<double>*>(&m)) psp(tag, s->AsSpMat());
  else {
    NEWMAT::Matrix a = m.AsMatrix();
    printf("%s full %d %d\n", tag, int(a.Nrows()), int(a.Ncols()));
    for (int c = 1; c <= a.Ncols(); c++) for (int r = 1; r <= a.Nrows(); r++) pd(tag, a(r, c));
  }
}

// The test cost-function: sum_i (p_i - c_i)^2 + 0.5 (p_i^2 - d_i)^2 + sum_i q (p_{i+1} - p_i)^2,
// its gradient and its (tridiagonal) Hessian as a sparse matrix of precision `prec` (or dense:
// `full`); `numeric`: FSL's finite-difference grad and hess.
class TestCF : public NonlinCF
{
public:
  TestCF(const NEWMAT::ColumnVector& c, const NEWMAT::ColumnVector& d, double q, int mode)
    : _c(c), _d(d), _q(q), _mode(mode) {}
  virtual double cf(const NEWMAT::ColumnVector& p) const {
    int n = p.Nrows(); double s = 0.0;
    for (int i = 1; i <= n; i++) {
      double t1 = p(i) - _c(i); double t2 = p(i) * p(i) - _d(i);
      s += t1 * t1 + 0.5 * (t2 * t2);
    }
    for (int i = 1; i < n; i++) { double t = p(i + 1) - p(i); s += _q * (t * t); }
    return s;
  }
  virtual NEWMAT::ReturnMatrix grad(const NEWMAT::ColumnVector& p) const {
    if (_mode == 3) return NonlinCF::grad(p);
    int n = p.Nrows(); NEWMAT::ColumnVector g(n);
    for (int i = 1; i <= n; i++) {
      double t2 = p(i) * p(i) - _d(i);
      double v = 2.0 * (p(i) - _c(i)) + 2.0 * (t2 * p(i));
      if (i > 1) v += 2.0 * _q * (p(i) - p(i - 1));
      if (i < n) v -= 2.0 * _q * (p(i + 1) - p(i));
      g(i) = v;
    }
    g.Release(); return g;
  }
  virtual std::shared_ptr<BFMatrix> hess(const NEWMAT::ColumnVector& p, std::shared_ptr<BFMatrix> iptr) const {
    if (_mode == 3) return NonlinCF::hess(p, iptr);
    int n = p.Nrows();
    vector<unsigned int> irp, jcp; vector<double> val;
    for (int c = 1; c <= n; c++) {
      jcp.push_back(irp.size());
      if (c > 1) { irp.push_back(c - 2); val.push_back(-2.0 * _q); }
      double dg = 2.0 + 2.0 * (3.0 * p(c) * p(c) - _d(c));
      if (c > 1) dg += 2.0 * _q;
      if (c < n) dg += 2.0 * _q;
      irp.push_back(c - 1); val.push_back(dg);
      if (c < n) { irp.push_back(c); val.push_back(-2.0 * _q); }
    }
    jcp.push_back(irp.size());
    if (_mode == 0) return std::shared_ptr<BFMatrix>(new SparseBFMatrix<double>(n, n, irp.data(), jcp.data(), val.data()));
    if (_mode == 1) return std::shared_ptr<BFMatrix>(new SparseBFMatrix<float>(n, n, irp.data(), jcp.data(), val.data()));
    SpMat<double> s(n, n, irp.data(), jcp.data(), val.data());
    return std::shared_ptr<BFMatrix>(new FullBFMatrix(s.AsNEWMAT()));
  }
private:
  NEWMAT::ColumnVector _c, _d; double _q; int _mode;
};

template<class T>
static void spmat_ops(const char* pre, const Csc& A, const Csc& B, const NEWMAT::Matrix& D,
                      const NEWMAT::ColumnVector& x, const NEWMAT::ColumnVector& b)
{
  string p(pre);
  auto tag = [&](const char* t) { static string s; s = p + t; return s.c_str(); };
  SpMat<T> a(A.m, A.n, A.irp.data(), A.jcp.data(), A.val.data());
  SpMat<T> bb(B.m, B.n, B.irp.data(), B.jcp.data(), B.val.data());
  psp(tag("A"), a);
  pvec(tag("Ax"), a * x);
  for (unsigned int nt : {2u, 3u, 4u, 7u}) {
    SpMat<T> at(a); at.SetNthreads(nt);
    char t[32]; snprintf(t, sizeof t, "Ax_nt%u", nt);
    pvec(tag(t), at * x);
  }
  pvec(tag("Atx"), a.trans_mult(x));
  { SpMat<T> c(a); c += a; psp(tag("A+A"), c); }
  { SpMat<T> c(a); c += bb; psp(tag("A+B"), c); c *= 0.3; psp(tag("(A+B)*0.3"), c); }
  { SpMat<T> c(a); c -= bb; psp(tag("A-=B"), c); }
  { SpMat<T> c = 0.7 * bb; SpMat<T> e(a); e += c; psp(tag("A+0.7B"), e); }
  { SpMat<T> c(bb); c += a; psp(tag("B+A"), c); SpMat<T> e(c); e += bb; psp(tag("B+A+B"), e); }
  psp(tag("At"), a.t());
  psp(tag("AtA"), a.TransMultSelf());
  psp(tag("A*B"), a * bb);
  { SpMat<T> c(a); c.Set(1, A.n, T(1.25)); c.AddTo(2, 1, T(0.5)); c.AddTo(A.m, 2, T(-3.0)); psp(tag("set"), c);
    pd(tag("peek"), double(c.Peek(1, A.n))); pd(tag("peek"), double(c.Peek(A.m, 1))); }
  { NEWMAT::Matrix Dt = D.t(); SpMat<T> c(a); c |= SpMat<T>(Dt); psp(tag("A|Dt"), c); }
  { SpMat<T> c(a); c &= SpMat<T>(D); psp(tag("A&D"), c); }
  for (double tol : {1e-3, 1e-10}) {
    pvec(tag("cg"), a.SolveForx(b, SYM_POSDEF, tol, 500));
    pvec(tag("bicg"), a.SolveForx(b, UNKNOWN, tol, 500));
  }
  pvec(tag("cg_it3"), a.SolveForx(b, SYM_POSDEF, 1e-12, 3));
  pvec(tag("bicg_it3"), a.SolveForx(b, ASYM, 1e-12, 3));
  // The topup Hessian's assembly: nonlin_bit (sparse) with a dense interaction block below and
  // to the right, AddToMe of a scaled sparse matrix and of its transpose.
  std::shared_ptr<BFMatrix> nb(new SparseBFMatrix<T>(A.m, A.n, A.irp.data(), A.jcp.data(), A.val.data()));
  std::shared_ptr<BFMatrix> tmp(new SparseBFMatrix<T>(B.m, B.n, B.irp.data(), B.jcp.data(), B.val.data()));
  nb->AddToMe(*tmp);
  nb->AddToMe(*(tmp->Transpose()));
  nb->MulMeByScalar(2.0 / 37.0);
  nb->AddToMe(*tmp, 0.001);
  pbf(tag("bf"), *nb);
  NEWMAT::Matrix inter = D.t();
  std::shared_ptr<BFMatrix> bfi(new FullBFMatrix(inter));
  NEWMAT::Matrix mv(D.Nrows(), D.Nrows()); mv = 0.0;
  for (int i = 1; i <= D.Nrows(); i++) { mv(i, i) = 3.0 + i; if (i > 1) { mv(i, i - 1) = 0.25; mv(i - 1, i) = 0.25; } }
  std::shared_ptr<BFMatrix> move(new FullBFMatrix(mv));
  nb->VertConcatBelowMe(*(bfi->Transpose()));
  bfi->VertConcatBelowMe(*move);
  nb->HorConcat2MyRight(*bfi);
  pbf(tag("bfcat"), *nb);
  NEWMAT::ColumnVector bx(nb->Ncols());
  for (int i = 1; i <= bx.Nrows(); i++) bx(i) = (i <= x.Nrows()) ? x(i) : 0.5 * i;
  pvec(tag("bfcat*x"), nb->MulByVec(bx));
  pvec(tag("bfcat\\x"), nb->SolveForx(bx, SYM_POSDEF, 1e-8, 200));
}

static void run_nonlin(const char* tag, NLMethod m, int mode, const NEWMAT::ColumnVector& c,
                       const NEWMAT::ColumnVector& d, double q, int maxiter, bool lmset)
{
  TestCF cf(c, d, q, mode);
  NEWMAT::ColumnVector start(c.Nrows()); start = 0.0;
  NonlinParam np(c.Nrows(), m, start);
  if (lmset) { np.SetEquationSolverMaxIter(500); np.SetEquationSolverTol(1.0e-3); }
  np.SetMaxIter(maxiter);
  NonlinOut st = nonlin(np, cf);
  printf("%s status %d niter %d\n", tag, int(st), np.NIter());
  pd(tag, np.CF());
  pd(tag, np.Lambda());
  pvec(tag, np.Par());
}

int main(int argc, char* argv[])
{
  if (argc < 2) { cerr << "usage: miscmaths_probe <case dir>" << endl; return 2; }
  string dir(argv[1]);
  int lm_it, scg_it;
  { ifstream p(dir + "/params.txt"); p >> lm_it >> scg_it; }
  try {
    Csc A = read_csc(dir + "/A.csc"), B = read_csc(dir + "/B.csc");
    vector<char> db = slurp(dir + "/D.bin");
    unsigned int dr, dc; memcpy(&dr, db.data(), 4); memcpy(&dc, db.data() + 4, 4);
    NEWMAT::Matrix D(dr, dc);
    for (unsigned int c = 0; c < dc; c++) for (unsigned int r = 0; r < dr; r++) {
      double v; memcpy(&v, db.data() + 8 + 8 * (c * dr + r), 8); D(r + 1, c + 1) = v;
    }
    NEWMAT::ColumnVector x = read_vec(dir + "/x.bin", A.n), b = read_vec(dir + "/b.bin", A.n);
    spmat_ops<double>("d_", A, B, D, x, b);
    spmat_ops<float>("f_", A, B, D, x, b);
    int n = A.n;
    NEWMAT::ColumnVector c = read_vec(dir + "/cf.bin", n, 0), d = read_vec(dir + "/cf.bin", n, n);
    double q = read_vec(dir + "/cf.bin", 1, 2 * n)(1);
    run_nonlin("lm_d", NL_LM, 0, c, d, q, lm_it, true);
    run_nonlin("lm_f", NL_LM, 1, c, d, q, lm_it, true);
    run_nonlin("lm_def", NL_LM, 0, c, d, q, lm_it, false);
    run_nonlin("scg", NL_SCG, 0, c, d, q, scg_it, false);
    run_nonlin("scg_short", NL_SCG, 0, c, d, q, 3, false);
    if (n <= 8) {
      run_nonlin("lm_full", NL_LM, 2, c, d, q, lm_it, true);
      run_nonlin("lm_numeric", NL_LM, 3, c, d, q, lm_it, true);
    }
  }
  catch (const std::exception& e) {
    printf("exception %s\n", e.what());
  }
  return 0;
}
