// SPDX-License-Identifier: Apache-2.0
// A black-box probe of FreeSurfer 8.2.0's volume functions (specs/freesurfer-volume.md, parts 1 to 5:
// the volume-geometry text block and its copies, matrices, comparison and printer; the voxel
// arithmetic; coordinates, orientation, centre of mass, principal components, the running average,
// the header comparison and dump; the samplers, moves, the head's box and the transform reader; the
// CMA label tests and segmentation functions; part 6a: resolution, planes, fills, resampling by whole
// factors, changes of type; part 6b: frame-range samplers, the trilinear kernel and splatting,
// derivatives, gradients, neighbour counts, linear transforms; part 6c: resampling onto a template,
// replacing a value; part 7a: statistics, orientation strings, voxel-to-RAS matrices; fs.bspline: the B-spline coefficients, their sampling, transforms and
// factor-of-two pyramids, specs/freesurfer-bspline.md), linked against the unit oracle's libutils.a (GCC 8.4.0,
// FreeSurfer's flags). It holds only calls of FreeSurfer's functions (and of the C library's) by name
// and the reading and printing of their arguments, results and the structures' fields: no FreeSurfer
// code. The commands and encodings are in README.md beside this file (the volume, geometry and header
// encodings are those of the fs.mri probe, ../freesurfer-mri-probe/); a candidate program takes the
// same ones.
//
//   fs_volume_probe batch <file>   one call per line of <file>, each in a child process of its own
//                                  with an alarm; one result line per call on standard output
#include <fcntl.h>
#include <signal.h>
#include <sys/stat.h>
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
#include <cwchar>
#include <string>
#include <vector>

#include "cma.h"
#include "mriBSpline.h"
#include "colortab.h"
#include "diag.h"
#include "error.h"
#include "matrix.h"
#include "mri.h"
#include "transform.h"
#include "utils.h"

// functions of FreeSurfer's library that its headers do not declare
int MRIcopyVolGeomToMRI(MRI *mri, const VOL_GEOM *vg);
int MRIcubicSampleVolumeFrame(MRI *mri, double x, double y, double z, int frame, double *pval);
int MRIsincSampleVolumeFrame(MRI *mri, double x, double y, double z, int frame, int hw, double *pval);
double ham_sinc(double x, double fullwidth);

const char *Progname = "fs_volume_probe";

// the guard allocator's switch (guard_malloc.c of the fs.maths probe), when it is preloaded
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
static bool take(const char *s)
{
  if (!peek(s)) return false;
  P++;
  return true;
}
static int geti() { return (int)strtol(tok().c_str(), NULL, 10); }
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
static void puti(long long v) { out(std::to_string(v)); }
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
static std::string hexbytes(const char *p, size_t n)
{
  std::string o = "x";
  char b[3];
  for (size_t i = 0; i < n; i++) {
    snprintf(b, sizeof b, "%02x", (unsigned char)p[i]);
    o += b;
  }
  return o;
}
static void puts_(const char *s) { out(s ? hexbytes(s, strlen(s)) : "null"); }
static std::string slurp(const char *path)
{
  std::string s;
  FILE *fp = fopen(path, "rb");
  if (!fp) return s;
  int c;
  while ((c = fgetc(fp)) != EOF) s.push_back((char)c);
  fclose(fp);
  return s;
}

// a string field of a fixed size: at most size - 1 bytes are taken
static void setname(char *dst, size_t size, const std::string &s)
{
  size_t n = s.size() < size ? s.size() : size - 1;
  memcpy(dst, s.data(), n);
  dst[n] = 0;
}

// matrices: null, or <rows> <cols> and the floats row by row
static MATRIX *getm()
{
  if (take("null")) return NULL;
  int rows = geti(), cols = geti();
  MATRIX *m = MatrixAlloc(rows, cols, MATRIX_REAL);
  for (int r = 1; r <= rows; r++)
    for (int c = 1; c <= cols; c++) m->rptr[r][c] = getf();
  return m;
}
static void putm(const MATRIX *m)
{
  if (!m) {
    out("null");
    return;
  }
  puti(m->rows);
  puti(m->cols);
  for (int r = 1; r <= m->rows; r++)
    for (int c = 1; c <= m->cols; c++) putf(m->rptr[r][c]);
}
// a returned matrix: `given` when it is the given matrix itself, then its state
static void putm_res(const MATRIX *res, const MATRIX *given)
{
  if (given && res == given) out("given");
  putm(res);
}

// a geometry: valid w h d xsize ysize zsize ras_good x_r x_a x_s y_r y_a y_s z_r z_a z_s c_r c_a c_s
// s_r s_a s_s name (without `valid` for a volume's own geometry: README.md)
static void getg(VOL_GEOM *v, bool with_valid = true)
{
  if (with_valid) v->valid = geti();
  v->width = geti();
  v->height = geti();
  v->depth = geti();
  v->xsize = getf();
  v->ysize = getf();
  v->zsize = getf();
  v->ras_good_flag = geti();
  v->x_r = getf();
  v->x_a = getf();
  v->x_s = getf();
  v->y_r = getf();
  v->y_a = getf();
  v->y_s = getf();
  v->z_r = getf();
  v->z_a = getf();
  v->z_s = getf();
  v->c_r = getf();
  v->c_a = getf();
  v->c_s = getf();
  v->s_r = getf();
  v->s_a = getf();
  v->s_s = getf();
  setname(v->fname, sizeof v->fname, getx());
}
// the fields, then the cached voxel-to-RAS (16 floats as stored, or null), the cached RAS-to-voxel
// and the registration matrix
static void putg(const VOL_GEOM *v, bool with_valid = true)
{
  if (with_valid) puti(v->valid);
  puti(v->width);
  puti(v->height);
  puti(v->depth);
  putf(v->xsize);
  putf(v->ysize);
  putf(v->zsize);
  puti(v->ras_good_flag);
  for (float f : {v->x_r, v->x_a, v->x_s, v->y_r, v->y_a, v->y_s, v->z_r, v->z_a, v->z_s, v->c_r, v->c_a,
                  v->c_s, v->s_r, v->s_a, v->s_s})
    putf(f);
  puts_(v->fname);
  if (v->i_to_r__) {
    out("cache");
    for (int i = 0; i < 16; i++) putf(v->i_to_r__->mat[i]);
  }
  else
    out("null");
  putm(v->r_to_i__);
  putm(v->register_mat);
}
// a geometry argument followed by its cache flag (0: none; 1: the cache computed from the fields)
// and its registration matrix
static void getg_full(VOL_GEOM *v, bool with_valid = true)
{
  getg(v, with_valid);
  if (geti()) MRIreInitCache(v);
  v->register_mat = getm();
}

