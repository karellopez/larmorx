// SPDX-License-Identifier: Apache-2.0
// fs.volume's probe: FreeSurfer 8.2.0's volume functions (transform.cpp's VOL_GEOM text block,
// copies and matrices; mri.cpp's voxel arithmetic, coordinates and geometry; cma.cpp) called case
// by case, linked with the unit oracle's libutils as the official programs are (build.sh). CLOSED
// TO FREESURFER CLEAN-ROOM IMPLEMENTERS: it names FreeSurfer's functions (running it is a
// black-box run of FreeSurfer's code).
//
//     fsvolume <cases>     one output line per case line (absolute path; see check.py)
//
// Every case runs in a child process whose standard output and error go to a scratch file beside
// the cases; the parent prints `<result> | <text>`, the text C-escaped (\xHH for bytes outside
// printable ASCII and for the backslash), or `EXIT <status> | <text>` / `CRASH <signal> | <text>`
// when FreeSurfer's code ended the child. Floats and doubles in and out are their bits in hex;
// byte strings (files) are hex, `-` for none.

#include <fcntl.h>
#include <signal.h>
#include <sys/wait.h>
#include <unistd.h>

#include <sys/stat.h>

#include <cerrno>
#include <cinttypes>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>

#include "cma.h"
#include "colortab.h"
#include "diag.h"
#include "error.h"
#include "histo.h"
#include "image.h"
#include "mri_circulars.h"
#include "matrix.h"
#include "mri.h"
#include "mri2.h"
#include "mri_conform.h"
#include "region.h"
#include "talairachex.h"
#include "transform.h"
#include "utils.h"

extern double vg_isEqual_Threshold;
extern double MRIsampleVolumeDerivativeSign;
// defined in mri.cpp, not declared in its headers
double ham_sinc(double x, double fullwidth);
int MRIcubicSampleVolumeFrame(MRI *mri, double x, double y, double z, int frame, double *pval);
int MRIsincSampleVolumeFrame(MRI *mri, double x, double y, double z, int frame, int hw, double *pval);
int MRIsampleSeqVolume(const MRI *mri, double x, double y, double z, float *valvect, int firstframe, int lastframe);

const char *Progname = "fsvolume";

