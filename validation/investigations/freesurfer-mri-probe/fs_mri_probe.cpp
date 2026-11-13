// SPDX-License-Identifier: Apache-2.0
// A black-box probe of FreeSurfer 8.2.0's volume structure (specs/freesurfer-mri.md: the geometry,
// the volume, its voxels, header and colour-table copies, the tkregister and FSL matrices,
// coordinates, comparisons, names, the full copy, the volume's MINC transform, shapes, the bounding
// box and the linear transform arrays), linked against the unit oracle's libutils.a (GCC 8.4.0,
// FreeSurfer's flags). It holds only calls of FreeSurfer's functions (and of the C library's) by
// name and the reading and printing of their arguments, results and the structures' fields: no
// FreeSurfer code. The commands and encodings are in README.md beside this file; a candidate
// program takes the same ones.
//
//   fs_mri_probe batch <file>   one call per line of <file>, each in a child process of its own
//                               with an alarm; one result line per call on standard output
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

#include "colortab.h"
#include "diag.h"
#include "error.h"
#include "matrix.h"
#include "mri.h"
#include "transform.h"
#include "utils.h"

// functions of FreeSurfer's library that its headers do not declare
int MRIcopyVolGeomToMRI(MRI *mri, const VOL_GEOM *vg);
int MRIcopyVolGeomFromMRI(const MRI *mri, VOL_GEOM *vg);

const char *Progname = "fs_mri_probe";

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

