// SPDX-License-Identifier: Apache-2.0
// fs.surfio's probe: FreeSurfer 8.2.0's surface structure helpers (mrisurf_aaa.h, mrisurf.h,
// mrisurf_base.h, mrisurf_topology.h, mrisurf_sphere_interp.h, mrisurf_base.cpp's status texts,
// modVnum, MRISvertexCoord2XYZ_float/_double) and the MRIS and MRIS_MP views' accessors; since
// relay 6 also mrisurf.cpp's MRISaddCommandLine, mrisVertexHash (with mrisVertexVSize) and
// MRISdiffSimple, and mrisurf_base.cpp's temporaries, per-vertex floats and file-name setters;
// called case by case, linked with the unit oracle's libutils as the official programs are
// (build.sh).
// CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: it names FreeSurfer's functions (running it is a
// black-box run of FreeSurfer's code).
//
//     fssurfio <cases>     one output line per case line (absolute path; see check.py)
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

#include "diag.h"
#include "error.h"
#include "mrisurf_metricProperties.h"
#include "mrisurf_MRIS.h"
#include "mrisurf_MRIS_MP.h"
#include "mrisurf_SurfaceFromMRIS_generated.h"
#include "mrisurf_SurfaceFromMRIS_MP_generated.h"
#include "mrisurf_base.h"
#include "mrisurf_sphere_interp.h"

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

int main(int argc, char *argv[])
{
  if (argc != 2) {
    fprintf(stderr, "usage: fssurfio <cases>\n");
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
