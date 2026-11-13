// SPDX-License-Identifier: Apache-2.0
// fs.io's probe: FreeSurfer 8.2.0's file helpers (fio.cpp's readers and writers of one number on
// C streams and znzlib files, its name helpers and file tests, TSV::read and TSV::write;
// bfileio.cpp's byte swaps; MRIio_old.cpp's lcalloc) called case by case, linked with the unit
// oracle's libutils as the official programs are (build.sh). CLOSED TO FREESURFER CLEAN-ROOM
// IMPLEMENTERS: it names FreeSurfer's functions (running it is a black-box run of FreeSurfer's
// code).
//
//     fsio <cases>     one output line per case line (absolute path; see check.py)
//
// Every case runs in a child process, in a fixture directory made beside the cases (a file `f`,
// a directory `d` with a file `d/i`, an unreadable file `nr`, a read-only directory `ro`, links
// `ln` -> f, `dln` -> d, `bad` -> nowhere); its standard output and error go to a scratch file;
// the parent prints `<result> | <text>`, the text C-escaped (\xHH for bytes outside printable
// ASCII and for the backslash), or `EXIT <status> | <text>` / `CRASH <signal> | <text>` when
// FreeSurfer's code ended the child (each child is limited to 1 GiB of address space, 64 MiB of
// output and 5 seconds; text beyond 1 MiB is shown as `BIGTEXT <bytes>`). Numbers read are printed as their bytes in hex; strings as `N` (NULL) or `x<hex>`.

#include <fcntl.h>
#include <ftw.h>
#include <signal.h>
#include <sys/resource.h>
#include <sys/stat.h>
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

#include "bfileio.h"
#include "colortab.h"
#include "fstagsio.h"
#include "matrix.h"
#include "mri.h"
#include "tags.h"
#include "diag.h"
#include "error.h"
#include "fio.h"
#include "MRIio_old.h"
#include "znzlib.h"
#include "dti.h"
#include "fscnpy.h"
#include "gifti_io.h"
#include "mri_identify.h"
#include "DICOMRead.h"
#include "Bruker.h"
#include "signa.h"
#include "mosaic.h"
#include "matfile.h"
#include "registerio.h"

// gifti.cpp's two file-local helpers, compiled into the probe by gifti_statics.cpp
giiDataArray *fsio_gifti_alloc_and_add_darray(gifti_image *image);
double fsio_gifti_get_DA_value_2D(giiDataArray *da, int row, int col);

const char *Progname = "fsio";

