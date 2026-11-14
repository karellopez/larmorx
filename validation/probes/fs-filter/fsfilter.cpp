// SPDX-License-Identifier: Apache-2.0
// fs.filter's probe: FreeSurfer 8.2.0's volume filters (the fast-marching distance maps
// MRIextractDistanceMap and MRIdistanceTransform, ...) called case by case, linked with the unit
// oracle's libutils as the official programs are (build.sh). CLOSED TO FREESURFER CLEAN-ROOM
// IMPLEMENTERS: it names FreeSurfer's functions (running it is a black-box run of FreeSurfer's code).
//
//     fsfilter <cases>     one output line per case line (absolute path; see check.py)
//
// Every case runs in a child process whose standard output and error go to a scratch file beside
// the cases; the parent prints `<result> | <text>`, the text C-escaped (\xHH for bytes outside
// printable ASCII and for the backslash), or `EXIT <status> | <text>` / `CRASH <signal> | <text>`
// when FreeSurfer's code ended the child. Floats in and out are their bits in hex; a voxel token
// `v*k` stands for k voxels of the value v.

#include <fcntl.h>
#include <sys/stat.h>
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

#include "connectcomp.h"
#include "diag.h"
#include "error.h"
#include "fastmarching.h"
#include "fmriutils.h"
#include "label.h"
#include "matrix.h"
#include "volcluster.h"
#include "mri.h"
#include "mrinorm.h"
#include "mrisegment.h"
#include "utils.h"

MRI *MRIstdNonzero(MRI *mri_src, MRI *mri_dst, MRI *mri_mean, int wsize);
int MRImaxSegmentArea(MRI_SEGMENTATION *mriseg);
MRI *MRISaccentuate(MRI *mri_src, MRI *mri_dst, int lo_thresh, int hi_thresh);
MRI *MRISfloodoutside(MRI *mri_src, MRI *mri_dst);
float distance(float x, float y, float z);

const char *Progname = "fsfilter";

