// SPDX-License-Identifier: Apache-2.0
// fs.surfio's probe: FreeSurfer 8.2.0's surface structure helpers (mrisurf_aaa.h, mrisurf.h,
// mrisurf_base.h, mrisurf_topology.h, mrisurf_sphere_interp.h, mrisurf_base.cpp's status texts,
// modVnum, MRISvertexCoord2XYZ_float/_double) and the MRIS and MRIS_MP views' accessors; since
// relay 6 also mrisurf.cpp's MRISaddCommandLine, mrisVertexHash (with mrisVertexVSize) and
// MRISdiffSimple, and mrisurf_base.cpp's temporaries, per-vertex floats and file-name setters,
// and (`surf`) mrisurf_metricProperties.cpp's coordinate operations with the distances' storage
// (mrisurf_base.cpp) and MRISreverseFaceOrder; since relay 7 (`metric`) the averaging, the
// sphere's centre, the neighbour distances, the areas and the curvature operations of
// mrisurf_metricProperties.cpp; since relay 8 (`posn`) the saved vertex positions and normals, the
// canonical coordinates and the face-normal cache; called case by case, linked with the unit oracle's libutils as
// the official programs are (build.sh; `fssurfio-mpstub`, built with PROBE_MP_STUB, replaces
// MRIScomputeMetricProperties by a stub that writes `MP`, for MRISsmoothSurface).
// CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: it names FreeSurfer's functions (running it is a
// black-box run of FreeSurfer's code).
//
//     fssurfio <cases>     one output line per case line (absolute path; see check.py)
//
// Every case runs in a child process whose standard output and error go to a scratch file beside
// the cases; the parent prints `<result> | <text>`, the text C-escaped (\xHH for bytes outside
// printable ASCII and for the backslash), or `EXIT <status> | <text>` / `CRASH <signal> | <text>`
// when FreeSurfer's code ended the child. Floats and doubles in and out are their bits in hex.

#include <dirent.h>
#include <fcntl.h>
#include <signal.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <unistd.h>

#include <cerrno>
#include <cinttypes>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <algorithm>
#include <map>
#include <string>
#include <vector>

#include "diag.h"
#include "error.h"
#include "mrisurf_metricProperties.h"
#include "mrisurf_MRIS.h"
#include "mrisurf_MRIS_MP.h"
#include "mrisurf_SurfaceFromMRIS_generated.h"
#include "mrisurf_SurfaceFromMRIS_MP_generated.h"
#include "mrisurf_base.h"
#include "mrisurf_sphere_interp.h"
#include "matrix.h"
#include "mrisutils.h"
#include "transform.h"
#include "topo_parms.h"

// declared in utils/ only (mrisurf_deform.h) or nowhere (the MRIS_MP overload), defined in libutils
int mrisFlipPatch(MRIS *mris);
void MRISrotate(MRIS_MP *mris, float alpha, float beta, float gamma);
int mrisCountCompressed(MRIS *mris, double min_dist);
void mrisDumpVertex(FILE *file, MRIS const *mris, int vno);
int signum_eval(float af);
int mrisOrientSurface(MRIS *mris);
int mrisOrientPlane(MRIS *mris);
// mrisurf_io.cpp's functions of queue item 3.1-48 that no header declares, and its file-local
// ones through io_statics.cpp (which builds mrisurf_io.cpp into the probe)
MATRIX *getSRASToTalSRAS(LT *lt);
int fssurfio_read_curvature_into_array(const char *fname, int size, float **out);
int fssurfio_read_new_curvature_into_array(const char *fname, int size, float **out);
int fssurfio_read_curv_old_format(MRIS *mris, const char *fname, MRI *curvmri);
void fssurfio_apply_fsgii_read(char *file_to_read, const char *fname, int *filetype);
void fssurfio_apply_fsgii_write(char *file_to_write, const char *fname, int *filetype);
MRIS *fssurfio_read_ascii_file(const char *fname);
MRIS *fssurfio_read_geo_file(const char *fname);
int fssurfio_read_geo_file_positions(MRIS *mris, const char *fname);
SMALL_SURFACE *fssurfio_read_triangle_vertex_positions_only(const char *fname);
int fssurfio_read_triangle_file_positions(MRIS *mris, const char *fname);
bool fssurfio_quad_combine(int quad[4], int vA[3], int vB[3]);
MRIS *fssurfio_read_triangle_file(const char *fname, double nVFMultiplier);
MRIS *fssurfio_read_quadrangle_file(const char *fname, double nVFMultiplier);
int fssurfio_write_quadrangle_file(MRIS *mris, const char *fname);
// mrisurf_io_stl.cpp's reader and writer (declared in mrisurf_io.cpp only)
MRIS *mrisReadSTL(const std::string &filename);
bool mrisWriteSTL(const MRIS *mris, const std::string &filename);

const char *Progname = "fssurfio";

