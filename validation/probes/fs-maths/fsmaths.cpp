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

#include <fcntl.h>
#include <signal.h>
#include <sys/wait.h>
#include <unistd.h>

#include <algorithm>
#include <cerrno>
#include <cinttypes>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <iostream>
#include <string>
#include <vector>

#include "affine.h"
#include "diag.h"
#include "dmatrix.h"
#include "error.h"
#include "histo.h"
#include "log.h"
#include "matrix.h"
#include "numerics.h"
#include "pdf.h"
#include "prime.h"
#include "utils.h"

// declared in utils/matrix.cpp only (external linkage)
int MatrixIsSymmetric(MATRIX *matrix);
MATRIX *MatrixCalculateEigenSystemHelper(MATRIX *m, float *evalues, MATRIX *m_evectors, int isSymmetric);
MATRIX *MatrixSVDPseudoInverse(MATRIX *m, MATRIX *m_pseudo_inv);

// defined in utils/fs_powell.cpp (oct_register_mosaic declares it so)
extern void (*powell_iteration_func)(float *p, int nparms);

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

// the file the I/O commands read and write, in the working directory (the cases' directory)
const char *REL_FILE = "fsmaths-file.tmp";

// a file's bytes from a hexadecimal token ("-" for none), written to path
void puthexfile(const char *hex, const char *path)
{
  FILE *fp = fopen(path, "wb");
  if (!fp) fail("cannot write the scratch file");
  if (strcmp(hex, "-") != 0)
    for (size_t i = 0; hex[i] && hex[i + 1]; i += 2) {
      char b[3] = {hex[i], hex[i + 1], 0};
      fputc((int)strtol(b, NULL, 16), fp);
    }
  fclose(fp);
}

// ---- the minimizers' cases (OpenPowell, OpenPowell2, OpenDFPMin) ----
// A seeded cost of n parameters (1-based, as FreeSurfer's callers index them), in float, the
// same expressions as the port's side (tests/fs_maths/cases.rs): 0 a quadratic, 1 Rosenbrock's,
// 2 the quadratic rounded down to a multiple of 1/q (its gradient the quadratic's), 3 weighted
// absolute values (bounded below: with cross terms it would fall without end, the bracketing
// would run into infinities). Every call is hashed (FNV-1a over the bits).
struct Problem {
  int kind = 0, n = 0;
  float f0 = 0, b = 0, q = 1;
  float a[16] = {0}, c[16] = {0};
  bool iter_set = false;
  uint64_t hash = 0xcbf29ce484222325ULL;
  int nf = 0, ng = 0, ni = 0, ns = 0, nu = 0;
} P;

void fnv(uint32_t v)
{
  for (int k = 0; k < 4; k++) {
    P.hash ^= (v >> (8 * k)) & 0xff;
    P.hash *= 0x100000001b3ULL;
  }
}

uint32_t bits(float f)
{
  uint32_t u;
  memcpy(&u, &f, 4);
  return u;
}

void hashp(const float *p, int from)
{
  for (int i = from; i <= P.n; i++) fnv(bits(p[i]));
}

float cost(float p[])
{
  int n = P.n;
  float s = P.f0;
  if (P.kind == 1) {
    for (int i = 1; i < n; i++) {
      float t = p[i + 1] - p[i] * p[i];
      float u = 1.0f - p[i];
      s = s + 100.0f * t * t + u * u;
    }
  }
  else {
    for (int i = 1; i <= n; i++) {
      float d = p[i] - P.c[i];
      if (P.kind == 3)
        s = s + P.a[i] * fabsf(d);
      else
        s = s + P.a[i] * d * d;
    }
    if (P.kind != 3)
      for (int i = 1; i < n; i++) s = s + P.b * ((p[i] - P.c[i]) * (p[i + 1] - P.c[i + 1]));
    if (P.kind == 2) s = floorf(s * P.q) / P.q;
  }
  P.nf++;
  fnv(1);
  hashp(p, 1);
  fnv(bits(s));
  return s;
}