// volumes: <w> <h> <d> <type> <nframes> <h>, h 1 a header without voxels (MRIallocHeader), 0 with
// voxels (MRIallocSequence)
static MRI *getv()
{
  int w = geti(), h = geti(), d = geti(), type = geti(), nf = geti(), hdr = geti();
  return hdr ? MRIallocHeader(w, h, d, type, nf) : MRIallocSequence(w, h, d, type, nf);
}
static bool has_voxels(const MRI *m) { return m->chunk != NULL && m->ischunked; }
// the voxels' bytes from a seed: byte i is (seed + 101 i) mod 256; seed -1 leaves them
static void fill(MRI *m, int seed)
{
  if (seed < 0 || !has_voxels(m)) return;
  unsigned char *b = (unsigned char *)m->chunk;
  for (size_t i = 0; i < m->bytes_total; i++) b[i] = (unsigned char)((seed + 101u * i) & 0xffu);
}
static void putvox(const MRI *m)
{
  if (!has_voxels(m)) {
    out("novox");
    return;
  }
  out(hexbytes((const char *)m->chunk, m->bytes_total));
}
static void puthdr(const MRI *m)
{
  puti(m->width);
  puti(m->height);
  puti(m->depth);
  puti(m->nframes);
  puti(m->type);
  for (size_t n : {m->bytes_per_vox, m->bytes_total, m->vox_per_row, m->vox_per_slice, m->vox_per_vol, m->vox_total})
    puti((long long)n);
  puti(m->imnr0);
  puti(m->imnr1);
  for (float f : {m->xstart, m->ystart, m->zstart, m->xend, m->yend, m->zend, m->fov, m->thick, m->ps}) putf(f);
  puti(m->scale);
  puti(m->roi.dx);
  puti(m->roi.dy);
  puti(m->roi.dz);
  puti(m->version);
  puti(m->intent);
  puti(m->dof);
  puti(m->brightness);
  puti(m->yinvert);
  puti(m->ptype);
  putd(m->outside_val);
  putd(m->flip_angle);
  for (float f : {m->tr, m->te, m->ti, m->location, m->FieldStrength}) putf(f);
  puts_(m->pedir);
  puti(m->bvec_space);
  putm(m->bvals);
  putm(m->bvecs);
  puti(m->warpFieldFormat);
  puti(m->gcamorphSpacing);
  putd(m->gcamorphExp_k);
  puti(m->ncmds);
  for (int i = 0; i < m->ncmds; i++) puts_(m->cmdlines[i]);
  puts_(m->subject_name);
  puts_(m->path_to_t1);
  puts_(m->fname_format);
  puts_(m->gdf_image_stem);
  puts_(m->transform_fname);
  putm(m->AutoAlign);
  putm(m->origRas2Vox);
  puti(m->free_transform);
  puti(m->linear_transform != NULL);
  puti(m->ct != NULL);
  putg(m, false);
}
static void putidx(const MRI *m)
{
  if (!m->xi) {
    out("noindex");
    return;
  }
  const int ext[3] = {m->width, m->height, m->depth};
  const int *tab[3] = {m->xi, m->yi, m->zi};
  for (int k = 0; k < 3; k++)
    for (int i : {-500, -1, 0, 1, ext[k] - 1, ext[k], ext[k] + 499}) puti(tab[k][i]);
}
static void putframes(const MRI *m)
{
  if (!m->frames) {
    out("noframes");
    return;
  }
  for (int f = 0; f < m->nframes; f++) {
    const MRI_FRAME &x = m->frames[f];
    puti(x.type);
    for (float v : {x.TE, x.TR, x.flip, x.TI, x.TD}) putf(v);
    puti(x.sequence_type);
    putf(x.echo_spacing);
    putf(x.echo_train_len);
    for (int i = 0; i < 3; i++) putf(x.read_dir[i]);
    for (int i = 0; i < 3; i++) putf(x.pe_dir[i]);
    for (int i = 0; i < 3; i++) putf(x.slice_dir[i]);
    puti(x.label);
    puts_(x.name);
    puti(x.dof);
    putm(x.m_ras2vox);
    putf(x.thresh);
    puti(x.units);
    for (double v : {x.DX, x.DY, x.DZ, x.DR, x.DP, x.DS, x.bvalue, x.TM}) putd(v);
    puti(x.diffusion_type);
    for (long v : {x.D1_ramp, x.D1_flat, x.D2_ramp, x.D2_flat, x.D3_ramp, x.D3_flat, x.D4_ramp, x.D4_flat}) puti(v);
    for (double v : {x.D1_amp, x.D2_amp, x.D3_amp, x.D4_amp}) putd(v);
  }
}

// colour tables: <n> <name> <version> <idbase> <schema>, then per entry 0 (none) or 1 <name> <ri> <gi>
// <bi> <ai> <rf> <gf> <bf> <af> <tissue type> <count>, then the tissue-type table (null, or a table
// whose own tissue-type table is null)
static COLOR_TABLE *getct(bool inner = false)
{
  if (take("null")) return NULL;
  int n = geti();
  COLOR_TABLE *ct = (COLOR_TABLE *)calloc(1, sizeof(COLOR_TABLE));
  ct->nentries = n;
  ct->entries = (COLOR_TABLE_ENTRY **)calloc(n > 0 ? n : 1, sizeof(COLOR_TABLE_ENTRY *));
  setname(ct->fname, sizeof ct->fname, getx());
  ct->version = geti();
  ct->idbase = geti();
  setname(ct->TissueTypeSchema, sizeof ct->TissueTypeSchema, getx());
  for (int i = 0; i < n; i++) {
    if (!geti()) continue;
    CTE *e = (CTE *)calloc(1, sizeof(CTE));
    setname(e->name, sizeof e->name, getx());
    e->ri = geti();
    e->gi = geti();
    e->bi = geti();
    e->ai = geti();
    e->rf = getf();
    e->gf = getf();
    e->bf = getf();
    e->af = getf();
    e->TissueType = geti();
    e->count = geti();
    ct->entries[i] = e;
  }
  ct->ctabTissueType = inner ? (take("null"), (COLOR_TABLE *)NULL) : getct(true);
  return ct;
}
// as given, but each entry's count only with `counts`
static void putct(const COLOR_TABLE *ct, bool counts)
{
  if (!ct) {
    out("null");
    return;
  }
  puti(ct->nentries);
  puts_(ct->fname);
  puti(ct->version);
  puti(ct->idbase);
  puts_(ct->TissueTypeSchema);
  for (int i = 0; i < ct->nentries; i++) {
    const CTE *e = ct->entries[i];
    if (!e) {
      out("0");
      continue;
    }
    out("1");
    puts_(e->name);
    puti(e->ri);
    puti(e->gi);
    puti(e->bi);
    puti(e->ai);
    putf(e->rf);
    putf(e->gf);
    putf(e->bf);
    putf(e->af);
    puti(e->TissueType);
    if (counts) puti(e->count);
  }
  putct(ct->ctabTissueType, counts);
}

// the header fields (README.md, "H"): those the header copy copies, then some it does not
static void seth(MRI *m)
{
  m->version = geti();
  m->intent = geti();
  m->dof = geti();
  m->mean = getd();
  m->xsize = getf();
  m->ysize = getf();
  m->zsize = getf();
  m->imnr0 = geti();
  m->imnr1 = geti();
  m->ptype = geti();
  m->fov = getf();
  m->thick = getf();
  m->ps = getf();
  m->location = getf();
  m->xstart = getf();
  m->xend = getf();
  m->ystart = getf();
  m->yend = getf();
  m->zstart = getf();
  m->zend = getf();
  m->flip_angle = getd();
  m->tr = getf();
  m->te = getf();
  m->ti = getf();
  setname(m->fname, sizeof m->fname, getx());
  for (float *f : {&m->x_r, &m->x_a, &m->x_s, &m->y_r, &m->y_a, &m->y_s, &m->z_r, &m->z_a, &m->z_s, &m->c_r, &m->c_a, &m->c_s})
    *f = getf();
  m->ras_good_flag = geti();
  m->brightness = geti();
  setname(m->subject_name, sizeof m->subject_name, getx());
  setname(m->path_to_t1, sizeof m->path_to_t1, getx());
  setname(m->fname_format, sizeof m->fname_format, getx());
  setname(m->gdf_image_stem, sizeof m->gdf_image_stem, getx());
  setname(m->transform_fname, sizeof m->transform_fname, getx());
  int n = geti();
  for (int i = 0; i < m->ncmds; i++) free(m->cmdlines[i]);
  for (int i = 0; i < n; i++) {
    std::string s = getx();
    m->cmdlines[i] = (char *)calloc(s.size() + 1, 1);
    memcpy(m->cmdlines[i], s.data(), s.size());
  }
  m->ncmds = n;
  MATRIX *reg = getm(), *aa = getm(), *orig = getm();
  if (reg) {
    MatrixFree(&m->register_mat);
    m->register_mat = reg;
  }
  if (aa) {
    MatrixFree(&m->AutoAlign);
    m->AutoAlign = aa;
  }
  if (orig) {
    MatrixFree(&m->origRas2Vox);
    m->origRas2Vox = orig;
  }
  COLOR_TABLE *ct = getct();
  if (ct) m->ct = ct;
  // fields the header copy leaves
  m->valid = geti();
  m->s_r = getf();
  m->s_a = getf();
  m->s_s = getf();
  m->scale = geti();
  m->yinvert = geti();
  m->outside_val = getd();
  m->FieldStrength = getf();
}

// a volume with its geometry: V then G
static MRI *getvg()
{
  MRI *m = getv();
  getg(m);
  return m;
}