namespace {

std::vector<std::string> toks;
size_t pos;
std::string out;

[[noreturn]] void fail(const char *what)
{
  fprintf(stderr, "fssurfio: %s\n", what);
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

// A byte string from a token: `h` and its bytes in hex (`h` alone is the empty string).
std::string geth()
{
  const char *t = next();
  if (t[0] != 'h') fail("a byte token starts with h");
  std::string r;
  for (const char *p = t + 1; p[0] && p[1]; p += 2) {
    char b[3] = {p[0], p[1], 0};
    r += (char)strtoul(b, NULL, 16);
  }
  return r;
}
void oh(const char *p)
{
  if (!p) { out += "NULL "; return; }
  char b[4];
  out += "h";
  for (const unsigned char *q = (const unsigned char *)p; *q; q++) {
    snprintf(b, sizeof b, "%02x", *q);
    out += b;
  }
  out += " ";
}
std::vector<std::string> keep;  // names whose pointers FreeSurfer keeps

// The 15 coordinate sets of a VERTEX, in the order the cases give them.
void set_coords(VERTEX *v)
{
  v->x = getf(); v->y = getf(); v->z = getf();
  v->origx = getf(); v->origy = getf(); v->origz = getf();
  v->tx = getf(); v->ty = getf(); v->tz = getf();
  v->cx = getf(); v->cy = getf(); v->cz = getf();
  v->infx = getf(); v->infy = getf(); v->infz = getf();
  v->pialx = getf(); v->pialy = getf(); v->pialz = getf();
  v->t2x = getf(); v->t2y = getf(); v->t2z = getf();
  v->fx = getf(); v->fy = getf(); v->fz = getf();
  v->whitex = getf(); v->whitey = getf(); v->whitez = getf();
  v->nx = getf(); v->ny = getf(); v->nz = getf();
  v->pnx = getf(); v->pny = getf(); v->pnz = getf();
  v->wnx = getf(); v->wny = getf(); v->wnz = getf();
  v->targx = getf(); v->targy = getf(); v->targz = getf();
  v->l4x = getf(); v->l4y = getf(); v->l4z = getf();
  v->dx = getf(); v->dy = getf(); v->dz = getf();
}

template <class T> T *arr(int n) { return (T *)calloc(n > 0 ? n : 1, sizeof(T)); }

// The metric accessors through a view (SurfaceFromMRIS::AllM or SurfaceFromMRIS_MP::AllM): every
// getter of every vertex and face, then the surface's.
template <class S> void read_view(S s, int nv, int nf, int ndist)
{
  for (int vno = 0; vno < nv; vno++) {
    auto v = s.vertices(vno);
    for (int i = 0; i < ndist; i++) { of(v.dist(i)); of(v.dist_orig(i)); }
    oi(v.dist_capacity());
    of(v.x()); of(v.y()); of(v.z()); of(v.origx()); of(v.origy()); of(v.origz());
    of(v.nx()); of(v.ny()); of(v.nz()); of(v.wnx()); of(v.wny()); of(v.wnz());
    of(v.dx()); of(v.dy()); of(v.dz()); of(v.curv()); of(v.cx()); of(v.cy()); of(v.cz());
    of(v.pialx()); of(v.pialy()); of(v.pialz()); of(v.whitex()); of(v.whitey()); of(v.whitez());
    of(v.area()); of(v.origarea()); oi(v.fno()); oi(v.neg()); oi(v.border()); oi(v.ripflag());
  }
  for (int fno = 0; fno < nf; fno++) {
    auto f = s.faces(fno);
    for (int i = 0; i < 3; i++) oi(f.v(i).vno());
    of(f.area());
    angles_per_triangle_t a = f.angle(), o = f.orig_angle();
    for (int i = 0; i < 3; i++) { of(a[i]); of(o[i]); }
    oi(f.ripflag());
  }
  oi(s.nvertices()); oi(s.nfaces());
  of(s.xctr()); of(s.yctr()); of(s.zctr()); of(s.xlo()); of(s.ylo()); of(s.zlo());
  of(s.xhi()); of(s.yhi()); of(s.zhi()); of(s.total_area()); od(s.avg_vertex_area());
  od(s.avg_vertex_dist()); od(s.std_vertex_dist()); of(s.orig_area()); of(s.neg_area());
  of(s.neg_orig_area()); od(s.radius()); oi(s.status()); oi(s.origxyz_status()); oi(s.patch());
  oi(s.noscale());
}

// Every setter of the views, with values from the case (`vno`, `fno` 0).
template <class S> void write_view(S s)
{
  auto v = s.vertices(0);
  v.set_x(getf()); v.set_y(getf()); v.set_z(getf());
  v.set_nx(getf()); v.set_ny(getf()); v.set_nz(getf());
  v.set_wnx(getf()); v.set_wny(getf()); v.set_wnz(getf());
  v.set_dx(getf()); v.set_dy(getf()); v.set_dz(getf());
  v.set_curv(getf()); v.set_cx(getf()); v.set_cy(getf()); v.set_cz(getf());
  v.set_pialx(getf()); v.set_pialy(getf()); v.set_pialz(getf());
  v.set_whitex(getf()); v.set_whitey(getf()); v.set_whitez(getf());
  v.set_area(getf()); v.set_origarea(getf()); v.set_fno(geti());
  v.set_neg((char)geti()); v.set_border((char)geti()); v.set_ripflag((char)geti());
  auto f = s.faces(0);
  int k = geti();
  f.set_v(0, s.vertices(k));
  f.set_area(getf());
  angles_per_triangle_t a, o;
  for (int i = 0; i < 3; i++) { a[i] = getf(); o[i] = getf(); }
  f.set_angle(a); f.set_orig_angle(o);
  f.set_ripflag((char)geti());
  s.set_xctr(getf()); s.set_yctr(getf()); s.set_zctr(getf());
  s.set_xlo(getf()); s.set_ylo(getf()); s.set_zlo(getf());
  s.set_xhi(getf()); s.set_yhi(getf()); s.set_zhi(getf());
  s.set_total_area(getf()); s.set_avg_vertex_area(getd()); s.set_avg_vertex_dist(getd());
  s.set_std_vertex_dist(getd()); s.set_orig_area(getf()); s.set_neg_area(getf());
  s.set_neg_orig_area(getf()); s.set_status((MRIS_Status)geti());
  s.set_origxyz_status((MRIS_Status)geti()); s.set_patch(geti());
}

// `view`: an MRIS with nv vertices and nf faces and an MRIS_MP of other values loaded on it by
// hand, read and written through both representations' views.
void view_case()
{
  int nv = geti(), nf = geti(), ndist = 2;
  MRIS *m = MRISalloc(nv, nf);
  MRIS_MP *mp = (MRIS_MP *)calloc(1, sizeof(MRIS_MP));
  MRISMP_ctr(mp);
  mp->underlyingMRIS = m;
  *(int *)&mp->nvertices = nv;
  *(int *)&mp->nfaces = nf;
#define A(N, T, n) mp->N = arr<T>(n)
  A(v_x, float, nv); A(v_y, float, nv); A(v_z, float, nv);
  A(v_origx, float, nv); A(v_origy, float, nv); A(v_origz, float, nv);
  A(v_nx, float, nv); A(v_ny, float, nv); A(v_nz, float, nv);
  A(v_wnx, float, nv); A(v_wny, float, nv); A(v_wnz, float, nv);
  A(v_dx, float, nv); A(v_dy, float, nv); A(v_dz, float, nv); A(v_curv, float, nv);
  A(v_cx, float, nv); A(v_cy, float, nv); A(v_cz, float, nv);
  A(v_pialx, float, nv); A(v_pialy, float, nv); A(v_pialz, float, nv);
  A(v_whitex, float, nv); A(v_whitey, float, nv); A(v_whitez, float, nv);
  A(v_area, float, nv); A(v_origarea, float, nv); A(v_fno, int, nv); A(v_neg, char, nv);
  A(v_border, char, nv); A(v_ripflag, char, nv); A(v_dist_capacity, int, nv);
  A(v_dist, float *, nv); A(v_dist_orig, float *, nv);
  A(f_v, vertices_per_face_t, nf); A(f_area, float, nf); A(f_angle, angles_per_triangle_t, nf);
  A(f_orig_angle, angles_per_triangle_t, nf); A(f_ripflag, char, nf);
#undef A
  // both representations' values from the case, the MRIS's first
  for (int vno = 0; vno < nv; vno++) {
    VERTEX *v = &m->vertices[vno];
    v->dist = arr<float>(ndist);
    v->dist_orig = arr<float>(ndist);
    for (int i = 0; i < ndist; i++) { v->dist[i] = getf(); v->dist_orig[i] = getf(); }
    v->dist_capacity = geti();
    set_coords(v);
    v->curv = getf(); v->area = getf(); v->origarea = getf(); v->fno = geti();
    v->neg = (char)geti(); v->border = (char)geti(); v->ripflag = (char)geti();
  }
  for (int fno = 0; fno < nf; fno++) {
    FACE *f = &m->faces[fno];
    for (int i = 0; i < 3; i++) f->v[i] = geti();
    f->area = getf();
    for (int i = 0; i < 3; i++) { f->angle[i] = getf(); f->orig_angle[i] = getf(); }
    f->ripflag = (char)geti();
  }
  m->xctr = getf(); m->yctr = getf(); m->zctr = getf(); m->xlo = getf(); m->ylo = getf();
  m->zlo = getf(); m->xhi = getf(); m->yhi = getf(); m->zhi = getf(); m->total_area = getf();
  m->avg_vertex_area = getd(); m->avg_vertex_dist = getd(); m->std_vertex_dist = getd();
  m->orig_area = getf(); m->neg_area = getf(); m->neg_orig_area = getf(); m->radius = getd();
  m->status = (MRIS_Status)geti(); m->origxyz_status = (MRIS_Status)geti(); m->patch = geti();
  m->noscale = geti();
  for (int vno = 0; vno < nv; vno++) {
    mp->v_dist[vno] = arr<float>(ndist);
    for (int i = 0; i < ndist; i++) mp->v_dist[vno][i] = getf();
    mp->v_dist_orig[vno] = m->vertices[vno].dist_orig;
    mp->v_dist_capacity[vno] = geti();
    mp->v_x[vno] = getf(); mp->v_y[vno] = getf(); mp->v_z[vno] = getf();
    mp->v_origx[vno] = getf(); mp->v_origy[vno] = getf(); mp->v_origz[vno] = getf();
    mp->v_nx[vno] = getf(); mp->v_ny[vno] = getf(); mp->v_nz[vno] = getf();
    mp->v_wnx[vno] = getf(); mp->v_wny[vno] = getf(); mp->v_wnz[vno] = getf();
    mp->v_dx[vno] = getf(); mp->v_dy[vno] = getf(); mp->v_dz[vno] = getf();
    mp->v_curv[vno] = getf(); mp->v_cx[vno] = getf(); mp->v_cy[vno] = getf();
    mp->v_cz[vno] = getf(); mp->v_pialx[vno] = getf(); mp->v_pialy[vno] = getf();
    mp->v_pialz[vno] = getf(); mp->v_whitex[vno] = getf(); mp->v_whitey[vno] = getf();
    mp->v_whitez[vno] = getf(); mp->v_area[vno] = getf(); mp->v_origarea[vno] = getf();
    mp->v_fno[vno] = geti(); mp->v_neg[vno] = (char)geti(); mp->v_border[vno] = (char)geti();
    mp->v_ripflag[vno] = (char)geti();
  }
  for (int fno = 0; fno < nf; fno++) {
    for (int i = 0; i < 3; i++) mp->f_v[fno][i] = geti();
    mp->f_area[fno] = getf();
    for (int i = 0; i < 3; i++) { mp->f_angle[fno][i] = getf(); mp->f_orig_angle[fno][i] = getf(); }
    mp->f_ripflag[fno] = (char)geti();
  }
  *(float *)&mp->xctr = getf(); mp->yctr = getf(); mp->zctr = getf(); mp->xlo = getf();
  mp->ylo = getf(); mp->zlo = getf(); mp->xhi = getf(); mp->yhi = getf(); mp->zhi = getf();
  mp->total_area = getf(); mp->avg_vertex_area = getd(); mp->avg_vertex_dist = getd();
  mp->std_vertex_dist = getd(); mp->orig_area = getf(); mp->neg_area = getf();
  mp->neg_orig_area = getf(); *(double *)&mp->radius = getd();
  *(MRIS_Status *)&mp->status = (MRIS_Status)geti();
  *(MRIS_Status *)&mp->origxyz_status = (MRIS_Status)geti();
  mp->patch = geti(); mp->noscale = geti();

  read_view(SurfaceFromMRIS::AllM::Surface(m), nv, nf, ndist);
  read_view(SurfaceFromMRIS_MP::AllM::Surface(mp), nv, nf, ndist);
  out += "| ";
  write_view(SurfaceFromMRIS::AllM::Surface(m));
  write_view(SurfaceFromMRIS_MP::AllM::Surface(mp));
  // what the writes reached, read back through both views
  read_view(SurfaceFromMRIS::AllM::Surface(m), nv, nf, ndist);
  read_view(SurfaceFromMRIS_MP::AllM::Surface(mp), nv, nf, ndist);
  oi(mp->status); oi(mp->patch);
}

// `surf`: an MRIS from the case (coordinates, rip flags, neighbourhoods, distances with their
// storage, faces, the box), a sequence of the coordinate and distance operations, then its state.
void dump_surface(MRIS *m)
{
  for (int vno = 0; vno < m->nvertices; vno++) {
    VERTEX *v = &m->vertices[vno];
    of(v->x); of(v->y); of(v->z); of(v->origx); of(v->origy); of(v->origz);
    of(v->cx); of(v->cy); of(v->cz);
    for (int o = 0; o < 2; o++) {
      float *d = o ? v->dist_orig : v->dist;
      int cap = o ? v->dist_orig_capacity : v->dist_capacity;
      void *st = o ? m->dist_orig_storage[vno] : m->dist_storage[vno];
      oi(d != NULL); oi(st != NULL); oi(cap);
      if (d) for (int k = 0; k < cap; k++) of(d[k]);
    }
    VERTEX_TOPOLOGY *vt = &m->vertices_topology[vno];
    for (int k = 0; k < vt->num; k++) oi(vt->n[k]);
  }
  for (int fno = 0; fno < m->nfaces; fno++) for (int k = 0; k < 3; k++) oi(m->faces[fno].v[k]);
  oi(m->dist_alloced_flags); oi(m->dist_nsize); oi(m->origxyz_status); oi(m->status);
  of(m->xlo); of(m->ylo); of(m->zlo); of(m->xhi); of(m->yhi); of(m->zhi);
  of(m->xctr); of(m->yctr); of(m->zctr);
}

void surf_case()
{
  if (geti()) setenv("FREESURFER_reportLogicErrors", "1", 1);
  int nv = geti(), nf = geti();
  MRIS *m = MRISalloc(nv, nf);
  m->status = (MRIS_Status)geti();
  m->origxyz_status = (MRIS_Status)geti();
  m->dist_alloced_flags = (char)geti();
  m->dist_nsize = (char)geti();
  for (int vno = 0; vno < nv; vno++) {
    VERTEX *v = &m->vertices[vno];
    VERTEX_TOPOLOGY *vt = &m->vertices_topology[vno];
    v->x = getf(); v->y = getf(); v->z = getf();
    v->origx = getf(); v->origy = getf(); v->origz = getf();
    v->cx = getf(); v->cy = getf(); v->cz = getf();
    v->tx = getf(); v->ty = getf(); v->tz = getf();
    v->nx = getf(); v->ny = getf(); v->nz = getf();
    v->dx = getf(); v->dy = getf(); v->dz = getf();
    v->ripflag = (char)geti();
    vt->vnum = (short)geti(); vt->v2num = (short)geti(); vt->v3num = (short)geti();
    vt->vtotal = (short)geti(); vt->nsizeMax = (uchar)geti();
    int num = geti();
    vt->num = (uchar)num;
    free(vt->f); free(vt->n);
    vt->f = arr<int>(num); vt->n = arr<uchar>(num);
    for (int k = 0; k < num; k++) { vt->f[k] = geti(); vt->n[k] = (uchar)geti(); }
    for (int o = 0; o < 2; o++) {
      // the buffer: -1 none, else its length, the capacity, whether the field points at it
      int len = geti();
      float *buf = NULL;
      int cap = 0, valid = 0;
      if (len >= 0) {
        buf = arr<float>(len);
        cap = geti(); valid = geti();
        for (int k = 0; k < len; k++) buf[k] = getf();
      }
      void **st = o ? &m->dist_orig_storage[vno] : &m->dist_storage[vno];
      *st = buf;
      *(float **)(o ? &v->dist_orig : &v->dist) = valid ? buf : NULL;
      *(int *)(o ? &v->dist_orig_capacity : &v->dist_capacity) = valid ? cap : 0;
    }
  }
  for (int fno = 0; fno < nf; fno++) for (int k = 0; k < 3; k++) m->faces[fno].v[k] = geti();
  m->xlo = getf(); m->ylo = getf(); m->zlo = getf(); m->xhi = getf(); m->yhi = getf();
  m->zhi = getf(); m->xctr = getf(); m->yctr = getf(); m->zctr = getf();
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    static bool later1, later2;
    if (op == "S") { int vno = geti(); float x = getf(), y = getf(), z = getf();
      MRISsetXYZwkr(m, vno, x, y, z, "probe.cpp", 7, &later1); }
    else if (op == "O") { int vno = geti(); float x = getf(), y = getf(), z = getf();
      MRISsetOriginalXYZwkr(m, vno, x, y, z, "probe.cpp", 8, &later2); }
    else if (op == "I") {
      std::vector<float> a(3 * (nv > 0 ? nv : 1));
      for (int i = 0; i < 3 * nv; i++) a[i] = getf();
      MRISimportXYZ(m, &a[0], &a[nv], &a[2 * nv]);
    }
    else if (op == "E") { float *px, *py, *pz; MRISexportXYZ(m, &px, &py, &pz);
      for (int i = 0; i < nv; i++) { of(px[i]); of(py[i]); of(pz[i]); } }
    else if (op == "C") {
      int n2 = geti();
      MRIS *b = MRISalloc(n2, 0);
      for (int i = 0; i < n2; i++) { b->vertices[i].x = getf(); b->vertices[i].y = getf(); b->vertices[i].z = getf(); }
      MRIScopyXYZ(m, b);
    }
    else if (op == "F") MRISsetOriginalXYZfromXYZ(m);
    else if (op == "R") { int w = geti(); double sd = 0; od(MRISavgVetexRadius(m, w ? &sd : NULL)); od(sd); }
    else if (op == "M") {
      int which = geti();
      std::vector<float> a(3 * (nv > 0 ? nv : 1));
      for (int i = 0; i < 3 * nv; i++) a[i] = getf();
      float *loc[3] = {&a[0], &a[nv], &a[2 * nv]};
      oi(MRISimportVertexCoords(m, loc, which));
    }
    else if (op == "V") { int d = geti(), r = geti(), w = geti(); oi(MRISreverseCoords(m, d, r, w)); }
    else if (op == "W") { int w = geti(), r = geti(); oi(MRISreverse(m, w, r)); }
    else if (op == "K") oi(MRISscale(m, getd()));
    else if (op == "P") oi(mrisFlipPatch(m));
    else if (op == "T") { float a = getf(), b = getf(), c = getf(); oi(MRIStranslate(m, a, b, c)); }
    else if (op == "Q") { double w[6]; for (int i = 0; i < 6; i++) w[i] = getd();
      MRISscaleThenTranslate(m, w[0], w[1], w[2], w[3], w[4], w[5]); }
    else if (op == "A") { float a = getf(), b = getf(), c = getf(); oi(MRISanisotropicScale(m, a, b, c)); }
    else if (op == "Z") { float a = getf(), b = getf(), c = getf(); MRISrotate(m, m, a, b, c); }
    else if (op == "Y") { MRIS_MP *mp = (MRIS_MP *)calloc(1, sizeof(MRIS_MP)); MRISMP_ctr(mp);
      MRISrotate(mp, 0.1f, 0.2f, 0.3f); }
    else if (op == "X") {
      int r = geti(), c = geti();
      MATRIX *M = r > 0 ? MatrixAlloc(r, c, MATRIX_REAL) : NULL;
      for (int i = 1; i <= r; i++) for (int j = 1; j <= c; j++) *MATRIX_RELT(M, i, j) = getf();
      oi(MRISmatrixMultiply(m, M));
    }
    else if (op == "H") oi(MRISsphericalCoords(m));
    else if (op == "G") { float a = getf(), b = getf(), c = getf(); MRISmoveOrigin(m, a, b, c); }
    else if (op == "N") MRIScenter(m, m);
    else if (op == "L") { double a, b, c; MRIScenterCOG2(m, &a, &b, &c); od(a); od(b); od(c); }
    else if (op == "U") MRIScenterCOG(m);
    else if (op == "B") { float a = getf(), b = getf(); MRISblendXYZandTXYZ(m, a, b); }
    else if (op == "D") MRISblendXYZandNXYZ(m, getf());
    else if (op == "J") MRIStranslate_along_vertex_dxdydz(m, m, getd());
    else if (op == "Dm") oi(mrisComputeSurfaceDimensions(m));
    else if (op == "md") MRISmakeDist(m, geti());
    else if (op == "mo") MRISmakeDistOrig(m, geti());
    else if (op == "gd") { int vno = geti(); MRISgrowDist(m, vno, geti()); }
    else if (op == "go") { int vno = geti(); MRISgrowDistOrig(m, vno, geti()); }
    else if (op == "sd") {
      int vno = geti(), cap = geti(), ncap = geti();
      float *p = mrisStealDistStore(m, vno, cap);
      for (int i = 0; i < cap; i++) p[i] = getf();
      mrisSetDist(m, vno, p, ncap);
    }
    else if (op == "fd") MRISfreeDistsButNotOrig(m);
    else if (op == "fo") MRISfreeDistOrigs(m);
    else fail("surf op");
  }
  out += "| ";
  dump_surface(m);
}

// `metric`: an MRIS from the case (15 coordinate sets, curvatures, areas, rip flags, neighbour
// lists, distances with their storage, faces), a sequence of the averaging, centring, distance,
// area and curvature operations, then its state. An operation whose memory accesses would leave
// what the case allocated prints `G` and is skipped (the port applies the same test).
std::vector<int> vlen;  // each vertex's neighbour-list length

int vnum_at(VERTEX_TOPOLOGY *vt, int n)
{
  return n == 1 ? vt->vnum : n == 2 ? vt->v2num : n == 3 ? vt->v3num : 0;
}

bool guard(MRIS *m, const std::string &op)
{
  for (int vno = 0; vno < m->nvertices; vno++) {
    VERTEX *v = &m->vertices[vno];
    VERTEX_TOPOLOGY *vt = &m->vertices_topology[vno];
    if (v->ripflag) continue;
    if (op == "co" && vt->vtotal > 0 && (!v->dist_orig || v->dist_orig_capacity < vt->vtotal)) return false;
    if (op == "sc") {
      int vs = mrisVertexVSize(m, vno);
      if (vs > 0 && (!v->dist || v->dist_capacity < vs)) return false;
    }
    if (op == "kd" && m->dist_nsize >= 1 && m->dist_nsize <= 3 && v->dist &&
        v->dist_capacity < vnum_at(vt, m->dist_nsize)) return false;
    if (op == "ko" && m->dist_orig_nsize >= 1 && m->dist_orig_nsize <= 3 && v->dist_orig &&
        v->dist_orig_capacity < vnum_at(vt, m->dist_orig_nsize)) return false;
    if (op == "pe" && vt->vtotal > 0 && ((v->dist && v->dist_capacity < vt->vtotal) ||
        (v->dist_orig && v->dist_orig_capacity < vt->vtotal))) return false;
  }
  return true;
}

void dump_metric(MRIS *m)
{
  for (int vno = 0; vno < m->nvertices; vno++) {
    VERTEX *v = &m->vertices[vno];
    of(v->x); of(v->y); of(v->z); of(v->origx); of(v->origy); of(v->origz);
    of(v->curv); of(v->H); of(v->val2); of(v->d); of(v->area); of(v->origarea);
    for (int o = 0; o < 2; o++) {
      float *d = o ? v->dist_orig : v->dist;
      int cap = o ? v->dist_orig_capacity : v->dist_capacity;
      void *st = o ? m->dist_orig_storage[vno] : m->dist_storage[vno];
      oi(d != NULL); oi(st != NULL); oi(cap);
      if (d) for (int k = 0; k < cap; k++) of(d[k]);
    }
  }
  for (int fno = 0; fno < m->nfaces; fno++) of(m->faces[fno].area);
  oi(m->dist_alloced_flags); oi(m->dist_nsize); oi(m->dist_orig_nsize); oi(m->status);
  oi(m->origxyz_status); of(m->max_curv); of(m->min_curv);
  of(m->xlo); of(m->ylo); of(m->zlo); of(m->xhi); of(m->yhi); of(m->zhi);
  of(m->xctr); of(m->yctr); of(m->zctr);
}

void metric_case()
{
  if (geti()) setenv("FREESURFER_reportLogicErrors", "1", 1);
  int fixes = geti();
  if (fixes & 1) setenv("FREESURFER_fix_MRISsmoothSurface1", "1", 1);
  if (fixes & 2) setenv("FREESURFER_fix_missing_dist_orig", "1", 1);
  if (geti()) Gdiag |= DIAG_SHOW | DIAG_VERBOSE;
  int nv = geti(), nf = geti();
  MRIS *m = MRISalloc(nv, nf);
  m->status = (MRIS_Status)geti();
  m->origxyz_status = (MRIS_Status)geti();
  m->dist_alloced_flags = (char)geti();
  m->dist_nsize = (char)geti();
  m->dist_orig_nsize = (char)geti();
  m->nsize = (char)geti();
  m->patch = geti();
  m->total_area = getf(); m->orig_area = getf();
  m->max_curv = getf(); m->min_curv = getf();
  vlen.assign(nv > 0 ? nv : 1, 0);
  for (int vno = 0; vno < nv; vno++) {
    VERTEX *v = &m->vertices[vno];
    VERTEX_TOPOLOGY *vt = &m->vertices_topology[vno];
    set_coords(v);
    v->curv = getf(); v->H = getf(); v->val2 = getf(); v->d = getf();
    v->area = getf(); v->origarea = getf();
    v->ripflag = (char)geti();
    vt->vnum = (short)geti(); vt->v2num = (short)geti(); vt->v3num = (short)geti();
    vt->vtotal = (short)geti(); vt->nsizeMax = (uchar)geti();
    int len = geti();
    vlen[vno] = len;
    free(vt->v);
    vt->v = arr<int>(len);
    for (int k = 0; k < len; k++) vt->v[k] = geti();
    for (int o = 0; o < 2; o++) {
      int blen = geti();
      float *buf = NULL;
      int cap = 0, valid = 0;
      if (blen >= 0) {
        buf = arr<float>(blen);
        cap = geti(); valid = geti();
        for (int k = 0; k < blen; k++) buf[k] = getf();
      }
      void **st = o ? &m->dist_orig_storage[vno] : &m->dist_storage[vno];
      *st = buf;
      *(float **)(o ? &v->dist_orig : &v->dist) = valid ? buf : NULL;
      *(int *)(o ? &v->dist_orig_capacity : &v->dist_capacity) = valid ? cap : 0;
    }
  }
  for (int fno = 0; fno < nf; fno++) {
    FACE *f = &m->faces[fno];
    for (int k = 0; k < 3; k++) f->v[k] = geti();
    f->area = getf();
    f->ripflag = (char)geti();
  }
  MRISsavedXYZ *saved = NULL;
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    if (op == "co" || op == "sc" || op == "kd" || op == "ko" || op == "pe") {
      if (!guard(m, op)) {
        out += "G ";
        if (op == "sc") getf();
        continue;
      }
    }
    if (op == "av") oi(MRISaverageVertexPositions(m, geti()));
    else if (op == "sm") { int n = geti(); float st = getf(); MRISsmoothSurface(m, n, st); }
    else if (op == "cs") MRIScenterSphere(m);
    else if (op == "co") oi(MRISclearOrigDistances(m));
    else if (op == "sc") oi(MRISscaleDistances(m, getf()));
    else if (op == "kd") oi(mrisCheckDist(m));
    else if (op == "ko") oi(mrisCheckDistOrig(m));
    else if (op == "vd") oi(mrisComputeVertexDistances(m));
    else if (op == "vo") oi(mrisComputeOriginalVertexDistances(m));
    else if (op == "vn") { mrisComputeOriginalVertexDistancesIfNecessary(m); }
    else if (op == "pe") od(MRISpercentDistanceError(m));
    else if (op == "ca") oi(MRISclearOrigArea(m));
    else if (op == "cv") MRISclearOrigAreaAndVal2(m);
    else if (op == "ar") { int fac = geti(); of(mrisComputeArea(m, fac, geti())); }
    else if (op == "zn") oi(MRISzeroNegativeAreas(m));
    else if (op == "lo" || op == "lt") {
      int fno = geti(), which = op == "lt" ? geti() : 0;
      double u[3][3];
      if (op == "lo") oi(load_orig_triangle_vertices(m, fno, u[0], u[1], u[2]));
      else oi(load_triangle_vertices(m, fno, u[0], u[1], u[2], which));
      for (int i = 0; i < 3; i++) for (int j = 0; j < 3; j++) od(u[i][j]);
    }
    else if (op == "ex") {
      int which = geti();
      std::vector<float> a(3 * (nv > 0 ? nv : 1), -7.0f);
      float *loc[3] = {&a[0], &a[nv], &a[2 * nv]};
      oi(MRISextractVertexCoords(m, loc, which));
      for (int i = 0; i < 3 * nv; i++) of(a[i]);
    }
    else if (op == "cc") oi(MRISclearCurvature(m));
    else if (op == "c2") MRISclearCurvAndVal2(m);
    else if (op == "rc") oi(MRISrectifyCurvature(m));
    else if (op == "sk") { float a = getf(), b = getf(); oi(MRISscaleCurvatures(m, a, b)); }
    else if (op == "sh") oi(MRISsmoothCurvatures(m, geti()));
    else if (op == "zm") oi(MRISzeroMeanCurvature(m));
    else if (op == "nc") {
      int w = geti();
      if (w == NORM_MEDIAN && nv == 0) { out += "G "; continue; }
      oi(MRISnormalizeCurvature(m, w));
    }
    else if (op == "nv") oi(MRISnormalizeCurvatureVariance(m));
    else if (op == "cp") oi(mrisCountCompressed(m, getd()));
    else if (op == "sH") oi(MRISstoreMeanCurvature(m));
    else if (op == "mm") oi(mrisComputeCurvatureMinMax(m));
    else if (op == "sx") { if (saved) MRISfreeXYZ(m, &saved); saved = MRISsaveXYZ(m); }
    else if (op == "lx") { if (saved) MRISloadXYZ(m, saved); else out += "N "; }
    else if (op == "px") { if (saved) MRISpopXYZ(m, &saved); else out += "N "; oi(saved != NULL); }
    else if (op == "lb") {
      MRIS *b = MRISalloc(nv + 1, 0);
      MRISsavedXYZ *sb = MRISsaveXYZ(b);
      MRISloadXYZ(m, sb);
    }
    else fail("metric op");
  }
  out += "| ";
  dump_metric(m);
}