void gradient(float p[], float g[])
{
  int n = P.n;
  for (int i = 1; i <= n; i++) g[i] = 0.0f;
  if (P.kind == 1) {
    for (int i = 1; i < n; i++) {
      float t = p[i + 1] - p[i] * p[i];
      float u = 1.0f - p[i];
      g[i] = g[i] + (-400.0f * t * p[i] - 2.0f * u);
      g[i + 1] = g[i + 1] + 200.0f * t;
    }
  }
  else {
    for (int i = 1; i <= n; i++) {
      float d = p[i] - P.c[i];
      float gi;
      if (P.kind == 3)
        gi = P.a[i] * (d > 0.0f ? 1.0f : (d < 0.0f ? -1.0f : 0.0f));
      else
        gi = 2.0f * P.a[i] * d;
      if (i > 1 && P.kind != 3) gi = gi + P.b * (p[i - 1] - P.c[i - 1]);
      if (i < n && P.kind != 3) gi = gi + P.b * (p[i + 1] - P.c[i + 1]);
      g[i] = gi;
    }
  }
  P.ng++;
  fnv(2);
  hashp(p, 1);
  hashp(g, 1);
}

void iteration(float *p, int nparms)
{
  P.ni++;
  fnv(3);
  fnv((uint32_t)nparms);
  hashp(p, 0);
}

void step(int itno, float sse, void *parms, float *p)
{
  (void)parms;
  P.ns++;
  fnv(4);
  fnv((uint32_t)itno);
  fnv(bits(sse));
  hashp(p, 1);
}

void user(float p[])
{
  P.nu++;
  fnv(5);
  hashp(p, 1);
}

// the problem's tokens: kind n f0 b q a[1..n] c[1..n]
void getproblem()
{
  P = Problem();
  P.kind = geti();
  P.n = geti();
  P.f0 = getf();
  P.b = getf();
  P.q = getf();
  for (int i = 1; i <= P.n; i++) P.a[i] = getf();
  for (int i = 1; i <= P.n; i++) P.c[i] = getf();
}

void puthash()
{
  char b[32];
  snprintf(b, sizeof b, "%016" PRIx64 " ", P.hash);
  line_out += b;
}

// standard output and error captured into files beside the cases while FreeSurfer runs
struct Capture {
  int so, se;
  std::string fo, fe;
};

void cap_begin(Capture &c, const char *tmp)
{
  fflush(stdout);
  fflush(stderr);
  std::cerr.flush();
  c.fo = std::string(tmp) + ".out";
  c.fe = std::string(tmp) + ".err";
  c.so = dup(1);
  c.se = dup(2);
  int fo = open(c.fo.c_str(), O_WRONLY | O_CREAT | O_TRUNC, 0600);
  int fe = open(c.fe.c_str(), O_WRONLY | O_CREAT | O_TRUNC, 0600);
  dup2(fo, 1);
  dup2(fe, 2);
  close(fo);
  close(fe);
}

// VNL's messages name their source file (__FILE__): the unit oracle's ITK tree is written as the
// official build's, /home/rd521/4.13.2_ubuntu20/itk/4.13.2/src/ITK (strings of the official
// mri_ca_register), on every line that starts with a path into ITK's VNL
std::string official_itk(const std::string &s)
{
  const std::string key = "/Modules/ThirdParty/VNL/";
  std::string r;
  size_t i = 0;
  while (i < s.size()) {
    size_t e = s.find('\n', i);
    size_t end = e == std::string::npos ? s.size() : e + 1;
    std::string line = s.substr(i, end - i);
    size_t k = line.find(key);
    if (k != std::string::npos && !line.empty() && line[0] == '/')
      line = "/home/rd521/4.13.2_ubuntu20/itk/4.13.2/src/ITK" + line.substr(k);
    r += line;
    i = end;
  }
  return r;
}

