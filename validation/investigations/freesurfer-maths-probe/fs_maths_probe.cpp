// SPDX-License-Identifier: Apache-2.0
// A black-box probe of FreeSurfer 8.2.0's mathematics (specs/freesurfer-maths.md), linked against
// the unit oracle's libutils.a (GCC 8.4.0, FreeSurfer's flags). It holds only calls of FreeSurfer's
// functions (and of the C library's) by name, the reading and printing of their arguments and
// results, and the probe's own test costs for the minimizers: no FreeSurfer code. The commands,
// encodings and costs are in README.md beside this file; a candidate program takes the same ones.
//
//   fs_maths_probe batch <file>   one call per line of <file>, each in a child process of its own
//                                 with an alarm; one result line per call on standard output
#include <fcntl.h>
#include <signal.h>
#include <sys/wait.h>
#include <unistd.h>

#include <cerrno>
#include <cinttypes>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>

#include "affine.h"
#include "diag.h"
#include "dmatrix.h"
#include "error.h"
#include "histo.h"
#include "matrix.h"
#include "numerics.h"
#include "pdf.h"
#include "prime.h"
#include "randomfields.h"
#include "utils.h"

// functions of FreeSurfer's library that its headers do not declare
int MatrixIsSymmetric(MATRIX *matrix);
MATRIX *MatrixCalculateEigenSystemHelper(MATRIX *m, float *evalues, MATRIX *m_evectors, int isSymmetric);
MATRIX *MatrixSVDPseudoInverse(MATRIX *m, MATRIX *m_pseudo_inv);
extern void (*powell_iteration_func)(float *p, int nparms);

const char *Progname = "fs_maths_probe";

// the guard allocator's switch (guard_malloc.c), when it is preloaded (the guard runs)
extern "C" void guard_malloc_enable(void) __attribute__((weak));

typedef std::vector<std::string> Toks;

static Toks T;
static size_t P;
static std::string R;  // the result line

[[noreturn]] static void bad(const char *what)
{
  fprintf(stderr, "probe: %s\n", what);
  _exit(3);
}

// ---- encodings (README.md, "Encodings") ------------------------------------------------------

static const std::string &tok()
{
  if (P >= T.size()) bad("missing argument");
  return T[P++];
}
static bool peek(const char *s) { return P < T.size() && T[P] == s; }
static int geti() { return (int)strtol(tok().c_str(), NULL, 10); }
static long getl() { return strtol(tok().c_str(), NULL, 10); }
static float getf()
{
  uint32_t u = (uint32_t)strtoul(tok().c_str(), NULL, 16);
  float f;
  memcpy(&f, &u, 4);
  return f;
}
static double getd()
{
  uint64_t u = (uint64_t)strtoull(tok().c_str(), NULL, 16);
  double d;
  memcpy(&d, &u, 8);
  return d;
}
static std::string getx()
{
  const std::string &t = tok();
  if (t.empty() || t[0] != 'x') bad("a string must start with x");
  std::string s;
  for (size_t i = 1; i + 1 < t.size(); i += 2) s.push_back((char)strtol(t.substr(i, 2).c_str(), NULL, 16));
  return s;
}

static void out(const std::string &s)
{
  R += s;
  R += ' ';
}
static void puti(long v) { out(std::to_string(v)); }
static void putf(float f)
{
  uint32_t u;
  memcpy(&u, &f, 4);
  char b[16];
  snprintf(b, sizeof b, "%08" PRIx32, u);
  out(b);
}
static void putd(double d)
{
  uint64_t u;
  memcpy(&u, &d, 8);
  char b[24];
  snprintf(b, sizeof b, "%016" PRIx64, u);
  out(b);
}
static std::string hexbytes(const std::string &s)
{
  std::string o = "x";
  char b[3];
  for (unsigned char c : s) {
    snprintf(b, sizeof b, "%02x", c);
    o += b;
  }
  return o;
}
static std::string slurp(const char *path, bool *found = NULL)
{
  std::string s;
  FILE *fp = fopen(path, "rb");
  if (found) *found = fp != NULL;
  if (!fp) return s;
  int c;
  while ((c = fgetc(fp)) != EOF) s.push_back((char)c);
  fclose(fp);
  return s;
}
static void putfile(const char *path)
{
  bool found;
  std::string s = slurp(path, &found);
  out(found ? hexbytes(s) : "nofile");
}
static void writefile(const char *path, const std::string &bytes)
{
  FILE *fp = fopen(path, "wb");
  if (!fp) bad("cannot write a file in the working directory");
  fwrite(bytes.data(), 1, bytes.size(), fp);
  fclose(fp);
}

// histograms: <alloc> <nbins> <bin_size> <min> <max> <bins: nbins floats> <counts: nbins floats>
static HISTOGRAM *geth()
{
  if (peek("null")) {
    P++;
    return NULL;
  }
  int alloc = geti(), nbins = geti();
  HISTOGRAM *h = HISTOalloc(alloc);
  h->nbins = nbins;
  h->bin_size = getf();
  h->min = getf();
  h->max = getf();
  for (int b = 0; b < nbins; b++) h->bins[b] = getf();
  for (int b = 0; b < nbins; b++) h->counts[b] = getf();
  return h;
}
static void puth(const HISTOGRAM *h)
{
  if (!h) {
    out("null");
    return;
  }
  puti(h->nbins);
  puti(h->max_bins);
  putf(h->bin_size);
  putf(h->min);
  putf(h->max);
  for (int b = 0; b < h->nbins; b++) putf(h->bins[b]);
  for (int b = 0; b < h->nbins; b++) putf(h->counts[b]);
}
// an output argument: new (none given), self (the first input), other (the second), or a histogram
static HISTOGRAM *geth_out(HISTOGRAM *first, HISTOGRAM *second = NULL)
{
  if (peek("new")) {
    P++;
    return NULL;
  }
  if (peek("self")) {
    P++;
    return first;
  }
  if (peek("other")) {
    P++;
    return second;
  }
  return geth();
}
// the returned histogram, then the given output's state (when one was given)
static void puth_res(const HISTOGRAM *res, const HISTOGRAM *given)
{
  if (given && res == given)
    out("given");
  else
    puth(res);
  if (given) puth(given);
}