// `posn` (relay 8): an MRIS from the case (15 coordinate sets, k1, k2, areas, angles, rip
// flags, neighbour counts, distances with their storage, faces with their face-normal cache),
// a sequence of the saved positions and normals, the canonical coordinates and the face-normal
// cache's operations (mrisurf_metricProperties.cpp's 3.1-50 part a, with the 3.1-49 functions
// that wait for it), then its state. `sm` and `rm` print `G` and are skipped where their reads
// or writes would leave what the case allocated (the port applies the same test).
bool posn_guard(MRIS *m, const std::string &op)
{
  for (int vno = 0; vno < m->nvertices; vno++) {
    VERTEX *v = &m->vertices[vno];
    if (v->ripflag) continue;
    int vs = mrisVertexVSize(m, vno);
    if (vs <= 0) continue;
    if (op == "sm" && v->dist &&
        (v->dist_capacity < vs || (v->dist_orig && v->dist_orig_capacity < vs))) return false;
    if (op == "rm" && (!v->dist || v->dist_capacity < vs || !v->dist_orig ||
        v->dist_orig_capacity < vs)) return false;
  }
  return true;
}

void dump_posn(MRIS *m)
{
  for (int vno = 0; vno < m->nvertices; vno++) {
    VERTEX *v = &m->vertices[vno];
    float c[45] = {v->x, v->y, v->z, v->origx, v->origy, v->origz, v->tx, v->ty, v->tz,
                   v->cx, v->cy, v->cz, v->infx, v->infy, v->infz, v->pialx, v->pialy, v->pialz,
                   v->t2x, v->t2y, v->t2z, v->fx, v->fy, v->fz, v->whitex, v->whitey, v->whitez,
                   v->nx, v->ny, v->nz, v->pnx, v->pny, v->pnz, v->wnx, v->wny, v->wnz,
                   v->targx, v->targy, v->targz, v->l4x, v->l4y, v->l4z, v->dx, v->dy, v->dz};
    for (float x : c) of(x);
    of(v->theta); of(v->phi); of(v->area); of(v->origarea);
    for (int o = 0; o < 2; o++) {
      float *d = o ? v->dist_orig : v->dist;
      int cap = o ? v->dist_orig_capacity : v->dist_capacity;
      void *st = o ? m->dist_orig_storage[vno] : m->dist_storage[vno];
      oi(d != NULL); oi(st != NULL); oi(cap);
      if (d) for (int k = 0; k < cap; k++) of(d[k]);
    }
  }
  for (int fno = 0; fno < m->nfaces; fno++) {
    FACE *f = &m->faces[fno];
    of(f->area);
    for (int k = 0; k < 3; k++) of(f->angle[k]);
    for (int k = 0; k < 3; k++) of(f->orig_angle[k]);
    FaceNormCacheEntry *e = &m->faceNormCacheEntries[fno];
    of(e->nx); of(e->ny); of(e->nz); of(e->orig_area);
    oi(m->faceNormDeferredEntries[fno].deferred);
  }
  od(m->radius); of(m->total_area); of(m->orig_area);
  oi(m->status); oi(m->origxyz_status); oi(m->dist_nsize); oi(m->dist_orig_nsize);
  oi(m->dist_alloced_flags);
  of(m->xlo); of(m->ylo); of(m->zlo); of(m->xhi); of(m->yhi); of(m->zhi);
  of(m->xctr); of(m->yctr); of(m->zctr);
}

void posn_case()
{
  if (geti()) setenv("FREESURFER_reportLogicErrors", "1", 1);
  if (geti()) Gdiag |= DIAG_SHOW | DIAG_VERBOSE;
  int nv = geti(), nf = geti();
  MRIS *m = MRISalloc(nv, nf);
  m->status = (MRIS_Status)geti();
  m->origxyz_status = (MRIS_Status)geti();
  m->dist_alloced_flags = (char)geti();
  m->dist_nsize = (char)geti();
  m->dist_orig_nsize = (char)geti();
  m->radius = getd();
  m->total_area = getf(); m->orig_area = getf();
  for (int vno = 0; vno < nv; vno++) {
    VERTEX *v = &m->vertices[vno];
    VERTEX_TOPOLOGY *vt = &m->vertices_topology[vno];
    set_coords(v);
    v->k1 = getf(); v->k2 = getf(); v->area = getf(); v->origarea = getf();
    v->theta = getf(); v->phi = getf();
    v->ripflag = (char)geti();
    vt->vnum = (short)geti(); vt->v2num = (short)geti(); vt->v3num = (short)geti();
    vt->vtotal = (short)geti(); vt->nsizeMax = (uchar)geti();
    for (int o = 0; o < 2; o++) {
      int blen = geti();
      float *buf = NULL;
      int cap = 0, valid = 0;
      if (blen >= 0) {
        buf = arr<float>(blen);
        cap = geti(); valid = geti();
        for (int k = 0; k < blen; k++) buf[k] = getf();
      }
      void **st = o ? &m->dist_orig_storage[vno] : &m->dist_storage[vno];
      *st = buf;
      *(float **)(o ? &v->dist_orig : &v->dist) = valid ? buf : NULL;
      *(int *)(o ? &v->dist_orig_capacity : &v->dist_capacity) = valid ? cap : 0;
    }
  }
  for (int fno = 0; fno < nf; fno++) {
    FACE *f = &m->faces[fno];
    for (int k = 0; k < 3; k++) f->v[k] = geti();
    f->area = getf();
    for (int k = 0; k < 3; k++) f->angle[k] = getf();
    for (int k = 0; k < 3; k++) f->orig_angle[k] = getf();
    f->ripflag = (char)geti();
    FaceNormCacheEntry *e = &m->faceNormCacheEntries[fno];
    e->nx = getf(); e->ny = getf(); e->nz = getf(); e->orig_area = getf();
    m->faceNormDeferredEntries[fno].deferred = (char)geti();
  }
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    if ((op == "sm" || op == "rm") && !posn_guard(m, op)) { out += "G "; continue; }
    if (op == "sv") oi(MRISsaveVertexPositions(m, geti()));
    else if (op == "rv") oi(MRISrestoreVertexPositions(m, geti()));
    else if (op == "sn") oi(MRISsaveNormals(m, geti()));
    else if (op == "rn") oi(MRISrestoreNormals(m, geti()));
    else if (op == "cc") oi(MRIScomputeCanonicalCoordinates(m));
    else if (op == "ar") od(MRISaverageCanonicalRadius(m));
    else if (op == "fo") od(MRIScomputeFolding(m));
    else if (op == "gn") {
      FaceNormCacheEntry const *e = getFaceNorm(m, geti());
      of(e->nx); of(e->ny); of(e->nz); of(e->orig_area);
    }
    else if (op == "sf") { int f = geti(); float a = getf(), b = getf(), c = getf(); setFaceNorm(m, f, a, b, c); }
    else if (op == "so") { int f = geti(); setFaceOrigArea(m, f, getf()); }
    else if (op == "go") of(getFaceOrigArea(m, geti()));
    else if (op == "cn") computeDefectFaceNormal(m, geti());
    else if (op == "df") mrisurf_deferSetFaceNorms(m);
    else if (op == "rf") mrisurf_recomputeFaceNorms(m);
    else if (op == "uf") mrisurf_undeferSetFaceNorms(m);
    else if (op == "oa") of(MRIScomputeOrigArea(m));
    else if (op == "sa") MRISsetOrigArea(m);
    else if (op == "sm") oi(MRISstoreMetricProperties(m));
    else if (op == "rm") oi(MRISrestoreMetricProperties(m));
    else if (op == "ro") oi(MRISrestoreOldPositions(m));
    else if (op == "st") oi(MRISstoreCurrentPositions(m));
    else fail("posn op");
  }
  out += "| ";
  dump_posn(m);
}

// `mprop` (relay 10, 3.1-51/52): an MRIS from the case (coordinates, normals, areas, flags, a
// mesh's topology with each vertex's faces and corners, distances with their storage, faces with
// their face-normal cache), the environment's switches of MRIScomputeMetricProperties, then 1 to
// 4 of its operations (MRIScomputeMetricProperties, MRIScomputeTriangleProperties,
// MRIScomputeNormals, MRIScomputeAvgInterVertexDist, MRISupdateEllipsoidSurface,
// mrisOrientSurface, mrisOrientPlane, mrisComputeSurfaceDimensions, and the callers
// MRISsmoothSurface, mrisDisturbVertices, MRISrecenter, MRISscaleBrainArea,
// MRISscaleVertexCoordinates, MRISscaleBrain), then its state.
void dump_mprop(MRIS *m)
{
  for (int vno = 0; vno < m->nvertices; vno++) {
    VERTEX *v = &m->vertices[vno];
    float c[10] = {v->x, v->y, v->z, v->nx, v->ny, v->nz, v->area, v->origarea, v->cx, v->curv};
    for (float x : c) of(x);
    oi(v->border); oi(v->neg); oi(v->fno);
    for (int o = 0; o < 2; o++) {
      float *d = o ? v->dist_orig : v->dist;
      int cap = o ? v->dist_orig_capacity : v->dist_capacity;
      void *st = o ? m->dist_orig_storage[vno] : m->dist_storage[vno];
      oi(d != NULL); oi(d != NULL || st != NULL); oi(cap);
      if (d) for (int k = 0; k < cap; k++) of(d[k]);
    }
  }
  for (int fno = 0; fno < m->nfaces; fno++) {
    FACE *f = &m->faces[fno];
    of(f->area);
    for (int k = 0; k < 3; k++) of(f->angle[k]);
    for (int k = 0; k < 3; k++) of(f->orig_angle[k]);
    FaceNormCacheEntry *e = &m->faceNormCacheEntries[fno];
    of(e->nx); of(e->ny); of(e->nz); of(e->orig_area);
    oi(m->faceNormDeferredEntries[fno].deferred);
  }
  of(m->xlo); of(m->ylo); of(m->zlo); of(m->xhi); of(m->yhi); of(m->zhi);
  of(m->xctr); of(m->yctr); of(m->zctr);
  of(m->total_area); od(m->avg_vertex_area); od(m->avg_vertex_dist); od(m->std_vertex_dist);
  of(m->neg_area); of(m->neg_orig_area);
  oi(m->dist_nsize); oi(m->dist_alloced_flags);
}

