// SPDX-License-Identifier: Apache-2.0
// fs.robust's probe: FreeSurfer 8.2.0's robust-registration helpers (mri_robust_register/'s
// MyMRI.cpp, MyMatrix.cpp, CostFunctions.cpp, compiled from the tag with the official flags and
// linked with the unit oracle's libutils, ITK 4.13.2's VNL and Ubuntu 22.04's LAPACK and BLAS as the
// official programs are: build.sh) called case by case. CLOSED TO FREESURFER CLEAN-ROOM
// IMPLEMENTERS: it names FreeSurfer's functions (running it is a black-box run of FreeSurfer's
// code).
//
//     fsrobust <cases>     one output line per case line (absolute path; see check.py)
//
// Every case runs in a child process whose standard output and error go to a scratch file beside
// the cases; the parent prints `<result> | <text>`, the text C-escaped, or `EXIT <status> | <text>`
// / `CRASH <signal> | <text>` when FreeSurfer's code ended the child. Floats and doubles in and out
// are their bits in hex. The volumes of the cases are fs-volume's (fsvolume.cpp's get_vol).

#include <dlfcn.h>
#include <fcntl.h>
#include <signal.h>
#include <sys/wait.h>
#include <unistd.h>

#include <cerrno>
#include <cinttypes>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>

#include "MyMatrix.h"
#include "CostFunctions.h"
// getPrefilter and getDerfilter are private members: opened for the probe (their own headers are
// already included, so only MyMRI's class is affected; the layout does not change)
#define private public
#include "MyMRI.h"
#undef private

// 3.2-03 (relay 42): the registration step's classes. Their own headers and every header they
// include first (so the access change below touches only these classes; the layout does not
// change): the protected and private members of Regression, RegistrationStep and RegRobust (its
// fields the step reads) are opened for the probe.
#include <fstream>
#include <limits>
#include <sstream>
#include <typeinfo>
#include <vnl/algo/vnl_qr.h>
#include <vnl/algo/vnl_svd.h>
#include "Quaternion.h"
#include "Transformation.h"
#include "RobustGaussian.h"
#define private public
#define protected public
#include "Regression.h"
#include "Registration.h"
#include "RegRobust.h"
#include "RegistrationStep.h"
#undef protected
#undef private

#include "error.h"
#include "matrix.h"
#include "mri.h"

// cabs and csqrt (LAPACK's ABS and SQRT of a complex number, std::abs and std::sqrt of
// std::complex<double>) are linked with --wrap to complexrt.c's: the correctly rounded hypot in both
// (the project's decision for MatrixSqrt, track C relay 41)

// the double acos of the official code (linked with --wrap=acos, build.sh): the project decided
// the correctly rounded acos for reproduce too (glibc's is not: GLIBC-07; the decision of
// 2026-11-04 applied to FreeSurfer, track C relay 38): CORE-MATH's (oracles/core-math-preload)
// for numbers, glibc's for a NaN argument
extern "C" double __real_acos(double);
namespace {
double (*cr_acos)(double) = nullptr;
}
extern "C" double __wrap_acos(double x)
{
  if (!cr_acos || x != x) return __real_acos(x);
  return cr_acos(x);
}

const char *Progname = "fsrobust";

