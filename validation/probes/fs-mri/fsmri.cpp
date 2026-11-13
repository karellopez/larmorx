// SPDX-License-Identifier: Apache-2.0
// fs.mri's probe: FreeSurfer 8.2.0's MRI structure (mri.h, mri.cpp, transform.cpp's VOL_GEOM
// helpers, colortab.cpp's CTABdeepCopy) called case by case, linked with the unit oracle's
// libutils as the official programs are (build.sh). CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS:
// it names FreeSurfer's functions (running it is a black-box run of FreeSurfer's code).
//
//     fsmri <cases>     one output line per case line (absolute path; see check.py)
//
// Every case runs in a child process whose standard output and error go to a scratch file beside
// the cases; the parent prints `<result> | <text>`, the text C-escaped (\xHH for bytes outside
// printable ASCII and for the backslash), or `EXIT <status> | <text>` / `CRASH <signal> | <text>`
// when FreeSurfer's code ended the child. Floats and doubles in and out are their bits in hex.

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

#include "colortab.h"
#include "diag.h"
#include "error.h"
#include "matrix.h"
#include "mri.h"
#include "transform.h"
#include "utils.h"

int MRIcopyVolGeomToMRI(MRI *mri, const VOL_GEOM *vg);
int MRIcopyVolGeomFromMRI(const MRI *mri, VOL_GEOM *vg);

const char *Progname = "fsmri";

