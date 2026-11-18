// SPDX-License-Identifier: Apache-2.0
// fs.gca's probe (track A relay 45): FreeSurfer 8.2.0's MRIStransform (a surface moved through a
// TRANSFORM: a GCA morph or a linear transform array) called case by case on surfaces, morphs,
// linear transform arrays and volume headers the case builds, linked with the unit oracle's
// libutils as the official programs are (build.sh). CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS:
// it names FreeSurfer's functions (running it is a black-box run of FreeSurfer's code).
//
//     fsgcam <cases>     one output line per case line (absolute path; see check.py)
//
// Every case runs in a child process (1 GiB of address space, 5 seconds; `msf` 4 GiB, 120 s) whose standard output
// and error go to a scratch file beside the cases; the parent prints `<result> | <text>`, the text
// C-escaped (\xHH for bytes outside printable ASCII and for the backslash), or `EXIT <status> |
// <text>` / `CRASH <signal> | <text>` when FreeSurfer's code ended the child. Floats and doubles
// in and out are their bits in hex.
//
// `msf ...`: real surfaces and transforms (run_files below).
// `mst <gdiag> <gdiag_no> <volumes> <surface> <transform> [<volume>] [<volume>]`:
//   volumes   nn (NULL, NULL), gn (mri given), ng (mri_dst given), gg (both), ss (one volume as
//             both: mris_sphere's call); a given volume is `MRIallocHeader` of its geometry's
//             extent (bytes, one frame), the geometry's other fields set, `MRIreInitCache`
//   surface   nv useRealRAS <geom>, then nv x (x y z ripflag)
//   transform L nxforms type m_L(16 floats) <src geom> <dst geom>   (a linear transform array;
//             a second transform, when there is one, is LTAalloc's identity)
//             M width height depth spacing type <image geom> <atlas geom>, then for each node
//             (x outermost, z innermost) x y z (doubles) invalid  (GCAMalloc, then the fields)
//   geom      valid width height depth xsize ysize zsize ras_good_flag x_r x_a x_s y_r y_a y_s
//             z_r z_a z_s c_r c_a c_s fname (`-` for none)
// The result: MRIStransform's return, every vertex's x y z, the surface's extent and centre
// (xlo xhi ylo yhi zlo zhi xctr yctr zctr), its geometry; for a morph also its type, an FNV-1a 64
// hash of its nodes' positions, original positions, validity and original areas, its image
// geometry.

#include <fcntl.h>
#include <signal.h>
#include <sys/resource.h>
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
#include "gcamorph.h"
#include "matrix.h"
#include "mri.h"
#include "mrisurf.h"
#include "transform.h"

const char *Progname = "fsgcam";