// the guard of `ad` and `an`: every unripped vertex with an unripped 1-hop neighbour has room
// for its 1-hop distances (MRIScomputeAvgInterVertexDist reads them unchecked)
bool avg_guard(MRIS *m)
{
  for (int vno = 0; vno < m->nvertices; vno++) {
    VERTEX *v = &m->vertices[vno];
    VERTEX_TOPOLOGY *vt = &m->vertices_topology[vno];
    if (v->ripflag) continue;
    bool any = false;
    for (int k = 0; k < vt->vnum; k++) if (!m->vertices[vt->v[k]].ripflag) any = true;
    if (any && (!v->dist || v->dist_capacity < vt->vnum)) return false;
  }
  return true;
}

void mprop_case()
{
  int envs = geti();
  if (envs & 1) setenv("FREESURFER_OLD_MRIScomputeMetricProperties", "1", 1);
  if (envs & 2) setenv("FREESURFER_NEW_MRIScomputeMetricProperties", "1", 1);
  if (envs & 4) setenv("FS_FASTER_MP", "1", 1);
  if (envs & 8) setenv("FS_WEIGHTS_NORMS_BY_ANGLE", "1", 1);
  if (envs & 16) setenv("FREESURFER_fastApproxAtan2f_use_atanf2", "1", 1);
  if (envs & 32) setenv("FREESURFER_reportLogicErrors", "1", 1);
  if (envs & 64) setenv("FREESURFER_SEED", "1234", 1);
  if (geti()) Gdiag |= DIAG_SHOW | DIAG_VERBOSE;
  UnitizeNormalFace = geti();
  MRISsetFixVertexAreaValue(geti());
  int nv = geti(), nf = geti();
  MRIS *m = MRISalloc(nv, nf);
  m->status = (MRIS_Status)geti();
  m->origxyz_status = (MRIS_Status)geti();
  m->nsize = (char)geti();
  m->dist_alloced_flags = (char)geti();
  m->dist_nsize = (char)geti();
  m->radius = getd();
  m->total_area = getf();
  m->avg_vertex_dist = getd();
  for (int vno = 0; vno < nv; vno++) {
    VERTEX *v = &m->vertices[vno];
    VERTEX_TOPOLOGY *vt = &m->vertices_topology[vno];
    v->x = getf(); v->y = getf(); v->z = getf();
    v->origx = getf(); v->origy = getf(); v->origz = getf();
    v->cx = getf(); v->cy = getf(); v->cz = getf();
    v->whitex = getf(); v->whitey = getf(); v->whitez = getf();
    v->pialx = getf(); v->pialy = getf(); v->pialz = getf();
    v->wnx = getf(); v->wny = getf(); v->wnz = getf();
    v->nx = getf(); v->ny = getf(); v->nz = getf();
    v->curv = getf(); v->area = getf(); v->origarea = getf();
    v->border = (char)geti(); v->fno = geti(); v->neg = (char)geti(); v->ripflag = (char)geti();
    vt->vnum = (short)geti(); vt->v2num = (short)geti(); vt->v3num = (short)geti();
    vt->vtotal = (short)geti(); vt->nsizeMax = (uchar)geti();
    int len = geti();
    free(vt->v);
    vt->v = arr<int>(len);
    for (int k = 0; k < len; k++) vt->v[k] = geti();
    int num = geti();
    vt->num = (uchar)num;
    free(vt->f);
    free(vt->n);
    vt->f = arr<int>(num);
    vt->n = arr<uchar>(num);
    for (int k = 0; k < num; k++) vt->f[k] = geti();
    for (int k = 0; k < num; k++) vt->n[k] = (uchar)geti();
    for (int o = 0; o < 2; o++) {
      int blen = geti();
      float *buf = NULL;
      int cap = 0, valid = 0;
      if (blen >= 0) {
        buf = arr<float>(blen);
        cap = geti(); valid = geti();
        for (int k = 0; k < blen; k++) buf[k] = getf();
      }
      void **st = o ? &m->dist_orig_storage[vno] : &m->dist_storage[vno];
      *st = buf;
      *(float **)(o ? &v->dist_orig : &v->dist) = valid ? buf : NULL;
      *(int *)(o ? &v->dist_orig_capacity : &v->dist_capacity) = valid ? cap : 0;
    }
  }
  for (int fno = 0; fno < nf; fno++) {
    FACE *f = &m->faces[fno];
    for (int k = 0; k < 3; k++) f->v[k] = geti();
    f->ripflag = (char)geti();
    f->area = getf();
    for (int k = 0; k < 3; k++) f->angle[k] = getf();
    for (int k = 0; k < 3; k++) f->orig_angle[k] = getf();
    FaceNormCacheEntry *e = &m->faceNormCacheEntries[fno];
    e->nx = getf(); e->ny = getf(); e->nz = getf(); e->orig_area = getf();
    m->faceNormDeferredEntries[fno].deferred = (char)geti();
  }
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    if (op == "mp") oi(MRIScomputeMetricProperties(m));
    else if (op == "tp") oi(MRIScomputeTriangleProperties(m));
    else if (op == "cn") oi(MRIScomputeNormals(m));
    else if ((op == "ad" || op == "an") && !avg_guard(m)) out += "G ";
    else if (op == "ad") { double sd = 0; MRIScomputeAvgInterVertexDist(m, &sd); od(sd); }
    else if (op == "an") MRIScomputeAvgInterVertexDist(m, NULL);
    else if (op == "ue") oi(MRISupdateEllipsoidSurface(m));
    else if (op == "os") oi(mrisOrientSurface(m));
    else if (op == "op") oi(mrisOrientPlane(m));
    else if (op == "sd") oi(mrisComputeSurfaceDimensions(m));
    else if (op == "ss") { int n = geti(); float st = getf(); MRISsmoothSurface(m, n, st); }
    else if (op == "dv") mrisDisturbVertices(m, getd());
    else if (op == "rc") { int mv = geti(); int tg = geti(); MRISrecenter(m, mv, tg); }
    else if (op == "sb") oi(MRISscaleBrainArea(m));
    else if (op == "sv") oi(MRISscaleVertexCoordinates(m, getd()));
    else if (op == "sB") MRISscaleBrain(m, m, getf());
    else fail("mprop op");
  }
  out += "| ";
  dump_mprop(m);
}

// `life` (relay 9, 3.1-47 part a): a surface from MRISalloc or MRISoverAlloc, a sequence of its
// reallocations, counts, settings, rip-flag operations, arrays and dumps, then its state and
// MRISfree. An operation that would read or write past what the surface has allocated prints `G`
// and is skipped (the port applies the same test).
std::map<const void *, int> lens;  // the lengths of the topology arrays the case gave

bool life_has_vertices(MRIS *m)
{
  return m->vertices && m->vertices_topology && m->dist_storage && m->dist_orig_storage;
}

// mrisDumpVertex's reads stay inside what the case allocated
bool dump_ok(MRIS *m, int vno)
{
  if (vno < 0 || vno >= m->nvertices || !life_has_vertices(m)) return false;
  VERTEX_TOPOLOGY *vt = &m->vertices_topology[vno];
  VERTEX *v = &m->vertices[vno];
  int vs = mrisVertexVSize(m, vno);
  if (vs > 0) {
    if (!vt->v || lens[vt->v] < vs) return false;
    if (v->dist && v->dist_capacity < vs) return false;
    if (v->dist_orig && v->dist_orig_capacity < vs) return false;
  }
  if (vt->num > 0 && (!vt->f || !vt->n || lens[vt->f] < vt->num)) return false;
  return true;
}

void dump_life(MRIS *m)
{
  oi(m->nvertices); oi(m->max_vertices); oi(m->nfaces); oi(m->max_faces);
  oi(m->nverticesFrozen); oi(m->nsize); oi(m->max_nsize); oi(m->dist_alloced_flags);
  oi(m->vertices != NULL); oi(m->vertices_topology != NULL); oi(m->dist_storage != NULL);
  oi(m->dist_orig_storage != NULL); oi(m->faces != NULL); oi(m->faceNormCacheEntries != NULL);
  oi(m->faceNormDeferredEntries != NULL);
  oi(m->dx2 != NULL); oi(m->dy2 != NULL); oi(m->dz2 != NULL);
  if (life_has_vertices(m)) {
    for (int vno = 0; vno < m->max_vertices; vno++) {
      VERTEX *v = &m->vertices[vno];
      VERTEX_TOPOLOGY *vt = &m->vertices_topology[vno];
      of(v->x); of(v->y); of(v->z); of(v->odx); of(v->dz);
      oi(v->ripflag); oi(v->oripflag); oi(v->marked);
      oi(vt->vnum); oi(vt->vtotal); oi(vt->nsizeMax); oi(vt->nsizeCur); oi(vt->num);
      oi(vt->v != NULL); oi(vt->f != NULL);
      for (int o = 0; o < 2; o++) {
        float *d = o ? v->dist_orig : v->dist;
        int cap = o ? v->dist_orig_capacity : v->dist_capacity;
        void *st = o ? m->dist_orig_storage[vno] : m->dist_storage[vno];
        oi(d != NULL); oi(st != NULL); oi(cap);
        if (d) for (int k = 0; k < cap; k++) of(d[k]);
      }
    }
  }
  if (m->faces && m->faceNormCacheEntries && m->faceNormDeferredEntries) {
    for (int fno = 0; fno < m->max_faces; fno++) {
      FACE *f = &m->faces[fno];
      oi(f->v[0]); oi(f->v[1]); oi(f->v[2]); oi(f->ripflag); oi(f->oripflag); oi(f->marked);
      FaceNormCacheEntry *e = &m->faceNormCacheEntries[fno];
      of(e->nx); of(e->ny); of(e->nz); of(e->orig_area);
      oi(m->faceNormDeferredEntries[fno].deferred);
    }
  }
}

void print_flags(char *p, int n)
{
  if (!p) { out += "NULL "; return; }
  for (int k = 0; k < n; k++) oi(p[k]);
  free(p);
}