namespace {

std::vector<std::string> toks;
size_t pos;
std::string out;
std::string scratch;

[[noreturn]] void fail(const char *what)
{
  fprintf(stderr, "fsvolume: %s\n", what);
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
std::string gethex()
{
  std::string h = next(), r;
  if (h == "-") return r;
  for (size_t i = 0; i + 1 < h.size(); i += 2) r += (char)strtoul(h.substr(i, 2).c_str(), NULL, 16);
  return r;
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
void os(const char *s) { out += (s && *s) ? std::string(s) : std::string("-"); out += " "; }
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

// the geometry fields, in the order the cases give them
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
// the fields the VOL_GEOM functions here read or write (not the cached matrices)
// with_valid: an MRI's `valid` is never set by its constructors (memory contents): not printed
void put_geom(const VOL_GEOM *v, bool with_valid = true)
{
  if (with_valid) oi(v->valid);
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


// the float voxels the cases draw from (FTAB in tests/fs_volume/cases.rs)
const uint32_t FTAB[] = {
  0x00000000u, 0x80000000u, 0x3f800000u, 0xbf800000u, 0x3f000000u, 0x3fc00000u, 0x40200000u,
  0xc0200000u, 0x437f0000u, 0x437f8000u, 0x43800000u, 0xc3962000u, 0x477fff9au, 0x501502f9u,
  0xd01502f9u, 0x7f7fc99eu, 0x000116c2u, 0x7f800000u, 0xff800000u, 0x7fc00000u, 0x7fa12345u,
  0xffc00001u, 0x40490fd0u, 0x24e69595u, 0x42fecccdu, 0x3dcccccdu, 0x41200000u, 0xc1a00000u};
const size_t NF = sizeof FTAB / sizeof FTAB[0];

// a volume of the case: w h d type nframes hdr seed, then its geometry, its header fields and
// what to do with its cached matrices (0 keep, 1 MRIreInitCache, 2 drop them)
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
  // the geometry's fields but its extent (the volume's own)
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
  return m;
}

void oam(const AffineMatrix *am)
{
  if (!am) { out += "NULL "; return; }
  for (int i = 0; i < 16; i++) of(am->mat[i]);
}

// FNV-1a (64 bits) of bytes: the voxels of a conformed (256^3) volume
uint64_t fnv1a(const unsigned char *p, size_t n)
{
  uint64_t h = 0xcbf29ce484222325ull;
  for (size_t i = 0; i < n; i++) {
    h ^= p[i];
    h *= 0x100000001b3ull;
  }
  return h;
}

// the volume; with `hash`, its voxels' FNV-1a and byte count instead of their hex
void put_vol(const MRI *m, bool hash = false)
{
  if (!m) { out += "NULL "; return; }
  oi(m->width); oi(m->height); oi(m->depth); oi(m->nframes); oi(m->type);
  oi(m->imnr0); oi(m->imnr1);
  of(m->xstart); of(m->xend); of(m->ystart); of(m->yend); of(m->zstart); of(m->zend);
  of(m->thick); of(m->ps); of(m->tr); of(m->te); of(m->ti); od(m->flip_angle); oi(m->dof);
  put_geom(m, false);
  oam(m->i_to_r__);
  om(m->r_to_i__);
  if (m->chunk && m->ischunked) {
    if (hash) {
      char b[48];
      snprintf(b, sizeof b, "fnv %016" PRIx64 " %zu ", fnv1a((const unsigned char *)m->chunk, m->bytes_total),
               (size_t)m->bytes_total);
      out += b;
    } else {
      std::string b((const char *)m->chunk, m->bytes_total);
      ohex(b);
    }
  } else {
    out += "nochunk ";
  }
}

// an image of the case: rows cols format frames seed xsize ysize (ImageAlloc, then its pixels)
IMAGE *get_img()
{
  int rows = geti(), cols = geti(), format = geti(), nf = geti();
  unsigned seed = (unsigned)geti();
  IMAGE *I = ImageAlloc(rows, cols, format, nf);
  size_t n = (size_t)I->sizeimage * (size_t)I->num_frame;
  if (I->image) {
    unsigned char *b = I->image;
    if (format == PFFLOAT) {
      for (size_t i = 0; i < n / 4; i++) memcpy(b + 4 * i, &FTAB[(i * 7 + seed) % NF], 4);
    } else {
      for (size_t k = 0; k < n; k++) b[k] = (unsigned char)((k * 37u + seed * 101u + (k >> 3) * 13u) & 0xffu);
    }
  }
  I->xsize = getf();
  I->ysize = getf();
  return I;
}

void put_img(const IMAGE *I)
{
  if (!I) { out += "NULL "; return; }
  oi(I->rows); oi(I->cols); oi(I->orows); oi(I->ocols); oi(I->num_frame); oi(I->pixel_format);
  oi(I->sizepix); oi((long long)I->sizeimage); of(I->xsize); of(I->ysize);
  if (I->image) ohex(std::string((const char *)I->image, (size_t)I->sizeimage * (size_t)I->num_frame));
  else out += "nopix ";
}

// the destination of f(src, dst, ...): 0 NULL, 1 a volume of the case, 2 the source itself
MRI *get_dst(MRI *src)
{
  int mode = geti();
  if (mode == 1) return get_vol();
  if (mode == 2) return src;
  return NULL;
}
// of f(mri1, mri2, dst): 0 NULL, 1 a volume, 2 mri1, 3 mri2
MRI *get_dst2(MRI *a, MRI *b)
{
  int mode = geti();
  if (mode == 1) return get_vol();
  if (mode == 2) return a;
  if (mode == 3) return b;
  return NULL;
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
void spill(const std::string &path, const std::string &bytes)
{
  FILE *f = fopen(path.c_str(), "wb");
  if (!f) fail("cannot write the case's file");
  fwrite(bytes.data(), 1, bytes.size(), f);
  fclose(f);
}

// the IS_* macros of cma.h on a label, as 0/1 characters, in cma.h's order (cases.rs's MACROS)
template <typename T> std::string cma_bits(T v)
{
  std::string b;
  b += IS_FUTURE_WMSA(v) ? '1' : '0'; b += IS_LH_LAYER(v) ? '1' : '0'; b += IS_CC(v) ? '1' : '0'; b += IS_RH_LAYER(v) ? '1' : '0'; b += IS_LAYER(v) ? '1' : '0'; b += IS_UNKNOWN(v) ? '1' : '0'; b += IS_BRAIN(v) ? '1' : '0'; b += IS_WM(v) ? '1' : '0'; b += IS_CEREBRAL_WM(v) ? '1' : '0'; b += IS_HYPO(v) ? '1' : '0'; b += IS_WMSA(v) ? '1' : '0'; b += IS_WMH(v) ? '1' : '0'; b += IS_THALAMUS(v) ? '1' : '0'; b += IS_GM(v) ? '1' : '0'; b += IS_VENTRAL_DC(v) ? '1' : '0'; b += IS_CEREBELLAR_WM(v) ? '1' : '0'; b += IS_CEREBELLAR_GM(v) ? '1' : '0'; b += IS_CEREBELLUM(v) ? '1' : '0'; b += IS_CHOROID(v) ? '1' : '0'; b += IS_HIPPO(v) ? '1' : '0'; b += IS_AMYGDALA(v) ? '1' : '0'; b += IS_MTL(v) ? '1' : '0'; b += IS_CORTEX(v) ? '1' : '0'; b += IS_LAT_VENT(v) ? '1' : '0'; b += IS_CSF(v) ? '1' : '0'; b += IS_INF_LAT_VENT(v) ? '1' : '0'; b += IS_VENTRICLE(v) ? '1' : '0'; b += IS_CAUDATE(v) ? '1' : '0'; b += IS_PUTAMEN(v) ? '1' : '0'; b += IS_CLAUSTRUM(v) ? '1' : '0'; b += IS_ACCUMBENS(v) ? '1' : '0'; b += IS_PALLIDUM(v) ? '1' : '0'; b += LABEL_WITH_NO_TOPOLOGY_CONSTRAINT(v) ? '1' : '0'; b += IS_FIMBRIA(v) ? '1' : '0'; b += IS_GRAY_MATTER(v) ? '1' : '0'; b += IS_WHITE_MATTER(v) ? '1' : '0'; b += IS_FLUID(v) ? '1' : '0'; b += IS_LH_CLASS(v) ? '1' : '0'; b += IS_RH_CLASS(v) ? '1' : '0'; b += IS_GRAY_CLASS(v) ? '1' : '0'; b += IS_WHITE_CLASS(v) ? '1' : '0'; b += IS_CSF_CLASS(v) ? '1' : '0';
  for (int c : {CSF_CLASS, GM_CLASS, WM_CLASS, OTHER_CLASS}) b += IS_CLASS(v, c) ? '1' : '0';
  return b;
}

// voxels of frame 0 painted with values of the case: n, then x y z value
void paint(MRI *m)
{
  int n = geti();
  for (int i = 0; i < n; i++) {
    int x = geti(), y = geti(), z = geti();
    float v = getf();
    MRIsetVoxVal(m, x, y, z, 0, v);
  }
}

// frames filled, then voxels painted (MRIsetVoxVal): k, then the values frames 0 to k - 1 are
// filled with; n, then x y z frame value
void fill_paint(MRI *m)
{
  int k = geti();
  for (int f = 0; f < k; f++) {
    float v = getf();
    for (int x = 0; x < m->width; x++)
      for (int y = 0; y < m->height; y++)
        for (int z = 0; z < m->depth; z++) MRIsetVoxVal(m, x, y, z, f, v);
  }
  int n = geti();
  for (int i = 0; i < n; i++) {
    int x = geti(), y = geti(), z = geti(), f = geti();
    float v = getf();
    MRIsetVoxVal(m, x, y, z, f, v);
  }
}

// a label list: rows cols, then rows * cols numbers
MATRIX *get_list()
{
  int rows = geti(), cols = geti();
  MATRIX *m = MatrixAlloc(rows, cols, MATRIX_REAL);
  for (int r = 1; r <= rows; r++)
    for (int c = 1; c <= cols; c++) m->rptr[r][c] = getf();
  return m;
}

// a 4 x 4 matrix of the case or NULL: a flag, then the 16 elements row by row
MATRIX *get_m44()
{
  if (!geti()) return NULL;
  MATRIX *m = MatrixAlloc(4, 4, MATRIX_REAL);
  for (int r = 1; r <= 4; r++)
    for (int c = 1; c <= 4; c++) m->rptr[r][c] = getf();
  return m;
}

// an LTA's type, its first transform's type, matrix and geometries
void put_lta(const LTA *l)
{
  if (!l) { out += "NULL "; return; }
  oi(l->type);
  oi(l->xforms[0].type);
  om(l->xforms[0].m_L);
  put_geom(&l->xforms[0].src);
  put_geom(&l->xforms[0].dst);
}

// a histogram of the case or NULL: its allocation (-1: NULL), the bins in use, a seed for its
// old contents (relay 24)
HISTOGRAM *get_histo()
{
  int k = geti();
  if (k < 0) return NULL;
  int nb = geti(), seed = geti();
  HISTOGRAM *h = HISTOalloc(k);
  for (int i = 0; i < k; i++) {
    h->bins[i] = 0.5f * (float)i + (float)seed;
    h->counts[i] = (float)((i * 7 + seed) % 5);
  }
  h->nbins = nb;
  return h;
}

void put_histo(const HISTOGRAM *h)
{
  if (!h) { out += "NULL "; return; }
  oi(h->nbins); oi(h->max_bins); of(h->bin_size); of(h->min); of(h->max);
  int n = h->nbins < h->max_bins ? h->nbins : h->max_bins;
  for (int i = 0; i < n; i++) { of(h->bins[i]); of(h->counts[i]); }
}

// every frame's voxels from a pattern of two populations: on, seed, a, b, spread (on 0: nothing)
void fill_pat(MRI *m)
{
  if (!geti()) return;
  unsigned seed = (unsigned)geti();
  int a = geti(), b = geti(), sp = geti();
  for (int f = 0; f < m->nframes; f++)
    for (int z = 0; z < m->depth; z++)
      for (int y = 0; y < m->height; y++)
        for (int x = 0; x < m->width; x++) {
          unsigned h = ((unsigned)x * 73856093u) ^ ((unsigned)y * 19349663u) ^ ((unsigned)z * 83492791u) ^
                       ((unsigned)f * 2654435761u) ^ (seed * 97u);
          h = h * 1103515245u + 12345u;
          int v = (((h >> 8) & 1u) ? a : b) + (int)((h >> 9) % (unsigned)(2 * sp + 1)) - sp;
          MRIsetVoxVal(m, x, y, z, f, (float)v);
        }
}

void get_region(MRI_REGION *r)
{
  r->x = geti(); r->y = geti(); r->z = geti();
  r->dx = geti(); r->dy = geti(); r->dz = geti();
}

void run_case()
{
  std::string cmd = next();
  if (cmd == "vgwrite") {
    VOL_GEOM vg;
    set_geom(&vg);
    std::string path = scratch + ".vg";
    FILE *fp = fopen(path.c_str(), "w");
    writeVolGeom(fp, &vg);
    fclose(fp);
    ohex(slurp(path));
  } else if (cmd == "vgread") {
    VOL_GEOM vg;
    set_geom(&vg);
    std::string path = scratch + ".vg";
    spill(path, gethex());
    FILE *fp = fopen(path.c_str(), "r");
    errno = 0;
    readVolGeom(fp, &vg);
    int e = errno;
    put_geom(&vg);
    oi(ftell(fp));
    oi(feof(fp) != 0);
    oi(ferror(fp) != 0);
    oi(e);
    std::string rest;
    int ch;
    while (rest.size() < 64 && (ch = fgetc(fp)) != EOF) rest += (char)ch;
    ohex(rest);
    fclose(fp);
  } else if (cmd == "vgprint0") {
    VOL_GEOM vg;
    set_geom(&vg);
    vg_print(&vg);
    oi(0);
  } else if (cmd == "getvg" || cmd == "copyvg") {
    VOL_GEOM a, b;
    set_geom(&a);
    set_geom(&b);
    if (cmd == "getvg") getVolGeom(&a, &b);
    else copyVolGeom(&a, &b);
    put_geom(&b);
  } else if (cmd == "vgi2r") {
    VOL_GEOM vg;
    set_geom(&vg);
    om(vg_i_to_r(&vg));
  } else if (cmd == "vgr2i") {
    VOL_GEOM vg;
    set_geom(&vg);
    om(vg_r_to_i(&vg));
  } else if (cmd == "tkrv2r") {
    VOL_GEOM vg;
    set_geom(&vg);
    om(TkrVox2RASfromVolGeom(&vg));
  } else if (cmd == "vgeq0") {
    VOL_GEOM a, b;
    set_geom(&a);
    set_geom(&b);
    double thresh = getd();
    vg_isEqual_Threshold = getd();
    oi(vg_isNotEqualThresh(&a, &b, thresh));
    oi(vg_isEqual(&a, &b));
  } else if (cmd == "clone") {
    MRI *src = get_vol();
    MRI *dst = geti() ? get_vol() : NULL;
    MRI *r = MRIclone(src, dst);
    oi(dst != NULL && r == dst);
    put_vol(r);
  } else if (cmd == "clonesp") {
    MRI *src = get_vol();
    int t = geti(), nf = geti();
    put_vol(MRIcloneBySpace(src, t, nf));
  } else if (cmd == "clear") {
    MRI *m = get_vol();
    oi(MRIclear(m));
    put_vol(m);
  } else if (cmd == "match") {
    MRI *a = get_vol();
    MRI *b = get_vol();
    int w = geti(), h = geti(), d = geti(), nocheck = geti();
    oi(MRImatch(a, b));
    oi(MRImatchDimensions(a, b));
    oi(MRIcheckSize(a, nocheck ? NULL : b, w, h, d));
  } else if (cmd == "cpframe" || cmd == "cpframes") {
    MRI *src = get_vol();
    MRI *dst = get_dst(src);
    MRI *r;
    if (cmd == "cpframe") {
      int sf = geti(), df = geti();
      r = MRIcopyFrame(src, dst, sf, df);
    } else {
      int s0 = geti(), s1 = geti(), d0 = geti();
      r = MRIcopyFrames(src, dst, s0, s1, d0);
    }
    oi(r != NULL && r == dst);
    put_vol(r);
  } else if (cmd == "linscale" || cmd == "smul" || cmd == "smulf" || cmd == "thresh" || cmd == "uthresh" ||
             cmd == "invc" || cmd == "binz" || cmd == "abs" || cmd == "pos" || cmd == "neg") {
    MRI *src = get_vol();
    MRI *dst = get_dst(src);
    MRI *r = NULL;
    if (cmd == "linscale") {
      float sc = getf(), off = getf();
      int nz = geti();
      r = MRIlinearScale(src, dst, sc, off, nz);
    } else if (cmd == "smul") {
      r = MRIscalarMul(src, dst, getf());
    } else if (cmd == "smulf") {
      float sc = getf();
      r = MRIscalarMulFrame(src, dst, sc, geti());
    } else if (cmd == "thresh") {
      r = MRIthreshold(src, dst, getf());
    } else if (cmd == "uthresh") {
      r = MRIupperthresholdAllFrames(src, dst, getf());
    } else if (cmd == "invc") {
      r = MRIinvertContrast(src, dst, getf());
    } else if (cmd == "binz") {
      float t = getf(), lo = getf(), hi = getf();
      r = MRIbinarize(src, dst, t, lo, hi);
    } else if (cmd == "abs") {
      r = MRIabs(src, dst);
    } else if (cmd == "pos") {
      r = MRIpos(src, dst);
    } else {
      r = MRIneg(src, dst);
    }
    oi(r != NULL && r == dst);
    put_vol(r);
  } else if (cmd == "mul2" || cmd == "sub2" || cmd == "add2" || cmd == "div2" || cmd == "sam") {
    MRI *a = get_vol();
    MRI *b = get_vol();
    MRI *dst = get_dst2(a, b);
    MRI *r;
    if (cmd == "mul2") r = MRImultiply(a, b, dst);
    else if (cmd == "sub2") r = MRIsubtract(a, b, dst);
    else if (cmd == "add2") r = MRIadd(a, b, dst);
    else if (cmd == "div2") r = MRIdivide(a, b, dst);
    else r = MRIscaleAndMultiply(a, getf(), b, dst);
    oi(r != NULL && r == dst);
    put_vol(r);
  } else if (cmd == "vrange") {
    MRI *m = get_vol();
    float lo = -7, hi = -7;
    oi(MRIvalRange(m, &lo, &hi));
    of(lo); of(hi);
  } else if (cmd == "vrangef") {
    MRI *m = get_vol();
    float lo = -7, hi = -7;
    oi(MRIvalRangeFrame(m, &lo, &hi, geti()));
    of(lo); of(hi);
  } else if (cmd == "vrangereg") {
    MRI *m = get_vol();
    MRI_REGION r;
    r.x = geti(); r.y = geti(); r.z = geti(); r.dx = geti(); r.dy = geti(); r.dz = geti();
    float lo = -7, hi = -7;
    oi(MRIvalRangeRegion(m, &lo, &hi, &r));
    of(lo); of(hi);
  } else if (cmd == "lrange") {
    MRI *m = get_vol();
    MRI *l = get_vol();
    float lo = -7, hi = -7;
    oi(MRIlabelValRange(m, l, geti(), &lo, &hi));
    of(lo); of(hi);
  } else if (cmd == "extract") {
    MRI *src = get_vol();
    MRI *dst = geti() ? get_vol() : NULL;
    int x0 = geti(), y0 = geti(), z0 = geti(), dx = geti(), dy = geti(), dz = geti();
    int x1 = geti(), y1 = geti(), z1 = geti();
    MRI *r = MRIextractInto(src, dst, x0, y0, z0, dx, dy, dz, x1, y1, z1);
    oi(r != NULL && r == dst);
    put_vol(r);
    oam(src->i_to_r__);
  } else if (cmd == "cras") {
    MRI *src = get_vol();
    MRI *dst = get_vol();
    int x0 = geti(), y0 = geti(), z0 = geti(), x1 = geti(), y1 = geti(), z1 = geti();
    double r, a, s;
    MRIcalcCRASforExtractedVolume(src, dst, x0, y0, z0, x1, y1, z1, &r, &a, &s);
    od(r); od(a); od(s);
    oam(src->i_to_r__);
    om(src->r_to_i__);
  } else if (cmd == "avg") {
    MRI *src = get_vol();
    MRI *dst = get_dst(src);
    int dof = geti();
    MRI *r = MRIaverage(src, dof, dst);
    oi(r != NULL && r == dst);
    put_vol(r);
  } else if (cmd == "dsign") {
    od(MRIsetSampleVolumeDerivativeSign(getd()));
  } else if (cmd == "sras") {
    MRI *m = get_vol();
    int k = geti();
    MATRIX *r;
    if (k == 0) r = surfaceRASFromVoxel_(m);
    else if (k == 1) r = voxelFromSurfaceRAS_(m);
    else if (k == 2) r = surfaceRASFromRAS_(m);
    else r = GetSurfaceRASToVoxelMatrix(m);
    om(r);
    oam(m->i_to_r__);
    om(m->r_to_i__);
  } else if (cmd == "spt") {
    MRI *m = get_vol();
    int k = geti();
    double x = getd(), y = getd(), z = getd(), a = -7, b = -7, c = -7;
    int r;
    if (k == 0) r = MRIRASToSurfaceRAS(m, x, y, z, &a, &b, &c);
    else if (k == 1) r = MRIvoxelToSurfaceRAS(m, x, y, z, &a, &b, &c);
    else r = MRIsurfaceRASToVoxel(m, x, y, z, &a, &b, &c);
    oi(r); od(a); od(b); od(c);
    oam(m->i_to_r__);
    om(m->r_to_i__);
  } else if (cmd == "ostr") {
    MRI *m = get_vol();
    char o[4] = {'x', 'x', 'x', 'x'};
    oi(MRIdircosToOrientationString(m, o));
    ohex(std::string(o, 4));
    os(MRIsliceDirectionName(m));
    oi(getSliceDirection(m));
  } else if (cmd == "vtal") {
    MRI *m = get_vol();
    if (geti()) {
      m->linear_transform = (Transform *)calloc(1, sizeof(Transform));
      for (int i = 0; i < 4; i++)
        for (int j = 0; j < 4; j++) Transform_elem(*m->linear_transform, i, j) = getd();
    }
    double x = getd(), y = getd(), z = getd(), a = -7, b = -7, c = -7;
    oi(MRIvoxelToTalairach(m, x, y, z, &a, &b, &c));
    od(a); od(b); od(c);
    oam(m->i_to_r__);
    om(m->r_to_i__);
  } else if (cmd == "com" || cmd == "pca") {
    MRI *m = get_vol();
    BUFTYPE th = (BUFTYPE)geti();
    double means[3] = {-7, -7, -7};
    if (cmd == "com") {
      oi(MRIcenterOfMass(m, means, th));
      od(means[0]); od(means[1]); od(means[2]);
    } else {
      MATRIX *ev = MatrixAlloc(3, 3, MATRIX_REAL);
      float evalues[3] = {-7, -7, -7};
      oi(MRIprincipleComponents(m, ev, evalues, means, th));
      od(means[0]); od(means[1]); od(means[2]);
      of(evalues[0]); of(evalues[1]); of(evalues[2]);
      om(ev);
    }
  } else if (cmd == "cmph") {
    MRI *a = get_vol();
    a->fov = getf(); a->location = getf(); a->ptype = geti();
    int mode = geti();
    MRI *b = NULL;
    if (mode == 1) {
      b = get_vol();
      b->fov = getf(); b->location = getf(); b->ptype = geti();
    } else if (mode == 2) {
      b = a;
    }
    oi(MRIcompareHeaders(a, b));
    oi(MRIcompareHeaders(b, a));
  } else if (cmd == "dump") {
    MRI *m = get_vol();
    m->fov = getf();
    m->brightness = geti();
    strcpy(m->subject_name, gets().c_str());
    strcpy(m->path_to_t1, gets().c_str());
    strcpy(m->fname_format, gets().c_str());
    if (geti()) {
      m->register_mat = MatrixAlloc(4, 4, MATRIX_REAL);
      for (int i = 1; i <= 4; i++)
        for (int j = 1; j <= 4; j++) m->register_mat->rptr[i][j] = getf();
    }
    fflush(stdout);
    oi(MRIdump(m, stdout));
    fflush(stdout);
  } else if (cmd == "stri" || cmd == "styp" || cmd == "sfrm" || cmd == "sfty" || cmd == "scub" ||
             cmd == "scubf" || cmd == "ssinc" || cmd == "ssincf") {
    MRI *m = get_vol();
    m->outside_val = getd();
    int n = geti();
    for (int i = 0; i < n; i++) {
      double x = getd(), y = getd(), z = getd();
      int frame = geti(), aux = geti();
      double val = -7;
      int r;
      if (cmd == "stri") r = MRIsampleVolume(m, x, y, z, &val);
      else if (cmd == "styp") r = MRIsampleVolumeType(m, x, y, z, &val, aux);
      else if (cmd == "sfrm") r = MRIsampleVolumeFrame(m, x, y, z, frame, &val);
      else if (cmd == "sfty") r = MRIsampleVolumeFrameType(m, x, y, z, frame, aux, &val);
      else if (cmd == "scub") r = MRIcubicSampleVolume(m, x, y, z, &val);
      else if (cmd == "scubf") r = MRIcubicSampleVolumeFrame(m, x, y, z, frame, &val);
      else if (cmd == "ssinc") r = MRIsincSampleVolume(m, x, y, z, aux, &val);
      else r = MRIsincSampleVolumeFrame(m, x, y, z, frame, aux, &val);
      oi(r);
      od(val);
    }
  } else if (cmd == "trans" || cmd == "rot") {
    MRI *src = get_vol();
    MRI *dst = get_dst(src);
    MRI *r;
    if (cmd == "trans") {
      double dx = getd(), dy = getd(), dz = getd();
      r = MRItranslate(src, dst, dx, dy, dz);
    } else {
      MATRIX *mR = MatrixAlloc(3, 3, MATRIX_REAL);
      for (int i = 1; i <= 3; i++)
        for (int j = 1; j <= 3; j++) mR->rptr[i][j] = getf();
      MATRIX *mO = NULL;
      if (geti()) {
        mO = MatrixAlloc(3, 1, MATRIX_REAL);
        for (int i = 1; i <= 3; i++) mO->rptr[i][1] = getf();
      }
      r = MRIrotate_I(src, dst, mR, mO);
    }
    oi(r != NULL && r == dst);
    put_vol(r);
  } else if (cmd == "rotx" || cmd == "roty" || cmd == "rotz") {
    MRI *src = get_vol();
    MRI *dst = geti() ? get_vol() : NULL;
    float angle = getf();
    MRI *r;
    if (cmd == "rotx") r = MRIrotateX_I(src, dst, angle);
    else if (cmd == "roty") r = MRIrotateY_I(src, dst, angle);
    else r = MRIrotateZ_I(src, dst, angle);
    oi(r != NULL && r == dst);
    put_vol(r);
    put_vol(dst);
  } else if (cmd == "skull") {
    MRI *m = get_vol();
    int nb = geti();
    for (int b = 0; b < nb; b++) {
      int x0 = geti(), y0 = geti(), z0 = geti(), x1 = geti(), y1 = geti(), z1 = geti();
      float v = getf();
      for (int z = z0; z <= z1; z++)
        for (int y = y0; y <= y1; y++)
          for (int x = x0; x <= x1; x++) MRIsetVoxVal(m, x, y, z, 0, v);
    }
    int th = geti();
    MRI_REGION box;
    box.x = box.y = box.z = box.dx = box.dy = box.dz = -7;
    fflush(stdout);
    int r = MRIfindApproximateSkullBoundingBox(m, th, &box);
    fflush(stdout);
    oi(r);
    oi(box.x); oi(box.y); oi(box.z); oi(box.dx); oi(box.dy); oi(box.dz);
  } else if (cmd == "tftype") {
    int n = geti();
    for (int i = 0; i < n; i++) oi(TransformFileNameType(gethex().c_str()));
  } else if (cmd == "ltaread") {
    // a relative name in the scratch's folder (the messages name it)
    std::string dir = scratch.substr(0, scratch.rfind('/'));
    if (chdir(dir.c_str())) fail("chdir");
    std::string name = "fsvol-lta" + gethex();
    std::string bytes = gethex();
    int have = geti();
    int avg = geti();  // USE_AVERAGE305: 0 unset, 1 "true", 2 set and empty
    if (avg) setenv("USE_AVERAGE305", avg == 1 ? "true" : "", 1);
    else unsetenv("USE_AVERAGE305");
    unlink(name.c_str());
    if (have) spill(name, bytes);
    errno = 0;
    fflush(stdout);
    LTA *lta = LTAreadEx(name.c_str());
    fflush(stdout);
    oi(errno);
    if (!lta) {
      out += "NULL ";
    } else {
      oi(lta->type);
      oi(lta->num_xforms);
      for (int i = 0; i < lta->num_xforms; i++) {
        LINEAR_TRANSFORM *lt = &lta->xforms[i];
        of(lt->x0); of(lt->y0); of(lt->z0); of(lt->sigma);
        om(lt->m_L);
        oi(lt->type);
        put_geom(&lt->src);
        put_geom(&lt->dst);
      }
      ohex(std::string(lta->subject));
      of(lta->fscale);
    }
    if (have) unlink(name.c_str());
  } else if (cmd == "coef" || cmd == "hsinc") {
    int n = geti();
    for (int i = 0; i < n; i++) {
      double x = getd();
      if (cmd == "coef") od(MRIcubicCoeff(x, geti()));
      else od(ham_sinc(x, getd()));
    }
  } else if (cmd == "cmalab") {
    int n = geti();
    for (int i = 0; i < n; i++) {
      int v = geti();
      float f = getf();
      out += cma_bits(v) + " " + cma_bits(f) + " " + cma_bits((double)f) + " ";
      oi(IsSubCorticalGray(v));
      fflush(stdout);
      oi(MRIasegContraLatLabel(v));
      fflush(stdout);
    }
  } else if (cmd == "cmavol") {
    MRI *aseg = get_vol();
    paint(aseg);
    MRI *ribbon = get_vol();
    paint(ribbon);
    int hemi = geti();
    fflush(stdout);
    od(SupraTentorialVolCorrection(aseg, ribbon));
    od(CorticalGMVolCorrection(aseg, ribbon, hemi));
    fflush(stdout);
  } else if (cmd == "lrswap") {
    MRI *aseg = get_vol();
    paint(aseg);
    fflush(stdout);
    MRI *r = MRIlrswapAseg(aseg);
    fflush(stdout);
    put_vol(r);
  } else if (cmd == "fixrib") {
    MRI *aseg = get_vol();
    paint(aseg);
    MRI *ribbon = get_vol();
    paint(ribbon);
    MRI *dst = get_dst(aseg);
    MRI *r = MRIfixAsegWithRibbon(aseg, ribbon, dst);
    oi(r != NULL && r == dst);
    put_vol(r);
  } else if (cmd == "lrseg") {
    MRI *seg = get_vol();
    paint(seg);
    MATRIX *lh = get_list(), *rh = get_list();
    fflush(stdout);
    MRI *r = MRIlrswapSeg(seg, lh, rh);
    fflush(stdout);
    put_vol(r);
  } else if (cmd == "ohe") {
    MRI *seg = get_vol();
    paint(seg);
    int n = geti();
    std::vector<int> list;
    for (int i = 0; i < n; i++) list.push_back(geti());
    int force1 = geti();
    fflush(stdout);
    MRI *r = MRIoneHotEncode(seg, list, force1);
    fflush(stdout);
    put_vol(r);
  } else if (cmd == "bvc" || cmd == "bvr") {
    // the subject's folder beside the scratch file; its stats folder when the case says so
    std::string dir = scratch.substr(0, scratch.rfind('/'));
    if (chdir(dir.c_str())) fail("chdir");
    std::string subj = "fsvol-subj";
    unlink((subj + "/stats/brainvol.stats").c_str());
    rmdir((subj + "/stats").c_str());
    rmdir(subj.c_str());
    if (geti()) {
      mkdir(subj.c_str(), 0700);
      mkdir((subj + "/stats").c_str(), 0700);
    }
    if (cmd == "bvc") {
      std::vector<double> stats;
      for (int i = 0; i < 16; i++) stats.push_back(getd());
      fflush(stdout);
      CacheBrainVolumeStats(stats, subj, ".");
      ohex(slurp(subj + "/stats/brainvol.stats"));
    } else {
      spill(subj + "/stats/brainvol.stats", gethex());
      fflush(stdout);
      std::vector<double> stats = ReadCachedBrainVolumeStats(subj, ".");
      fflush(stdout);
      for (double v : stats) od(v);
    }
    unlink((subj + "/stats/brainvol.stats").c_str());
    rmdir((subj + "/stats").c_str());
    rmdir(subj.c_str());
  } else if (cmd == "setres") {
    MRI *m = get_vol();
    float a = getf(), b = getf(), c = getf();
    oi(MRIsetResolution(m, a, b, c));
    put_vol(m);
  } else if (cmd == "vfill") {
    MRI *m = get_vol();
    oi(MRIvalueFill(m, getf()));
    put_vol(m);
  } else if (cmd == "fillfg") {
    MRI *src = get_vol();
    MRI *dst = get_dst(src);
    int sx = geti(), sy = geti(), sz = geti(), th = geti(), fv = geti();
    int npix = -7;
    MRI *r = MRIfillFG(src, dst, sx, sy, sz, th, fv, &npix);
    oi(npix);
    oi(r != NULL && r == dst);
    put_vol(r);
  } else if (cmd == "explane") {
    MRI *src = get_vol();
    MRI *dst = geti() ? get_vol() : NULL;
    int o = geti(), w = geti();
    MRI *r = MRIextractPlane(src, dst, o, w);
    oi(r != NULL && r == dst);
    put_vol(r);
  } else if (cmd == "fillplane") {
    MRI *mask = get_vol();
    MRI *dst = get_vol();
    int o = geti(), w = geti(), fv = geti();
    MRI *r = MRIfillPlane(mask, dst, o, w, fv);
    oi(r == dst);
    put_vol(dst);
  } else if (cmd == "ups") {
    MRI *src = get_vol();
    MRI *dst = geti() ? get_vol() : NULL;
    int k = geti(), n = geti();
    fflush(stdout);
    MRI *r = k == 0 ? MRIupsampleN(src, dst, n) : k == 1 ? MRIupsample2(src, dst) : MRIupsampleNConserve(src, dst, n);
    fflush(stdout);
    oi(r != NULL && r == dst);
    put_vol(r);
  } else if (cmd == "dns2" || cmd == "dnsn") {
    MRI *src = get_vol();
    MRI *dst = geti() ? get_vol() : NULL;
    MRI *r;
    if (cmd == "dns2") {
      r = MRIdownsample2(src, dst);
    } else {
      float fc = getf(), fr = getf(), fs = getf();
      int keep = geti();
      fflush(stdout);
      r = MRIdownsampleN(src, dst, fc, fr, fs, keep);
      fflush(stdout);
    }
    oi(r != NULL && r == dst);
    put_vol(r);
  } else if (cmd == "mft") {
    MRI *m = get_vol();
    int f = geti();
    float th = getf();
    od(MRImeanFrameThresh(m, f, th));
  } else if (cmd == "lim") {
    MRI *m = get_vol();
    float a = getf(), b = getf();
    oi(MRIlimits(m, &a, &b));
    of(a);
    of(b);
  } else if (cmd == "chtype") {
    MRI *m = get_vol();
    int seq = geti(), dt = geti();
    float lo = getf(), hi = getf();
    int ns = geti();
    if (geti()) setenv("FS_FORCE_BIN_CHECK", "1", 1);
    else unsetenv("FS_FORCE_BIN_CHECK");
    fflush(stdout);
    MRI *r = seq ? MRISeqchangeType(m, dt, lo, hi, ns) : MRIchangeType(m, dt, lo, hi, ns);
    fflush(stdout);
    put_vol(r);
    put_vol(m);
  } else if (cmd == "splat") {
    MRI *m = get_vol();
    int n = geti();
    for (int i = 0; i < n; i++) {
      double x = getd(), y = getd(), z = getd();
      int frame = geti();
      double val = getd();
      oi(frame == 0 && i % 2 ? MRIinterpolateIntoVolume(m, x, y, z, val)
                             : MRIinterpolateIntoVolumeFrame(m, x, y, z, frame, val));
    }
    put_vol(m);
  } else if (cmd == "sseq" || cmd == "sxyz") {
    MRI *m = get_vol();
    m->outside_val = getd();
    int n = geti();
    for (int i = 0; i < n; i++) {
      int r;
      std::vector<float> v;
      if (cmd == "sseq") {
        double x = getd(), y = getd(), z = getd();
        int first = geti(), last = geti(), type = geti();
        v.assign(last + 1 > 0 ? last + 1 : 1, -7.0f);
        fflush(stdout);
        r = type < 0 ? MRIsampleSeqVolume(m, x, y, z, v.data(), first, last)
                     : MRIsampleSeqVolumeType(m, x, y, z, v.data(), first, last, type);
        fflush(stdout);
      } else {
        int x = geti(), y = geti(), z = geti(), fb = geti(), fe = geti(), k = geti();
        v.assign(fe - fb > 0 ? fe - fb : 1, -7.0f);
        r = k ? MRIsampleVolumeFrame_xyzInt_nRange_floats(m, x, y, z, fb, fe, v.data())
              : MRIsampleVolumeFrameType_xyzInt_nRange_SAMPLE_NEAREST_floats(m, x, y, z, fb, fe, v.data());
      }
      oi(r);
      for (float f : v) of(f);
    }
  } else if (cmd == "tkern") {
    MRI *m = get_vol();
    int n = geti();
    for (int i = 0; i < n; i++) {
      double x = getd(), y = getd(), z = getd();
      double k[8];
      for (int j = 0; j < 8; j++) k[j] = -7;
      double *r = MRItrilinKernel(m, x, y, z, k);
      oi(r == k);
      for (int j = 0; j < 8; j++) od(k[j]);
    }
  } else if (cmd == "deriv" || cmd == "grad") {
    MRI *m = get_vol();
    MRIsampleVolumeDerivativeSign = getd();
    int n = geti();
    for (int i = 0; i < n; i++) {
      double x = getd(), y = getd(), z = getd();
      int kind = geti();
      if (cmd == "deriv") {
        double dx = getd(), dy = getd(), dz = getd(), sigma = getd(), mag = -7;
        fflush(stdout);
        oi(kind ? MRIsampleVolumeDerivativeScale(m, x, y, z, dx, dy, dz, &mag, sigma)
                : MRIsampleVolumeDerivative(m, x, y, z, dx, dy, dz, &mag));
        fflush(stdout);
        od(mag);
      } else {
        int frame = geti();
        double a = -7, b = -7, c = -7;
        fflush(stdout);
        oi(kind ? MRIsampleVolumeGradientFrame(m, x, y, z, &a, &b, &c, frame)
                : MRIsampleVolumeGradient(m, x, y, z, &a, &b, &c));
        fflush(stdout);
        od(a); od(b); od(c);
      }
    }
  } else if (cmd == "nbrs") {
    MRI *m = get_vol();
    int n = geti();
    for (int i = 0; i < n; i++) {
      int kind = geti(), x = geti(), y = geti(), z = geti(), a = geti(), b = geti();
      float lo = getf(), hi = getf();
      int r;
      fflush(stdout);
      switch (kind) {
      case 0: r = MRIneighborsOn(m, x, y, z, a); break;
      case 1: r = MRIneighborsOn3x3(m, x, y, z, a); break;
      case 2: r = MRIneighborsInWindow(m, x, y, z, a, b); break;
      case 3: r = MRIneighbors(m, x, y, z, a); break;
      case 4: r = MRIneighborsInRange(m, x, y, z, a, lo, hi); break;
      case 5: r = MRIneighbors3x3(m, x, y, z, a); break;
      case 6: r = MRIneighborsOff(m, x, y, z, a); break;
      default: r = MRIneighborsOff3x3(m, x, y, z, a); break;
      }
      fflush(stdout);
      oi(r);
    }
  } else if (cmd == "lxf") {
    MRI *src = get_vol();
    src->outside_val = getd();
    MRI *dst = geti() ? get_vol() : NULL;
    int kind = geti(), interp = geti();
    MATRIX *A = NULL;
    if (geti()) {
      A = MatrixAlloc(4, 4, MATRIX_REAL);
      for (int i = 1; i <= 4; i++)
        for (int j = 1; j <= 4; j++) A->rptr[i][j] = getf();
    }
    fflush(stdout);
    MRI *r;
    switch (kind) {
    case 0: r = MRIlinearTransformInterp(src, dst, A, interp); break;
    case 1: r = MRIlinearTransform(src, dst, A); break;
    case 2: r = MRIinverseLinearTransform(src, dst, A); break;
    case 3: r = MRIapplyRASlinearTransformInterp(src, dst, A, interp); break;
    case 4: r = MRIapplyRASlinearTransform(src, dst, A); break;
    default: r = MRIapplyRASinverseLinearTransform(src, dst, A); break;
    }
    fflush(stdout);
    oi(r != NULL && r == dst);
    put_vol(r);
  } else if (cmd == "rsmat") {
    MRI *src = get_vol();
    MRI *tmpl = get_vol();
    if (geti()) Gdiag |= DIAG_SHOW | DIAG_VERBOSE;
    fflush(stdout);
    MATRIX *m = MRIgetResampleMatrix(src, tmpl);
    fflush(stdout);
    om(m);
  } else if (cmd == "rsfill") {
    MRI *src = get_vol();
    MRI *tmpl = get_vol();
    int type = geti();
    float fill = getf();
    int plain = geti();
    if (geti()) Gdiag |= DIAG_SHOW | DIAG_VERBOSE;
    fflush(stdout);
    MRI *r = plain ? MRIresample(src, tmpl, type) : MRIresampleFill(src, tmpl, type, fill);
    fflush(stdout);
    put_vol(r);
  } else if (cmd == "repval") {
    MRI *src = get_vol();
    MRI *dst = get_dst(src);
    MRI *r;
    if (geti()) {
      int a = geti(), b = geti();
      r = MRIreplaceValuesUchar(src, dst, (BUFTYPE)a, (BUFTYPE)b);
    } else {
      float a = getf(), b = getf();
      r = MRIreplaceValues(src, dst, a, b);
    }
    fflush(stdout);
    oi(r != NULL && r == dst);
    put_vol(r);
  } else if (cmd == "stats") {
    MRI *m = get_vol();
    if (geti()) {
      fflush(stdout);
      int r = MRIprintStats(m, stdout);
      fflush(stdout);
      oi(r);
    } else {
      float mn, mx, mean, sd;
      int n = -7;
      uint32_t u = 0x7fa5a5a5u;
      memcpy(&mn, &u, 4); memcpy(&mx, &u, 4); memcpy(&mean, &u, 4); memcpy(&sd, &u, 4);
      oi(MRIstats(m, &mn, &mx, &n, &mean, &sd));
      oi(n); of(mn); of(mx); of(mean); of(sd);
    }
  } else if (cmd == "type2s") {
    os(MRItype2str(geti()));
  } else if (cmd == "cropbb") {
    MRI *m = get_vol();
    MRI_REGION b;
    b.x = geti(); b.y = geti(); b.z = geti(); b.dx = geti(); b.dy = geti(); b.dz = geti();
    oi(MRIcropBoundingBox(m, &b));
    oi(b.x); oi(b.y); oi(b.z); oi(b.dx); oi(b.dy); oi(b.dz);
  } else if (cmd == "crass") {
    MRI *src = get_vol();
    MRI *dst = get_vol();
    if (geti()) Gdiag |= DIAG_SHOW | DIAG_VERBOSE;
    double r = 0, a = 0, ss = 0;
    MRIcalcCRASforSampledVolume(src, dst, &r, &a, &ss);
    fflush(stderr);
    od(r); od(a); od(ss);
    oam(src->i_to_r__);
    om(src->r_to_i__);
  } else if (cmd == "chkostr") {
    std::string o = gethex();
    char *e = MRIcheckOrientationString(o.c_str());
    if (e) { ohex(std::string(e)); free(e); }
    else out += "NULL ";
  } else if (cmd == "o2dc") {
    MRI *m = get_vol();
    std::string o = gethex();
    fflush(stdout);
    oi(MRIorientationStringToDircos(m, o.c_str()));
    fflush(stdout);
    put_geom(m, false);
  } else if (cmd == "s44") {
    MATRIX *m = NULL;
    int rows = geti(), cols = geti(), cplx = geti();
    if (rows > 0) m = MatrixAlloc(rows, cols, cplx ? MATRIX_COMPLEX : MATRIX_REAL);
    float v[16];
    for (int i = 0; i < 16; i++) v[i] = getf();
    oi(stuff_four_by_four(m, v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8], v[9], v[10], v[11], v[12], v[13], v[14], v[15]));
    if (m && cplx) {
      oi(m->rows); oi(m->cols);
      for (int r = 1; r <= m->rows; r++)
        for (int c = 1; c <= m->cols; c++) { of(MATRIX_CELT_REAL(m, r, c)); of(MATRIX_CELT_IMAG(m, r, c)); }
    } else {
      om(m);
    }
  } else if (cmd == "vx2rx") {
    MRI *src = get_vol();
    MRI *dst = geti() ? get_vol() : NULL;
    MATRIX *mv = geti() ? get_list() : NULL;
    MATRIX *mr = geti() ? get_list() : NULL;
    fflush(stdout);
    MATRIX *r = MRIvoxelXformToRasXform(src, dst, mv, mr);
    fflush(stdout);
    oi(r != NULL && r == mr);
    om(r);
  } else if (cmd == "v2v") {
    MRI *src = get_vol();
    MRI *dst = get_vol();
    fflush(stdout);
    MATRIX *r = MRIgetVoxelToVoxelXform(src, dst);
    fflush(stdout);
    om(r);
  } else if (cmd == "setv2r") {
    MRI *m = get_vol();
    int e = geti();
    std::string ev = gethex();
    if (e) setenv("FS_SetVoxToRasXform_Change_VoxSize", ev.c_str(), 1);
    MATRIX *x = get_list();
    fflush(stdout);
    errno = 0;
    oi(MRIsetVoxelToRasXform(m, x));
    fflush(stdout);
    oi(errno);
    put_vol(m);
  } else if (cmd == "toimgv") {
    MRI *m = get_vol();
    IMAGE *I = geti() ? get_img() : NULL;
    int slice = geti(), view = geti(), frame = geti();
    fflush(stdout);
    IMAGE *r = MRItoImageView(m, I, slice, view, frame);
    fflush(stdout);
    put_img(r);
  } else if (cmd == "toimg") {
    MRI *m = get_vol();
    IMAGE *I = geti() ? get_img() : NULL;
    int slice = geti();
    fflush(stdout);
    IMAGE *r = MRItoImage(m, I, slice);
    fflush(stdout);
    oi(r != NULL && r == I);
    put_img(r);
  } else if (cmd == "img2mri") {
    IMAGE *I = get_img();
    fflush(stdout);
    MRI *m = ImageToMRI(I);
    fflush(stdout);
    put_vol(m);
    of(I->xsize); of(I->ysize);
  } else if (cmd == "smpar") {
    MRI *m = get_vol();
    paint(m);
    int count = geti();
    fflush(stdout);
    MRI *r = MRIsmoothParcellation(m, count);
    fflush(stdout);
    put_vol(r);
  } else if (cmd == "erasebp") {
    MRI *m = get_vol();
    int b = geti();
    fflush(stdout);
    oi(MRIeraseBorderPlanes(m, b));
    fflush(stdout);
    put_vol(m);
  } else if (cmd == "nbhd") {
    MRI *m = get_vol();
    paint(m);
    int which = geti(), wsize = geti(), x = geti(), y = geti(), z = geti();
    int frame = geti();
    float thresh = getf();
    fflush(stdout);
    int r = 0;
    if (which == 0) r = MRImaxInNbhd(m, wsize, x, y, z, frame);
    else if (which == 1) r = MRIcountNonzeroInNbhd(m, wsize, x, y, z);
    else if (which == 2) r = MRIareNonzeroInNbhd(m, wsize, x, y, z);
    else r = MRIcountThreshInNbhd(m, wsize, x, y, z, thresh);
    fflush(stdout);
    oi(r);
  } else if (cmd == "nearnz") {
    MRI *m = get_vol();
    paint(m);
    int wsize = geti();
    double xr = getd(), yr = getd(), zr = getd();
    int want = geti();
    int xv = -9, yv = -9, zv = -9;
    fflush(stdout);
    float r = MRIfindNearestNonzeroLocation(m, wsize, xr, yr, zr, want ? &xv : NULL, want ? &yv : NULL, want ? &zv : NULL);
    fflush(stdout);
    of(r); oi(xv); oi(yv); oi(zv);
  } else if (cmd == "lmeans") {
    MRI *src = get_vol();
    MRI *lab = get_vol();
    paint(lab);
    int label = geti(), which = geti();
    int x0 = geti(), y0 = geti(), z0 = geti(), whalf = geti(), want = geti();
    double sd = -7.0;
    fflush(stdout);
    if (which == 0) {
      VECTOR *v = MRImeanInLabelMultispectral(src, lab, label);
      fflush(stdout);
      om(v);
    } else if (which == 1) {
      od(MRImeanInLabel(src, lab, label));
    } else if (which == 2) {
      od(MRImeanAndStdInLabel(src, lab, label, want ? &sd : NULL));
      od(sd);
    } else if (which == 3) {
      od(MRImeanInLabelInRegion(src, lab, label, x0, y0, z0, whalf, want ? &sd : NULL));
      od(sd);
    } else {
      od(MRImaxInLabelInRegion(src, lab, label, x0, y0, z0, whalf));
    }
  } else if (cmd == "mkpos") {
    MRI *src = get_vol();
    MRI *dst = geti() ? get_vol() : NULL;
    fflush(stdout);
    MRI *r = MRImakePositive(src, dst);
    fflush(stdout);
    oi(r != NULL && r == dst);
    put_vol(r);
  } else if (cmd == "mlbv") {
    MRI *src = get_vol();
    paint(src);
    MRI *dst = geti() ? get_vol() : NULL;
    int label = geti(), mark = geti(), six = geti();
    fflush(stdout);
    MRI *r = MRImarkLabelBorderVoxels(src, dst, label, mark, six);
    fflush(stdout);
    oi(r != NULL && r == dst);
    put_vol(r);
  } else if (cmd == "lnbhd") {
    MRI *lab = get_vol();
    paint(lab);
    MRI *vals = geti() ? get_vol() : NULL;
    int x = geti(), y = geti(), z = geti(), whalf = geti(), maxl = geti();
    std::vector<int> counts(maxl, -5);
    std::vector<float> means(maxl, -5.0f);
    fflush(stdout);
    oi(MRIcomputeLabelNbhd(lab, vals, x, y, z, counts.data(), vals ? means.data() : NULL, whalf, maxl));
    fflush(stdout);
    for (int c : counts) oi(c);
    if (vals) for (float f : means) of(f);
  } else if (cmd == "linbhd") {
    MRI *m = get_vol();
    paint(m);
    int x = geti(), y = geti(), z = geti(), whalf = geti(), label = geti(), which = geti();
    fflush(stdout);
    int r = which < 0 ? MRIlabelsInNbhd(m, x, y, z, whalf, label) : MRIlabelsInPlanarNbhd(m, x, y, z, whalf, label, which);
    fflush(stdout);
    oi(r);
  } else if (cmd == "lbbox") {
    MRI *m = get_vol();
    paint(m);
    MRI_REGION b;
    oi(MRIlabelBoundingBox(m, geti(), &b));
    oi(b.x); oi(b.y); oi(b.z); oi(b.dx); oi(b.dy); oi(b.dz);
  } else if (cmd == "fillbox") {
    MRI *m = get_vol();
    MRI_REGION b;
    b.x = geti(); b.y = geti(); b.z = geti(); b.dx = geti(); b.dy = geti(); b.dz = geti();
    float v = getf();
    fflush(stdout);
    oi(MRIfillBox(m, &b, v));
    fflush(stdout);
    put_vol(m);
  } else if (cmd == "mskz") {
    MRI *src = get_vol();
    MRI *mask = get_vol();
    paint(mask);
    MRI *dst = get_dst(src);
    fflush(stdout);
    MRI *r = MRImaskZero(src, mask, dst);
    fflush(stdout);
    oi(r != NULL && r == dst);
    put_vol(r);
  } else if (cmd == "clipwm") {
    MRI *t1 = get_vol();
    paint(t1);
    MRI *wm = get_vol();
    paint(wm);
    fflush(stdout);
    oi(MRIclipBrightWM(t1, wm));
    fflush(stdout);
    put_vol(t1);
  } else if (cmd == "pvlab") {
    MRI *lab = get_vol();
    fill_paint(lab);
    MRI *vals = geti() ? get_vol() : NULL;
    if (vals) fill_paint(vals);
    int label = geti(), maxl = geti();
    MRI *mix = geti() ? get_vol() : NULL;
    MRI *nbr = geti() ? get_vol() : NULL;
    MRI *sb = geti() ? get_vol() : NULL;
    if (sb) fill_paint(sb);
    fflush(stdout);
    float r = MRIvoxelsInLabelWithPartialVolumeEffects(lab, vals, label, mix, nbr, maxl, sb);
    fflush(stdout);
    of(r);
    put_vol(mix);
    put_vol(nbr);
  } else if (cmd == "randn" || cmd == "const") {
    int dims[4];
    for (int &d : dims) d = geti();
    float a = getf(), b = getf();
    long seed = geti();
    MRI *m = geti() ? get_vol() : NULL;
    setRandomSeed(seed);
    fflush(stdout);
    MRI *r = cmd == "randn" ? MRIrandn(dims[0], dims[1], dims[2], dims[3], a, b, m)
                            : MRIconst(dims[0], dims[1], dims[2], dims[3], a, m);
    fflush(stdout);
    oi(r != NULL && r == m);
    put_vol(r);
  } else if (cmd == "nseq" || cmd == "nfr") {
    MRI *m = get_vol();
    fill_paint(m);
    int which = geti();
    float target = getf();
    fflush(stdout);
    int r = cmd == "nseq" ? MRInormalizeSequence(m, target)
            : which == 0  ? MRInormalizeFrames(m)
            : which == 1  ? MRInormalizeFramesMean(m)
                          : MRInormalizeFramesFirst(m);
    fflush(stdout);
    oi(r);
    put_vol(m);
  } else if (cmd == "sort") {
    MRI *in = get_vol();
    fill_paint(in);
    MRI *mask = geti() ? get_vol() : NULL;
    if (mask) fill_paint(mask);
    MRI *s = geti() ? get_vol() : NULL;
    fflush(stdout);
    MRI *r = MRIsort(in, mask, s);
    fflush(stdout);
    oi(r != NULL && r == s);
    put_vol(r);
  } else if (cmd == "revsl") {
    MRI *in = get_vol();
    fill_paint(in);
    if (geti()) in->bvals = get_list();
    if (geti()) in->bvecs = get_list();
    in->bvec_space = geti();
    MRI *o = geti() ? get_vol() : NULL;
    fflush(stdout);
    MRI *r = MRIreverseSliceOrder(in, o);
    fflush(stdout);
    oi(r != NULL && r == o);
    put_vol(r);
    if (r) {
      om(r->bvals);
      om(r->bvecs);
      oi(r->bvec_space);
    }
  } else if (cmd == "cdtype") {
    MRI *in = get_vol();
    int type = geti();
    fflush(stdout);
    MRI *r = MRIcloneDifferentType(in, type);
    fflush(stdout);
    put_vol(r);
  } else if (cmd == "nonmax") {
    MRI *in = get_vol();
    fill_paint(in);
    MRI *s = geti() ? get_vol() : NULL;
    float thresh = getf();
    int dir = geti();
    fflush(stdout);
    MRI *r = MRInonMaxSuppress(in, s, thresh, dir);
    fflush(stdout);
    oi(r != NULL && r == s);
    put_vol(r);
  } else if (cmd == "xrpad") {
    MRI *in = get_vol();
    fill_paint(in);
    MRI_REGION reg;
    int hr = geti();
    if (hr) {
      reg.x = geti(); reg.y = geti(); reg.z = geti();
      reg.dx = geti(); reg.dy = geti(); reg.dz = geti();
    }
    int pad = geti();
    fflush(stdout);
    MRI *r = MRIextractRegionAndPad(in, NULL, hr ? &reg : NULL, pad);
    fflush(stdout);
    put_vol(r);
  } else if (cmd == "ordidx" || cmd == "vperm") {
    MRI *in = get_vol();
    fill_paint(in);
    long seed = geti();
    int n = in->width * in->height * in->depth;
    std::vector<short> xs(n > 0 ? n : 0, -7), ys(n > 0 ? n : 0, -7), zs(n > 0 ? n : 0, -7);
    setRandomSeed(seed);
    fflush(stdout);
    int r = cmd == "ordidx" ? MRIorderIndices(in, xs.data(), ys.data(), zs.data())
                            : MRIcomputeVoxelPermutation(in, xs.data(), ys.data(), zs.data());
    fflush(stdout);
    oi(r);
    for (short v : xs) oi(v);
    for (short v : ys) oi(v);
    for (short v : zs) oi(v);
  } else if (cmd == "rms") {
    MRI *in = get_vol();
    fill_paint(in);
    MRI *o = get_vol();
    fflush(stdout);
    MRIrms(in, o);
    fflush(stdout);
    put_vol(o);
  } else if (cmd == "vfrac") {
    MRI *src = get_vol();
    MATRIX *m = get_list();
    MRI *seg = get_vol();
    fill_paint(seg);
    MRI *fr = get_vol();
    fill_paint(fr);
    fflush(stdout);
    oi(MRIcomputeVolumeFractions(src, m, seg, fr));
    fflush(stdout);
    put_vol(fr);
  } else if (cmd == "cdt") {
    MRI *a = get_vol();
    fill_paint(a);
    MRI *b = get_vol();
    fill_paint(b);
    MRI *d = geti() ? get_vol() : NULL;
    fflush(stdout);
    MRI *r = MRIcombineDistanceTransforms(a, b, d);
    fflush(stdout);
    oi(r != NULL && r == d);
    put_vol(r);
  } else if (cmd == "diffv") {
    MRI *a = get_vol();
    fill_paint(a);
    MRI *b = get_vol();
    fill_paint(b);
    MRI *mask = geti() ? get_vol() : NULL;
    if (mask) fill_paint(mask);
    double thresh = getd();
    MRI *amask = geti() ? get_vol() : NULL;
    if (amask) fill_paint(amask);
    int mode = geti();  // the application's destination: 0 new, 1 a volume, 2 in place
    MRI *dst = mode == 1 ? get_vol() : NULL;
    if (geti()) {
      setRandomSeed(1);  // CTABalloc draws random colours
      a->ct = CTABalloc(2);
    }
    int extra = geti();
    std::vector<std::vector<double>> more;
    for (int i = 0; i < extra; i++) {
      std::vector<double> v(5);
      for (double &d : v) d = getd();
      more.push_back(v);
    }
    fflush(stdout);
    std::vector<std::vector<double>> *dv = MRIdiff2Vect(a, b, thresh, mask);
    fflush(stdout);
    if (!dv) {
      out += "NULL ";
    } else {
      oi((long long)dv->size());
      for (auto &row : *dv)
        for (double d : row) od(d);
      for (auto &v : more) dv->push_back(v);
      int napply = -1;
      MRI *to = mode == 2 ? a : dst;
      fflush(stdout);
      MRI *r = MRIapplyDiffVect(a, amask, *dv, &napply, to);
      fflush(stdout);
      oi(napply);
      oi(r != NULL && r == to);
      put_vol(r);
    }
  } else if (cmd == "tiv") {
    std::string dir = scratch.substr(0, scratch.rfind('/'));
    if (chdir(dir.c_str())) fail("chdir");
    std::string name = "fsvol-lta" + gethex();
    std::string bytes = gethex();
    int have = geti();
    int avg = geti();
    if (avg) setenv("USE_AVERAGE305", avg == 1 ? "true" : "", 1);
    else unsetenv("USE_AVERAGE305");
    double scale = getd();
    unlink(name.c_str());
    if (have) spill(name, bytes);
    double det = -7.0;
    errno = 0;
    fflush(stdout);
    double icv = MRIestimateTIV((char *)name.c_str(), scale, &det);
    fflush(stdout);
    od(icv);
    od(det);
  } else if (cmd == "fpow") {
    MRI *m = get_vol();
    fill_paint(m);
    int n = geti();
    std::vector<float> p(n > 0 ? n : 1, 1.0f);
    for (int i = 0; i < n; i++) p[i] = getf();
    fflush(stdout);
    oi(mri_framepower(m, p.data()));
    fflush(stdout);
    put_vol(m);
  } else if (cmd == "mbin") {
    MRI *v = get_vol();
    fill_paint(v);
    int mode = geti();
    MRI *d = mode == 1 ? get_vol() : mode == 2 ? v : NULL;
    float th = getf();
    int ht = geti();
    std::string tail = ht ? gethex() : "";
    int inv = geti();
    int nover = -9;
    fflush(stdout);
    MRI *r = mri_binarize(v, th, ht ? tail.c_str() : NULL, inv, d, &nover);
    fflush(stdout);
    oi(nover);
    oi(r != NULL && r == d);
    put_vol(r);
  } else if (cmd == "reshape") {
    MRI *v = get_vol();
    fill_paint(v);
    int a = geti(), b = geti(), c = geti(), d = geti();
    fflush(stdout);
    MRI *r = mri_reshape(v, a, b, c, d);
    fflush(stdout);
    put_vol(r);
    oi(v->nframes);
  } else if (cmd == "mmax") {
    MRI *a = get_vol();
    fill_paint(a);
    MRI *b = get_vol();
    fill_paint(b);
    MRI *d = geti() ? get_vol() : NULL;
    fflush(stdout);
    MRI *r = MRImax(a, b, d);
    fflush(stdout);
    oi(r != NULL && r == d);
    put_vol(r);
  } else if (cmd == "mconst") {
    MRI *a = get_vol();
    fill_paint(a);
    double k = getd();
    int mul = geti();
    MRI *d = geti() ? get_vol() : NULL;
    fflush(stdout);
    MRI *r = mul ? MRImultiplyConst(a, k, d) : MRIaddConst(a, k, d);
    fflush(stdout);
    oi(r != NULL && r == d);
    put_vol(r);
  } else if (cmd == "fbin") {
    MRI *a = get_vol();
    fill_paint(a);
    double th = getd();
    MRI *m = geti() ? get_vol() : NULL;
    if (m) fill_paint(m);
    fflush(stdout);
    MRI *r = MRIframeBinarize(a, th, m);
    fflush(stdout);
    oi(r != NULL && r == m);
    put_vol(r);
  } else if (cmd == "vote") {
    MRI *a = get_vol();
    fill_paint(a);
    MRI *m = geti() ? get_vol() : NULL;
    if (m) fill_paint(m);
    MRI *v = geti() ? get_vol() : NULL;
    int ex = geti();
    fflush(stdout);
    MRI *r = MRIvote(a, m, v, ex);
    fflush(stdout);
    oi(r != NULL && r == v);
    put_vol(r);
    put_vol(a);
  } else if (cmd == "msq") {
    MRI *a = get_vol();
    fill_paint(a);
    int which = geti();
    MRI *m = geti() ? get_vol() : NULL;
    if (m) fill_paint(m);
    MRI *d = geti() ? get_vol() : NULL;
    fflush(stdout);
    MRI *r = which == 0 ? MRIsquare(a, m, d) : which == 1 ? MRIsquareRoot(a, m, d) : MRIsqrt(a, d);
    fflush(stdout);
    oi(r != NULL && r == d);
    put_vol(r);
  } else if (cmd == "crop") {
    MRI *a = get_vol();
    fill_paint(a);
    int p[6];
    for (int &x : p) x = geti();
    fflush(stdout);
    MRI *r = MRIcrop(a, p[0], p[1], p[2], p[3], p[4], p[5]);
    fflush(stdout);
    put_vol(r);
  } else if (cmd == "revs") {
    MRI *a = get_vol();
    fill_paint(a);
    MRI *d = geti() ? get_vol() : NULL;
    fflush(stdout);
    MRI *r = MRIreverseSlices(a, d);
    fflush(stdout);
    oi(r != NULL && r == d);
    put_vol(r);
  } else if (cmd == "cutend") {
    MRI *a = get_vol();
    fill_paint(a);
    int n = geti();
    fflush(stdout);
    MRI *r = MRIcutEndSlices(a, n);
    fflush(stdout);
    put_vol(r);
  } else if (cmd == "segids") {
    MRI *a = get_vol();
    fill_paint(a);
    int frame = geti(), not0 = geti();
    int n = -9;
    fflush(stdout);
    int *l = not0 ? MRIsegIdListNot0(a, &n, frame) : MRIsegIdList(a, &n, frame);
    fflush(stdout);
    oi(n);
    for (int i = 0; i < n; i++) oi(l[i]);
  } else if (cmd == "hcos") {
    MRI *a = get_vol();
    fill_paint(a);
    double al = getd();
    MRI *d = geti() ? get_vol() : NULL;
    fflush(stdout);
    MRI *r = MRIhalfCosBias(a, al, d);
    fflush(stdout);
    oi(r != NULL && r == d);
    put_vol(r);
  } else if (cmd == "vv" || cmd == "vvsm") {
    MRI *src = get_vol();
    fill_paint(src);
    src->outside_val = getd();
    MRI *targ = get_vol();
    fill_paint(targ);
    MATRIX *M = get_m44();
    int interp = geti();
    float param = getf();
    if (geti()) Gdiag_no = 1;
    if (geti()) Gdiag |= DIAG_VERBOSE;
    MRI *vsm = NULL;
    LTA *lta = NULL;
    int pedir = 0;
    if (cmd == "vvsm") {
      if (geti()) {
        vsm = get_vol();
        fill_paint(vsm);
        vsm->outside_val = getd();
      }
      if (geti()) {
        lta = LTAalloc(1, NULL);
        lta->type = lta->xforms[0].type = geti();
        for (int r = 1; r <= 4; r++)
          for (int c = 1; c <= 4; c++) lta->xforms[0].m_L->rptr[r][c] = getf();
      }
      pedir = geti();
    }
    fflush(stdout);
    int r = cmd == "vv" ? MRIvol2Vol(src, targ, M, interp, param)
                         : MRIvol2VolVSM(src, targ, M, interp, param, vsm, lta, pedir);
    fflush(stdout);
    oi(r);
    put_vol(targ);
  } else if (cmd == "vvk") {
    MRI *src = get_vol();
    fill_paint(src);
    MRI *targ = get_vol();
    fill_paint(targ);
    MATRIX *M = get_m44();
    fflush(stdout);
    MRI *r = MRIvol2VolTLKernel(src, targ, M);
    fflush(stdout);
    oi(r == NULL);
    put_vol(targ);
  } else if (cmd == "vvd") {
    MRI *mov = get_vol();
    MRI *targ = get_vol();
    MATRIX *R = get_m44();
    fflush(stdout);
    MRI *r = MRIvol2VolDelta(mov, targ, R);
    fflush(stdout);
    put_vol(r);
  } else if (cmd == "maskup") {
    MRI *src = get_vol();
    fill_paint(src);
    MRI *mask = NULL;
    if (geti()) {
      mask = get_vol();
      fill_paint(mask);
    }
    int up = geti(), npad = geti(), cons = geti();
    if (geti()) Gdiag_no = 1;
    LTA *l = NULL;
    fflush(stdout);
    MRI *r = MRImaskAndUpsample(src, mask, up, npad, cons, &l);
    fflush(stdout);
    put_vol(r);
    put_lta(r ? l : NULL);
  } else if (cmd == "cov" || cmd == "mpca") {
    MRI *a = get_vol();
    fill_paint(a);
    MRI *mask = NULL;
    if (geti()) {
      mask = get_vol();
      fill_paint(mask);
    }
    fflush(stdout);
    if (cmd == "cov") {
      MATRIX *m = MRIcovarianceMatrix(a, mask);
      fflush(stdout);
      om(m);
    } else {
      MATRIX *U = NULL;
      VECTOR *S = NULL;
      MRI *V = NULL;
      int r = MRIpca(a, &U, &S, &V, mask);
      fflush(stdout);
      oi(r);
      om(U);
      om(S);
      put_vol(V);
    }
  } else if (cmd == "pcast") {
    int n = geti();
    MATRIX *S = MatrixAlloc(1, n, MATRIX_REAL);
    for (int i = 1; i <= n; i++) S->rptr[1][i] = getf();
    int mode = geti();
    std::string dir = scratch.substr(0, scratch.rfind('/'));
    if (chdir(dir.c_str())) fail("chdir");
    unlink("fsvol-pca.dat");
    fflush(stdout);
    int r = mode == 0 ? PrintPCAStats(stdout, S)
                      : WritePCAStats((char *)(mode == 2 ? "fsvol-nodir/pca.dat" : "fsvol-pca.dat"), S);
    // the program's end flushes the file WritePCAStats leaves open
    fflush(NULL);
    oi(r);
    ohex(slurp("fsvol-pca.dat"));
    unlink("fsvol-pca.dat");
  } else if (cmd == "fdr") {
    int n = geti();
    std::vector<MRI *> vols(n), masks(n), outs(n);
    std::vector<int> frames(n);
    for (int i = 0; i < n; i++) {
      vols[i] = get_vol();
      fill_paint(vols[i]);
      frames[i] = geti();
      masks[i] = NULL;
      if (geti()) {
        masks[i] = get_vol();
        fill_paint(masks[i]);
      }
      int mode = geti();
      outs[i] = mode == 1 ? get_vol() : mode == 2 ? vols[i] : NULL;
    }
    double fdr = getd();
    int sign = geti(), lg = geti(), dbg = geti(), passmask = geti(), passout = geti();
    double vwth = -7;
    fflush(stdout);
    int r = MRIfdr2vwth(vols.data(), n, frames.data(), fdr, sign, lg, passmask ? masks.data() : NULL, &vwth,
                        passout ? outs.data() : NULL, dbg);
    fflush(stdout);
    oi(r);
    od(vwth);
    for (int i = 0; i < n; i++)
      if (passout && outs[i]) put_vol(outs[i]);
  } else if (cmd == "rgbb") {
    MRI *m = get_vol();
    fill_paint(m);
    int kind = geti();
    int p[6] = {0, 0, 0, 0, 0, 0};
    int npad = 0;
    if (kind == 1)
      for (int &x : p) x = geti();
    else
      npad = geti();
    fflush(stdout);
    MRI_REGION *r = kind == 0 ? REGIONgetBoundingBox(m, npad)
                    : kind == 1 ? REGIONgetBoundingBoxM(m, p)
                                : REGIONgetBoundingBoxEqOdd(m, npad);
    fflush(stdout);
    if (!r) {
      out += "NULL ";
    } else {
      oi(r->x); oi(r->y); oi(r->z); oi(r->dx); oi(r->dy); oi(r->dz);
    }
  } else if (cmd == "vxn") {
    MRI *m = get_vol();
    fill_paint(m);
    int n = geti();
    for (int i = 0; i < n; i++) {
      int x = geti(), y = geti(), z = geti(), w = geti(), f = geti();
      fflush(stdout);
      of(MRIvoxelDx(m, x, y, z));
      of(MRIvoxelDy(m, x, y, z));
      of(MRIvoxelDz(m, x, y, z));
      of(MRIvoxelMean(m, x, y, z, w, f));
      of(MRIvoxelDirection(m, x, y, z, w));
      of(MRIvoxelGradientDir2ndDerivative(m, x, y, z, w));
      fflush(stdout);
    }
  } else if (cmd == "talcras" || cmd == "talmat" || cmd == "talpt" || cmd == "talvol") {
    // talairachex.cpp: kind, then the volume (painted and an optional second volume for
    // talvol), the transform array, NO_AVERAGE305, DIAG_SHOW, DIAG_VERBOSE; kind 4 of talpt: a
    // 4 x 4 matrix instead
    int kind = cmd == "talcras" ? 0 : geti();
    MATRIX *M = NULL;
    MRI *m = NULL, *tal = NULL;
    LTA *lta = NULL;
    int interp = 0;
    if (cmd == "talpt" && kind == 4) {
      M = MatrixAlloc(4, 4, MATRIX_REAL);
      for (int r = 1; r <= 4; r++)
        for (int c = 1; c <= 4; c++) M->rptr[r][c] = getf();
    } else {
      m = get_vol();
      if (cmd == "talvol") {
        fill_paint(m);
        if (geti()) tal = get_vol();
      }
      if (geti()) {
        int n = geti();
        lta = LTAalloc(n, NULL);
        lta->type = geti();
        if (n >= 1) {
          for (int r = 1; r <= 4; r++)
            for (int c = 1; c <= 4; c++) lta->xforms[0].m_L->rptr[r][c] = getf();
          set_geom(&lta->xforms[0].dst);
        }
      }
      if (geti()) setenv("NO_AVERAGE305", "true", 1);
      else unsetenv("NO_AVERAGE305");
      if (geti()) Gdiag |= DIAG_SHOW;
      if (geti()) Gdiag |= DIAG_VERBOSE;
      if (cmd == "talvol" && kind == 0) interp = geti();
    }
    if (cmd == "talcras") {
      fflush(stdout);
      int r = ModifyTalairachCRAS(m, lta);
      fflush(stdout);
      oi(r);
      put_vol(m);
    } else if (cmd == "talmat") {
      fflush(stdout);
      MATRIX *R = kind == 0 ? MtalairachFromVoxel(m, lta)
                  : kind == 1 ? MtalVoxelFromVoxel(m, lta)
                  : kind == 2 ? MvoxelFromTalairach(m, lta)
                              : MvoxelFromTalVoxel(m, lta);
      fflush(stdout);
      om(R);
    } else if (cmd == "talpt") {
      int n = geti();
      for (int i = 0; i < n; i++) {
        double x = getd(), y = getd(), z = getd();
        double px = -7, py = -7, pz = -7;
        fflush(stdout);
        int r = 0;
        if (kind == 4) TransformWithMatrix(M, x, y, z, &px, &py, &pz);
        else if (kind == 0) r = MRIvoxelToTalairachEx(m, x, y, z, &px, &py, &pz, lta);
        else if (kind == 1) r = MRIvoxelToTalairachVoxelEx(m, x, y, z, &px, &py, &pz, lta);
        else if (kind == 2) r = MRItalairachToVoxelEx(m, x, y, z, &px, &py, &pz, lta);
        else r = MRItalairachVoxelToVoxelEx(m, x, y, z, &px, &py, &pz, lta);
        fflush(stdout);
        oi(r);
        od(px); od(py); od(pz);
      }
    } else {
      fflush(stdout);
      MRI *r = kind == 0 ? MRItoTalairachExInterp(m, tal, lta, interp)
               : kind == 1 ? MRItoTalairachEx(m, tal, lta)
                           : MRIfromTalairachEx(m, tal, lta);
      fflush(stdout);
      oi(r != NULL && r == tal);
      put_vol(r);
    }
  } else if (cmd == "dilate" || cmd == "dilu") {
    // MRIdilate / MRIdilateUchar (mriset.cpp): the painted source, the destination mode
    MRI *src = get_vol();
    fill_paint(src);
    MRI *dst = get_dst(src);
    fflush(stdout);
    MRI *r = cmd == "dilate" ? MRIdilate(src, dst) : MRIdilateUchar(src, dst);
    fflush(stdout);
    oi(r != NULL && r == dst);
    put_vol(r);
  } else if (cmd == "msegs") {
    MRI *seg = get_vol();
    fill_paint(seg);
    MRI *dst = get_dst(seg);
    int n = geti();
    std::vector<int> list(n + 1);
    for (int i = 0; i < n; i++) list[i] = geti();
    int maskid = geti();
    fflush(stdout);
    MRI *r = MRImatchSegs(seg, list.data(), n, maskid, dst);
    fflush(stdout);
    oi(r != NULL && r == dst);
    put_vol(r);
  } else if (cmd == "sstat" || cmd == "hseg") {
    MRI *seg = get_vol();
    fill_paint(seg);
    MRI *vol = get_vol();
    fill_paint(vol);
    int segid = geti();
    if (cmd == "sstat") {
      int frame = geti();
      float mn = -7, mx = -7, rg = -7, mean = -7, sd = -7;
      fflush(stdout);
      int n = MRIsegStats(seg, segid, vol, frame, &mn, &mx, &rg, &mean, &sd);
      fflush(stdout);
      oi(n);
      of(mn); of(mx); of(rg); of(mean); of(sd);
    } else {
      double bmin = getd(), bmax = getd(), bdelta = getd();
      fflush(stdout);
      HISTOGRAM *h = HISTOseg(seg, segid, vol, bmin, bmax, bdelta);
      fflush(stdout);
      oi(h->nbins);
      for (int i = 0; i < h->nbins; i++) { of(h->bins[i]); of(h->counts[i]); }
    }
  } else if (cmd == "conform") {
    // mri_conform.cpp: 0 MRIconform (the voxels hashed), 1 MRIconformedTemplate, 2 MRIgetConformMatrix
    int kind = geti();
    MRI *m = get_vol();
    fill_paint(m);
    if (kind == 0) {
      fflush(stdout);
      MRI *r = MRIconform(m);
      fflush(stdout);
      put_vol(r, true);
    } else if (kind == 1) {
      int cw = geti();
      double cs = getd();
      int keep = geti();
      fflush(stdout);
      MRI *r = MRIconformedTemplate(m, cw, cs, keep);
      fflush(stdout);
      put_vol(r);
    } else {
      fflush(stdout);
      MATRIX *M = MRIgetConformMatrix(m);
      fflush(stdout);
      om(M);
      put_geom(m, false);
    }
  } else if (cmd == "union" || cmd == "isect" || cmd == "mand" || cmd == "mxor") {
    // mriset.cpp's two-volume functions: a, b (each painted), the destination mode, parameters
    MRI *a = get_vol();
    fill_paint(a);
    MRI *b = get_vol();
    fill_paint(b);
    MRI *dst = get_dst2(a, b);
    int t1 = 0, t2 = 0;
    if (cmd == "mand") t1 = geti();
    if (cmd == "mxor") { t1 = geti(); t2 = geti(); }
    fflush(stdout);
    MRI *r = cmd == "union" ? MRIunion(a, b, dst)
             : cmd == "isect" ? MRIintersect(a, b, dst)
             : cmd == "mand"  ? MRIand(a, b, dst, t1)
                              : MRIxor(a, b, dst, t1, t2);
    fflush(stdout);
    oi(r != NULL && r == dst);
    put_vol(r);
  } else if (cmd == "compl" || cmd == "edges" || cmd == "erodenn" || cmd == "erode" || cmd == "erode2d" ||
             cmd == "erodeseg" || cmd == "dilseg" || cmd == "dtl" || cmd == "dtli" || cmd == "dlu" ||
             cmd == "dlab" || cmd == "openn" || cmd == "mopen" || cmd == "mclose" || cmd == "dil6") {
    // mriset.cpp's one-volume functions: the painted source, the destination mode, parameters
    MRI *src = get_vol();
    fill_paint(src);
    MRI *dst = get_dst(src);
    MRI *val = NULL, *mask = NULL, *r = NULL;
    int n = 0;
    if (cmd == "dtl" || cmd == "dtli") { val = get_vol(); fill_paint(val); }
    if (cmd == "dilseg" && geti()) { mask = get_vol(); fill_paint(mask); }
    fflush(stdout);
    if (cmd == "compl") r = MRIcomplement(src, dst);
    else if (cmd == "edges") { double sv = getd(); fflush(stdout); r = MRIsetEdges(src, sv, dst); }
    else if (cmd == "erodenn") { int nn = geti(), ee = geti(); fflush(stdout); r = MRIerodeNN(src, dst, nn, ee); }
    else if (cmd == "erode") r = MRIerode(src, dst);
    else if (cmd == "erode2d") r = MRIerode2D(src, dst);
    else if (cmd == "erodeseg") { int ne = geti(), th = geti(); fflush(stdout); r = MRIerodeSegmentation(src, dst, ne, th); }
    else if (cmd == "dilseg") {
      int nd = geti(), mf = geti();
      double mt = getd();
      n = -7;
      fflush(stdout);
      r = MRIdilateSegmentation(src, dst, nd, mask, mf, mt, &n);
    }
    else if (cmd == "dtl" || cmd == "dtli") {
      int label = geti(), niter = geti(), th = geti();
      fflush(stdout);
      r = cmd == "dtl" ? MRIdilateThreshLabel(src, val, dst, label, niter, th)
                       : MRIdilateInvThreshLabel(src, val, dst, label, niter, th);
    }
    else if (cmd == "dlu" || cmd == "dlab") {
      int label = geti(), niter = geti();
      fflush(stdout);
      r = cmd == "dlu" ? MRIdilateLabelUchar(src, dst, label, niter) : MRIdilateLabel(src, dst, label, niter);
    }
    else if (cmd == "openn") { int order = geti(); fflush(stdout); r = MRIopenN(src, dst, order); }
    else if (cmd == "mopen") r = MRIopen(src, dst);
    else if (cmd == "mclose") r = MRIclose(src, dst);
    else r = MRIdilate6(src, dst);
    fflush(stdout);
    oi(r != NULL && r == dst);
    if (cmd == "dilseg") oi(n);
    put_vol(r);
  } else if (cmd == "rlist" || cmd == "rvonly" || cmd == "mdiff" || cmd == "mmask" || cmd == "tmask" ||
             cmd == "mnot" || cmd == "slv" || cmd == "b2c") {
    // mriset.cpp's masks and replacements: the painted source, the destination mode, a second
    // painted volume (mask or labels) where the function takes one, parameters
    MRI *src = get_vol();
    fill_paint(src);
    MRI *dst = get_dst(src);
    MRI *m2 = NULL, *r = NULL;
    if (cmd == "mdiff" || cmd == "mmask" || cmd == "tmask" || cmd == "slv" || (cmd == "rlist" && geti())) {
      m2 = get_vol();
      fill_paint(m2);
    }
    if (cmd == "rlist") {
      int n = geti();
      std::vector<int> a(n + 1), b(n + 1);
      for (int i = 0; i < n; i++) a[i] = geti();
      for (int i = 0; i < n; i++) b[i] = geti();
      fflush(stdout);
      r = MRIreplaceList(src, a.data(), b.data(), n, m2, dst);
    } else if (cmd == "rvonly") {
      float iv = getf(), ov = getf();
      fflush(stdout);
      r = MRIreplaceValuesOnly(src, dst, iv, ov);
    } else if (cmd == "mdiff" || cmd == "mmask") {
      int mk = geti();
      float ov = getf();
      fflush(stdout);
      r = cmd == "mdiff" ? MRImaskDifferentGeometry(src, m2, dst, mk, ov) : MRImask(src, m2, dst, mk, ov);
    } else if (cmd == "tmask") {
      float th = getf(), ov = getf();
      fflush(stdout);
      r = MRIthresholdMask(src, m2, dst, th, ov);
    } else if (cmd == "mnot") {
      fflush(stdout);
      r = MRInot(src, dst);
    } else if (cmd == "slv") {
      int label = geti();
      float v = getf();
      fflush(stdout);
      r = MRIsetLabelValues(src, m2, dst, label, v);
    } else {
      fflush(stdout);
      r = MRIbinMaskToCol(src, dst);
    }
    fflush(stdout);
    oi(r != NULL && r == dst);
    put_vol(r);
  } else if (cmd == "nmask" || cmd == "von" || cmd == "vinl" || cmd == "linv" || cmd == "cent" || cmd == "pthr" ||
             cmd == "cnbhd") {
    // mriset.cpp's counts and measures of one painted volume
    MRI *m = get_vol();
    fill_paint(m);
    if (cmd == "nmask") { fflush(stdout); oi(MRInMask(m)); }
    else if (cmd == "von") { int th = geti(); fflush(stdout); oi(MRItotalVoxelsOn(m, th)); }
    else if (cmd == "vinl") { int l = geti(); fflush(stdout); oi(MRIvoxelsInLabel(m, l)); }
    else if (cmd == "linv") { int l = geti(); fflush(stdout); oi(MRIlabelInVolume(m, l)); }
    else if (cmd == "cent") {
      double xc = -7, yc = -7, zc = -7;
      fflush(stdout);
      oi(MRIcomputeCentroid(m, &xc, &yc, &zc));
      od(xc); od(yc); od(zc);
    } else if (cmd == "pthr") {
      MRI *mask = NULL;
      if (geti()) { mask = get_vol(); fill_paint(mask); }
      int frame = geti();
      double pct = getd();
      fflush(stdout);
      od(MRIpercentThresh(m, mask, frame, pct));
    } else {
      int ws = geti(), x = geti(), y = geti(), z = geti(), v = geti();
      fflush(stdout);
      oi(MRIcountValInNbhd(m, ws, x, y, z, v));
    }
    fflush(stdout);
  } else if (cmd == "grow" || cmd == "onfg" || cmd == "offbg" || cmd == "cplab" || cmd == "cplv") {
    // mriset.cpp's functions on two or three painted volumes changed in place
    MRI *a = get_vol();
    fill_paint(a);
    MRI *b = get_vol();
    fill_paint(b);
    MRI *c = NULL;
    if (cmd == "onfg" || cmd == "cplv") { c = get_vol(); fill_paint(c); }
    int r = 0;
    if (cmd == "grow") { int il = geti(), ol = geti(); fflush(stdout); r = MRIgrowLabel(a, b, il, ol); }
    else if (cmd == "onfg") { fflush(stdout); r = MRIturnOnFG(a, b, c); }
    else if (cmd == "offbg") { fflush(stdout); r = MRIturnOffBG(a, b); }
    else if (cmd == "cplab") { int l = geti(); fflush(stdout); r = MRIcopyLabel(a, b, l); }
    else { int l = geti(); fflush(stdout); r = MRIcopyLabeledVoxels(a, b, c, l); }
    fflush(stdout);
    oi(r);
    put_vol(a);
    put_vol(b);
    if (c) put_vol(c);
  } else if (cmd == "hreg" || cmd == "hvox" || cmd == "hthr") {
    // mrihisto.cpp: MRIhistogramRegion, MRIhistogramVoxel, MRIhistogramRegionWithThreshold
    MRI *m = get_vol();
    fill_paint(m);
    fill_pat(m);
    int nbins = geti();
    HISTOGRAM *h = get_histo();
    HISTOGRAM *r;
    if (cmd == "hvox") {
      int x = geti(), y = geti(), z = geti(), ws = geti();
      MRI *t = NULL;
      if (geti()) { t = get_vol(); fill_paint(t); fill_pat(t); }
      float th = getf();
      fflush(stdout);
      r = MRIhistogramVoxel(m, nbins, h, x, y, z, ws, t, th);
    } else {
      MRI_REGION reg;
      get_region(&reg);
      if (cmd == "hreg") {
        fflush(stdout);
        r = MRIhistogramRegion(m, nbins, h, &reg);
      } else {
        MRI *t = get_vol();
        fill_paint(t);
        fill_pat(t);
        float th = getf();
        int fr = geti();
        fflush(stdout);
        r = MRIhistogramRegionWithThreshold(m, nbins, h, &reg, t, th, fr);
      }
    }
    fflush(stdout);
    oi(r != NULL && r == h);
    put_histo(r);
  } else if (cmd == "hist" || cmd == "geq") {
    // mrihisto.cpp: MRIhistogram, MRIgetEqualizeHisto
    MRI *m = get_vol();
    fill_paint(m);
    fill_pat(m);
    if (cmd == "hist") {
      int nb = geti();
      fflush(stdout);
      HISTOGRAM *r = MRIhistogram(m, nb);
      fflush(stdout);
      put_histo(r);
    } else {
      HISTOGRAM *h = get_histo();
      int lo = geti(), hi = geti(), norm = geti();
      fflush(stdout);
      HISTOGRAM *r = MRIgetEqualizeHisto(m, h, lo, hi, norm);
      fflush(stdout);
      oi(r != NULL && r == h);
      put_histo(r);
    }
  } else if (cmd == "heq") {
    // mrihisto.cpp: MRIhistoEqualize (the template another volume)
    MRI *src = get_vol();
    fill_paint(src);
    fill_pat(src);
    MRI *tpl = get_vol();
    fill_paint(tpl);
    fill_pat(tpl);
    MRI *dst = get_dst(src);
    int lo = geti(), hi = geti();
    fflush(stdout);
    MRI *r = MRIhistoEqualize(src, tpl, dst, lo, hi);
    fflush(stdout);
    oi(r != NULL && r == dst);
    put_vol(r);
  } else if (cmd == "hlr" || cmd == "hlab") {
    // mrihisto.cpp: MRIhistogramLabelRegion, MRIhistogramLabel
    MRI *m = get_vol();
    fill_paint(m);
    fill_pat(m);
    MRI *l = get_vol();
    fill_paint(l);
    int label = geti(), nb = geti();
    HISTOGRAM *r;
    if (cmd == "hlr") {
      MRI_REGION reg;
      get_region(&reg);
      fflush(stdout);
      r = MRIhistogramLabelRegion(m, l, &reg, label, nb);
    } else {
      fflush(stdout);
      r = MRIhistogramLabel(m, l, label, nb);
    }
    fflush(stdout);
    put_histo(r);
  } else if (cmd == "hsegm") {
    // mrihisto.cpp: MRIhistoSegment (the labelled volume: NULL, another painted one, the source)
    MRI *src = get_vol();
    fill_paint(src);
    fill_pat(src);
    MRI *dst = get_dst(src);
    if (dst && dst != src) fill_paint(dst);
    int wl = geti(), wh = geti(), gh = geti(), ws = geti();
    float sg = getf();
    if (geti()) Gdiag |= DIAG_SHOW;
    fflush(stdout);
    MRI *r = MRIhistoSegment(src, dst, wl, wh, gh, ws, sg);
    fflush(stdout);
    oi(r != NULL && r == dst);
    put_vol(r);
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
    fprintf(stderr, "usage: fsvolume <cases>\n");
    return 2;
  }
  if (argv[1][0] != '/') fail("the cases must be given by an absolute path");
  scratch = std::string(argv[1]) + ".out";
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
      alarm(5);
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
  unlink((scratch + ".vg").c_str());
  return 0;
}