namespace {

std::vector<std::string> toks;
size_t pos;
std::string out;
std::string dat;  // the data file's absolute path

[[noreturn]] void fail(const char *what)
{
  fprintf(stderr, "fsio: %s\n", what);
  exit(2);
}

const char *next()
{
  if (pos >= toks.size()) fail("missing token");
  return toks[pos++].c_str();
}
long long geti() { return strtoll(next(), NULL, 10); }
uint32_t getx32() { return (uint32_t)strtoul(next(), NULL, 16); }

std::vector<unsigned char> unhex(const char *s)
{
  std::vector<unsigned char> b;
  if (strcmp(s, "-") == 0) return b;
  for (size_t i = 0; s[i] && s[i + 1]; i += 2) {
    char t[3] = {s[i], s[i + 1], 0};
    b.push_back((unsigned char)strtoul(t, NULL, 16));
  }
  return b;
}
// `N` or `x<hex>`; `isnull` set for N
std::string getstr(bool *isnull)
{
  const char *t = next();
  *isnull = (t[0] == 'N');
  std::string s;
  if (*isnull) return s;
  std::vector<unsigned char> b = unhex(t[1] ? t + 1 : "-");
  s.assign(b.begin(), b.end());
  return s;
}

void oi(long long v) { out += std::to_string(v) + " "; }
void ox(const void *p, size_t n)
{
  // the value's bytes most significant first (x86-64 memory is little-endian)
  const unsigned char *b = (const unsigned char *)p;
  char t[4];
  for (size_t i = n; i-- > 0;) {
    snprintf(t, sizeof t, "%02x", b[i]);
    out += t;
  }
  out += " ";
}
void obytes(const std::vector<unsigned char> &b)
{
  char t[4];
  if (b.empty()) out += "-";
  for (unsigned char c : b) {
    snprintf(t, sizeof t, "%02x", c);
    out += t;
  }
  out += " ";
}
void ostr(const char *s)
{
  if (!s) { out += "N "; return; }
  out += "x";
  obytes(std::vector<unsigned char>(s, s + strlen(s)));
  if (!*s) { out.pop_back(); out.pop_back(); out += " "; }
}

void write_file(const std::vector<unsigned char> &b)
{
  FILE *fp = fopen(dat.c_str(), "wb");
  if (!fp) fail("cannot write the data file");
  if (!b.empty()) fwrite(b.data(), 1, b.size(), fp);
  fclose(fp);
}
std::vector<unsigned char> read_file(const std::string &name)
{
  std::vector<unsigned char> b;
  FILE *fp = fopen(name.c_str(), "rb");
  if (!fp) return b;
  int c;
  while ((c = fgetc(fp)) != EOF) b.push_back((unsigned char)c);
  fclose(fp);
  return b;
}
// the decompressed bytes of a znz file
std::vector<unsigned char> read_znz(const std::string &name, int gz)
{
  std::vector<unsigned char> b;
  znzFile z = znzopen(name.c_str(), "rb", gz);
  if (znz_isnull(z)) return b;
  unsigned char c;
  while (znzread(&c, 1, 1, z) == 1) b.push_back(c);
  znzclose(z);
  return b;
}


// ---- piece 2: tags, FStagsIO, VOL_GEOM, binary colour tables ----
float getf() { uint32_t u = getx32(); float f; memcpy(&f, &u, 4); return f; }
double getd() { uint64_t u = strtoull(next(), NULL, 16); double d; memcpy(&d, &u, 8); return d; }
void of(float f) { ox(&f, 4); }
void od(double d) { ox(&d, 8); }
// the bytes of a char[STRLEN] buffer up to its last non-zero byte
void obuf(const char *b)
{
  size_t n = STRLEN;
  while (n > 0 && b[n - 1] == 0) n--;
  out += "x";
  obytes(std::vector<unsigned char>(b, b + n));
  if (n == 0) { out.pop_back(); out.pop_back(); out += " "; }
}
// a C string of at most STRLEN bytes
void ocs(const char *s)
{
  size_t n = strnlen(s, STRLEN);
  out += "x";
  obytes(std::vector<unsigned char>(s, s + n));
  if (n == 0) { out.pop_back(); out.pop_back(); out += " "; }
}
// two names strcpy'd one after the other into a buffer
void names_into(char *buf)
{
  bool n1, n2;
  std::string a = getstr(&n1), b = getstr(&n2);
  strcpy(buf, a.c_str());
  strcpy(buf, b.c_str());
}
void read_vg(VOL_GEOM &v)
{
  v.valid = (int)geti(); v.width = (int)geti(); v.height = (int)geti(); v.depth = (int)geti();
  float *f[] = {&v.xsize, &v.ysize, &v.zsize, &v.x_r, &v.x_a, &v.x_s, &v.y_r, &v.y_a, &v.y_s,
                &v.z_r, &v.z_a, &v.z_s, &v.c_r, &v.c_a, &v.c_s, &v.s_r, &v.s_a, &v.s_s};
  for (float *p : f) *p = getf();
  names_into(v.fname);
}
void print_vg(const VOL_GEOM &v)
{
  oi(v.valid); oi(v.width); oi(v.height); oi(v.depth);
  const float f[] = {v.xsize, v.ysize, v.zsize, v.x_r, v.x_a, v.x_s, v.y_r, v.y_a, v.y_s,
                     v.z_r, v.z_a, v.z_s, v.c_r, v.c_a, v.c_s, v.s_r, v.s_a, v.s_s};
  for (float x : f) of(x);
  obuf(v.fname);
}
MATRIX *read_mat()
{
  int k = (int)geti();
  if (k == 0) return NULL;
  MATRIX *m = MatrixAlloc(k, k, MATRIX_REAL);
  for (int r = 1; r <= k; r++)
    for (int c = 1; c <= k; c++) m->rptr[r][c] = getf();
  return m;
}
void print_mat(const MATRIX *m)
{
  if (!m) { out += "N "; return; }
  oi(m->rows);
  for (int r = 1; r <= m->rows; r++)
    for (int c = 1; c <= m->cols; c++) of(m->rptr[r][c]);
}
COLOR_TABLE *read_ct()
{
  COLOR_TABLE *ct = (COLOR_TABLE *)calloc(1, sizeof(COLOR_TABLE));
  ct->version = (int)geti();
  ct->nentries = (int)geti();
  bool n;
  std::string fn = getstr(&n);
  strcpy(ct->fname, fn.c_str());
  ct->entries = (COLOR_TABLE_ENTRY **)calloc(ct->nentries > 0 ? ct->nentries : 1, sizeof(CTE *));
  for (int i = 0; i < ct->nentries; i++) {
    if (geti() == 0) continue;
    CTE *e = (CTE *)calloc(1, sizeof(CTE));
    std::string nm = getstr(&n);
    strcpy(e->name, nm.c_str());
    e->ri = (int)geti(); e->gi = (int)geti(); e->bi = (int)geti(); e->ai = (int)geti();
    ct->entries[i] = e;
  }
  return ct;
}
void print_ct(const COLOR_TABLE *ct)
{
  if (!ct) { out += "N "; return; }
  oi(ct->nentries); oi(ct->version); ocs(ct->fname);
  // the non-empty entries, each after its index (a table can have millions of empty places)
  for (int i = 0; i < ct->nentries; i++) {
    const CTE *e = ct->entries[i];
    if (!e) continue;
    out += "@" + std::to_string(i) + " ";
    ocs(e->name); oi(e->ri); oi(e->gi); oi(e->bi); oi(e->ai);
    of(e->rf); of(e->gf); of(e->bf); of(e->af);
  }
}
void read_frame(MRI_FRAME *f)
{
  f->type = (int)geti();
  f->TE = getf(); f->TR = getf(); f->flip = getf(); f->TI = getf(); f->TD = getf();
  f->TM = getd();
  f->sequence_type = (int)geti();
  f->echo_spacing = getf(); f->echo_train_len = getf();
  for (int i = 0; i < 3; i++) f->read_dir[i] = getf();
  for (int i = 0; i < 3; i++) f->pe_dir[i] = getf();
  for (int i = 0; i < 3; i++) f->slice_dir[i] = getf();
  f->label = (int)geti();
  names_into(f->name);
  f->dof = (int)geti();
  if (f->m_ras2vox) MatrixFree(&f->m_ras2vox);
  f->m_ras2vox = read_mat();
  f->thresh = getf();
  f->units = (int)geti();
  f->DX = getd(); f->DY = getd(); f->DZ = getd(); f->DR = getd(); f->DP = getd(); f->DS = getd();
  f->bvalue = getd();
  long *ramp[] = {&f->D1_ramp, &f->D2_ramp, &f->D3_ramp, &f->D4_ramp};
  long *flat[] = {&f->D1_flat, &f->D2_flat, &f->D3_flat, &f->D4_flat};
  double *amp[] = {&f->D1_amp, &f->D2_amp, &f->D3_amp, &f->D4_amp};
  for (int i = 0; i < 4; i++) { *ramp[i] = strtoll(next(), NULL, 10); *flat[i] = strtoll(next(), NULL, 10); *amp[i] = getd(); }
}
void print_frame(const MRI_FRAME *f)
{
  oi(f->type); of(f->TE); of(f->TR); of(f->flip); of(f->TI); of(f->TD); od(f->TM);
  oi(f->sequence_type); of(f->echo_spacing); of(f->echo_train_len);
  for (int i = 0; i < 3; i++) of(f->read_dir[i]);
  for (int i = 0; i < 3; i++) of(f->pe_dir[i]);
  for (int i = 0; i < 3; i++) of(f->slice_dir[i]);
  oi(f->label); obuf(f->name); oi(f->dof); print_mat(f->m_ras2vox); of(f->thresh); oi(f->units);
  od(f->DX); od(f->DY); od(f->DZ); od(f->DR); od(f->DP); od(f->DS); od(f->bvalue);
  const long ramp[] = {f->D1_ramp, f->D2_ramp, f->D3_ramp, f->D4_ramp};
  const long flat[] = {f->D1_flat, f->D2_flat, f->D3_flat, f->D4_flat};
  const double amp[] = {f->D1_amp, f->D2_amp, f->D3_amp, f->D4_amp};
  for (int i = 0; i < 4; i++) { oi(ramp[i]); oi(flat[i]); od(amp[i]); }
}
// the data file in mode 0 (plain), 1 (compressed) or 2 (plain, opened as compressed)
znzFile data_znz(int mode, const std::vector<unsigned char> &b)
{
  if (mode == 1) {
    znzFile w = znzopen(dat.c_str(), "wb", 1);
    if (znz_isnull(w)) fail("cannot write the data file");
    if (!b.empty()) znzwrite((void *)b.data(), 1, b.size(), w);
    znzclose(w);
  } else {
    write_file(b);
  }
  znzFile z = znzopen(dat.c_str(), "rb", mode != 0);
  if (znz_isnull(z)) fail("cannot open the data file");
  return z;
}
int ***labels_alloc(int x0, int y0, int z0)
{
  int ***l = (int ***)calloc(x0 > 0 ? x0 : 1, sizeof(int **));
  for (int x = 0; x < x0; x++) {
    l[x] = (int **)calloc(y0 > 0 ? y0 : 1, sizeof(int *));
    for (int y = 0; y < y0; y++) {
      l[x][y] = (int *)calloc(z0 > 0 ? z0 : 1, sizeof(int));
      for (int z = 0; z < z0; z++) l[x][y][z] = -1;
    }
  }
  return l;
}
bool more() { return pos < toks.size(); }

// ---- piece 3: .npy volumes, gifti.cpp's two helpers, dti.cpp's b-value files and names ----
// a fixture name, or `-` for the file x.npy written in the fixture from the hex token that follows
// (a name relative to the fixture: messages that print it read the same on both sides)
std::string where_of(const char *w, const char *hexdata)
{
  if (strcmp(w, "-") == 0) {
    std::vector<unsigned char> b = unhex(hexdata);
    FILE *fp = fopen("x.npy", "wb");
    if (!fp) fail("cannot write x.npy");
    if (!b.empty()) fwrite(b.data(), 1, b.size(), fp);
    fclose(fp);
    return "x.npy";
  }
  return w;
}
// a file's bytes, or N when there is no such file
void ofile(const std::string &name)
{
  struct stat st;
  if (stat(name.c_str(), &st) != 0 || !S_ISREG(st.st_mode)) { out += "N "; return; }
  obytes(read_file(name));
}
MATRIX *read_rmat()
{
  int rows = (int)geti(), cols = (int)geti();
  if (rows < 0) return NULL;
  MATRIX *m = MatrixAlloc(rows, cols, MATRIX_REAL);
  for (int r = 1; r <= rows; r++)
    for (int c = 1; c <= cols; c++) m->rptr[r][c] = getf();
  return m;
}
void print_rmat(const MATRIX *m)
{
  if (!m) { out += "N "; return; }
  oi(m->rows); oi(m->cols);
  for (int r = 1; r <= m->rows; r++)
    for (int c = 1; c <= m->cols; c++) of(m->rptr[r][c]);
}

// piece 4 (3.1-27): the format probe. A case's files are made in the fixture's folder `w`
// (removed first): `-`, or entries separated by `,`: `D<hex name>` a folder, `F<hex name>:<size>`
// a file of that many zero bytes, then `+<offset>:<hex bytes>` patches.
int rm_entry(const char *p, const struct stat *, int, struct FTW *);
void setup(const char *spec)
{
  nftw("w", rm_entry, 16, FTW_DEPTH | FTW_PHYS);
  if (mkdir("w", 0700)) fail("cannot make w");
  if (strcmp(spec, "-") == 0) return;
  std::string all(spec);
  size_t p = 0;
  while (p <= all.size()) {
    size_t q = all.find(',', p);
    if (q == std::string::npos) q = all.size();
    std::string e = all.substr(p, q - p);
    p = q + 1;
    if (e.empty()) continue;
    if (e[0] == 'D') {
      std::vector<unsigned char> n = unhex(e.c_str() + 1);
      if (mkdir(std::string(n.begin(), n.end()).c_str(), 0700)) fail("cannot make a folder");
      continue;
    }
    size_t c = e.find(':');
    std::vector<unsigned char> n = unhex(e.substr(1, c - 1).c_str());
    size_t plus = e.find('+', c);
    long size = strtol(e.substr(c + 1, plus == std::string::npos ? std::string::npos : plus - c - 1).c_str(), NULL, 10);
    std::vector<unsigned char> b(size, 0);
    while (plus != std::string::npos) {
      size_t c2 = e.find(':', plus);
      size_t nx = e.find('+', plus + 1);
      long off = strtol(e.substr(plus + 1, c2 - plus - 1).c_str(), NULL, 10);
      std::vector<unsigned char> h = unhex(e.substr(c2 + 1, nx == std::string::npos ? std::string::npos : nx - c2 - 1).c_str());
      for (size_t k = 0; k < h.size(); k++)
        if (off + (long)k < (long)b.size()) b[off + k] = h[k];
      plus = nx;
    }
    FILE *fp = fopen(std::string(n.begin(), n.end()).c_str(), "wb");
    if (!fp) fail("cannot write a case file");
    if (!b.empty()) fwrite(b.data(), 1, b.size(), fp);
    fclose(fp);
  }
}

// piece 5 (3.1-27): mosaics and MATLAB files
void print_anymat(const MATRIX *m)
{
  if (!m) { out += "N "; return; }
  oi(m->rows); oi(m->cols); oi(m->type);
  // more than 65,536 numbers: FNV-1a (64 bits) of the floats' bytes in memory order
  static long hash_min = getenv("FSIO_HASH_MIN") ? atol(getenv("FSIO_HASH_MIN")) : 65536;
  bool big = (long)m->rows * m->cols > hash_min;
  uint64_t h = 0xcbf29ce484222325ull;
  auto put = [&](float f) {
    if (!big) { of(f); return; }
    unsigned char b[4];
    memcpy(b, &f, 4);
    for (unsigned char x : b) { h ^= x; h *= 1099511628211ull; }
  };
  for (int r = 1; r <= m->rows; r++)
    for (int c = 1; c <= m->cols; c++) {
      if (m->type == MATRIX_COMPLEX) { put(MATRIX_CELT_REAL(m, r, c)); put(MATRIX_CELT_IMAG(m, r, c)); }
      else put(*MATRIX_RELT(m, r, c));
    }
  if (big) { char t[32]; snprintf(t, sizeof t, "fnv%016" PRIx64 " ", h); out += t; }
}


// piece 6 (relay 16): reading volumes (MRIreadHeader, MRIreadInfo, MRIreadType, mghRead) and
// registration files (regio_read_register). Names are printed with the fixture's path removed and
// escaped (the port's fixture is elsewhere).
std::string fixpfx;
}
MRI *mri_read(const char *fname, int type, int volume_flag, int start_frame, int end_frame,
              std::vector<MRI *> *mrivector);