void life_case()
{
  MRIS *m;
  std::string init = next();
  if (init == "A") {
    int nv = geti(), nf = geti();
    m = MRISalloc(nv, nf);
  } else {
    int mv = geti(), mf = geti(), nv = geti(), nf = geti();
    m = MRISoverAlloc(mv, mf, nv, nf);
  }
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    if (op == "rv") {
      int a = geti(), b = geti();
      if (m->nvertices < 0) { out += "G "; continue; }
      oi(MRISreallocVertices(m, a, b));
    } else if (op == "rf") {
      int a = geti(), b = geti();
      if (m->nfaces < 0) { out += "G "; continue; }
      oi(MRISreallocFaces(m, a, b));
    } else if (op == "af") {
      int a = geti();
      if (m->nfaces < 0) { out += "G "; continue; }
      oi(MRISallocateFaces(m, a));
    } else if (op == "ov") {
      int a = geti(), b = geti(), c = geti(), d = geti();
      if (m->nvertices < 0 || m->nfaces < 0) { out += "G "; continue; }
      MRISoverAllocVerticesAndFaces(m, a, b, c, d);
    } else if (op == "ra") {
      int a = geti(), b = geti();
      if (m->nvertices < 0 || m->nfaces < 0) { out += "G "; continue; }
      MRISreallocVerticesAndFaces(m, a, b);
    } else if (op == "gv") { MRISgrowNVertices(m, geti());
    } else if (op == "tv") { MRIStruncateNVertices(m, geti());
    } else if (op == "xv") { MRISremovedVertices(m, geti());
    } else if (op == "gf") { MRISgrowNFaces(m, geti());
    } else if (op == "tf") { MRIStruncateNFaces(m, geti());
    } else if (op == "xf") { MRISremovedFaces(m, geti());
    } else if (op == "fz") { MRISacquireNverticesFrozen(m);
    } else if (op == "fr") { MRISreleaseNverticesFrozen(m);
    } else if (op == "sv") {
      int vno = geti();
      float c[8];
      for (float &x : c) x = getf();
      int rip = geti(), marked = geti();
      if (vno >= m->max_vertices || !life_has_vertices(m)) { out += "G "; continue; }
      VERTEX *v = &m->vertices[vno];
      v->x = c[0]; v->y = c[1]; v->z = c[2];
      v->odx = c[3]; v->ody = c[4]; v->odz = c[5]; v->dx = c[6]; v->dy = c[7];
      v->dz = c[0]; v->nx = c[1]; v->ny = c[2]; v->nz = c[3];
      v->ripflag = (char)rip; v->marked = marked;
    } else if (op == "st") {
      int vno = geti(), vnum = geti(), vtotal = geti(), smax = geti(), scur = geti(), num = geti();
      std::vector<int> nb(vtotal > 0 ? vtotal : 0), fs(num), ns(num);
      for (int &x : nb) x = geti();
      for (int &x : fs) x = geti();
      for (int &x : ns) x = geti();
      if (vno >= m->max_vertices || !life_has_vertices(m)) { out += "G "; continue; }
      VERTEX_TOPOLOGY *vt = &m->vertices_topology[vno];
      vt->vnum = (short)vnum; vt->vtotal = (short)vtotal;
      vt->nsizeMax = (uchar)smax; vt->nsizeCur = (uchar)scur; vt->num = (uchar)num;
      vt->v = (int *)malloc(nb.size() * sizeof(int) + 1);
      memcpy(vt->v, nb.data(), nb.size() * sizeof(int));
      lens[vt->v] = (int)nb.size();
      vt->f = (int *)malloc(num * sizeof(int) + 1);
      memcpy(vt->f, fs.data(), num * sizeof(int));
      lens[vt->f] = num;
      vt->n = (uchar *)malloc(num + 1);
      for (int i = 0; i < num; i++) vt->n[i] = (uchar)ns[i];
    } else if (op == "sd") {
      int vno = geti();
      if (vno >= m->nvertices || !life_has_vertices(m)) { out += "G "; continue; }
      MRISmakeDist(m, vno);
      VERTEX *v = &m->vertices[vno];
      for (int i = 0; i < v->dist_capacity; i++) v->dist[i] = (float)(i + 1 + 10 * vno);
    } else if (op == "sf") {
      int fno = geti(), a = geti(), b = geti(), c = geti(), rip = geti(), marked = geti();
      float n[4];
      for (float &x : n) x = getf();
      int deferred = geti();
      if (fno >= m->max_faces || !m->faces) { out += "G "; continue; }
      FACE *f = &m->faces[fno];
      f->v[0] = a; f->v[1] = b; f->v[2] = c; f->ripflag = (char)rip; f->marked = marked;
      FaceNormCacheEntry *e = &m->faceNormCacheEntries[fno];
      e->nx = n[0]; e->ny = n[1]; e->nz = n[2]; e->orig_area = n[3];
      m->faceNormDeferredEntries[fno].deferred = (char)deferred;
    } else if (op == "ex") {
      if (m->nfaces > 0 && !m->faces) { out += "G "; continue; }
      if (m->nvertices > 0 && !m->vertices) { out += "G "; continue; }
      print_flags(MRISexportFaceRipflags(m), m->nfaces);
      print_flags(MRISexportVertexRipflags(m), m->nvertices);
    } else if (op == "im") {
      int s = geti();
      if ((m->nfaces > 0 && !m->faces) || (m->nvertices > 0 && !m->vertices)) { out += "G "; continue; }
      std::vector<char> ff(m->nfaces > 0 ? m->nfaces : 0), vf(m->nvertices > 0 ? m->nvertices : 0);
      for (size_t i = 0; i < ff.size(); i++) ff[i] = (char)(((int)i * 7 + s) % 3 - 1);
      for (size_t i = 0; i < vf.size(); i++) vf[i] = (char)(((int)i * 5 + s) % 3 - 1);
      MRISimportFaceRipflags(m, ff.data());
      MRISimportVertexRipflags(m, vf.data());
    } else if (op == "cr") {
      if (m->nvertices > 0 && !m->vertices) { out += "G "; continue; }
      oi(MRIScountRipped(m));
    } else if (op == "so" || op == "re" || op == "ur") {
      if ((m->nfaces > 0 && !m->faces) || (m->nvertices > 0 && !m->vertices)) { out += "G "; continue; }
      oi(op == "so" ? MRISstoreRipFlags(m) : op == "re" ? MRISrestoreRipFlags(m) : MRISunrip(m));
    } else if (op == "ga") {
      if ((m->nfaces > 0 && !m->faces) || (m->nvertices > 0 && !m->vertices) ||
          m->nvertices < 0 || m->nfaces < 0) { out += "G "; continue; }
      float *a = MRISgetVertexArray(m);
      for (int i = 0; i < 3 * m->nvertices; i++) of(a[i]);
      delete[] a;
      float *b = MRISgetVertexNormalArray(m);
      for (int i = 0; i < 3 * m->nvertices; i++) of(b[i]);
      delete[] b;
      int *c = MRISgetFaceArray(m);
      for (int i = 0; i < 3 * m->nfaces; i++) oi(c[i]);
      delete[] c;
    } else if (op == "gn") {
      bool ok = m->nfaces >= 0 && (m->nfaces == 0 || (m->faces && life_has_vertices(m)));
      for (int fno = 0; ok && fno < m->nfaces; fno++)
        for (int i = 0; i < 3; i++)
          if (m->faces[fno].v[i] < 0 || m->faces[fno].v[i] >= m->max_vertices) ok = false;
      if (!ok) { out += "G "; continue; }
      float *a = MRISgetFaceNormalArray(m);
      for (int i = 0; i < 3 * m->nfaces; i++) of(a[i]);
      delete[] a;
    } else if (op == "eg") {
      oi(MRISallocExtraGradients(m));
    } else if (op == "cn") {
      if (m->nvertices > 0 && !m->vertices) { out += "G "; continue; }
      MRIScheckForNans(m);
    } else if (op == "dv") {
      int vno = geti();
      if (!dump_ok(m, vno)) { out += "G "; continue; }
      mrisDumpVertex(stdout, m, vno);
    } else if (op == "df") {
      int fno = geti();
      if (fno < 0 || fno >= m->nfaces || !m->faces) { out += "G "; continue; }
      mrisDumpFace(m, fno, stdout);
    } else if (op == "ds") {
      bool ok = true;
      for (int vno = 0; vno < std::min(10, m->nvertices); vno++) ok = ok && dump_ok(m, vno);
      if (m->nfaces > 0 && !m->faces) ok = false;
      if (!ok) { out += "G "; continue; }
      mrisDumpShape(stdout, m);
    } else {
      fail("life op");
    }
  }
  out += "| ";
  dump_life(m);
  MRISfree(&m);
  oi(m == NULL);
}

// `meta` (relay 9): MRIScopyMetadata from a surface with a type, status, hemisphere, volume
// geometry, transform array and Talairach matrices into a new surface or a given one.
void set_vg(VOL_GEOM *vg)
{
  vg->valid = geti(); vg->width = geti(); vg->height = geti(); vg->depth = geti();
  vg->xsize = getf(); vg->ysize = getf(); vg->zsize = getf(); vg->ras_good_flag = geti();
  float *f[15] = {&vg->x_r, &vg->x_a, &vg->x_s, &vg->y_r, &vg->y_a, &vg->y_s, &vg->z_r, &vg->z_a,
                  &vg->z_s, &vg->c_r, &vg->c_a, &vg->c_s, &vg->s_r, &vg->s_a, &vg->s_s};
  for (float *p : f) *p = getf();
  std::string name = geth();
  snprintf(vg->fname, sizeof vg->fname, "%s", name.c_str());
}

void dump_vg(VOL_GEOM *vg)
{
  oi(vg->valid); oi(vg->width); oi(vg->height); oi(vg->depth);
  of(vg->xsize); of(vg->ysize); of(vg->zsize); oi(vg->ras_good_flag);
  float f[15] = {vg->x_r, vg->x_a, vg->x_s, vg->y_r, vg->y_a, vg->y_s, vg->z_r, vg->z_a,
                 vg->z_s, vg->c_r, vg->c_a, vg->c_s, vg->s_r, vg->s_a, vg->s_s};
  for (float x : f) of(x);
  oh(vg->fname);
  oi(vg->i_to_r__ != NULL); oi(vg->r_to_i__ != NULL);
}

void set_meta(MRIS *m)
{
  m->type = geti(); m->status = (MRIS_Status)geti(); m->hemisphere = geti();
  m->free_transform = geti();
  set_vg(&m->vg);
  int nx = geti();
  if (nx > 0) {
    m->lta = LTAalloc(nx, NULL);
    m->lta->type = geti();
  }
  for (int k = 0; k < 2; k++) {
    if (!geti()) continue;
    MATRIX *mt = MatrixAlloc(4, 4, MATRIX_REAL);
    for (int r = 1; r <= 4; r++)
      for (int c = 1; c <= 4; c++) *MATRIX_RELT(mt, r, c) = getf();
    if (k) m->TalSRASToSRAS_ = mt; else m->SRASToTalSRAS_ = mt;
  }
}

void dump_meta(MRIS *m)
{
  oi(m->nvertices); oi(m->nfaces); oi(m->type); oi(m->status); oi(m->hemisphere);
  oi(m->free_transform);
  dump_vg(&m->vg);
  if (m->lta) { oi(m->lta->num_xforms); oi(m->lta->type); } else out += "NULL ";
  for (MATRIX *mt : {m->SRASToTalSRAS_, m->TalSRASToSRAS_}) {
    if (!mt) { out += "NULL "; continue; }
    for (int r = 1; r <= 4; r++)
      for (int c = 1; c <= 4; c++) of(*MATRIX_RELT(mt, r, c));
  }
}

void meta_case()
{
  int nv = geti(), nf = geti();
  MRIS *src = MRISalloc(nv, nf);
  set_meta(src);
  MRIS *dst = NULL;
  if (geti()) {
    int nv2 = geti(), nf2 = geti();
    dst = MRISalloc(nv2, nf2);
    set_meta(dst);
  }
  MRIS *r = MRIScopyMetadata(src, dst);
  oi(r == dst);
  out += "| ";
  dump_meta(r);
  MRISfree(&src);
  MRISfree(&r);
}

// `misc` (relay 9): the curvature names, the registration's scales, the column printing, the
// progress bar, the vector searches, signum_eval and the two sort comparisons.
std::vector<std::vector<float>> keepf;

void misc_case()
{
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    if (op == "cn") {
      int nth = geti();
      keep.push_back(geth());
      oi(MRISsetCurvatureName(nth, keep.back().c_str()));
    } else if (op == "pc") {
      oi(MRISprintCurvatureNames(stdout));
    } else if (op == "ss") {
      int n = geti();
      if (n < 0) {
        oi(MRISsetRegistrationSigmas(NULL, 0));
      } else {
        int len = geti();
        keepf.emplace_back(len + 1);  // never empty: an empty vector's data() may be NULL
        for (int i = 0; i < len; i++) keepf.back()[i] = getf();
        oi(MRISsetRegistrationSigmas(keepf.back().data(), n));
      }
      od(nsigmas);
      for (int i = 0; i < (int)nsigmas; i++) of(sigmas[i]);
    } else if (op == "cs") {
      std::string l = geth(), r = geth();
      cprints(l.c_str(), r.c_str());
    } else if (op == "cd") {
      std::string l = geth();
      cprintd(l.c_str(), geti());
    } else if (op == "cf") {
      std::string l = geth();
      cprintf(l.c_str(), getf());
    } else if (op == "vp") {
      int nv = geti(), from = geti(), to = geti(), hasmsg = geti();
      std::string msg = geth();
      MRIS *m = MRISalloc(nv, 0);
      for (int v = from; v <= to; v++) oi(MRIS_vertexProgress_print(m, v, hasmsg ? msg.c_str() : NULL));
      MRISfree(&m);
    } else if (op == "ve") {
      int rows = geti();
      MATRIX *v = MatrixAlloc(rows, 1, MATRIX_REAL);
      for (int r = 1; r <= rows; r++) *MATRIX_RELT(v, r, 1) = getf();
      float t = getf();
      oi(VECTOR_elementIndex_findNotEqual(v, t));
      oi(VECTOR_elementIndex_find(v, t));
      MatrixFree(&v);
    } else if (op == "sg") {
      oi(signum_eval(getf()));
    } else if (op == "vc") {
      VERTEX_SORT a, b;
      a.vtxno = geti(); a.x = getf(); a.y = getf(); a.z = getf();
      b.vtxno = geti(); b.x = getf(); b.y = getf(); b.z = getf();
      oi(CompareVertexCoords(&a, &b)); oi(CompareVertexCoords(&b, &a));
    } else if (op == "fc") {
      FACE_SORT a, b;
      a.faceno = geti(); a.v0 = geti(); a.v1 = geti(); a.v2 = geti();
      b.faceno = geti(); b.v0 = geti(); b.v1 = geti(); b.v2 = geti();
      oi(CompareFaceVertices(&a, &b)); oi(CompareFaceVertices(&b, &a));
    } else {
      fail("misc op");
    }
  }
}

// `edges` (relay 9): edgesIntersect on pairs of edges of a surface whose canonical coordinates
// the case gives.
void edges_case()
{
  int nv = geti();
  MRIS *m = MRISalloc(nv, 0);
  for (int vno = 0; vno < nv; vno++) {
    VERTEX *v = &m->vertices[vno];
    v->cx = getf(); v->cy = getf(); v->cz = getf();
  }
  int n = geti();
  for (int k = 0; k < n; k++) {
    EDGE a, b;
    memset(&a, 0, sizeof a);
    memset(&b, 0, sizeof b);
    a.vno1 = geti(); a.vno2 = geti(); b.vno1 = geti(); b.vno2 = geti();
    oi(edgesIntersect(m, &a, &b));
  }
  MRISfree(&m);
}

// `sfile` (relay 11, 3.1-47 part b): the surface library's small file readers and writers
// (mrisurf_io.cpp): MRISsetReadFrame/MRISgetReadFrame, MRISreadTriangleProperties,
// MRISwriteTriangleProperties, MRISwriteDists, MRISwriteAreaError, MRISwriteAngleError,
// MRISwriteCurvatureToWFile, MRISreadValuesBak, MRISreadImagValues, MRIScopyMarksToAnnotation,
// MRISmaxMarked, MRISwriteAscii, MRISwriteNormalsAscii, MRISwriteVTK, MRISwriteCurvVTK. Each case
// runs in an empty work folder (`<cases>.d`, with a subfolder `d`): the case's input files are
// written first, then a surface from MRISoverAlloc with the case's vertices, faces (every
// allocated place) and face-normal cache, then 1 to 3 operations on file names relative to the
// folder, then the surface's state and every file in the folder (its relative name and bytes,
// sorted by name). A name argument is passed with two NUL bytes before it (what the hemisphere's
// scan reads for a dot at index 0 or 1). `G`: an operation that would read or write memory the
// surface does not own (the port applies the same test).
std::string workdir;

void clear_dir(const std::string &d, bool top)
{
  if (DIR *dp = opendir(d.c_str())) {
    while (struct dirent *e = readdir(dp)) {
      std::string n = e->d_name;
      if (n == "." || n == "..") continue;
      std::string p = d + "/" + n;
      struct stat st;
      if (lstat(p.c_str(), &st) == 0 && S_ISDIR(st.st_mode)) clear_dir(p, false);
      else unlink(p.c_str());
    }
    closedir(dp);
  }
  if (!top) rmdir(d.c_str());
}

const char *name_tok()
{
  std::string n = geth();
  char *p = (char *)calloc(n.size() + 3, 1);
  memcpy(p + 2, n.data(), n.size());
  return p + 2;
}

void ox(const std::string &b)
{
  char h[4];
  out += "x";
  for (unsigned char c : b) {
    snprintf(h, sizeof h, "%02x", c);
    out += h;
  }
  out += " ";
}

// every regular file under the work folder, by its name relative to it ('/' between parts)
void list_files(const std::string &d, const std::string &rel, std::vector<std::string> &files)
{
  if (DIR *dp = opendir(d.c_str())) {
    while (struct dirent *e = readdir(dp)) {
      std::string n = e->d_name;
      if (n == "." || n == "..") continue;
      std::string p = d + "/" + n, r = rel.empty() ? n : rel + "/" + n;
      struct stat st;
      if (lstat(p.c_str(), &st) != 0) continue;
      if (S_ISDIR(st.st_mode)) list_files(p, r, files);
      else if (S_ISREG(st.st_mode)) files.push_back(r);
    }
    closedir(dp);
  }
}