// MINC transforms: none, or <kind> <inverse flag> <16 doubles> <16 doubles> (the forward and the
// inverse matrix, row by row); kind 0 a linear transform, 1 a concatenation of none, others a kind
// FreeSurfer's MINC code does not support
static bool getxfm(MRI *m)
{
  if (take("none")) return false;
  int kind = geti(), inv = geti();
  General_transform &t = m->transform;
  memset(&t, 0, sizeof t);
  t.type = (Transform_types)kind;
  t.inverse_flag = inv != 0;
  t.linear_transform = (Transform *)calloc(1, sizeof(Transform));
  t.inverse_linear_transform = (Transform *)calloc(1, sizeof(Transform));
  for (Transform *x : {t.linear_transform, t.inverse_linear_transform})
    for (int i = 0; i < 4; i++)
      for (int j = 0; j < 4; j++) Transform_elem(*x, i, j) = getd();
  m->linear_transform = inv ? t.inverse_linear_transform : t.linear_transform;
  m->inverse_linear_transform = inv ? t.linear_transform : t.inverse_linear_transform;
  m->free_transform = 1;
  return true;
}
static void putxfm(const MRI *m)
{
  puti(m->free_transform);
  for (const Transform *x : {m->linear_transform, m->inverse_linear_transform}) {
    if (!x) {
      out("null");
      continue;
    }
    for (int i = 0; i < 4; i++)
      for (int j = 0; j < 4; j++) putd(Transform_elem(*x, i, j));
  }
  if (m->free_transform) {
    puti((int)m->transform.type);
    puti(m->transform.inverse_flag ? 1 : 0);
  }
}

// a bounding box's volume: V, a fill seed, then n settings <c> <r> <s> <f> <value> (MRIsetVoxVal)
static MRI *getbv()
{
  MRI *m = getv();
  fill(m, geti());
  int n = geti();
  for (int i = 0; i < n; i++) {
    int c = geti(), r = geti(), s = geti(), f = geti();
    MRIsetVoxVal(m, c, r, s, f, getf());
  }
  return m;
}

// linear transforms: <x0> <y0> <z0> <sigma> <m_L> <m_dL> <m_last_dL> <type> <label> <src> <dst>, each
// geometry G, its cache flag and its registration matrix
static void getlt(LT *lt)
{
  lt->x0 = getf();
  lt->y0 = getf();
  lt->z0 = getf();
  lt->sigma = getf();
  for (MATRIX **mp : {&lt->m_L, &lt->m_dL, &lt->m_last_dL}) {
    MATRIX *m = getm();
    if (*mp) MatrixFree(mp);
    *mp = m;
  }
  lt->type = geti();
  lt->label = geti();
  getg_full(&lt->src);
  getg_full(&lt->dst);
}
static void putlt(const LT *lt)
{
  putf(lt->x0);
  putf(lt->y0);
  putf(lt->z0);
  putf(lt->sigma);
  putm(lt->m_L);
  putm(lt->m_dL);
  putm(lt->m_last_dL);
  puti(lt->type);
  puti(lt->label);
  putg(&lt->src);
  putg(&lt->dst);
}
// arrays: <n> <type> <fscale> <subject>, then the n transforms and the n inverse transforms
static LTA *getlta()
{
  int n = geti();
  LTA *lta = LTAalloc(n, NULL);
  lta->type = geti();
  lta->fscale = getf();
  setname(lta->subject, sizeof lta->subject, getx());
  for (int i = 0; i < n; i++) getlt(&lta->xforms[i]);
  for (int i = 0; i < n; i++) getlt(&lta->inv_xforms[i]);
  return lta;
}
static void putlta(const LTA *lta)
{
  if (!lta) {
    out("null");
    return;
  }
  puti(lta->num_xforms);
  puti(lta->type);
  putf(lta->fscale);
  puts_(lta->subject);
  for (int i = 0; i < lta->num_xforms; i++) putlt(&lta->xforms[i]);
  for (int i = 0; i < lta->num_xforms; i++) putlt(&lta->inv_xforms[i]);
}

static void put_volume(const MRI *m)
{
  puthdr(m);
  putidx(m);
  putframes(m);
  putvox(m);
}
static MATRIX *copy_or_null(const MATRIX *m) { return m ? MatrixCopy(m, NULL) : NULL; }

// ---- the commands (README.md, "Commands") ------------------------------------------------------
// ---- the volumes of these commands (README.md, "Encodings") ------------------------------------

// float voxels of a `t<seed>` fill: voxel i (of the whole buffer, frames included) is TABLE[(7 i +
// seed) mod 34]
static const uint32_t TABLE[] = {
    0x00000000, 0x80000000, 0x3f000000, 0xbf000000, 0x3f800000, 0xbf800000, 0x40200000, 0x437f8000,
    0x437e8000, 0x43800000, 0xbfc00000, 0x501502f9, 0xd01502f9, 0x7f7fffff, 0xff7fffff, 0x00000001,
    0x7f800000, 0xff800000, 0x7fc00000, 0xffc00000, 0x7fc12345, 0x7f800001, 0x40500000, 0x42c80000,
    0x1e3ce508, 0x477fff80, 0x46ffff00, 0xc7000080, 0x4f000000, 0x4f800000, 0x41200000, 0xc1200000,
    0x3dcccccd, 0x40490fdb};
static const size_t NTABLE = sizeof TABLE / sizeof TABLE[0];

