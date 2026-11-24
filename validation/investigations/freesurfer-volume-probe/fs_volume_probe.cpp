// SPDX-License-Identifier: Apache-2.0
// A black-box probe of FreeSurfer 8.2.0's volume functions (specs/freesurfer-volume.md, parts 1 to 5:
// the volume-geometry text block and its copies, matrices, comparison and printer; the voxel
// arithmetic; coordinates, orientation, centre of mass, principal components, the running average,
// the header comparison and dump; the samplers, moves, the head's box and the transform reader; the
// CMA label tests and segmentation functions; part 6a: resolution, planes, fills, resampling by whole
// factors, changes of type; part 6b: frame-range samplers, the trilinear kernel and splatting,
// derivatives, gradients, neighbour counts, linear transforms; part 6c: resampling onto a template,
// replacing a value; part 7a: statistics, orientation strings, voxel-to-RAS matrices; part 7b: slices
// as 2-D images, neighbourhoods and labels; a transform array's change of type and its other
// operations (§54); segmentations onto another grid, SPM warps, segment borders and statistics,
// concatenation, edits, shifts, the entorhinal fix, the amygdala-cortex junction, crops (§55 to
// §57); volume-to-volume resampling, frame covariance and principal components, the false discovery
// rate, regions and voxel neighbourhood measures (part 9, §41 to §44); the Talairach helpers, dilation
// and segment helpers, conforming, set operations, morphology, masks and counts, and the volume
// histograms (part 10, §46 to §52); filling a coarser grid, bright voxels outside the white matter,
// voxel topology and the binary-volume classes (part 11a, §59 and §60); the rotation and scale of a
// transform (§64) and voxel lists of a region (§65); fs.bspline: the
// B-spline coefficients, their sampling, transforms and factor-of-two pyramids,
// specs/freesurfer-bspline.md), linked against the unit oracle's libutils.a (GCC 8.4.0,
// FreeSurfer's flags). It holds only calls of FreeSurfer's functions (and of the C library's) by name
// and the reading and printing of their arguments, results and the structures' fields: no FreeSurfer
// code. The commands and encodings are in README.md beside this file (the volume, geometry and header
// encodings are those of the fs.mri probe, ../freesurfer-mri-probe/); a candidate program takes the
// same ones.
//
//   fs_volume_probe batch <file>   one call per line of <file>, each in a child process of its own
//                                  with an alarm; one result line per call on standard output
#include <dirent.h>
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
#include "image.h"
#include "mri_circulars.h"
#include "colortab.h"
#include "diag.h"
#include "error.h"
#include "matrix.h"
#include "mri.h"
#include "mri2.h"
#include "mri_conform.h"
#include "region.h"
#include "talairachex.h"
#include "resample.h"
#include "transform.h"
#include "utils.h"
#include "voxlist.h"
#include "mriTransform.h"