void sfile_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  errno = 0;
  int envs = geti();
  if (envs & 1) setenv("FS_POSIX", "1", 1);
  if (envs & 2) {
    std::string f = geth();
    setenv("MRIS_READVAL_FRAME", f.c_str(), 1);
  }
  int gd = geti();
  if (gd & 1) Gdiag |= DIAG_SHOW;
  if (gd & 2) Gdiag |= DIAG_VERBOSE;
  MRISsetFixVertexAreaValue(geti());
  int nin = geti();
  for (int k = 0; k < nin; k++) {
    std::string name = geth(), bytes = geth();
    FILE *fp = fopen(name.c_str(), "wb");
    if (!fp) fail("an input file");
    fwrite(bytes.data(), 1, bytes.size(), fp);
    fclose(fp);
  }
  errno = 0;
  int maxv = geti(), maxf = geti(), nv = geti(), nf = geti();
  MRIS *m = MRISoverAlloc(maxv, maxf, nv, nf);
  m->hemisphere = geti();
  std::string fname = geth();
  snprintf(m->fname.data(), STRLEN, "%s", fname.c_str());
  for (int vno = 0; vno < nv; vno++) {
    VERTEX *v = &m->vertices[vno];
    VERTEX_TOPOLOGY *vt = &m->vertices_topology[vno];
    v->x = getf(); v->y = getf(); v->z = getf();
    v->nx = getf(); v->ny = getf(); v->nz = getf();
    v->d = getf(); v->curv = getf(); v->val = getf(); v->valbak = getf(); v->imag_val = getf();
    v->area = getf(); v->origarea = getf();
    v->marked = geti(); v->annotation = geti(); v->ripflag = (char)geti();
    int num = geti();
    vt->num = (uchar)num;
    free(vt->f);
    free(vt->n);
    vt->f = arr<int>(num);
    vt->n = arr<uchar>(num);
    for (int k = 0; k < num; k++) vt->f[k] = geti();
    for (int k = 0; k < num; k++) vt->n[k] = (uchar)geti();
  }
  for (int fno = 0; fno < maxf; fno++) {
    FACE *f = &m->faces[fno];
    for (int k = 0; k < 3; k++) f->v[k] = geti();
    f->ripflag = (char)geti();
    f->area = getf();
    for (int k = 0; k < 3; k++) f->angle[k] = getf();
    for (int k = 0; k < 3; k++) f->orig_angle[k] = getf();
    FaceNormCacheEntry *e = &m->faceNormCacheEntries[fno];
    e->nx = getf(); e->ny = getf(); e->nz = getf(); e->orig_area = getf();
    m->faceNormDeferredEntries[fno].deferred = (char)geti();
  }
  m->orig_area = getf();
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    if (op == "sf") MRISsetReadFrame(geti());
    else if (op == "gf") oi(MRISgetReadFrame());
    else if (op == "rt" || op == "wt") {
      // a surface name of 1,024 bytes or more overruns FileNamePath's buffer (FS-134), and the
      // writer reads the place after the last face past the array when there is none (FS-128)
      const char *nm = name_tok();
      if (strlen(nm) >= STRLEN || (op == "wt" && nf > 0 && nf == maxf)) out += "G ";
      else if (op == "rt") oi(MRISreadTriangleProperties(m, nm));
      else oi(MRISwriteTriangleProperties(m, nm));
    }
    else if (op == "wd") oi(MRISwriteDists(m, name_tok()));
    else if (op == "wa") oi(MRISwriteAreaError(m, name_tok()));
    else if (op == "wg") oi(MRISwriteAngleError(m, name_tok()));
    else if (op == "wc") oi(MRISwriteCurvatureToWFile(m, name_tok()));
    else if (op == "rb") oi(MRISreadValuesBak(m, name_tok()));
    else if (op == "ri") oi(MRISreadImagValues(m, name_tok()));
    else if (op == "ca") oi(MRIScopyMarksToAnnotation(m));
    else if (op == "mm" && maxv == 0) out += "G ";  // vertex 0 past an empty array (FS-132)
    else if (op == "mm") oi(MRISmaxMarked(m));
    else if (op == "wA") oi(MRISwriteAscii(m, name_tok()));
    else if (op == "wn") oi(MRISwriteNormalsAscii(m, name_tok()));
    else if (op == "wv") oi(MRISwriteVTK(m, name_tok()));
    else if (op == "wC") oi(MRISwriteCurvVTK(m, name_tok()));
    else fail("sfile op");
  }
  out += "| ";
  for (int vno = 0; vno < nv; vno++) {
    VERTEX *v = &m->vertices[vno];
    float c[6] = {v->valbak, v->imag_val, v->area, v->origarea, v->nx, v->nz};
    for (float x : c) of(x);
    oi(v->annotation);
  }
  for (int fno = 0; fno < nf; fno++) {
    FACE *f = &m->faces[fno];
    of(f->area);
    for (int k = 0; k < 3; k++) of(f->angle[k]);
    for (int k = 0; k < 3; k++) of(f->orig_angle[k]);
    of(m->faceNormCacheEntries[fno].orig_area);
    oi(m->faceNormDeferredEntries[fno].deferred);
  }
  of(m->orig_area); of(m->total_area);
  out += "| ";
  std::vector<std::string> files;
  list_files(".", "", files);
  std::sort(files.begin(), files.end());
  for (const std::string &name : files) {
    oh(name.c_str());
    FILE *fp = fopen(name.c_str(), "rb");
    if (!fp) { out += "NOFILE "; continue; }
    std::string b;
    int ch;
    while ((ch = fgetc(fp)) != EOF) b += (char)ch;
    fclose(fp);
    ox(b);
  }
}

// `cfile` (relay 12, 3.1-48 part a): mrisurf_io.cpp's curvature, value and format files that need
// nothing unported: mrisReadAsciiCurvatureFile, mrisWriteAsciiCurvatureFile,
// __mrisreadcurvoldformat, MRISreadCurvatureVector, MRISreadCurvatureIntoArray,
// MRISreadNewCurvatureIntoArray, MRISreadNewCurvatureVector, MRISreadFloatFile,
// MRISreadBinaryAreas, MRISbuildFileName_read, MRISwriteDecimation, MRISreadDecimation,
// __MRISapplyFSGIIread, __MRISapplyFSGIIwrite, MRISreadVTK, MRISwriteGeo, MRISwriteICO,
// getSRASToTalSRAS, MatlabPlotFace, MatlabPlotVertex. As `sfile`: an empty work folder (with a
// subfolder `d`), the case's input files, a surface from MRISalloc with the case's vertices and
// faces, 1 to 3 operations on names relative to the folder, then the surface's state and every
// file in the folder. `G`: an operation that would read or write memory the program does not own
// (the port applies the same test). A volume for the values (`mt` >= 0: MRIalloc(nv, 1, 1, mt))
// is printed after the operation.
MRI *case_mri(int nv)
{
  int t = geti();
  if (t < 0 || nv < 1) return NULL;
  return MRIalloc(nv, 1, 1, t);
}

void dump_mri(MRI *mri)
{
  if (!mri) { out += "NOMRI "; return; }
  for (int i = 0; i < mri->width; i++) of(MRIgetVoxVal(mri, i, 0, 0, 0));
  MRIfree(&mri);
}

void dump_floats(const float *p, int n)
{
  if (!p) { out += "NULL "; return; }
  for (int i = 0; i < n; i++) of(p[i]);
}

void dump_vtk_surface(MRIS *s)
{
  oi(s->nvertices); oi(s->nfaces); oi(s->type); oi(s->max_vertices); oi(s->max_faces);
  for (int vno = 0; vno < s->nvertices; vno++) {
    VERTEX *v = &s->vertices[vno];
    of(v->x); of(v->y); of(v->z); of(v->curv); of(v->val);
    oi(s->vertices_topology[vno].num);
  }
  for (int fno = 0; fno < s->nfaces; fno++) for (int k = 0; k < 3; k++) oi(s->faces[fno].v[k]);
  of(s->max_curv); of(s->min_curv); oi(s->dist_alloced_flags);
}

void cfile_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  errno = 0;
  int envs = geti();
  if (envs & 1) setenv("FS_POSIX", "1", 1);
  if (envs & 2) {
    std::string f = geth();
    setenv("FS_GII", f.c_str(), 1);
  }
  int gd = geti();
  if (gd & 1) Gdiag |= DIAG_SHOW;
  if (gd & 2) Gdiag |= DIAG_VERBOSE;
  int nin = geti();
  for (int k = 0; k < nin; k++) {
    std::string name = geth(), bytes = geth();
    FILE *fp = fopen(name.c_str(), "wb");
    if (!fp) fail("an input file");
    fwrite(bytes.data(), 1, bytes.size(), fp);
    fclose(fp);
  }
  errno = 0;
  int nv = geti(), nf = geti();
  MRIS *m = MRISalloc(nv, nf);
  m->hemisphere = geti();
  std::string fname = geth();
  snprintf(m->fname.data(), STRLEN, "%s", fname.c_str());
  for (int vno = 0; vno < nv; vno++) {
    VERTEX *v = &m->vertices[vno];
    v->x = getf(); v->y = getf(); v->z = getf();
    v->nx = getf(); v->ny = getf(); v->nz = getf();
    v->d = getf(); v->curv = getf(); v->val = getf(); v->origarea = getf();
    v->ripflag = (char)geti(); v->undefval = geti(); v->fixedval = geti();
  }
  for (int fno = 0; fno < nf; fno++) {
    FACE *f = &m->faces[fno];
    for (int k = 0; k < 3; k++) f->v[k] = geti();
    f->ripflag = (char)geti();
  }
  m->max_curv = getf();
  m->min_curv = getf();
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    if (op == "ra") { const char *nm = name_tok(); MRI *mri = case_mri(nv);
      oi(mrisReadAsciiCurvatureFile(m, nm, mri)); dump_mri(mri); }
    else if (op == "wa") oi(mrisWriteAsciiCurvatureFile(m, (char *)name_tok()));
    else if (op == "ro") { const char *nm = name_tok(); MRI *mri = case_mri(nv);
      oi(fssurfio_read_curv_old_format(m, nm, mri)); dump_mri(mri); }
    else if (op == "cv" || op == "nv") {
      // a name of 1,024 bytes or more is copied into a 1,024-byte buffer (FS-126: abort)
      const char *nm = name_tok();
      float *p = op == "cv" ? MRISreadCurvatureVector(m, nm) : MRISreadNewCurvatureVector(m, nm);
      dump_floats(p, nv);
    }
    else if (op == "ca" || op == "na") {
      const char *nm = name_tok();
      int size = geti();
      float *p = NULL;
      oi(op == "ca" ? fssurfio_read_curvature_into_array(nm, size, &p)
                   : fssurfio_read_new_curvature_into_array(nm, size, &p));
      dump_floats(p, size);
    }
    else if (op == "rf") oi(MRISreadFloatFile(m, name_tok()));
    else if (op == "rr") oi(MRISreadBinaryAreas(m, name_tok()));
    else if (op == "bf") {
      const char *nm = name_tok();
      // the unchecked sprintf and strcpy into the caller's buffer
      if (strlen(nm) + strlen(m->fname.data()) + 8 >= STRLEN) { out += "G "; continue; }
      char buf[STRLEN];
      oi(MRISbuildFileName_read(m, nm, buf));
      oh(buf);
    }
    else if (op == "wd") oi(MRISwriteDecimation(m, (char *)name_tok()));
    else if (op == "rd") oi(MRISreadDecimation(m, (char *)name_tok()));
    else if (op == "gr" || op == "gw") {
      const char *nm = name_tok();
      int type = geti();
      const char *g = getenv("FS_GII");
      // the unchecked sprintf into the caller's buffer of MRISwrite (FS_GII set: it comes first)
      if (op == "gw" && g && !strcmp(g, ".gii") && strlen(nm) + 5 > STRLEN) { out += "G "; continue; }
      static char buf[4 * STRLEN];
      snprintf(buf, sizeof buf, "%s", nm);
      if (op == "gr") fssurfio_apply_fsgii_read(buf, nm, &type);
      else fssurfio_apply_fsgii_write(buf, nm, &type);
      oh(buf); oi(type);
    }
    else if (op == "vt") {
      int given = geti();
      const char *nm = name_tok();
      MRI *mri = case_mri(nv);
      MRIS *r = MRISreadVTK(given ? m : NULL, nm, mri);
      if (!r) out += "NULL ";
      else if (r == m) out += "SAME ";
      else { out += "NEW "; dump_vtk_surface(r); }
      dump_mri(mri);
    }
    else if (op == "wg") {
      const char *nm = name_tok();
      // an unripped face of a ripped vertex reads a vertex number never written
      bool bad = false;
      for (int fno = 0; fno < nf; fno++) {
        FACE *f = &m->faces[fno];
        if (f->ripflag) continue;
        for (int n = 0; n < 3; n++) if (m->vertices[f->v[n]].ripflag) bad = true;
      }
      if (bad) { out += "G "; continue; }
      oi(MRISwriteGeo(m, nm));
    }
    else if (op == "wi") oi(MRISwriteICO(m, name_tok()));
    else if (op == "st") {
      LTA *lta = LTAalloc(1, NULL);
      LT *lt = &lta->xforms[0];
      for (int r = 1; r <= 4; r++) for (int c = 1; c <= 4; c++) *MATRIX_RELT(lt->m_L, r, c) = getf();
      lt->src.c_r = getf(); lt->src.c_a = getf(); lt->src.c_s = getf();
      lt->dst.c_r = getf(); lt->dst.c_a = getf(); lt->dst.c_s = getf();
      MATRIX *res = getSRASToTalSRAS(lt);
      if (!res) out += "NULL ";
      else for (int r = 1; r <= 4; r++) for (int c = 1; c <= 4; c++) of(*MATRIX_RELT(res, r, c));
    }
    else if (op == "mf" || op == "mv") {
      const char *nm = name_tok();
      int which = geti();
      char color = (char)geti();
      double len = getd();
      FILE *fp = fopen(nm, "w");
      if (!fp) { out += "NOFP "; continue; }
      oi(op == "mf" ? MatlabPlotFace(fp, m, which, color, len) : MatlabPlotVertex(fp, m, which, color, len));
      fclose(fp);
    }
    else if (op == "un") UnitizeNormalFace = geti();
    else fail("cfile op");
  }
  out += "| ";
  for (int vno = 0; vno < nv; vno++) {
    VERTEX *v = &m->vertices[vno];
    float c[10] = {v->x, v->y, v->z, v->nx, v->ny, v->nz, v->d, v->curv, v->val, v->origarea};
    for (float x : c) of(x);
    oi(v->undefval); oi(v->fixedval);
  }
  of(m->max_curv); of(m->min_curv);
  out += "| ";
  std::vector<std::string> files;
  list_files(".", "", files);
  std::sort(files.begin(), files.end());
  for (const std::string &name : files) {
    oh(name.c_str());
    FILE *fp = fopen(name.c_str(), "rb");
    if (!fp) { out += "NOFILE "; continue; }
    std::string b;
    int ch;
    while ((ch = fgetc(fp)) != EOF) b += (char)ch;
    fclose(fp);
    ox(b);
  }
}