// W: V, a fill (a seed: the bytes of fill(); t<seed>: float voxels from TABLE), the geometry's
// fields but its extent (xsize ysize zsize ras_good x_r .. c_s s_r s_a s_s), the header fields
// imnr0 imnr1 xstart xend ystart yend zstart zend thick ps tr te ti flip_angle(double) dof fov
// location ptype, the cache (0 as the allocation left it, 1 recomputed from the fields, 2 none),
// the registration matrix (a matrix or null), the colour table (or null)
static MRI *getw()
{
  MRI *m = getv();
  const std::string &f = tok();
  if (!f.empty() && f[0] == 't') {
    unsigned seed = (unsigned)strtoul(f.c_str() + 1, NULL, 10);
    if (has_voxels(m) && m->type == MRI_FLOAT) {
      unsigned char *b = (unsigned char *)m->chunk;
      for (size_t i = 0; i < m->bytes_total / 4; i++) memcpy(b + 4 * i, &TABLE[(7 * i + seed) % NTABLE], 4);
    }
    else
      fill(m, (int)seed);
  }
  else
    fill(m, (int)strtol(f.c_str(), NULL, 10));
  m->xsize = getf();
  m->ysize = getf();
  m->zsize = getf();
  m->ras_good_flag = geti();
  for (float *x : {&m->x_r, &m->x_a, &m->x_s, &m->y_r, &m->y_a, &m->y_s, &m->z_r, &m->z_a, &m->z_s, &m->c_r, &m->c_a,
                   &m->c_s, &m->s_r, &m->s_a, &m->s_s})
    *x = getf();
  m->imnr0 = geti();
  m->imnr1 = geti();
  for (float *x : {&m->xstart, &m->xend, &m->ystart, &m->yend, &m->zstart, &m->zend, &m->thick, &m->ps, &m->tr,
                   &m->te, &m->ti})
    *x = getf();
  m->flip_angle = getd();
  m->dof = geti();
  m->fov = getf();
  m->location = getf();
  m->ptype = geti();
  int cache = geti();
  if (cache == 1)
    MRIreInitCache(m);
  else if (cache == 2) {
    AffineMatrixFree(&m->i_to_r__);
    MatrixFree(&m->r_to_i__);
  }
  MATRIX *reg = getm();
  if (reg) {
    MatrixFree(&m->register_mat);
    m->register_mat = reg;
  }
  COLOR_TABLE *ct = getct();
  if (ct) m->ct = ct;
  return m;
}
static void putv(const MRI *m)
{
  if (!m) {
    out("null");
    return;
  }
  puthdr(m);
  putct(m->ct, true);
  putvox(m);
}
// D, the destination: new (none), self (the first source), other (the second), or a W
static MRI *getdst(MRI *a, MRI *b = NULL)
{
  if (take("new")) return NULL;
  if (take("self")) return a;
  if (take("other")) return b;
  return getw();
}
// the result: `self`, `other` or `given` when it is that volume, then its state; then the given
// destination's state when the result is not it
static void putres(const MRI *res, const MRI *a, const MRI *b, const MRI *given)
{
  if (res && res == a) out("self");
  else if (res && b && res == b) out("other");
  else if (res && res == given) out("given");
  putv(res);
  if (given && given != res && given != a && given != b) putv(given);
}
// voxels of frame 0 set (MRIsetVoxVal): n, then n times x y z value (a float)
static void paint(MRI *m)
{
  int n = geti();
  for (int i = 0; i < n; i++) {
    int x = geti(), y = geti(), z = geti();
    float v = getf();
    MRIsetVoxVal(m, x, y, z, 0, v);
  }
}
// the label tests of cma.h on one value, as 0/1 in this order (README.md, "cmalab")
template <typename L> static std::string labtests(L l)
{
  const bool t[] = {
      IS_FUTURE_WMSA(l), IS_LH_LAYER(l), IS_CC(l), IS_RH_LAYER(l), IS_LAYER(l), IS_UNKNOWN(l), IS_BRAIN(l),
      IS_WM(l), IS_CEREBRAL_WM(l), IS_HYPO(l), IS_WMSA(l), IS_WMH(l), IS_THALAMUS(l), IS_GM(l),
      IS_VENTRAL_DC(l), IS_CEREBELLAR_WM(l), IS_CEREBELLAR_GM(l), IS_CEREBELLUM(l), IS_CHOROID(l),
      IS_HIPPO(l), IS_AMYGDALA(l), IS_MTL(l), IS_CORTEX(l), IS_LAT_VENT(l), IS_CSF(l), IS_INF_LAT_VENT(l),
      IS_VENTRICLE(l), IS_CAUDATE(l), IS_PUTAMEN(l), IS_CLAUSTRUM(l), IS_ACCUMBENS(l), IS_PALLIDUM(l),
      LABEL_WITH_NO_TOPOLOGY_CONSTRAINT(l), IS_FIMBRIA(l), IS_GRAY_MATTER(l), IS_WHITE_MATTER(l),
      IS_FLUID(l), IS_LH_CLASS(l), IS_RH_CLASS(l), IS_GRAY_CLASS(l), IS_WHITE_CLASS(l), IS_CSF_CLASS(l),
      IS_CLASS(l, CSF_CLASS), IS_CLASS(l, GM_CLASS), IS_CLASS(l, WM_CLASS), IS_CLASS(l, OTHER_CLASS)};
  std::string b;
  for (bool x : t) b += x ? '1' : '0';
  return b;
}
// the transform file of `ltaread` and `ltareads`: written (unless `nofile`), read, removed
static void read_transform(const std::string &name, bool have, const std::string &bytes)
{
  unlink(name.c_str());
  if (have) {
    FILE *fp = fopen(name.c_str(), "wb");
    if (!fp) bad("cannot write the transform file");
    fwrite(bytes.data(), 1, bytes.size(), fp);
    fclose(fp);
  }
  errno = 0;
  LTA *lta = LTAreadEx(name.c_str());
  puti(errno);
  putlta(lta);
  if (have) unlink(name.c_str());
}
// the subject folder of `bvc` and `bvr`: subj/stats in the working directory, made or not
static void subject_folder(bool make)
{
  unlink("subj/stats/brainvol.stats");
  rmdir("subj/stats");
  rmdir("subj");
  if (make) {
    mkdir("subj", 0700);
    mkdir("subj/stats", 0700);
  }
}
// a region: x y z dx dy dz, or null
static MRI_REGION *getreg(MRI_REGION *r)
{
  if (take("null")) return NULL;
  r->x = geti();
  r->y = geti();
  r->z = geti();
  r->dx = geti();
  r->dy = geti();
  r->dz = geti();
  return r;
}

static const float UNSET = 7.0f;  // an output the call may leave
static const double UNSETD = 7.0;

// ---- the commands (README.md, "Commands") ------------------------------------------------------