// functions of FreeSurfer's library that its headers do not declare
int MRIcopyVolGeomToMRI(MRI *mri, const VOL_GEOM *vg);
int MRIcubicSampleVolumeFrame(MRI *mri, double x, double y, double z, int frame, double *pval);
int MRIsincSampleVolumeFrame(MRI *mri, double x, double y, double z, int frame, int hw, double *pval);
double ham_sinc(double x, double fullwidth);
MRI *LTAinverseTransformInterp(MRI *mri_src, MRI *mri_dst, LTA *lta, int interp);
// the threshold of vg_isEqual (a global of FreeSurfer's library)
extern double vg_isEqual_Threshold;

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
  if (!f.empty() && f[0] == 'i') {
    // i<a>,<b>,...: the buffer's 32-bit words from the list in order, repeated
    std::vector<uint32_t> w;
    for (const char *q = f.c_str() + 1; *q;) {
      char *e;
      w.push_back((uint32_t)strtoll(q, &e, 10));
      q = *e == ',' ? e + 1 : e;
      if (e == q && *e) bad("a bad i fill");
    }
    if (has_voxels(m) && !w.empty()) {
      unsigned char *b = (unsigned char *)m->chunk;
      for (size_t i = 0; i + 4 <= m->bytes_total; i += 4) memcpy(b + i, &w[(i / 4) % w.size()], 4);
    }
  }
  else if (!f.empty() && f[0] == 't') {
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
// a volume as putv prints it, but its voxels as a 64-bit FNV-1a hash of their bytes (h and 16 hex digits):
// the start state 1469598103934665603 (FNV's usual offset basis without its last digit), the usual prime
static void putvh(const MRI *m)
{
  if (!m) {
    out("null");
    return;
  }
  puthdr(m);
  putct(m->ct, true);
  if (!has_voxels(m)) {
    out("novox");
    return;
  }
  uint64_t h = 1469598103934665603ULL;
  const unsigned char *b = (const unsigned char *)m->chunk;
  for (size_t i = 0; i < m->bytes_total; i++) {
    h ^= b[i];
    h *= 1099511628211ULL;
  }
  char s[24];
  snprintf(s, sizeof s, "h%016" PRIx64, h);
  out(s);
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

// histograms (README.md, "H"; the fs.maths probe's encoding): null, or <alloc> <nbins> <bin_size> <min> <max>
// <nbins bin values> <nbins counts> (floats but the first two): HISTOalloc(alloc), then nbins and the fields set
static HISTOGRAM *geth()
{
  if (take("null")) return NULL;
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
// a histogram: null, or nbins max_bins bin_size min max, then the values and the counts of its first
// min(nbins, max_bins) bins
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
  int n = std::min(h->nbins, h->max_bins);
  for (int b = 0; b < n; b++) putf(h->bins[b]);
  for (int b = 0; b < n; b++) putf(h->counts[b]);
}
// the returned histogram (`given` when it is the given one), then the given one's state
static void puth_res(const HISTOGRAM *res, const HISTOGRAM *given)
{
  if (given && res == given)
    out("given");
  else
    puth(res);
  if (given) puth(given);
}

static const float UNSET = 7.0f;  // an output the call may leave
static const double UNSETD = 7.0;

// images (README.md, "I"): null, or <rows> <cols> <format> <frames> (ImageAlloc), <xsize> <ysize>,
// a fill (a seed: byte i of the pixels is (seed + 101 i) mod 256; -1 as allocated; t<seed>: float i
// is TABLE[(7 i + seed) mod 34]), then <orows> <ocols> <rows> <cols> <num_frame> written over the
// header (-1 keeps the allocation's). A given image is marked in a field the image functions do not
// use, so that a new image allocated where a freed given one was is not taken for it.
static const int GIVEN_MARK = 0x5a17;
static size_t img_bytes(const IMAGE *I)
{
  return I->image ? (size_t)I->sizeimage * (size_t)(I->num_frame > 0 ? I->num_frame : 0) : 0;
}
static void putimg(const IMAGE *I, size_t bytes)
{
  for (int v : {I->orows, I->ocols, I->rows, I->cols, I->pixel_format, I->num_frame}) puti(v);
  putf(I->xsize);
  putf(I->ysize);
  out(I->image ? hexbytes((const char *)I->image, bytes) : "null");
}
// the result of a call that may return the given image, a new one or none
static void putimg_res(const IMAGE *r, const IMAGE *given, size_t given_bytes)
{
  if (!r) {
    out("null");
    return;
  }
  bool g = given && r == given && r->numcolor == GIVEN_MARK;
  out(g ? "given" : "new");
  putimg(r, g ? given_bytes : img_bytes(r));
}
static IMAGE *getimg(size_t *bytes)
{
  *bytes = 0;
  if (take("null")) return NULL;
  int rows = geti(), cols = geti(), fmt = geti(), nf = geti();
  IMAGE *I = ImageAlloc(rows, cols, fmt, nf);
  *bytes = img_bytes(I);
  I->xsize = getf();
  I->ysize = getf();
  const std::string &f = tok();
  unsigned char *b = I->image;
  if (b && !f.empty() && f[0] == 't') {
    unsigned seed = (unsigned)strtoul(f.c_str() + 1, NULL, 10);
    for (size_t i = 0; i + 4 <= *bytes; i += 4) memcpy(b + i, &TABLE[(7 * (i / 4) + seed) % NTABLE], 4);
  }
  else if (b) {
    long seed = strtol(f.c_str(), NULL, 10);
    if (seed >= 0)
      for (size_t i = 0; i < *bytes; i++) b[i] = (unsigned char)((seed + 101u * i) & 0xffu);
  }
  for (int *x : {&I->orows, &I->ocols, &I->rows, &I->cols, &I->num_frame}) {
    int v = geti();
    if (v != -1) *x = v;
  }
  I->numcolor = GIVEN_MARK;
  return I;
}

// a volume or none: null, or W P
static MRI *getwn()
{
  if (take("null")) return NULL;
  MRI *m = getw();
  paint(m);
  return m;
}
// BF, a BinarizeMRI's fields (README.md): BinType thmin thmax n match×n maskthmin maskthmax zce zre zse
// OnVal OffVal fstart fend invert mritype DoAbs m_debug (the doubles as doubles, the rest integers)
static void getbin(BinarizeMRI *b)
{
  b->BinType = geti();
  b->thmin = getd();
  b->thmax = getd();
  int n = geti();
  for (int i = 0; i < n; i++) b->matchlist.push_back(geti());
  b->maskthmin = getd();
  b->maskthmax = getd();
  b->ZeroColEdges = geti();
  b->ZeroRowEdges = geti();
  b->ZeroSliceEdges = geti();
  b->OnVal = getd();
  b->OffVal = getd();
  b->fstart = geti();
  b->fend = geti();
  b->invert = geti();
  b->mritype = geti();
  b->DoAbs = geti();
  b->m_debug = geti();
}
// the fields a binarize call may change: fstart fend nhits OnVal OffVal
static void putbin(const BinarizeMRI *b)
{
  puti(b->fstart);
  puti(b->fend);
  puti(b->nhits);
  putd(b->OnVal);
  putd(b->OffVal);
}
// DF, a DEMorphBinVol's fields: morphtype topo nnbrsthresh nmorph frame mm_mritype mm_debug
static void getdem(DEMorphBinVol *d)
{
  d->morphtype = geti();
  d->topo = geti();
  d->nnbrsthresh = geti();
  d->nmorph = geti();
  d->frame = geti();
  d->mm_mritype = geti();
  d->mm_debug = geti();
}
// VL, a voxel list: null, or <alloc> <nvox>: VLSTalloc(alloc), its arrays filled (voxel i: xi 1000 + i,
// yi 2000 + i, zi 3000 + i, fi 4000 + i, xd i + 0.5, yd i + 0.25, zd i + 0.125, vsrc -1 - i, vdst -2 - i),
// then nvox set
static VOXEL_LIST *getvl(int *alloc)
{
  *alloc = 0;
  if (take("null")) return NULL;
  int n = geti(), nv = geti();
  VOXEL_LIST *vl = VLSTalloc(n);
  for (int i = 0; i < n; i++) {
    vl->xi[i] = 1000 + i;
    vl->yi[i] = 2000 + i;
    vl->zi[i] = 3000 + i;
    vl->fi[i] = 4000 + i;
    vl->xd[i] = (float)i + 0.5f;
    vl->yd[i] = (float)i + 0.25f;
    vl->zd[i] = (float)i + 0.125f;
    vl->vsrc[i] = -1.0f - (float)i;
    vl->vdst[i] = -2.0f - (float)i;
  }
  vl->nvox = nv;
  *alloc = n;
  return vl;
}
// a voxel list: null, or max_vox nvox type, `self` when its volume is the given one (else `other` or
// `none`), then for each of its first len voxels xi yi zi fi (integers) xd yd zd vsrc vdst (floats)
static void putvl(const VOXEL_LIST *vl, const MRI *mri, int len)
{
  if (!vl) {
    out("null");
    return;
  }
  puti(vl->max_vox);
  puti(vl->nvox);
  puti(vl->type);
  out(vl->mri == mri ? "self" : vl->mri ? "other" : "none");
  for (int i = 0; i < len; i++) {
    puti(vl->xi[i]);
    puti(vl->yi[i]);
    puti(vl->zi[i]);
    puti(vl->fi[i]);
    putf(vl->xd[i]);
    putf(vl->yd[i]);
    putf(vl->zd[i]);
    putf(vl->vsrc[i]);
    putf(vl->vdst[i]);
  }
}

// VF, a voxel list with its fields (README.md): VL, or f <alloc> <nvox> <type> <mask> <k> and k times
// xi yi zi fi xd yd zd vsrc vdst: VLSTalloc(alloc), VL's patterns, the first k voxels' fields as given,
// the slope arrays of the mask (1 mx, 2 my, 4 mz, 8 t; alloc floats each, entry i: i + 0.75), then nvox
// and type set
static VOXEL_LIST *getvf(int *alloc)
{
  if (!take("f")) return getvl(alloc);
  int n = geti(), nv = geti(), type = geti(), mask = geti(), k = geti();
  VOXEL_LIST *vl = VLSTalloc(n);
  for (int i = 0; i < n; i++) {
    vl->xi[i] = 1000 + i;
    vl->yi[i] = 2000 + i;
    vl->zi[i] = 3000 + i;
    vl->fi[i] = 4000 + i;
    vl->xd[i] = (float)i + 0.5f;
    vl->yd[i] = (float)i + 0.25f;
    vl->zd[i] = (float)i + 0.125f;
    vl->vsrc[i] = -1.0f - (float)i;
    vl->vdst[i] = -2.0f - (float)i;
  }
  for (int i = 0; i < k; i++) {
    int xi = geti(), yi = geti(), zi = geti(), fi = geti();
    float xd = getf(), yd = getf(), zd = getf(), vs = getf(), vd = getf();
    if (i >= n) continue;  // fields past the allocation are read and dropped
    vl->xi[i] = xi;
    vl->yi[i] = yi;
    vl->zi[i] = zi;
    vl->fi[i] = fi;
    vl->xd[i] = xd;
    vl->yd[i] = yd;
    vl->zd[i] = zd;
    vl->vsrc[i] = vs;
    vl->vdst[i] = vd;
  }
  float **arrays[4] = {&vl->mx, &vl->my, &vl->mz, &vl->t};
  for (int b = 0; b < 4; b++) {
    if (!(mask & (1 << b))) continue;
    *arrays[b] = (float *)calloc(n > 0 ? (size_t)n : 1, sizeof(float));
    for (int i = 0; i < n; i++) (*arrays[b])[i] = (float)i + 0.75f;
  }
  vl->nvox = nv;
  vl->type = type;
  *alloc = n;
  return vl;
}
// a voxel list (printed) and its slope arrays: for each of mx, my, mz, t `null`, or `given` and its first
// `given_len` entries when it is the array the list was given with, else `new` and its first `new_len`
static void putvf(const VOXEL_LIST *vl, const MRI *mri, int len, float *const given[4], int given_len,
                  int new_len)
{
  putvl(vl, mri, len);
  if (!vl) return;
  const float *arrays[4] = {vl->mx, vl->my, vl->mz, vl->t};
  for (int b = 0; b < 4; b++) {
    if (!arrays[b]) {
      out("null");
      continue;
    }
    bool old = given && arrays[b] == given[b];
    out(old ? "given" : "new");
    int n = old ? given_len : new_len;
    for (int i = 0; i < n; i++) putf(arrays[b][i]);
  }
}
static void slopes_of(const VOXEL_LIST *vl, float *s[4])
{
  s[0] = vl ? vl->mx : NULL;
  s[1] = vl ? vl->my : NULL;
  s[2] = vl ? vl->mz : NULL;
  s[3] = vl ? vl->t : NULL;
}
// a file or folder removed, a folder with everything in it
static void rmtree(const std::string &p)
{
  struct stat st;
  if (lstat(p.c_str(), &st) != 0) return;
  if (S_ISDIR(st.st_mode)) {
    std::vector<std::string> names;
    if (DIR *d = opendir(p.c_str())) {
      while (struct dirent *e = readdir(d)) {
        std::string n = e->d_name;
        if (n != "." && n != "..") names.push_back(n);
      }
      closedir(d);
    }
    for (const std::string &n : names) rmtree(p + "/" + n);
    rmdir(p.c_str());
  }
  else
    unlink(p.c_str());
}
static void write_file(const std::string &path, const std::string &bytes)
{
  FILE *fp = fopen(path.c_str(), "wb");
  if (!fp) bad("cannot write a file of the call");
  fwrite(bytes.data(), 1, bytes.size(), fp);
  fclose(fp);
}

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
  // ---- part 7b (specs/freesurfer-volume.md §34, §35) and §54's change of a transform array's type ----
  else if (c == "toimgv" || c == "toimg") {
    MRI *a = getw();
    paint(a);
    size_t gb = 0;
    IMAGE *I = getimg(&gb);
    int slice = geti();
    IMAGE *r;
    if (c == "toimgv") {
      int view = geti(), frame = geti();
      r = MRItoImageView(a, I, slice, view, frame);
    }
    else
      r = MRItoImage(a, I, slice);
    fflush(stdout);
    putimg_res(r, I, gb);
  }
  else if (c == "img2mri") {
    size_t gb = 0;
    IMAGE *I = getimg(&gb);
    if (!I) bad("img2mri needs an image");
    MRI *m = ImageToMRI(I);
    fflush(stdout);
    putf(I->xsize);
    putf(I->ysize);
    putv(m);
  }
  else if (c == "smpar") {
    MRI *a = getw();
    paint(a);
    int k = geti();
    putres(MRIsmoothParcellation(a, k), a, NULL, NULL);
  }
  else if (c == "erasebp") {
    MRI *a = getw();
    int b = geti();
    puti(MRIeraseBorderPlanes(a, b));
    putv(a);
  }
  else if (c == "nbhd") {
    MRI *a = getw();
    paint(a);
    int n = geti();
    for (int i = 0; i < n; i++) {
      int k = geti(), ws = geti(), x = geti(), y = geti(), z = geti(), fr = geti();
      float th = getf();
      int v = k == 0   ? MRImaxInNbhd(a, ws, x, y, z, fr)
              : k == 1 ? MRIcountNonzeroInNbhd(a, ws, x, y, z)
              : k == 2 ? MRIcountThreshInNbhd(a, ws, x, y, z, th)
                       : MRIareNonzeroInNbhd(a, ws, x, y, z);
      puti(v);
    }
  }
  else if (c == "nearnz") {
    MRI *a = getw();
    paint(a);
    int n = geti();
    for (int i = 0; i < n; i++) {
      int ws = geti();
      double x = getd(), y = getd(), z = getd();
      int wp = geti();
      int xv = -7, yv = -7, zv = -7;
      float v = MRIfindNearestNonzeroLocation(a, ws, x, y, z, wp ? &xv : NULL, wp ? &yv : NULL, wp ? &zv : NULL);
      putf(v);
      puti(xv);
      puti(yv);
      puti(zv);
    }
  }
  else if (c == "lmeans") {
    int k = geti();
    MRI *a = getw(), *l = getw();
    paint(l);
    int label = geti(), x0 = geti(), y0 = geti(), z0 = geti(), wh = geti(), p = geti();
    double sd = UNSETD;
    if (k == 0) {
      VECTOR *v = MRImeanInLabelMultispectral(a, l, label);
      putm(v);
    }
    else if (k == 1)
      putd(MRImeanInLabel(a, l, label));
    else if (k == 2) {
      putd(MRImeanAndStdInLabel(a, l, label, p ? &sd : NULL));
      putd(sd);
    }
    else if (k == 3) {
      putd(MRImeanInLabelInRegion(a, l, label, x0, y0, z0, wh, p ? &sd : NULL));
      putd(sd);
    }
    else
      putd(MRImaxInLabelInRegion(a, l, label, x0, y0, z0, wh));
  }
  else if (c == "mkpos") {
    MRI *a = getw(), *d = getdst(a);
    MRI *r = MRImakePositive(a, d);
    putres(r, a, NULL, d);
  }
  else if (c == "mlbv") {
    MRI *a = getw();
    paint(a);
    MRI *d = getdst(a);
    int label = geti(), mark = geti(), six = geti();
    MRI *r = MRImarkLabelBorderVoxels(a, d, label, mark, six);
    putres(r, a, NULL, d);
  }
  else if (c == "lnbhd") {
    MRI *l = getw();
    paint(l);
    MRI *v = take("null") ? NULL : getw();
    int x = geti(), y = geti(), z = geti(), wh = geti(), ml = geti(), wm = geti();
    // arrays of exactly max_labels entries (one when it is 0), so that a write past them is a write
    // past the allocation (the guard runs)
    size_t na = ml > 0 ? (size_t)ml : 1;
    int *counts = (int *)malloc(na * sizeof(int));
    float *means = (float *)malloc(na * sizeof(float));
    for (size_t i = 0; i < na; i++) {
      counts[i] = -7;
      means[i] = UNSET;
    }
    puti(MRIcomputeLabelNbhd(l, v, x, y, z, counts, wm ? means : NULL, wh, ml));
    for (int i = 0; i < ml; i++) puti(counts[i]);
    if (wm)
      for (int i = 0; i < ml; i++) putf(means[i]);
  }
  else if (c == "linbhd") {
    MRI *a = getw();
    paint(a);
    int n = geti();
    for (int i = 0; i < n; i++) {
      int k = geti(), x = geti(), y = geti(), z = geti(), wh = geti(), label = geti(), which = geti();
      puti(k == 0 ? MRIlabelsInNbhd(a, x, y, z, wh, label) : MRIlabelsInPlanarNbhd(a, x, y, z, wh, label, which));
    }
  }
  else if (c == "lbbox") {
    MRI *a = getw();
    paint(a);
    int label = geti();
    MRI_REGION b;
    b.x = b.y = b.z = b.dx = b.dy = b.dz = -7;
    puti(MRIlabelBoundingBox(a, label, &b));
    for (int v : {b.x, b.y, b.z, b.dx, b.dy, b.dz}) puti(v);
  }
  else if (c == "fillbox") {
    MRI *a = getw();
    MRI_REGION b;
    if (!getreg(&b)) bad("fillbox needs a box");
    float v = getf();
    puti(MRIfillBox(a, &b, v));
    putv(a);
  }
  else if (c == "mskz") {
    MRI *a = getw(), *m = getw();
    paint(m);
    MRI *d = getdst(a, m);
    MRI *r = MRImaskZero(a, m, d);
    putres(r, a, m, d);
  }
  else if (c == "clipwm") {
    MRI *t = getw(), *w = getw();
    paint(w);
    puti(MRIclipBrightWM(t, w));
    fflush(stdout);
    putv(t);
  }
  else if (c == "ltachg") {
    LTA *lta = getlta();
    int type = geti();
    LTA *r = LTAchangeType(lta, type);
    fflush(stdout);
    out(r == lta ? "self" : "other");
    putlta(r);
  }
  // relay 14 (§54): the other operations on a transform array
  else if (c == "ltainv") {
    int k = geti();  // 0 no array given, 1 a second array given, 2 the array itself
    LTA *lta = getlta();
    LTA *d = k == 1 ? getlta() : k == 2 ? lta : NULL;
    LTA *r = LTAinvert(lta, d);
    fflush(stdout);
    out(r == NULL ? "null" : r == lta ? "self" : r == d ? "given" : "new");
    putlta(r);
    if (k == 1 && r != d) putlta(d);
    if (k != 2 && r != lta) putlta(lta);
  }
  else if (c == "ltacat" || c == "ltacat2") {
    int n = c == "ltacat2" ? 2 : geti();
    std::vector<LTA *> v;
    for (int i = 0; i < n; i++) v.push_back(getlta());
    int reduce = geti();
    LTA *r = c == "ltacat2" ? LTAconcat2(v[0], v[1], reduce) : LTAconcat(v.data(), n, reduce);
    fflush(stdout);
    putlta(r);
    for (LTA *l : v) putlta(l);
  }
  else if (c == "ltared") {
    LTA *lta = getlta();
    LTA *r = LTAreduce(lta);
    fflush(stdout);
    putlta(r);
    putlta(lta);
  }
  else if (c == "ltasrc") {
    LTA *lta = getlta();
    MRI *m = getw();
    int s = LTAmriIsSource(lta, m);
    int t = LTAmriIsTarget(lta, m);
    fflush(stdout);
    puti(s);
    puti(t);
  }
  else if (c == "ltaprt") {
    LTA *lta = getlta();
    int r = LTAprint(stdout, lta);
    fflush(stdout);
    puti(r);
  }
  // part 8 (§37 to §39): the partial-volume count of a label; random and constant volumes,
  // frames, orders, fractions, difference lists; voxelwise operations
  else if (c == "pvlab") {
    MRI *l = getw();
    paint(l);
    MRI *v = take("null") ? NULL : getw();
    if (v) paint(v);
    int label = geti(), maxl = geti();
    MRI *mix = take("null") ? NULL : getw();
    MRI *nbr = take("null") ? NULL : getw();
    MRI *sb = take("null") ? NULL : getw();
    if (sb) paint(sb);
    putf(MRIvoxelsInLabelWithPartialVolumeEffects(l, v, label, mix, nbr, maxl, sb));
    putv(mix);
    putv(nbr);
  }
  else if (c == "randn" || c == "const") {
    int n[4];
    for (int &x : n) x = geti();
    float a = getf(), b = c == "randn" ? getf() : 0.0f;
    long seed = c == "randn" ? geti() : 0;
    MRI *g = take("null") ? NULL : getw();
    if (c == "randn") srand48(seed);
    MRI *r = c == "randn" ? MRIrandn(n[0], n[1], n[2], n[3], a, b, g) : MRIconst(n[0], n[1], n[2], n[3], a, g);
    putres(r, NULL, NULL, g);
  }
  else if (c == "nseq" || c == "nfr") {
    MRI *a = getw();
    paint(a);
    int k = geti();
    float t = getf();
    puti(c == "nseq"  ? MRInormalizeSequence(a, t)
         : k == 0     ? MRInormalizeFrames(a)
         : k == 1     ? MRInormalizeFramesMean(a)
                      : MRInormalizeFramesFirst(a));
    putv(a);
  }
  else if (c == "sort") {
    MRI *a = getw();
    paint(a);
    MRI *m = take("null") ? NULL : getw();
    if (m) paint(m);
    MRI *d = getdst(a, m);
    MRI *r = MRIsort(a, m, d);
    putres(r, a, m, d);
  }
  else if (c == "revsl") {
    MRI *a = getw();
    paint(a);
    a->bvals = getm();
    a->bvecs = getm();
    a->bvec_space = geti();
    MRI *d = getdst(a);
    MRI *r = MRIreverseSliceOrder(a, d);
    putres(r, a, NULL, d);
    if (r) {
      putm(r->bvals);
      putm(r->bvecs);
      puti(r->bvec_space);
    }
  }
  else if (c == "cdtype") {
    MRI *a = getw();
    int type = geti();
    MRI *r = MRIcloneDifferentType(a, type);
    putv(r);
  }
  else if (c == "ordidx" || c == "vperm") {
    MRI *a = getw();
    paint(a);
    long seed = geti();
    long n = (long)a->width * a->height * a->depth;
    size_t na = n > 0 ? (size_t)n : 1;
    short *xs = (short *)malloc(na * sizeof(short)), *ys = (short *)malloc(na * sizeof(short)),
          *zs = (short *)malloc(na * sizeof(short));
    for (size_t i = 0; i < na; i++) xs[i] = ys[i] = zs[i] = -7;
    if (c == "vperm") setRandomSeed(seed);
    puti(c == "ordidx" ? MRIorderIndices(a, xs, ys, zs) : MRIcomputeVoxelPermutation(a, xs, ys, zs));
    for (short *p : {xs, ys, zs})
      for (long i = 0; i < n; i++) puti(p[i]);
  }
  else if (c == "nonmax") {
    MRI *a = getw();
    paint(a);
    MRI *d = getdst(a);
    float t = getf();
    int dir = geti();
    MRI *r = MRInonMaxSuppress(a, d, t, dir);
    putres(r, a, NULL, d);
  }
  else if (c == "xrpad") {
    MRI *a = getw();
    paint(a);
    MRI_REGION reg, *rp = getreg(&reg);
    int pad = geti();
    MRI *r = MRIextractRegionAndPad(a, NULL, rp, pad);
    putv(r);
  }
  else if (c == "rms") {
    MRI *a = getw();
    paint(a);
    MRI *o = getw();
    MRIrms(a, o);
    putv(o);
  }
  else if (c == "vfrac") {
    MRI *a = getw();
    MATRIX *m = getm();
    MRI *s = getw();
    paint(s);
    MRI *f = getw();
    puti(MRIcomputeVolumeFractions(a, m, s, f));
    putv(f);
  }
  else if (c == "cdt") {
    MRI *a = getw();
    paint(a);
    MRI *b = getw();
    paint(b);
    MRI *d = getdst(a, b);
    MRI *r = MRIcombineDistanceTransforms(a, b, d);
    putres(r, a, b, d);
  }
  else if (c == "diffv") {
    MRI *a = getw();
    paint(a);
    MRI *b = getw();
    paint(b);
    MRI *m = take("null") ? NULL : getw();
    if (m) paint(m);
    double t = getd();
    std::vector<std::vector<double>> *dv = MRIdiff2Vect(a, b, t, m);
    if (!dv)
      out("null");
    else {
      puti((long long)dv->size());
      for (const std::vector<double> &row : *dv)
        for (double x : row) putd(x);
    }
  }
  else if (c == "applyv") {
    MRI *a = getw();
    paint(a);
    MRI *m = take("null") ? NULL : getw();
    if (m) paint(m);
    int n = geti();
    std::vector<std::vector<double>> dv;
    for (int i = 0; i < n; i++) {
      std::vector<double> row(5);
      for (double &x : row) x = getd();
      dv.push_back(row);
    }
    MRI *d = getdst(a, m);
    int napply = -7;
    MRI *r = MRIapplyDiffVect(a, m, dv, &napply, d);
    puti(napply);
    putres(r, a, m, d);
  }
  else if (c == "tiv") {
    std::string name = getx();
    bool have = !take("nofile");
    std::string bytes = have ? getx() : std::string();
    int avg = geti();  // USE_AVERAGE305: 0 unset, 1 set to "true", 2 set and empty
    double scale = getd();
    int pdet = geti();
    if (avg) setenv("USE_AVERAGE305", avg == 1 ? "true" : "", 1);
    else unsetenv("USE_AVERAGE305");
    unlink(name.c_str());
    if (have) {
      FILE *fp = fopen(name.c_str(), "wb");
      if (!fp) bad("cannot write the transform file");
      fwrite(bytes.data(), 1, bytes.size(), fp);
      fclose(fp);
    }
    double det = UNSETD;
    std::vector<char> nm(name.begin(), name.end());
    nm.push_back('\0');
    putd(MRIestimateTIV(nm.data(), scale, pdet ? &det : NULL));
    putd(det);
    if (have) unlink(name.c_str());
  }
  else if (c == "dimmis") {
    MRI *a = getw(), *b = getw();
    int f = geti();
    puti(MRIdimMismatch(a, b, f));
  }
  else if (c == "fpow") {
    MRI *a = getw();
    paint(a);
    int n = geti();
    // exactly n powers (one when n is 0), so that a read past them is a read past the allocation
    float *p = (float *)malloc((n > 0 ? (size_t)n : 1) * sizeof(float));
    p[0] = UNSET;
    for (int i = 0; i < n; i++) p[i] = getf();
    puti(mri_framepower(a, p));
    putv(a);
  }
  else if (c == "mbin") {
    MRI *a = getw();
    paint(a);
    MRI *d = getdst(a);
    float t = getf();
    std::string tail;
    bool ht = !take("null");
    if (ht) tail = getx();
    int inv = geti();
    int nover = -7;
    MRI *r = mri_binarize(a, t, ht ? tail.c_str() : NULL, inv, d, &nover);
    puti(nover);
    putres(r, a, NULL, d);
  }
  else if (c == "reshape") {
    MRI *a = getw();
    paint(a);
    int n[4];
    for (int &x : n) x = geti();
    MRI *r = mri_reshape(a, n[0], n[1], n[2], n[3]);
    putv(r);
    puti(a->nframes);
  }
  else if (c == "msq") {
    int k = geti();
    MRI *a = getw();
    paint(a);
    MRI *m = take("null") ? NULL : getw();
    if (m) paint(m);
    MRI *d = getdst(a, m);
    MRI *r = k == 0 ? MRIsquare(a, m, d) : k == 1 ? MRIsquareRoot(a, m, d) : MRIsqrt(a, d);
    putres(r, a, m, d);
  }
  else if (c == "mmax") {
    MRI *a = getw();
    paint(a);
    MRI *b = getw();
    paint(b);
    MRI *d = getdst(a, b);
    MRI *r = MRImax(a, b, d);
    putres(r, a, b, d);
  }
  else if (c == "mconst") {
    MRI *a = getw();
    paint(a);
    double k = getd();
    int mul = geti();
    MRI *d = getdst(a);
    MRI *r = mul ? MRImultiplyConst(a, k, d) : MRIaddConst(a, k, d);
    putres(r, a, NULL, d);
  }
  else if (c == "fbin") {
    MRI *a = getw();
    paint(a);
    double t = getd();
    MRI *m = take("null") ? NULL : getw();
    if (m) paint(m);
    MRI *r = MRIframeBinarize(a, t, m);
    putres(r, a, NULL, m);
  }
  else if (c == "vote") {
    MRI *a = getw();
    paint(a);
    MRI *m = take("null") ? NULL : getw();
    if (m) paint(m);
    MRI *d = getdst(a, m);
    int ex = geti();
    MRI *r = MRIvote(a, m, d, ex);
    putres(r, a, m, d);
    putv(a);
  }
  else if (c == "crop") {
    MRI *a = getw();
    paint(a);
    int p[6];
    for (int &x : p) x = geti();
    putv(MRIcrop(a, p[0], p[1], p[2], p[3], p[4], p[5]));
  }
  else if (c == "revs") {
    MRI *a = getw();
    paint(a);
    MRI *d = getdst(a);
    MRI *r = MRIreverseSlices(a, d);
    putres(r, a, NULL, d);
  }
  else if (c == "cutend") {
    MRI *a = getw();
    paint(a);
    int n = geti();
    putv(MRIcutEndSlices(a, n));
  }
  else if (c == "segids") {
    MRI *a = getw();
    paint(a);
    int frame = geti(), not0 = geti();
    int n = -7;
    int *l = not0 ? MRIsegIdListNot0(a, &n, frame) : MRIsegIdList(a, &n, frame);
    puti(n);
    if (!l)
      out("null");
    else
      for (int i = 0; i < n; i++) puti(l[i]);
  }
  else if (c == "hcos") {
    MRI *a = getw();
    paint(a);
    double al = getd();
    MRI *d = getdst(a);
    MRI *r = MRIhalfCosBias(a, al, d);
    putres(r, a, NULL, d);
  }
  // relay 15 (§55 to §57): a segmentation onto another grid, SPM warps, the codes' names; segment
  // borders, statistics, indices, concatenation, edits, shifts, the entorhinal fix, the
  // amygdala-cortex junction; crops
  else if (c == "a2v") {
    MRI *a = getw();
    paint(a);
    MATRIX *m = getm();
    MRI *t = getw();
    double ft = getd();
    COLOR_TABLE *ct = getct();
    MRI *hit = NULL;
    MRI *r = MRIaseg2vol(a, m, t, ft, &hit, ct);
    fflush(stdout);
    putv(r);
    putv(r ? hit : NULL);
  }
  else if (c == "a2vmu") {
    MRI *a = getw();
    paint(a);
    LTA *l = getlta();
    double ft = getd();
    int usf = geti();
    COLOR_TABLE *ct = getct();
    MRI *hit = NULL;
    MRI *r = MRIaseg2volMU(a, l, ft, &hit, usf, ct);
    fflush(stdout);
    putv(r);
    putv(r ? hit : NULL);
    putlta(l);
  }
  else if (c == "spmw") {
    MRI *v = getw();
    paint(v);
    LTA *l = take("null") ? NULL : getlta();
    MRI *w = getw();
    paint(w);
    int lr = geti(), interp = geti();
    MRI *d = take("new") ? NULL : getw();
    MRI *r = MRIapplySpmWarp(v, l, w, lr, interp, d);
    fflush(stdout);
    putres(r, v, w, d);
    putlta(l);
  }
  else if (c == "icode") {
    std::string s = getx();
    puti(interpolation_code(s.c_str()));
    puti(float2int_code(s.c_str()));
  }
  else if (c == "sbound") {
    MRI *a = getw();
    paint(a);
    putv(MRIsegBoundary(a));
  }
  else if (c == "sborder") {
    MRI *a = getw();
    paint(a);
    int topo = geti(), ml = -7;
    MRI *r = MRIsegBorder(a, &ml, topo);
    fflush(stdout);
    puti(ml);
    putv(r);
  }
  else if (c == "idxno") {
    MRI *a = getw();
    paint(a);
    MRI *d = getdst(a);
    MRI *r = MRIindexNo(a, d);
    putres(r, a, NULL, d);
  }
  else if (c == "ssrob") {
    MRI *s = getw();
    paint(s);
    MRI *m = getw();
    paint(m);
    int id = geti(), f = geti();
    float pct = getf();
    float mn = -7, mx = -7, rg = -7, mean = -7, sd = -7;
    puti(MRIsegStatsRobust(s, id, m, f, &mn, &mx, &rg, &mean, &sd, pct));
    for (float x : {mn, mx, rg, mean, sd}) putf(x);
  }
  else if (c == "sfavg") {
    MRI *s = getw();
    paint(s);
    MRI *m = getw();
    paint(m);
    int id = geti();
    std::vector<double> av(m->nframes > 0 ? (size_t)m->nframes : 1, -7.0);
    puti(MRIsegFrameAvg(s, id, m, av.data()));
    for (int f = 0; f < m->nframes; f++) putd(av[(size_t)f]);
  }
  else if (c == "zcat") {
    MRI *a = getw();
    paint(a);
    MRI *b = getw();
    paint(b);
    int nskip = geti();
    MRI *d = getdst(a, b);
    MRI *r = MRIzconcat(a, b, nskip, d);
    fflush(stdout);
    putres(r, a, b, d);
  }
  else if (c == "aedit") {
    MRI *n = getw();
    paint(n);
    MRI *o = getw();
    paint(o);
    MRI *m = getw();
    paint(m);
    MRI *d = getdst(n, o);
    MRI *r = MRIapplyEdits(n, o, m, d);
    fflush(stdout);
    putres(r, n, o, d);
  }
  else if (c == "shdim") {
    MRI *a = getw();
    paint(a);
    int dim = geti(), ns = geti(), wrap = geti();
    MRI *r = MRIshiftDim(a, dim, ns, wrap);
    fflush(stdout);
    putv(r);
  }
  else if (c == "fento") {
    MRI *a = getw();
    paint(a);
    MRI *e = getw();
    paint(e);
    int level = geti();
    double lh = getd(), rh = getd();
    int acj = geti();
    int n = MRIfixEntoWM(a, e, level, lh, rh, acj);
    fflush(stdout);
    puti(n);
    putv(a);
  }
  else if (c == "lacj") {
    MRI *a = getw();
    paint(a);
    int topo = geti();
    MRI *d = take("new") ? NULL : getw();
    // the new output's colour table draws its colours: a fixed seed first, as a program sets one
    setRandomSeed(1);
    MRI *r = LabelAmygalaCortalJunction(a, topo, d);
    fflush(stdout);
    putres(r, a, NULL, d);
  }
  else if (c == "cfov") {
    MRI *a = getw();
    paint(a);
    int cf = geti(), rf = geti(), sf = geti();
    double th = getd();
    MRI_REGION *r = MRIcropToFoV(a, cf, rf, sf, th);
    fflush(stdout);
    for (int v : {r->x, r->y, r->z, r->dx, r->dy, r->dz}) puti(v);
  }
  else if (c == "ccrs" || c == "crras") {
    MRI *a = getw();
    paint(a);
    if (c == "crras") a->valid = geti();
    double ctr[3];
    for (double &x : ctr) x = getd();
    int fov[3];
    for (int &x : fov) x = geti();
    int nk = geti();
    std::vector<int> keep;
    for (int i = 0; i < nk; i++) keep.push_back(geti());
    LTA *l = c == "crras" && !take("null") ? getlta() : NULL;
    MRI *r = c == "ccrs" ? MRIcropAroundCRS(a, ctr, fov, keep) : MRIcropAroundRAS(a, ctr, fov, l, keep);
    fflush(stdout);
    putv(r);
    if (l) putlta(l);
  }
  // relay 16 (part 9, §41 to §44): volume-to-volume resampling with and without a voxel-shift map,
  // the trilinear kernel, the displacement volume, a registration's array, the masked and
  // upsampled source; the frame covariance, principal components and their statistics, the false
  // discovery rate's threshold; the region boxes; the voxel neighbourhood measures
  else if (c == "vv" || c == "vvsm" || c == "vvtk") {
    MRI *s = getw();
    paint(s);
    s->outside_val = getd();
    MRI *t = getw();
    paint(t);
    MATRIX *m = getm();
    int interp = geti();
    float param = getf();
    MRI *vsm = NULL;
    LTA *l = NULL;
    int pedir = 0;
    if (c != "vv") {
      if (!take("null")) {
        vsm = getw();
        paint(vsm);
        vsm->outside_val = getd();
      }
      l = take("null") ? NULL : getlta();
      pedir = geti();
    }
    fflush(stdout);
    int r = c == "vv"     ? MRIvol2Vol(s, t, m, interp, param)
            : c == "vvtk" ? MRIvol2VolTkRegVSM(s, t, m, interp, param, vsm, l, pedir)
                          : MRIvol2VolVSM(s, t, m, interp, param, vsm, l, pedir);
    fflush(stdout);
    puti(r);
    putv(t);
    if (c != "vv") putlta(l);
  }
  else if (c == "vvk") {
    MRI *s = getw();
    paint(s);
    MRI *t = getw();
    paint(t);
    MATRIX *m = getm();
    fflush(stdout);
    MRI *r = MRIvol2VolTLKernel(s, t, m);
    fflush(stdout);
    puti(r == NULL);
    putv(t);
  }
  else if (c == "vvd") {
    MRI *mv = getw();
    paint(mv);
    MRI *t = getw();
    paint(t);
    MATRIX *m = getm();
    fflush(stdout);
    MRI *r = MRIvol2VolDelta(mv, t, m);
    fflush(stdout);
    putv(r);
  }
  else if (c == "rd2lta") {
    MRI *t = getw();
    MRI *mv = getw();
    MATRIX *m = getm();
    fflush(stdout);
    LTA *l = TransformRegDat2LTA(t, mv, m);
    fflush(stdout);
    putlta(l);
  }
  else if (c == "maskup") {
    MRI *s = getw();
    paint(s);
    MRI *mk = NULL;
    if (!take("null")) {
      mk = getw();
      paint(mk);
    }
    int up = geti(), npad = geti(), cons = geti();
    LTA *l = NULL;
    fflush(stdout);
    MRI *r = MRImaskAndUpsample(s, mk, up, npad, cons, &l);
    fflush(stdout);
    putv(r);
    putlta(r ? l : NULL);
  }
  else if (c == "cov" || c == "mpca") {
    MRI *a = getw();
    paint(a);
    MRI *mk = NULL;
    if (!take("null")) {
      mk = getw();
      paint(mk);
    }
    fflush(stdout);
    if (c == "cov") {
      MATRIX *m = MRIcovarianceMatrix(a, mk);
      fflush(stdout);
      putm(m);
    }
    else {
      MATRIX *U = NULL;
      VECTOR *S = NULL;
      MRI *V = NULL;
      int r = MRIpca(a, &U, &S, &V, mk);
      fflush(stdout);
      puti(r);
      putm(U);
      putm(S);
      putv(V);
    }
  }
  else if (c == "pcast") {
    // the singular values (a row vector), then 0 (PrintPCAStats on standard output), 1
    // (WritePCAStats into pca.dat, printed after the program flushes it) or 2 (into a folder that
    // does not exist)
    int n = geti();
    MATRIX *S = MatrixAlloc(1, n, MATRIX_REAL);
    for (int i = 1; i <= n; i++) S->rptr[1][i] = getf();
    int mode = geti();
    unlink("pca.dat");
    fflush(stdout);
    int r = mode == 0 ? PrintPCAStats(stdout, S)
                      : WritePCAStats((char *)(mode == 2 ? "no-such-folder/pca.dat" : "pca.dat"), S);
    // a program's end flushes the file WritePCAStats leaves open
    fflush(NULL);
    puti(r);
    std::string f = slurp("pca.dat");
    out(hexbytes(f.data(), f.size()));
    unlink("pca.dat");
  }
  else if (c == "fdr") {
    // n inputs, each: W P, its frame, a mask (W P or null), its output (new: none, self: the
    // input, or a W); then fdr, sign, log10flag, debug, whether the masks and the outputs are passed
    int n = geti();
    std::vector<MRI *> vols((size_t)n), masks((size_t)n), outs((size_t)n);
    std::vector<int> frames((size_t)n);
    for (int i = 0; i < n; i++) {
      vols[(size_t)i] = getw();
      paint(vols[(size_t)i]);
      frames[(size_t)i] = geti();
      masks[(size_t)i] = NULL;
      if (!take("null")) {
        masks[(size_t)i] = getw();
        paint(masks[(size_t)i]);
      }
      outs[(size_t)i] = take("new") ? NULL : take("self") ? vols[(size_t)i] : getw();
    }
    double fdr = getd();
    int sign = geti(), lg = geti(), dbg = geti(), passmask = geti(), passout = geti();
    double vwth = 7.0;
    fflush(stdout);
    int r = MRIfdr2vwth(vols.data(), n, frames.data(), fdr, sign, lg, passmask ? masks.data() : NULL, &vwth,
                        passout ? outs.data() : NULL, dbg);
    fflush(stdout);
    puti(r);
    putd(vwth);
    for (int i = 0; i < n; i++)
      if (passout && outs[(size_t)i]) putv(outs[(size_t)i]);
  }
  else if (c == "rgbb") {
    // kind 0 REGIONgetBoundingBox (a pad), 1 REGIONgetBoundingBoxM (six pads), 2
    // REGIONgetBoundingBoxEqOdd (a pad)
    MRI *m = getw();
    paint(m);
    int kind = geti();
    int p[6] = {0, 0, 0, 0, 0, 0};
    int npad = 0;
    if (kind == 1)
      for (int &x : p) x = geti();
    else
      npad = geti();
    fflush(stdout);
    MRI_REGION *r = kind == 0   ? REGIONgetBoundingBox(m, npad)
                    : kind == 1 ? REGIONgetBoundingBoxM(m, p)
                                : REGIONgetBoundingBoxEqOdd(m, npad);
    fflush(stdout);
    if (!r)
      out("null");
    else
      for (int v : {r->x, r->y, r->z, r->dx, r->dy, r->dz}) puti(v);
  }
  else if (c == "rgprt") {
    MRI_REGION rg;
    rg.x = geti();
    rg.y = geti();
    rg.z = geti();
    rg.dx = geti();
    rg.dy = geti();
    rg.dz = geti();
    fflush(stdout);
    REGIONprint(stdout, &rg);
    fflush(stdout);
    MRI_REGION *cp = REGIONcopy(&rg, NULL);
    for (int v : {cp->x, cp->y, cp->z, cp->dx, cp->dy, cp->dz}) puti(v);
    REGIONclear(cp);
    for (int v : {cp->x, cp->y, cp->z, cp->dx, cp->dy, cp->dz}) puti(v);
  }
  else if (c == "vxn") {
    // n points: x y z wsize frame; each prints Dx, Dy, Dz, the mean, the direction and the
    // second derivative along the gradient
    MRI *m = getw();
    paint(m);
    int n = geti();
    for (int i = 0; i < n; i++) {
      int x = geti(), y = geti(), z = geti(), w = geti(), f = geti();
      fflush(stdout);
      putf(MRIvoxelDx(m, x, y, z));
      putf(MRIvoxelDy(m, x, y, z));
      putf(MRIvoxelDz(m, x, y, z));
      putf(MRIvoxelMean(m, x, y, z, w, f));
      putf(MRIvoxelDirection(m, x, y, z, w));
      putf(MRIvoxelGradientDir2ndDerivative(m, x, y, z, w));
      fflush(stdout);
    }
  }
  // part 10a: the Talairach helpers (§46), dilation and the segment helpers (§48)
  else if (c == "talcras") {
    MRI *m = getw();
    paint(m);
    LTA *l = take("null") ? NULL : getlta();
    fflush(stdout);
    int r = ModifyTalairachCRAS(m, l);
    fflush(stdout);
    puti(r);
    putv(m);
  }
  else if (c == "talmat") {
    int kind = geti();
    MRI *m = getw();
    paint(m);
    LTA *l = take("null") ? NULL : getlta();
    fflush(stdout);
    MATRIX *res = kind == 0   ? MtalairachFromVoxel(m, l)
                  : kind == 1 ? MtalVoxelFromVoxel(m, l)
                  : kind == 2 ? MvoxelFromTalairach(m, l)
                              : MvoxelFromTalVoxel(m, l);
    fflush(stdout);
    putm(res);
  }
  else if (c == "talpt") {
    int kind = geti();
    MRI *m = NULL;
    MATRIX *M = NULL;
    LTA *l = NULL;
    if (kind == 4)
      M = getm();
    else {
      m = getw();
      paint(m);
      l = take("null") ? NULL : getlta();
    }
    int n = geti();
    for (int i = 0; i < n; i++) {
      double x = getd(), y = getd(), z = getd(), px = UNSETD, py = UNSETD, pz = UNSETD;
      int r = 0;
      fflush(stdout);
      if (kind == 4) TransformWithMatrix(M, x, y, z, &px, &py, &pz);
      else if (kind == 0) r = MRIvoxelToTalairachEx(m, x, y, z, &px, &py, &pz, l);
      else if (kind == 1) r = MRIvoxelToTalairachVoxelEx(m, x, y, z, &px, &py, &pz, l);
      else if (kind == 2) r = MRItalairachToVoxelEx(m, x, y, z, &px, &py, &pz, l);
      else r = MRItalairachVoxelToVoxelEx(m, x, y, z, &px, &py, &pz, l);
      fflush(stdout);
      puti(r);
      putd(px);
      putd(py);
      putd(pz);
    }
  }
  else if (c == "talvol") {
    int kind = geti();
    MRI *m = getw();
    paint(m);
    MRI *t = take("new") ? NULL : getw();
    if (t) paint(t);
    LTA *l = take("null") ? NULL : getlta();
    int interp = kind == 0 ? geti() : 0;
    fflush(stdout);
    MRI *res = kind == 0   ? MRItoTalairachExInterp(m, t, l, interp)
               : kind == 1 ? MRItoTalairachEx(m, t, l)
                           : MRIfromTalairachEx(m, t, l);
    fflush(stdout);
    putres(res, m, NULL, t);
  }
  else if (c == "dilate" || c == "dilu") {
    MRI *a = getw();
    paint(a);
    MRI *d = getdst(a);
    fflush(stdout);
    MRI *res = c == "dilate" ? MRIdilate(a, d) : MRIdilateUchar(a, d);
    fflush(stdout);
    putres(res, a, NULL, d);
  }
  else if (c == "dilo") {
    // relay 19: MRIdilate with the source's outside value (a double) set first
    MRI *a = getw();
    paint(a);
    a->outside_val = getd();
    MRI *d = getdst(a);
    fflush(stdout);
    MRI *res = MRIdilate(a, d);
    fflush(stdout);
    putres(res, a, NULL, d);
  }
  else if (c == "msegs") {
    MRI *a = getw();
    paint(a);
    MRI *d = getdst(a);
    int n = geti();
    std::vector<int> list(n > 0 ? (size_t)n : 1);
    for (int i = 0; i < n; i++) list[(size_t)i] = geti();
    int id = geti();
    fflush(stdout);
    MRI *res = MRImatchSegs(a, list.data(), n, id, d);
    fflush(stdout);
    putres(res, a, NULL, d);
  }
  else if (c == "sstat" || c == "hseg") {
    MRI *s = getw();
    paint(s);
    MRI *v = getw();
    paint(v);
    int id = geti();
    if (c == "sstat") {
      int f = geti();
      float mn = 7.0f, mx = 7.0f, rg = 7.0f, me = 7.0f, sd = 7.0f;
      fflush(stdout);
      int n = MRIsegStats(s, id, v, f, &mn, &mx, &rg, &me, &sd);
      fflush(stdout);
      puti(n);
      for (float x : {mn, mx, rg, me, sd}) putf(x);
    }
    else {
      double lo = getd(), hi = getd(), dl = getd();
      fflush(stdout);
      HISTOGRAM *h = HISTOseg(s, id, v, lo, hi, dl);
      fflush(stdout);
      // the histogram: max_bins nbins bin_size min max, then each bin's value and count
      puti(h->max_bins);
      puti(h->nbins);
      for (float x : {h->bin_size, h->min, h->max}) putf(x);
      for (int i = 0; i < h->nbins; i++) {
        putf(h->bins[i]);
        putf(h->counts[i]);
      }
    }
  }
  // part 10b (relay 19): conforming (§49), set operations, morphology, masks and counts (§50)
  else if (c == "conform") {
    // k 0 MRIconform, 1 MRIgetConformMatrix, 2 MRIconformedTemplate (width, size, KeepDC); then the
    // input; volumes with their voxels hashed
    int k = geti();
    MRI *m = getw();
    paint(m);
    int w = 0, keep = 0;
    double sz = 0;
    if (k == 2) {
      w = geti();
      sz = getd();
      keep = geti();
    }
    fflush(stdout);
    if (k == 0) {
      MRI *res = MRIconform(m);
      fflush(stdout);
      putvh(res);
    }
    else if (k == 1) {
      MATRIX *M = MRIgetConformMatrix(m);
      fflush(stdout);
      putm(M);
    }
    else {
      MRI *res = MRIconformedTemplate(m, w, sz, keep);
      fflush(stdout);
      putvh(res);
    }
    putvh(m);
  }
  else if (c == "union" || c == "isect" || c == "mand" || c == "mxor") {
    MRI *a = getw();
    paint(a);
    MRI *b = getw();
    paint(b);
    MRI *d = getdst(a, b);
    int t1 = 0, t2 = 0;
    if (c == "mand") t1 = geti();
    if (c == "mxor") {
      t1 = geti();
      t2 = geti();
    }
    fflush(stdout);
    MRI *res = c == "union"   ? MRIunion(a, b, d)
               : c == "isect" ? MRIintersect(a, b, d)
               : c == "mand"  ? MRIand(a, b, d, t1)
                              : MRIxor(a, b, d, t1, t2);
    fflush(stdout);
    putres(res, a, b, d);
  }
  else if (c == "compl" || c == "erode" || c == "erode2d" || c == "dil6" || c == "mnot" || c == "b2c") {
    MRI *a = getw();
    paint(a);
    MRI *d = getdst(a);
    fflush(stdout);
    MRI *res = c == "compl"     ? MRIcomplement(a, d)
               : c == "erode"   ? MRIerode(a, d)
               : c == "erode2d" ? MRIerode2D(a, d)
               : c == "dil6"    ? MRIdilate6(a, d)
               : c == "mnot"    ? MRInot(a, d)
                                : MRIbinMaskToCol(a, d);
    fflush(stdout);
    putres(res, a, NULL, d);
  }
  else if (c == "edges") {
    MRI *a = getw();
    paint(a);
    double v = getd();
    MRI *d = getdst(a);
    fflush(stdout);
    MRI *res = MRIsetEdges(a, v, d);
    fflush(stdout);
    putres(res, a, NULL, d);
  }
  else if (c == "erodenn" || c == "erodeseg") {
    MRI *a = getw();
    paint(a);
    MRI *d = getdst(a);
    int i1 = geti(), i2 = geti();
    fflush(stdout);
    MRI *res = c == "erodenn" ? MRIerodeNN(a, d, i1, i2) : MRIerodeSegmentation(a, d, i1, i2);
    fflush(stdout);
    putres(res, a, NULL, d);
  }
  else if (c == "dilseg") {
    // seg, out, nDils, the mask (null or a volume), its frame and threshold; then nchanges (7 unset)
    MRI *a = getw();
    paint(a);
    MRI *d = getdst(a);
    int n = geti();
    MRI *mk = NULL;
    if (!take("null")) {
      mk = getw();
      paint(mk);
    }
    int frame = geti();
    double th = getd();
    int nch = 7;
    fflush(stdout);
    MRI *res = MRIdilateSegmentation(a, d, n, mk, frame, th, &nch);
    fflush(stdout);
    putres(res, a, NULL, d);
    puti(nch);
  }
  else if (c == "dtl") {
    // k 0 MRIdilateThreshLabel, 1 MRIdilateInvThreshLabel (src, val, dst, label, niter, thresh)
    int k = geti();
    MRI *a = getw();
    paint(a);
    MRI *v = getw();
    paint(v);
    MRI *d = getdst(a, v);
    int label = geti(), niter = geti(), th = geti();
    fflush(stdout);
    MRI *res = k == 0 ? MRIdilateThreshLabel(a, v, d, label, niter, th)
                      : MRIdilateInvThreshLabel(a, v, d, label, niter, th);
    fflush(stdout);
    putres(res, a, v, d);
  }
  else if (c == "dlab") {
    // k 0 MRIdilateLabel, 1 MRIdilateLabelUchar (src, dst, label, niter)
    int k = geti();
    MRI *a = getw();
    paint(a);
    MRI *d = getdst(a);
    int label = geti(), niter = geti();
    fflush(stdout);
    MRI *res = k == 0 ? MRIdilateLabel(a, d, label, niter) : MRIdilateLabelUchar(a, d, label, niter);
    fflush(stdout);
    putres(res, a, NULL, d);
  }
  else if (c == "openn") {
    // k 0 MRIopenN (an order), 1 MRIopen, 2 MRIclose
    int k = geti();
    MRI *a = getw();
    paint(a);
    MRI *d = getdst(a);
    int order = k == 0 ? geti() : 0;
    fflush(stdout);
    MRI *res = k == 0 ? MRIopenN(a, d, order) : k == 1 ? MRIopen(a, d) : MRIclose(a, d);
    fflush(stdout);
    putres(res, a, NULL, d);
  }
  else if (c == "rlist") {
    // seg, n pairs (source, target), the mask (null or a volume), out
    MRI *a = getw();
    paint(a);
    int n = geti();
    std::vector<int> s((size_t)(n > 0 ? n : 1)), t((size_t)(n > 0 ? n : 1));
    for (int i = 0; i < n; i++) {
      s[(size_t)i] = geti();
      t[(size_t)i] = geti();
    }
    MRI *mk = NULL;
    if (!take("null")) {
      mk = getw();
      paint(mk);
    }
    MRI *d = getdst(a, mk);
    fflush(stdout);
    MRI *res = MRIreplaceList(a, s.data(), t.data(), n, mk, d);
    fflush(stdout);
    putres(res, a, mk, d);
  }
  else if (c == "rvonly") {
    MRI *a = getw();
    paint(a);
    MRI *d = getdst(a);
    float in = getf(), ov = getf();
    fflush(stdout);
    MRI *res = MRIreplaceValuesOnly(a, d, in, ov);
    fflush(stdout);
    putres(res, a, NULL, d);
  }
  else if (c == "mmask" || c == "tmask") {
    // mmask k (0 MRImask, 1 MRImaskDifferentGeometry) src mask dst m out; tmask src mask dst
    // thresh out (MRIthresholdMask)
    int k = c == "mmask" ? geti() : 0;
    MRI *a = getw();
    paint(a);
    MRI *mk = getw();
    paint(mk);
    MRI *d = getdst(a, mk);
    MRI *res;
    if (c == "mmask") {
      int m = geti();
      float ov = getf();
      fflush(stdout);
      res = k == 0 ? MRImask(a, mk, d, m, ov) : MRImaskDifferentGeometry(a, mk, d, m, ov);
    }
    else {
      float th = getf(), ov = getf();
      fflush(stdout);
      res = MRIthresholdMask(a, mk, d, th, ov);
    }
    fflush(stdout);
    putres(res, a, mk, d);
  }
  else if (c == "grow" || c == "onfg" || c == "offbg") {
    // grow mri filled in out (MRIgrowLabel); onfg mri fg bg (MRIturnOnFG); offbg mri bg
    // (MRIturnOffBG): the result, then every volume
    MRI *a = getw();
    paint(a);
    MRI *b = getw();
    paint(b);
    MRI *e = NULL;
    int i1 = 0, i2 = 0;
    if (c == "onfg") {
      e = getw();
      paint(e);
    }
    else if (c == "grow") {
      i1 = geti();
      i2 = geti();
    }
    fflush(stdout);
    int r = c == "grow" ? MRIgrowLabel(a, b, i1, i2) : c == "onfg" ? MRIturnOnFG(a, b, e) : MRIturnOffBG(a, b);
    fflush(stdout);
    puti(r);
    putv(a);
    putv(b);
    if (e) putv(e);
  }
  else if (c == "cplab" || c == "cplv") {
    // cplab src dst label (MRIcopyLabel); cplv src lab dst label (MRIcopyLabeledVoxels): the count,
    // then dst (`self` or `other` as D)
    MRI *a = getw();
    paint(a);
    MRI *lb = NULL;
    if (c == "cplv") {
      lb = getw();
      paint(lb);
    }
    MRI *d = getdst(a, lb);
    int label = geti();
    fflush(stdout);
    int n = c == "cplab" ? MRIcopyLabel(a, d, label) : MRIcopyLabeledVoxels(a, lb, d, label);
    fflush(stdout);
    puti(n);
    putv(d);
  }
  else if (c == "slv") {
    MRI *a = getw();
    paint(a);
    MRI *lb = getw();
    paint(lb);
    MRI *d = getdst(a, lb);
    int label = geti();
    float v = getf();
    fflush(stdout);
    MRI *res = MRIsetLabelValues(a, lb, d, label, v);
    fflush(stdout);
    putres(res, a, lb, d);
  }
  else if (c == "cntv") {
    // k 0 MRItotalVoxelsOn, 1 MRIvoxelsInLabel, 2 MRIlabelInVolume (an int), 3 MRInMask
    int k = geti();
    MRI *a = getw();
    paint(a);
    int i = k < 3 ? geti() : 0;
    fflush(stdout);
    int r = k == 0   ? MRItotalVoxelsOn(a, i)
            : k == 1 ? MRIvoxelsInLabel(a, i)
            : k == 2 ? MRIlabelInVolume(a, i)
                     : MRInMask(a);
    fflush(stdout);
    puti(r);
  }
  else if (c == "cent") {
    MRI *a = getw();
    paint(a);
    double x = UNSETD, y = UNSETD, z = UNSETD;
    fflush(stdout);
    int r = MRIcomputeCentroid(a, &x, &y, &z);
    fflush(stdout);
    puti(r);
    putd(x);
    putd(y);
    putd(z);
  }
  else if (c == "pthr") {
    // the volume, the mask (null or a volume), the frame, the percentage
    MRI *a = getw();
    paint(a);
    MRI *mk = NULL;
    if (!take("null")) {
      mk = getw();
      paint(mk);
    }
    int frame = geti();
    double pct = getd();
    fflush(stdout);
    double r = MRIpercentThresh(a, mk, frame, pct);
    fflush(stdout);
    putd(r);
  }
  else if (c == "cnbhd") {
    // n points: wsize x y z val
    MRI *a = getw();
    paint(a);
    int n = geti();
    for (int i = 0; i < n; i++) {
      int w = geti(), x = geti(), y = geti(), z = geti(), v = geti();
      fflush(stdout);
      puti(MRIcountValInNbhd(a, w, x, y, z, v));
      fflush(stdout);
    }
  }
  // part 10c (relay 20): volume histograms (§52)
  else if (c == "hreg" || c == "hthr") {
    MRI *a = getw();
    paint(a);
    HISTOGRAM *h = geth();
    int n = geti();
    MRI_REGION r, *rp = getreg(&r);
    MRI *t = NULL;
    float th = 0;
    int fr = 0;
    if (c == "hthr") {
      t = getw();
      paint(t);
      th = getf();
      fr = geti();
    }
    fflush(stdout);
    HISTOGRAM *res =
        c == "hreg" ? MRIhistogramRegion(a, n, h, rp) : MRIhistogramRegionWithThreshold(a, n, h, rp, t, th, fr);
    fflush(stdout);
    puth_res(res, h);
  }
  else if (c == "hvox") {
    MRI *a = getw();
    paint(a);
    HISTOGRAM *h = geth();
    int n = geti(), x = geti(), y = geti(), z = geti(), w = geti();
    MRI *t = NULL;
    if (!take("null")) {
      t = getw();
      paint(t);
    }
    float th = getf();
    fflush(stdout);
    HISTOGRAM *res = MRIhistogramVoxel(a, n, h, x, y, z, w, t, th);
    fflush(stdout);
    puth_res(res, h);
  }
  else if (c == "hist") {
    MRI *a = getw();
    paint(a);
    int n = geti();
    fflush(stdout);
    HISTOGRAM *res = MRIhistogram(a, n);
    fflush(stdout);
    puth(res);
  }
  else if (c == "geq") {
    MRI *a = getw();
    paint(a);
    HISTOGRAM *h = geth();
    int lo = geti(), hi = geti(), norm = geti();
    fflush(stdout);
    HISTOGRAM *res = MRIgetEqualizeHisto(a, h, lo, hi, norm);
    fflush(stdout);
    puth_res(res, h);
  }
  else if (c == "heq") {
    MRI *a = getw();
    paint(a);
    MRI *b = getw();
    paint(b);
    MRI *d = getdst(a, b);
    int lo = geti(), hi = geti();
    fflush(stdout);
    MRI *res = MRIhistoEqualize(a, b, d, lo, hi);
    fflush(stdout);
    putres(res, a, b, d);
  }
  else if (c == "hlr" || c == "hlab") {
    MRI *a = getw();
    paint(a);
    MRI *l = a;
    if (!take("self")) {
      l = getw();
      paint(l);
    }
    MRI_REGION r, *rp = NULL;
    if (c == "hlr") rp = getreg(&r);
    int lab = geti(), n = geti();
    fflush(stdout);
    HISTOGRAM *res = c == "hlr" ? MRIhistogramLabelRegion(a, l, rp, lab, n) : MRIhistogramLabel(a, l, lab, n);
    fflush(stdout);
    puth(res);
  }
  else if (c == "hsegm") {
    MRI *a = getw();
    paint(a);
    MRI *d = getdst(a);
    int wl = geti(), wh = geti(), gh = geti(), ws = geti();
    float sg = getf();
    fflush(stdout);
    MRI *res = MRIhistoSegment(a, d, wl, wh, gh, ws, sg);
    fflush(stdout);
    putres(res, a, NULL, d);
  }
  // ---- part 11a (§59, §60), §64's and §65's functions of mri_cc --------------------------------
  else if (c == "vvfill") {
    MRI *a = getw();
    paint(a);
    MRI *m = getwn();
    LTA *l = getlta();
    int usf = geti(), cons = geti();
    MRI *d = take("new") ? NULL : getw();
    fflush(stdout);
    MRI *res = MRIvol2VolFill(a, m, l, usf, cons, d);
    fflush(stdout);
    putres(res, a, NULL, d);
    putlta(l);
  }
  else if (c == "fbnwm") {
    MRI *a = getw();
    paint(a);
    MRI *b = getw();
    paint(b);
    fflush(stdout);
    MRI *res = MRIfindBrightNonWM(a, b);
    fflush(stdout);
    putres(res, a, b, NULL);
  }
  else if (c == "qecc") {
    MRI *a = getw();
    paint(a);
    MRI *m = getwn();
    int x = geti(), y = geti(), z = geti();
    fflush(stdout);
    int v = QuadEulerCharChange(a, m, x, y, z);
    fflush(stdout);
    puti(v);
  }
  else if (c == "vtero") {
    MRI *a = getw();
    paint(a);
    MRI *m = getwn();
    fflush(stdout);
    int v = MRIvolTopoErodeOne(a, m);
    fflush(stdout);
    puti(v);
    putv(a);
  }
  else if (c == "vtdil") {
    MRI *a = getw();
    paint(a);
    int on = geti();
    fflush(stdout);
    int v = MRIvolTopoDilateOne(a, on);
    fflush(stdout);
    puti(v);
    putv(a);
  }
  else if (c == "fscm") {
    FixSubCortMassHA f;
    f.subcorticalmass = getw();
    paint(f.subcorticalmass);
    f.aseg = getw();
    paint(f.aseg);
    f.nDilate = geti();
    fflush(stdout);
    int v = f.FixSCM();
    fflush(stdout);
    puti(v);
    putv(f.subcorticalmass);
    putv(f.mask);
  }
  else if (c == "binmri") {
    BinarizeMRI b;
    getbin(&b);
    int k = geti();
    for (int i = 0; i < k; i++) {
      MRI *a = getwn();
      MRI *m = getwn();
      MRI *d = getdst(a);
      fflush(stdout);
      MRI *res = b.binarize(a, m, d);
      fflush(stdout);
      putres(res, a, NULL, d);
      putbin(&b);
    }
  }
  else if (c == "binq") {
    BinarizeMRI b;
    getbin(&b);
    MRI *a = getw();
    paint(a);
    MRI *m = getwn();
    int x = geti(), y = geti(), z = geti(), f = geti();
    fflush(stdout);
    int v = b.qualifies(a, m, x, y, z, f);
    fflush(stdout);
    puti(v);
  }
  else if (c == "bindump") {
    BinarizeMRI b;
    getbin(&b);
    MRI *m = getwn();
    fflush(stdout);
    int v = b.dump(NULL, m, NULL);
    fflush(stdout);
    puti(v);
  }
  else if (c == "demchk") {
    DEMorphBinVol d;
    getdem(&d);
    MRI *a = getwn();
    MRI *m = getwn();
    MRI *o = getwn();
    fflush(stdout);
    int v = d.check(a, m, o);
    fflush(stdout);
    puti(v);
  }
  else if (c == "demdump") {
    DEMorphBinVol d;
    getdem(&d);
    fflush(stdout);
    int v = d.dump(NULL);
    fflush(stdout);
    puti(v);
  }
  else if (c == "demcf") {
    DEMorphBinVol d;
    getdem(&d);
    MRI *a = getw();
    paint(a);
    int fr = geti();
    fflush(stdout);
    MRI *res = d.copyFrame(a, fr);
    fflush(stdout);
    putres(res, a, NULL, NULL);
  }
  else if (c == "demm" || c == "demt" || c == "deml") {
    DEMorphBinVol d;
    getdem(&d);
    MRI *a = getw();
    paint(a);
    MRI *res, *o = NULL;
    if (c == "demm") {
      fflush(stdout);
      res = d.morph(a);
    }
    else {
      double lo = 0, hi = 0;
      std::vector<int> ml;
      if (c == "demt") {
        lo = getd();
        hi = getd();
      }
      else {
        int n = geti();
        for (int i = 0; i < n; i++) ml.push_back(geti());
      }
      MRI *m = getwn();
      o = getwn();
      fflush(stdout);
      res = c == "demt" ? d.morph(a, lo, hi, m, o) : d.morph(a, ml, m, o);
    }
    fflush(stdout);
    putres(res, a, NULL, o);
    puti(d.nchangestot);
  }
  else if (c == "trns") {
    int k = geti();
    MATRIX *t = getm(), *o = getm();
    fflush(stdout);
    Trns_tErr e = k == 0 ? Trns_ExtractScaleMatrix(t, o) : Trns_ExtractRotationMatrix(t, o);
    fflush(stdout);
    puti((int)e);
    putm(o);
    putm(t);
  }
  else if (c == "vlcir") {
    MRI *a = getw();
    paint(a);
    float lo = getf(), hi = getf();
    int alloc;
    VOXEL_LIST *vl = getvl(&alloc);
    int skip = geti(), border = geti();
    MRI_REGION r, *rp = getreg(&r);
    fflush(stdout);
    VOXEL_LIST *res = VLSTcreateInRegion(a, lo, hi, vl, skip, border, rp);
    fflush(stdout);
    out(vl && res == vl ? "given" : "new");
    // the count's entries; a given list of a larger allocation keeps its arrays: all of them (entries
    // past the count keep their values)
    int len = res ? res->nvox : 0;
    if (vl && alloc > len) len = alloc;
    putvl(res, a, len);
  }
  // part 11b (§62 to §65): writing a transform array, a volume moved by one, the MNI and tkregister
  // helpers, rotations and affine parameters, voxel lists; and §67: the inverse move, the conversions
  // between voxel and RAS transforms, the composition
  else if (c == "ltawr") {
    int k = geti(), n = geti();
    std::vector<std::string> dirs;
    for (int i = 0; i < n; i++) dirs.push_back(getx());
    LTA *l = getlta();
    std::string name = getx();
    for (const std::string &d : dirs) mkdir(d.c_str(), 0700);
    unlink(name.c_str());
    fflush(stdout);
    int r = k ? LTAwrite(l, name.c_str()) : LTAwriteEx(l, name.c_str());
    fflush(stdout);
    puti(r);
    struct stat st;
    if (stat(name.c_str(), &st) == 0 && S_ISREG(st.st_mode)) {
      std::string s = slurp(name.c_str());
      out(hexbytes(s.data(), s.size()));
    }
    else
      out("nofile");
    unlink(name.c_str());
    for (size_t i = dirs.size(); i-- > 0;) rmtree(dirs[i]);
  }
  else if (c == "ltadvg") {
    VOL_GEOM vg;
    getg(&vg);
    fflush(stdout);
    int r = LTAdumpVolGeom(stdout, &vg);
    fflush(stdout);
    puti(r);
  }
  else if (c == "ltapt") {
    LTA *l = getlta();
    float x = getf(), y = getf(), z = getf();
    MATRIX *m = getm();
    int e0 = geti();
    fflush(stdout);
    errno = e0;
    MATRIX *r = LTAtransformAtPoint(l, x, y, z, m);
    int e = errno;
    fflush(stdout);
    putm_res(r, m);
    puti(e);
  }
  else if (c == "ltaxf" || c == "ltaixf") {
    MRI *a = getw();
    paint(a);
    MRI *d = take("new") ? NULL : getw();
    LTA *l = getlta();
    int interp = geti();
    fflush(stdout);
    MRI *res = c == "ltaixf"  ? LTAinverseTransformInterp(a, d, l, interp)
               : interp < 0 ? LTAtransform(a, d, l)
                            : LTAtransformInterp(a, d, l, interp);
    fflush(stdout);
    putres(res, a, NULL, d);
    putlta(l);
  }
  else if (c == "ltacor") {
    LTA *l = getlta();
    fflush(stdout);
    int r = LTAvoxelTransformToCoronalRasTransform(l);
    fflush(stdout);
    puti(r);
    putlta(l);
  }
  else if (c == "fixmni") {
    float x = getf(), y = getf(), z = getf();
    float a = UNSET, b = UNSET, s = UNSET;
    int r = FixMNITal(x, y, z, &a, &b, &s);
    puti(r);
    putf(a);
    putf(b);
    putf(s);
  }
  else if (c == "ltamod") {
    LTA *l = getlta();
    MRI *s = getwn(), *d = getwn();
    vg_isEqual_Threshold = getd();
    fflush(stdout);
    int r = LTAmodifySrcDstGeom(l, s, d);
    fflush(stdout);
    puti(r);
    putlta(l);
  }
  else if (c == "devxfm") {
    // the subjects' folder `subjects` in the working directory: mk 0 none, 1 subjects/<subject>, 2 also
    // its mri/transforms, the original volume (0 none, 1 orig.mgz's bytes, 2 orig.mgh's, 3 an empty
    // folder orig) and the transform file (its bytes, or nofile) at mri/transforms/<name or talairach.xfm>
    int mk = geti();
    std::string subj = getx();
    bool hsd = !take("null");
    std::string sd = hsd ? getx() : std::string();
    bool hname = !take("null");
    std::string name = hname ? getx() : std::string();
    int ok = geti();
    std::string ob = ok == 1 || ok == 2 ? getx() : std::string();
    bool hx = !take("nofile");
    std::string xb = hx ? getx() : std::string();
    MATRIX *x = getm();
    rmtree("subjects");
    std::string base = "subjects/" + subj;
    if (mk >= 1) {
      mkdir("subjects", 0700);
      mkdir(base.c_str(), 0700);
    }
    if (mk >= 2) {
      mkdir((base + "/mri").c_str(), 0700);
      mkdir((base + "/mri/transforms").c_str(), 0700);
      if (ok == 1) write_file(base + "/mri/orig.mgz", ob);
      if (ok == 2) write_file(base + "/mri/orig.mgh", ob);
      if (ok == 3) mkdir((base + "/mri/orig").c_str(), 0700);
      if (hx) write_file(base + "/mri/transforms/" + (hname ? name : std::string("talairach.xfm")), xb);
    }
    fflush(stdout);
    MATRIX *r = DevolveXFMWithSubjectsDir(subj.c_str(), x, hname ? name.c_str() : NULL, hsd ? sd.c_str() : NULL);
    fflush(stdout);
    putm_res(r, x);
    rmtree("subjects");
  }
  else if (c == "angrm") {
    double ang[3];
    for (double &v : ang) v = getd();
    putm(MRIangles2RotMat(ang));
  }
  else if (c == "affp2m") {
    double p[12];
    for (double &v : p) v = getd();
    MATRIX *m = getm();
    fflush(stdout);
    MATRIX *r = TranformAffineParams2Matrix(p, m);
    fflush(stdout);
    putm_res(r, m);
  }
  else if (c == "affm2p") {
    MATRIX *m = getm();
    double p[12];
    for (double &v : p) v = UNSETD;
    fflush(stdout);
    double *r = TranformExtractAffineParams(m, p);
    fflush(stdout);
    out(r == NULL ? "null" : r == p ? "given" : "other");
    for (double v : p) putd(v);
  }
  else if (c == "ltarot") {
    LTA *l = getlta();
    fflush(stdout);
    int r = LTAmat2RotMat(l);
    fflush(stdout);
    puti(r);
    putlta(l);
  }
  else if (c == "rmsmj") {
    MATRIX *a = getm(), *b = getm();
    double rad = getd();
    fflush(stdout);
    double r = RMSregDiffMJ(a, b, rad);
    fflush(stdout);
    putd(r);
  }
  else if (c == "vlcre") {
    MRI *a = getw();
    paint(a);
    float lo = getf(), hi = getf();
    int alloc;
    VOXEL_LIST *vl = getvf(&alloc);
    int skip = geti(), border = geti();
    float *s[4];
    slopes_of(vl, s);
    fflush(stdout);
    VOXEL_LIST *res = VLSTcreate(a, lo, hi, vl, skip, border);
    fflush(stdout);
    out(vl && res == vl ? "given" : "new");
    int len = res ? res->nvox : 0;
    if (vl && res == vl && alloc > len) len = alloc;
    putvf(res, a, len, s, alloc, 0);
  }
  else if (c == "vlsort") {
    int as, ad;
    VOXEL_LIST *src = getvf(&as);
    VOXEL_LIST *dst = take("null") ? NULL : getvf(&ad);
    fflush(stdout);
    VOXEL_LIST *res = VLSTsort(src, dst);
    fflush(stdout);
    out(dst && res == dst ? "given" : "new");
    putvl(res, NULL, dst && res == dst ? ad : res ? res->nvox : 0);
    putvl(src, NULL, as);
  }
  else if (c == "vlfree") {
    int a;
    VOXEL_LIST *vl = take("null") ? NULL : getvf(&a);
    int r = VLSTfree(&vl);
    puti(r);
    out(vl ? "set" : "null");
  }
  else if (c == "vlstat") {
    int a;
    VOXEL_LIST *vl = getvf(&a);
    MRI *m = getwn();
    vl->mri = m;
    fflush(stdout);
    VLSTcomputeStats(vl);
    fflush(stdout);
    putd(vl->mean);
    putd(vl->std);
  }
  else if (c == "vlcopy") {
    int k = geti(), as, ad = 0;
    VOXEL_LIST *src = getvf(&as);
    VOXEL_LIST *dst = take("null") ? NULL : getvf(&ad);
    int start = geti(), num = geti();
    float *s[4];
    slopes_of(dst, s);
    fflush(stdout);
    VOXEL_LIST *res = k ? VLSTcopyInto(src, dst, start, num) : VLSTcopy(src, dst, start, num);
    fflush(stdout);
    out(dst && res == dst ? "given" : "new");
    int len = dst && res == dst ? ad : k ? start + num : num;
    putvf(res, NULL, len, s, ad, num);
    putvl(src, NULL, as);
  }
  else if (c == "vladd") {
    int k = geti(), a;
    VOXEL_LIST *vl = getvf(&a);
    int x = geti(), y = geti(), z = geti();
    float xd = getf(), yd = getf(), zd = getf(), vs = getf(), vd = getf();
    fflush(stdout);
    int r = k == 0   ? VLSTadd(vl, x, y, z, xd, yd, zd)
            : k == 1 ? VLSTaddUnique(vl, x, y, z, xd, yd, zd)
            : k == 2 ? VLSTaddWithValue(vl, x, y, z, xd, yd, zd, vs, vd)
                     : VLSTinList(vl, x, y, z);
    fflush(stdout);
    puti(r);
    putvl(vl, NULL, a);
  }
  else if (c == "ltav2r" || c == "ltar2v") {
    LTA *l = getlta();
    MRI *s = getwn(), *d = getwn();
    fflush(stdout);
    int r = c == "ltav2r" ? LTAvoxelToRasXform(l, s, d) : LTArasToVoxelXform(l, s, d);
    fflush(stdout);
    puti(r);
    putlta(l);
  }
  else if (c == "ltacmp") {
    LTA *l = getlta();
    MATRIX *left = getm(), *right = getm();
    int k = geti();  // the destination: 0 none, 1 another array, 2 the array itself
    LTA *d = k == 1 ? getlta() : k == 2 ? l : NULL;
    fflush(stdout);
    LTA *r = LTAcompose(l, left, right, d);
    fflush(stdout);
    out(r == NULL ? "null" : r == l ? "self" : r == d ? "given" : "new");
    putlta(r);
    if (k == 1 && r != d) putlta(d);
    if (r != l) putlta(l);
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
  long gdiag_no = -1;
  bool set_gdiag_no = false;
  int gxyz[3] = {-1, -1, -1};  // the debugging voxel (Gx Gy Gz), -1 as FreeSurfer starts them
  std::vector<std::string> envs;
  for (; first < line.size(); first++) {
    if (line[first][0] == '@')
      seconds = (unsigned)strtoul(line[first].c_str() + 1, NULL, 10);
    else if (line[first].compare(0, 6, "+gxyz=") == 0) {
      // +gxyz=<x>,<y>,<z> sets the debugging voxel for this call
      if (sscanf(line[first].c_str() + 6, "%d,%d,%d", &gxyz[0], &gxyz[1], &gxyz[2]) != 3)
        bad("+gxyz= needs x,y,z");
    }
    else if (line[first].compare(0, 7, "+gdiag=") == 0)
      gdiag = strtoul(line[first].c_str() + 7, NULL, 16);
    else if (line[first].compare(0, 9, "+gdiagno=") == 0) {
      gdiag_no = strtol(line[first].c_str() + 9, NULL, 10);
      set_gdiag_no = true;
    }
    else if (line[first].compare(0, 5, "+env=") == 0)
      envs.push_back(line[first].substr(5));
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
    Gx = gxyz[0];
    Gy = gxyz[1];
    Gz = gxyz[2];
    if (set_gdiag_no) {
      // as a program's diagnostic set-up leaves them: the level, and the diagnostic stream
      Gdiag_no = (int)gdiag_no;
      Gstdout = stdout;
    }
    // +env=NAME=VALUE sets a variable for this call (NAME= sets it empty)
    for (const std::string &e : envs) {
      size_t eq = e.find('=');
      if (eq == std::string::npos) bad("+env= needs NAME=VALUE");
      setenv(e.substr(0, eq).c_str(), e.substr(eq + 1).c_str(), 1);
    }
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