// `sread` (relay 13, 3.1-48 part b): mrisurf_io.cpp's file-local surface readers, through
// io_statics.cpp: mrisReadAsciiFile, mrisReadGeoFile, mrisReadGeoFilePositions,
// mrisReadTriangleFileVertexPositionsOnly, mrisReadTriangleFilePositions, and quadCombine (the old
// writer's quadrangles). As `cfile`: an empty work folder (with a subfolder `d`), whether
// FREESURFER_reportLogicErrors is set, Gdiag's bits, the case's input files, a surface from MRISalloc (coordinates, rip flags, faces, the distances' flags and size:
// MRISsetXYZ's message when the distances are marked allocated), 1 to 3 operations, then the case
// surface's coordinates and distance state. A surface a reader returns is printed in full, the small surface's vertices too.
void dump_read_surface(MRIS *s)
{
  if (!s) { out += "NULL "; return; }
  oi(s->nvertices); oi(s->nfaces); oi(s->type); oi(s->patch); oi(s->status);
  oi(s->max_vertices); oi(s->max_faces);
  for (int vno = 0; vno < s->nvertices; vno++) {
    VERTEX *v = &s->vertices[vno];
    of(v->x); of(v->y); of(v->z); oi(v->ripflag);
    oi(s->vertices_topology[vno].num);
  }
  for (int fno = 0; fno < s->nfaces; fno++) {
    for (int k = 0; k < 3; k++) oi(s->faces[fno].v[k]);
    oi(s->faces[fno].ripflag);
  }
  oi(s->dist_alloced_flags);
}

void sread_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  if (geti()) setenv("FREESURFER_reportLogicErrors", "1", 1);
  int gd = geti();
  if (gd & 1) Gdiag |= DIAG_SHOW;
  if (gd & 2) Gdiag |= DIAG_VERBOSE;
  int nin = geti();
  for (int k = 0; k < nin; k++) {
    std::string name = geth(), bytes = geth();
    FILE *fp = fopen(name.c_str(), "wb");
    if (!fp) fail("an input file");
    fwrite(bytes.data(), 1, bytes.size(), fp);
    fclose(fp);
  }
  errno = 0;
  int nv = geti(), nf = geti();
  MRIS *m = MRISalloc(nv, nf);
  std::string fname = geth();
  snprintf(m->fname.data(), STRLEN, "%s", fname.c_str());
  for (int vno = 0; vno < nv; vno++) {
    VERTEX *v = &m->vertices[vno];
    v->x = getf(); v->y = getf(); v->z = getf();
    v->ripflag = (char)geti();
  }
  for (int fno = 0; fno < nf; fno++) for (int k = 0; k < 3; k++) m->faces[fno].v[k] = geti();
  m->dist_alloced_flags = (char)geti();
  m->dist_nsize = geti();
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    if (op == "ra") dump_read_surface(fssurfio_read_ascii_file(name_tok()));
    else if (op == "rg") dump_read_surface(fssurfio_read_geo_file(name_tok()));
    else if (op == "rp") oi(fssurfio_read_geo_file_positions(m, name_tok()));
    else if (op == "rv") {
      SMALL_SURFACE *s = fssurfio_read_triangle_vertex_positions_only(name_tok());
      if (!s) { out += "NULL "; continue; }
      oi(s->nvertices);
      if (!s->vertices) { out += "NOVERTICES "; continue; }
      for (int vno = 0; vno < s->nvertices; vno++) {
        of(s->vertices[vno].x); of(s->vertices[vno].y); of(s->vertices[vno].z);
      }
      MRISSfree(&s);
    }
    else if (op == "rt") oi(fssurfio_read_triangle_file_positions(m, name_tok()));
    else if (op == "qc") {
      int a[3], b[3], q[4] = {-7, -7, -7, -7};
      for (int &x : a) x = geti();
      for (int &x : b) x = geti();
      oi(fssurfio_quad_combine(q, a, b));
      for (int x : q) oi(x);
    }
    else fail("sread op");
  }
  out += "| ";
  for (int vno = 0; vno < nv; vno++) {
    VERTEX *v = &m->vertices[vno];
    of(v->x); of(v->y); of(v->z);
  }
  oi(m->dist_alloced_flags); oi(m->dist_nsize);
}

// `stl` (relay 13, 3.1-49): mrisurf_io_stl.cpp's mrisReadSTL (binary or text) and mrisWriteSTL.
// As `sread`: an empty work folder (with `d`), Gdiag's bits, the case's input files, a surface
// from MRISalloc (coordinates, faces, the face-normal cache's entries and deferred flags) for the
// writer, 1 to 3 operations, then every file in the folder. A surface the reader returns is
// printed in full: counts and type, each vertex's coordinates, face count, neighbour counts and
// lists, face lists and positions, each face's vertices and cached normal.
void dump_stl_surface(MRIS *s)
{
  if (!s) { out += "NULL "; return; }
  oi(s->nvertices); oi(s->nfaces); oi(s->type); oi(s->max_vertices); oi(s->max_faces);
  for (int vno = 0; vno < s->nvertices; vno++) {
    VERTEX *v = &s->vertices[vno];
    VERTEX_TOPOLOGY *vt = &s->vertices_topology[vno];
    of(v->x); of(v->y); of(v->z);
    oi(vt->num); oi(vt->vnum); oi(vt->vtotal);
    for (int k = 0; k < vt->vnum; k++) oi(vt->v[k]);
    for (int k = 0; k < vt->num; k++) { oi(vt->f[k]); oi(vt->n[k]); }
  }
  for (int fno = 0; fno < s->nfaces; fno++) {
    for (int k = 0; k < 3; k++) oi(s->faces[fno].v[k]);
    FaceNormCacheEntry *e = &s->faceNormCacheEntries[fno];
    of(e->nx); of(e->ny); of(e->nz);
    oi(s->faceNormDeferredEntries[fno].deferred);
  }
}

void stl_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  int gd = geti();
  if (gd & 1) Gdiag |= DIAG_SHOW;
  if (gd & 2) Gdiag |= DIAG_VERBOSE;
  int nin = geti();
  for (int k = 0; k < nin; k++) {
    std::string name = geth(), bytes = geth();
    FILE *fp = fopen(name.c_str(), "wb");
    if (!fp) fail("an input file");
    fwrite(bytes.data(), 1, bytes.size(), fp);
    fclose(fp);
  }
  errno = 0;
  int nv = geti(), nf = geti();
  MRIS *m = MRISalloc(nv, nf);
  for (int vno = 0; vno < nv; vno++) {
    VERTEX *v = &m->vertices[vno];
    v->x = getf(); v->y = getf(); v->z = getf();
  }
  for (int fno = 0; fno < nf; fno++) {
    for (int k = 0; k < 3; k++) m->faces[fno].v[k] = geti();
    FaceNormCacheEntry *e = &m->faceNormCacheEntries[fno];
    e->nx = getf(); e->ny = getf(); e->nz = getf(); e->orig_area = getf();
    m->faceNormDeferredEntries[fno].deferred = (char)geti();
  }
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    if (op == "rs") dump_stl_surface(mrisReadSTL(name_tok()));
    else if (op == "ws") oi(mrisWriteSTL(m, name_tok()));
    else fail("stl op");
  }
  out += "| ";
  std::vector<std::string> files;
  list_files(".", "", files);
  std::sort(files.begin(), files.end());
  for (const std::string &name : files) {
    oh(name.c_str());
    FILE *fp = fopen(name.c_str(), "rb");
    if (!fp) { out += "NOFILE "; continue; }
    std::string b;
    int ch;
    while ((ch = fgetc(fp)) != EOF) b += (char)ch;
    fclose(fp);
    ox(b);
  }
}

// `tri` (relay 14, 3.1-48): mrisurf_io.cpp's binary triangle and quadrangle surface files:
// mrisReadTriangleFile, __MRISreadQuadrangleFile, MRISwriteTriangularSurface (with its space tags)
// and __MRISwriteQuadrangleFile. An empty work folder (with `d`), Gdiag's bits, the environment
// (a mask: TRIANGULARSURFACE_NOEXTRA_READ, TRIANGULARSURFACE_NOEXTRA_WRITE, FS_SKIP_TAGS, USER,
// LOGNAME, the last two set to the case's user name; FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING
// always set to the case's date), the case's input files, a surface from MRISalloc for the writers
// (coordinates, faces, the real-RAS flag, the volume geometry, a Talairach matrix or none, the
// group-average area, the command lines and their count), 1 to 3 operations, then every file in
// the folder. A surface a reader returns is printed in full: counts, type, room, each vertex's
// coordinates and face count (and, for `rq`, the first k entries of its face list, k from the
// case), each face, the real-RAS flag, the group-average area, the volume geometry, the command
// lines.
void dump_tri_surface(MRIS *s, const std::vector<int> &nf)
{
  if (!s) { out += "NULL "; return; }
  oi(s->nvertices); oi(s->nfaces); oi(s->type); oi(s->max_vertices); oi(s->max_faces);
  for (int vno = 0; vno < s->nvertices; vno++) {
    VERTEX *v = &s->vertices[vno];
    VERTEX_TOPOLOGY *vt = &s->vertices_topology[vno];
    of(v->x); of(v->y); of(v->z); oi(vt->num);
    oi(vt->f != NULL);
    int k = vno < (int)nf.size() ? nf[vno] : 0;
    if (vt->f) for (int j = 0; j < k && j < vt->num; j++) oi(vt->f[j]);
  }
  for (int fno = 0; fno < s->nfaces; fno++) for (int k = 0; k < 3; k++) oi(s->faces[fno].v[k]);
  oi(s->useRealRAS); of(s->group_avg_surface_area);
  dump_vg(&s->vg);
  oi(s->ncmds);
  for (int i = 0; i < s->ncmds && i < MAX_CMDS; i++) oh(s->cmdlines[i]);
}

// relay 15: MRISwrite in every format its name chooses (GIFTI excepted: it comes with gifti.cpp's
// surface part), MRISwriteVertexLocations, MRISwritePatchAscii and MRISreadVerticesOnly, in an
// empty work folder (with a subfolder d): Gdiag's bits, the environment (a mask:
// TRIANGULARSURFACE_NOEXTRA_WRITE, USER; the creation time's replacement and the licence check's
// side door always set), the case's input files, a surface from MRISalloc (per vertex the current,
// white and pial coordinates, rip flag and border; per face its vertices and rip flag; its type,
// useRealRAS, volume geometry and name), 1 to 3 operations, the surface's current coordinates,
// then every file in the folder
void swr_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  int gd = geti();
  if (gd & 1) Gdiag |= DIAG_SHOW;
  if (gd & 2) Gdiag |= DIAG_VERBOSE;
  int env = geti();
  std::string user = geth(), date = geth();
  if (env & 1) setenv("TRIANGULARSURFACE_NOEXTRA_WRITE", "1", 1);
  if (env & 2) setenv("USER", user.c_str(), 1);
  setenv("FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING", date.c_str(), 1);
  setenv("SURFER_SIDEDOOR", "1", 1);
  int nin = geti();
  for (int k = 0; k < nin; k++) {
    std::string name = geth(), bytes = geth();
    FILE *fp = fopen(name.c_str(), "wb");
    if (!fp) fail("an input file");
    fwrite(bytes.data(), 1, bytes.size(), fp);
    fclose(fp);
  }
  errno = 0;
  int nv = geti(), nf = geti();
  MRIS *m = MRISalloc(nv, nf);
  for (int vno = 0; vno < nv; vno++) {
    VERTEX *v = &m->vertices[vno];
    v->x = getf(); v->y = getf(); v->z = getf();
    v->whitex = getf(); v->whitey = getf(); v->whitez = getf();
    v->pialx = getf(); v->pialy = getf(); v->pialz = getf();
    v->ripflag = geti();
    v->border = geti();
  }
  for (int fno = 0; fno < nf; fno++) {
    for (int k = 0; k < 3; k++) m->faces[fno].v[k] = geti();
    m->faces[fno].ripflag = geti();
  }
  m->type = geti();
  m->useRealRAS = geti();
  set_vg(&m->vg);
  std::string sf = geth();
  if (sf.size() >= STRLEN) fail("a surface name longer than the surface's buffer");
  strcpy(m->fname, sf.c_str());
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    char *nm = (char *)name_tok();
    if (op == "w") oi(MRISwrite(m, nm));
    else if (op == "wl") { int which = geti(); oi(MRISwriteVertexLocations(m, nm, which)); }
    else if (op == "wp") oi(MRISwritePatchAscii(m, nm));
    else if (op == "rv") {
      SMALL_SURFACE *ss = MRISreadVerticesOnly(nm);
      if (!ss) { out += "NULL "; continue; }
      oi(ss->nvertices);
      for (int vno = 0; vno < ss->nvertices; vno++) {
        of(ss->vertices[vno].x); of(ss->vertices[vno].y); of(ss->vertices[vno].z);
      }
      MRISSfree(&ss);
    }
    else fail("swr op");
  }
  for (int vno = 0; vno < nv; vno++) { of(m->vertices[vno].x); of(m->vertices[vno].y); of(m->vertices[vno].z); }
  out += "| ";
  std::vector<std::string> files;
  list_files(".", "", files);
  std::sort(files.begin(), files.end());
  for (const std::string &name : files) {
    oh(name.c_str());
    FILE *fp = fopen(name.c_str(), "rb");
    if (!fp) { out += "NOFILE "; continue; }
    std::string b;
    int ch;
    while ((ch = fgetc(fp)) != EOF) b += (char)ch;
    fclose(fp);
    ox(b);
  }
}

// relay 15: the surface library's file names: MRISfileNameType and the curvature and annotation
// name builders (MRISwriteCurvature_getfilename, MRISreadCurvatureFile_getfilename,
// MRISwriteAnnotation_getfilename, MRISreadAnnotation_getfilename, with and without a surface),
// in an empty work folder with the folders d, d/s and label: the environment (a mask: FS_POSIX,
// FS_GII .gii, FS_GII x), the case's input files, the surface's hemisphere and name, 1 to 4
// operations; each operation's answer and the name it chose (a 8,192-byte buffer)
void fnam_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  mkdir("d/s", 0700);
  mkdir("label", 0700);
  int env = geti();
  if (env & 1) setenv("FS_POSIX", "1", 1);
  if (env & 2) setenv("FS_GII", ".gii", 1);
  if (env & 4) setenv("FS_GII", "x", 1);
  int nin = geti();
  for (int k = 0; k < nin; k++) {
    std::string name = geth(), bytes = geth();
    FILE *fp = fopen(name.c_str(), "wb");
    if (!fp) fail("an input file");
    fwrite(bytes.data(), 1, bytes.size(), fp);
    fclose(fp);
  }
  errno = 0;
  MRIS *m = MRISalloc(0, 0);
  m->hemisphere = geti();
  std::string sf = geth();
  if (sf.size() >= STRLEN) fail("a surface name longer than the surface's buffer");
  strcpy(m->fname, sf.c_str());
  int nops = geti();
  static char buf[8192];
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    const char *nm = name_tok();
    memset(buf, 0, sizeof buf);
    if (op == "t") { oi(MRISfileNameType(nm)); continue; }
    if (op == "wc") oi(MRISwriteCurvature_getfilename(m, nm, buf));
    else if (op == "rc") oi(MRISreadCurvatureFile_getfilename(m, nm, buf));
    else if (op == "wa") oi(MRISwriteAnnotation_getfilename(m, nm, buf));
    else if (op == "ra") oi(MRISreadAnnotation_getfilename(m, nm, buf));
    else if (op == "rn") oi(MRISreadAnnotation_getfilename(NULL, nm, buf));
    else fail("fnam op");
    oh(buf);
  }
}