namespace {

std::vector<std::string> toks;
size_t pos;
std::string out;

[[noreturn]] void fail(const char *what)
{
  fprintf(stderr, "fsgcam: %s\n", what);
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
void os(const char *s) { out += (s && *s) ? std::string(s) : std::string("-"); out += " "; }

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
  std::string f = gets();
  strcpy(v->fname, f.c_str());
}

void put_geom(const VOL_GEOM *v)
{
  oi(v->valid);
  oi(v->width); oi(v->height); oi(v->depth);
  of(v->xsize); of(v->ysize); of(v->zsize);
  oi(v->ras_good_flag);
  of(v->x_r); of(v->x_a); of(v->x_s);
  of(v->y_r); of(v->y_a); of(v->y_s);
  of(v->z_r); of(v->z_a); of(v->z_s);
  of(v->c_r); of(v->c_a); of(v->c_s);
  os(v->fname);
}

// a volume header of the case's geometry (its `valid` is not an MRI's)
MRI *get_volume()
{
  VOL_GEOM vg;
  set_geom(&vg);
  MRI *m = MRIallocHeader(vg.width, vg.height, vg.depth, MRI_UCHAR, 1);
  m->xsize = vg.xsize; m->ysize = vg.ysize; m->zsize = vg.zsize;
  m->ras_good_flag = vg.ras_good_flag;
  m->x_r = vg.x_r; m->x_a = vg.x_a; m->x_s = vg.x_s;
  m->y_r = vg.y_r; m->y_a = vg.y_a; m->y_s = vg.y_s;
  m->z_r = vg.z_r; m->z_a = vg.z_a; m->z_s = vg.z_s;
  m->c_r = vg.c_r; m->c_a = vg.c_a; m->c_s = vg.c_s;
  strcpy(m->fname, vg.fname);
  MRIreInitCache(m);
  return m;
}

uint64_t fnv(uint64_t h, const void *p, size_t n)
{
  const unsigned char *b = (const unsigned char *)p;
  for (size_t i = 0; i < n; i++) {
    h ^= b[i];
    h *= 0x100000001b3ULL;
  }
  return h;
}

// `msf <surface> <transform> nn|ss [<volume>]`: real files. nn is mri_normalize's call
// (TransformRead, TransformInvert(transform, NULL), MRIStransform(mris, NULL, transform, NULL));
// ss is mris_sphere's (LTAread, the transform's type the array's, MRIread of the volume,
// MRIStransform(mris, mri, &transform, mri)). The result: the steps' returns, the vertex count,
// an FNV-1a 64 hash of every vertex's x y z, the extent and centre, the surface's geometry.
void run_files()
{
  // MRIread's licence check (chklc) needs FREESURFER_HOME (any value with FS_LICENSE set)
  setenv("FREESURFER_HOME", "/nonexistent-freesurfer-home", 1);
  if (!getenv("FS_LICENSE")) setenv("FS_LICENSE", "/usr/local/freesurfer/license.txt", 1);
  std::string sf = next(), xf = next(), vols = next();
  MRIS *mris = MRISread(sf.c_str());
  if (!mris) { oi(-1); return; }
  TRANSFORM tr, *tp = NULL;
  MRI *mri = NULL;
  if (vols == "nn") {
    tp = TransformRead(xf.c_str());
    if (!tp) { oi(-2); return; }
    oi(TransformInvert(tp, NULL));
  } else {
    LTA *lta = LTAread(xf.c_str());
    if (!lta) { oi(-2); return; }
    std::string vf = next();
    mri = MRIread(vf.c_str());
    if (!mri) { oi(-3); return; }
    tr.type = lta->type;
    tr.xform = (void *)lta;
    tp = &tr;
  }
  oi(MRIStransform(mris, mri, tp, mri));
  oi(mris->nvertices);
  uint64_t h = 0xcbf29ce484222325ULL;
  for (int vno = 0; vno < mris->nvertices; vno++) {
    float p[3] = {mris->vertices[vno].x, mris->vertices[vno].y, mris->vertices[vno].z};
    h = fnv(h, p, sizeof p);
  }
  char b[24];
  snprintf(b, sizeof b, "%016" PRIx64 " ", h);
  out += b;
  of(mris->xlo); of(mris->xhi); of(mris->ylo); of(mris->yhi); of(mris->zlo); of(mris->zhi);
  of(mris->xctr); of(mris->yctr); of(mris->zctr);
  put_geom(&mris->vg);
}

void run_case()
{
  std::string cmd = next();
  if (cmd == "msf") {
    run_files();
    return;
  }
  if (cmd != "mst") fail("unknown command");
  Gdiag = (int)strtoul(next(), NULL, 16);
  Gdiag_no = geti();
  std::string vols = next();
  int nv = geti();
  int real = geti();
  MRIS *mris = MRISalloc(nv, 0);
  mris->useRealRAS = real;
  set_geom(&mris->vg);
  for (int vno = 0; vno < nv; vno++) {
    VERTEX *v = &mris->vertices[vno];
    v->x = getf(); v->y = getf(); v->z = getf();
    v->ripflag = (char)geti();
  }
  TRANSFORM transform;
  memset(&transform, 0, sizeof transform);
  GCA_MORPH *gcam = NULL;
  std::string kind = next();
  if (kind == "L") {
    int nx = geti();
    LTA *lta = LTAalloc(nx, NULL);
    lta->type = geti();
    if (nx > 0) {
      LT *lt = &lta->xforms[0];
      lt->type = lta->type;
      for (int r = 1; r <= 4; r++)
        for (int c = 1; c <= 4; c++) *MATRIX_RELT(lt->m_L, r, c) = getf();
      set_geom(&lt->src);
      set_geom(&lt->dst);
    } else {
      // the case's numbers for the missing transform are read and dropped
      for (int i = 0; i < 16; i++) getf();
      VOL_GEOM tmp;
      set_geom(&tmp);
      set_geom(&tmp);
    }
    transform.type = lta->type;
    transform.xform = (void *)lta;
  } else if (kind == "M") {
    int w = geti(), h = geti(), d = geti();
    gcam = GCAMalloc(w, h, d);
    gcam->spacing = geti();
    gcam->type = geti();
    set_geom(&gcam->image);
    set_geom(&gcam->atlas);
    for (int x = 0; x < w; x++)
      for (int y = 0; y < h; y++)
        for (int z = 0; z < d; z++) {
          GCA_MORPH_NODE *n = &gcam->nodes[x][y][z];
          n->x = getd(); n->y = getd(); n->z = getd();
          n->invalid = (char)geti();
        }
    transform.type = MORPH_3D_TYPE;
    transform.xform = (void *)gcam;
  } else {
    fail("unknown transform");
  }
  MRI *mri = NULL, *mri_dst = NULL;
  if (vols == "gn" || vols == "gg" || vols == "ss") mri = get_volume();
  if (vols == "ng" || vols == "gg") mri_dst = get_volume();
  if (vols == "ss") mri_dst = mri;
  oi(MRIStransform(mris, mri, &transform, mri_dst));
  for (int vno = 0; vno < nv; vno++) {
    VERTEX *v = &mris->vertices[vno];
    of(v->x); of(v->y); of(v->z);
  }
  of(mris->xlo); of(mris->xhi); of(mris->ylo); of(mris->yhi); of(mris->zlo); of(mris->zhi);
  of(mris->xctr); of(mris->yctr); of(mris->zctr);
  put_geom(&mris->vg);
  if (gcam) {
    oi(gcam->type);
    uint64_t hsh = 0xcbf29ce484222325ULL;
    for (int x = 0; x < gcam->width; x++)
      for (int y = 0; y < gcam->height; y++)
        for (int z = 0; z < gcam->depth; z++) {
          GCA_MORPH_NODE *n = &gcam->nodes[x][y][z];
          double p[6] = {n->x, n->y, n->z, n->origx, n->origy, n->origz};
          hsh = fnv(hsh, p, sizeof p);
          hsh = fnv(hsh, &n->invalid, 1);
          float a[3] = {n->orig_area, n->orig_area1, n->orig_area2};
          hsh = fnv(hsh, a, sizeof a);
        }
    char b[24];
    snprintf(b, sizeof b, "%016" PRIx64 " ", hsh);
    out += b;
    put_geom(&gcam->image);
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
    fprintf(stderr, "usage: fsgcam <cases>\n");
    return 2;
  }
  if (argv[1][0] != '/') fail("the cases must be given by an absolute path");
  std::string scratch = std::string(argv[1]) + ".out";
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
      // real files (msf) get 4 GiB and 120 seconds: a recorded head's morph holds about 2
      // million nodes
      bool files = strncmp(line.c_str(), "msf ", 4) == 0;
      rlim_t as = files ? (4UL << 30) : (1UL << 30);
      struct rlimit lim = {as, as};
      setrlimit(RLIMIT_AS, &lim);
      alarm(files ? 120 : 5);
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