namespace {

std::vector<std::string> toks;
size_t pos;
std::string out;
std::string scratch;

[[noreturn]] void fail(const char *what)
{
  fprintf(stderr, "fsrobust: %s\n", what);
  exit(2);
}

const char *next()
{
  if (pos >= toks.size()) fail("missing token");
  return toks[pos++].c_str();
}
int geti() { return (int)strtol(next(), NULL, 10); }
float getf()
{
  uint32_t u = (uint32_t)strtoul(next(), NULL, 16);
  float f;
  memcpy(&f, &u, 4);
  return f;
}
double getd()
{
  uint64_t u = (uint64_t)strtoull(next(), NULL, 16);
  double d;
  memcpy(&d, &u, 8);
  return d;
}
std::string gets() { std::string s = next(); return s == "-" ? std::string() : s; }

void oi(long long v) { out += std::to_string(v) + " "; }
void of(float f)
{
  uint32_t u;
  memcpy(&u, &f, 4);
  char b[16];
  snprintf(b, sizeof b, "%08x ", u);
  out += b;
}
void od(double d)
{
  uint64_t u;
  memcpy(&u, &d, 8);
  char b[24];
  snprintf(b, sizeof b, "%016" PRIx64 " ", u);
  out += b;
}
void ohex(const std::string &s)
{
  if (s.empty()) { out += "- "; return; }
  char h[4];
  for (unsigned char c : s) { snprintf(h, sizeof h, "%02x", c); out += h; }
  out += " ";
}

void om(const MATRIX *m)
{
  if (!m) { out += "NULL "; return; }
  oi(m->rows);
  oi(m->cols);
  for (int r = 1; r <= m->rows; r++)
    for (int c = 1; c <= m->cols; c++) of(m->rptr[r][c]);
}
void ovm(const vnl_matrix<double> &m)
{
  oi(m.rows());
  oi(m.cols());
  for (unsigned r = 0; r < m.rows(); r++)
    for (unsigned c = 0; c < m.cols(); c++) od(m[r][c]);
}

void ocm(const vnl_matrix<vcl_complex<double> > &m)
{
  oi(m.rows());
  oi(m.cols());
  for (unsigned r = 0; r < m.rows(); r++)
    for (unsigned c = 0; c < m.cols(); c++) {
      od(m[r][c].real());
      od(m[r][c].imag());
    }
}

void set_geom(VOL_GEOM *v)
{
  v->valid = geti();
  v->width = geti();
  v->height = geti();
  v->depth = geti();
  v->xsize = getf(); v->ysize = getf(); v->zsize = getf();
  v->ras_good_flag = geti();
  v->x_r = getf(); v->x_a = getf(); v->x_s = getf();
  v->y_r = getf(); v->y_a = getf(); v->y_s = getf();
  v->z_r = getf(); v->z_a = getf(); v->z_s = getf();
  v->c_r = getf(); v->c_a = getf(); v->c_s = getf();
  v->s_r = getf(); v->s_a = getf(); v->s_s = getf();
  std::string f = gets();
  strcpy(v->fname, f.c_str());
}
void put_geom(const VOL_GEOM *v)
{
  oi(v->width); oi(v->height); oi(v->depth);
  of(v->xsize); of(v->ysize); of(v->zsize);
  oi(v->ras_good_flag);
  of(v->x_r); of(v->x_a); of(v->x_s);
  of(v->y_r); of(v->y_a); of(v->y_s);
  of(v->z_r); of(v->z_a); of(v->z_s);
  of(v->c_r); of(v->c_a); of(v->c_s);
  of(v->s_r); of(v->s_a); of(v->s_s);
  ohex(std::string(v->fname));
}

// the float voxels the cases draw from (FTAB in tests/fs_robust/cases.rs, fs-volume's table)
const uint32_t FTAB[] = {
  0x00000000u, 0x80000000u, 0x3f800000u, 0xbf800000u, 0x3f000000u, 0x3fc00000u, 0x40200000u,
  0xc0200000u, 0x437f0000u, 0x437f8000u, 0x43800000u, 0xc3962000u, 0x477fff9au, 0x501502f9u,
  0xd01502f9u, 0x7f7fc99eu, 0x000116c2u, 0x7f800000u, 0xff800000u, 0x7fc00000u, 0x7fa12345u,
  0xffc00001u, 0x40490fd0u, 0x24e69595u, 0x42fecccdu, 0x3dcccccdu, 0x41200000u, 0xc1a00000u};
const size_t NF = sizeof FTAB / sizeof FTAB[0];

// a volume of the case (fs-volume's get_vol): w h d type nframes hdr seed, its geometry, its header
// fields and what to do with its cached matrices (0 keep, 1 MRIreInitCache, 2 drop them); then n
// voxels painted (x y z frame value), so a case can hold ordinary images
MRI *get_vol()
{
  int w = geti(), h = geti(), d = geti(), type = geti(), nf = geti(), hdr = geti();
  unsigned seed = (unsigned)geti();
  MRI *m = hdr ? MRIallocHeader(w, h, d, type, nf) : MRIallocSequence(w, h, d, type, nf);
  if (m->chunk && m->ischunked) {
    unsigned char *b = (unsigned char *)m->chunk;
    if (type == MRI_FLOAT) {
      for (size_t i = 0; i < m->bytes_total / 4; i++) memcpy(b + 4 * i, &FTAB[(i * 7 + seed) % NF], 4);
    } else {
      for (size_t k = 0; k < m->bytes_total; k++)
        b[k] = (unsigned char)((k * 37u + seed * 101u + (k >> 3) * 13u) & 0xffu);
    }
  }
  set_geom(m);
  m->width = w; m->height = h; m->depth = d;
  m->imnr0 = geti(); m->imnr1 = geti();
  m->xstart = getf(); m->xend = getf(); m->ystart = getf(); m->yend = getf();
  m->zstart = getf(); m->zend = getf();
  m->thick = getf(); m->ps = getf();
  m->tr = getf(); m->te = getf(); m->ti = getf(); m->flip_angle = getd();
  m->dof = geti();
  int cache = geti();
  if (cache == 1) MRIreInitCache(m);
  else if (cache == 2) { AffineMatrixFree(&m->i_to_r__); MatrixFree(&m->r_to_i__); }
  int n = geti();
  for (int i = 0; i < n; i++) {
    int x = geti(), y = geti(), z = geti(), f = geti();
    float v = getf();
    MRIsetVoxVal(m, x, y, z, f, v);
  }
  return m;
}

void oam(const AffineMatrix *am)
{
  if (!am) { out += "NULL "; return; }
  for (int i = 0; i < 16; i++) of(am->mat[i]);
}

void put_vol(const MRI *m)
{
  if (!m) { out += "NULL "; return; }
  oi(m->width); oi(m->height); oi(m->depth); oi(m->nframes); oi(m->type);
  oi(m->imnr0); oi(m->imnr1);
  of(m->xstart); of(m->xend); of(m->ystart); of(m->yend); of(m->zstart); of(m->zend);
  of(m->thick); of(m->ps); of(m->tr); of(m->te); of(m->ti); od(m->flip_angle); oi(m->dof);
  od(m->outside_val);
  put_geom(m);
  oam(m->i_to_r__);
  om(m->r_to_i__);
  if (m->chunk && m->ischunked) {
    std::string b((const char *)m->chunk, m->bytes_total);
    ohex(b);
  } else {
    out += "nochunk ";
  }
}

// a vnl matrix of the case: rows cols, then the doubles row by row
vnl_matrix<double> get_vm()
{
  int r = geti(), c = geti();
  vnl_matrix<double> m(r, c);
  for (int i = 0; i < r; i++)
    for (int j = 0; j < c; j++) m[i][j] = getd();
  return m;
}
vnl_matrix_fixed<double, 4, 4> get_m4()
{
  vnl_matrix_fixed<double, 4, 4> m;
  for (int i = 0; i < 4; i++)
    for (int j = 0; j < 4; j++) m[i][j] = getd();
  return m;
}

// ---- 3.2-03 (relay 42): RobustGaussian, Regression, Quaternion, the transformations and the
// registration step

template <class T> T getv();
template <> float getv<float>() { return getf(); }
template <> double getv<double>() { return getd(); }
void ov(float v) { of(v); }
void ov(double v) { od(v); }
template <class T> void ovec(const vnl_vector<T> &v)
{
  oi(v.size());
  for (unsigned i = 0; i < v.size(); i++) ov(v[i]);
}
template <class T> void oarr(const T *a, int n)
{
  for (int i = 0; i < n; i++) ov(a[i]);
}

// RobustGaussian<T>::<op>(a, n[, k | d]): the result, then the array as the call left it
template <class T> void rgauss()
{
  std::string op = next();
  int n = geti();
  std::vector<T> a(n > 0 ? n : 1);
  for (int i = 0; i < n; i++) a[i] = getv<T>();
  T *p = a.data();
  if (op == "median") {
    ov(RobustGaussian<T>::median(p, n));
  } else if (op == "medianI") {
    std::pair<T, T> r = RobustGaussian<T>::medianI(p, n);
    ov(r.first);
    ov(r.second);
  } else if (op == "mad") {
    T d = getv<T>();
    ov(RobustGaussian<T>::mad(p, n, d));
  } else if (op == "kth" || op == "qsel" || op == "kthI" || op == "qselI") {
    int k = geti();
    if (op == "kth") ov(RobustGaussian<T>::kth_smallest(p, n, k));
    else if (op == "qsel") ov(RobustGaussian<T>::quick_select(p, n, k));
    else {
      std::pair<T, int> r = op == "kthI" ? RobustGaussian<T>::kth_smallestI(p, n, k)
                                         : RobustGaussian<T>::quick_selectI(p, n, k);
      ov(r.first);
      oi(r.second);
    }
  } else {
    fail("unknown rgauss op");
  }
  oarr(p, n);
}

// Regression<T>: regress <T> <method> <floatsvd> <verbose> <sat> <sig> <hasA> <rows> <cols>
// [A row by row] <b> [<wsize> | <w>]; methods: rob (getRobustEstW), ls (getLSEst), wls
// (getWeightedLSEst), wlsf (getWeightedLSEstFloat), sigma (getSigmaMAD with d), tukey
// (getSqrtTukeyDiaWeights). Prints the result, the weights, the last error, weight and zero
// percentages, and b as the call left it.
template <class T> void regress()
{
  std::string method = next();
  int fsvd = geti(), verbose = geti();
  double sat = getd(), sig = getd();
  int hasA = geti(), rows = geti(), cols = geti();
  vnl_matrix<T> A(hasA ? rows : 0, hasA ? cols : 0);
  if (hasA)
    for (int i = 0; i < rows; i++)
      for (int j = 0; j < cols; j++) A[i][j] = getv<T>();
  vnl_vector<T> b(rows);
  for (int i = 0; i < rows; i++) b[i] = getv<T>();
  Regression<T> *R = hasA ? new Regression<T>(A, b) : new Regression<T>(b);
  R->setVerbose(verbose);
  R->setFloatSvd(fsvd != 0);
  if (method == "rob") {
    int wsize = geti();
    vnl_vector<T> w(wsize);
    for (int i = 0; i < wsize; i++) w[i] = getv<T>();
    vnl_vector<T> p = R->getRobustEstW(w, sat, sig);
    ovec(p);
    ovec(w);
  } else if (method == "ls") {
    ovec(R->getLSEst());
  } else if (method == "wls" || method == "wlsf") {
    vnl_vector<T> w(rows);
    for (int i = 0; i < rows; i++) w[i] = getv<T>();
    ovec(method == "wls" ? R->getWeightedLSEst(w) : R->getWeightedLSEstFloat(w));
  } else if (method == "sigma") {
    T d = getv<T>();
    ov(R->getSigmaMAD(b, d));
  } else if (method == "tukey") {
    vnl_vector<T> w(rows);
    R->getSqrtTukeyDiaWeights(b, w, sat);
    ovec(w);
  } else {
    fail("unknown regress method");
  }
  std::cout << std::flush;
  od(R->getLastError());
  od(R->getLastWeightPercent());
  od(R->getLastZeroWeightPercent());
  ovec(b);
  delete R;
}

// the transformation models by number (Registration::setTransformation's and RegPowell's)
Transformation *make_trans(int code)
{
  switch (code) {
    case 0: return new Transform2dAffine;
    case 1: return new Transform2dAffine2;
    case 2: return new Transform2dIsoscale;
    case 3: return new Transform2dIsoscale2;
    case 4: return new Transform2dRigid;
    case 5: return new Transform2dRigid2;
    case 6: return new Transform2dTranslate;
    case 7: return new Transform3dAffine;
    case 8: return new Transform3dAffine2;
    case 9: return new Transform3dIsoscale;
    case 10: return new Transform3dIsoscale2;
    case 11: return new Transform3dRigid;
    case 12: return new Transform3dRigid2;
    case 13: return new Transform3dTranslate;
    case 14: return new Transform3dIdentity;
  }
  fail("unknown transformation");
}

void put_m4(const vnl_matrix_fixed<double, 4, 4> &m)
{
  for (int i = 0; i < 4; i++)
    for (int j = 0; j < 4; j++) od(m[i][j]);
}

// trans <code> <ptype> <np> <params> <identity> <x> <fx> <y> <fy> <z> <fz>: the transformation's
// parameters set from a std::vector<double> (ptype 0), a vnl_vector<float> (1) or a
// vnl_vector<double> (2), optionally setIdentity() after; its DOF, parameters, steps, matrix and
// gradient (the last two may end the program)
void trans_case()
{
  int code = geti(), ptype = geti(), np = geti();
  std::vector<double> pd(np);
  vnl_vector<float> pf(np);
  vnl_vector<double> pv(np);
  for (int i = 0; i < np; i++) {
    if (ptype == 1) pf[i] = getf();
    else pd[i] = pv[i] = getd();
  }
  int identity = geti();
  unsigned x = (unsigned)geti();
  float fx = getf();
  unsigned y = (unsigned)geti();
  float fy = getf();
  unsigned z = (unsigned)geti();
  float fz = getf();
  Transformation *t = make_trans(code);
  if (ptype == 0) t->setParameters(pd);
  else if (ptype == 1) t->setParameters(pf);
  else t->setParameters(pv);
  if (identity) t->setIdentity();
  oi(t->getDOF());
  ovec(t->getParameters());
  ovec(t->getSteps());
  std::cout << std::flush;
  put_m4(t->getMatrix());
  ovec(t->getGradient(x, fx, y, fy, z, fz));
  delete t;
}

// quat rv <v1> <v2> <v3> | rva <alpha> <v1> <v2> <v3> | zyx <z> <y> <x>: the quaternion and
// its rotation matrix
void quat_case()
{
  std::string op = next();
  Quaternion q;
  if (op == "rv") {
    double a = getd(), b = getd(), c = getd();
    q.importRotVec(a, b, c);
  } else if (op == "rva") {
    double al = getd(), a = getd(), b = getd(), c = getd();
    q.importRotVec(al, a, b, c);
  } else if (op == "zyx") {
    double a = getd(), b = getd(), c = getd();
    q.importZYXAngles(a, b, c);
  } else {
    fail("unknown quat op");
  }
  od(q.getReal());
  std::vector<double> im = q.getImg();
  for (double v : im) od(v);
  std::vector<double> m = q.getRotMatrix3d();
  for (double v : m) od(v);
}

// rstep <T> <floatsvd> <costfun> <trans> <sat> <iscale> <subsamplesize> <verbose> <calls>, then
// per call the source and target volumes: RegistrationStep<T>(R).computeRegistrationStep(S, T)
// with a RegRobust R whose fields the step reads set from the case; per call the matrix, the
// intensity parameter, the weight checks, the parameters, the weights image and the index image
template <class T> void rstep()
{
  int fsvd = geti(), costfun = geti(), code = geti();
  double sat = getd();
  int iscale = geti(), ss = geti(), verbose = geti(), calls = geti();
  RegRobust R;
  R.sat = sat;
  R.iscale = iscale != 0;
  R.costfun = (Registration::Cost)costfun;
  R.trans = make_trans(code);
  R.subsamplesize = ss;
  R.verbose = verbose;
  R.debug = 0;
  RegistrationStep<T> S(R);
  S.setFloatSVD(fsvd != 0);
  for (int c = 0; c < calls; c++) {
    MRI *ms = get_vol();
    MRI *mt = get_vol();
    std::pair<vnl_matrix_fixed<double, 4, 4>, double> md = S.computeRegistrationStep(ms, mt);
    std::cout << std::flush;
    put_m4(md.first);
    od(md.second);
    if (costfun == Registration::ROB) {
      od(S.getwcheck());
      od(S.getwchecksqrt());
    }
    od(S.getzeroweights());
    ovec(S.pvec);
    put_vol(S.getWeights());
    put_vol(S.mri_indexing);
    MRIfree(&ms);
    MRIfree(&mt);
  }
}

// 3.2-03 part 2 (relay 43): the registration (RegRobust with Registration's code).
//
// A structured volume of the registration cases: w h d type, the geometry's sizes (floats),
// cosines x_r x_a x_s y_r y_a y_s z_r z_a z_s and centre c_r c_a c_s (floats), then the voxels of
// an analytic picture: n blobs (cx cy cz amp r2, doubles), the background and a texture amplitude
// (doubles), and a 3 x 4 voxel transform (doubles, row by row) applied to (x, y, z) first; all in
// double, left to right, no fused operations (the port computes the same bits); a byte or short
// volume takes (int)(v + 0.5) of v clamped to [0, 255] or [0, 32767]; set with MRIsetVoxVal.
MRI *get_blob()
{
  int w = geti(), h = geti(), d = geti(), type = geti();
  MRI *m = MRIalloc(w, h, d, type);
  m->xsize = getf(); m->ysize = getf(); m->zsize = getf();
  m->x_r = getf(); m->x_a = getf(); m->x_s = getf();
  m->y_r = getf(); m->y_a = getf(); m->y_s = getf();
  m->z_r = getf(); m->z_a = getf(); m->z_s = getf();
  m->c_r = getf(); m->c_a = getf(); m->c_s = getf();
  m->ras_good_flag = 1;
  MRIreInitCache(m);
  int nb = geti();
  std::vector<double> b(5 * nb);
  for (double &v : b) v = getd();
  double bg = getd(), tex = getd();
  double A[12];
  for (double &a : A) a = getd();
  for (int z = 0; z < d; z++)
    for (int y = 0; y < h; y++)
      for (int x = 0; x < w; x++) {
        double px = A[0] * x + A[1] * y + A[2] * z + A[3];
        double py = A[4] * x + A[5] * y + A[6] * z + A[7];
        double pz = A[8] * x + A[9] * y + A[10] * z + A[11];
        double v = bg;
        for (int k = 0; k < nb; k++) {
          double dx = px - b[5 * k], dy = py - b[5 * k + 1], dz = pz - b[5 * k + 2];
          double d2 = dx * dx + dy * dy + dz * dz;
          v = v + b[5 * k + 3] * b[5 * k + 4] / (b[5 * k + 4] + d2);
        }
        int ti = ((int)(px * 3.0) + (int)(py * 5.0) + (int)(pz * 7.0)) % 7;
        v = v + tex * ti;
        if (type != MRI_FLOAT) {
          double hi = type == MRI_UCHAR ? 255.0 : 32767.0;
          v = v < 0 ? 0 : v > hi ? hi : v;
          v = (double)(int)(v + 0.5);
        }
        MRIsetVoxVal(m, x, y, z, 0, (float)v);
      }
  return m;
}

// FNV-1a (64 bits) of bytes
uint64_t fnv1a(const unsigned char *p, size_t n)
{
  uint64_t h = 0xcbf29ce484222325ull;
  for (size_t i = 0; i < n; i++) {
    h ^= p[i];
    h *= 0x100000001b3ull;
  }
  return h;
}

// a volume's extent, type, voxel sizes, outside value, cached voxel-to-RAS and its voxels' FNV-1a
void put_vol_hash(const MRI *m)
{
  if (!m) { out += "NULL "; return; }
  oi(m->width); oi(m->height); oi(m->depth); oi(m->nframes); oi(m->type);
  of(m->xsize); of(m->ysize); of(m->zsize); od(m->outside_val);
  oam(m->i_to_r__);
  if (m->chunk && m->ischunked) {
    char b[40];
    snprintf(b, sizeof b, "fnv %016" PRIx64 " %zu ", fnv1a((const unsigned char *)m->chunk, m->bytes_total),
             m->bytes_total);
    out += b;
  } else {
    out += "nochunk ";
  }
}

void put_vm4(const vnl_matrix<double> &m)
{
  if (m.rows() != 4 || m.cols() != 4) { out += "EMPTY "; return; }
  for (int i = 0; i < 4; i++)
    for (int j = 0; j < 4; j++) od(m[i][j]);
}

// rreg <doubleprec> <model: 0 rigid, 1 affine, 2 translation, 3 isoscale, 4 iscale only> <iscale>
//   <symmetry> <sat> <wlimit> <satit> <nomulti> <iterate> <epsit> <highit> <minsize> <maxsize>
//   <subsample> <inittrans> <initorient> <initscaling> <verbose> <keeptype> <minit: 0 | 1 and 16
//   doubles> <iscaleinit> <mov blob> <dst blob>: mri_robust_register's set-up (initRegistration's
//   order), setSourceAndTarget, then findSaturation and computeMultiresRegistration (satit),
//   computeIterativeRegistration (nomulti) or computeMultiresRegistration; prints the images as
//   set, Rsrc, Rtrg, the saturation found, the final voxel transform, the intensity scale,
//   converged, sat, wcheck, Mfinal, the half-way maps, the weights and the centroids
void rreg()
{
  int dbl = geti(), model = geti(), iscale = geti(), sym = geti();
  double sat = getd(), wlimit = getd();
  int satit = geti(), nomulti = geti(), iterate = geti();
  double epsit = getd();
  int highit = geti(), minsize = geti(), maxsize = geti(), ss = geti();
  int inittrans = geti(), initorient = geti(), initscaling = geti(), verbose = geti();
  int keeptype = geti(), hasminit = geti();
  vnl_matrix<double> minit;
  if (hasminit) {
    minit.set_size(4, 4);
    for (int r = 0; r < 4; r++)
      for (int c = 0; c < 4; c++) minit[r][c] = getd();
  }
  double iscaleinit = getd();
  MRI *mov = get_blob();
  MRI *dst = get_blob();
  RegRobust R;
  if (model != 1) R.setRigid();
  else R.setAffine();
  if (model == 2) R.setTransonly();
  if (model == 3) R.setIsoscale();
  R.setIscale(iscale != 0);
  if (model == 4) R.setIscaleOnly();
  R.setVerbose(verbose);
  R.setDebug(0);
  R.setHighit(highit);
  R.setInitTransform(inittrans != 0);
  R.setInitOrient(initorient != 0);
  R.setInitScaling(initscaling != 0);
  R.setDoublePrec(dbl != 0);
  R.setSymmetry(sym != 0);
  R.setCost(Registration::ROB);
  R.setSaturation(sat);
  R.setWLimit(wlimit);
  R.setName("reg");
  if (ss > 0) R.setSubsampleSize(ss);
  R.setMinSize(minsize);
  R.setMaxSize(maxsize);
  if (hasminit) R.setMinitOrig(minit);
  R.setIscaleInit(iscaleinit);
  fflush(stdout);
  R.setSourceAndTarget(mov, dst, keeptype != 0);
  std::cout << std::flush;
  MRIfree(&mov);
  MRIfree(&dst);
  put_vol_hash(R.mri_source);
  put_vol_hash(R.mri_target);
  put_vm4(R.Rsrc);
  put_vm4(R.Rtrg);
  oi(R.resample);
  if (satit) {
    od(R.findSaturation());
    R.computeMultiresRegistration(0, iterate, epsit);
  } else if (nomulti) {
    R.computeIterativeRegistration(iterate, epsit);
  } else {
    R.computeMultiresRegistration(0, iterate, epsit);
  }
  std::cout << std::flush;
  put_m4(R.getFinalVox2Vox());
  std::cout << std::flush;
  od(R.getFinalIscale());
  oi(R.getConverged());
  od(R.sat);
  // wcheck is never written before the first iteration (memory contents): printed after one only
  if (R.mov2weights.rows() == 4) od(R.wcheck);
  else out += "nowcheck ";
  put_vm4(R.Mfinal);
  if (R.mov2weights.rows() == 4) {
    std::pair<vnl_matrix_fixed<double, 4, 4>, vnl_matrix_fixed<double, 4, 4> > hw = R.getHalfWayMaps();
    std::cout << std::flush;
    put_m4(hw.first);
    put_m4(hw.second);
  } else {
    out += "nohalfway ";
  }
  put_vol_hash(R.getWeights());
  put_vol_hash(R.mri_hweights);
  if (R.centroidS.size() == 3 && R.centroidT.size() == 3) {
    vnl_vector_fixed<double, 4> c = R.getCentroidS();
    for (int i = 0; i < 4; i++) od(c[i]);
    c = R.getCentroidT();
    for (int i = 0; i < 4; i++) od(c[i]);
    c = R.getCentroidSinT();
    std::cout << std::flush;
    for (int i = 0; i < 4; i++) od(c[i]);
  } else {
    out += "nocentroids ";
  }
}

std::string slurp(const std::string &path)
{
  std::string r;
  if (FILE *f = fopen(path.c_str(), "rb")) {
    int ch;
    while ((ch = fgetc(f)) != EOF) r += (char)ch;
    fclose(f);
  }
  return r;
}

void run_case()
{
  std::string cmd = next();
  if (cmd == "vscale") {
    // MRIvalscale(src, NULL | src, s, b)
    MRI *src = get_vol();
    int same = geti();
    double s = getd(), b = getd();
    MRI *r = MyMRI::MRIvalscale(src, same ? src : NULL, s, b);
    oi(r == src);
    put_vol(r);
  } else if (cmd == "prefilter") {
    put_vol(MyMRI::getPrefilter());
  } else if (cmd == "derfilter") {
    put_vol(MyMRI::getDerfilter());
  } else if (cmd == "rand") {
    int p = geti();
    double v = MyMRI::getRand(p);
    od(v);
    oi(p);
  } else if (cmd == "subsample") {
    MRI *src = get_vol();
    int fix = geti(), randpos = geti();
    put_vol(MyMRI::subSample(src, NULL, fix != 0, randpos));
  } else if (cmd == "blur") {
    MRI *src = get_vol();
    int same = geti();
    MRI *r = MyMRI::getBlur(src, same ? src : NULL);
    oi(r == src);
    put_vol(r);
  } else if (cmd == "partials") {
    MRI *src = get_vol();
    MRI *fx = NULL, *fy = NULL, *fz = NULL, *bl = NULL;
    bool ok = MyMRI::getPartials(src, fx, fy, fz, bl);
    oi(ok);
    put_vol(fx);
    put_vol(fy);
    put_vol(fz);
    put_vol(bl);
  } else if (cmd == "rightsize") {
    MRI *src = get_vol();
    float cs = getf();
    int conform = geti();
    std::vector<int> r = MyMRI::findRightSize(src, cs, conform != 0);
    for (int v : r) oi(v);
  } else if (cmd == "bg") {
    MRI *src = get_vol();
    of(MyMRI::getBackground(src));
  } else if (cmd == "ltx") {
    // MyMRI::MRIlinearTransform(src, NULL | a volume of the case, m)
    MRI *src = get_vol();
    int given = geti();
    MRI *dst = given ? get_vol() : NULL;
    vnl_matrix_fixed<double, 4, 4> m = get_m4();
    MRI *r = MyMRI::MRIlinearTransform(src, dst, m);
    oi(r != NULL && r == dst);
    put_vol(r);
  } else if (cmd == "centroid") {
    MRI *src = get_vol();
    src->outside_val = getd();
    std::vector<double> c = CostFunctions::centroid(src);
    for (double v : c) od(v);
  } else if (cmd == "orient") {
    MRI *src = get_vol();
    vnl_matrix_fixed<double, 3, 3> e = CostFunctions::orientation(src);
    for (int i = 0; i < 3; i++)
      for (int j = 0; j < 3; j++) od(e[i][j]);
  } else if (cmd == "v2m") {
    vnl_matrix<double> m = get_vm();
    MATRIX *r = MyMatrix::convertVNL2MATRIX(m, NULL);
    om(r);
  } else if (cmd == "vv2m") {
    int n = geti();
    vnl_vector<double> v(n);
    for (int i = 0; i < n; i++) v[i] = getd();
    om(MyMatrix::convertVNL2MATRIX(v, NULL));
  } else if (cmd == "m2v") {
    int r = geti(), c = geti();
    MATRIX *m = MatrixAlloc(r, c, MATRIX_REAL);
    for (int i = 1; i <= r; i++)
      for (int j = 1; j <= c; j++) m->rptr[i][j] = getf();
    ovm(MyMatrix::convertMATRIX2VNL(m));
  } else if (cmd == "getvnl") {
    int n = geti();
    std::vector<double> d(n);
    for (int i = 0; i < n; i++) d[i] = getd();
    int r = geti();
    ovm(MyMatrix::getVNLMatrix(d, r));
  } else if (cmd == "isdiag") {
    vnl_matrix<double> m = get_vm();
    double eps = getd();
    oi(MyMatrix::isDiag(m, eps));
  } else if (cmd == "polar") {
    vnl_matrix<double> a = get_vm();
    vnl_matrix<double> r, s;
    MyMatrix::PolarDecomposition(a, r, s);
    ovm(r);
    ovm(s);
  } else if (cmd == "polar2") {
    vnl_matrix<double> a = get_vm();
    vnl_matrix<double> r, s;
    vnl_diag_matrix<double> d;
    MyMatrix::Polar2Decomposition(a, r, s, d);
    ovm(r);
    ovm(s);
    for (unsigned i = 0; i < d.size(); i++) od(d[i]);
  } else if (cmd == "adist") {
    vnl_matrix<double> a = get_m4().as_matrix();
    int hasb = geti();
    vnl_matrix<double> b = hasb ? get_m4().as_matrix() : vnl_matrix<double>();
    double r = getd();
    od(MyMatrix::AffineTransDistSq(a, b, r));
  } else if (cmd == "rdist") {
    vnl_matrix<double> a = get_m4().as_matrix();
    int hasb = geti();
    vnl_matrix<double> b = hasb ? get_m4().as_matrix() : vnl_matrix<double>();
    od(MyMatrix::RigidTransDistSq(a, b));
  } else if (cmd == "fdiff") {
    vnl_matrix<double> a = get_vm();
    vnl_matrix<double> b(a.rows(), a.cols());
    for (unsigned i = 0; i < a.rows(); i++)
      for (unsigned j = 0; j < a.cols(); j++) b[i][j] = getd();
    od(MyMatrix::getFrobeniusDiff(a, b));
  } else if (cmd == "rlog") {
    od(MyMatrix::RotMatrixLogNorm(get_m4()));
  } else if (cmd == "schur") {
    // MyMatrix::SchurComplex(A, U, T) of a square matrix (LAPACK's zgees)
    vnl_matrix<double> a = get_vm();
    unsigned n = a.rows();
    vnl_matrix<vcl_complex<double> > u(n, n), t(n, n);
    MyMatrix::SchurComplex(a, u, t);
    ocm(u);
    ocm(t);
  } else if (cmd == "sqrtm") {
    // MyMatrix::MatrixSqrt(A) of a square matrix
    ovm(MyMatrix::MatrixSqrt(get_vm()));
  } else if (cmd == "rgauss") {
    if (geti()) rgauss<double>();
    else rgauss<float>();
  } else if (cmd == "regress") {
    if (geti()) regress<double>();
    else regress<float>();
  } else if (cmd == "trans") {
    trans_case();
  } else if (cmd == "quat") {
    quat_case();
  } else if (cmd == "rstep") {
    if (geti()) rstep<double>();
    else rstep<float>();
  } else if (cmd == "rreg") {
    rreg();
  } else {
    fail("unknown command");
  }
}

std::string escape(const std::string &s)
{
  std::string r;
  char b[8];
  for (unsigned char c : s) {
    if (c < 0x20 || c > 0x7e || c == '\\') {
      snprintf(b, sizeof b, "\\x%02x", c);
      r += b;
    } else {
      r += (char)c;
    }
  }
  return r;
}

const char *MARK = "\n@@RESULT ";

}  // namespace