namespace {
void oname(const char *s)
{
  if (!s) { out += "N "; return; }
  std::string t(s);
  for (size_t at; !fixpfx.empty() && (at = t.find(fixpfx)) != std::string::npos;) t.erase(at, fixpfx.size());
  out += "n";
  char b[8];
  for (unsigned char c : t) {
    if (c <= 0x20 || c > 0x7e || c == '\\') { snprintf(b, sizeof b, "\\x%02x", c); out += b; }
    else out += (char)c;
  }
  out += " ";
}
void fnv_bytes(const unsigned char *b, size_t n)
{
  uint64_t h = 0xcbf29ce484222325ull;
  for (size_t i = 0; i < n; i++) { h ^= b[i]; h *= 1099511628211ull; }
  char t[32];
  snprintf(t, sizeof t, "fnv%016" PRIx64 " ", h);
  out += t;
}
void print_t(const Transform *t)
{
  if (!t) { out += "N "; return; }
  for (int i = 0; i < 4; i++)
    for (int j = 0; j < 4; j++) od(Transform_elem(*t, i, j));
}
// a VOL_GEOM of an MRI's morph fields: valid and the extent are memory contents until a morph tag
// sets them (VOL_GEOM's constructor leaves them), so they are not printed
void print_vg6(const VOL_GEOM &v)
{
  const float f[] = {v.xsize, v.ysize, v.zsize, v.x_r, v.x_a, v.x_s, v.y_r, v.y_a, v.y_s,
                     v.z_r, v.z_a, v.z_s, v.c_r, v.c_a, v.c_s, v.s_r, v.s_a, v.s_s};
  for (float x : f) of(x);
  oname(v.fname);
}
void print_mri6(const MRI *m)
{
  if (!m) { out += "N "; return; }
  oi(m->width); oi(m->height); oi(m->depth); oi(m->nframes); oi(m->type); oi(m->imnr0); oi(m->imnr1);
  const float f[] = {m->xsize, m->ysize, m->zsize, m->x_r, m->x_a, m->x_s, m->y_r, m->y_a, m->y_s,
                     m->z_r, m->z_a, m->z_s, m->c_r, m->c_a, m->c_s};
  for (float x : f) of(x);
  oi(m->ras_good_flag);
  const float g[] = {m->xstart, m->xend, m->ystart, m->yend, m->zstart, m->zend, m->fov, m->thick, m->ps,
                     m->tr, m->te, m->ti};
  for (float x : g) of(x);
  od(m->flip_angle); of(m->FieldStrength);
  oi(m->dof); oi(m->version); oi(m->intent);
  oi(m->ncmds);
  for (int i = 0; i < m->ncmds; i++) oname(m->cmdlines[i]);
  oname(m->transform_fname); oi(m->free_transform);
  print_t(m->linear_transform); print_t(m->inverse_linear_transform);
  oname(m->pedir);
  print_rmat(m->AutoAlign); print_rmat(m->origRas2Vox);
  oi(m->warpFieldFormat); oi(m->gcamorphSpacing); od(m->gcamorphExp_k);
  print_vg6(m->gcamorph_image_vg); print_vg6(m->gcamorph_atlas_vg);
  print_rmat(m->gcamorphAffine);
  if (!m->gcamorphLabel) out += "N ";
  else {
    std::vector<unsigned char> b;
    for (int c = 0; c < m->width; c++)
      for (int r = 0; r < m->height; r++)
        for (int k = 0; k < m->depth; k++) {
          int v = m->gcamorphLabel[c][r][k];
          unsigned char t[4];
          memcpy(t, &v, 4);
          b.insert(b.end(), t, t + 4);
        }
    fnv_bytes(b.data(), b.size());
  }
  oname(m->fname);
  if (!m->i_to_r__) out += "N ";
  else for (int i = 0; i < 16; i++) of(m->i_to_r__->mat[i]);
  print_rmat(m->r_to_i__);
  if (!m->chunk || !m->ischunked) out += "nochunk ";
  else fnv_bytes((const unsigned char *)m->chunk, m->bytes_total);
}
void set_env6(int bits)
{
  if (bits & 1) setenv("FS_SKIP_TAGS", "1", 1);
  if (bits & 2) setenv("FS_MGZIO_USEVOXELBUFREAD", "1", 1);
}

void run_piece6(const std::string &cmd)
{
  char cwd[4096];
  if (getcwd(cwd, sizeof cwd)) fixpfx = std::string(cwd) + "/";
  // standard error byte-oriented, as in a program that has written to it: glibc's perror then
  // leaves errno alone (on an unoriented, write-only standard error it sets EINVAL)
  fwide(stderr, -1);
  if (cmd == "mrh" || cmd == "mri" || cmd == "mrd" || cmd == "mgr") {
    Gdiag = strtoull(next(), NULL, 16);
    if (cmd != "mgr") MRIIO_Strip_Pound = (int)geti();
    set_env6((int)geti());
    int type = -1, rv = 0, frame = -1;
    if (cmd == "mrh" || cmd == "mrd") type = (int)geti();
    if (cmd == "mgr") { rv = (int)geti(); frame = (int)geti(); }
    bool nn;
    std::string name = getstr(&nn);
    setup(next());
    errno = 0;
    MRI *m = NULL;
    if (cmd == "mrh") m = MRIreadHeader(name.c_str(), type);
    else if (cmd == "mri") m = MRIreadInfo(name.c_str());
    else if (cmd == "mrd") m = mri_read(name.c_str(), type, TRUE, -1, -1, NULL);
    else m = mghRead(name.c_str(), rv, frame);
    print_mri6(m);
    oi(errno);
  } else if (cmd == "rrr") {
    bool nn;
    std::string name = getstr(&nn);
    setup(next());
    errno = 0;
    char *subject = NULL;
    float ip = -7, bp = -7, in = -7;
    MATRIX *R = NULL;
    int f2i = -7;
    int r = regio_read_register(name.c_str(), &subject, &ip, &bp, &in, &R, &f2i);
    oi(r); oname(subject); of(ip); of(bp); of(in); print_rmat(R); oi(f2i);
    oi(errno);
  } else {
    fail("unknown command");
  }
}

void run_piece5(const std::string &cmd)
{
  if (cmd == "mvm" || cmd == "mmv") {
    int a[7];
    for (int &v : a) v = (int)geti();
    int null_oob = (int)geti();
    int x = -7, y = -7, z = -7, oob = 7;
    int r;
    if (cmd == "mvm")
      r = VolSS2MosSS(a[0], a[1], a[2], a[3], a[4], a[5], a[6], &x, &y, null_oob ? NULL : &oob);
    else
      r = MosSS2VolSS(a[0], a[1], a[2], a[3], a[4], a[5], a[6], &x, &y, &z, null_oob ? NULL : &oob);
    oi(r); oi(x); oi(y); oi(z); oi(oob);
  } else if (cmd == "mck") {
    oi(CheckMosaic());
  } else if (cmd == "mlw") {
    bool n;
    std::string name = getstr(&n);
    int rows = (int)geti(), cols = (int)geti(), cplx = (int)geti();
    MATRIX *m = MatrixAlloc(rows, cols, cplx ? MATRIX_COMPLEX : MATRIX_REAL);
    for (int r = 1; r <= rows; r++)
      for (int c = 1; c <= cols; c++) {
        if (cplx) { MATRIX_CELT_REAL(m, r, c) = getf(); MATRIX_CELT_IMAG(m, r, c) = getf(); }
        else *MATRIX_RELT(m, r, c) = getf();
      }
    bool fn_null;
    std::string fname = getstr(&fn_null);
    setup(next());
    errno = 0;
    oi(MatlabWrite(m, fname.c_str(), n ? NULL : name.c_str()));
    ofile(fname);
    MatrixFree(&m);
  } else if (cmd == "mlr" || cmd == "mfr" || cmd == "mrc") {
    Gdiag = strtoull(next(), NULL, 16);
    Gdiag_no = (int)geti();
    int type = cmd == "mfr" ? (int)geti() : 0;
    bool vn = true;
    std::string var = cmd == "mrc" ? getstr(&vn) : "";
    bool fn_null;
    std::string fname = getstr(&fn_null);
    setup(next());
    errno = 0;
    if (cmd == "mlr") {
      MATRIX *m = MatlabRead(fname.c_str());
      print_anymat(m);
      if (m) MatrixFree(&m);
    } else if (cmd == "mfr") {
      MATFILE *mf = MatFileRead(fname.c_str(), type);
      if (!mf) out += "N ";
      else {
        oi(mf->type); oi(mf->mrows); oi(mf->ncols); oi(mf->imagf); oi(mf->namlen); oi(mf->version);
        long n = (long)mf->mrows * mf->ncols * (type == MAT_FLOAT ? 4 : 1);
        if (!mf->data || n < 0) out += "N ";
        else obytes(std::vector<unsigned char>(mf->data, mf->data + n));
      }
    } else {
      MLFC *c = ReadMatlabFileContents(fname.c_str());
      if (!c) out += "N ";
      else { oi(c->nvars); MLFCprint(stdout, c); MLFCfree(&c); }
      if (!vn) print_anymat(ReadMatlabFileVariable(fname.c_str(), var.c_str()));
    }
    oi(errno);
    Gdiag = 0;
    Gdiag_no = -1;
  } else {
    run_piece6(cmd);
  }
}

void run_piece4(const std::string &cmd)
{
  if (cmd == "sty") {
    bool n;
    std::string s = getstr(&n);
    oi(string_to_type(s.c_str()));
  } else if (cmd == "tts") {
    char *r = type_to_string((int)geti());
    ostr(r);
    free(r);
  } else if (cmd == "mgvn") {
    MRIIO_Strip_Pound = (int)geti();
    bool n;
    std::string s = getstr(&n);
    static char buf[1 << 16];
    MRIgetVolumeName(s.c_str(), buf);
    ostr(buf);
    MRIIO_Strip_Pound = 1;
  } else if (cmd == "mid" || cmd == "isx" || cmd == "idf" || cmd == "dcm") {
    std::string fn = cmd == "mid" ? "" : next();
    int dbg = (cmd == "mid" || cmd == "dcm") ? (int)geti() : 0;
    bool n;
    std::string name = getstr(&n);
    setup(next());
    if (dbg) setenv("FS_DICOM_DEBUG", "1", 1);
    char *c = (char *)name.c_str();
    errno = 0;
    if (cmd == "mid") {
      oi(mri_identify(c));
    } else if (cmd == "dcm") {
      for (char k : fn) {
        if (k == 'd') oi(IsDICOM(c));
        else if (k == 's') oi(IsSiemensDICOM(c));
        else if (k == 'm') oi(mri_identify(c));
        else fail("unknown dcm step");
      }
    } else if (cmd == "isx") {
      int r;
      if (fn == "cor") r = is_cor(c);
      else if (fn == "brik") r = is_brik(c);
      else if (fn == "siemens") r = is_siemens(c);
      else if (fn == "genesis") r = is_genesis(c);
      else if (fn == "gelx") r = is_ge_lx(c);
      else if (fn == "analyze") r = is_analyze(c);
      else if (fn == "mnc") r = is_mnc(c);
      else if (fn == "mgh") r = is_mgh(c);
      else if (fn == "bshort") r = is_bshort(c);
      else if (fn == "bfloat") r = is_bfloat(c);
      else if (fn == "bhdr") r = is_bhdr(c);
      else if (fn == "sdt") r = is_sdt(c);
      else if (fn == "gdf") r = is_gdf(c);
      else if (fn == "otl") r = is_otl(c);
      else if (fn == "ximg") r = is_ximg(c);
      else if (fn == "nifti1") r = is_nifti1(c);
      else if (fn == "nii") r = is_nii(c);
      else if (fn == "nrrd") r = is_nrrd(c);
      else if (fn == "curv") r = IDisCurv(c);
      else if (fn == "signa") r = is_signa(c);
      else if (fn == "bruker") r = is_bruker(c);
      else fail("unknown test");
      oi(r);
    } else {
      if (fn == "stem") { char *r = IDstemFromName(c); ostr(r); free(r); }
      else if (fn == "ext") { char *r = IDextensionFromName(c); ostr(r); free(r); }
      else if (fn == "name") { char *r = IDnameFromStem(c); ostr(r); free(r); }
      else if (fn == "type") oi(IDtypeFromStem(c));
      else if (fn == "bstem") { char *r = bhdr_stem(c); ostr(r); free(r); }
      else if (fn == "bfirst") { char *r = bhdr_firstslicefname(c); ostr(r); free(r); }
      else if (fn == "bprecs") { char *r = bhdr_precisionstring(c); ostr(r); free(r); }
      else if (fn == "bprec") oi(bhdr_precision(c));
      else if (fn == "bruk") {
        static char f[5][4096];
        memset(f, 0, sizeof f);
        oi(checkBrukerFiles(c, f[0], f[1], f[2], f[3], f[4], 1));
        for (int k = 0; k < 5; k++) ostr(f[k]);
      } else fail("unknown helper");
    }
    oi(errno);
    if (cmd == "dcm" || cmd == "mid") COND_DumpConditions();
  } else {
    run_piece5(cmd);
  }
}

void run_piece3(const std::string &cmd)
{
  if (cmd == "npy") {
    int verbose = (int)geti();
    const char *w = next();
    std::string path = where_of(w, next());
    fscnpy *c = new fscnpy();
    MRI *m = c->npy2mri(path.c_str(), verbose != 0);
    if (!m) { out += "N "; return; }
    oi(m->width); oi(m->height); oi(m->depth); oi(m->nframes); oi(m->type);
    oi((long long)m->bytes_per_vox); oi((long long)m->bytes_total);
    obytes(std::vector<unsigned char>((unsigned char *)m->chunk,
                                      (unsigned char *)m->chunk + (m->chunk ? m->bytes_total : 0)));
  } else if (cmd == "gda") {
    int verb = (int)geti();
    int isnull = (int)geti();
    int n = (int)geti();
    gifti_set_verb(verb);
    gifti_image *im = isnull ? NULL : (gifti_image *)calloc(1, sizeof(gifti_image));
    giiDataArray *da = NULL;
    for (int k = 0; k < n; k++) {
      da = fsio_gifti_alloc_and_add_darray(im);
      oi(da != NULL);
      if (im) { oi(im->numDA); oi(da != NULL && da == im->darray[im->numDA - 1]); }
    }
    if (da) {
      oi(da->intent); oi(da->datatype); oi(da->ind_ord); oi(da->num_dim);
      for (int i = 0; i < 6; i++) oi(da->dims[i]);
      oi(da->encoding); oi(da->endian); oi(da->ext_fname != NULL); oi(da->ext_offset);
      oi(da->meta.length); oi(da->numCS); oi(da->data != NULL); oi(da->nvals); oi(da->nbyper);
      oi(da->ex_atrs.length);
    }
    gifti_set_verb(1);
  } else if (cmd == "gdv") {
    int isnull = (int)geti(), hasdata = (int)geti();
    giiDataArray *da = isnull ? NULL : (giiDataArray *)calloc(1, sizeof(giiDataArray));
    int ind_ord = (int)geti(), num_dim = (int)geti(), d0 = (int)geti(), d1 = (int)geti();
    int datatype = (int)geti(), row = (int)geti(), col = (int)geti();
    std::vector<unsigned char> b = unhex(next());
    if (da) {
      da->ind_ord = ind_ord; da->num_dim = num_dim; da->dims[0] = d0; da->dims[1] = d1;
      da->datatype = datatype;
      if (hasdata) {
        // room after the values: a read FreeSurfer makes beyond them stays in the probe's memory
        da->data = calloc(b.size() + 64, 1);
        memcpy(da->data, b.data(), b.size());
      }
    }
    double v = fsio_gifti_get_DA_value_2D(da, row, col);
    od(v);
  } else if (cmd == "bvw") {
    std::string which = next();
    MATRIX *m = read_rmat();
    const char *w = next();
    std::string path = strcmp(w, "-") == 0 ? dat : std::string(w);
    if (strcmp(w, "-") == 0) unlink(dat.c_str());
    int r = which == "v" ? DTIwriteBValues(m, path.c_str()) : DTIwriteBVectors(m, path.c_str());
    oi(r);
    ofile(path);
  } else if (cmd == "psn") {
    bool n;
    std::string s = getstr(&n);
    uint64_t u = 0x7ff8deadbeef0001ull;
    double b;
    memcpy(&b, &u, 8);
    int nth = -7777;
    oi(DTIparsePulseSeqName(s.c_str(), &b, &nth));
    od(b);
    oi(nth);
  } else if (cmd == "bcs") {
    Gdiag_no = (int)geti();
    int space = (int)geti(), desired = (int)geti();
    MRI *mri = MRIallocHeader(1, 1, 1, MRI_UCHAR, 1);
    float *f[] = {&mri->x_r, &mri->x_a, &mri->x_s, &mri->y_r, &mri->y_a, &mri->y_s,
                  &mri->z_r, &mri->z_a, &mri->z_s};
    for (float *p : f) *p = getf();
    mri->bvec_space = space;
    mri->bvecs = read_rmat();
    oi(DTIbvecChangeSpace(mri, desired));
    oi(mri->bvec_space);
    print_rmat(mri->bvecs);
    Gdiag_no = -1;
  } else {
    run_piece4(cmd);
  }
}

void run_piece2(const std::string &cmd)
{
  if (cmd == "vgw") {
    int gz = (int)geti(), nifti = (int)geti(), shearless = (int)geti();
    VOL_GEOM v;
    read_vg(v);
    znzFile z = znzopen(dat.c_str(), "wb", gz);
    v.write(z, nifti, shearless);
    znzclose(z);
    obytes(read_znz(dat, gz));
  } else if (cmd == "vgr") {
    int mode = (int)geti(), nifti = (int)geti(), shearless = (int)geti();
    Gdiag_no = (int)geti();
    VOL_GEOM v;
    read_vg(v);
    znzFile z = data_znz(mode, unhex(next()));
    v.read(z, nifti, shearless);
    print_vg(v);
    oi(znzeof(z) != 0);
    oi(znztell(z));
    znzclose(z);
  } else if (cmd == "fsw") {
    std::string op = next();
    int gz = (int)geti(), nifti = (int)geti();
    Gdiag = (int)geti();
    znzFile z = znzopen(dat.c_str(), "wb", gz);
    FStagsIO io(z, nifti);
    int r = 0;
    if (op == "tag") {
      int tag = (int)geti();
      std::vector<unsigned char> d = unhex(next());
      long long dlen = geti();
      d.resize(d.size() + 8, 0);
      r = io.write_tag(tag, d.data(), dlen);
    } else if (op == "mat") {
      int tag = (int)geti();
      MATRIX *m = read_mat();
      r = io.write_matrix(m, tag);
    } else if (op == "ctab") {
      r = io.write_old_colortable(read_ct());
    } else if (op == "frames") {
      int nf = (int)geti();
      MRI *mri = MRIallocHeader(1, 1, 1, MRI_UCHAR, nf);
      for (int i = 0; i < nf; i++) read_frame(&mri->frames[i]);
      r = io.write_mri_frames(mri);
    } else if (op == "geom") {
      int shearless = (int)geti();
      VOL_GEOM a, b;
      read_vg(a);
      read_vg(b);
      r = io.write_gcamorph_geom(&a, &b, shearless);
    } else if (op == "meta") {
      int a = (int)geti(), b = (int)geti();
      r = io.write_gcamorph_meta(a, b, getd());
    } else if (op == "labels") {
      int x0 = (int)geti(), y0 = (int)geti(), z0 = (int)geti();
      int ***l = labels_alloc(x0, y0, z0);
      for (int x = 0; x < x0; x++)
        for (int y = 0; y < y0; y++)
          for (int zz = 0; zz < z0; zz++) l[x][y][zz] = (int)geti();
      r = io.write_gcamorph_labels(x0, y0, z0, l);
    } else if (op == "dof") {
      r = io.write_dof((int)geti());
    } else if (op == "scan") {
      MRI *mri = MRIallocHeader(1, 1, 1, MRI_UCHAR, 1);
      mri->te = getf(); mri->ti = getf(); mri->flip_angle = getd(); mri->FieldStrength = getf();
      bool n;
      std::string pe = getstr(&n);
      if (!n) mri->pedir = strdup(pe.c_str());
      r = io.write_scan_parameters(mri);
    } else if (op == "ras" || op == "rasx") {
      VOL_GEOM v;
      read_vg(v);
      r = op == "ras" ? io.write_ras_xform(&v) : io.write_ras_xform_extra(&v);
    } else if (op == "end") {
      r = io.write_endtag();
    } else fail("unknown writer");
    znzclose(z);
    oi(r);
    obytes(read_znz(dat, gz));
  } else if (cmd == "fsr") {
    int mode = (int)geti(), nifti = (int)geti();
    Gdiag = (int)geti();
    Gdiag_no = (int)geti();
    znzFile z = data_znz(mode, unhex(next()));
    FStagsIO io(z, nifti);
    while (more()) {
      std::string op = next();
      out += op + ": ";
      if (op == "t") {
        int zl = (int)geti();
        long long len = -7777;
        oi(io.read_tagid_len(&len, zl));
        oi(len);
      } else if (op == "d") {
        long long n = geti();
        std::vector<unsigned char> b((size_t)n + 1, 0xee);
        oi(io.read_data(b.data(), n));
        b.resize((size_t)n);
        obytes(b);
      } else if (op == "m") {
        errno = 0;
        MATRIX *m = io.read_matrix();
        int e = errno;
        print_mat(m);
        oi(e);
      } else if (op == "c") {
        print_ct(io.read_old_colortable());
      } else if (op == "f") {
        int nf = (int)geti();
        long long len = geti();
        MRI *mri = MRIallocHeader(1, 1, 1, MRI_UCHAR, nf);
        oi(io.read_mri_frames(mri, len));
        for (int i = 0; i < nf; i++) print_frame(&mri->frames[i]);
      } else if (op == "g") {
        int shearless = (int)geti();
        VOL_GEOM a, b;
        read_vg(a);
        read_vg(b);
        oi(io.read_gcamorph_geom(&a, &b, shearless));
        print_vg(a);
        print_vg(b);
      } else if (op == "e") {
        int a = 0, b = 0;
        double c = 0;
        oi(io.read_gcamorph_meta(&a, &b, &c));
        oi(a); oi(b); od(c);
      } else if (op == "l") {
        int x0 = (int)geti(), y0 = (int)geti(), z0 = (int)geti();
        int ***l = labels_alloc(x0, y0, z0);
        oi(io.read_gcamorph_labels(x0, y0, z0, l));
        for (int x = 0; x < x0; x++)
          for (int y = 0; y < y0; y++)
            for (int zz = 0; zz < z0; zz++) oi(l[x][y][zz]);
      } else if (op == "o") {
        int d = 0;
        oi(io.read_dof(&d));
        oi(d);
      } else if (op == "s") {
        long long dlen = geti();
        MRI *mri = MRIallocHeader(1, 1, 1, MRI_UCHAR, 1);
        oi(io.read_scan_parameters(mri, dlen));
        of(mri->te); of(mri->ti); od(mri->flip_angle); of(mri->FieldStrength);
        if (mri->pedir) ocs(mri->pedir); else out += "N ";
      } else if (op == "r" || op == "x") {
        VOL_GEOM v;  // C++ leaves these four unset (memory contents): the probe sets them
        v.valid = v.width = v.height = v.depth = 0;
        oi(op == "r" ? io.read_ras_xform(&v) : io.read_ras_xform_extra(&v));
        print_vg(v);
      } else if (op == "k") {
        int tag = (int)geti();
        long long len = geti();
        oi(io.skip_tag(tag, len));
      } else fail("unknown reader");
    }
    oi(znzeof(z) != 0);
    oi(znztell(z));
    znzclose(z);
  } else if (cmd == "fsl") {
    std::string op = next();
    long long r = 0;
    if (op == "tag") {
      int tag = (int)geti(); long long len = geti(); int n = (int)geti(), a = (int)geti();
      r = FStagsIO::getlen_tag(tag, len, n, a);
    } else if (op == "mat") {
      int n = (int)geti(), a = (int)geti();
      r = FStagsIO::getlen_matrix(n, a);
    } else if (op == "ctab") {
      int n = (int)geti(), a = (int)geti();
      r = FStagsIO::getlen_old_colortable(read_ct(), n, a);
    } else if (op == "frames") {
      int nf = (int)geti(), n = (int)geti(), a = (int)geti();
      MRI *mri = MRIallocHeader(1, 1, 1, MRI_UCHAR, nf);
      r = FStagsIO::getlen_mri_frames(mri, n, a);
    } else if (op == "geom") {
      bool n1, n2;
      std::string s = getstr(&n1), t = getstr(&n2);
      int n = (int)geti(), a = (int)geti(), sh = (int)geti();
      r = FStagsIO::getlen_gcamorph_geom(s.c_str(), t.c_str(), n, a, sh);
    } else if (op == "meta") {
      r = FStagsIO::getlen_gcamorph_meta((int)geti());
    } else if (op == "labels") {
      int x = (int)geti(), y = (int)geti(), zz = (int)geti(), l = (int)geti(), n = (int)geti(), a = (int)geti();
      r = FStagsIO::getlen_gcamorph_labels(x, y, zz, l, n, a);
    } else if (op == "dof") {
      r = FStagsIO::getlen_dof(0, (int)geti());
    } else if (op == "scan") {
      int a = (int)geti();
      bool n;
      std::string pe = getstr(&n);
      MRI *mri = MRIallocHeader(1, 1, 1, MRI_UCHAR, 1);
      if (!n) mri->pedir = strdup(pe.c_str());
      r = FStagsIO::getlen_scan_parameters(mri, a);
    } else if (op == "ras") {
      VOL_GEOM v;
      r = FStagsIO::getlen_ras_xform(&v, (int)geti());
    } else if (op == "rasx") {
      VOL_GEOM v;
      r = FStagsIO::getlen_ras_xform_extra(&v, (int)geti());
    } else if (op == "end") {
      r = FStagsIO::getlen_endtag((int)geti());
    } else if (op == "sizeof") {
      r = (long long)sizeof(MRI_FRAME) * 1000000 + (long long)sizeof(CTE);
    } else fail("unknown length");
    oi(r);
  } else if (cmd == "ctw") {
    int stream = (int)geti(), gz = (int)geti();
    Gdiag = (int)geti();
    COLOR_TABLE *ct = read_ct();
    int r;
    if (stream == 0) {
      FILE *fp = fopen(dat.c_str(), "wb");
      r = CTABwriteIntoBinary(ct, fp);
      fclose(fp);
      oi(r);
      obytes(read_file(dat));
    } else {
      znzFile z = znzopen(dat.c_str(), "wb", gz);
      r = znzCTABwriteIntoBinary(ct, z);
      znzclose(z);
      oi(r);
      obytes(read_znz(dat, gz));
    }
  } else if (cmd == "ctr") {
    int stream = (int)geti(), mode = (int)geti();
    Gdiag = (int)geti();
    Gdiag_no = (int)geti();
    std::vector<unsigned char> b = unhex(next());
    if (stream == 0) {
      write_file(b);
      FILE *fp = fopen(dat.c_str(), "rb");
      print_ct(CTABreadFromBinary(fp));
      oi(feof(fp) != 0);
      oi(ftell(fp));
      fclose(fp);
    } else {
      znzFile z = data_znz(mode, b);
      print_ct(znzCTABreadFromBinary(z));
      oi(znzeof(z) != 0);
      oi(znztell(z));
      znzclose(z);
    }
  } else if (cmd == "tw" || cmd == "ztw") {
    bool zn = cmd == "ztw";
    int gz = zn ? (int)geti() : 0;
    FILE *fp = NULL;
    znzFile z = NULL;
    if (zn) z = znzopen(dat.c_str(), "wb", gz); else fp = fopen(dat.c_str(), "wb");
    while (more()) {
      std::string op = next();
      out += op + ": ";
      if (op == "s") {
        int tag = (int)geti(); long long len = geti(), here = -7777;
        oi(zn ? znzTAGwriteStart(z, tag, &here, len) : TAGwriteStart(fp, tag, &here, len));
        oi(here);
      } else if (op == "w") {
        int tag = (int)geti();
        std::vector<unsigned char> d = unhex(next());
        long long len = geti();
        d.resize(d.size() + 8, 0);
        oi(TAGwrite(fp, tag, d.data(), len));
      } else if (op == "m") {
        int tag = (int)geti();
        MATRIX *m = read_mat();
        oi(zn ? znzWriteMatrix(z, m, tag) : TAGwriteMatrix(fp, m, tag));
      } else if (op == "e") {
        oi(zn ? znzTAGwriteEnd(z, 0) : TAGwriteEnd(fp, 0));
      } else fail("unknown tag writer");
    }
    if (zn) znzclose(z); else fclose(fp);
    obytes(zn ? read_znz(dat, gz) : read_file(dat));
  } else if (cmd == "tr" || cmd == "ztr") {
    bool zn = cmd == "ztr";
    int mode = zn ? (int)geti() : 0;
    Gdiag_no = (int)geti();
    std::vector<unsigned char> b = unhex(next());
    FILE *fp = NULL;
    znzFile z = NULL;
    if (zn) z = data_znz(mode, b);
    else { write_file(b); fp = fopen(dat.c_str(), "rb"); }
    while (more()) {
      std::string op = next();
      out += op + ": ";
      if (op == "t") {
        long long len = -7777;
        if (zn) { int zl = (int)geti(); oi(znzTAGreadStart(z, &len, zl)); }
        else oi(TAGreadStart(fp, &len));
        oi(len);
      } else if (op == "k") {
        int tag = (int)geti();
        long long len = geti();
        oi(TAGskip(fp, tag, len));
      } else if (op == "d") {
        long long n = geti();
        std::vector<unsigned char> d((size_t)n + 1, 0xee);
        oi(TAGread(fp, d.data(), n));
        d.resize((size_t)n);
        obytes(d);
      } else if (op == "m") {
        errno = 0;
        MATRIX *m = zn ? znzReadMatrix(z) : TAGreadMatrix(fp);
        int e = errno;
        print_mat(m);
        oi(e);
      } else fail("unknown tag reader");
    }
    if (zn) { oi(znzeof(z) != 0); oi(znztell(z)); znzclose(z); }
    else { oi(feof(fp) != 0); oi(ftell(fp)); fclose(fp); }
  } else if (cmd == "cmdl") {
    std::vector<std::string> args;
    while (more()) { bool n; args.push_back(getstr(&n)); }
    std::vector<char *> argv;
    for (std::string &a : args) argv.push_back((char *)a.c_str());
    static char line[1 << 16];
    oi(TAGmakeCommandLineString((int)argv.size(), argv.data(), line));
    out += "x";
    obytes(std::vector<unsigned char>(line, line + strlen(line)));
    if (!line[0]) { out.pop_back(); out.pop_back(); out += " "; }
  } else {
    run_piece3(cmd);
  }
}

void run_case()
{
  std::string cmd = next();
  if (cmd == "rd") {
    std::string fn = next();
    Gdiag_no = (int)geti();
    int count = (int)geti();
    int32_t init = (int32_t)getx32();
    write_file(unhex(next()));
    FILE *fp = fopen(dat.c_str(), "rb");
    if (!fp) fail("cannot open the data file");
    for (int k = 0; k < count; k++) {
      if (fn == "r1") { int v = 0x5a5a5a5a; oi(fread1(&v, fp)); ox(&v, 4); }
      else if (fn == "r2") { int v = 0x5a5a5a5a; oi(fread2(&v, fp)); ox(&v, 4); }
      else if (fn == "r3") { int v = 0x5a5a5a5a; oi(fread3(&v, fp)); ox(&v, 4); }
      else if (fn == "rd") { double d = freadDouble(fp); ox(&d, 8); }
      else if (fn == "ri") { int i = freadInt(fp); ox(&i, 4); }
      else if (fn == "rl") { long long l = freadLong(fp); ox(&l, 8); }
      else if (fn == "rix") { int i = init; oi(freadIntEx(&i, fp)); ox(&i, 4); }
      else if (fn == "rf") { float f = freadFloat(fp); ox(&f, 4); }
      else if (fn == "r4") { float v; memcpy(&v, &init, 4); oi(fread4(&v, fp)); ox(&v, 4); }
      else if (fn == "gf") { float f = getf(fp); ox(&f, 4); }
      else if (fn == "rs") { short s = freadShort(fp); ox(&s, 2); }
      else if (fn == "rfx") { float f; memcpy(&f, &init, 4); oi(freadFloatEx(&f, fp)); ox(&f, 4); }
      else if (fn == "rdx") {
        uint64_t u = ((uint64_t)(uint32_t)init << 32) | (uint32_t)init;
        double d; memcpy(&d, &u, 8); oi(freadDoubleEx(&d, fp)); ox(&d, 8);
      }
      else if (fn == "rsx") { short s = (short)init; oi(freadShortEx(&s, fp)); ox(&s, 2); }
      else fail("unknown reader");
    }
    oi(feof(fp) != 0);
    fclose(fp);
  } else if (cmd == "zr") {
    std::string fn = next();
    Gdiag_no = (int)geti();
    int mode = (int)geti();
    int count = (int)geti();
    uint32_t init = getx32();
    std::vector<unsigned char> b = unhex(next());
    if (mode == 1) {
      znzFile w = znzopen(dat.c_str(), "wb", 1);
      if (znz_isnull(w)) fail("cannot write the data file");
      if (!b.empty()) znzwrite(b.data(), 1, b.size(), w);
      znzclose(w);
    } else {
      write_file(b);
    }
    znzFile z = znzopen(dat.c_str(), "rb", mode != 0);
    if (znz_isnull(z)) fail("cannot open the data file");
    for (int k = 0; k < count; k++) {
      if (fn == "z1") { int v = 0x5a5a5a5a; oi(znzread1(&v, z)); ox(&v, 4); }
      else if (fn == "zs") { short s = znzreadShort(z); ox(&s, 2); }
      else if (fn == "zi") { int i = znzreadInt(z); ox(&i, 4); }
      else if (fn == "zl") { long long l = znzreadLong(z); ox(&l, 8); }
      else if (fn == "zf") { float f = znzreadFloat(z); ox(&f, 4); }
      else if (fn == "zd") { double d = znzreadDouble(z); ox(&d, 8); }
      else if (fn == "zix") { int i = (int)init; oi(znzreadIntEx(&i, z)); ox(&i, 4); }
      else if (fn == "zfx") { float f; memcpy(&f, &init, 4); oi(znzreadFloatEx(&f, z)); ox(&f, 4); }
      else if (fn == "z2") { int v = 0x5a5a5a5a; oi(znzread2(&v, z)); ox(&v, 4); }
      else if (fn == "z3") { int v = 0x5a5a5a5a; oi(znzread3(&v, z)); ox(&v, 4); }
      else if (fn == "z4") { float v; memcpy(&v, &init, 4); oi(znzread4(&v, z)); ox(&v, 4); }
      else if (fn == "zdx") {
        uint64_t u = ((uint64_t)init << 32) | init;
        double d; memcpy(&d, &u, 8); oi(znzreadDoubleEx(&d, z)); ox(&d, 8);
      }
      else if (fn == "zsx") { short s = (short)init; oi(znzreadShortEx(&s, z)); ox(&s, 2); }
      else fail("unknown reader");
    }
    oi(znzeof(z) != 0);
    znzclose(z);
  } else if (cmd == "wr") {
    std::string fn = next();
    const char *v = next();
    FILE *fp = fopen(dat.c_str(), "wb");
    if (!fp) fail("cannot write the data file");
    int r;
    if (fn == "w2") r = fwrite2((int)strtoll(v, NULL, 10), fp);
    else if (fn == "w3") r = fwrite3((int)strtoll(v, NULL, 10), fp);
    else if (fn == "wi") r = fwriteInt((int)strtoll(v, NULL, 10), fp);
    else if (fn == "wl") r = fwriteLong(strtoll(v, NULL, 10), fp);
    else if (fn == "wf") { uint32_t u = (uint32_t)strtoul(v, NULL, 16); float f; memcpy(&f, &u, 4); r = fwriteFloat(f, fp); }
    else if (fn == "w1") r = fwrite1((int)strtoll(v, NULL, 10), fp);
    else if (fn == "w4") r = fwrite4((int)strtoll(v, NULL, 10), fp);
    else if (fn == "wsh") r = fwriteShort((short)strtoll(v, NULL, 10), fp);
    else if (fn == "pf") { uint32_t u = (uint32_t)strtoul(v, NULL, 16); float f; memcpy(&f, &u, 4); r = putf(f, fp); }
    else if (fn == "wd") { uint64_t u = strtoull(v, NULL, 16); double d; memcpy(&d, &u, 8); r = fwriteDouble(d, fp); }
    else fail("unknown writer");
    fclose(fp);
    oi(r);
    obytes(read_file(dat));
  } else if (cmd == "zw") {
    std::string fn = next();
    int gz = (int)geti();
    const char *v = next();
    znzFile z = znzopen(dat.c_str(), "wb", gz);
    if (znz_isnull(z)) fail("cannot write the data file");
    int r;
    if (fn == "zws") r = znzwriteShort((short)strtoll(v, NULL, 10), z);
    else if (fn == "zwus") r = znzwriteUShort((unsigned short)strtoll(v, NULL, 10), z);
    else if (fn == "zwi") r = znzwriteInt((int)strtoll(v, NULL, 10), z);
    else if (fn == "zwl") r = znzwriteLong(strtoll(v, NULL, 10), z);
    else if (fn == "zwf") { uint32_t u = (uint32_t)strtoul(v, NULL, 16); float f; memcpy(&f, &u, 4); r = znzwriteFloat(f, z); }
    else if (fn == "zwd") { uint64_t u = strtoull(v, NULL, 16); double d; memcpy(&d, &u, 8); r = znzwriteDouble(d, z); }
    else if (fn == "zw1") r = znzwrite1((int)strtoll(v, NULL, 10), z);
    else if (fn == "zw2") r = znzwrite2((int)strtoll(v, NULL, 10), z);
    else if (fn == "zw3") r = znzwrite3((int)strtoll(v, NULL, 10), z);
    else if (fn == "zw4") r = znzwrite4((int)strtoll(v, NULL, 10), z);
    else fail("unknown writer");
    znzclose(z);
    oi(r);
    obytes(read_znz(dat, gz));
  } else if (cmd == "dirname") {
    bool n;
    std::string s = getstr(&n);
    char *r = fio_dirname(n ? NULL : s.c_str());
    ostr(r);
    free(r);
  } else if (cmd == "basename") {
    bool n, ne;
    std::string s = getstr(&n);
    std::string e = getstr(&ne);
    char *r = fio_basename(n ? NULL : s.c_str(), ne ? NULL : e.c_str());
    ostr(r);
    free(r);
  } else if (cmd == "ext") {
    bool n;
    std::string s = getstr(&n);
    char *r = fio_extension(n ? NULL : s.c_str());
    ostr(r);
    free(r);
  } else if (cmd == "fsx") {
    std::string fn = next();
    bool n;
    std::string s = getstr(&n);
    int r, e;
    errno = 0;
    if (fn == "exists") { r = fio_FileExistsReadable(s.c_str()); e = errno; }
    else if (fn == "isdir") { r = fio_IsDirectory(s.c_str()); e = errno; }
    else if (fn == "dirw0" || fn == "dirw1") {
      int f = fn == "dirw1";
      r = fio_DirIsWritable(s.c_str(), f);
      e = errno;
      std::string junk = f ? s + ".junk.54_-_sdfklj" : s + "/.junk.54_-_sdfklj";
      struct stat st;
      int left = stat(junk.c_str(), &st) == 0;
      oi(r);
      oi(left);
      r = -1;
    } else fail("unknown test");
    if (r != -1) oi(r);
    oi(e);
  } else if (cmd == "tsv") {
    int debug = (int)geti();
    bool nofmt;
    std::string fmt = getstr(&nofmt);
    const char *content = next();
    unlink("t.tsv");
    if (strcmp(content, "N") != 0) {
      std::vector<unsigned char> b = unhex(content);
      FILE *fp = fopen("t.tsv", "wb");
      if (!fp) fail("cannot write t.tsv");
      if (!b.empty()) fwrite(b.data(), 1, b.size(), fp);
      fclose(fp);
    }
    TSV t;
    t.debug = debug;
    errno = 0;
    oi(t.read("t.tsv"));
    int e = errno;
    oi(t.ncols());
    for (const std::string &h : t.headers) {
      out += "x";
      obytes(std::vector<unsigned char>(h.begin(), h.end()));
      if (h.empty()) { out.pop_back(); out.pop_back(); out += " "; }
    }
    oi(t.nrows());
    for (const std::vector<double> &row : t.data) {
      oi((long long)row.size());
      for (double v : row) ox(&v, 8);
    }
    oi(e);
    if (!nofmt) {
      oi(t.write("o.tsv", fmt));
      obytes(read_file("o.tsv"));
      unlink("o.tsv");
    }
    unlink("t.tsv");
  } else if (cmd == "swap") {
    std::string fn = next();
    long count = (long)geti();
    int buflen = (int)geti();
    std::vector<unsigned char> b = unhex(next());
    // room after the buffer, so that FreeSurfer's writes past it stay in the probe's memory
    std::vector<unsigned char> room(b);
    room.resize(b.size() + 64, 0xee);
    if (fn == "s2") byteswapbufshort(room.data(), count);
    else if (fn == "s4") byteswapbuffloat(room.data(), count);
    else if (fn == "s8") byteswapbufdouble(room.data(), count);
    else fail("unknown swap");
    room.resize(buflen);
    obytes(room);
  } else if (cmd == "calloc") {
    unsigned long long nmemb = strtoull(next(), NULL, 10);
    unsigned long long size = strtoull(next(), NULL, 10);
    char *p = lcalloc((size_t)nmemb, (size_t)size);
    unsigned long long n = nmemb * size;
    int nonzero = 0;
    for (unsigned long long i = 0; i < n && i < 64; i++) nonzero |= p[i] != 0;
    out += "ok ";
    oi(nonzero);
    free(p);
  } else {
    run_piece2(cmd);
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
std::string realfix;

int rm_entry(const char *p, const struct stat *, int, struct FTW *)
{
  chmod(p, 0700);
  return remove(p);
}

void make_fixture(const std::string &fix)
{
  nftw(fix.c_str(), rm_entry, 16, FTW_DEPTH | FTW_PHYS);
  if (mkdir(fix.c_str(), 0700)) fail("cannot make the fixture");
  auto file = [&](const std::string &n, const char *text) {
    FILE *f = fopen((fix + "/" + n).c_str(), "w");
    if (!f) fail("cannot write a fixture file");
    fputs(text, f);
    fclose(f);
  };
  file("f", "abc\n");
  mkdir((fix + "/d").c_str(), 0700);
  file("d/i", "x");
  file("nr", "secret");
  chmod((fix + "/nr").c_str(), 0);
  mkdir((fix + "/ro").c_str(), 0500);
  if (symlink("f", (fix + "/ln").c_str()) || symlink("d", (fix + "/dln").c_str()) ||
      symlink("nowhere", (fix + "/bad").c_str()))
    fail("cannot make the fixture's links");
}

}  // namespace

int main(int argc, char *argv[])
{
  if (argc != 2) {
    fprintf(stderr, "usage: fsio <cases>\n");
    return 2;
  }
  if (argv[1][0] != '/') fail("the cases must be given by an absolute path");
  std::string scratch = std::string(argv[1]) + ".out";
  dat = std::string(argv[1]) + ".dat";
  std::string fix = std::string(argv[1]) + ".fix";
  make_fixture(fix);
  char rp[4096];
  realfix = realpath(fix.c_str(), rp) ? std::string(rp) : fix;
  FILE *fp = fopen(argv[1], "r");
  if (!fp) fail("cannot open the cases");
  std::vector<std::string> lines;
  static char buf[1 << 20];
  while (fgets(buf, sizeof buf, fp)) lines.push_back(buf);
  fclose(fp);
  for (const std::string &line : lines) {
    if (line.size() >= sizeof buf) fail("a case longer than the buffer");
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
      if (chdir(fix.c_str())) _exit(4);
      // a case that reads for ever (TSV::read, FS-172) stops at 1 GiB of memory or 5 seconds
      struct rlimit rl = {1ul << 30, 1ul << 30};
      setrlimit(RLIMIT_AS, &rl);
      // and at most 64 MiB of messages (a reader that prints for each of millions of entries)
      struct rlimit fl = {64ul << 20, 64ul << 20};
      setrlimit(RLIMIT_FSIZE, &fl);
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
    std::string text;
    if (FILE *r = fopen(scratch.c_str(), "rb")) {
      int ch;
      while ((ch = fgetc(r)) != EOF) text += (char)ch;
      fclose(r);
    }
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
    // piece 6: the fixture's path removed from the messages (the port's fixture is elsewhere)
    {
      static const char *p6[] = {"mrh", "mri", "mrd", "mgr", "rrr"};
      std::string c0 = line.substr(0, line.find_first_of(" \t\n"));
      for (const char *k : p6)
        if (c0 == k)
          for (const std::string &pf : {realfix + "/", fix + "/"})
            for (size_t at; (at = text.find(pf)) != std::string::npos;) text.erase(at, pf.size());
    }
    // messages beyond 1 MiB are shown by their length (the port does the same)
    if (text.size() > (1u << 20)) text = "BIGTEXT " + std::to_string(text.size());
    printf("%s | %s\n", result.c_str(), escape(text).c_str());
  }
  unlink(scratch.c_str());
  unlink(dat.c_str());
  nftw(fix.c_str(), rm_entry, 16, FTW_DEPTH | FTW_PHYS);
  return 0;
}
