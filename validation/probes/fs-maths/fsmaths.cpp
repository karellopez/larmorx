// SPDX-License-Identifier: Apache-2.0
// CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: it calls FreeSurfer 8.2.0's library functions by
// name, from its headers.
//
// fsmaths: runs FreeSurfer 8.2.0's fs.maths functions (histo.cpp, matrix.cpp, dmatrix.cpp,
// affine.h) of the unit oracle's build (validation/investigations/freesurfer-unit-oracle: the
// official build's utils library, GCC 8.4.0, the official flags) on the cases of a case file,
// one output line per case, every number as the hexadecimal bits of its float or double. The
// cases and the port's own lines come from larmorx-freesurfer-core's example fs_maths_probe;
// check.py compares the two. Built by build.sh against the unit oracle's build tree.
//
// Written for LARMORX (Apache-2.0); the binary it builds links FreeSurfer's code and stays on
// the machine that built it.

#include <cinttypes>
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
#include "pdf.h"
#include "prime.h"
#include "utils.h"

// declared in utils/matrix.cpp only (external linkage)
int MatrixIsSymmetric(MATRIX *matrix);

const char *Progname = "fsmaths";

namespace {

std::vector<std::string> toks;
size_t pos;
std::string line_out;

[[noreturn]] void fail(const char *what)
{
  fprintf(stderr, "fsmaths: %s\n", what);
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

void puti(int v)
{
  char b[32];
  snprintf(b, sizeof b, "%d ", v);
  line_out += b;
}

void putf(float f)
{
  uint32_t u;
  memcpy(&u, &f, 4);
  char b[32];
  snprintf(b, sizeof b, "%08" PRIx32 " ", u);
  line_out += b;
}

void putd(double d)
{
  uint64_t u;
  memcpy(&u, &d, 8);
  char b[32];
  snprintf(b, sizeof b, "%016" PRIx64 " ", u);
  line_out += b;
}

HISTOGRAM *geth()
{
  int nbins = geti();
  HISTOGRAM *h = HISTOalloc(nbins);
  h->bin_size = getf();
  h->min = getf();
  h->max = getf();
  for (int b = 0; b < nbins; b++) h->bins[b] = getf();
  for (int b = 0; b < nbins; b++) h->counts[b] = getf();
  return h;
}

void puth(const HISTOGRAM *h)
{
  if (!h) {
    line_out += "NULL ";
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

MATRIX *getm(int type)
{
  int rows = geti(), cols = geti();
  MATRIX *m = MatrixAlloc(rows, cols, type);
  int n = rows * cols * (type == MATRIX_COMPLEX ? 2 : 1);
  for (int i = 0; i < n; i++) m->data[i] = getf();
  return m;
}

// the whole array: the two floats before data, the numbers
void putm_raw(const MATRIX *m)
{
  int n = m->rows * m->cols * (m->type == MATRIX_COMPLEX ? 2 : 1);
  puti(m->rows);
  puti(m->cols);
  for (int i = -2; i < n; i++) putf(m->data[i]);
}

// a matrix a function returned, or NULL
void putm_n(const MATRIX *m)
{
  if (!m) {
    line_out += "NULL ";
    return;
  }
  putm_raw(m);
}

// a matrix of a type given first
MATRIX *getmt()
{
  int t = geti();
  return getm(t);
}

DMATRIX *getdm()
{
  int rows = geti(), cols = geti();
  DMATRIX *m = DMatrixAlloc(rows, cols, MATRIX_REAL);
  for (int i = 0; i < rows * cols; i++) m->data[i] = getd();
  return m;
}

void putdm(const DMATRIX *m)
{
  if (!m) {
    line_out += "NULL ";
    return;
  }
  puti(m->rows);
  puti(m->cols);
  for (int i = 0; i < m->rows * m->cols; i++) putd(m->data[i]);
}

// a file a function wrote, as the hexadecimal of its bytes
void putfile(const char *path)
{
  FILE *fp = fopen(path, "rb");
  if (!fp) {
    line_out += "NOFILE ";
    return;
  }
  int c;
  char b[4];
  while ((c = fgetc(fp)) != EOF) {
    snprintf(b, sizeof b, "%02x", c);
    line_out += b;
  }
  fclose(fp);
  line_out += " ";
}

void run_case(const char *tmpfile)
{
  std::string cmd = next();
  if (cmd == "smooth") {
    HISTOGRAM *h = geth();
    float s = getf();
    puth(HISTOsmooth(h, NULL, s));
  }
  else if (cmd == "smoothip") {
    HISTOGRAM *h = geth();
    float s = getf();
    HISTOsmooth(h, h, s);
    puth(h);
  }
  else if (cmd == "smoothinto") {
    HISTOGRAM *h = geth(), *d = geth();
    float s = getf();
    HISTOsmooth(h, d, s);
    puth(d);
  }
  else if (cmd == "lastpeak") {
    HISTOGRAM *h = geth();
    int w = geti();
    puti(HISTOfindLastPeak(h, w, getf()));
  }
  else if (cmd == "firstpeak") {
    HISTOGRAM *h = geth();
    int w = geti();
    puti(HISTOfindFirstPeak(h, w, getf()));
  }
  else if (cmd == "valley") {
    HISTOGRAM *h = geth();
    int w = geti(), i0 = geti(), i1 = geti();
    puti(HISTOfindValley(h, w, i0, i1));
  }
  else if (cmd == "lastpeakreg") {
    HISTOGRAM *h = geth();
    int w = geti();
    float p = getf();
    int i0 = geti(), i1 = geti();
    puti(HISTOfindLastPeakInRegion(h, w, p, i0, i1));
  }
  else if (cmd == "highest") {
    HISTOGRAM *h = geth();
    int b0 = geti(), b1 = geti();
    puti(HISTOfindHighestPeakInRegion(h, b0, b1));
  }
  else if (cmd == "addsample") {
    HISTOGRAM *h = geth();
    int n = geti();
    std::vector<float> v(n);
    for (int i = 0; i < n; i++) v[i] = getf();
    float bmin = getf(), bmax = getf();
    for (int i = 0; i < n; i++) puti(HISTOaddSample(h, v[i], bmin, bmax));
    puth(h);
  }
  else if (cmd == "prevvalley") {
    HISTOGRAM *h = geth();
    puti(HISTOfindPreviousValley(h, geti()));
  }
  else if (cmd == "nextvalley") {
    HISTOGRAM *h = geth();
    puti(HISTOfindNextValley(h, geti()));
  }
  else if (cmd == "nextpeak") {
    HISTOGRAM *h = geth();
    int b0 = geti(), w = geti();
    puti(HISTOfindNextPeak(h, b0, w));
  }
  else if (cmd == "prevpeak") {
    HISTOGRAM *h = geth();
    int b0 = geti(), w = geti();
    puti(HISTOfindPreviousPeak(h, b0, w));
  }
  else if (cmd == "startpeak") {
    HISTOGRAM *h = geth();
    int b0 = geti();
    puti(HISTOfindStartOfPeak(h, b0, getf()));
  }
  else if (cmd == "endpeak") {
    HISTOGRAM *h = geth();
    int b0 = geti();
    puti(HISTOfindEndOfPeak(h, b0, getf()));
  }
  else if (cmd == "currentpeak") {
    HISTOGRAM *h = geth();
    int b0 = geti(), w = geti();
    puti(HISTOfindCurrentPeak(h, b0, w, getf()));
  }
  else if (cmd == "fillholes") {
    HISTOGRAM *h = geth();
    HISTOfillHoles(h);
    puth(h);
  }
  else if (cmd == "totalreg") {
    HISTOGRAM *h = geth();
    int b0 = geti(), b1 = geti();
    putf(HISTOtotalInRegion(h, b0, b1));
    putf(HISTOtotal(h));
  }
  else if (cmd == "clearzero") {
    HISTOGRAM *h = geth();
    HISTOclearZeroBin(h);
    puth(h);
  }
  else if (cmd == "findbin") {
    HISTOGRAM *h = geth();
    float v = getf();
    puti(HISTOfindBin(h, v));
    putd(HISTOgetCount(h, v));
    puti(HISTOfindBinWithCount(h, v));
  }
  else if (cmd == "correlate") {
    HISTOGRAM *a = geth(), *b = geth();
    putd(HISTOcorrelate(a, b));
  }
  else if (cmd == "linfit") {
    HISTOGRAM *a = geth(), *b = geth();
    double amin = getd(), amax = getd(), bmin = getd(), bmax = getd();
    float pa, pb;
    putd(HISTOfindLinearFit(a, b, amin, amax, bmin, bmax, &pa, &pb));
    putf(pa);
    putf(pb);
  }
  else if (cmd == "linscale") {
    HISTOGRAM *h = geth();
    float s = getf(), o = getf();
    puth(HISTOlinearScale(h, NULL, s, o));
    HISTOlinearScale(h, h, s, o);
    puth(h);
  }
  else if (cmd == "threshsum") {
    HISTOGRAM *a = geth(), *b = geth();
    putf(HISTOthreshSum(a, b, getf()));
  }
  else if (cmd == "makepdf") {
    HISTOGRAM *h = geth();
    puth(HISTOmakePDF(h, NULL));
    HISTOmakePDF(h, h);
    puth(h);
  }
  else if (cmd == "bins") {
    int n = geti();
    double mn = getd(), mx = getd();
    puth(HISTObins(n, mn, mx));
  }
  else if (cmd == "count") {
    HISTOGRAM *h = geth();
    int n = geti();
    std::vector<double> v(n);
    for (int i = 0; i < n; i++) v[i] = getd();
    for (int i = 0; i < n; i++) puti(HISTOvalToBin(h, v[i]));
    HISTOcount(h, v.data(), n);
    puth(h);
  }
  else if (cmd == "init") {
    int n = geti();
    double mn = getd(), mx = getd();
    puth(HISTOinit(NULL, n, mn, mx));
  }
  else if (cmd == "initgiven") {
    HISTOGRAM *h = geth();
    int n = geti();
    double mn = getd(), mx = getd();
    HISTOinit(h, n, mn, mx);
    puth(h);
  }
  else if (cmd == "median") {
    HISTOGRAM *h = geth();
    putd(HISTOfindMedian(h));
  }
  else if (cmd == "cdf") {
    HISTOGRAM *h = geth();
    puth(HISTOmakeCDF(h, NULL));
    puth(HISTOmakeReverseCDF(h, NULL));
  }
  else if (cmd == "robust") {
    HISTOGRAM *h = geth();
    double p = getd(), off = -1, sig = -1;
    puti(HISTOrobustGaussianFit(h, p, &off, &sig));
    putd(off);
    putd(sig);
    putf(h->bin_size);
  }
  else if (cmd == "abs") {
    HISTOGRAM *h = geth();
    puth(HISTOabs(h, NULL));
  }
  else if (cmd == "entropy") {
    HISTOGRAM *h = geth();
    putd(HISTOgetEntropy(h));
  }
  else if (cmd == "soap") {
    HISTOGRAM *h = geth();
    int n = geti();
    puth(HISTOsoapBubbleZeros(h, NULL, n));
    HISTOsoapBubbleZeros(h, h, n);
    puth(h);
  }
  else if (cmd == "fwhm") {
    HISTOGRAM *h = geth();
    putf(HISTOcomputeFWHM(h, geti()));
  }
  else if (cmd == "sumnorm") {
    HISTOGRAM *h = geth();
    HISTOsumNorm(h);
    puth(h);
  }
  else if (cmd == "cumsum") {
    HISTOGRAM *h = geth();
    puth(HISTOcumsum(h, NULL));
    HISTOcumsum(h, h);
    puth(h);
  }
  else if (cmd == "clearbins") {
    HISTOGRAM *h = geth();
    int a = geti(), b = geti();
    puth(HISTOclearBins(h, NULL, a, b));
    HISTOclearBins(h, h, a, b);
    puth(h);
  }
  else if (cmd == "addsub") {
    HISTOGRAM *a = geth(), *b = geth();
    puth(HISTOadd(a, b, NULL));
    puth(HISTOsubtract(a, b, NULL));
    puth(HISTOadd(a, NULL, NULL));
    HISTOadd(a, b, a);
    puth(a);
  }
  else if (cmd == "clearcounts") {
    HISTOGRAM *h = geth();
    puth(HISTOclearCounts(h, NULL));
    HISTOclearCounts(h, h);
    puth(h);
  }
  else if (cmd == "copy") {
    HISTOGRAM *h = geth();
    puth(HISTOcopy(h, NULL));
    HISTOGRAM *c = HISTOclear(h, NULL);
    puth(c);
    HISTOclear(h, h);
    puth(h);
  }
  else if (cmd == "plot") {
    HISTOGRAM *h = geth();
    remove(tmpfile);
    puti(HISTOplot(h, tmpfile));
    putfile(tmpfile);
    remove(tmpfile);
    puti(HISTOwriteTxt(h, tmpfile));
    putfile(tmpfile);
  }
  else if (cmd == "mmd") {
    MATRIX *a = getm(MATRIX_REAL), *b = getm(MATRIX_REAL);
    MATRIX *c = MatrixMultiplyD(a, b, NULL);
    putm_raw(c);
    MATRIX *d = MatrixCopy(a, NULL);
    putm_raw(d);
  }
  else if (cmd == "mmdc") {
    int t1 = geti(), t2 = geti();
    MATRIX *a = getm(t1), *b = getm(t2);
    MATRIX *c = MatrixMultiplyD(a, b, NULL);
    putm_raw(c);
    MATRIX *a2 = MatrixCopy(a, NULL);
    MatrixMultiplyD(a2, b, c);
    putm_raw(c);
    putm_raw(MatrixCopy(a, NULL));
  }
  else if (cmd == "affine") {
    MATRIX *a = getm(MATRIX_REAL), *b = getm(MATRIX_REAL);
    AffineMatrix A, B, C;
    SetAffineMatrix(&A, a);
    SetAffineMatrix(&B, b);
    AffineVector x, y;
    float x0 = getf(), x1 = getf(), x2 = getf();
    SetAffineVector(&x, x0, x1, x2);
    x.vec[3] = getf();
    AffineMV(&y, &A, &x);
    for (int i = 0; i < 4; i++) putf(y.vec[i]);
    AffineMM(&C, &A, &B);
    for (int i = 0; i < 16; i++) putf(C.mat[i]);
    MATRIX *m = MatrixAlloc(4, 4, MATRIX_REAL);
    GetAffineMatrix(m, &C);
    putm_raw(m);
    int ix, iy, iz;
    GetFloorAffineVector(&y, &ix, &iy, &iz);
    puti(ix);
    puti(iy);
    puti(iz);
  }
  else if (cmd == "dmat") {
    DMATRIX *a = getdm(), *b = getdm();
    double v1 = getd(), v2 = getd();
    putdm(DMatrixAddMul(a, b, v1, v2, NULL));
    putdm(DMatrixAdd(a, b, NULL));
    putdm(DMatrixSubtract(a, b, NULL));
    putdm(DMatrixScalarMul(a, v1, NULL));
    putdm(DMatrixTranspose(a, NULL));
    putdm(DMatrixCopy(a, NULL));
    putd(DMatrixMaxAbs(a));
  }
  else if (cmd == "dmmul") {
    DMATRIX *a = getdm(), *b = getdm(), *u = getdm(), *v = getdm();
    putdm(DMatrixMultiply(a, b, NULL));
    putd(DVectorDot(u, v));
  }
  else if (cmd == "dliststats") {
    int n = geti();
    std::vector<double> v(n);
    for (int i = 0; i < n; i++) v[i] = getd();
    double *st = DListStats(v.data(), n, NULL);
    for (int i = 0; i < 5; i++) putd(st[i]);
  }
  else if (cmd == "mmf") {
    int t1 = geti(), t2 = geti();
    MATRIX *a = getm(t1), *b = getm(t2);
    MATRIX *c = MatrixMultiply(a, b, NULL);
    putm_n(c);
    MATRIX *a2 = MatrixCopy(a, NULL);
    MatrixMultiply(a2, b, c);
    putm_n(c);
  }
  else if (cmd == "melt") {
    MATRIX *a = getm(MATRIX_REAL), *b = getm(MATRIX_REAL);
    float s = getf();
    int k = geti(), k1 = geti(), k2 = geti();
    int r = a->rows, c = a->cols;
    putm_n(MatrixTranspose(a, NULL));
    putm_n(MatrixAdd(a, b, NULL));
    putm_n(MatrixSubtract(a, b, NULL));
    putm_n(MatrixScalarMul(a, s, NULL));
    putm_n(MatrixSquareElts(a, NULL));
    MATRIX *cl = MatrixCopy(a, NULL);
    MatrixClear(cl);
    putm_n(cl);
    putm_n(MatrixIdentity(r, NULL));
    MATRIX *t = MatrixCopy(a, NULL);
    putm_n(MatrixIdentity(k, t));
    putm_n(MatrixDiag(a, NULL));
    puti(MatrixIsSymmetric(a));
    if (r == c) {
      MATRIX *at = MatrixTranspose(a, NULL);
      puti(MatrixIsSymmetric(MatrixAdd(a, at, NULL)));
    }
    putf(MatrixTrace(a));
    putd(MatrixSumElts(a));
    puti(MatrixIsZero(a));
    puti(MatrixCheck(a));
    putm_n(MatrixConstVal(s, r, c, NULL));
    putm_n(MatrixNormalizeColScale(a, NULL));
    putm_n(MatrixNormalizeCol(a, NULL, NULL));
    putm_n(MatrixReshape(a, NULL, c, r));
    MATRIX *d = MatrixAlloc(k1, k2, MATRIX_REAL);
    MatrixReshape(a, d, 0, 0);
    putm_n(d);
  }
  else if (cmd == "mcplx") {
    MATRIX *a = getm(MATRIX_COMPLEX);
    float s = getf();
    putm_n(MatrixTranspose(a, NULL));
    putm_n(MatrixAdd(a, a, NULL));
    putm_n(MatrixScalarMul(a, s, NULL));
    putm_n(MatrixSquareElts(a, NULL));
    MATRIX *cl = MatrixAlloc(a->rows, a->cols, MATRIX_COMPLEX);
    memcpy(cl->data, a->data, sizeof(float) * 2 * a->rows * a->cols);
    MatrixClear(cl);
    putm_n(cl);
    putf(MatrixTrace(a));
    putd(MatrixSumElts(a));
    puti(MatrixIsZero(a));
    putm_n(MatrixDiag(a, NULL));
    putm_n(MatrixConstVal(s, 0, 0, MatrixCopy(a, NULL)));
  }
  else if (cmd == "mregion") {
    int which = geti();
    MATRIX *src = getmt();
    int given = geti();
    MATRIX *dst = NULL;
    if (given) dst = getmt();
    int sr = geti(), sc = geti(), rows = geti(), cols = geti(), dr = geti(), dc = geti();
    MATRIX *res;
    if (which == 0)
      res = MatrixCopyRegion(src, dst, sr, sc, rows, cols, dr, dc);
    else if (which == 1)
      res = MatrixCopyRealRegion(src, dst, sr, sc, rows, cols, dr, dc);
    else
      res = MatrixCopyImagRegion(src, dst, sr, sc, rows, cols, dr, dc);
    putm_n(res);
    if (dst) putm_raw(dst);
  }
  else if (cmd == "mr2c") {
    MATRIX *a = getm(MATRIX_REAL), *b = getm(MATRIX_REAL), *o = getm(MATRIX_COMPLEX);
    putm_n(MatrixRealToComplex(a, b, NULL));
    putm_n(MatrixRealToComplex(a, b, o));
  }
  else if (cmd == "mrot") {
    int n = geti();
    float angle = getf();
    int which = geti();
    putm_n(MatrixAllocRotation(n, angle, which));
    MATRIX *m = getm(MATRIX_REAL);
    putm_n(MatrixReallocRotation(n, angle, which, m));
  }
  else if (cmd == "mvec") {
    MATRIX *v1 = getm(MATRIX_REAL), *v2 = getm(MATRIX_REAL), *v3 = getm(MATRIX_REAL);
    float norm = getf();
    putf(VectorLen(v1));
    putf(VectorDot(v1, v2));
    putf(VectorDistance(v1, v2));
    putm_n(VectorCrossProduct(v1, v2, NULL));
    putf(VectorTripleProduct(v1, v2, v3));
    XYZ xyz;
    float len;
    XYZ_NORMALIZED_LOAD(&xyz, &len, v1->rptr[1][1], v1->rptr[2][1], v1->rptr[3][1]);
    putf(xyz.x);
    putf(xyz.y);
    putf(xyz.z);
    putf(len);
    putf(XYZApproxAngle_knownLength(&xyz, v2->rptr[1][1], v2->rptr[2][1], v2->rptr[3][1], norm));
  }
  else if (cmd == "v3angle") {
    MATRIX *v1 = getm(MATRIX_REAL), *v2 = getm(MATRIX_REAL);
    putd(Vector3Angle(v1, v2));
  }
  else if (cmd == "mgauss") {
    int len = geti();
    float std1 = getf(), std2 = getf(), w1 = getf();
    int norm = geti();
    putm_n(GaussianMatrix(len, std1, norm, NULL));
    putm_n(GaussianMatrix2(len, std1, std2, w1, norm, NULL));
  }
  else if (cmd == "msim") {
    MATRIX *src = getm(MATRIX_REAL), *mul = getm(MATRIX_REAL);
    putm_n(MatrixSimilarityTransform(src, mul, NULL));
  }
  else if (cmd == "mprint") {
    MATRIX *a = getmt();
    int prec = geti();
    FILE *fp = fopen(tmpfile, "w");
    puti(MatrixPrint(fp, a, prec));
    puti(MatrixPrintFmt(fp, "%10f", a));
    fclose(fp);
    putfile(tmpfile);
    remove(tmpfile);
  }
  else if (cmd == "primes") {
    int nmax = geti(), n = geti(), p = geti();
    int np = 0;
    int *pr = GetPrimes(nmax, &np);
    puti(np);
    long long sum = 0;
    for (int i = 0; i < np; i++) sum += pr[i];
    char b[32];
    snprintf(b, sizeof b, "%lld ", sum);
    line_out += b;
    if (np) puti(pr[np - 1]);
    int nf = 0;
    int *f = GetPrimeFactors(n, &nf);
    puti(nf);
    for (int i = 0; i < nf; i++) puti(f[i]);
    puti(GetClosestPrimeFactor(n, p));
    puti(GetClosestPrimeFactorLess(n, p));
  }
  else if (cmd == "pdfg") {
    long seed = geti();
    int k = geti();
    srand48(seed);
    for (int i = 0; i < k; i++) putd(PDFgaussian());
  }
  else
    fail("unknown case");
}

}  // namespace

int main(int argc, char *argv[])
{
  if (argc != 2) {
    fprintf(stderr, "usage: fsmaths <cases>\n");
    return 2;
  }
  // the scratch file of the commands that write files: beside the cases, removed after use
  std::string tmp = std::string(argv[1]) + ".tmp";
  FILE *fp = fopen(argv[1], "r");
  if (!fp) fail("cannot open the cases");
  static char buf[1 << 20];
  while (fgets(buf, sizeof buf, fp)) {
    toks.clear();
    pos = 0;
    for (char *t = strtok(buf, " \t\n"); t; t = strtok(NULL, " \t\n")) toks.push_back(t);
    if (toks.empty()) continue;
    line_out.clear();
    run_case(tmp.c_str());
    if (!line_out.empty() && line_out.back() == ' ') line_out.pop_back();
    printf("%s\n", line_out.c_str());
    fflush(stdout);
  }
  fclose(fp);
  return 0;
}