void tri_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  int gd = geti();
  if (gd & 1) Gdiag |= DIAG_SHOW;
  if (gd & 2) Gdiag |= DIAG_VERBOSE;
  int env = geti();
  std::string user = geth(), date = geth();
  if (env & 1) setenv("TRIANGULARSURFACE_NOEXTRA_READ", "1", 1);
  if (env & 2) setenv("TRIANGULARSURFACE_NOEXTRA_WRITE", "1", 1);
  if (env & 4) setenv("FS_SKIP_TAGS", "1", 1);
  if (env & 8) setenv("USER", user.c_str(), 1);
  if (env & 16) setenv("LOGNAME", user.c_str(), 1);
  setenv("FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING", date.c_str(), 1);
  int nin = geti();
  for (int k = 0; k < nin; k++) {
    std::string name = geth(), bytes = geth();
    FILE *fp = fopen(name.c_str(), "wb");
    if (!fp) fail("an input file");
    fwrite(bytes.data(), 1, bytes.size(), fp);
    fclose(fp);
  }
  errno = 0;
  int nv = geti(), nf = geti();
  MRIS *m = MRISalloc(nv, nf);
  for (int vno = 0; vno < nv; vno++) {
    VERTEX *v = &m->vertices[vno];
    v->x = getf(); v->y = getf(); v->z = getf();
  }
  for (int fno = 0; fno < nf; fno++) for (int k = 0; k < 3; k++) m->faces[fno].v[k] = geti();
  m->useRealRAS = geti();
  set_vg(&m->vg);
  int tr = geti();
  if (tr) {
    int rows = geti(), cols = geti();
    m->SRASToTalSRAS_ = MatrixAlloc(rows, cols, MATRIX_REAL);
    for (int r = 1; r <= rows; r++) for (int c = 1; c <= cols; c++) *MATRIX_RELT(m->SRASToTalSRAS_, r, c) = getf();
  }
  m->group_avg_surface_area = getf();
  int nl = geti();
  for (int i = 0; i < nl; i++) {
    std::string l = geth();
    m->cmdlines[i] = (char *)calloc(l.size() + 1, 1);
    memcpy(m->cmdlines[i], l.data(), l.size());
  }
  m->ncmds = geti();
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    if (op == "rt" || op == "rq") {
      const char *nm = name_tok();
      double mult = getd();
      int kn = geti();
      std::vector<int> nfl;
      for (int j = 0; j < kn; j++) nfl.push_back(geti());
      MRIS *s = op == "rt" ? fssurfio_read_triangle_file(nm, mult) : fssurfio_read_quadrangle_file(nm, mult);
      dump_tri_surface(s, nfl);
    }
    else if (op == "wt") oi(MRISwriteTriangularSurface(m, name_tok()));
    else if (op == "wq") oi(fssurfio_write_quadrangle_file(m, name_tok()));
    else fail("tri op");
  }
  out += "| ";
  std::vector<std::string> files;
  list_files(".", "", files);
  std::sort(files.begin(), files.end());
  for (const std::string &name : files) {
    oh(name.c_str());
    FILE *fp = fopen(name.c_str(), "rb");
    if (!fp) { out += "NOFILE "; continue; }
    std::string b;
    int ch;
    while ((ch = fgetc(fp)) != EOF) b += (char)ch;
    fclose(fp);
    ox(b);
  }
}

void run_case()
{
  std::string cmd = next();
  if (cmd == "coord") {
    int which = geti();
    VERTEX v;
    memset(&v, 0, sizeof v);
    set_coords(&v);
    float x = 1, y = 2, z = 3;
    MRISvertexCoord2XYZ_float(&v, which, &x, &y, &z);
    of(x); of(y); of(z);
    double dx = 1, dy = 2, dz = 3;
    MRISvertexCoord2XYZ_double(&v, which, &dx, &dy, &dz);
    od(dx); od(dy); od(dz);
    // the MRIS views' which_coords, at three of their levels
    MRIS *m = MRISalloc(1, 0);
    m->vertices[0] = v;
    float a, b, c;
    SurfaceFromMRIS::XYZPositionConsequences::Surface(m).vertices(0).which_coords(which, &a, &b, &c);
    of(a); of(b); of(c);
    SurfaceFromMRIS::Topology::Surface(m).vertices(0).which_coords(which, &a, &b, &c);
    of(a); of(b); of(c);
    SurfaceFromMRIS::AllM::Surface(m).vertices(0).which_coords(which, &a, &b, &c);
    of(a); of(b); of(c);
  } else if (cmd == "vnum") {
    int i = geti();
    VERTEX_TOPOLOGY vt;
    memset(&vt, 0, sizeof vt);
    vt.vnum = (short)geti(); vt.v2num = (short)geti(); vt.v3num = (short)geti();
    oi(VERTEXvnum(&vt, i));
  } else if (cmd == "nbr") {
    int nv = geti(), vno1 = geti(), vno2 = geti(), hasv = geti(), vnum = geti();
    MRIS *m = MRISalloc(nv > 0 ? nv : 1, 0);
    m->nvertices = nv;
    if (0 <= vno1 && vno1 < (nv > 0 ? nv : 1)) {
      VERTEX_TOPOLOGY *vt = &m->vertices_topology[vno1];
      free(vt->v);
      vt->v = NULL;
      if (hasv) {
        vt->v = arr<int>(vnum);
        for (int k = 0; k < vnum; k++) vt->v[k] = geti();
      }
      vt->vnum = (short)vnum;
    }
    oi(mrisVertexNeighborIndex(m, vno1, vno2));
    oi(mrisVerticesAreNeighbors(m, vno1, vno2));
    if (hasv || vnum <= 0) oi(vertexNeighbor(m, vno1, vno2));
  } else if (cmd == "modvnum") {
    if (geti()) Gdiag |= DIAG_VERBOSE;
    MRIS *m = MRISalloc(3, 0);
    int vno = geti();
    m->vertices_topology[vno].vnum = (short)geti();
    int n = geti();
    for (int k = 0; k < n; k++) {
      std::string op = next();
      short a = (short)geti();
      if (op == "S") oi(setVnum(m, vno, a));
      else if (op == "C") oi(clearVnum(m, vno));
      else if (op == "A") oi(vnumAdd(m, vno, a));
      else if (op == "D") oi(addVnum(m, vno, a));
      else fail("modvnum op");
    }
    oi(m->vertices_topology[vno].vnum);
  } else if (cmd == "norm") {
    float v[3] = {getf(), getf(), getf()};
    of(mrisNormalize(v));
    of(v[0]); of(v[1]); of(v[2]);
  } else if (cmd == "close") {
    float a = getf(), b = getf();
    oi(closeEnough(a, b));
  } else if (cmd == "proj") {
    float cx = getf(), cy = getf(), cz = getf(), r = getf();
    float x, y, z;
    oi(project_point_onto_sphere(cx, cy, cz, r, &x, &y, &z));
    of(x); of(y); of(z);
  } else if (cmd == "sign") {
    float f = getf();
    double d = getd();
    od(sign(f)); od(sign(d));
  } else if (cmd == "status") {
    if (geti()) Gdiag |= DIAG_VERBOSE;
    MRIS_Status s1 = (MRIS_Status)geti(), s2 = (MRIS_Status)geti();
    out += MRIS_Status_text(s1); out += " ";
    oi(MRIS_Status_distanceFormula(s1));
    oi(areCompatible(s1, s2));
    checkOrigXYZCompatible(s1, s2);
    out += MRIS_Status_text(s2);
  } else if (cmd == "sortcmp") {
    float a = getf(), b = getf();
    oi(mris_sort_compare_float(&a, &b));
  } else if (cmd == "angles") {
    angles_per_triangle_t a, b;
    for (int i = 0; i < 3; i++) a[i] = getf();
    for (int i = 0; i < 3; i++) b[i] = getf();
    oi(cmpAnglesPerTriangle(a, b));
    copyAnglesPerTriangle(a, b);
    of(a[0]); of(a[1]); of(a[2]);
    oi(cmpAnglesPerTriangle(a, b));
  } else if (cmd == "split") {
    int a = geti(), b = geti();
    oi(WHICH_FACE_SPLIT(a, b));
  } else if (cmd == "vec3") {
    // the tokens read in order (a function's arguments are evaluated in no fixed order)
    double w[7];
    for (int k = 0; k < 7; k++) w[k] = getd();
    Vec3 u(w[0], w[1], w[2]), v(w[3], w[4], w[5]);
    double s = w[6];
    Vec3 p = u + v, q = u - v, r = u * s, t = s * v, c = cross(u, v);
    od(p[0]); od(p[1]); od(p[2]); od(q[0]); od(q[1]); od(q[2]);
    od(u * v); od(r[0]); od(r[1]); od(r[2]); od(t[0]); od(t[1]); od(t[2]);
    of(dot(u, v)); od(c[0]); od(c[1]); od(c[2]);
  } else if (cmd == "view") {
    view_case();
  } else if (cmd == "fixarea") {
    oi(MRISgetFixVertexAreaValue());
    oi(MRISsetFixVertexAreaValue(geti()));
    oi(MRISgetFixVertexAreaValue());
  } else if (cmd == "addcmd") {
    // ncmds set to n0 (the earlier lines left NULL), then one MRISaddCommandLine
    MRIS *m = MRISalloc(1, 0);
    int n0 = geti();
    std::string line = geth();
    m->ncmds = n0;
    int r = MRISaddCommandLine(m, line);
    oi(r);
    if (n0 < MAX_CMDS) { oi(m->ncmds); oh(m->cmdlines[n0]); }
  } else if (cmd == "vhash") {
    MRIS *m = MRISalloc(2, 0);
    int vno = geti();
    VERTEX_TOPOLOGY *vt = &m->vertices_topology[vno];
    VERTEX *v = &m->vertices[vno];
    free(vt->v); vt->v = NULL; free(vt->f); vt->f = NULL; free(vt->n); vt->n = NULL;
    vt->vnum = (short)geti(); vt->v2num = (short)geti(); vt->v3num = (short)geti();
    vt->vtotal = (short)geti(); vt->nsizeMaxClock = (short)geti();
    vt->nsizeMax = (uchar)geti(); vt->nsizeCur = (uchar)geti(); vt->num = (uchar)geti();
    int nv = geti();
    if (nv >= 0) { vt->v = arr<int>(nv); for (int k = 0; k < nv; k++) vt->v[k] = geti(); }
    int nf = geti();
    if (nf >= 0) {
      vt->f = arr<int>(nf); vt->n = arr<uchar>(nf);
      for (int k = 0; k < nf; k++) { vt->f[k] = geti(); vt->n[k] = (uchar)geti(); }
    }
    for (float **d : {&v->dist, &v->dist_orig}) {
      int nd = geti();
      if (nd >= 0) { *d = arr<float>(nd); for (int k = 0; k < nd; k++) (*d)[k] = getf(); }
      else *d = NULL;
    }
    // values outside the neighbourhood, which do not enter the hash
    set_coords(v);
    v->val = getf(); v->curv = getf(); v->ripflag = (char)geti(); v->marked = (short)geti();
    oi(mrisVertexVSize(m, vno));
    MRIS_HASH h;
    mrisVertexHash(&h, m, vno);
    out += std::to_string((unsigned long)h.hash) + " ";
  } else if (cmd == "diff") {
    Gdiag_no = geti();
    int nv1 = geti(), nv2 = geti(), nf1 = geti(), nf2 = geti();
    MRIS *a = MRISalloc(nv1, nf1), *b = MRISalloc(nv2, nf2);
    int nv = nv1 < nv2 ? nv1 : nv2;
    for (int k = 0; k < nv; k++) {
      for (MRIS *m : {a, b}) {
        m->vertices_topology[k].vnum = (short)geti();
        VERTEX *v = &m->vertices[k];
        v->ripflag = (char)geti();
        v->x = getf(); v->y = getf(); v->z = getf();
        v->val = getf();
      }
    }
    int ndiffmin = geti();
    double rmsthresh = getd();
    int verbosity = geti();
    oi(MRISdiffSimple(a, b, ndiffmin, rmsthresh, verbosity));
    for (int k = 0; k < nv; k++) of(a->vertices[k].val);
  } else if (cmd == "temps") {
    MRIS *m = MRISalloc(1, 0);
    m->tempsAssigned = geti();
    int n = geti();
    for (int k = 0; k < n; k++) {
      std::string op = next();
      MRIS_TempAssigned t = (MRIS_TempAssigned)geti();
      if (op == "A") oi(MRIS_acquireTemp(m, t));
      else if (op == "C") MRIS_checkAcquiredTemp(m, t, 123);
      else if (op == "R") MRIS_releaseTemp(m, t, 123);
      else fail("temps op");
      oi(m->tempsAssigned);
    }
  } else if (cmd == "fpv") {
    MRIS *m = MRISalloc(1, 0);
    m->nvertices = geti();
    m->nverticesFrozen = geti();
    int n = geti();
    std::vector<float *> ps;
    for (int k = 0; k < n; k++) { ps.push_back(MRISmakeFloatPerVertex(m)); oi(ps.back() != NULL); oi(m->nverticesFrozen); }
    for (float *&p : ps) { MRISfreeFloatPerVertex(m, &p); oi(p != NULL); oi(m->nverticesFrozen); }
  } else if (cmd == "surf") {
    surf_case();
  } else if (cmd == "metric") {
    metric_case();
  } else if (cmd == "posn") {
    posn_case();
  } else if (cmd == "life") {
    life_case();
  } else if (cmd == "meta") {
    meta_case();
  } else if (cmd == "misc") {
    misc_case();
  } else if (cmd == "edges") {
    edges_case();
  } else if (cmd == "mprop") {
    mprop_case();
  } else if (cmd == "sfile") {
    sfile_case();
  } else if (cmd == "cfile") {
    cfile_case();
  } else if (cmd == "sread") {
    sread_case();
  } else if (cmd == "stl") {
    stl_case();
  } else if (cmd == "tri") {
    tri_case();
  } else if (cmd == "fnam") {
    fnam_case();
  } else if (cmd == "swr") {
    swr_case();
  } else if (cmd == "names") {
    int n = geti();
    keep.reserve(n);
    for (int k = 0; k < n; k++) {
      std::string op = next();
      keep.push_back(geth());
      char *p = &keep.back()[0];
      if (op == "I") oi(MRISsetInflatedFileName(p));
      else if (op == "S") oi(MRISsetSulcFileName(p));
      else if (op == "O") oi(MRISsetOriginalFileName(p));
      else fail("names op");
    }
    for (int k = 0; k < 3; k++) { oh(mrisurf_surface_names[k]); oh(curvature_names[k]); }
    od(NEG_AREA_K);
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

#ifdef PROBE_MP_STUB
// The stub binary's MRIScomputeMetricProperties (linked with --allow-multiple-definition, so
// the library's own calls reach it): it marks the call and computes nothing.
int MRIScomputeMetricProperties(MRIS *mris)
{
  (void)mris;
  out += "MP ";
  return 0;
}
#endif

int main(int argc, char *argv[])
{
  if (argc != 2) {
    fprintf(stderr, "usage: fssurfio <cases>\n");
    return 2;
  }
  if (argv[1][0] != '/') fail("the cases must be given by an absolute path");
  std::string scratch = std::string(argv[1]) + ".out";
  workdir = std::string(argv[1]) + ".d";
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
  clear_dir(workdir, false);
  return 0;
}