namespace {

std::vector<std::string> toks;
size_t pos;
std::string out;

[[noreturn]] void fail(const char *what)
{
  fprintf(stderr, "fsmri: %s\n", what);
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

void om(const MATRIX *m)
{
  if (!m) { out += "NULL "; return; }
  oi(m->rows);
  oi(m->cols);
  for (int r = 1; r <= m->rows; r++)
    for (int c = 1; c <= m->cols; c++) of(m->rptr[r][c]);
}
void oam(const AffineMatrix *am)
{
  if (!am) { out += "NULL "; return; }
  for (int i = 0; i < 16; i++) of(am->mat[i]);
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
  os(v->fname);
  oam(v->i_to_r__);
  om(v->r_to_i__);
  om(v->register_mat);
}

// a matrix of the cases: rows cols, then the numbers
MATRIX *get_matrix()
{
  int rows = geti(), cols = geti();
  MATRIX *m = MatrixAlloc(rows, cols, MATRIX_REAL);
  for (int r = 1; r <= rows; r++)
    for (int c = 1; c <= cols; c++) m->rptr[r][c] = getf();
  return m;
}
MATRIX *get_opt_matrix() { return geti() ? get_matrix() : NULL; }

void put_header(const MRI *m)
{
  oi(m->width); oi(m->height); oi(m->depth); oi(m->nframes); oi(m->type);
  oi((long long)m->bytes_per_vox); oi((long long)m->bytes_total);
  oi((long long)m->vox_per_row); oi((long long)m->vox_per_slice); oi((long long)m->vox_per_vol);
  oi((long long)m->vox_total);
  oi(m->imnr0); oi(m->imnr1);
  of(m->xstart); of(m->ystart); of(m->zstart); of(m->xend); of(m->yend); of(m->zend);
  of(m->fov); of(m->thick); of(m->ps); oi(m->scale);
  // roi.x, roi.y, roi.z: never set by the constructors (memory contents): not printed
  oi(m->roi.dx); oi(m->roi.dy); oi(m->roi.dz);
  oi(m->version); oi(m->intent); oi(m->dof); oi(m->brightness); oi(m->yinvert); oi(m->ptype);
  od(m->outside_val); od(m->flip_angle); of(m->tr); of(m->te); of(m->ti); of(m->location);
  oi(m->ncmds);
  for (int i = 0; i < m->ncmds; i++) os(m->cmdlines[i]);
  os(m->subject_name); os(m->path_to_t1); os(m->fname_format); os(m->gdf_image_stem);
  os(m->transform_fname);
  om(m->AutoAlign); om(m->origRas2Vox);
  oi(m->free_transform);
  put_geom(m, false);
}

void put_indices(const MRI *m)
{
  if (!m->xi) { out += "noindex "; return; }
  int lim[3] = {m->width, m->height, m->depth};
  int *tab[3] = {m->xi, m->yi, m->zi};
  for (int k = 0; k < 3; k++)
    for (int i : {-500, -2, -1, 0, 1, lim[k] - 1, lim[k], lim[k] + 1, lim[k] + 499}) oi(tab[k][i]);
}

void put_frames(const MRI *m)
{
  if (!m->frames) { out += "noframes "; return; }
  for (int f = 0; f < m->nframes; f++) {
    const MRI_FRAME &fr = m->frames[f];
    oi(fr.type); of(fr.TE); of(fr.TR); oi(fr.label); oi(fr.dof); of(fr.thresh);
    om(fr.m_ras2vox);
  }
}

void put_chunk(const MRI *m)
{
  // a volume without voxels (MRIallocHeader) leaves ischunked unset: chunk is NULL first
  if (!m->chunk || !m->ischunked) { out += "nochunk "; return; }
  const unsigned char *b = (const unsigned char *)m->chunk;
  char h[4];
  out += "chunk:";
  for (size_t i = 0; i < m->bytes_total; i++) {
    snprintf(h, sizeof h, "%02x", b[i]);
    out += h;
  }
  out += " ";
}

COLOR_TABLE *get_ctab(int depth)
{
  int n = geti();
  COLOR_TABLE *ct = (COLOR_TABLE *)calloc(1, sizeof(COLOR_TABLE));
  ct->nentries = n;
  ct->entries = (COLOR_TABLE_ENTRY **)calloc(n > 0 ? n : 1, sizeof(COLOR_TABLE_ENTRY *));
  strcpy(ct->fname, gets().c_str());
  ct->version = geti();
  ct->idbase = geti();
  strcpy(ct->TissueTypeSchema, gets().c_str());
  for (int i = 0; i < n; i++) {
    if (!geti()) continue;
    CTE *e = (CTE *)calloc(1, sizeof(CTE));
    strcpy(e->name, gets().c_str());
    e->ri = geti(); e->gi = geti(); e->bi = geti(); e->ai = geti();
    e->rf = getf(); e->gf = getf(); e->bf = getf(); e->af = getf();
    e->TissueType = geti();
    e->count = geti();
    ct->entries[i] = e;
  }
  ct->ctabTissueType = (depth == 0 && geti()) ? get_ctab(1) : NULL;
  return ct;
}

void put_ctab(const COLOR_TABLE *ct)
{
  if (!ct) { out += "NULL "; return; }
  oi(ct->nentries); os(ct->fname); oi(ct->version); oi(ct->idbase); os(ct->TissueTypeSchema);
  for (int i = 0; i < ct->nentries; i++) {
    const CTE *e = ct->entries[i];
    if (!e) { out += "0 "; continue; }
    out += "1 ";
    os(e->name); oi(e->ri); oi(e->gi); oi(e->bi); oi(e->ai);
    of(e->rf); of(e->gf); of(e->bf); of(e->af); oi(e->TissueType);
    // e->count: CTABdeepCopy leaves it unset (memory contents): not printed
  }
  put_ctab(ct->ctabTissueType);
}

// an MRI the cases describe: w h d type nframes hdr, then optional header fields
MRI *get_mri()
{
  int w = geti(), h = geti(), d = geti(), type = geti(), nf = geti(), hdr = geti();
  return hdr ? MRIallocHeader(w, h, d, type, nf) : MRIallocSequence(w, h, d, type, nf);
}

// the header fields MRIcopyHeader reads, then the optional pointers
void set_header(MRI *m)
{
  m->version = geti(); m->intent = geti(); m->dof = geti(); m->mean = getd();
  m->xsize = getf(); m->ysize = getf(); m->zsize = getf();
  m->imnr0 = geti(); m->imnr1 = geti(); m->ptype = geti();
  m->fov = getf(); m->thick = getf(); m->ps = getf(); m->location = getf();
  m->xstart = getf(); m->xend = getf(); m->ystart = getf(); m->yend = getf();
  m->zstart = getf(); m->zend = getf();
  m->flip_angle = getd(); m->tr = getf(); m->te = getf(); m->ti = getf();
  strcpy(m->fname, gets().c_str());
  m->x_r = getf(); m->x_a = getf(); m->x_s = getf();
  m->y_r = getf(); m->y_a = getf(); m->y_s = getf();
  m->z_r = getf(); m->z_a = getf(); m->z_s = getf();
  m->c_r = getf(); m->c_a = getf(); m->c_s = getf();
  m->ras_good_flag = geti(); m->brightness = geti();
  strcpy(m->subject_name, gets().c_str());
  strcpy(m->path_to_t1, gets().c_str());
  strcpy(m->fname_format, gets().c_str());
  strcpy(m->gdf_image_stem, gets().c_str());
  strcpy(m->transform_fname, gets().c_str());
  int n = geti();
  for (int i = 0; i < m->ncmds; i++) free(m->cmdlines[i]);
  for (int i = 0; i < n; i++) {
    std::string s = gets();
    m->cmdlines[i] = (char *)calloc(s.size() + 1, 1);
    strcpy(m->cmdlines[i], s.c_str());
  }
  m->ncmds = n;
  if (MATRIX *r = get_opt_matrix()) { MatrixFree(&m->register_mat); m->register_mat = r; }
  if (MATRIX *a = get_opt_matrix()) { MatrixFree(&m->AutoAlign); m->AutoAlign = a; }
  if (MATRIX *o = get_opt_matrix()) { MatrixFree(&m->origRas2Vox); m->origRas2Vox = o; }
  if (geti()) m->ct = get_ctab(0);
}

void run_case()
{
  std::string cmd = next();
  if (cmd == "alloc") {
    MRI *m = get_mri();
    put_header(m); put_indices(m); put_frames(m); put_chunk(m);
    MRIfree(&m);
    oi(m == NULL);
  } else if (cmd == "allocvg") {
    VOL_GEOM vg;
    set_geom(&vg);
    int type = geti(), nf = geti(), hdr = geti();
    MRI *m = new MRI(vg, type, nf, hdr);
    put_header(m); put_indices(m); put_frames(m); put_chunk(m);
    delete m;
  } else if (cmd == "fromvg") {
    VOL_GEOM vg;
    set_geom(&vg);
    int type = geti(), nf = geti(), hdr = geti();
    MRI *m = MRIallocFromVolGeom(&vg, type, nf, hdr);
    put_header(m); put_chunk(m);
    MRIfree(&m);
  } else if (cmd == "xfm") {
    MRI *m = get_mri();
    set_geom(m);
    int base = geti(), shear = geti();
    MATRIX *x = MRIxfmCRS2XYZ(m, base, shear != 0);
    om(x);
    om(MRImatrixOfDirectionCosines(m, NULL));
    om(MRImatrixOfVoxelSizes(m, NULL));
    om(MRImatrixOfTranslations(m, NULL));
    om(extract_i_to_r(m));
    om(extract_r_to_i(m));
  } else if (cmd == "setv2r") {
    MRI *m = get_mri();
    set_geom(m);
    MATRIX *v = get_matrix();
    oi(MRIsetVox2RASFromMatrix(m, v));
    put_geom(m);
  } else if (cmd == "p0") {
    MRI *m = get_mri();
    set_geom(m);
    double r0 = getd(), a0 = getd(), s0 = getd();
    oi(MRIp0ToCRAS(m, r0, a0, s0));
    put_geom(m);
  } else if (cmd == "reinit") {
    MRI *m = get_mri();
    set_geom(m);
    oi(MRIreInitCache(m));
    put_geom(m);
  } else if (cmd == "vox") {
    MRI *m = get_mri();
    unsigned seed = (unsigned)geti();
    if (m->chunk && m->ischunked) {
      unsigned char *b = (unsigned char *)m->chunk;
      for (size_t i = 0; i < m->bytes_total; i++) b[i] = (unsigned char)((i * 37u + seed) & 0xffu);
    }
    m->outside_val = getd();
    int nops = geti();
    for (int k = 0; k < nops; k++) {
      std::string op = next();
      int c = geti(), r = geti(), s = geti(), f = geti(), mode = geti();
      if (op == "set") {
        float v = getf();
        oi(MRIsetVoxVal(m, c, r, s, f, v, mode));
      } else if (op == "get") {
        of(MRIgetVoxVal(m, c, r, s, f, mode));
      } else if (op == "dx") {
        of(MRIgetVoxDx(m, c, r, s, f));
      } else if (op == "dy") {
        of(MRIgetVoxDy(m, c, r, s, f));
      } else {
        of(MRIgetVoxDz(m, c, r, s, f));
      }
    }
    put_chunk(m);
  } else if (cmd == "copyhdr") {
    MRI *src = get_mri();
    set_header(src);
    MRI *dst = NULL;
    if (geti()) {
      dst = get_mri();
      set_header(dst);
    }
    MRI *res = MRIcopyHeader(src, dst);
    oi(res == dst);
    put_header(res);
    put_ctab(res->ct);
  } else if (cmd == "pulse") {
    MRI *a = MRIallocHeader(1, 1, 1, MRI_UCHAR, 1), *b = MRIallocHeader(1, 1, 1, MRI_UCHAR, 1);
    a->flip_angle = getd(); a->tr = getf(); a->te = getf(); a->ti = getf();
    oi(MRIcopyPulseParameters(a, b));
    od(b->flip_angle); of(b->tr); of(b->te); of(b->ti);
  } else if (cmd == "ctab") {
    COLOR_TABLE *ct = get_ctab(0);
    COLOR_TABLE *cp = CTABdeepCopy(ct);
    put_ctab(cp);
    oi(CTABfree(&cp));
    oi(cp == NULL);
  } else if (cmd == "vgcopy") {
    VOL_GEOM a;
    set_geom(&a);
    if (geti()) { MRI *t = MRIallocHeader(2, 2, 2, MRI_UCHAR, 1); a.i_to_r__ = AffineMatrixCopy(t->i_to_r__, NULL); a.r_to_i__ = MatrixCopy(t->r_to_i__, NULL); }
    VOL_GEOM b(a);
    put_geom(&b);
  } else if (cmd == "vgassign") {
    VOL_GEOM a;
    set_geom(&a);
    MRI *dst = get_mri();
    set_geom(dst);
    if (geti()) { a.i_to_r__ = AffineMatrixCopy(dst->i_to_r__, NULL); a.r_to_i__ = MatrixCopy(dst->r_to_i__, NULL); MRIreInitCache(&a); }
    *(VOL_GEOM *)dst = a;
    put_geom(dst);
  } else if (cmd == "usevg") {
    VOL_GEOM vg;
    set_geom(&vg);
    MRI *m = get_mri();
    useVolGeomToMRI(&vg, m);
    put_geom(m, false);
  } else if (cmd == "vgtomri") {
    VOL_GEOM vg;
    set_geom(&vg);
    MRI *m = get_mri();
    oi(MRIcopyVolGeomToMRI(m, &vg));
    put_geom(m, false);
  } else if (cmd == "vgfrommri") {
    MRI *m = get_mri();
    set_geom(m);
    VOL_GEOM vg;
    set_geom(&vg);
    oi(MRIcopyVolGeomFromMRI(m, &vg));
    put_geom(&vg);
  } else if (cmd == "intent") {
    int code = geti();
    os(MRI::intentName(code));
    oi(MRI::intentCode(next()));
  } else if (cmd == "sizeof") {
    oi((long long)MRIsizeof(geti()));
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
    fprintf(stderr, "usage: fsmri <cases>\n");
    return 2;
  }
  if (argv[1][0] != '/') fail("the cases must be given by an absolute path");
  std::string scratch = std::string(argv[1]) + ".out";
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
    printf("%s | %s\n", result.c_str(), escape(text).c_str());
  }
  unlink(scratch.c_str());
  return 0;
}