int main(int argc, char *argv[])
{
  if (argc != 2) {
    fprintf(stderr, "usage: fsrobust <cases>\n");
    return 2;
  }
  if (argv[1][0] != '/') fail("the cases must be given by an absolute path");
  scratch = std::string(argv[1]) + ".out";
  {
    char self[4096];
    ssize_t k = readlink("/proc/self/exe", self, sizeof self - 1);
    if (k <= 0) fail("readlink /proc/self/exe");
    self[k] = 0;
    std::string so = std::string(self);
    so = so.substr(0, so.rfind('/')) + "/../../core-math-preload/libcrmath.so";
    void *h = dlopen(so.c_str(), RTLD_NOW | RTLD_LOCAL);
    if (!h) fail("cannot load oracles/core-math-preload/libcrmath.so");
    cr_acos = (double (*)(double))dlsym(h, "acos");
    if (!cr_acos) fail("no acos in libcrmath.so");
  }
  FILE *fp = fopen(argv[1], "r");
  if (!fp) fail("cannot open the cases");
  std::vector<std::string> lines;
  static char buf[1 << 20];
  while (fgets(buf, sizeof buf, fp)) lines.push_back(buf);
  fclose(fp);
  for (const std::string &line : lines) {
    if (line.size() >= sizeof buf - 1) fail("a case longer than the buffer");
    fflush(stdout);
    pid_t pid = fork();
    if (pid < 0) fail("fork");
    if (pid == 0) {
      int fd = open(scratch.c_str(), O_WRONLY | O_CREAT | O_TRUNC, 0600);
      if (fd < 0) _exit(3);
      dup2(fd, 1);
      dup2(fd, 2);
      close(fd);
      setvbuf(stdout, NULL, _IONBF, 0);
      // a registration (rreg) gets 60 s, every other case 5
      alarm(line.compare(0, 5, "rreg ") == 0 ? 60 : 5);
      memcpy(buf, line.c_str(), line.size() + 1);
      toks.clear();
      pos = 0;
      for (char *t = strtok(buf, " \t\n"); t; t = strtok(NULL, " \t\n")) toks.push_back(t);
      if (!toks.empty()) {
        out.clear();
        Gerror = NO_ERROR;
        errno = 0;
        run_case();
        if (!out.empty() && out.back() == ' ') out.pop_back();
        fputs(MARK, stdout);
        fputs(out.c_str(), stdout);
      }
      fflush(stdout);
      _exit(0);
    }
    int status = 0;
    waitpid(pid, &status, 0);
    std::string text = slurp(scratch);
    std::string result;
    size_t at = text.rfind(MARK);
    bool clean = WIFEXITED(status) && WEXITSTATUS(status) == 0 && at != std::string::npos;
    if (clean) {
      result = text.substr(at + strlen(MARK));
      text = text.substr(0, at);
    } else if (WIFSIGNALED(status)) {
      result = "CRASH " + std::to_string(WTERMSIG(status));
    } else {
      result = "EXIT " + std::to_string(WEXITSTATUS(status));
    }
    printf("%s | %s\n", result.c_str(), escape(text).c_str());
  }
  unlink(scratch.c_str());
  return 0;
}