static void call(const std::string &c)
{
  if (c == "alloc") {
    MRI *m = getv();
    put_volume(m);
    puti(MRIfree(&m));
    puti(m == NULL);
  }
  else if (c == "allocvg" || c == "fromvg") {
    VOL_GEOM vg;
    getg(&vg);
    int type = geti(), nf = geti(), hdr = geti();
    if (c == "allocvg") {
      MRI *m = new MRI(vg, type, nf, hdr);
      put_volume(m);
      delete m;
    }
    else {
      MRI *m = MRIallocFromVolGeom(&vg, type, nf, hdr);
      put_volume(m);
      puti(MRIfree(&m));
    }
  }
  else if (c == "free2") {
    MRI *m = getv();
    puti(MRIfree(&m));
    puti(MRIfree(&m));
  }
  else if (c == "sizeof")
    puti((long long)MRIsizeof(geti()));
  else if (c == "intent") {
    int code = geti();
    puts_(MRI::intentName(code));
    puti(MRI::intentCode(getx().c_str()));
  }
  else if (c == "xfm") {
    MRI *m = getvg();
    int base = geti(), shear = geti();
    MATRIX *given = getm();
    putm(MRIxfmCRS2XYZ(m, base, shear != 0));
    MATRIX *g = copy_or_null(given);
    putm_res(MRImatrixOfDirectionCosines(m, g), g);
    g = copy_or_null(given);
    putm_res(MRImatrixOfVoxelSizes(m, g), g);
    g = copy_or_null(given);
    putm_res(MRImatrixOfTranslations(m, g), g);
    putm(extract_i_to_r(m));
    putm(extract_r_to_i(m));
  }
  else if (c == "setv2r") {
    MRI *m = getvg();
    MATRIX *v = getm();
    puti(MRIsetVox2RASFromMatrix(m, v));
    putg(m, false);
  }
  else if (c == "p0") {
    MRI *m = getvg();
    double r0 = getd(), a0 = getd(), s0 = getd();
    puti(MRIp0ToCRAS(m, r0, a0, s0));
    putg(m, false);
  }
  else if (c == "reinit") {
    MRI *m = getvg();
    puti(MRIreInitCache(m));
    putg(m, false);
  }
  else if (c == "vox") {
    MRI *m = getv();
    fill(m, geti());
    m->outside_val = getd();
    int n = geti();
    for (int k = 0; k < n; k++) {
      std::string op = tok();
      int col = geti(), row = geti(), sl = geti(), f = geti();
      if (op == "get") {
        int mode = geti();
        putf(MRIgetVoxVal(m, col, row, sl, f, mode));
      }
      else if (op == "set") {
        int mode = geti();
        float v = getf();
        puti(MRIsetVoxVal(m, col, row, sl, f, v, mode));
      }
      else if (op == "dx")
        putf(MRIgetVoxDx(m, col, row, sl, f));
      else if (op == "dy")
        putf(MRIgetVoxDy(m, col, row, sl, f));
      else if (op == "dz")
        putf(MRIgetVoxDz(m, col, row, sl, f));
      else
        bad("unknown voxel operation");
    }
    putvox(m);
  }
  else if (c == "copyhdr" || c == "copy") {
    MRI *src = getv();
    fill(src, geti());
    seth(src);
    MRI *dst = NULL;
    if (!take("none")) {
      dst = getv();
      fill(dst, geti());
      if (c == "copyhdr")
        seth(dst);
      else
        dst->ptype = geti();
    }
    MRI *res = c == "copyhdr" ? MRIcopyHeader(src, dst) : MRIcopy(src, dst);
    out(!res ? "null" : res == dst ? "given" : "new");
    if (res) {
      puthdr(res);
      putd(res->mean);
      putct(res->ct, false);
      if (c == "copy") putvox(res);
    }
  }
  else if (c == "copyxfm") {
    MRI *src = MRIallocHeader(2, 2, 2, MRI_UCHAR, 1);
    getxfm(src);
    MRI *dst = NULL;
    if (!take("new")) {
      dst = MRIallocHeader(2, 2, 2, MRI_UCHAR, 1);
      getxfm(dst);
    }
    MRI *res = MRIcopyHeader(src, dst);
    out(res == dst ? "given" : "new");
    putxfm(res);
  }
  else if (c == "pulse") {
    MRI *a = MRIallocHeader(1, 1, 1, MRI_UCHAR, 1), *b = MRIallocHeader(1, 1, 1, MRI_UCHAR, 1);
    a->flip_angle = getd();
    a->tr = getf();
    a->te = getf();
    a->ti = getf();
    puti(MRIcopyPulseParameters(a, b));
    putd(b->flip_angle);
    putf(b->tr);
    putf(b->te);
    putf(b->ti);
  }
  else if (c == "ctab") {
    COLOR_TABLE *ct = getct();
    int counts = geti();
    COLOR_TABLE *cp = CTABdeepCopy(ct);
    putct(cp, counts != 0);
    puti(CTABfree(&cp));
    puti(cp == NULL);
  }
  else if (c == "ctabnull") {
    COLOR_TABLE *none = NULL;
    puti(CTABfree(&none));
    putct(CTABdeepCopy(NULL), false);
  }
  else if (c == "vgcopy") {
    VOL_GEOM a;
    getg_full(&a);
    VOL_GEOM b(a);
    putg(&b);
  }
  else if (c == "vgassign") {
    VOL_GEOM a, b;
    getg_full(&a);
    getg_full(&b);
    b = a;
    putg(&b);
  }
  else if (c == "usevg" || c == "vgtomri") {
    VOL_GEOM vg;
    getg(&vg);
    MRI *m = getvg();
    if (c == "usevg")
      useVolGeomToMRI(&vg, m);
    else
      puti(MRIcopyVolGeomToMRI(m, &vg));
    putg(m, false);
  }
  else if (c == "vgfrommri") {
    MRI *m = getvg();
    VOL_GEOM vg;
    getg(&vg);
    puti(MRIcopyVolGeomFromMRI(m, &vg));
    putg(&vg);
  }
  else if (c == "tkreg") {
    MRI *m = getvg();
    int base = geti(), shear = geti();
    putm(MRIxfmCRS2XYZtkreg(m));
    putm(MRIxfmCRS2XYZfsl(m));
    putm(m->get_Vox2RAS(base, shear != 0));
    putm(m->get_RAS2Vox(base, shear != 0));
    putm(m->get_Vox2TkregRAS());
    putm(m->get_TkregRAS2Vox());
    putm(m->get_RAS2TkregRAS());
    putm(m->get_TkregRAS2RAS());
  }
  else if (c == "ras2s") {
    MRI *m = getvg();
    MATRIX *given = getm();
    MATRIX *g = copy_or_null(given);
    putm_res(RASFromSurfaceRAS_(m, g), g);
    putm(VGtkreg2RAS(m, NULL));
    g = copy_or_null(given);
    putm_res(VGras2tkreg(m, g), g);
    putm(surfaceRASFromRAS_(m));
  }
  else if (c == "tk2nat" || c == "nat2tk" || c == "v2v" || c == "tkv2v") {
    MRI *a = getvg(), *b = getvg();
    MATRIX *r = getm();
    MATRIX *res = c == "tk2nat"   ? MRItkReg2Native(a, b, r)
                  : c == "nat2tk" ? MRItkRegMtx(a, b, r)
                  : c == "v2v"    ? MRItkRegMtxFromVox2Vox(a, b, r)
                                  : MRIvoxToVoxFromTkRegMtx(a, b, r);
    putm_res(res, r);
  }
  else if (c == "fixtk") {
    MRI *m = getvg();
    long seed = geti();
    MATRIX *r = getm();
    srand48(seed);
    putm_res(MRIfixTkReg(m, r), r);
    putd(drand48());
  }
  else if (c == "fsl2tk" || c == "tk2fsl") {
    if (take("unset"))
      unsetenv("FSLOUTPUTTYPE");
    else
      setenv("FSLOUTPUTTYPE", getx().c_str(), 1);
    MRI *a = getvg(), *b = getvg();
    MATRIX *r = getm();
    putm_res(c == "fsl2tk" ? MRIfsl2TkReg(a, b, r) : MRItkreg2FSL(a, b, r), r);
  }
  else if (c == "crs1") {
    MATRIX *q = getm();
    putm_res(MtxCRS1toCRS0(q), q);
  }
  else if (c == "sphinx") {
    VOL_GEOM vg;
    getg(&vg);
    puti(MRIhfs2Sphinx(&vg));
    putg(&vg);
  }
  else if (c == "v2w") {
    MRI *m = getvg();
    int cache = geti();
    if (cache == 1) {
      AffineMatrixFree(&m->i_to_r__);
      MatrixFree(&m->r_to_i__);
    }
    else if (cache == 2)
      MRIreInitCache(m);
    int n = geti();
    for (int i = 0; i < n; i++) {
      double x = getd(), y = getd(), z = getd();
      double a = 0, b = 0, d = 0;
      if (geti())
        MRIvoxelToWorld(m, x, y, z, &a, &b, &d);
      else {
        int ia = 0, ib = 0, ic = 0;
        MRIworldToVoxel(m, x, y, z, &a, &b, &d);
        MRIworldToVoxelIndex(m, x, y, z, &ia, &ib, &ic);
        puti(ia);
        puti(ib);
        puti(ic);
      }
      putd(a);
      putd(b);
      putd(d);
    }
    putg(m, false);
  }
  else if (c == "codes") {
    std::string a = getx();
    int ca = geti();
    std::string b = getx();
    int cb = geti();
    puti(MRIinterpCode(a.c_str()));
    puts_(MRIinterpString(ca));
    puti(MRIprecisionCode(b.c_str()));
    puts_(MRIprecisionString(cb));
  }
  else if (c == "vgprint") {
    VOL_GEOM vg;
    getg(&vg);
    int nocheck = geti(), fmt = geti();
    vg.vgprint(nocheck != 0);
    if (fmt)
      vg.geomprint("geometry %s %d:\n", "of", 7);
    else
      vg.geomprint();
  }
  else if (c == "vgeq") {
    VOL_GEOM a, b;
    getg(&a);
    getg(&b);
    double t = getd();
    int debug = geti();
    MATRIX *sform = getm();
    vg_isEqual_Threshold = t;
    puti(a == b);
    puti(VOL_GEOM::isNotEqualThresh(&a, &b, t));
    puti(VOL_GEOM::checkgeom(&a, &b, t, debug != 0));
    puti(VOL_GEOM::checkvox2ras(&a, &b, t, debug != 0, sform));
  }
  else if (c == "diffv2r") {
    MATRIX *a = getm(), *b = getm();
    double t = getd();
    std::string msg = getx();
    int debug = geti();
    puti(VOL_GEOM::diffvox2ras(a, b, t, msg.c_str(), debug != 0));
  }
  else if (c == "shearless") {
    MRI *m = getvg();
    m->shearless_components();
    putg(m, false);
  }
  else if (c == "tomat") {
    VOL_GEOM vg;
    getg(&vg);
    MATRIX *g = getm();
    putm_res(vg.toMatrix(g), g);
  }
  else if (c == "shape") {
    int w = geti(), h = geti(), d = geti(), f = geti();
    int w2 = geti(), h2 = geti(), d2 = geti(), f2 = geti();
    MRI::Shape a({w, h, d, f}), b({w2, h2, d2, f2});
    puti(a == b);
    puti(a != b);
  }
  else if (c == "bbox") {
    MRI *m = getbv();
    int thresh = geti();
    MRI_REGION box;
    box.x = box.y = box.z = box.dx = box.dy = box.dz = -7;
    puti(MRIboundingBox(m, thresh, &box));
    for (int v : {box.x, box.y, box.z, box.dx, box.dy, box.dz}) puti(v);
  }
  else if (c == "ltaalloc") {
    int n = geti();
    MRI *m = take("none") ? NULL : getbv();
    LTA *lta = LTAalloc(n, m);
    putlta(lta);
    puti(LTAfree(&lta));
    puti(lta == NULL);
  }
  else if (c == "ltacopy") {
    LTA *src = getlta();
    LTA *dst = take("none") ? NULL : getlta();
    LTA *res = LTAcopy(src, dst);
    out(!res ? "null" : res == dst ? "given" : "new");
    putlta(res);
    if (dst && res != dst) putlta(dst);
  }
  else if (c == "ltcopy") {
    LTA *a = getlta();
    bool none = take("null");
    LTA *b = none ? NULL : getlta();
    if (a->num_xforms < 1 || (b && b->num_xforms < 1)) bad("ltcopy needs a transform on each side");
    LT *res = LTcopy(&a->xforms[0], b ? &b->xforms[0] : NULL);
    out(!res ? "null" : b && res == &b->xforms[0] ? "given" : "other");
    if (res) putlt(res);
  }
  else if (c == "ltafreenull") {
    LTA *none = NULL;
    puti(LTAfree(&none));
  }
  else
    bad("unknown command");
}

// ---- running a line in a child process -----------------------------------------------------------

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
  const char *fo = ".fs-mri-probe.out", *fe = ".fs-mri-probe.err";
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
  std::string so = slurp(fo), se = slurp(fe);
  remove(fo);
  remove(fe);
  return res + " | " + hexbytes(so.data(), so.size()) + " " + hexbytes(se.data(), se.size());
}

int main(int argc, char **argv)
{
  if (argc != 3 || strcmp(argv[1], "batch") != 0) {
    fprintf(stderr, "usage: fs_mri_probe batch <file> (README.md)\n");
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