namespace {

std::vector<std::string> toks;
size_t pos;
std::string out;
std::string scratch;

[[noreturn]] void fail(const char *what)
{
  fprintf(stderr, "fsfilter: %s\n", what);
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

// a volume of the case: w h d type nframes xsize ysize zsize, then its stored voxels (every frame;
// floats as their bits, the integer types as decimal numbers; `v*k` for k equal voxels)
MRI *get_vol()
{
  int w = geti(), h = geti(), d = geti(), type = geti(), nf = geti();
  MRI *m = MRIallocSequence(w, h, d, type, nf);
  m->xsize = getf();
  m->ysize = getf();
  m->zsize = getf();
  size_t n = (size_t)w * h * d * nf, i = 0;
  unsigned char *b = (unsigned char *)m->chunk;
  while (i < n) {
    std::string t = next();
    size_t star = t.find('*');
    size_t k = star == std::string::npos ? 1 : strtoul(t.c_str() + star + 1, NULL, 10);
    std::string v = t.substr(0, star);
    for (size_t j = 0; j < k && i < n; j++, i++) {
      switch (type) {
        case MRI_UCHAR: b[i] = (unsigned char)strtol(v.c_str(), NULL, 10); break;
        case MRI_SHORT: { short s = (short)strtol(v.c_str(), NULL, 10); memcpy(b + 2 * i, &s, 2); break; }
        case MRI_USHRT: { unsigned short s = (unsigned short)strtol(v.c_str(), NULL, 10); memcpy(b + 2 * i, &s, 2); break; }
        case MRI_INT: { int s = (int)strtol(v.c_str(), NULL, 10); memcpy(b + 4 * i, &s, 4); break; }
        case MRI_LONG: { long s = strtol(v.c_str(), NULL, 10); memcpy(b + 8 * i, &s, 8); break; }
        case MRI_FLOAT: { uint32_t u = (uint32_t)strtoul(v.c_str(), NULL, 16); memcpy(b + 4 * i, &u, 4); break; }
        default: fail("a voxel type the cases do not write");
      }
    }
  }
  return m;
}

// an optional volume: 0 for none, 1 and a volume
MRI *get_opt() { return geti() ? get_vol() : NULL; }

double getd()
{
  uint64_t u = (uint64_t)strtoull(next(), NULL, 16);
  double d;
  memcpy(&d, &u, 8);
  return d;
}

// a volume with its centre: get_vol's tokens, then c_r c_a c_s (float bits)
MRI *get_volc()
{
  MRI *m = get_vol();
  m->c_r = getf();
  m->c_a = getf();
  m->c_s = getf();
  MRIreInitCache(m);
  return m;
}

// a kernel: its length, then its weights (float bits)
std::vector<float> get_kernel()
{
  int n = geti();
  std::vector<float> k(n > 0 ? n : 0);
  for (int i = 0; i < n; i++) k[i] = getf();
  return k;
}

// the volume: extent, type, frames, outside_val, xsize, then its stored voxels as get_vol reads them
void put_vol(const MRI *m)
{
  if (!m) { out += "NULL "; return; }
  oi(m->width); oi(m->height); oi(m->depth); oi(m->type); oi(m->nframes);
  od(m->outside_val);
  of(m->xsize);
  size_t n = (size_t)m->width * m->height * m->depth * m->nframes;
  const unsigned char *b = (const unsigned char *)m->chunk;
  std::string prev;
  size_t run = 0;
  auto flush = [&]() {
    if (!run) return;
    out += prev;
    if (run > 1) out += "*" + std::to_string(run);
    out += " ";
  };
  for (size_t i = 0; i < n; i++) {
    char t[32];
    switch (m->type) {
      case MRI_UCHAR: snprintf(t, sizeof t, "%d", b[i]); break;
      case MRI_SHORT: { short s; memcpy(&s, b + 2 * i, 2); snprintf(t, sizeof t, "%d", s); break; }
      case MRI_USHRT: { unsigned short s; memcpy(&s, b + 2 * i, 2); snprintf(t, sizeof t, "%d", s); break; }
      case MRI_INT: { int s; memcpy(&s, b + 4 * i, 4); snprintf(t, sizeof t, "%d", s); break; }
      case MRI_LONG: { long s; memcpy(&s, b + 8 * i, 8); snprintf(t, sizeof t, "%ld", s); break; }
      case MRI_FLOAT: { uint32_t u; memcpy(&u, b + 4 * i, 4); snprintf(t, sizeof t, "%08x", u); break; }
      default: fail("a voxel type the cases do not print");
    }
    if (run && prev == t) {
      run++;
    } else {
      flush();
      prev = t;
      run = 1;
    }
  }
  flush();
}

// the header fields fs.filter's functions set: the voxel sizes, direction cosines, centre, start
// and end coordinates, thickness, pixel size (float bits), scale, imnr0, imnr1
void put_hdr(const MRI *m)
{
  if (!m) return;
  for (float f : {m->xsize, m->ysize, m->zsize, m->x_r, m->x_a, m->x_s, m->y_r, m->y_a, m->y_s, m->z_r,
                  m->z_a, m->z_s, m->c_r, m->c_a, m->c_s, m->xstart, m->xend, m->ystart, m->yend,
                  m->zstart, m->zend, m->thick, m->ps})
    of(f);
  oi(m->scale); oi(m->imnr0); oi(m->imnr1);
}

void put_full(const MRI *m)
{
  put_vol(m);
  put_hdr(m);
}

// the destination of a function F(src, ..., dst): 0 none, 1 a given volume, 2 the source itself
MRI *get_dst(int mode, MRI *src) { return mode == 1 ? get_vol() : mode == 2 ? src : NULL; }

// the result r of a call given dst: whether it is dst, then r's volume (NULL when NULL), then dst's
// when one was given and r is another
void put_result(MRI *r, MRI *dst)
{
  oi(r == dst);
  put_full(r);
  if (dst && r != dst) put_full(dst);
}

bool filter_case(const std::string &cmd)
{
  if (cmd == "g1d") {
    float sigma = getf();
    int max_len = geti();
    put_full(MRIgaussian1d(sigma, max_len));
  } else if (cmd == "cv1" || cmd == "cvb" || cmd == "cvs" || cmd == "cvi") {
    // cv1|cvb|cvs|cvi <src> <dst?> <kernel> axis src_frame dst_frame
    MRI *src = get_vol();
    MRI *dst = get_opt();
    std::vector<float> k = get_kernel();
    int axis = geti(), sf = geti(), df = geti();
    float *kp = k.data();
    int len = (int)k.size();
    MRI *r = cmd == "cv1" ? MRIconvolve1d(src, dst, kp, len, axis, sf, df)
           : cmd == "cvb" ? MRIconvolve1dByte(src, dst, kp, len, axis, sf, df)
           : cmd == "cvs" ? MRIconvolve1dShort(src, dst, kp, len, axis, sf, df)
                          : MRIconvolve1dInt(src, dst, kp, len, axis, sf, df);
    put_result(r, dst);
  } else if (cmd == "cvg") {
    // cvg <src> <dst mode> [<dst>] <kernel volume>
    MRI *src = get_vol();
    MRI *dst = get_dst(geti(), src);
    MRI *kv = get_vol();
    MRI *r = MRIconvolveGaussian(src, dst, kv);
    put_result(r, dst);
  } else if (cmd == "gsn" || cmd == "gs") {
    // gsn <src> <dst mode> [<dst>] cstd rstd sstd (double bits); gs <src> <dst mode> [<dst>] std norm
    MRI *src = get_vol();
    MRI *dst = get_dst(geti(), src);
    MRI *r;
    if (cmd == "gsn") {
      double c = getd(), rr = getd(), s = getd();
      r = MRIgaussianSmoothNI(src, c, rr, s, dst);
    } else {
      double s = getd();
      int norm = geti();
      r = MRIgaussianSmooth(src, s, norm, dst);
    }
    put_result(r, dst);
  } else if (cmd == "mgs") {
    // mgs <src> <mask?> <dst mode> [<dst>] std
    MRI *src = get_vol();
    MRI *mask = get_opt();
    MRI *dst = get_dst(geti(), src);
    double s = getd();
    put_result(MRImaskedGaussianSmooth(src, mask, s, dst), dst);
  } else if (cmd == "mean") {
    MRI *src = get_vol();
    MRI *dst = get_opt();
    int wsize = geti();
    put_result(MRImean(src, dst, wsize), dst);
  } else if (cmd == "std" || cmd == "stdnz") {
    MRI *src = get_vol();
    MRI *dst = get_opt();
    MRI *mean = get_vol();
    int wsize = geti();
    MRI *r = cmd == "std" ? MRIstd(src, dst, mean, wsize) : MRIstdNonzero(src, dst, mean, wsize);
    put_result(r, dst);
  } else if (cmd == "red" || cmd == "red2") {
    MRI *src = get_volc();
    MRI *dst = get_opt();
    put_result(cmd == "red" ? MRIreduce(src, dst) : MRIreduce2D(src, dst), dst);
    put_hdr(src);
  } else if (cmd == "r1d" || cmd == "rsl") {
    MRI *src = get_vol();
    MRI *dst = get_opt();
    std::vector<float> k = get_kernel();
    int axis = geti();
    MRI *r = cmd == "r1d" ? MRIreduce1d(src, dst, k.data(), (int)k.size(), axis)
                          : MRIreduceSlice(src, dst, k.data(), (int)k.size(), axis);
    put_result(r, dst);
  } else if (cmd == "xsob" || cmd == "ysob" || cmd == "zsob") {
    MRI *src = get_vol();
    MRI *dst = get_opt();
    int frame = geti();
    MRI *r = cmd == "xsob" ? MRIxSobel(src, dst, frame) : cmd == "ysob" ? MRIySobel(src, dst, frame)
                                                         : MRIzSobel(src, dst, frame);
    put_result(r, dst);
  } else if (cmd == "sob" || cmd == "sobf") {
    MRI *src = get_vol();
    MRI *grad = get_opt();
    MRI *mag = get_opt();
    MRI *r = cmd == "sob" ? MRIsobel(src, grad, mag) : MRIsobelFrame(src, grad, mag, geti());
    put_result(r, grad);
    if (mag) put_full(mag);
  } else if (cmd == "div" || cmd == "lap") {
    MRI *src = get_vol();
    MRI *dst = get_opt();
    put_result(cmd == "div" ? MRIdivergence(src, dst) : MRIlaplacian(src, dst), dst);
  } else if (cmd == "mbv") {
    MRI *src = get_vol();
    MRI *dst = get_opt();
    put_result(MRImarkBorderVoxels(src, dst), dst);
  } else if (cmd == "bcv") {
    // bcv <src> <labeled> wsize x y z: the label, then pw and pg (float bits)
    MRI *src = get_vol();
    MRI *lab = get_vol();
    int ws = geti(), x = geti(), y = geti(), z = geti();
    float pw = -1, pg = -1;
    oi(MRIborderClassifyVoxel(src, lab, ws, x, y, z, &pw, &pg));
    of(pw);
    of(pg);
  } else if (cmd == "rcl") {
    // rcl <src> <labeled> <dst mode: 0 none, 1 given, 2 the labels> [<dst>] wm_low gray_hi wsize
    MRI *src = get_vol();
    MRI *lab = get_vol();
    int m = geti();
    MRI *dst = m == 1 ? get_vol() : m == 2 ? lab : NULL;
    float lo = getf(), hi = getf();
    int ws = geti();
    oi(MRIreclassify(src, lab, dst, lo, hi, ws));
    put_full(lab);
    if (dst && dst != lab) put_full(dst);
  } else if (cmd == "ccs") {
    MRI *t1 = get_vol();
    MRI *lab = get_vol();
    float lo = getf(), hi = getf();
    float a = -1, b = -1, c = -1, d = -1;
    oi(MRIcomputeClassStatistics(t1, lab, lo, hi, &a, &b, &c, &d));
    of(a);
    of(b);
    of(c);
    of(d);
  } else if (cmd == "rmh" || cmd == "lcc6" || cmd == "lcc18") {
    MRI *v = get_vol();
    if (cmd == "rmh") RemoveHoles(v);
    else if (cmd == "lcc6") GetLargestCC6(v);
    else GetLargestCC18(v);
    put_full(v);
  } else if (cmd == "gf6" || cmd == "gf18") {
    // gf6|gf18 <volume> <labels> label x y z: the size and bounds, then the labels
    MRI *v = get_vol();
    MRI *lab = get_vol();
    int label = geti();
    POINTI p;
    p.x = geti();
    p.y = geti();
    p.z = geti();
    int sz = -1, a = -1, b = -1, c = -1, d = -1, e = -1, f = -1;
    if (cmd == "gf6") GrassFire6(v, lab, label, &p, &sz, &a, &b, &c, &d, &e, &f);
    else GrassFire18(v, lab, label, &p, &sz, &a, &b, &c, &d, &e, &f);
    oi(sz); oi(a); oi(b); oi(c); oi(d); oi(e); oi(f);
    put_full(lab);
  } else {
    return false;
  }
  return true;
}

// a matrix: rows cols, then its elements row by row (float bits)
MATRIX *get_mat()
{
  int rows = geti(), cols = geti();
  MATRIX *m = MatrixAlloc(rows, cols, MATRIX_REAL);
  for (int r = 1; r <= rows; r++)
    for (int c = 1; c <= cols; c++) m->rptr[r][c] = getf();
  return m;
}

// the matrix: rows cols, then its elements row by row (float bits); NULL when NULL
void put_mat(const MATRIX *m)
{
  if (!m) { out += "NULL "; return; }
  oi(m->rows); oi(m->cols);
  for (int r = 1; r <= m->rows; r++)
    for (int c = 1; c <= m->cols; c++) of(m->rptr[r][c]);
}

// fmriutils.cpp's frame functions
bool fmri_case(const std::string &cmd)
{
  if (cmd == "fmm") {
    MRI *src = get_vol();
    MATRIX *M = get_mat();
    MRI *dst = get_opt();
    put_result(fMRImatrixMultiply(src, M, dst), dst);
  } else if (cmd == "fcov" || cmd == "far1") {
    // fcov <src> lag adj <mask?> <dst?>; far1 <src> adj <mask?> <dst?>
    MRI *src = get_vol();
    int lag = cmd == "fcov" ? geti() : 0;
    float adj = getf();
    MRI *mask = get_opt();
    MRI *dst = get_opt();
    MRI *r = cmd == "fcov" ? fMRIcovariance(src, lag, adj, mask, dst) : fMRItemporalAR1(src, adj, mask, dst);
    put_result(r, dst);
  } else if (cmd == "fskip" || cmd == "fdrop" || cmd == "ffrm") {
    MRI *src = get_vol();
    int n = geti();
    MRI *dst = get_opt();
    MRI *r = cmd == "fskip" ? fMRInskip(src, n, dst) : cmd == "fdrop" ? fMRIndrop(src, n, dst) : fMRIframe(src, n, dst);
    put_result(r, dst);
  } else if (cmd == "vmax" || cmd == "vmin" || cmd == "conj" || cmd == "fmed" || cmd == "fsum") {
    MRI *src = get_vol();
    MRI *dst = get_opt();
    MRI *r = cmd == "vmax" ? MRIvolMax(src, dst) : cmd == "vmin" ? MRIvolMin(src, dst)
           : cmd == "conj" ? MRIconjunct(src, dst) : cmd == "fmed" ? MRIframeMedian(src, dst)
                           : MRIframeSum(src, dst);
    put_result(r, dst);
  } else if (cmd == "vmaxi") {
    MRI *src = get_vol();
    int base = geti();
    MRI *mask = get_opt();
    MRI *dst = get_opt();
    put_result(MRIvolMaxIndex(src, base, mask, dst), dst);
  } else if (cmd == "fmean") {
    // fmean <src> <dst?> n w1 .. wn (double bits)
    MRI *src = get_vol();
    MRI *dst = get_opt();
    int n = geti();
    std::vector<double> w(n);
    for (int i = 0; i < n; i++) w[i] = getd();
    put_result(MRIframeMean(src, dst, w), dst);
  } else if (cmd == "fsub") {
    MRI *src = get_vol();
    int start = geti(), delta = geti(), stop = geti();
    MRI *dst = get_opt();
    put_result(fMRIsubSample(src, start, delta, stop, dst), dst);
  } else if (cmd == "fext") {
    MRI *src = get_vol();
    int n = geti();
    std::vector<int> fl(n);
    for (int i = 0; i < n; i++) fl[i] = geti();
    put_full(fMRIextractFrames(src, fl));
  } else if (cmd == "asl") {
    put_mat(ASLinterpMatrix(geti()));
  } else if (cmd == "f2m") {
    // f2m <src> <0 | 1 matrix>: whether the result is the given matrix, the result, the given one
    MRI *src = get_vol();
    MATRIX *M = geti() ? get_mat() : NULL;
    MATRIX *r = fMRItoMatrix(src, M);
    oi(r == M);
    put_mat(r);
    if (M && r != M) put_mat(M);
  } else if (cmd == "m2f") {
    MATRIX *M = get_mat();
    MRI *v = get_vol();
    MRI *mask = get_opt();
    oi(fMRIfromMatrix(M, v, mask));
    put_full(v);
  } else if (cmd == "scm") {
    put_full(fMRIspatialCorMatrix(get_vol()));
  } else if (cmd == "fcum" || cmd == "fnorm") {
    MRI *src = get_vol();
    MRI *mask = get_opt();
    MRI *dst = get_opt();
    put_result(cmd == "fcum" ? fMRIcumSum(src, mask, dst) : MRIframeNorm(src, mask, dst), dst);
  } else {
    return false;
  }
  return true;
}

// the clusters: the count, then each one's members, largest member, its value and voxel size
// (float bits), then its members' column, row and slice (and x, y, z when xyz); NULL when NULL
void put_clusters(VOLCLUSTER **l, int n, int xyz)
{
  if (!l) { out += "NULL "; return; }
  oi(n);
  for (int i = 0; i < n; i++) {
    VOLCLUSTER *vc = l[i];
    oi(vc->nmembers); oi(vc->maxmember); of(vc->maxval); of(vc->voxsize);
    for (int k = 0; k < vc->nmembers; k++) {
      oi(vc->col[k]); oi(vc->row[k]); oi(vc->slc[k]);
      if (xyz) { of(vc->x[k]); of(vc->y[k]); of(vc->z[k]); }
    }
  }
}

MATRIX *get_optmat() { return geti() ? get_mat() : NULL; }

// volcluster.cpp's clusters
bool clus_case(const std::string &cmd)
{
  if (cmd == "clus") {
    // clus <vol> frame thmin thmax thsign minsize <mask?> <xfm?> allowdiag gdiag valopt: the
    // clusters, then clustClusterList2Vol's volume
    MRI *v = get_vol();
    int frame = geti();
    float lo = getf(), hi = getf();
    int sign = geti();
    float minsize = getf();
    MRI *mask = get_opt();
    MATRIX *x = get_optmat();
    int diag = geti();
    Gdiag_no = geti();
    int valopt = geti();
    int n = -1;
    VOLCLUSTER **l = clustGetClusters(v, frame, lo, hi, sign, minsize, mask, &n, x, diag);
    put_clusters(l, n, x != NULL);
    put_full(clustClusterList2Vol(l, n, v, frame, valopt));
  } else if (cmd == "cprd") {
    // cprd <vol> thmin thsign <xfm?> distthresh: the clusters clustPruneByDistance keeps
    MRI *v = get_vol();
    float lo = getf();
    int sign = geti();
    MATRIX *x = get_optmat();
    float dist = getf();
    int n = -1, nkeep = -1;
    VOLCLUSTER **l = clustGetClusters(v, 0, lo, 0, sign, 0, NULL, &n, x);
    VOLCLUSTER **p = clustPruneByDistance(l, n, dist, &nkeep);
    put_clusters(p, nkeep, x != NULL);
  } else if (cmd == "cgrow") {
    // cgrow <vol> thmin thsign col row slc allowdiag npassesmax: the grown cluster, the hit map
    MRI *v = get_vol();
    float lo = getf();
    int sign = geti();
    int c = geti(), r = geti(), s = geti(), diag = geti(), np = geti();
    int nh = -1, *hc = NULL, *hr = NULL, *hs = NULL;
    MRI *hm = clustInitHitMap(v, 0, lo, 0, sign, &nh, &hc, &hr, &hs, NULL, 0);
    VOLCLUSTER *k = clustGrow(c, r, s, hm, diag, np);
    put_clusters(&k, 1, 0);
    put_full(hm);
  } else if (cmd == "isl" || cmd == "hol") {
    // isl <mask> <dst mode> [<dst>] thresh nkeep; hol ... fill (doubles as their bits)
    MRI *m = get_vol();
    MRI *dst = get_dst(geti(), m);
    double th = getd();
    int nk = geti();
    MRI *r = cmd == "isl" ? MRIremoveVolumeIslands(m, th, nk, dst)
                          : MRIremoveVolumeHoles(m, th, nk, getd(), dst);
    put_result(r, dst);
  } else if (cmd == "rwc") {
    // rwc <norm> <seg> <dst mode> [<dst>] nitersmax: the result, then the segmentation
    MRI *norm = get_vol();
    MRI *seg = get_vol();
    MRI *dst = get_dst(geti(), seg);
    int it = geti();
    put_result(MRIreclassifyWMCtxNonPar(norm, seg, it, dst), dst);
    put_full(seg);
  } else if (cmd == "amb") {
    // amb <norm> <seg> <0 | 1 statseg> Qwm Qctx NdilWM NdilCtx: the result, then the FNV-1a of
    // the wm.pdf.dat and ctx.pdf.dat it writes into the current directory (<cases>.out.d)
    MRI *norm = get_vol();
    MRI *seg = get_vol();
    MRI *stat = geti() ? get_vol() : seg;
    double qw = getd(), qc = getd();
    int nw = geti(), nc = geti();
    std::string d = scratch + ".d";
    mkdir(d.c_str(), 0700);
    if (chdir(d.c_str()) != 0) fail("chdir");
    unlink("wm.pdf.dat");
    unlink("ctx.pdf.dat");
    put_full(MRIclassifyAmbiguousNonPar(norm, seg, stat, qw, qc, nw, nc));
    for (const char *f : {"wm.pdf.dat", "ctx.pdf.dat"}) {
      FILE *fp = fopen(f, "rb");
      if (!fp) { out += "NOFILE "; continue; }
      uint64_t h = 0xcbf29ce484222325ull;
      int ch;
      while ((ch = fgetc(fp)) != EOF) h = (h ^ (unsigned char)ch) * 0x100000001b3ull;
      fclose(fp);
      unlink(f);
      char t[24];
      snprintf(t, sizeof t, "%016" PRIx64 " ", h);
      out += t;
    }
  } else if (cmd == "cvr") {
    float lo = getf(), hi = getf();
    int sign = geti();
    for (int i = 0; i < 8; i++) oi(clustValueInRange(getf(), lo, hi, sign));
  } else {
    return false;
  }
  return true;
}

// a segmented volume: 0 and a volume (get_vol), or 1 and a pattern of ones and zeros (uchar, voxel
// sizes 1): w h d p_lo p_hi zsplit seed, voxel i (slice z) 1 when a 32-bit hash of i and the seed,
// modulo 1000, is below p_lo (z < zsplit) or p_hi; p_lo -1: a checkerboard below zsplit; p_lo -2: a
// checkerboard beside a pillar of the last zsplit columns
MRI *get_segvol()
{
  if (!geti()) return get_vol();
  int w = geti(), h = geti(), d = geti(), plo = geti(), phi = geti(), zsplit = geti();
  uint32_t seed = (uint32_t)strtoul(next(), NULL, 10);
  MRI *m = MRIalloc(w, h, d, MRI_UCHAR);
  m->xsize = m->ysize = m->zsize = 1;
  unsigned char *b = (unsigned char *)m->chunk;
  size_t n = (size_t)w * h * d;
  for (size_t i = 0; i < n; i++) {
    int z = (int)(i / ((size_t)w * h)), y = (int)(i % ((size_t)w * h) / w), xx = (int)(i % w);
    if (plo == -1 && z < zsplit) {
      b[i] = (xx + y + z) % 2 == 0;
    } else if (plo == -2) {
      b[i] = xx >= w - zsplit || (xx + y + z) % 2 == 0;
    } else {
      uint32_t x = (uint32_t)i * 2654435761u + seed * 40503u;
      x ^= x >> 13;
      x *= 0x5bd1e995u;
      x ^= x >> 15;
      b[i] = (int)(x % 1000) < (z < zsplit ? plo : phi) ? 1 : 0;
    }
  }
  return m;
}

// the segmentation: nsegments max_segments, then every slot's nvoxels, area, label, found, centroid,
// bounds, ignore and voxels; its FNV-1a (64 bits) as H<hex> when longer than 4000 characters
void put_seg(const MRI_SEGMENTATION *sg)
{
  if (!sg) { out += "NULL "; return; }
  std::string keep;
  keep.swap(out);
  oi(sg->nsegments); oi(sg->max_segments);
  for (int s = 0; s < sg->max_segments; s++) {
    const MRI_SEGMENT *m = &sg->segments[s];
    oi(m->nvoxels); of(m->area); oi(m->label); oi(m->found); of(m->cx); of(m->cy); of(m->cz);
    oi(m->x0); oi(m->x1); oi(m->y0); oi(m->y1); oi(m->z0); oi(m->z1); oi(m->ignore);
    for (int v = 0; v < m->nvoxels; v++) { oi(m->voxels[v].x); oi(m->voxels[v].y); oi(m->voxels[v].z); }
  }
  std::string t;
  t.swap(out);
  out.swap(keep);
  if (t.size() > 4000) {
    uint64_t hsh = 0xcbf29ce484222325ull;
    for (unsigned char c : t) hsh = (hsh ^ c) * 0x100000001b3ull;
    char b[24];
    snprintf(b, sizeof b, "H%016" PRIx64 " ", hsh);
    out += b;
  } else {
    out += t;
  }
}

// mrisegment.cpp's segments: seg|mseg <segvol> low hi nops <op>...: MRIsegment or MRImaxsegment,
// then each operation's results, then the segmentation; salloc max_segments max_voxels
bool seg_case(const std::string &cmd)
{
  // mrinorm.cpp: vor <src> <ctrl> <dst?>; soap <src> <ctrl> <dst mode> [<dst>] niter min_change: the result,
  // then the control volume (vor)
  if (cmd == "vor") {
    MRI *src = get_vol();
    MRI *ctrl = get_vol();
    MRI *dst = get_opt();
    put_result(MRIbuildVoronoiDiagram(src, ctrl, dst), dst);
    put_vol(ctrl);
    return true;
  }
  if (cmd == "vsr") {
    // vsr <volume file> <label file> label type niter min_change: the control points where the
    // label volume (as int) equals label, the volume as uchar or float (type 0 or 3), its Voronoi
    // diagram, then the soap bubble in place (mri_normalize's call); the FNV-1a of the diagram, of
    // the bubble and of the control volume after
    MRI *src = MRIread(next());
    MRI *lab = MRIread(next());
    if (!src || !lab) fail("cannot read the volume");
    int label = geti(), type = geti(), niter = geti();
    float mc = getf();
    MRI *ctrl = MRIalloc(lab->width, lab->height, lab->depth, MRI_UCHAR);
    for (int z = 0; z < lab->depth; z++)
      for (int y = 0; y < lab->height; y++)
        for (int x = 0; x < lab->width; x++) MRIvox(ctrl, x, y, z) = (int)MRIgetVoxVal(lab, x, y, z, 0) == label;
    if (type == MRI_FLOAT) {
      MRI *f = MRIallocSequence(src->width, src->height, src->depth, MRI_FLOAT, 1);
      MRIcopyHeader(src, f);
      for (int z = 0; z < src->depth; z++)
        for (int y = 0; y < src->height; y++)
          for (int x = 0; x < src->width; x++) MRIFvox(f, x, y, z) = MRIgetVoxVal(src, x, y, z, 0);
      src = f;
    }
    MRI *bias = MRIbuildVoronoiDiagram(src, ctrl, NULL);
    for (int k = 0; k < 3; k++) {
      const MRI *m = k == 2 ? ctrl : bias;
      if (k == 1) MRIsoapBubble(bias, ctrl, bias, niter, mc);
      uint64_t h = 0xcbf29ce484222325ull;
      const unsigned char *b = (const unsigned char *)m->chunk;
      for (size_t i = 0; i < m->bytes_total; i++) h = (h ^ b[i]) * 0x100000001b3ull;
      char t[24];
      snprintf(t, sizeof t, "%016" PRIx64 " ", h);
      out += t;
    }
    return true;
  }
  if (cmd == "soap") {
    MRI *src = get_vol();
    MRI *ctrl = get_vol();
    MRI *dst = get_dst(geti(), src);
    int niter = geti();
    float mc = getf();
    put_result(MRIsoapBubble(src, ctrl, dst, niter, mc), dst);
    return true;
  }
  // mriflood.cpp: flood <src> <dst mode> [<dst>]; acc <src> <dst mode> [<dst>] lo hi; dist x y z
  if (cmd == "dist") {
    float x = getf(), y = getf(), z = getf();
    of(distance(x, y, z));
    return true;
  }
  if (cmd == "flood" || cmd == "acc") {
    MRI *src = get_vol();
    MRI *dst = get_dst(geti(), src);
    MRI *r;
    if (cmd == "flood") {
      r = MRISfloodoutside(src, dst);
    } else {
      int lo = geti(), hi = geti();
      r = MRISaccentuate(src, dst, lo, hi);
    }
    put_result(r, dst);
    return true;
  }
  if (cmd == "salloc") {
    int ms = geti(), mv = geti();
    MRI_SEGMENTATION *sg = MRIsegmentAlloc(ms, mv);
    put_seg(sg);
    return true;
  }
  if (cmd != "seg" && cmd != "mseg") return false;
  MRI *vol = get_segvol();
  float lo = getf(), hi = getf();
  int nops = geti();
  MRI_SEGMENTATION *sg = cmd == "seg" ? MRIsegment(vol, lo, hi) : MRImaxsegment(vol, lo, hi);
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    out += op + " ";
    if (op == "D") {
      MRI *m = get_vol();
      oi(MRIsegmentDilate(sg, m));
    } else if (op == "T") {
      MRI *b = get_vol();
      MRI *t = get_vol();
      int l = geti(), h = geti();
      oi(MRIsegmentDilateThreshold(sg, b, t, l, h));
    } else if (op == "R") {
      oi(MRIremoveSmallSegments(sg, geti()));
    } else if (op == "C") {
      oi(MRIcompactSegments(sg));
    } else if (op == "I") {
      int s = geti();
      MRI *dst = get_opt();
      put_result(MRIsegmentToImage(vol, dst, sg, s), dst);
    } else if (op == "V") {
      int s = geti();
      MRI *m = get_vol();
      float val = getf();
      oi(MRIsetSegmentValue(m, sg, s, val));
      put_vol(m);
    } else if (op == "F") {
      int s = geti();
      MRI *dst = get_opt();
      float f = getf();
      put_result(MRIsegmentFill(sg, s, dst, f), dst);
    } else if (op == "M") {
      oi(MRIsegmentMax(sg));
    } else if (op == "N") {
      oi(MRIfindMaxSegmentNumber(sg));
    } else if (op == "A") {
      oi(MRImaxSegmentArea(sg));
    } else if (op == "L") {
      oi(MRIsegmentClearIgnoreFlags(sg));
    } else if (op == "G") {
      int s = geti();
      if (s >= 0 && s < sg->max_segments) sg->segments[s].ignore = 1;
    } else if (op == "E") {
      int mn = geti();
      MRI *m = get_vol();
      oi(MRIeraseSmallSegments(sg, m, mn));
      put_vol(m);
    } else {
      fail("unknown segment operation");
    }
  }
  put_seg(sg);
  return true;
}

std::string slurp_file(const std::string &path)
{
  std::string r;
  if (FILE *f = fopen(path.c_str(), "rb")) {
    int ch;
    while ((ch = fgetc(f)) != EOF) r += (char)ch;
    fclose(f);
  }
  return r;
}

// ---- relay 29: mrinorm.cpp's normalisation ------------------------------------------------------

// control points: n, then n voxel triples; MRI3dUseLabelControlPoints of a label in voxel
// coordinates (mrinorm.cpp's file-scope points)
void set_pts(MRI *mri)
{
  int n = geti();
  LABEL *l = LabelAlloc(n > 0 ? n : 1, NULL, NULL);
  l->coords = LABEL_COORDS_VOXEL;
  l->n_points = n;
  for (int i = 0; i < n; i++) {
    l->lv[i].x = geti();
    l->lv[i].y = geti();
    l->lv[i].z = geti();
  }
  MRI3dUseLabelControlPoints(mri, l);
}

std::string unhex(const char *h)
{
  std::string r;
  if (h[0] != 'h') fail("a hex token");
  for (const char *p = h + 1; p[0] && p[1]; p += 2) {
    char b[3] = {p[0], p[1], 0};
    r += (char)strtol(b, NULL, 16);
  }
  return r;
}

bool norm2_case(const std::string &cmd)
{
  if (cmd == "spl") {
    // spl <src> <dst mode> [<dst>] field ran_mode ran npoints <inputs> <outputs>
    MRI *src = get_vol();
    MRI *dst = get_dst(geti(), src);
    int fm = geti();
    int rm = geti();
    const char *rs = next();
    if (rm) setenv("RAN", rs, 1);
    setRandomSeed(1234);
    int n = geti();
    std::vector<float> in(n > 80 ? n : 80, 0.0f), op(n > 80 ? n : 80, 0.0f);
    for (int i = 0; i < n; i++) in[i] = getf();
    for (int i = 0; i < n; i++) op[i] = getf();
    MRI *field = NULL;
    MRI *r = MRIsplineNormalize(src, dst, fm ? &field : NULL, in.data(), op.data(), n);
    put_result(r, dst);
    if (fm) put_full(field);
  } else if (cmd == "ninit") {
    // ninit <volume with centre> xform_mode [16 doubles: m[c][r]] noavg above below wsize desired sigma
    MRI *m = get_volc();
    int xm = geti();
    if (xm) {
      Transform *t = (Transform *)calloc(1, sizeof(Transform));
      for (int c = 0; c < 4; c++)
        for (int r = 0; r < 4; r++) t->m[c][r] = getd();
      Transform *ti = (Transform *)calloc(1, sizeof(Transform));
      if (xm == 1) m->linear_transform = t;
      m->inverse_linear_transform = ti;
    }
    if (geti()) setenv("NO_AVERAGE305", "1", 1);
    int above = geti(), below = geti(), ws = geti(), des = geti();
    float sg = getf();
    MNI *mni = (MNI *)calloc(1, sizeof(MNI));
    oi(MRInormInit(m, mni, above, below, ws, des, sg));
    oi(mni->windows_above_t0); oi(mni->windows_below_t0); oi(mni->desired_wm_value); of(mni->smooth_sigma);
    int nw = mni->windows_above_t0 + mni->windows_below_t0;
    for (int i = 0; i < nw && i < MAX_SPLINE_POINTS; i++) {
      MRI_REGION *g = &mni->regions[i];
      oi(g->x); oi(g->y); oi(g->z); oi(g->dx); oi(g->dy); oi(g->dz);
    }
  } else if (cmd == "npk") {
    // npk mode above below desired sigma maxgrad npeaks {y dy nbins bin_size min max {bin count}*}*
    // <80 inputs> <80 outputs>: MRInormFindPeaks (mode 0) or MRInormCheckPeaks (mode 1)
    int mode = geti();
    MNI *mni = (MNI *)calloc(1, sizeof(MNI));
    mni->windows_above_t0 = geti();
    mni->windows_below_t0 = geti();
    mni->desired_wm_value = geti();
    mni->smooth_sigma = getf();
    mni->max_gradient = getf();
    int np = geti();
    int nw = mni->windows_above_t0 + mni->windows_below_t0;
    for (int i = 0; i < nw; i++) {
      mni->regions[i].y = geti();
      mni->regions[i].dy = geti();
      int nb = geti();
      HISTOGRAM *h = HISTOalloc(nb);
      h->bin_size = getf();
      h->min = getf();
      h->max = getf();
      for (int b = 0; b < nb; b++) {
        h->bins[b] = getf();
        h->counts[b] = getf();
      }
      mni->histograms[i] = *h;
    }
    float in[MAX_SPLINE_POINTS], op[MAX_SPLINE_POINTS];
    for (int i = 0; i < MAX_SPLINE_POINTS; i++) in[i] = getf();
    for (int i = 0; i < MAX_SPLINE_POINTS; i++) op[i] = getf();
    oi(mode == 0 ? MRInormFindPeaks(mni, in, op) : MRInormCheckPeaks(mni, in, op, np));
    for (int i = 0; i < MAX_SPLINE_POINTS; i++) of(in[i]);
    for (int i = 0; i < MAX_SPLINE_POINTS; i++) of(op[i]);
  } else if (cmd == "gfc") {
    // gfc <src> wm above below <ctrl?> <not control?> <points>
    MRI *src = get_vol();
    float wm = getf(), ab = getf(), be = getf();
    MRI *ctrl = get_opt();
    MRI *nc = get_opt();
    set_pts(src);
    put_result(MRInormGentlyFindControlPoints(src, wm, ab, be, ctrl, nc), ctrl);
  } else if (cmd == "fcw") {
    // fcw <src> wm above below <ctrl?> whalf_mm debug(0 NULL, 1 "", 2 "dbg") <not control?>
    MRI *src = get_vol();
    float wm = getf(), ab = getf(), be = getf();
    MRI *ctrl = get_opt();
    float wh = getf();
    int dm = geti();
    MRI *nc = get_opt();
    int n = -7;
    MRI *r = MRInormFindControlPointsInWindow(src, wm, ab, be, ctrl, wh, dm == 0 ? NULL : dm == 1 ? "" : "dbg",
                                              &n, 0, nc);
    oi(n);
    put_result(r, ctrl);
  } else if (cmd == "afc") {
    // afc <ctrl> value <mri?> <points>: MRInormAddFileControlPoints twice
    MRI *ctrl = get_vol();
    int value = geti();
    MRI *m = get_opt();
    set_pts(ctrl);
    oi(MRInormAddFileControlPoints(ctrl, value, m));
    oi(MRInormAddFileControlPoints(ctrl, value, m));
    put_full(ctrl);
  } else if (cmd == "bbi") {
    // bbi <src> <ctrl> sigma
    MRI *src = get_vol();
    MRI *ctrl = get_vol();
    float sg = getf();
    put_full(MRIbuildBiasImage(src, ctrl, NULL, sg));
    put_full(ctrl);
  } else if (cmd == "g3d") {
    // g3d <src> <bias?> wm <norm mode> [<norm>] above below only_file sigma <not control?> <points>
    MRI *src = get_vol();
    MRI *bias = get_opt();
    float wm = getf();
    MRI *norm = get_dst(geti(), src);
    float ab = getf(), be = getf();
    int of_ = geti();
    float sg = getf();
    MRI *nc = get_opt();
    set_pts(src);
    put_result(MRI3dGentleNormalize(src, bias, wm, norm, ab, be, of_, sg, nc), norm);
    put_full(src);
  } else if (cmd == "abc") {
    // abc <in> <bias> <out mode> [<out>] target
    MRI *in = get_vol();
    MRI *bias = get_vol();
    MRI *dst = get_dst(geti(), in);
    float tv = getf();
    put_result(MRIapplyBiasCorrectionSameGeometry(in, bias, dst, tv), dst);
  } else if (cmd == "g3df") {
    // g3df <volume file> above below sigma: MRI3dGentleNormalize of a recorded head's volume in
    // place (wm 110, no bias, no control-point file), as mri_normalize calls it; the result's
    // extent, type and the FNV-1a (64 bits) of its voxels
    MRI *src = MRIread(next());
    if (!src) fail("cannot read the volume");
    float ab = getf(), be = getf(), sg = getf();
    MRI *r = MRI3dGentleNormalize(src, NULL, 110, src, ab, be, 0, sg, NULL);
    oi(r == src); oi(r->width); oi(r->height); oi(r->depth); oi(r->type);
    uint64_t h = 0xcbf29ce484222325ull;
    const unsigned char *b = (const unsigned char *)r->chunk;
    for (size_t i = 0; i < r->bytes_total; i++) h = (h ^ b[i]) * 0x100000001b3ull;
    char t[24];
    snprintf(t, sizeof t, "%016" PRIx64 " ", h);
    out += t;
  } else if (cmd == "ufc") {
    // ufc <volume with centre> thresh_mode thresh <file bytes in hex>: the file written as cp.dat
    // in <cases>.out.d, MRI3dUseFileControlPoints, then MRInormAddFileControlPoints(new byte volume,
    // 255, NULL), the volume, and control.edited.dat up to its "written by" line (hex) or NOFILE
    MRI *m = get_volc();
    int tm = geti();
    const char *ts = next();
    if (tm) setenv("FS_THRESH_CTRL", ts, 1);
    std::string bytes = unhex(next());
    std::string d = scratch + ".d";
    mkdir(d.c_str(), 0700);
    if (chdir(d.c_str()) != 0) fail("chdir");
    unlink("control.edited.dat");
    FILE *fp = fopen("cp.dat", "wb");
    fwrite(bytes.data(), 1, bytes.size(), fp);
    fclose(fp);
    oi(MRI3dUseFileControlPoints(m, "cp.dat"));
    MRI *c = MRIalloc(m->width, m->height, m->depth, MRI_UCHAR);
    oi(MRInormAddFileControlPoints(c, 255, NULL));
    put_full(c);
    std::string e = slurp_file("control.edited.dat");
    unlink("cp.dat");
    if (e.empty()) {
      out += "NOFILE ";
    } else {
      size_t w = e.find("written by");
      if (w != std::string::npos) e = e.substr(0, w);
      out += "h";
      char b[4];
      for (unsigned char ch : e) {
        snprintf(b, sizeof b, "%02x", ch);
        out += b;
      }
      out += " ";
      unlink("control.edited.dat");
    }
  } else {
    return false;
  }
  return true;
}

void run_case()
{
  std::string cmd = next();
  if (filter_case(cmd) || fmri_case(cmd) || clus_case(cmd) || seg_case(cmd) || norm2_case(cmd)) return;
  if (cmd == "edm" || cmd == "dtr") {
    // edm|dtr <src> <dst?> label max_distance mode <mask?>
    MRI *src = get_vol();
    MRI *dst = get_opt();
    int label = geti();
    float maxd = getf();
    int mode = geti();
    MRI *mask = get_opt();
    MRI *r = cmd == "edm" ? MRIextractDistanceMap(src, dst, label, maxd, mode, mask)
                          : MRIdistanceTransform(src, dst, label, maxd, mode, mask);
    oi(r == dst);
    put_vol(r);
  } else if (cmd == "edmf" || cmd == "dtrf") {
    // edmf|dtrf <volume file> label max_distance mode: a recorded head's volume (MRIread), the
    // result's extent, type, frames, outside_val, xsize and the FNV-1a (64 bits) of its voxels
    MRI *src = MRIread(next());
    if (!src) fail("cannot read the volume");
    int label = geti();
    float maxd = getf();
    int mode = geti();
    MRI *r = cmd == "edmf" ? MRIextractDistanceMap(src, NULL, label, maxd, mode, NULL)
                           : MRIdistanceTransform(src, NULL, label, maxd, mode, NULL);
    oi(r->width); oi(r->height); oi(r->depth); oi(r->type); oi(r->nframes);
    od(r->outside_val);
    of(r->xsize);
    uint64_t h = 0xcbf29ce484222325ull;
    const unsigned char *b = (const unsigned char *)r->chunk;
    for (size_t i = 0; i < r->bytes_total; i++) h = (h ^ b[i]) * 0x100000001b3ull;
    char t[24];
    snprintf(t, sizeof t, "%016" PRIx64 " ", h);
    out += t;
  } else {
    fail("unknown command");
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

// the recorded heads' cases run for minutes
bool toks_hint_long(const std::string &line)
{
  return line.compare(0, 5, "edmf ") == 0 || line.compare(0, 5, "dtrf ") == 0 || line.compare(0, 4, "vsr ") == 0 ||
         line.compare(0, 5, "g3df ") == 0;
}

}  // namespace

int main(int argc, char *argv[])
{
  if (argc != 2) {
    fprintf(stderr, "usage: fsfilter <cases>\n");
    return 2;
  }
  if (argv[1][0] != '/') fail("the cases must be given by an absolute path");
  scratch = std::string(argv[1]) + ".out";
  FILE *fp = fopen(argv[1], "r");
  if (!fp) fail("cannot open the cases");
  std::vector<std::string> lines;
  static char buf[1 << 22];
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
      alarm(toks_hint_long(line) ? 900 : 20);
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