// float matrices: <type> <rows> <cols> <rows x cols numbers, two floats each when complex>
static int numbers(const MATRIX *m) { return m->rows * m->cols * (m->type == MATRIX_COMPLEX ? 2 : 1); }
static MATRIX *getm()
{
  if (peek("null")) {
    P++;
    return NULL;
  }
  int type = geti(), rows = geti(), cols = geti();
  MATRIX *m = MatrixAlloc(rows, cols, type);
  if (!m) bad("a matrix argument the allocator refused");
  for (int i = 0; i < numbers(m); i++) m->data[i] = getf();
  return m;
}
// a matrix: its type, sizes, and every float of its array from the two before the numbers
static void putm(const MATRIX *m)
{
  if (!m) {
    out("null");
    return;
  }
  puti(m->type);
  puti(m->rows);
  puti(m->cols);
  for (int i = -2; i < numbers(m); i++) putf(m->data[i]);
}
static MATRIX *getm_out(MATRIX *first, MATRIX *second = NULL)
{
  if (peek("new")) {
    P++;
    return NULL;
  }
  if (peek("self")) {
    P++;
    return first;
  }
  if (peek("other")) {
    P++;
    return second;
  }
  return getm();
}
static void putm_res(const MATRIX *res, const MATRIX *given)
{
  if (given && res == given)
    out("given");
  else
    putm(res);
  if (given) putm(given);
}

// double matrices: <rows> <cols> <rows x cols doubles>
static DMATRIX *getdm()
{
  if (peek("null")) {
    P++;
    return NULL;
  }
  int rows = geti(), cols = geti();
  DMATRIX *m = DMatrixAlloc(rows, cols, MATRIX_REAL);
  if (!m) bad("a double matrix argument the allocator refused");
  for (int i = 0; i < rows * cols; i++) m->data[i] = getd();
  return m;
}
static void putdm(const DMATRIX *m)
{
  if (!m) {
    out("null");
    return;
  }
  puti(m->rows);
  puti(m->cols);
  for (int i = 0; i < m->rows * m->cols; i++) putd(m->data[i]);
}
static DMATRIX *getdm_out(DMATRIX *first, DMATRIX *second = NULL)
{
  if (peek("new")) {
    P++;
    return NULL;
  }
  if (peek("self")) {
    P++;
    return first;
  }
  if (peek("other")) {
    P++;
    return second;
  }
  return getdm();
}
static void putdm_res(const DMATRIX *res, const DMATRIX *given)
{
  if (given && res == given)
    out("given");
  else
    putdm(res);
  if (given) putdm(given);
}

// a stream argument: out (standard output), err (standard error), null
static FILE *getstream()
{
  const std::string &t = tok();
  if (t == "out") return stdout;
  if (t == "err") return stderr;
  if (t == "null") return NULL;
  bad("a stream is out, err or null");
}

// ---- the minimizers' test costs (README.md, "The test costs") -------------------------------

struct Cost {
  int kind = 0, n = 0;
  float f0 = 0, b = 0, q = 1;
  float a[33] = {0}, c[33] = {0};
  uint64_t hash = 0xcbf29ce484222325ULL;
  long nf = 0, ng = 0, ni = 0, ns = 0, nu = 0;
} C;

static void mix(uint32_t v)
{
  for (int k = 0; k < 4; k++) {
    C.hash ^= (v >> (8 * k)) & 0xffu;
    C.hash *= 0x100000001b3ULL;
  }
}
static uint32_t bitsf(float f)
{
  uint32_t u;
  memcpy(&u, &f, 4);
  return u;
}
static void mixp(const float *p, int from)
{
  for (int i = from; i <= C.n; i++) mix(bitsf(p[i]));
}
static float cost_value(float p[])
{
  float s = C.f0;
  if (C.kind == 1) {
    for (int i = 1; i < C.n; i++) {
      float t = p[i + 1] - p[i] * p[i];
      float u = 1.0f - p[i];
      s = s + 100.0f * t * t + u * u;
    }
  }
  else {
    for (int i = 1; i <= C.n; i++) {
      float d = p[i] - C.c[i];
      s = C.kind == 3 ? s + C.a[i] * fabsf(d) : s + C.a[i] * d * d;
    }
    if (C.kind != 3)
      for (int i = 1; i < C.n; i++) s = s + C.b * ((p[i] - C.c[i]) * (p[i + 1] - C.c[i + 1]));
    if (C.kind == 2) s = floorf(s * C.q) / C.q;
  }
  C.nf++;
  mix(1);
  mixp(p, 1);
  mix(bitsf(s));
  return s;
}
static void cost_gradient(float p[], float g[])
{
  for (int i = 1; i <= C.n; i++) g[i] = 0.0f;
  if (C.kind == 1) {
    for (int i = 1; i < C.n; i++) {
      float t = p[i + 1] - p[i] * p[i];
      float u = 1.0f - p[i];
      g[i] = g[i] + (-400.0f * t * p[i] - 2.0f * u);
      g[i + 1] = g[i + 1] + 200.0f * t;
    }
  }
  else {
    for (int i = 1; i <= C.n; i++) {
      float d = p[i] - C.c[i];
      float gi = C.kind == 3 ? C.a[i] * (d > 0.0f ? 1.0f : (d < 0.0f ? -1.0f : 0.0f)) : 2.0f * C.a[i] * d;
      if (C.kind != 3 && i > 1) gi = gi + C.b * (p[i - 1] - C.c[i - 1]);
      if (C.kind != 3 && i < C.n) gi = gi + C.b * (p[i + 1] - C.c[i + 1]);
      g[i] = gi;
    }
  }
  C.ng++;
  mix(2);
  mixp(p, 1);
  mixp(g, 1);
}
static void on_iteration(float *p, int nparms)
{
  C.ni++;
  mix(3);
  mix((uint32_t)nparms);
  mixp(p, 0);
}
static void on_step(int itno, float sse, void *parms, float *p)
{
  (void)parms;
  C.ns++;
  mix(4);
  mix((uint32_t)itno);
  mix(bitsf(sse));
  mixp(p, 1);
}
static void on_user(float p[])
{
  C.nu++;
  mix(5);
  mixp(p, 1);
}
// <kind> <n> <f0> <b> <q> <a[1..n]> <c[1..n]>
static void getcost()
{
  C = Cost();
  C.kind = geti();
  C.n = geti();
  if (C.n < 1 || C.n > 32) bad("a cost of 1 to 32 parameters");
  C.f0 = getf();
  C.b = getf();
  C.q = getf();
  for (int i = 1; i <= C.n; i++) C.a[i] = getf();
  for (int i = 1; i <= C.n; i++) C.c[i] = getf();
}
static void putcost()
{
  puti(C.nf);
  puti(C.ng);
  puti(C.ni);
  puti(C.ns);
  puti(C.nu);
  char b[24];
  snprintf(b, sizeof b, "%016" PRIx64, C.hash);
  out(b);
}