void putcap(const std::string &path)
{
  FILE *fp = fopen(path.c_str(), "rb");
  std::string raw, h;
  int ch;
  char b[4];
  while (fp && (ch = fgetc(fp)) != EOF) raw += (char)ch;
  raw = official_itk(raw);
  for (unsigned char c : raw) {
    snprintf(b, sizeof b, "%02x", c);
    h += b;
  }
  if (fp) fclose(fp);
  remove(path.c_str());
  line_out += h.empty() ? "- " : h + " ";
}

void cap_end(Capture &c)
{
  fflush(stdout);
  fflush(stderr);
  std::cerr.flush();
  dup2(c.so, 1);
  dup2(c.se, 2);
  close(c.so);
  close(c.se);
  putcap(c.fo);
  putcap(c.fe);
}

void putcounts()
{
  puti(P.nf);
  puti(P.ng);
  puti(P.ni);
  puti(P.ns);
  puti(P.nu);
  puthash();
}

void run_minimizer(const std::string &cmd, const char *tmpfile)
{
  getproblem();
  int n = P.n;
  float *p = vector(1, n);
  for (int i = 1; i <= n; i++) p[i] = getf();
  if (cmd == "dfpmin") {
    float tol = getf();
    int init = geti();
    int verbose = geti();
    int stepf = geti(), userf = geti();
    float f = cost(p);
    float fret = init == 0 ? f : (init == 1 ? 0.0f : 2.0f * f);
    int iter = -7;
    unsigned long gdiag = Gdiag;
    if (verbose) Gdiag |= DIAG_VERBOSE;
    Capture c;
    cap_begin(c, tmpfile);
    int r = OpenDFPMin(p, n, tol, &iter, &fret, cost, gradient, stepf ? step : NULL, NULL, userf ? user : NULL);
    Gdiag = gdiag;
    puti(r);
    puti(iter);
    putf(fret);
    for (int i = 1; i <= n; i++) putf(p[i]);
    putcounts();
    cap_end(c);
    free_vector(p, 1, n);
    return;
  }
  float **xi = matrix(1, n, 1, n);
  for (int r = 1; r <= n; r++)
    for (int k = 1; k <= n; k++) xi[r][k] = getf();
  int iter = -7;
  float fret = -7.0f;
  Capture c;
  if (cmd == "powell") {
    float tol = getf();
    P.iter_set = geti() != 0;
    powell_iteration_func = P.iter_set ? iteration : NULL;
    cap_begin(c, tmpfile);
    OpenPowell(p, xi, n, tol, &iter, &fret, cost);
  }
  else {
    float ftol = getf(), lin = getf();
    int maxit = geti(), gno = geti();
    P.iter_set = geti() != 0;
    powell_iteration_func = P.iter_set ? iteration : NULL;
    int save = Gdiag_no;
    Gdiag_no = gno;
    cap_begin(c, tmpfile);
    int r = OpenPowell2(p, xi, n, ftol, lin, maxit, &iter, &fret, cost);
    Gdiag_no = save;
    puti(r);
  }
  powell_iteration_func = NULL;
  puti(iter);
  putf(fret);
  for (int i = 1; i <= n; i++) putf(p[i]);
  for (int r = 1; r <= n; r++)
    for (int k = 1; k <= n; k++) putf(xi[r][k]);
  putcounts();
  cap_end(c);
  free_matrix(xi, 1, n, 1, n);
  free_vector(p, 1, n);
}

void run_case(const char *tmpfile);
bool in_child = false;

