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

#include <cerrno>
#include <cinttypes>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>

#include "diag.h"
#include "error.h"
#include "matrix.h"
#include "mri.h"
#include "transform.h"
#include "utils.h"

extern double vg_isEqual_Threshold;
// defined in mri.cpp, not declared in its headers
double ham_sinc(double x, double fullwidth);
int MRIcubicSampleVolumeFrame(MRI *mri, double x, double y, double z, int frame, double *pval);
int MRIsincSampleVolumeFrame(MRI *mri, double x, double y, double z, int frame, int hw, double *pval);

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
  } else if (cmd == "coef" || cmd == "hsinc") {
    int n = geti();
    for (int i = 0; i < n; i++) {
      double x = getd();
      if (cmd == "coef") od(MRIcubicCoeff(x, geti()));
      else od(ham_sinc(x, getd()));
    }
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