// ---- the commands (README.md, "Commands") -----------------------------------------------------

static void call(const std::string &c)
{
  // -- histograms (spec §5)
  if (c == "halloc") puth(HISTOalloc(geti()));
  else if (c == "hrealloc") {
    HISTOGRAM *h = geth();
    int n = geti();
    puth_res(HISTOrealloc(h, n), h);
  }
  else if (c == "hfree") {
    HISTOGRAM *h = geth();
    puti(HISTOfree(&h));
    out(h ? "set" : "none");
  }
  else if (c == "hcopy" || c == "hclear" || c == "hclearcounts" || c == "hpdf" || c == "habs" || c == "hcdf" ||
           c == "hrcdf" || c == "hcumsum") {
    HISTOGRAM *h = geth(), *o = geth_out(h);
    HISTOGRAM *r = c == "hcopy"        ? HISTOcopy(h, o)
                   : c == "hclear"      ? HISTOclear(h, o)
                   : c == "hclearcounts" ? HISTOclearCounts(h, o)
                   : c == "hpdf"        ? HISTOmakePDF(h, o)
                   : c == "habs"        ? HISTOabs(h, o)
                   : c == "hcdf"        ? HISTOmakeCDF(h, o)
                   : c == "hrcdf"       ? HISTOmakeReverseCDF(h, o)
                                        : HISTOcumsum(h, o);
    puth_res(r, o);
    if (o != h) puth(h);
  }
  else if (c == "hinit") {
    HISTOGRAM *o = geth_out(NULL);
    int n = geti();
    double lo = getd(), hi = getd();
    puth_res(HISTOinit(o, n, lo, hi), o);
  }
  else if (c == "hbins") {
    int n = geti();
    double lo = getd(), hi = getd();
    puth(HISTObins(n, lo, hi));
  }
  else if (c == "hadd" || c == "hsub") {
    HISTOGRAM *a = geth(), *b = geth(), *o = geth_out(a, b);
    puth_res(c == "hadd" ? HISTOadd(a, b, o) : HISTOsubtract(a, b, o), o);
    if (o != a) puth(a);
    if (o != b) puth(b);
  }
  else if (c == "hscale") {
    HISTOGRAM *h = geth(), *o = geth_out(h);
    float s = getf(), off = getf();
    puth_res(HISTOlinearScale(h, o, s, off), o);
  }
  else if (c == "hclearbins") {
    HISTOGRAM *h = geth(), *o = geth_out(h);
    int lo = geti(), hi = geti();
    puth_res(HISTOclearBins(h, o, lo, hi), o);
  }
  else if (c == "hsoap") {
    HISTOGRAM *h = geth(), *o = geth_out(h);
    int n = geti();
    puth_res(HISTOsoapBubbleZeros(h, o, n), o);
  }
  else if (c == "hsmooth") {
    HISTOGRAM *h = geth(), *o = geth_out(h);
    float s = getf();
    puth_res(HISTOsmooth(h, o, s), o);
  }
  else if (c == "hsumnorm" || c == "hfill" || c == "hclearzero") {
    HISTOGRAM *h = geth();
    puti(c == "hsumnorm" ? HISTOsumNorm(h) : c == "hfill" ? HISTOfillHoles(h) : HISTOclearZeroBin(h));
    puth(h);
  }
  else if (c == "hlastpeak" || c == "hfirstpeak") {
    HISTOGRAM *h = geth();
    int w = geti();
    float pct = getf();
    puti(c == "hlastpeak" ? HISTOfindLastPeak(h, w, pct) : HISTOfindFirstPeak(h, w, pct));
  }
  else if (c == "hvalley") {
    HISTOGRAM *h = geth();
    int w = geti(), i0 = geti(), i1 = geti();
    puti(HISTOfindValley(h, w, i0, i1));
  }
  else if (c == "hlastpeakin") {
    HISTOGRAM *h = geth();
    int w = geti();
    float pct = getf();
    int i0 = geti(), i1 = geti();
    puti(HISTOfindLastPeakInRegion(h, w, pct, i0, i1));
  }
  else if (c == "hhighest") {
    HISTOGRAM *h = geth();
    int b0 = geti(), b1 = geti();
    puti(HISTOfindHighestPeakInRegion(h, b0, b1));
  }
  else if (c == "hprevvalley" || c == "hnextvalley") {
    HISTOGRAM *h = geth();
    int b0 = geti();
    puti(c == "hprevvalley" ? HISTOfindPreviousValley(h, b0) : HISTOfindNextValley(h, b0));
  }
  else if (c == "hnextpeak" || c == "hprevpeak") {
    HISTOGRAM *h = geth();
    int b0 = geti(), w = geti();
    puti(c == "hnextpeak" ? HISTOfindNextPeak(h, b0, w) : HISTOfindPreviousPeak(h, b0, w));
  }
  else if (c == "hstartpeak" || c == "hendpeak") {
    HISTOGRAM *h = geth();
    int b0 = geti();
    float p = getf();
    puti(c == "hstartpeak" ? HISTOfindStartOfPeak(h, b0, p) : HISTOfindEndOfPeak(h, b0, p));
  }
  else if (c == "hcurrentpeak") {
    HISTOGRAM *h = geth();
    int b0 = geti(), w = geti();
    float p = getf();
    puti(HISTOfindCurrentPeak(h, b0, w, p));
  }
  else if (c == "hfindbin") {
    HISTOGRAM *h = geth();
    float v = getf();
    puti(HISTOfindBin(h, v));
    putd(HISTOgetCount(h, v));
    puti(HISTOfindBinWithCount(h, v));
  }
  else if (c == "hvaltobin") {
    HISTOGRAM *h = geth();
    puti(HISTOvalToBin(h, getd()));
  }
  else if (c == "hcount") {
    HISTOGRAM *h = geth();
    int n = geti();
    std::vector<double> v((size_t)(n > 0 ? n : 0));
    for (int i = 0; i < n; i++) v[(size_t)i] = getd();
    puti(HISTOcount(h, v.data(), n));
    puth(h);
  }
  else if (c == "haddsample") {
    HISTOGRAM *h = geth();
    int n = geti();
    std::vector<float> v((size_t)(n > 0 ? n : 0));
    for (int i = 0; i < n; i++) v[(size_t)i] = getf();
    float lo = getf(), hi = getf();
    for (int i = 0; i < n; i++) puti(HISTOaddSample(h, v[(size_t)i], lo, hi));
    puth(h);
  }
  else if (c == "htotal") {
    HISTOGRAM *h = geth();
    int b0 = geti(), b1 = geti();
    putf(HISTOtotalInRegion(h, b0, b1));
    putf(HISTOtotal(h));
  }
  else if (c == "hthresh") {
    HISTOGRAM *m = geth(), *h = geth();
    putf(HISTOthreshSum(m, h, getf()));
  }
  else if (c == "hmedian") putd(HISTOfindMedian(geth()));
  else if (c == "hentropy") putd(HISTOgetEntropy(geth()));
  else if (c == "hcorrelate") {
    HISTOGRAM *a = geth(), *b = geth();
    putd(HISTOcorrelate(a, b));
  }
  else if (c == "hlinfit") {
    HISTOGRAM *a = geth(), *b = geth();
    double a0 = getd(), a1 = getd(), b0 = getd(), b1 = getd();
    float pa = -7.0f, pb = -7.0f;
    putd(HISTOfindLinearFit(a, b, a0, a1, b0, b1, &pa, &pb));
    putf(pa);
    putf(pb);
  }
  else if (c == "hfwhm") {
    HISTOGRAM *h = geth();
    putf(HISTOcomputeFWHM(h, geti()));
  }
  else if (c == "hrobust") {
    HISTOGRAM *h = geth();
    double q = getd(), off = -7.0, sig = -7.0;
    puti(HISTOrobustGaussianFit(h, q, &off, &sig));
    putd(off);
    putd(sig);
    puth(h);
    // the plots it writes with the diagnostic bits write and verbose (+gdiag=10000008)
    for (const char *f : {"ha.plt", "hc.plt", "h.plt", "hs.plt", "hz.plt", "hp.plt"}) {
      putfile(f);
      remove(f);
    }
  }
  else if (c == "hplot" || c == "hwrite") {
    HISTOGRAM *h = geth();
    std::string name = getx();
    remove(name.c_str());
    puti(c == "hplot" ? HISTOplot(h, name.c_str()) : HISTOwriteTxt(h, name.c_str()));
    putfile(name.c_str());
    remove(name.c_str());
  }
  else if (c == "h2d") {
    // <nbins1> <nbins2> <bin_size1> <bin_size2> <min1> <min2> <bins1> <bins2> <counts row by row> <v1> <v2>
    int n1 = geti(), n2 = geti();
    HISTOGRAM2D *h = HISTO2Dalloc(n1, n2);
    h->bin_size1 = getf();
    h->bin_size2 = getf();
    h->min1 = getf();
    h->min2 = getf();
    for (int i = 0; i < n1; i++) h->bins1[i] = getf();
    for (int i = 0; i < n2; i++) h->bins2[i] = getf();
    for (int i = 0; i < n1; i++)
      for (int j = 0; j < n2; j++) h->counts[i][j] = getf();
    float v1 = getf(), v2 = getf();
    puti(HISTO2DfindBin1(h, v1));
    puti(HISTO2DfindBin2(h, v2));
    putd(HISTO2DgetCount(h, v1, v2));
  }
  // -- float matrices: allocation, copy, the double-accumulated product (spec §2)
  else if (c == "malloc") {
    int rows = geti(), cols = geti(), type = geti();
    putm(MatrixAlloc(rows, cols, type));
  }
  else if (c == "malloc2") {
    int rows = geti(), cols = geti(), type = geti();
    MatrixBuffer buf;
    memset(&buf, 0, sizeof buf);
    MATRIX *m = MatrixAlloc2(rows, cols, type, &buf);
    putm(m);
    if (m) {
      puti(m->inBuf);
      puti(MatrixFree(&m));
      out(m ? "set" : "none");
    }
  }
  else if (c == "mfree") {
    MATRIX *m = getm();
    puti(MatrixFree(&m));
    out(m ? "set" : "none");
  }
  else if (c == "mcopy") {
    MATRIX *a = getm(), *o = getm_out(a);
    putm_res(MatrixCopy(a, o), o);
  }
  else if (c == "mmuld" || c == "mmul") {
    MATRIX *a = getm(), *b = getm(), *o = getm_out(a, b);
    putm_res(c == "mmuld" ? MatrixMultiplyD(a, b, o) : MatrixMultiply(a, b, o), o);
    if (o != a) putm(a);
    if (o != b) putm(b);
  }
  // -- the 4 x 4 affine helpers (spec §3)
  else if (c == "affine") {
    MATRIX *a = getm(), *b = getm();
    AffineMatrix A, B, M;
    SetAffineMatrix(&A, a);
    SetAffineMatrix(&B, b);
    float x = getf(), y = getf(), z = getf();
    AffineVector v, w;
    SetAffineVector(&v, x, y, z);
    for (int i = 0; i < 4; i++) putf(v.vec[i]);
    v.vec[3] = getf();
    AffineMV(&w, &A, &v);
    for (int i = 0; i < 4; i++) putf(w.vec[i]);
    float gx, gy, gz;
    GetAffineVector(&w, &gx, &gy, &gz);
    putf(gx);
    putf(gy);
    putf(gz);
    int ix, iy, iz;
    GetFloorAffineVector(&w, &ix, &iy, &iz);
    puti(ix);
    puti(iy);
    puti(iz);
    AffineMM(&M, &A, &B);
    for (int i = 0; i < 16; i++) putf(M.mat[i]);
    MATRIX *g = MatrixAlloc(4, 4, MATRIX_REAL);
    GetAffineMatrix(g, &M);
    putm(g);
  }
  else if (c == "affineset") {
    MATRIX *a = getm();
    AffineMatrix A;
    SetAffineMatrix(&A, a);
    for (int i = 0; i < 16; i++) putf(A.mat[i]);
  }
  else if (c == "affineget") {
    MATRIX *a = getm();
    AffineMatrix A;
    for (int i = 0; i < 16; i++) A.mat[i] = getf();
    GetAffineMatrix(a, &A);
    putm(a);
  }
  // -- double matrices (spec §4, §12)
  else if (c == "dalloc") {
    int rows = geti(), cols = geti(), type = geti();
    putdm(DMatrixAlloc(rows, cols, type));
  }
  else if (c == "dcheck") {
    DMATRIX *a = getdm(), *b = getdm();
    int kind = geti();
    FILE *fp = getstream();
    std::string pre = getx();
    puti(DMatrixCheckDims(a, b, kind, fp, pre.c_str()));
  }
  else if (c == "daddmul") {
    DMATRIX *a = getdm(), *b = getdm(), *o = getdm_out(a, b);
    double v1 = getd(), v2 = getd();
    putdm_res(DMatrixAddMul(a, b, v1, v2, o), o);
  }
  else if (c == "dadd" || c == "dsub") {
    DMATRIX *a = getdm(), *b = getdm(), *o = getdm_out(a, b);
    putdm_res(c == "dadd" ? DMatrixAdd(a, b, o) : DMatrixSubtract(a, b, o), o);
  }
  else if (c == "dscale") {
    DMATRIX *a = getdm(), *o = getdm_out(a);
    double v = getd();
    putdm_res(DMatrixScalarMul(a, v, o), o);
  }
  else if (c == "dcopy" || c == "dtranspose") {
    DMATRIX *a = getdm(), *o = getdm_out(a);
    putdm_res(c == "dcopy" ? DMatrixCopy(a, o) : DMatrixTranspose(a, o), o);
  }
  else if (c == "dprint") {
    std::string fmt = getx();
    DMATRIX *a = getdm();
    puti(fmt.empty() ? DMatrixPrint(stdout, a) : DMatrixPrintFmt(stdout, fmt.c_str(), a));
  }
  else if (c == "dmul") {
    DMATRIX *a = getdm(), *b = getdm(), *o = getdm_out(a, b);
    putdm_res(DMatrixMultiply(a, b, o), o);
  }
  else if (c == "ddot") {
    DMATRIX *a = getdm(), *b = getdm();
    putd(DVectorDot(a, b));
  }
  else if (c == "dcmpf") {
    MATRIX *f = getm();
    DMATRIX *d = getdm();
    putd(DMatrixCompareFMatrix(f, d));
  }
  else if (c == "dcopyf") {
    MATRIX *f = getm();
    DMATRIX *o = getdm_out(NULL);
    putdm_res(DMatrixCopyFMatrix(f, o), o);
  }
  else if (c == "dconst") {
    double v = getd();
    int rows = geti(), cols = geti();
    DMATRIX *o = getdm_out(NULL);
    putdm_res(DMatrixConstVal(v, rows, cols, o), o);
  }
  else if (c == "dzero") {
    int rows = geti(), cols = geti();
    DMATRIX *o = getdm_out(NULL);
    putdm_res(DMatrixZero(rows, cols, o), o);
  }
  else if (c == "dmaxabs") putd(DMatrixMaxAbs(getdm()));
  else if (c == "dliststats") {
    int n = geti();
    std::vector<double> v((size_t)(n > 0 ? n : 0));
    for (int i = 0; i < n; i++) v[(size_t)i] = getd();
    double *st = DListStats(v.data(), n, NULL);
    for (int i = 0; i < 5; i++) putd(st[i]);
  }
  // -- float matrices, element-wise operations (spec §8.2)
  else if (c == "midentity") {
    int n = geti();
    MATRIX *o = getm_out(NULL);
    putm_res(MatrixIdentity(n, o), o);
  }
  else if (c == "mtranspose" || c == "msquare" || c == "mdiag" || c == "mclone") {
    MATRIX *a = getm();
    if (c == "mclone") {
      putm(MatrixClone(a));
      return;
    }
    MATRIX *o = getm_out(a);
    putm_res(c == "mtranspose" ? MatrixTranspose(a, o) : c == "msquare" ? MatrixSquareElts(a, o) : MatrixDiag(a, o), o);
  }
  else if (c == "madd" || c == "msub") {
    MATRIX *a = getm(), *b = getm(), *o = getm_out(a, b);
    putm_res(c == "madd" ? MatrixAdd(a, b, o) : MatrixSubtract(a, b, o), o);
  }
  else if (c == "mscale") {
    MATRIX *a = getm(), *o = getm_out(a);
    float v = getf();
    putm_res(MatrixScalarMul(a, v, o), o);
  }
  else if (c == "mclear") {
    MATRIX *a = getm();
    putm_res(MatrixClear(a), a);
  }
  else if (c == "msymmetric") puti(MatrixIsSymmetric(getm()));
  else if (c == "miszero") puti(MatrixIsZero(getm()));
  else if (c == "mcheck") puti(MatrixCheck(getm()));
  else if (c == "mtrace") putf(MatrixTrace(getm()));
  else if (c == "msum") putd(MatrixSumElts(getm()));
  else if (c == "mconst" || c == "mzero") {
    float v = c == "mconst" ? getf() : 0.0f;
    int rows = geti(), cols = geti();
    MATRIX *o = getm_out(NULL);
    putm_res(c == "mconst" ? MatrixConstVal(v, rows, cols, o) : MatrixZero(rows, cols, o), o);
  }
  else if (c == "mreshape") {
    MATRIX *a = getm(), *o = getm_out(a);
    int rows = geti(), cols = geti();
    putm_res(MatrixReshape(a, o, rows, cols), o);
  }
  else if (c == "mcolscale") {
    MATRIX *a = getm(), *s = getm_out(a);
    putm_res(MatrixNormalizeColScale(a, s), s);
  }
  else if (c == "mnormcol") {
    MATRIX *a = getm(), *o = getm_out(a), *s = getm_out(a);
    putm_res(MatrixNormalizeCol(a, o, s), o);
    if (s) putm(s);
  }
  // -- regions, real to complex (spec §8.3)
  else if (c == "mregion" || c == "mrealregion" || c == "mimagregion") {
    MATRIX *s = getm(), *o = getm_out(s);
    int sr = geti(), sc = geti(), rows = geti(), cols = geti(), dr = geti(), dc = geti();
    MATRIX *r = c == "mregion"       ? MatrixCopyRegion(s, o, sr, sc, rows, cols, dr, dc)
                : c == "mrealregion" ? MatrixCopyRealRegion(s, o, sr, sc, rows, cols, dr, dc)
                                     : MatrixCopyImagRegion(s, o, sr, sc, rows, cols, dr, dc);
    putm_res(r, o);
  }
  else if (c == "mr2c") {
    MATRIX *re = getm(), *im = getm(), *o = getm_out(re, im);
    putm_res(MatrixRealToComplex(re, im, o), o);
  }
  // -- rotations, vectors, printing, Gaussian matrices (spec §8.4 to §8.7)
  else if (c == "mrotation") {
    int n = geti();
    float angle = getf();
    int which = geti();
    MATRIX *o = getm_out(NULL);
    putm_res(o ? MatrixReallocRotation(n, angle, which, o) : MatrixAllocRotation(n, angle, which), o);
  }
  else if (c == "vlen") putf(VectorLen(getm()));
  else if (c == "vdot" || c == "vdist") {
    MATRIX *a = getm(), *b = getm();
    putf(c == "vdot" ? VectorDot(a, b) : VectorDistance(a, b));
  }
  else if (c == "vcross") {
    MATRIX *a = getm(), *b = getm(), *o = getm_out(a, b);
    putm_res(VectorCrossProduct(a, b, o), o);
  }
  else if (c == "vtriple") {
    MATRIX *a = getm(), *b = getm(), *d = getm();
    putf(VectorTripleProduct(a, b, d));
  }
  else if (c == "v3angle") {
    MATRIX *a = getm(), *b = getm();
    putd(Vector3Angle(a, b));
  }
  else if (c == "xyznorm") {
    float x = getf(), y = getf(), z = getf();
    XYZ v;
    float len = -7.0f;
    XYZ_NORMALIZED_LOAD(&v, &len, x, y, z);
    putf(v.x);
    putf(v.y);
    putf(v.z);
    putf(len);
  }
  else if (c == "xyzangle") {
    XYZ n;
    n.x = getf();
    n.y = getf();
    n.z = getf();
    float x = getf(), y = getf(), z = getf(), norm = getf();
    putf(XYZApproxAngle_knownLength(&n, x, y, z, norm));
  }
  else if (c == "mprint") {
    FILE *fp = getstream();
    MATRIX *a = getm();
    int p = geti();
    puti(MatrixPrint(fp, a, p));
  }
  else if (c == "mprintfmt") {
    std::string fmt = getx();
    MATRIX *a = getm();
    puti(MatrixPrintFmt(stdout, fmt.c_str(), a));
  }
  else if (c == "msimilarity") {
    MATRIX *s = getm(), *m = getm(), *o = getm_out(s, m);
    putm_res(MatrixSimilarityTransform(s, m, o), o);
  }
  else if (c == "mgaussian") {
    int n = geti();
    float sd = getf();
    int norm = geti();
    MATRIX *o = getm_out(NULL);
    putm_res(GaussianMatrix(n, sd, norm, o), o);
  }
  else if (c == "mgaussian2") {
    int n = geti();
    float s1 = getf(), s2 = getf(), w1 = getf();
    int norm = geti();
    MATRIX *o = getm_out(NULL);
    putm_res(GaussianMatrix2(n, s1, s2, w1, norm, o), o);
  }
  // -- inverse, determinant, QR (spec §8.8)
  else if (c == "minverse") {
    MATRIX *a = getm(), *o = getm_out(a);
    putm_res(MatrixInverse(a, o), o);
  }
  else if (c == "mdet") putf(MatrixDeterminant(getm()));
  else if (c == "mqr") {
    MATRIX *a = getm(), *q = getm(), *r = getm();
    puti(MatrixQRdecomposition(a, q, r));
    putm(q);
    putm(r);
  }
  // -- primes and random draws (spec §9, §10)
  else if (c == "primes") {
    int nmax = geti(), n = -7;
    int *p = GetPrimes(nmax, &n);
    if (!p) {
      out("null");
      return;
    }
    puti(n);
    for (int i = 0; i < n; i++) puti(p[i]);
  }
  else if (c == "pfactors") {
    int N = geti(), n = -7;
    int *p = GetPrimeFactors(N, &n);
    if (!p) {
      out("null");
      return;
    }
    puti(n);
    for (int i = 0; i < n; i++) puti(p[i]);
  }
  else if (c == "pclosest" || c == "pclosestless") {
    int N = geti(), p = geti();
    puti(c == "pclosest" ? GetClosestPrimeFactor(N, p) : GetClosestPrimeFactorLess(N, p));
  }
  else if (c == "gaussdraws") {
    long seed = getl();
    int k = geti();
    srand48(seed);
    for (int i = 0; i < k; i++) putd(PDFgaussian());
  }
  else if (c == "todseed") puti((long)PDFtodSeed());
  // -- the minimizers (spec §13)
  else if (c == "powell" || c == "powell2") {
    getcost();
    int n = C.n;
    float *p = vector(1, n);
    for (int i = 1; i <= n; i++) p[i] = getf();
    bool dirs = geti() != 0;
    float **xi = dirs ? matrix(1, n, 1, n) : NULL;
    if (dirs)
      for (int r = 1; r <= n; r++)
        for (int k = 1; k <= n; k++) xi[r][k] = getf();
    int iter = -7;
    float fret = -7.0f;
    if (c == "powell") {
      float tol = getf();
      powell_iteration_func = geti() ? on_iteration : NULL;
      OpenPowell(p, xi, n, tol, &iter, &fret, cost_value);
    }
    else {
      float ftol = getf(), lin = getf();
      int maxit = geti(), gno = geti();
      powell_iteration_func = geti() ? on_iteration : NULL;
      Gdiag_no = gno;
      puti(OpenPowell2(p, xi, n, ftol, lin, maxit, &iter, &fret, cost_value));
    }
    powell_iteration_func = NULL;
    puti(iter);
    putf(fret);
    for (int i = 1; i <= n; i++) putf(p[i]);
    if (dirs)
      for (int r = 1; r <= n; r++)
        for (int k = 1; k <= n; k++) putf(xi[r][k]);
    putcost();
  }
  else if (c == "dfpmin") {
    getcost();
    int n = C.n;
    float *p = vector(1, n);
    for (int i = 1; i <= n; i++) p[i] = getf();
    float tol = getf(), fret = getf();
    int verbose = geti(), stepf = geti(), userf = geti();
    if (verbose) Gdiag |= DIAG_VERBOSE;
    int iter = -7;
    puti(OpenDFPMin(p, n, tol, &iter, &fret, cost_value, cost_gradient, stepf ? on_step : NULL, NULL,
                    userf ? on_user : NULL));
    puti(iter);
    putf(fret);
    for (int i = 1; i <= n; i++) putf(p[i]);
    putcost();
  }
  // -- Numerical Recipes' vectors and matrices (spec §14)
  else if (c == "nrvector") {
    long nl = getl(), nh = getl();
    float *v = vector(nl, nh);
    for (long i = nl; i <= nh; i++) v[i] = (float)i;
    for (long i = nl; i <= nh; i++) putf(v[i]);
    free_vector(v, nl, nh);
  }
  else if (c == "nrmatrix") {
    long r0 = getl(), r1 = getl(), c0 = getl(), c1 = getl();
    float **m = matrix(r0, r1, c0, c1);
    for (long r = r0; r <= r1; r++)
      for (long k = c0; k <= c1; k++) m[r][k] = (float)(r * 100 + k);
    for (long r = r0; r <= r1; r++)
      for (long k = c0; k <= c1; k++) putf(m[r][k]);
    free_matrix(m, r0, r1, c0, c1);
  }
  // -- decompositions (spec §17)
  else if (c == "oeigen" || c == "oeigenns") {
    int n = geti();
    std::vector<float> a((size_t)(n * n > 0 ? n * n : 1)), ev((size_t)(n > 0 ? n : 1), 7.0f),
        vec((size_t)(n * n > 0 ? n * n : 1), 7.0f);
    for (int i = 0; i < n * n; i++) a[(size_t)i] = getf();
    puti(c == "oeigen" ? OpenEigenSystem(a.data(), n, ev.data(), vec.data())
                       : OpenNonSymmetricEigenSystem(a.data(), n, ev.data(), vec.data()));
    for (int i = 0; i < n; i++) putf(ev[(size_t)i]);
    for (int i = 0; i < n * n; i++) putf(vec[(size_t)i]);
  }
  else if (c == "meigen" || c == "meigenhelper") {
    MATRIX *a = getm();
    int sym = c == "meigenhelper" ? geti() : -1;
    MATRIX *o = getm_out(a);
    int n = a && a->rows > 0 ? a->rows : 1;
    std::vector<float> ev((size_t)n, 7.0f);
    MATRIX *r = sym < 0 ? MatrixEigenSystem(a, ev.data(), o) : MatrixCalculateEigenSystemHelper(a, ev.data(), o, sym);
    putm_res(r, o);
    for (int i = 0; i < n; i++) putf(ev[(size_t)i]);
  }
  else if (c == "msvd") {
    MATRIX *a = getm(), *w = getm(), *v = getm_out(a);
    putm_res(MatrixSVD(a, w, v), v);
    putm(a);
    putm(w);
  }
  else if (c == "msvdvalues") {
    MATRIX *a = getm();
    int n = geti();
    std::vector<float> ev((size_t)(n > 0 ? n : 1), 7.0f);
    putf(MatrixSVDEigenValues(a, ev.data()));
    for (int i = 0; i < n; i++) putf(ev[(size_t)i]);
  }
  else if (c == "mcond" || c == "mnscond") {
    MATRIX *a = getm();
    putf(c == "mcond" ? MatrixConditionNumber(a) : MatrixNSConditionNumber(a));
  }
  else if (c == "msvdinverse" || c == "mpinverse" || c == "msvdpinverse") {
    MATRIX *a = getm(), *o = getm_out(a);
    MATRIX *r = c == "msvdinverse" ? MatrixSVDInverse(a, o)
                : c == "mpinverse" ? MatrixPseudoInverse(a, o)
                                   : MatrixSVDPseudoInverse(a, o);
    putm_res(r, o);
  }
  // -- matrix files (spec §18): a file named by the case, in the working directory
  else if (c == "mreadtxt") {
    std::string name = getx();
    std::string bytes = getx();
    writefile(name.c_str(), bytes);
    errno = 0;
    putm(MatrixReadTxt(name.c_str(), NULL));
    puti(errno);
    remove(name.c_str());
  }
  else if (c == "mreadtxtmissing") {
    std::string name = getx();
    remove(name.c_str());
    errno = 0;
    putm(MatrixReadTxt(name.c_str(), NULL));
    puti(errno);
  }
  else if (c == "mreadasc" || c == "mreadbin") {
    std::string bytes = getx();
    MATRIX *o = getm_out(NULL);
    std::string name = "fs-maths-probe.dat";
    writefile(name.c_str(), bytes);
    FILE *fp = fopen(name.c_str(), "rb");
    errno = 0;
    MATRIX *r = c == "mreadasc" ? MatrixAsciiReadFrom(fp, o) : MatrixReadFrom(fp, o);
    putm_res(r, o);
    puti(errno);
    puti(ftell(fp));
    fclose(fp);
    remove(name.c_str());
  }
  else if (c == "mwriteasc" || c == "mwritebin") {
    MATRIX *a = getm();
    std::string name = "fs-maths-probe.dat";
    FILE *fp = fopen(name.c_str(), "wb");
    puti(c == "mwriteasc" ? MatrixAsciiWriteInto(fp, a) : MatrixWriteInto(fp, a));
    fclose(fp);
    putfile(name.c_str());
    remove(name.c_str());
  }
  else if (c == "mwritetxt") {
    std::string name = getx();
    MATRIX *a = getm();
    remove(name.c_str());
    puti(MatrixWriteTxt(name.c_str(), a));
    putfile(name.c_str());
    remove(name.c_str());
  }
  // -- random-field specifications (spec §19)
  else if (c == "rfname") {
    std::string name = getx();
    RFS r;
    memset(&r, 0, sizeof r);
    r.name = (char *)name.c_str();
    puti(RFname2Code(&r));
    puti(r.code);
  }
  else if (c == "rfinit") {
    unsigned long seed = strtoul(tok().c_str(), NULL, 10);
    RFS *r = RFspecInit(seed, NULL);
    out(r->name ? hexbytes(r->name) : "null");
    puti(r->code);
    puti(r->nparams);
    putd(r->mean);
    putd(r->stddev);
    puti((long)r->seed);
    unsigned long s2 = strtoul(tok().c_str(), NULL, 10);
    puti(RFspecSetSeed(r, s2));
    puti((long)r->seed);
  }
  else
    bad("unknown command");
}