// a case that may crash FreeSurfer runs in a child process (OpenDFPMin: FS-37; the robust
// Gaussian fit, whose reads outside its arrays, FS-22, sometimes reach unmapped memory): its line,
// or CRASH when the child dies of a signal (TIMEOUT when its alarm ends it)
void run_forked(const char *tmpfile, unsigned seconds)
{
  int fd[2];
  if (pipe(fd) != 0) fail("pipe");
  fflush(stdout);
  fflush(stderr);
  pid_t pid = fork();
  if (pid < 0) fail("fork");
  if (pid == 0) {
    close(fd[0]);
    alarm(seconds);
    line_out.clear();
    in_child = true;
    pos = 0;
    run_case(tmpfile);
    size_t off = 0;
    while (off < line_out.size()) {
      ssize_t w = write(fd[1], line_out.data() + off, line_out.size() - off);
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
  if (WIFSIGNALED(status) || (WIFEXITED(status) && WEXITSTATUS(status) != 0)) {
    // a signal, or FreeSurfer's code ended the program (ErrorExit: EXIT and its status)
    if (WIFSIGNALED(status))
      line_out = WTERMSIG(status) == SIGALRM ? "TIMEOUT " : "CRASH ";
    else
      line_out = "EXIT " + std::to_string(WEXITSTATUS(status)) + " ";
    std::string t = std::string(tmpfile);
    remove((t + ".out").c_str());
    remove((t + ".err").c_str());
  }
  else
    line_out = got;
}

// the histograms' commands: their cases carry NaNs and infinities (bin numbers from them are
// INT_MIN, FS-17, read and written far outside the arrays), so each runs in a child process with
// a 2-second alarm (their cases take microseconds; TIMEOUT where FreeSurfer never returns)
const char *const HIST_CMDS[] = {"smooth", "smoothip", "smoothinto", "lastpeak", "firstpeak", "valley",
  "lastpeakreg", "highest", "addsample", "prevvalley", "nextvalley", "nextpeak", "prevpeak", "startpeak",
  "endpeak", "currentpeak", "fillholes", "totalreg", "clearzero", "findbin", "correlate", "linfit",
  "linscale", "threshsum", "makepdf", "bins", "count", "init", "initgiven", "median", "cdf", "robust",
  "abs", "entropy", "soap", "fwhm", "sumnorm", "cumsum", "clearbins", "addsub", "clearcounts", "copy",
  "plot"};

bool is_hist_cmd(const std::string &cmd)
{
  for (const char *c : HIST_CMDS)
    if (cmd == c) return true;
  return false;
}

void run_case(const char *tmpfile)
{
  std::string cmd = next();
  if (!in_child && cmd == "dfpmin") {
    run_forked(tmpfile, 600);
    return;
  }
  if (!in_child && is_hist_cmd(cmd)) {
    run_forked(tmpfile, 2);
    return;
  }
  // the eigensystems and SVDs of matrices with infinite or NaN entries: VNL never returns on many
  // of them (EISPACK's pythag and balanc, LINPACK's ssvdc loop for ever: FS-47, ITK-112, FS-48),
  // and MatrixSVDInverse, MatrixPseudoInverse and MatrixSVDPseudoInverse crash on some shapes
  // (FS-44 to FS-46): TIMEOUT after 2 s (their other cases take microseconds)
  // (and MatrixReadFrom of a file that ends early: its sizes are the stack's contents, FS-49)
  if (!in_child && (cmd == "meig" || cmd == "msvd" || cmd == "msvdinv" || cmd == "mpinv" || cmd == "mrdbin")) {
    run_forked(tmpfile, 2);
    return;
  }
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
  else if (cmd == "minv") {
    MATRIX *a = getmt();
    putm_n(MatrixInverse(a, NULL));
    MATRIX *o = MatrixAlloc(a->rows, a->cols, a->type);
    MATRIX *r = MatrixInverse(a, o);
    line_out += r ? "OK " : "NULL ";
    putm_raw(o);
    putf(MatrixDeterminant(a));
  }
  else if (cmd == "mqr") {
    MATRIX *a = getm(MATRIX_REAL);
    MATRIX *q = MatrixAlloc(a->rows, a->rows, MATRIX_REAL);
    MATRIX *r = MatrixAlloc(a->rows, a->cols, MATRIX_REAL);
    puti(MatrixQRdecomposition(a, q, r));
    putm_raw(q);
    putm_raw(r);
  }
  else if (cmd == "meig") {
    // MatrixEigenSystem (new and given outputs) when `full`, and the helper with isSymmetric 1
    // (OpenEigenSystem whatever the matrix: FS-42's NaN cases) and 0
    int full = geti();
    MATRIX *a = getm(MATRIX_REAL);
    int n = a->rows > 0 ? a->rows : 1;
    std::vector<float> ev((size_t)n, 7.0f);
    Capture c;
    cap_begin(c, tmpfile);
    if (full) {
      putm_n(MatrixEigenSystem(a, ev.data(), NULL));
      for (int i = 0; i < n; i++) putf(ev[i]);
      MATRIX *o = MatrixAlloc(a->rows, a->rows, MATRIX_REAL);
      std::fill(ev.begin(), ev.end(), 7.0f);
      MATRIX *r = MatrixEigenSystem(a, ev.data(), o);
      line_out += r ? "OK " : "NULL ";
      putm_raw(o);
      for (int i = 0; i < n; i++) putf(ev[i]);
      std::fill(ev.begin(), ev.end(), 7.0f);
      putm_n(MatrixCalculateEigenSystemHelper(a, ev.data(), NULL, 0));
      for (int i = 0; i < n; i++) putf(ev[i]);
    }
    std::fill(ev.begin(), ev.end(), 7.0f);
    putm_n(MatrixCalculateEigenSystemHelper(a, ev.data(), NULL, 1));
    for (int i = 0; i < n; i++) putf(ev[i]);
    cap_end(c);
  }
  else if (cmd == "msvd") {
    MATRIX *a = getm(MATRIX_REAL);
    int rows = a->rows, cols = a->cols;
    std::vector<float> ev((size_t)(rows > 0 ? rows : 1), 7.0f);
    Capture c;
    cap_begin(c, tmpfile);
    putf(MatrixSVDEigenValues(a, ev.data()));
    for (int i = 0; i < rows; i++) putf(ev[i]);
    putf(MatrixConditionNumber(a));
    putf(MatrixNSConditionNumber(a));
    MATRIX *u = MatrixCopy(a, NULL);
    VECTOR *z = VectorAlloc(cols, MATRIX_REAL);
    putm_n(MatrixSVD(u, z, NULL));
    putm_raw(u);
    putm_raw(z);
    MATRIX *u2 = MatrixCopy(a, NULL);
    VECTOR *z2 = VectorAlloc(cols, MATRIX_REAL);
    MATRIX *v2 = MatrixAlloc(cols, cols, MATRIX_REAL);
    line_out += MatrixSVD(u2, z2, v2) ? "OK " : "NULL ";
    putm_raw(v2);
    putm_raw(u2);
    putm_raw(z2);
    cap_end(c);
  }
  else if (cmd == "msvdinv") {
    MATRIX *a = getm(MATRIX_REAL);
    Capture c;
    cap_begin(c, tmpfile);
    putm_n(MatrixSVDInverse(a, NULL));
    MATRIX *o = MatrixAlloc(a->cols, a->rows, MATRIX_REAL);
    line_out += MatrixSVDInverse(a, o) ? "OK " : "NULL ";
    putm_raw(o);
    cap_end(c);
  }
  else if (cmd == "mpinv") {
    // one call: 0 MatrixPseudoInverse(a, NULL), 1 with a cols x rows output, 2 with a rows x
    // cols one (the inner shape for fewer rows than columns: FreeSurfer frees it, so it is not
    // printed), 3 MatrixSVDPseudoInverse(a, NULL), 4 with a cols x rows output
    int which = geti();
    MATRIX *a = getm(MATRIX_REAL);
    Capture c;
    cap_begin(c, tmpfile);
    MATRIX *o = NULL;
    if (which == 1 || which == 4) o = MatrixAlloc(a->cols, a->rows, MATRIX_REAL);
    if (which == 2) o = MatrixAlloc(a->rows, a->cols, MATRIX_REAL);
    MATRIX *r = which < 3 ? MatrixPseudoInverse(a, o) : MatrixSVDPseudoInverse(a, o);
    if (r == o && o) {
      line_out += "GIVEN ";
      putm_raw(o);
    }
    else {
      putm_n(r);
      if (o && which != 2) putm_raw(o);
    }
    cap_end(c);
  }
  else if (cmd == "mrdtxt") {
    // MatrixReadTxt of a file (its bytes in hexadecimal); errno after it
    // (by a name relative to the cases' directory, the working directory: the messages print it)
    puthexfile(next(), REL_FILE);
    errno = 0;
    Capture c;
    cap_begin(c, tmpfile);
    putm_n(MatrixReadTxt(REL_FILE, NULL));
    puti(errno);
    cap_end(c);
    remove(REL_FILE);
  }
  else if (cmd == "mrdasc" || cmd == "mrdbin") {
    // MatrixAsciiReadFrom or MatrixReadFrom of a file, into a new matrix or a given one of the
    // type and sizes the case names; errno and the bytes read after it
    int given = geti();
    int t = geti(), r = geti(), cc = geti();
    puthexfile(next(), REL_FILE);
    Capture c;
    cap_begin(c, tmpfile);
    MATRIX *o = given ? MatrixAlloc(r, cc, t) : NULL;
    FILE *fp = fopen(REL_FILE, "rb");
    errno = 0;
    MATRIX *res = cmd == "mrdasc" ? MatrixAsciiReadFrom(fp, o) : MatrixReadFrom(fp, o);
    if (res && res == o) {
      line_out += "GIVEN ";
      putm_raw(o);
    }
    else {
      putm_n(res);
      if (o) putm_raw(o);
    }
    puti(errno);
    puti((int)ftell(fp));
    cap_end(c);
    fclose(fp);
    remove(REL_FILE);
  }
  else if (cmd == "mwr") {
    // MatrixAsciiWriteInto and MatrixWriteTxt of a matrix: the files' bytes
    MATRIX *a = getmt();
    FILE *fp = fopen(REL_FILE, "wb");
    puti(MatrixAsciiWriteInto(fp, a));
    fclose(fp);
    putfile(REL_FILE);
    Capture c;
    cap_begin(c, tmpfile);
    puti(MatrixWriteTxt(REL_FILE, a));
    cap_end(c);
    putfile(REL_FILE);
    remove(REL_FILE);
  }
  else if (cmd == "powell" || cmd == "powell2") {
    run_minimizer(cmd, tmpfile);
  }
  else if (cmd == "dfpmin") {
    run_minimizer(cmd, tmpfile);
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
  if (argv[1][0] != '/') fail("the cases must be given by an absolute path");
  {
    std::string dir = std::string(argv[1]);
    size_t slash = dir.rfind('/');
    dir = slash == std::string::npos ? std::string(".") : dir.substr(0, slash ? slash : 1);
    if (chdir(dir.c_str()) != 0) fail("cannot enter the cases' directory");
  }
  // every case read first and the file closed: a child process that FreeSurfer's code ends with
  // exit() flushes the stdio streams it inherited, and glibc's flush of an input stream moves the
  // file offset the parent shares back to that stream's position (the parent then read the
  // following cases again)
  FILE *fp = fopen(argv[1], "r");
  if (!fp) fail("cannot open the cases");
  std::vector<std::string> lines;
  static char buf[1 << 20];
  while (fgets(buf, sizeof buf, fp)) lines.push_back(buf);
  fclose(fp);
  for (const std::string &line : lines) {
    if (line.size() >= sizeof buf) fail("a case longer than the buffer");
    memcpy(buf, line.c_str(), line.size() + 1);
    toks.clear();
    pos = 0;
    for (char *t = strtok(buf, " \t\n"); t; t = strtok(NULL, " \t\n")) toks.push_back(t);
    if (toks.empty()) continue;
    line_out.clear();
    Gerror = NO_ERROR;
    errno = 0;
    run_case(tmp.c_str());
    if (!line_out.empty() && line_out.back() == ' ') line_out.pop_back();
    printf("%s\n", line_out.c_str());
    fflush(stdout);
  }
  return 0;
}