static void call(const std::string &c)
{
  // part 1: the geometry's text block, copies, matrices, comparison, printer
  if (c == "vgwrite") {
    VOL_GEOM vg;
    getg(&vg);
    FILE *fp = fopen("vg.out", "wb");
    writeVolGeom(fp, &vg);
    fclose(fp);
    std::string s = slurp("vg.out");
    out(hexbytes(s.data(), s.size()));
  }
  else if (c == "vgread") {
    std::string text = getx();
    VOL_GEOM vg;
    getg(&vg);
    FILE *w = fopen("vg.in", "wb");
    fwrite(text.data(), 1, text.size(), w);
    fclose(w);
    FILE *fp = fopen("vg.in", "rb");
    errno = 0;
    readVolGeom(fp, &vg);
    int e = errno;
    putg(&vg);
    puti(ftell(fp));
    puti(feof(fp) != 0);
    puti(ferror(fp) != 0);
    puti(e);
    char b[64];
    size_t n = fread(b, 1, sizeof b, fp);
    out(hexbytes(b, n));
    fclose(fp);
  }
  else if (c == "vgprint") {
    VOL_GEOM vg;
    getg(&vg);
    vg_print(&vg);
  }
  else if (c == "getvg" || c == "copyvg") {
    VOL_GEOM a, b;
    getg(&a);
    getg(&b);
    if (c == "getvg")
      getVolGeom(&a, &b);
    else
      copyVolGeom(&a, &b);
    putg(&b);
  }
  else if (c == "vgi2r" || c == "vgr2i" || c == "tkrv2r") {
    VOL_GEOM vg;
    getg(&vg);
    putm(c == "vgi2r" ? vg_i_to_r(&vg) : c == "vgr2i" ? vg_r_to_i(&vg) : TkrVox2RASfromVolGeom(&vg));
  }
  else if (c == "vgeq") {
    VOL_GEOM a, b;
    getg(&a);
    getg(&b);
    double t = getd();
    puti(vg_isNotEqualThresh(&a, &b, t));
    puti(vg_isEqual(&a, &b));
  }
  // part 2: voxel arithmetic
  else if (c == "clone") {
    MRI *a = getw(), *d = getdst(a);
    putres(MRIclone(a, d), a, NULL, d);
  }
  else if (c == "clonesp") {
    MRI *a = getw();
    int type = geti(), nf = geti();
    putv(MRIcloneBySpace(a, type, nf));
  }
  else if (c == "clear") {
    MRI *a = getw();
    puti(MRIclear(a));
    putv(a);
  }
  else if (c == "match") {
    MRI *a = getw(), *b = getw();
    puti(MRImatch(a, b));
    puti(MRImatchDimensions(a, b));
  }
  else if (c == "chsize") {
    MRI *a = getw(), *b = take("null") ? NULL : getw();
    int w = geti(), h = geti(), d = geti();
    puti(MRIcheckSize(a, b, w, h, d));
  }
  else if (c == "cpframe") {
    MRI *a = getw(), *d = getdst(a);
    int sf = geti(), df = geti();
    putres(MRIcopyFrame(a, d, sf, df), a, NULL, d);
  }
  else if (c == "cpframes") {
    MRI *a = getw(), *d = getdst(a);
    int s0 = geti(), s1 = geti(), d0 = geti();
    putres(MRIcopyFrames(a, d, s0, s1, d0), a, NULL, d);
  }
  else if (c == "linscale") {
    MRI *a = getw(), *d = getdst(a);
    float s = getf(), o = getf();
    int nz = geti();
    putres(MRIlinearScale(a, d, s, o, nz), a, NULL, d);
  }
  else if (c == "smul") {
    MRI *a = getw(), *d = getdst(a);
    float s = getf();
    putres(MRIscalarMul(a, d, s), a, NULL, d);
  }
  else if (c == "smulf") {
    MRI *a = getw(), *d = getdst(a);
    float s = getf();
    int f = geti();
    putres(MRIscalarMulFrame(a, d, s, f), a, NULL, d);
  }
  else if (c == "thresh" || c == "uthresh" || c == "invc") {
    MRI *a = getw(), *d = getdst(a);
    float t = getf();
    MRI *r = c == "thresh" ? MRIthreshold(a, d, t) : c == "uthresh" ? MRIupperthresholdAllFrames(a, d, t)
                                                                      : MRIinvertContrast(a, d, t);
    putres(r, a, NULL, d);
  }
  else if (c == "binz") {
    MRI *a = getw(), *d = getdst(a);
    float t = getf(), lo = getf(), hi = getf();
    putres(MRIbinarize(a, d, t, lo, hi), a, NULL, d);
  }
  else if (c == "abs" || c == "pos" || c == "neg") {
    MRI *a = getw(), *d = getdst(a);
    putres(c == "abs" ? MRIabs(a, d) : c == "pos" ? MRIpos(a, d) : MRIneg(a, d), a, NULL, d);
  }
  else if (c == "mul2" || c == "sub2" || c == "add2" || c == "div2") {
    MRI *a = getw(), *b = getw(), *d = getdst(a, b);
    MRI *r = c == "mul2" ? MRImultiply(a, b, d)
             : c == "sub2" ? MRIsubtract(a, b, d)
             : c == "add2" ? MRIadd(a, b, d)
                           : MRIdivide(a, b, d);
    putres(r, a, b, d);
  }
  else if (c == "sam") {
    MRI *a = getw();
    float s = getf();
    MRI *b = getw(), *d = getdst(a, b);
    putres(MRIscaleAndMultiply(a, s, b, d), a, b, d);
  }
  else if (c == "vrange" || c == "vrangef" || c == "vrangereg") {
    MRI *a = getw();
    float lo = UNSET, hi = UNSET;
    int r;
    if (c == "vrange")
      r = MRIvalRange(a, &lo, &hi);
    else if (c == "vrangef")
      r = MRIvalRangeFrame(a, &lo, &hi, geti());
    else {
      MRI_REGION reg;
      r = MRIvalRangeRegion(a, &lo, &hi, getreg(&reg));
    }
    puti(r);
    putf(lo);
    putf(hi);
  }
  else if (c == "lrange") {
    MRI *a = getw(), *l = getw();
    int label = geti();
    float lo = UNSET, hi = UNSET;
    puti(MRIlabelValRange(a, l, label, &lo, &hi));
    putf(lo);
    putf(hi);
  }
  else if (c == "extract" || c == "extinto" || c == "extreg") {
    MRI *a = getw(), *d = getdst(a);
    MRI *r;
    if (c == "extreg") {
      MRI_REGION reg;
      r = MRIextractRegion(a, d, getreg(&reg));
    }
    else {
      int x0 = geti(), y0 = geti(), z0 = geti(), dx = geti(), dy = geti(), dz = geti();
      if (c == "extract")
        r = MRIextract(a, d, x0, y0, z0, dx, dy, dz);
      else {
        int x1 = geti(), y1 = geti(), z1 = geti();
        r = MRIextractInto(a, d, x0, y0, z0, dx, dy, dz, x1, y1, z1);
      }
    }
    putres(r, a, NULL, d);
  }
  else if (c == "cras") {
    MRI *a = getw(), *d = getw();
    int x0 = geti(), y0 = geti(), z0 = geti(), x1 = geti(), y1 = geti(), z1 = geti();
    double r = UNSETD, aa = UNSETD, s = UNSETD;
    MRIcalcCRASforExtractedVolume(a, d, x0, y0, z0, x1, y1, z1, &r, &aa, &s);
    putd(r);
    putd(aa);
    putd(s);
    putg(a, false);
  }
  // part 3: coordinates and geometry
  else if (c == "sras") {
    MRI *a = getw();
    int k = geti();
    MATRIX *m = k == 0 ? surfaceRASFromVoxel_(a)
                : k == 1 ? voxelFromSurfaceRAS_(a)
                : k == 2 ? GetSurfaceRASToVoxelMatrix(a)
                         : surfaceRASFromRAS_(a);
    putm(m);
    putg(a, false);
  }
  else if (c == "spt") {
    MRI *a = getw();
    int k = geti();
    double x = getd(), y = getd(), z = getd(), u = UNSETD, v = UNSETD, w = UNSETD;
    int r = k == 0 ? MRIRASToSurfaceRAS(a, x, y, z, &u, &v, &w)
            : k == 1 ? MRIvoxelToSurfaceRAS(a, x, y, z, &u, &v, &w)
                     : MRIsurfaceRASToVoxel(a, x, y, z, &u, &v, &w);
    puti(r);
    putd(u);
    putd(v);
    putd(w);
    putg(a, false);
  }
  else if (c == "ostr") {
    VOL_GEOM vg;
    getg(&vg);
    char s[8];
    memset(s, 'Z', sizeof s);
    puti(MRIdircosToOrientationString(&vg, s));
    out(hexbytes(s, 5));
    puts_(MRIsliceDirectionName(&vg));
    puti(getSliceDirection(&vg));
  }
  else if (c == "vtal") {
    MRI *a = getw();
    getxfm(a);
    double x = getd(), y = getd(), z = getd(), u = UNSETD, v = UNSETD, w = UNSETD;
    puti(MRIvoxelToTalairach(a, x, y, z, &u, &v, &w));
    putd(u);
    putd(v);
    putd(w);
    putg(a, false);
  }
  else if (c == "inidx") {
    MRI *a = getw();
    double x = getd(), y = getd(), z = getd();
    puti(MRIindexNotInVolume(a, x, y, z));
  }
  else if (c == "snear") {
    MRI *a = getw();
    double x = getd(), y = getd(), z = getd();
    a->outside_val = getd();
    double v = UNSETD;
    puti(MRIsampleVolumeType(a, x, y, z, &v, SAMPLE_NEAREST));
    putd(v);
  }
  else if (c == "com") {
    MRI *a = getw();
    int t = geti();
    double means[3] = {UNSETD, UNSETD, UNSETD};
    puti(MRIcenterOfMass(a, means, (BUFTYPE)t));
    for (double x : means) putd(x);
  }
  else if (c == "pca") {
    MRI *a = getw();
    int t = geti();
    MATRIX *ev = MatrixAlloc(3, 3, MATRIX_REAL);
    float evalues[3] = {UNSET, UNSET, UNSET};
    double means[3] = {UNSETD, UNSETD, UNSETD};
    puti(MRIprincipleComponents(a, ev, evalues, means, (BUFTYPE)t));
    putm(ev);
    for (float x : evalues) putf(x);
    for (double x : means) putd(x);
  }
  else if (c == "avg") {
    MRI *a = getw();
    int dof = geti();
    MRI *d = getdst(a);
    putres(MRIaverage(a, dof, d), a, NULL, d);
  }
  else if (c == "cmph") {
    MRI *a = take("null") ? NULL : getw();
    MRI *b = take("null") ? NULL : take("self") ? a : getw();
    puti(MRIcompareHeaders(a, b));
  }
  else if (c == "vdet") {
    MRI *a = getw();
    putf(MRIvolumeDeterminant(a));
  }
  else if (c == "dump") {
    MRI *a = getw();
    puti(MRIdump(a, stdout));
  }
  else if (c == "dsign") {
    putd(MRIsetSampleVolumeDerivativeSign(getd()));
  }
  // ---- part 4 (the spec's §18 to §21)
  else if (c == "samp") {
    std::string k = tok();
    MRI *a = getw();
    a->outside_val = getd();
    int n = geti();
    for (int i = 0; i < n; i++) {
      double x = getd(), y = getd(), z = getd();
      int f = geti(), aux = geti();
      double v = UNSETD;
      int r;
      if (k == "tri") r = MRIsampleVolume(a, x, y, z, &v);
      else if (k == "typ") r = MRIsampleVolumeType(a, x, y, z, &v, aux);
      else if (k == "frm") r = MRIsampleVolumeFrame(a, x, y, z, f, &v);
      else if (k == "fty") r = MRIsampleVolumeFrameType(a, x, y, z, f, aux, &v);
      else if (k == "cub") r = MRIcubicSampleVolume(a, x, y, z, &v);
      else if (k == "cubf") r = MRIcubicSampleVolumeFrame(a, x, y, z, f, &v);
      else if (k == "sinc") r = MRIsincSampleVolume(a, x, y, z, aux, &v);
      else if (k == "sincf") r = MRIsincSampleVolumeFrame(a, x, y, z, f, aux, &v);
      else bad("unknown sampler");
      puti(r);
      putd(v);
    }
  }
  else if (c == "coef" || c == "hsinc") {
    int n = geti();
    for (int i = 0; i < n; i++) {
      double x = getd();
      if (c == "coef") putd(MRIcubicCoeff(x, geti()));
      else putd(ham_sinc(x, getd()));
    }
  }
  else if (c == "trans") {
    MRI *a = getw();
    MRI *d = getdst(a);
    double dx = getd(), dy = getd(), dz = getd();
    putres(MRItranslate(a, d, dx, dy, dz), a, NULL, d);
  }
  else if (c == "rot") {
    MRI *a = getw();
    MRI *d = getdst(a);
    MATRIX *mr = getm();
    MATRIX *mo = getm();
    putres(MRIrotate_I(a, d, mr, mo), a, NULL, d);
  }
  else if (c == "rotx" || c == "roty" || c == "rotz") {
    MRI *a = getw();
    MRI *d = getdst(a);
    float angle = getf();
    MRI *r = c == "rotx" ? MRIrotateX_I(a, d, angle) : c == "roty" ? MRIrotateY_I(a, d, angle) : MRIrotateZ_I(a, d, angle);
    putres(r, a, NULL, d);
  }
  else if (c == "skull") {
    MRI *a = getw();
    int t = geti();
    int n = geti();
    for (int i = 0; i < n; i++) {
      int x0 = geti(), y0 = geti(), z0 = geti(), x1 = geti(), y1 = geti(), z1 = geti();
      float v = getf();
      for (int z = z0; z <= z1; z++)
        for (int y = y0; y <= y1; y++)
          for (int x = x0; x <= x1; x++) MRIsetVoxVal(a, x, y, z, 0, v);
    }
    MRI_REGION box;
    box.x = box.y = box.z = box.dx = box.dy = box.dz = -7;
    puti(MRIfindApproximateSkullBoundingBox(a, t, &box));
    for (int v : {box.x, box.y, box.z, box.dx, box.dy, box.dz}) puti(v);
  }
  else if (c == "tftype") {
    int n = geti();
    for (int i = 0; i < n; i++) puti(TransformFileNameType(getx().c_str()));
  }
  else if (c == "ltaread") {
    std::string name = getx();
    bool have = !take("nofile");
    std::string bytes = have ? getx() : std::string();
    int avg = geti();  // USE_AVERAGE305: 0 unset, 1 set to "true", 2 set and empty
    if (avg) setenv("USE_AVERAGE305", avg == 1 ? "true" : "", 1);
    else unsetenv("USE_AVERAGE305");
    read_transform(name, have, bytes);
  }
  else if (c == "ltareads") {
    int n = geti();
    std::vector<std::string> names, texts;
    std::vector<bool> haves;
    for (int i = 0; i < n; i++) {
      names.push_back(getx());
      haves.push_back(!take("nofile"));
      texts.push_back(haves.back() ? getx() : std::string());
    }
    int avg = geti();
    if (avg) setenv("USE_AVERAGE305", avg == 1 ? "true" : "", 1);
    else unsetenv("USE_AVERAGE305");
    for (int i = 0; i < n; i++) read_transform(names[i], haves[i], texts[i]);
  }
  else if (c == "cmalab") {
    int n = geti();
    for (int i = 0; i < n; i++) {
      int v = geti();
      float f = getf();
      out(labtests(v));
      out(labtests(f));
      out(labtests((double)f));
      puti(IsSubCorticalGray(v));
      puti(MRIasegContraLatLabel(v));
    }
  }
  else if (c == "cmavol") {
    MRI *aseg = getw();
    paint(aseg);
    MRI *ribbon = getw();
    paint(ribbon);
    int hemi = geti();
    putd(SupraTentorialVolCorrection(aseg, ribbon));
    putd(CorticalGMVolCorrection(aseg, ribbon, hemi));
  }
  else if (c == "lrswap") {
    MRI *aseg = getw();
    paint(aseg);
    putv(MRIlrswapAseg(aseg));
  }
  else if (c == "fixrib") {
    MRI *aseg = getw();
    paint(aseg);
    MRI *ribbon = getw();
    paint(ribbon);
    MRI *d = getdst(aseg, ribbon);
    putres(MRIfixAsegWithRibbon(aseg, ribbon, d), aseg, ribbon, d);
  }
  else if (c == "lrseg") {
    MRI *seg = getw();
    paint(seg);
    MATRIX *lh = getm();
    MATRIX *rh = getm();
    putv(MRIlrswapSeg(seg, lh, rh));
  }
  else if (c == "ohe") {
    MRI *seg = getw();
    paint(seg);
    int n = geti();
    std::vector<int> list;
    for (int i = 0; i < n; i++) list.push_back(geti());
    int force1 = geti();
    putv(MRIoneHotEncode(seg, list, force1));
  }
  else if (c == "bvc") {
    subject_folder(geti() != 0);
    std::vector<double> stats;
    for (int i = 0; i < 16; i++) stats.push_back(getd());
    CacheBrainVolumeStats(stats, "subj", ".");
    FILE *fp = fopen("subj/stats/brainvol.stats", "rb");
    if (fp) {
      fclose(fp);
      std::string b = slurp("subj/stats/brainvol.stats");
      out(hexbytes(b.data(), b.size()));
    }
    else
      out("null");
    subject_folder(false);
  }
  else if (c == "bvr") {
    bool make = geti() != 0;
    std::string text = getx();
    subject_folder(make);
    if (make) {
      FILE *fp = fopen("subj/stats/brainvol.stats", "wb");
      if (!fp) bad("cannot write the statistics file");
      fwrite(text.data(), 1, text.size(), fp);
      fclose(fp);
    }
    std::vector<double> stats = ReadCachedBrainVolumeStats("subj", ".");
    puti((long long)stats.size());
    for (double v : stats) putd(v);
    subject_folder(false);
  }
  // part 6: resolution, planes, fills, resampling by whole factors, changes of type
  else if (c == "setres") {
    MRI *a = getw();
    float x = getf(), y = getf(), z = getf();
    puti(MRIsetResolution(a, x, y, z));
    putv(a);
  }
  else if (c == "vfill") {
    MRI *a = getw();
    puti(MRIvalueFill(a, getf()));
    putv(a);
  }
  else if (c == "fillfg") {
    MRI *a = getw(), *d = getdst(a);
    int sx = geti(), sy = geti(), sz = geti(), th = geti(), fv = geti();
    int npix = -7;
    MRI *r = MRIfillFG(a, d, sx, sy, sz, th, fv, &npix);
    puti(npix);
    putres(r, a, NULL, d);
  }
  else if (c == "explane") {
    MRI *a = getw(), *d = getdst(a);
    int o = geti(), w = geti();
    putres(MRIextractPlane(a, d, o, w), a, NULL, d);
  }
  else if (c == "fillplane") {
    MRI *mask = getw(), *d = getw();
    int o = geti(), w = geti(), fv = geti();
    putres(MRIfillPlane(mask, d, o, w, fv), mask, NULL, d);
  }
  else if (c == "ups") {
    int k = geti();  // 0 MRIupsampleN, 1 MRIupsample2, 2 MRIupsampleNConserve
    MRI *a = getw(), *d = getdst(a);
    int n = geti();
    MRI *r = k == 0 ? MRIupsampleN(a, d, n) : k == 1 ? MRIupsample2(a, d) : MRIupsampleNConserve(a, d, n);
    putres(r, a, NULL, d);
  }
  else if (c == "dns2") {
    MRI *a = getw(), *d = getdst(a);
    putres(MRIdownsample2(a, d), a, NULL, d);
  }
  else if (c == "dnsn") {
    MRI *a = getw(), *d = getdst(a);
    float fc = getf(), fr = getf(), fs = getf();
    int keep = geti();
    putres(MRIdownsampleN(a, d, fc, fr, fs, keep), a, NULL, d);
  }
  else if (c == "mft") {
    MRI *a = getw();
    int f = geti();
    putd(MRImeanFrameThresh(a, f, getf()));
  }
  else if (c == "lim") {
    MRI *a = getw();
    float lo = UNSET, hi = UNSET;
    puti(MRIlimits(a, &lo, &hi));
    putf(lo);
    putf(hi);
  }
  else if (c == "chtype") {
    int seq = geti();  // 0 MRIchangeType, 1 MRISeqchangeType
    MRI *a = getw();
    int t = geti();
    float lo = getf(), hi = getf();
    int ns = geti();
    if (geti())
      setenv("FS_FORCE_BIN_CHECK", "1", 1);
    else
      unsetenv("FS_FORCE_BIN_CHECK");
    MRI *r = seq ? MRISeqchangeType(a, t, lo, hi, ns) : MRIchangeType(a, t, lo, hi, ns);
    putres(r, a, NULL, NULL);
    if (r != a) putv(a);
  }
  // part 6b: frame-range samplers, the trilinear kernel and splatting; derivatives, gradients and
  // neighbour counts; linear transforms of a volume; part 6c: resampling onto a template, replacing
  // a value
  else if (c == "splat") {
    MRI *a = getw();
    int n = geti();
    for (int i = 0; i < n; i++) {
      int k = geti();
      double x = getd(), y = getd(), z = getd();
      int f = geti();
      double v = getd();
      puti(k ? MRIinterpolateIntoVolumeFrame(a, x, y, z, f, v) : MRIinterpolateIntoVolume(a, x, y, z, v));
    }
    putv(a);
  }
  else if (c == "sseq" || c == "sxyz") {
    MRI *a = getw();
    a->outside_val = getd();
    int n = geti();
    for (int i = 0; i < n; i++) {
      // the values' array has SLACK unset slots before index 0 and after the last index printed
      const int SLACK = 8;
      int r, lo, cnt;
      std::vector<float> buf;
      if (c == "sseq") {
        double x = getd(), y = getd(), z = getd();
        int first = geti(), last = geti(), type = geti();
        lo = first;
        cnt = last >= first ? last - first + 1 : 0;
        buf.assign((size_t)(SLACK + (last > 0 ? last : 0) + 1 + SLACK), UNSET);
        float *v = buf.data() + SLACK;
        r = type == -1 ? MRIsampleSeqVolume(a, x, y, z, v, first, last)
                       : MRIsampleSeqVolumeType(a, x, y, z, v, first, last, type);
        fflush(stdout);
        puti(r);
        for (int f = 0; f < cnt; f++) putf(v[lo + f]);
      }
      else {
        int x = geti(), y = geti(), z = geti(), begin = geti(), end = geti(), k = geti();
        cnt = end > begin ? end - begin : 0;
        buf.assign((size_t)(2 * SLACK + cnt), UNSET);
        float *v = buf.data() + SLACK;
        r = k ? MRIsampleVolumeFrame_xyzInt_nRange_floats(a, x, y, z, begin, end, v)
              : MRIsampleVolumeFrameType_xyzInt_nRange_SAMPLE_NEAREST_floats(a, x, y, z, begin, end, v);
        puti(r);
        for (int f = 0; f < cnt; f++) putf(v[f]);
      }
    }
  }
  else if (c == "tkern") {
    MRI *a = getw();
    int n = geti();
    for (int i = 0; i < n; i++) {
      double x = getd(), y = getd(), z = getd();
      bool given = geti() == 0;
      double k[8];
      for (double &w : k) w = UNSETD;
      double *r = MRItrilinKernel(a, x, y, z, given ? k : NULL);
      out(r == k ? "given" : "new");
      for (int j = 0; j < 8; j++) putd(r[j]);
    }
  }
  else if (c == "deriv") {
    MRI *a = getw();
    MRIsetSampleVolumeDerivativeSign(getd());
    int n = geti();
    for (int i = 0; i < n; i++) {
      int k = geti();
      double x = getd(), y = getd(), z = getd(), dx = getd(), dy = getd(), dz = getd(), sigma = getd();
      double mag = UNSETD;
      puti(k ? MRIsampleVolumeDerivativeScale(a, x, y, z, dx, dy, dz, &mag, sigma)
             : MRIsampleVolumeDerivative(a, x, y, z, dx, dy, dz, &mag));
      putd(mag);
    }
  }
  else if (c == "grad") {
    MRI *a = getw();
    int n = geti();
    for (int i = 0; i < n; i++) {
      int k = geti();
      double x = getd(), y = getd(), z = getd();
      int f = geti();
      double gx = UNSETD, gy = UNSETD, gz = UNSETD;
      puti(k ? MRIsampleVolumeGradientFrame(a, x, y, z, &gx, &gy, &gz, f)
             : MRIsampleVolumeGradient(a, x, y, z, &gx, &gy, &gz));
      putd(gx);
      putd(gy);
      putd(gz);
    }
  }
  else if (c == "nbrs") {
    MRI *a = getw();
    int n = geti();
    for (int i = 0; i < n; i++) {
      int k = geti(), x = geti(), y = geti(), z = geti(), p = geti(), q = geti();
      float lo = getf(), hi = getf();
      int r;
      if (k == 0) r = MRIneighborsOn(a, x, y, z, p);
      else if (k == 1) r = MRIneighborsOn3x3(a, x, y, z, p);
      else if (k == 2) r = MRIneighborsInWindow(a, x, y, z, p, q);
      else if (k == 3) r = MRIneighbors(a, x, y, z, p);
      else if (k == 4) r = MRIneighborsInRange(a, x, y, z, p, lo, hi);
      else if (k == 5) r = MRIneighbors3x3(a, x, y, z, p);
      else if (k == 6) r = MRIneighborsOff(a, x, y, z, p);
      else if (k == 7) r = MRIneighborsOff3x3(a, x, y, z, p);
      else bad("unknown neighbour count");
      puti(r);
    }
  }
  else if (c == "setvals") {
    MRI *a = getw();
    puti(MRIsetValues(a, getf()));
    putv(a);
  }
  else if (c == "rasvox") {
    MRI *a = getw();
    MRI *b = take("null") ? NULL : getw();
    MATRIX *m = getm();
    putm(MRIrasXformToVoxelXform(a, b, m, NULL));
  }
  else if (c == "lxf") {
    int k = geti();
    MRI *a = getw();
    a->outside_val = getd();
    MRI *d = getdst(a);
    MATRIX *m = getm();
    int interp = geti();
    MRI *r;
    if (k == 0) r = MRIlinearTransformInterp(a, d, m, interp);
    else if (k == 1) r = MRIlinearTransform(a, d, m);
    else if (k == 2) r = MRIinverseLinearTransform(a, d, m);
    else if (k == 3) r = MRIapplyRASlinearTransformInterp(a, d, m, interp);
    else if (k == 4) r = MRIapplyRASlinearTransform(a, d, m);
    else if (k == 5) r = MRIapplyRASinverseLinearTransform(a, d, m);
    else bad("unknown transform function");
    fflush(stdout);
    putres(r, a, NULL, d);
  }
  else if (c == "rsmat") {
    MRI *a = getw(), *b = getw();
    MATRIX *m = MRIgetResampleMatrix(a, b);
    fflush(stdout);
    putm(m);
  }
  else if (c == "rsfill") {
    MRI *a = getw(), *b = getw();
    int t = geti();
    float fv = getf();
    bool plain = geti() != 0;
    MRI *r = plain ? MRIresample(a, b, t) : MRIresampleFill(a, b, t, fv);
    fflush(stdout);
    putres(r, a, b, NULL);
  }
  else if (c == "repval") {
    MRI *a = getw(), *d = getdst(a);
    MRI *r;
    if (geti()) {
      int from = geti(), to = geti();
      r = MRIreplaceValuesUchar(a, d, (BUFTYPE)from, (BUFTYPE)to);
    }
    else {
      float from = getf(), to = getf();
      r = MRIreplaceValues(a, d, from, to);
    }
    putres(r, a, NULL, d);
  }
  // ---- part 7a (specs/freesurfer-volume.md §33) ----
  else if (c == "stats" || c == "pstats") {
    MRI *a = getw();
    if (c == "pstats") {
      puti(MRIprintStats(a, stdout));
      fflush(stdout);
    }
    else {
      float mn = UNSET, mx = UNSET, mean = UNSET, sd = UNSET;
      int n = -7;
      puti(MRIstats(a, &mn, &mx, &n, &mean, &sd));
      putf(mn);
      putf(mx);
      puti(n);
      putf(mean);
      putf(sd);
    }
  }
  else if (c == "type2s") {
    int n = geti();
    for (int i = 0; i < n; i++) puts_(MRItype2str(geti()));
  }
  else if (c == "cropbb") {
    MRI *a = getw();
    MRI_REGION box;
    getreg(&box);
    puti(MRIcropBoundingBox(a, &box));
    for (int v : {box.x, box.y, box.z, box.dx, box.dy, box.dz}) puti(v);
  }
  else if (c == "crass") {
    MRI *a = getw(), *b = getw();
    double r = UNSETD, aa = UNSETD, ss = UNSETD;
    MRIcalcCRASforSampledVolume(a, b, &r, &aa, &ss);
    fflush(stderr);
    putd(r);
    putd(aa);
    putd(ss);
    putg(a, false);
  }
  else if (c == "chkostr" || c == "o2dc") {
    MRI *a = c == "o2dc" ? getw() : NULL;
    std::string t = getx();
    // the string in an allocation of its own length and terminator, so that a read past it is a read
    // past the allocation (the guard runs)
    char *str = (char *)malloc(t.size() + 1);
    memcpy(str, t.data(), t.size());
    str[t.size()] = 0;
    if (c == "chkostr") {
      char *e = MRIcheckOrientationString(str);
      puts_(e);
    }
    else {
      puti(MRIorientationStringToDircos(a, str));
      fflush(stdout);
      for (float f : {a->x_r, a->x_a, a->x_s, a->y_r, a->y_a, a->y_s, a->z_r, a->z_a, a->z_s}) putf(f);
    }
  }
  else if (c == "s44") {
    int k = geti();
    MATRIX *m = NULL;
    if (k == 1) m = MatrixAlloc(4, 4, MATRIX_REAL);
    else if (k == 2) {
      int rows = geti(), cols = geti();
      m = MatrixAlloc(rows, cols, MATRIX_REAL);
    }
    else if (k == 3)
      m = MatrixAlloc(4, 4, MATRIX_COMPLEX);
    float v[16];
    for (float &f : v) f = getf();
    puti(stuff_four_by_four(m, v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8], v[9], v[10], v[11], v[12],
                            v[13], v[14], v[15]));
    if (m && m->type == MATRIX_COMPLEX)
      for (int i = 0; i < 32; i++) putf(m->data[i]);
    else
      putm(m);
  }
  else if (c == "vx2rx") {
    MRI *a = getw();
    MRI *b = take("null") ? NULL : getw();
    MATRIX *mv = getm(), *mr = getm();
    MATRIX *r = MRIvoxelXformToRasXform(a, b, mv, mr);
    putm_res(r, mr);
  }
  else if (c == "v2v") {
    MRI *a = getw(), *b = getw();
    putm(MRIgetVoxelToVoxelXform(a, b));
  }
  else if (c == "setv2r") {
    MRI *a = getw();
    MATRIX *m = getm();
    if (take("unset")) unsetenv("FS_SetVoxToRasXform_Change_VoxSize");
    else setenv("FS_SetVoxToRasXform_Change_VoxSize", getx().c_str(), 1);
    puti(MRIsetVoxelToRasXform(a, m));
    fflush(stdout);
    putg(a, false);
  }
  // ---- fs.bspline (specs/freesurfer-bspline.md) ----
  else if (c == "bsp") {
    int k = geti();
    MRI *a = getw();
    int deg = geti();
    MRI_BSPLINE *b = NULL;
    if (k == 1) b = MRIallocBSpline(a->width, a->height, a->depth, a->nframes);
    else if (k == 2) {
      b = MRIallocBSpline(1, 1, 1, 1);
      MRIfree(&b->coeff);
      b->coeff = getw();
    }
    else if (k == 3) {
      b = MRIallocBSpline(1, 1, 1, 1);
      MRIfree(&b->coeff);
    }
    else if (k != 0)
      bad("unknown B-spline kind");
    MRI_BSPLINE *r = MRItoBSpline(a, b, deg);
    fflush(stdout);
    out(r == b && b ? "given" : "new");
    puti(r->degree);
    puti(r->srctype);
    puti(r->srcneg);
    put_volume(r->coeff);
  }
  else if (c == "bsamp" || c == "bsraw") {
    MRI_BSPLINE *b;
    if (c == "bsamp") {
      int deg = geti();
      MRI *a = getw();
      a->outside_val = getd();
      b = MRItoBSpline(a, NULL, deg);
    }
    else {
      int deg = geti(), neg = geti();
      MRI *a = getw();
      b = MRIallocBSpline(a->width, a->height, a->depth, a->nframes);
      b->degree = deg;
      b->srctype = a->type;
      b->srcneg = neg;
      if (has_voxels(a) && a->type == MRI_FLOAT) memcpy(b->coeff->chunk, a->chunk, a->bytes_total);
      b->coeff->outside_val = getd();
    }
    fflush(stdout);
    int n = geti();
    for (int i = 0; i < n; i++) {
      double x = getd(), y = getd(), z = getd();
      int f = geti();
      double v = UNSETD;
      puti(MRIsampleBSpline(b, x, y, z, f, &v));
      putd(v);
    }
  }
  else if (c == "bsseq") {
    int deg = geti();
    MRI *a = getw();
    a->outside_val = getd();
    MRI_BSPLINE *b = MRItoBSpline(a, NULL, deg);
    fflush(stdout);
    int n = geti();
    for (int i = 0; i < n; i++) {
      double x = getd(), y = getd(), z = getd();
      int first = geti(), last = geti();
      if (first < 0) bad("a first frame below 0 writes before the array");
      std::vector<float> vals((size_t)(std::max(first, last) + 1 > 0 ? std::max(first, last) + 1 : 1), UNSET);
      puti(MRIsampleSeqBSpline(b, x, y, z, vals.data(), first, last));
      for (int f = first; f <= last; f++) putf(vals[(size_t)f]);
    }
  }
  else if (c == "bslxf" || c == "bsras" || c == "bslta") {
    int deg = geti();
    MRI *a = getw();
    MRI *d = getdst(a);
    MRI *r;
    if (c == "bslta") {
      LTA *lta = getlta();
      int avg = geti();
      if (avg) setenv("USE_AVERAGE305", avg == 1 ? "true" : "", 1);
      else unsetenv("USE_AVERAGE305");
      MRI_BSPLINE *b = MRItoBSpline(a, NULL, deg);
      fflush(stdout);
      r = LTAtransformBSpline(b, d, lta);
    }
    else {
      MATRIX *m = getm();
      MRI_BSPLINE *b = MRItoBSpline(a, NULL, deg);
      fflush(stdout);
      r = c == "bslxf" ? MRIlinearTransformBSpline(b, d, m) : MRIapplyRASlinearTransformBSpline(b, d, m);
    }
    fflush(stdout);
    putres(r, a, NULL, d);
  }
  else if (c == "bsdown" || c == "bsup") {
    MRI *a = getw(), *d = getdst(a);
    MRI *r = c == "bsdown" ? MRIdownsample2BSpline(a, d) : MRIupsample2BSpline(a, d);
    fflush(stdout);
    putres(r, a, NULL, d);
  }
  else if (c == "bsfree") {
    int k = geti();
    MRI_BSPLINE *b = NULL;
    if (k >= 1) b = MRIallocBSpline(1, 1, 1, 1);
    if (k == 1) MRIfree(&b->coeff);
    int r = MRIfreeBSpline(&b);
    puti(r);
    out(b ? "kept" : "none");
  }
  else
    bad("unknown command");
  if (P != T.size()) bad("extra arguments");
}

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
  const char *fo = ".fs-volume-probe.out", *fe = ".fs-volume-probe.err";
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
    // standard error byte-oriented, as in a program that has written to it: the C library's perror
    // then leaves errno as it was (specs/freesurfer-io.md §46)
    fwide(stderr, -1);
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
  std::string so = slurp(fo), se = slurp(fe);
  remove(fo);
  remove(fe);
  return res + " | " + hexbytes(so.data(), so.size()) + " " + hexbytes(se.data(), se.size());
}

int main(int argc, char **argv)
{
  if (argc != 3 || strcmp(argv[1], "batch") != 0) {
    fprintf(stderr, "usage: fs_volume_probe batch <file> (README.md)\n");
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