// ---- running a line in a child process -----------------------------------------------------------

// VNL's messages name their source file: the probe writes the official build's ITK folder there
static std::string official_itk(const std::string &s)
{
  static const std::string mark = "/Modules/ThirdParty/VNL/";
  static const std::string root = "/home/rd521/4.13.2_ubuntu20/itk/4.13.2/src/ITK";
  std::string r;
  size_t i = 0;
  while (i < s.size()) {
    size_t e = s.find('\n', i);
    size_t end = e == std::string::npos ? s.size() : e + 1;
    std::string line = s.substr(i, end - i);
    size_t k = line.find(mark);
    if (k != std::string::npos && line[0] == '/') line = root + line.substr(k);
    r += line;
    i = end;
  }
  return r;
}

// With MALLOC_PERTURB_ set (the comparison loop's guard runs), the stack below the call is filled
// with its byte first: a result that reads stack memory nothing wrote then changes with the byte.
__attribute__((noinline)) static void dirty_stack()
{
  const char *s = getenv("MALLOC_PERTURB_");
  if (!s || !*s) return;
  unsigned char f = (unsigned char)(strtol(s, NULL, 0) & 0xff);
  volatile unsigned char a[1 << 18];
  for (size_t i = 0; i < sizeof a; i++) a[i] = f;
}

