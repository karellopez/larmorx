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
#include "diag.h"
#include "error.h"
#include "image.h"
#include "mri_circulars.h"
#include "matrix.h"
#include "mri.h"
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

void put_vol(const MRI *m)
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
    std::string b((const char *)m->chunk, m->bytes_total);
    ohex(b);
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