static std::string run_line(const Toks &line)
{
  unsigned seconds = 2;
  size_t first = 0;
  unsigned long gdiag = 0;
  for (; first < line.size(); first++) {
    if (line[first][0] == '@')
      seconds = (unsigned)strtoul(line[first].c_str() + 1, NULL, 10);
    else if (line[first].compare(0, 7, "+gdiag=") == 0)
      gdiag = strtoul(line[first].c_str() + 7, NULL, 16);
    else
      break;
  }
  const char *fo = ".fs-maths-probe.out", *fe = ".fs-maths-probe.err";
  int fd[2];
  if (pipe(fd) != 0) bad("pipe");
  fflush(stdout);
  fflush(stderr);
  pid_t pid = fork();
  if (pid < 0) bad("fork");
  if (pid == 0) {
    if (guard_malloc_enable) guard_malloc_enable();
    close(fd[0]);
    int o = open(fo, O_WRONLY | O_CREAT | O_TRUNC, 0600), e = open(fe, O_WRONLY | O_CREAT | O_TRUNC, 0600);
    dup2(o, 1);
    dup2(e, 2);
    close(o);
    close(e);
    alarm(seconds);
    T.assign(line.begin() + (long)first, line.end());
    P = 0;
    R.clear();
    Gerror = NO_ERROR;
    Gdiag = gdiag;
    errno = 0;
    dirty_stack();
    call(tok());
    fflush(stdout);
    fflush(stderr);
    size_t off = 0;
    while (off < R.size()) {
      ssize_t w = write(fd[1], R.data() + off, R.size() - off);
      if (w <= 0) break;
      off += (size_t)w;
    }
    close(fd[1]);
    _exit(0);
  }
  close(fd[1]);
  std::string got;
  char buf[65536];
  ssize_t k;
  while ((k = read(fd[0], buf, sizeof buf)) > 0) got.append(buf, (size_t)k);
  close(fd[0]);
  int status = 0;
  waitpid(pid, &status, 0);
  std::string res;
  if (WIFSIGNALED(status))
    res = WTERMSIG(status) == SIGALRM ? "TIMEOUT" : "CRASH " + std::to_string(WTERMSIG(status));
  else if (WEXITSTATUS(status) != 0)
    res = "EXIT " + std::to_string(WEXITSTATUS(status));
  else {
    res = got;
    if (!res.empty() && res.back() == ' ') res.pop_back();
  }
  std::string so = official_itk(slurp(fo)), se = official_itk(slurp(fe));
  remove(fo);
  remove(fe);
  return res + " | " + hexbytes(so) + " " + hexbytes(se);
}

int main(int argc, char **argv)
{
  if (argc != 3 || strcmp(argv[1], "batch") != 0) {
    fprintf(stderr, "usage: fs_maths_probe batch <file> (README.md)\n");
    return 2;
  }
  // every line read before the first child runs: a child ended by exit() flushes the stdio streams
  // it inherited, which would move the shared file offset of an open input stream
  FILE *fp = strcmp(argv[2], "-") == 0 ? stdin : fopen(argv[2], "r");
  if (!fp) {
    fprintf(stderr, "probe: cannot open %s\n", argv[2]);
    return 2;
  }
  std::vector<Toks> lines;
  std::string cur;
  int ch;
  while ((ch = fgetc(fp)) != EOF) {
    if (ch == '\n') {
      Toks t;
      size_t i = 0;
      while (i < cur.size()) {
        while (i < cur.size() && (cur[i] == ' ' || cur[i] == '\t')) i++;
        size_t j = i;
        while (j < cur.size() && cur[j] != ' ' && cur[j] != '\t') j++;
        if (j > i) t.push_back(cur.substr(i, j - i));
        i = j;
      }
      lines.push_back(t);
      cur.clear();
    }
    else
      cur.push_back((char)ch);
  }
  if (fp != stdin) fclose(fp);
  for (const Toks &t : lines) {
    if (t.empty()) {
      printf("\n");
      continue;
    }
    printf("%s\n", run_line(t).c_str());
    fflush(stdout);
  }
  return 0;
}
