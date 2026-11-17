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

#include <zlib.h>

#include "diag.h"
#include "gifti.h"
#include "error.h"
#include "mrisurf_metricProperties.h"
#include "mrisurf_MRIS.h"
#include "mrisurf_MRIS_MP.h"
#include "mrisurf_SurfaceFromMRIS_generated.h"
#include "mrisurf_SurfaceFromMRIS_MP_generated.h"
#include "mrisurf_base.h"
#include "mrisurf_sphere_interp.h"
#include "matrix.h"
#include "mrisurf.h"
#include "mrisutils.h"
#include "surfgrad.h"
#include "transform.h"
#include "topo_parms.h"
#include "tritri.h"
#include "label.h"
#include "surfcluster.h"
#include "mrishash.h"
#include "mrishash_internals.h"
#include "mrisurf_vals.h"
#include "annotation.h"

// sclustReMap0 is defined in surfcluster.cpp but not declared in its header (relay 38)
int sclustReMap0(MRI_SURFACE *Surf, int nClusters, SCS *scs_sorted);
// MRIScopyValuesToVal2Bak likewise (mrisurf_vals.cpp)
int MRIScopyValuesToVal2Bak(MRI_SURFACE *mris);

// fastmarching.h's declaration (the header redefines mrisurf_metricProperties.h's macros)
void MRISextractOutsideDistanceMap(MRIS *mris, MRI *mri_src, int label, int offset, float resolution,
                                   float max_distance);

// declared in utils/ only (mrisurf_deform.h) or nowhere (the MRIS_MP overload), defined in libutils
int mrisFlipPatch(MRIS *mris);
void MRISrotate(MRIS_MP *mris, float alpha, float beta, float gamma);
int mrisCountCompressed(MRIS *mris, double min_dist);
void mrisDumpVertex(FILE *file, MRIS const *mris, int vno);
int signum_eval(float af);
int mrisOrientSurface(MRIS *mris);
int mrisOrientPlane(MRIS *mris);
int mrisTrackTotalDistance(MRIS *mris);
int mrisTrackTotalDistanceNew(MRIS *mris);
// mrisurf_io.cpp's functions of queue item 3.1-48 that no header declares, and its file-local
// ones through io_statics.cpp (which builds mrisurf_io.cpp into the probe)
MATRIX *getSRASToTalSRAS(LT *lt);
int mrisComputeOptimalPlane(MRIS *mris, int *vertices, int nvertices, double *pxn, double *pyn,
                            double *pzn, double *pxc, double *pyc, double *pzc);
int mrisCalculateCanonicalFaceCentroid(MRIS *mris, int fno, float *px, float *py, float *pz);
int fssurfio_read_curvature_into_array(const char *fname, int size, float **out);
int fssurfio_read_new_curvature_into_array(const char *fname, int size, float **out);
int fssurfio_read_curv_old_format(MRIS *mris, const char *fname, MRI *curvmri);
void fssurfio_apply_fsgii_read(char *file_to_read, const char *fname, int *filetype);
void fssurfio_apply_fsgii_write(char *file_to_write, const char *fname, int *filetype);
int fssurfio_read_values_into_array(const char *sname, int size, float **out);
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

// relay 19: the obsolete H sign has no declaration in FreeSurfer's headers
int MRIS_Hcurvature_determineSign(MRIS *apmris);
// relay 20: declared in mrisurf_sseTerms.h (not included here)
int mrisAverageSignedGradients(MRIS *mris, int num_avgs);
// relay 42 of track D (top): declared in mrisurf_topology.cpp or utils/mrisurf_deform.h only
int mrisRemoveFace(MRIS *mris, int fno);
int mrisRemoveVertexLink(MRIS *mris, int vno1, int vno2);
int mrisStoreVtotalInV3num(MRIS *mris);
void MRISgetNeighborsBeginEnd(MRIS const *mris, int vno, size_t inner_nbhd_size, size_t outer_nbhd_size,
                              size_t *neighborsIndexBegin, size_t *neighborsIndexEnd);

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

// relay 16: MRISread (with MRISreadOverAlloc, mrisReadTransform's search, the icosahedron reader and
// fs.surf's neighbour lists, neighbourhoods and distances) and MRISfastRead, in an empty work folder
// (with a subfolder d): Gdiag's bits, the environment (a mask: mrisCompleteTopology_new,
// MRISfindNeighborsAtVertex_new, MRISfindNeighborsAtVertex_old, TRIANGULARSURFACE_NOEXTRA_READ,
// FS_SKIP_TAGS, FS_GII=.gii, FREESURFER_always_check_vv, FS_FASTER_MP; the licence check's side door
// always set), the case's input files (with folders d and mri/transforms, where a case may put the
// subject's talairach.xfm for the surfaces in d), 1 or 2 reads (`r <name> <dotkr>`: MRISread; `f <name>`:
// MRISfastRead), each surface read printed in full: its counts, type, room, hemisphere, name,
// coordinate spaces, sizes, distances' state, mean neighbour count, radius, group-average flag,
// transform, volume geometry, box, metric totals; per vertex its coordinates, normal, area, original
// area, curvature, stored positions, border, rip flag, mark, face list with places, neighbour
// counts and sizes, neighbour list and distances; per face its vertices, rip flag, area, angles and
// cached normal
void dump_srd_surface(MRIS *s)
{
  if (!s) { out += "NULL "; return; }
  oi(s->nvertices); oi(s->nfaces); oi(s->type); oi(s->max_vertices); oi(s->max_faces);
  oi(s->hemisphere); oh(s->fname.data()); oi(s->useRealRAS); oi(s->orig_xyzspace);
  oi(s->patch); oi(s->status); oi(s->nsize); oi(s->max_nsize); oi(s->dist_nsize);
  oi(s->dist_orig_nsize); oi(s->dist_alloced_flags); of(s->avg_nbrs); od(s->radius);
  oi(s->group_avg_vtxarea_loaded); oi(s->lta != NULL); oi(s->free_transform);
  if (s->lta) {
    // relay 17: the Talairach transform MRISread found (its type, count, first transform's type,
    // source and destination geometries and matrix) and the two matrices cached from it
    oi(s->lta->type); oi(s->lta->num_xforms);
    LT *lt = &s->lta->xforms[0];
    oi(lt->type);
    dump_vg(&lt->src);
    dump_vg(&lt->dst);
    for (MATRIX *mt : {lt->m_L, s->SRASToTalSRAS_, s->TalSRASToSRAS_}) {
      if (!mt) { out += "NULL "; continue; }
      for (int r = 1; r <= 4; r++)
        for (int c = 1; c <= 4; c++) of(*MATRIX_RELT(mt, r, c));
    }
  }
  dump_vg(&s->vg);
  float b[10] = {s->xlo, s->ylo, s->zlo, s->xhi, s->yhi, s->zhi, s->xctr, s->yctr, s->zctr,
                 s->total_area};
  for (float x : b) of(x);
  od(s->avg_vertex_area); od(s->avg_vertex_dist); od(s->std_vertex_dist);
  of(s->neg_area); of(s->neg_orig_area);
  for (int vno = 0; vno < s->nvertices; vno++) {
    VERTEX *v = &s->vertices[vno];
    VERTEX_TOPOLOGY *vt = &s->vertices_topology[vno];
    float c[12] = {v->x, v->y, v->z, v->nx, v->ny, v->nz, v->area, v->origarea, v->curv,
                   v->tx, v->ty, v->tz};
    for (float x : c) of(x);
    oi(v->border); oi(v->ripflag); oi(v->marked);
    oi(vt->num);
    for (int k = 0; k < vt->num; k++) { oi(vt->f[k]); oi(vt->n[k]); }
    oi(vt->vnum); oi(vt->v2num); oi(vt->v3num); oi(vt->vtotal); oi(vt->nsizeMax);
    oi(vt->nsizeCur);
    int vs = mrisVertexVSize(s, vno);
    oi(vs);
    for (int k = 0; k < vs; k++) oi(vt->v[k]);
    for (int o = 0; o < 2; o++) {
      float *d = o ? v->dist_orig : v->dist;
      int cap = o ? v->dist_orig_capacity : v->dist_capacity;
      oi(d != NULL);
      if (d) for (int k = 0; k < vs && k < cap; k++) of(d[k]);
    }
  }
  for (int fno = 0; fno < s->nfaces; fno++) {
    FACE *f = &s->faces[fno];
    oi(f->v[0]); oi(f->v[1]); oi(f->v[2]); oi(f->ripflag);
    of(f->area);
    for (int k = 0; k < 3; k++) of(f->angle[k]);
    FaceNormCacheEntry *e = &s->faceNormCacheEntries[fno];
    of(e->nx); of(e->ny); of(e->nz); of(e->orig_area);
    oi(s->faceNormDeferredEntries[fno].deferred);
  }
}

void srd_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  mkdir("mri", 0700);
  mkdir("mri/transforms", 0700);
  int gd = geti();
  if (gd & 1) Gdiag |= DIAG_SHOW;
  if (gd & 2) Gdiag |= DIAG_VERBOSE;
  int env = geti();
  if (env & 1) setenv("mrisCompleteTopology_new", "1", 1);
  if (env & 2) setenv("MRISfindNeighborsAtVertex_new", "1", 1);
  if (env & 4) setenv("MRISfindNeighborsAtVertex_old", "1", 1);
  if (env & 8) setenv("TRIANGULARSURFACE_NOEXTRA_READ", "1", 1);
  if (env & 16) setenv("FS_SKIP_TAGS", "1", 1);
  if (env & 32) setenv("FS_GII", ".gii", 1);
  if (env & 64) setenv("FREESURFER_always_check_vv", "1", 1);
  if (env & 128) setenv("FS_FASTER_MP", "1", 1);
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
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    if (op == "r") {
      const char *nm = name_tok();
      bool dotkr = geti() != 0;
      dump_srd_surface(MRISread(nm, dotkr));
    }
    else if (op == "f") dump_srd_surface(MRISfastRead(name_tok()));
    else fail("srd op");
  }
}


// ---- relay 21: gii, the GIFTI surface reader (gifti.cpp's mrisReadGIFTIdanum) -------------------

// A surface as gii prints it: counts, type, hemisphere, name, real-RAS flag, volume geometry,
// centre, group-average area, command lines, the Talairach matrix, the colour table (each entry's
// name and colours; no alpha for a colour array's table), per vertex its coordinates, curvature, val, stat, annotation, dx dy dz and
// face count, per face its vertices
void dump_gii_surface(MRIS *s)
{
  oi(s->nvertices); oi(s->nfaces); oi(s->type); oi(s->hemisphere); oh(s->fname.data());
  oi(s->useRealRAS);
  dump_vg(&s->vg);
  of(s->xctr); of(s->yctr); of(s->zctr); of(s->group_avg_surface_area);
  oi(s->ncmds);
  for (int k = 0; k < s->ncmds; k++) oh(s->cmdlines[k]);
  if (!s->SRASToTalSRAS_) out += "NULL ";
  else
    for (int r = 1; r <= 4; r++)
      for (int c = 1; c <= 4; c++) of(*MATRIX_RELT(s->SRASToTalSRAS_, r, c));
  COLOR_TABLE *ct = s->ct;
  if (!ct) out += "NULL ";
  else {
    oi(ct->nentries); oi(ct->version); oh(ct->fname);
    for (int k = 0; k < ct->nentries; k++) {
      CTE *e = ct->entries[k];
      if (!e) { out += "- "; continue; }
      // a colour array's table (makeColorTable's Unknown_Label_<k>): its alpha is memory
      // contents (FS-393), not printed
      bool alpha = strncmp(e->name, "Unknown_Label_", 14) != 0;
      oh(e->name); oi(e->ri); oi(e->gi); oi(e->bi); if (alpha) oi(e->ai);
      of(e->rf); of(e->gf); of(e->bf); if (alpha) of(e->af);
    }
  }
  for (int vno = 0; vno < s->nvertices; vno++) {
    VERTEX *v = &s->vertices[vno];
    float c[9] = {v->x, v->y, v->z, v->curv, v->val, v->stat, v->dx, v->dy, v->dz};
    for (float x : c) of(x);
    oi(v->annotation); oi(s->vertices_topology[vno].num);
  }
  for (int fno = 0; fno < s->nfaces; fno++) {
    FACE *f = &s->faces[fno];
    oi(f->v[0]); oi(f->v[1]); oi(f->v[2]);
  }
}

// gii: Gdiag's bits; the case's input files; a surface kept for the reads that pass one (made by
// MRISalloc with <n> vertices and no faces, or none); a colour table for the reads that give one
// (its entries: present or not, name, r g b a); 1 to 3 reads `<name> <daNum> <flags>` (flags 1:
// pass the kept surface, 2: ask for the overlays, 4: give the colour table), each printed as
// NULL, PASSED or NEW with the surface, then the overlays (name, type, intent, shape data type,
// format, values per vertex, the volume's size and its first slice's values)
void gii_case()
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
  int nkeep = geti();
  MRIS *keep = nkeep > 0 ? MRISalloc(nkeep, 0) : NULL;
  int nct = geti();
  COLOR_TABLE *ctab = NULL;
  if (nct >= 0) {
    ctab = (COLOR_TABLE *)calloc(1, sizeof(COLOR_TABLE));
    ctab->nentries = nct;
    ctab->version = 2;
    ctab->entries = (COLOR_TABLE_ENTRY **)calloc(nct + 1, sizeof(COLOR_TABLE_ENTRY *));
    strcpy(ctab->fname, "user.ctab");
    for (int k = 0; k < nct; k++) {
      if (!geti()) continue;
      CTE *e = (CTE *)calloc(1, sizeof(CTE));
      std::string name = geth();
      strncpy(e->name, name.c_str(), sizeof(e->name) - 1);
      e->ri = geti(); e->gi = geti(); e->bi = geti(); e->ai = geti();
      e->rf = e->ri / 255.0f; e->gf = e->gi / 255.0f; e->bf = e->bi / 255.0f; e->af = e->ai / 255.0f;
      ctab->entries[k] = e;
    }
  }
  errno = 0;
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    const char *nm = name_tok();
    int danum = geti();
    int flags = geti();
    std::vector<OverlayInfoStruct> ov;
    MRIS *in = (flags & 1) ? keep : NULL;
    MRIS *r = mrisReadGIFTIfile(nm, in, (flags & 2) ? &ov : NULL, danum, (flags & 4) ? ctab : NULL);
    if (!r) out += "NULL ";
    else if (r == in) { out += "PASSED "; dump_gii_surface(r); }
    else { out += "NEW "; dump_gii_surface(r); }
    if (in) { out += "KEPT "; dump_gii_surface(in); }
    oi((long long)ov.size());
    for (OverlayInfoStruct &o : ov) {
      if (o.__foverlay) oh(o.__foverlay); else out += "NULL ";
      oi(o.__type); oi(o.__giftiIntent); oh(o.__shapedatatype); oi(o.__format);
      oi(o.__nValsPerVertex);
      MRI *m = o.__overlaymri;
      if (!m) { out += "NULL "; continue; }
      oi(m->width); oi(m->height); oi(m->depth); oi(m->nframes);
      for (int x = 0; x < m->width; x++) of(MRIgetVoxVal(m, x, 0, 0, 0));
    }
  }
}


// ---- relay 22: gwr, the GIFTI writers and GIFTI files as volumes ---------------------------------

// a written GIFTI file as both sides print it: the UserName's value written U, each <Data>
// payload Base64-decoded and inflated (zlib), written Z and its bytes in hex (B and the payload
// itself when it does not inflate): the compressed bytes are zlib 1.2.11's here and zlib-rs's
// in the port, the values the same
int b64val(unsigned char c)
{
  if (c >= 'A' && c <= 'Z') return c - 'A';
  if (c >= 'a' && c <= 'z') return c - 'a' + 26;
  if (c >= '0' && c <= '9') return c - '0' + 52;
  if (c == '+') return 62;
  if (c == '/') return 63;
  return -1;
}

std::string gii_normal(const std::string &f)
{
  std::string s = f;
  size_t u = s.find("<![CDATA[UserName]]></Name>");
  if (u != std::string::npos) {
    size_t v = s.find("<Value><![CDATA[", u);
    if (v != std::string::npos) {
      v += 16;
      size_t e = s.find("]]>", v);
      if (e != std::string::npos) s.replace(v, e - v, "U");
    }
  }
  std::string r;
  size_t p = 0;
  char h[4];
  for (;;) {
    size_t a = s.find("<Data>", p);
    size_t b = a == std::string::npos ? a : s.find("</Data>", a + 6);
    if (b == std::string::npos) { r += s.substr(p); break; }
    a += 6;
    r += s.substr(p, a - p);
    std::string pay = s.substr(a, b - a), bin;
    unsigned acc = 0;
    int nb = 0;
    for (unsigned char c : pay) {
      int v = b64val(c);
      if (v < 0) continue;
      acc = (acc << 6) | (unsigned)v;
      nb += 6;
      if (nb >= 8) { nb -= 8; bin.push_back((char)((acc >> nb) & 0xff)); }
    }
    std::string inf;
    bool ok = false;
    z_stream zs;
    memset(&zs, 0, sizeof zs);
    if (inflateInit(&zs) == Z_OK) {
      zs.next_in = (Bytef *)bin.data();
      zs.avail_in = (uInt)bin.size();
      unsigned char buf[4096];
      int ret;
      do {
        zs.next_out = buf;
        zs.avail_out = sizeof buf;
        ret = inflate(&zs, Z_NO_FLUSH);
        inf.append((char *)buf, sizeof buf - zs.avail_out);
      } while (ret == Z_OK);
      ok = ret == Z_STREAM_END;
      inflateEnd(&zs);
    }
    if (ok) {
      r += "Z";
      for (unsigned char c : inf) { snprintf(h, sizeof h, "%02x", c); r += h; }
    } else {
      r += "B" + pay;
    }
    p = b;
  }
  return r;
}

void dump_gii_file(const char *name)
{
  FILE *fp = fopen(name, "rb");
  if (!fp) { out += "NOFILE "; return; }
  std::string b;
  char buf[4096];
  size_t n;
  while ((n = fread(buf, 1, sizeof buf, fp)) > 0) b.append(buf, n);
  fclose(fp);
  ox(gii_normal(b));
}

void dump_ctab(COLOR_TABLE *ct)
{
  if (!ct) { out += "NULL "; return; }
  oi(ct->nentries); oi(ct->version); oh(ct->fname);
  for (int k = 0; k < ct->nentries; k++) {
    CTE *e = ct->entries[k];
    if (!e) { out += "- "; continue; }
    oh(e->name); oi(e->ri); oi(e->gi); oi(e->bi); oi(e->ai);
    of(e->rf); of(e->gf); of(e->bf); of(e->af);
  }
}

void dump_gwr_mri(MRI *m, int values)
{
  oi(m->width); oi(m->height); oi(m->depth); oi(m->nframes); oi(m->type); oi(m->intent);
  oi(m->version); of(m->tr);
  dump_ctab(m->ct);
  if (!values) return;
  for (int f = 0; f < m->nframes; f++)
    for (int s = 0; s < m->depth; s++)
      for (int r = 0; r < m->height; r++)
        for (int c = 0; c < m->width; c++) of(MRIgetVoxVal(m, c, r, s, f));
}

const char *opt_name()
{
  if (!geti()) return NULL;
  return name_tok();
}

// gwr: Gdiag's bits (1 show, 2 verbose), the creation time's replacement, input files (GIFTI
// files, a subject's talairach.xfm), a surface from MRISalloc (per vertex its coordinates,
// curvature, stat and annotation; the faces' vertices; its name, subject, patch flag, real RAS,
// volume geometry, centre, group-average area, command lines, Talairach matrix), a colour table or
// none, volumes (type, extent, frames, repetition time, values); then 1 to 4 of MRISwriteGIFTI,
// its volume overload, MRISwriteGIFTICombined, mriWriteGifti, getShapeStatIntentCount,
// MRISreadGiftiAsMRI and MRISwrite of a .gii name; each writer's file printed (gii_normal)
void gwr_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  mkdir("mri", 0700);
  mkdir("mri/transforms", 0700);
  int gd = geti();
  if (gd & 1) Gdiag |= DIAG_SHOW;
  if (gd & 2) Gdiag |= DIAG_VERBOSE;
  std::string date = geth();
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
  int nv = geti(), nf = geti();
  MRIS *m = MRISalloc(nv, nf);
  for (int vno = 0; vno < nv; vno++) {
    VERTEX *v = &m->vertices[vno];
    v->x = getf(); v->y = getf(); v->z = getf(); v->curv = getf(); v->stat = getf();
    v->annotation = geti();
  }
  for (int fno = 0; fno < nf; fno++) for (int k = 0; k < 3; k++) m->faces[fno].v[k] = geti();
  std::string fname = geth(), subj = geth();
  snprintf(m->fname.data(), STRLEN, "%s", fname.c_str());
  snprintf(m->subject_name.data(), STRLEN, "%s", subj.c_str());
  m->patch = geti();
  m->useRealRAS = geti();
  set_vg(&m->vg);
  m->xctr = getf(); m->yctr = getf(); m->zctr = getf();
  m->group_avg_surface_area = getf();
  int nl = geti();
  for (int i = 0; i < nl; i++) {
    std::string l = geth();
    m->cmdlines[i] = (char *)calloc(l.size() + 1, 1);
    memcpy(m->cmdlines[i], l.data(), l.size());
  }
  m->ncmds = nl;
  if (geti()) {
    int rows = geti(), cols = geti();
    m->SRASToTalSRAS_ = MatrixAlloc(rows, cols, MATRIX_REAL);
    for (int r = 1; r <= rows; r++) for (int c = 1; c <= cols; c++) *MATRIX_RELT(m->SRASToTalSRAS_, r, c) = getf();
  }
  int nct = geti();
  if (nct >= 0) {
    COLOR_TABLE *ctab = (COLOR_TABLE *)calloc(1, sizeof(COLOR_TABLE));
    ctab->nentries = nct;
    ctab->version = 2;
    ctab->entries = (COLOR_TABLE_ENTRY **)calloc(nct + 1, sizeof(COLOR_TABLE_ENTRY *));
    strcpy(ctab->fname, "user.ctab");
    for (int k = 0; k < nct; k++) {
      if (!geti()) continue;
      CTE *e = (CTE *)calloc(1, sizeof(CTE));
      std::string name = geth();
      strncpy(e->name, name.c_str(), sizeof(e->name) - 1);
      e->ri = geti(); e->gi = geti(); e->bi = geti(); e->ai = geti();
      e->rf = e->ri / 255.0f; e->gf = e->gi / 255.0f; e->bf = e->bi / 255.0f; e->af = e->ai / 255.0f;
      ctab->entries[k] = e;
    }
    m->ct = ctab;
  }
  int nvol = geti();
  std::vector<MRI *> vols;
  for (int k = 0; k < nvol; k++) {
    int type = geti(), w = geti(), h = geti(), d = geti(), fr = geti();
    MRI *v = MRIallocSequence(w, h, d, type, fr);
    v->tr = getf();
    for (int f = 0; f < fr; f++)
      for (int s = 0; s < d; s++)
        for (int r = 0; r < h; r++)
          for (int c = 0; c < w; c++) MRIsetVoxVal(v, c, r, s, f, getf());
    vols.push_back(v);
  }
  errno = 0;
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    out += op + " ";
    fflush(stdout);
    if (op == "w") {
      int intent = geti();
      const char *nm = name_tok();
      const char *cf = opt_name();
      oi(MRISwriteGIFTI(m, intent, nm, cf));
      fflush(stdout);
      dump_gii_file(nm);
    } else if (op == "wm") {
      int intent = geti(), vol = geti();
      const char *nm = name_tok();
      const char *cf = opt_name();
      const char *dt = opt_name();
      oi(MRISwriteGIFTI(m, vols[vol], intent, nm, cf, dt));
      fflush(stdout);
      dump_gii_file(nm);
    } else if (op == "c") {
      const char *nm = name_tok();
      int nov = geti();
      std::vector<OverlayInfoStruct> ov(nov);
      for (OverlayInfoStruct &o : ov) {
        o.__type = geti();
        o.__giftiIntent = geti();
        int vol = geti();
        o.__overlaymri = vol >= 0 ? vols[vol] : NULL;
        o.__foverlay = (char *)opt_name();
        std::string sd = geth();
        memset(o.__shapedatatype, 0, sizeof o.__shapedatatype);
        snprintf(o.__shapedatatype, sizeof o.__shapedatatype, "%s", sd.c_str());
        o.__format = GIFTI_FILE;
      }
      oi(MRISwriteGIFTICombined(m, &ov, nm));
      fflush(stdout);
      dump_gii_file(nm);
    } else if (op == "mw") {
      int vol = geti();
      const char *nm = name_tok();
      oi(mriWriteGifti(vols[vol], nm));
      fflush(stdout);
      dump_gii_file(nm);
    } else if (op == "sw") {
      const char *nm = name_tok();
      oi(MRISwrite(m, nm));
      fflush(stdout);
      dump_gii_file(nm);
    } else if (op == "n") {
      const char *nm = name_tok();
      int a = 7, b = 7;
      oi(getShapeStatIntentCount(nm, &a, &b));
      oi(a); oi(b);
    } else if (op == "r") {
      const char *nm = name_tok();
      int rv = geti();
      MRI *v = MRISreadGiftiAsMRI(nm, rv);
      fflush(stdout);
      if (!v) out += "NULL ";
      else dump_gwr_mri(v, rv);
    } else {
      fail("gwr op");
    }
  }
}


// ---- relay 17: mpb, the metric properties of 3.1-50 part b ----------------------------------------

void dump_mpb_vertices(MRIS *s)
{
  of(s->min_curv); of(s->max_curv);
  for (int vno = 0; vno < s->nvertices; vno++) {
    VERTEX *v = &s->vertices[vno];
    float c[13] = {v->curv, v->e1x, v->e1y, v->e1z, v->e2x, v->e2y, v->e2z,
                   v->pe1x, v->pe1y, v->pe1z, v->pe2x, v->pe2y, v->pe2z};
    for (float x : c) of(x);
  }
}

// mpb: Gdiag's bits (1 show, 2 verbose, 4 write), the environment (1: FS_FASTER_MP), a surface file
// read by MRISread (d/lh.s0), then its original area or not, for each vertex an override mask (1 ripped, 2 original, 4 white,
// 8 pial, 16 target, 32 canonical, 64 curvature, 128 normal) with its values, for each face a mask
// (1 ripped, 2 area) with its value, then the operations, each printing what it gives and, for the
// curvature edits and the tangent planes, every vertex's curvature and planes
void mpb_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  int gd = geti();
  if (gd & 1) Gdiag |= DIAG_SHOW;
  if (gd & 2) Gdiag |= DIAG_VERBOSE;
  if (gd & 4) Gdiag |= DIAG_WRITE;
  int env = geti();
  if (env & 1) setenv("FS_FASTER_MP", "1", 1);
  setenv("SURFER_SIDEDOOR", "1", 1);
  std::string bytes = geth();
  FILE *fp = fopen("d/lh.s0", "wb");
  if (!fp) fail("the surface file");
  fwrite(bytes.data(), 1, bytes.size(), fp);
  fclose(fp);
  errno = 0;
  MRIS *s = MRISread("d/lh.s0");
  if (!s) { out += "NULL "; return; }
  int nv = geti(), nf = geti();
  if (nv != s->nvertices || nf != s->nfaces) { oi(s->nvertices); oi(s->nfaces); out += "COUNTS "; return; }
  if (geti()) s->orig_area = getf();
  for (int vno = 0; vno < nv; vno++) {
    VERTEX *v = &s->vertices[vno];
    int m = geti();
    if (m & 1) v->ripflag = 1;
    if (m & 2) { v->origx = getf(); v->origy = getf(); v->origz = getf(); }
    if (m & 4) { v->whitex = getf(); v->whitey = getf(); v->whitez = getf(); }
    if (m & 8) { v->pialx = getf(); v->pialy = getf(); v->pialz = getf(); }
    if (m & 16) { v->targx = getf(); v->targy = getf(); v->targz = getf(); }
    if (m & 32) { v->cx = getf(); v->cy = getf(); v->cz = getf(); }
    if (m & 64) v->curv = getf();
    if (m & 128) { v->nx = getf(); v->ny = getf(); v->nz = getf(); }
  }
  for (int fno = 0; fno < nf; fno++) {
    int m = geti();
    if (m & 1) s->faces[fno].ripflag = 1;
    if (m & 2) s->faces[fno].area = getf();
  }
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    out += op + " ";
    if (op == "vc") {
      int a = geti(), b = geti();
      VECTOR *vv = VectorAlloc(3, MATRIX_REAL);
      oi(VERTICES_commonInFaces_find(&s->faces[a], &s->faces[b], vv));
      for (int i = 1; i <= 3; i++) of(VECTOR_ELT(vv, i));
      VectorFree(&vv);
    } else if (op == "ce") {
      int a = geti(), b = geti();
      of(FACES_commonEdgeLength_find(s, &s->faces[a], &s->faces[b]));
    } else if (op == "tp") {
      oi(mrisComputeTangentPlanes(s));
      dump_mpb_vertices(s);
    } else if (op == "st") {
      oi(MRISstoreTangentPlanes(s, geti()));
      dump_mpb_vertices(s);
    } else if (op == "op") {
      int n = geti();
      std::vector<int> list(n > 0 ? n : 1);
      for (int i = 0; i < n; i++) list[i] = geti();
      double r[6] = {7, 7, 7, 7, 7, 7};
      oi(mrisComputeOptimalPlane(s, list.data(), n, &r[0], &r[1], &r[2], &r[3], &r[4], &r[5]));
      for (double x : r) od(x);
    } else if (op == "fc") {
      int f = geti(), which = geti();
      float x = 7, y = 7, z = 7;
      if (which) mrisCalculateCanonicalFaceCentroid(s, f, &x, &y, &z);
      else mrisCalculateFaceCentroid(s, f, &x, &y, &z);
      of(x); of(y); of(z);
    } else if (op == "fa") {
      double sg = 7, mn = 7, mx = 7;
      od(MRIScomputeFaceAreaStats(s, &sg, &mn, &mx)); od(sg); od(mn); od(mx);
    } else if (op == "vs" || op == "ts") {
      double sg = 7, mn = 7, mx = 7;
      int a = 7, b = 7;
      if (op == "vs") od(MRIScomputeVertexSpacingStats(s, &sg, &mn, &mx, &a, &b, geti()));
      else od(MRIScomputeTotalVertexSpacingStats(s, &sg, &mn, &mx, &a, &b));
      od(sg); od(mn); od(mx); oi(a); oi(b);
    } else if (op == "nf") {
      oi(MRIScountNegativeFaces(s)); oi(MRIScountNegativeTriangles(s));
    } else if (op == "pt") {
      fflush(stdout);
      oi(MRISprintTessellationStats(s, stdout));
      fflush(stdout);
    } else if (op == "cv") {
      float x = getf(), y = getf(), z = getf(), d = 7;
      oi(MRISfindClosestVertex(s, x, y, z, &d, geti())); of(d);
    } else if (op == "cl") {
      float x = getf(), y = getf(), z = getf();
      oi(MRISfindClosestOriginalVertex(s, x, y, z));
      oi(MRISfindClosestCanonicalVertex(s, x, y, z));
      oi(MRISfindClosestWhiteVertex(s, x, y, z));
    } else {
      if (op == "sc") MRISsetCurvature(s, getf());
      else if (op == "tr") MRIStrinarizeCurvature(s, getf());
      else if (op == "sl") MRISscaleCurvature(s, getf());
      else if (op == "bi") {
        float t = getf(), lo = getf(), hi = getf();
        MRISbinarizeCurvature(s, t, lo, hi, geti());
      }
      else if (op == "th") { float t = getf(); MRISthresholdCurvature(s, t, geti()); }
      else if (op == "nz") { int d = geti(); fflush(stdout); MRISnormalizeField(s, d, geti()); fflush(stdout); }
      else if (op == "hc") { float t = getf(); fflush(stdout); MRIShistoThresholdCurvature(s, t); fflush(stdout); }
      else fail("mpb op");
      dump_mpb_vertices(s);
    }
  }
}

// ---- relay 18: mpc, the metric properties of 3.1-50 part b, second group -----------------------

void dump_planes(MRIS *s)
{
  for (int vno = 0; vno < s->nvertices; vno++) {
    VERTEX *v = &s->vertices[vno];
    float c[6] = {v->e1x, v->e1y, v->e1z, v->e2x, v->e2y, v->e2z};
    for (float x : c) of(x);
  }
}

void dump_dist_orig(MRIS *s)
{
  for (int vno = 0; vno < s->nvertices; vno++)
    for (int n = 0; n < s->vertices_topology[vno].vtotal; n++) of(s->vertices[vno].dist_orig[n]);
}

void dump_normals(MRIS *s)
{
  for (int vno = 0; vno < s->nvertices; vno++) {
    VERTEX *v = &s->vertices[vno];
    float c[18] = {v->x, v->y, v->z, v->nx, v->ny, v->nz, v->wnx, v->wny, v->wnz,
                   v->pnx, v->pny, v->pnz, v->onx, v->ony, v->onz, v->tx, v->ty, v->tz};
    for (float x : c) of(x);
  }
}

void dump_marks(MRIS *s)
{
  for (int vno = 0; vno < s->nvertices; vno++) {
    VERTEX *v = &s->vertices[vno];
    oi(v->marked); of(v->x); of(v->y); of(v->z);
  }
}

void dump_curv(MRIS *s)
{
  of(s->min_curv); of(s->max_curv);
  for (int vno = 0; vno < s->nvertices; vno++) of(s->vertices[vno].curv);
}

// mpc: Gdiag's bits (1 show, 2 verbose, 4 write), FREESURFER_SEED, FS_DISTURB_DISTANCES or not, a
// surface file read by MRISread (d/lh.s0), the status (-1: the read's), its original area or not,
// for each vertex (after MRISmakeDistOrig) a mask (1 ripped, 2 original, 4 canonical, 8 val, 16
// val2, 32 k1, 64 k2, 128 normal, 256 pial, 512 white, 1024 original distances, 2048 distances: a
// count and that many numbers, those past vtotal dropped) with its values, for each face its rip
// flag, then the operations, each printing what it gives
void mpc_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  int gd = geti();
  if (gd & 1) Gdiag |= DIAG_SHOW;
  if (gd & 2) Gdiag |= DIAG_VERBOSE;
  if (gd & 4) Gdiag |= DIAG_WRITE;
  setenv("SURFER_SIDEDOOR", "1", 1);
  setenv("FREESURFER_SEED", std::to_string(geti()).c_str(), 1);
  if (geti()) setenv("FS_DISTURB_DISTANCES", next(), 1);
  std::string bytes = geth();
  FILE *fp = fopen("d/lh.s0", "wb");
  if (!fp) fail("the surface file");
  fwrite(bytes.data(), 1, bytes.size(), fp);
  fclose(fp);
  errno = 0;
  MRIS *s = MRISread("d/lh.s0");
  if (!s) { out += "NULL "; return; }
  int nv = geti(), nf = geti();
  if (nv != s->nvertices || nf != s->nfaces) { oi(s->nvertices); oi(s->nfaces); out += "COUNTS "; return; }
  int status = geti();
  if (status >= 0) s->status = (MRIS_Status)status;
  if (geti()) s->orig_area = getf();
  for (int vno = 0; vno < nv; vno++) {
    MRISmakeDistOrig(s, vno);
    int vtotal = s->vertices_topology[vno].vtotal;
    VERTEX *v = &s->vertices[vno];
    int m = geti();
    if (m & 1) v->ripflag = 1;
    if (m & 2) { v->origx = getf(); v->origy = getf(); v->origz = getf(); }
    if (m & 4) { v->cx = getf(); v->cy = getf(); v->cz = getf(); }
    if (m & 8) v->val = getf();
    if (m & 16) v->val2 = getf();
    if (m & 32) v->k1 = getf();
    if (m & 64) v->k2 = getf();
    if (m & 128) { v->nx = getf(); v->ny = getf(); v->nz = getf(); }
    if (m & 256) { v->pialx = getf(); v->pialy = getf(); v->pialz = getf(); }
    if (m & 512) { v->whitex = getf(); v->whitey = getf(); v->whitez = getf(); }
    if (m & 4096) { v->x = getf(); v->y = getf(); v->z = getf(); }
    if (m & 8192) { v->wnx = getf(); v->wny = getf(); v->wnz = getf(); }
    if (m & 16384) v->marked = geti();
    if (m & 32768) { v->dx = getf(); v->dy = getf(); v->dz = getf(); }
    if (m & 65536) { v->odx = getf(); v->ody = getf(); v->odz = getf(); }
    if (m & 131072) v->area = getf();
    if (m & 262144) v->fixedval = 1;
    for (int bit : {1024, 2048}) {
      if (m & bit) {
        int k = geti();
        for (int j = 0; j < k; j++) {
          float x = getf();
          if (j < vtotal) {
            if (bit == 1024) v->dist_orig[j] = x;
            else v->dist[j] = x;
          }
        }
      }
    }
  }
  for (int fno = 0; fno < nf; fno++) {
    int m = geti();
    if (m & 1) s->faces[fno].ripflag = 1;
    if (m & 2) s->faces[fno].area = getf();
  }
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    out += op + " ";
    if (op == "fb") {
      int f = geti(), w = geti();
      double x = getd(), y = getd(), z = getd(), l[3] = {7, 7, 7};
      oi(face_barycentric_coords(s, f, w, x, y, z, &l[0], &l[1], &l[2]));
      for (double d : l) od(d);
    } else if (op == "fr" || op == "pp") {
      double c[4][3];
      for (int i = 0; i < 4; i++)
        for (int j = 0; j < 3; j++) c[i][j] = getd();
      if (op == "fr") {
        double l[3] = {7, 7, 7};
        oi(face_barycentric_coords(c[0], c[1], c[2], c[3][0], c[3][1], c[3][2], &l[0], &l[1], &l[2]));
        for (double d : l) od(d);
      } else {
        double proj[3] = {7, 7, 7}, e1[3] = {7, 7, 7}, e2[3] = {7, 7, 7};
        oi(project_point_to_plane(c[3], c[0], c[1], c[2], proj, e1, e2));
        for (double d : proj) od(d);
        for (double d : e1) od(d);
        for (double d : e2) od(d);
        for (double d : c[3]) od(d);
      }
    } else if (op == "sc") {
      int f = geti();
      double x = getd(), y = getd(), z = getd();
      int wc = geti(), wb = geti();
      float px = 7, py = 7, pz = 7;
      oi(MRISsampleFaceCoords(s, f, x, y, z, wc, wb, &px, &py, &pz));
      of(px); of(py); of(pz);
    } else if (op == "sf") {
      int f = geti(), w = geti();
      double x = getd(), y = getd(), z = getd(), a = getd(), b = getd(), c = getd();
      od(MRISsampleFace(s, f, w, x, y, z, a, b, c));
    } else if (op == "sn") {
      int f = geti();
      double x = getd(), y = getd(), z = getd();
      float px = 7, py = 7, pz = 7;
      MRISsampleFaceNormal(s, f, x, y, z, &px, &py, &pz);
      of(px); of(py); of(pz);
    } else if (op == "rh") {
      od(MRISrmsTPHeight(s));
      dump_planes(s);
    } else if (op == "ae") {
      int w = geti(), withfp = geti();
      fflush(stdout);
      double pct = MRIScomputeAnalyticDistanceError(s, w, withfp ? stdout : NULL);
      fflush(stdout);
      od(pct);
    } else if (op == "sa") {
      od(MRISstoreAnalyticDistances(s, geti()));
      dump_dist_orig(s);
    } else if (op == "dd") {
      double pct = getd();
      fflush(stdout);
      oi(MRISdisturbOriginalDistances(s, pct));
      fflush(stdout);
      dump_dist_orig(s);
    } else if (op == "hd") {
      od(MRIScomputeHausdorffDistance(s, geti()));
    } else if (op == "ur" || op == "uc") {
      oi(op == "ur" ? MRISuseCurvatureRatio(s) : MRISuseCurvatureContrast(s));
      dump_curv(s);
    } else if (op == "md") {
      double p[4][3];
      for (int i = 0; i < 4; i++)
        for (int j = 0; j < 3; j++) p[i][j] = getd();
      double dl = getd(), pmin[3] = {7, 7, 7};
      od(MinDistToTriangleBF(p[0], p[1], p[2], p[3], pmin, dl));
      for (double d : pmin) od(d);
    } else if (op == "sv") {
      double p[3] = {7, 7, 7};
      StuffVertexCoords(s, geti(), p);
      for (double d : p) od(d);
    } else if (op == "tt") {
      if (geti()) oi(mrisTrackTotalDistanceNew(s));
      else oi(mrisTrackTotalDistance(s));
      dump_curv(s);
    } else if (op == "tv") {
      int d = geti();
      fflush(stdout);
      double r = d ? MRIStotalVariationDifference(s) : MRIStotalVariation(s);
      fflush(stdout);
      od(r);
    } else if (op == "ke") {
      od(MRIScurvatureError(s, getd()));
    } else if (op == "uk") {
      int kind = geti();
      fflush(stdout);
      if (kind == 0) oi(MRISuseCurvatureDifference(s));
      else if (kind == 1) oi(MRISuseCurvatureMax(s));
      else if (kind == 2) oi(MRISuseCurvatureMin(s));
      else if (kind == 3) oi(MRISuseCurvatureStretch(s));
      else oi(MRISuseNegCurvature(s));
      fflush(stdout);
      dump_curv(s);
      oi(errno);
    } else if (op == "ca") {
      int a = geti(), b = geti(), c = geti(), m = geti();
      oi(containsAnotherVertexOnSphere(s, a, b, c, m));
      dump_marks(s);
    } else if (op == "cn") {
      int w = geti(), vno = geti();
      double n[3] = {7, 7, 7};
      oi(MRIScomputeNormal(s, w, vno, &n[0], &n[1], &n[2]));
      for (double d : n) od(d);
    } else if (op == "cs") {
      int set = geti(), vno = geti();
      float n[3] = {7, 7, 7};
      if (set == 0) oi(mrisComputeOrigNormal(s, vno, n));
      else if (set == 1) oi(mrisComputeWhiteNormal(s, vno, n));
      else oi(mrisComputePialNormal(s, vno, n));
      for (float x : n) of(x);
    } else if (op == "su") {
      int w = geti(), navgs = geti();
      fflush(stdout);
      oi(MRIScomputeSurfaceNormals(s, w, navgs));
      fflush(stdout);
      dump_normals(s);
    } else if (op == "co" || op == "cp" || op == "ct") {
      int nbhd = geti();
      fflush(stdout);
      if (op == "co") oi(MRISfindClosestOrigVertices(s, nbhd));
      else if (op == "cp") oi(MRISfindClosestPialVerticesCanonicalCoords(s, nbhd));
      else { float mt = getf(); oi(MRISmeasureCorticalThickness(s, nbhd, mt)); }
      fflush(stdout);
      dump_curv(s);
      dump_marks(s);
    } else if (op == "sfc") {
      int f = geti(), c = geti();
      double p[3] = {7, 7, 7};
      StuffFaceCoords(s, f, c, p);
      for (double d : p) od(d);
    } else fail("mpc op");
  }
}


// ---- relay 19: mpd, the discrete curvatures of 3.1-50 part b and computeVertexPseudoNormal ------

void dump_kh(MRIS *s)
{
  of(s->Kmin); of(s->Kmax); of(s->Hmin); of(s->Hmax); od(s->Ktotal);
  for (int vno = 0; vno < s->nvertices; vno++) {
    VERTEX *v = &s->vertices[vno];
    of(v->K); of(v->H); of(v->k1); of(v->k2); oi(v->marked);
  }
}

void dump_order(MRIS *s)
{
  for (int vno = 0; vno < s->nvertices; vno++) {
    VERTEX_TOPOLOGY *vt = &s->vertices_topology[vno];
    oi(s->vertices[vno].marked);
    for (int n = 0; n < vt->num; n++) { oi(vt->f[n]); oi(vt->n[n]); }
  }
}

// mpd: Gdiag's bits (1 show, 2 verbose), the environment (1: BEVIN_TESTING_dumpFacesAroundVertex),
// a surface file read by MRISread (d/lh.s0), for each vertex a mask (1 ripped, 2 marked: a number,
// 4 current position, 8 normal, 16 original position, 32 K and H, 64 area) with its values, for
// each face a mask (1 ripped, 2 area, 4 angles, 8 cached normal) with its values, then the
// operations, each printing what it gives
void mpd_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  int gd = geti();
  if (gd & 1) Gdiag |= DIAG_SHOW;
  if (gd & 2) Gdiag |= DIAG_VERBOSE;
  if (geti() & 1) setenv("BEVIN_TESTING_dumpFacesAroundVertex", "1", 1);
  setenv("SURFER_SIDEDOOR", "1", 1);
  std::string bytes = geth();
  FILE *fp = fopen("d/lh.s0", "wb");
  if (!fp) fail("the surface file");
  fwrite(bytes.data(), 1, bytes.size(), fp);
  fclose(fp);
  errno = 0;
  MRIS *s = MRISread("d/lh.s0");
  if (!s) { out += "NULL "; return; }
  int nv = geti(), nf = geti();
  if (nv != s->nvertices || nf != s->nfaces) { oi(s->nvertices); oi(s->nfaces); out += "COUNTS "; return; }
  for (int vno = 0; vno < nv; vno++) {
    VERTEX *v = &s->vertices[vno];
    int m = geti();
    if (m & 1) v->ripflag = 1;
    if (m & 2) v->marked = geti();
    if (m & 4) { v->x = getf(); v->y = getf(); v->z = getf(); }
    if (m & 8) { v->nx = getf(); v->ny = getf(); v->nz = getf(); }
    if (m & 16) { v->origx = getf(); v->origy = getf(); v->origz = getf(); }
    if (m & 32) { v->K = getf(); v->H = getf(); }
    if (m & 64) v->area = getf();
  }
  for (int fno = 0; fno < nf; fno++) {
    int m = geti();
    if (m & 1) s->faces[fno].ripflag = 1;
    if (m & 2) s->faces[fno].area = getf();
    if (m & 4) for (int k = 0; k < 3; k++) s->faces[fno].angle[k] = getf();
    if (m & 8) { float a = getf(), b = getf(), c = getf(); setFaceNorm(s, fno, a, b, c); }
  }
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    out += op + " ";
    if (op == "sff") {
      int sg = geti();
      oi(MRIScomputeSecondFundamentalFormDiscrete(s, sg));
      dump_kh(s);
      dump_order(s);
    } else if (op == "fvr") {
      oi(MRIS_facesAtVertices_reorder(s));
      dump_order(s);
    } else if (op == "khc") {
      oi(MRIS_discreteKH_compute(s));
      dump_kh(s);
    } else if (op == "k12") {
      int sg = geti();
      oi(MRIS_discretek1k2_compute(s, sg));
      dump_kh(s);
    } else if (op == "hcs") {
      int a = geti(), b = geti();
      oi(FACES_Hcurvature_determineSign(s, a, b));
    } else if (op == "anf") {
      int a = geti(), b = geti();
      of(FACES_angleNormal_find(s, a, b));
    } else if (op == "vfa") {
      int vno = geti(), n = s->vertices_topology[vno].num;
      VECTOR *a = VectorAlloc(n > 0 ? n : 1, MATRIX_REAL);
      oi(VERTEX_faceAngles_determine(s, vno, a));
      for (int i = 0; i < n; i++) of(VECTOR_ELT(a, i + 1));
      VectorFree(&a);
    } else if (op == "vmm") {
      int vno = geti(), mi = -7, ma = -7;
      float mn = 7, mx = 7;
      oi(VERTEX_faceMinMaxAngles_determine(s, vno, &mi, &mn, &ma, &mx));
      oi(mi); of(mn); oi(ma); of(mx);
    } else if (op == "mhs") {
      oi(MRIS_Hcurvature_determineSign(s));
      for (int vno = 0; vno < s->nvertices; vno++) oi(s->vertices[vno].undefval);
    } else if (op == "vpn") {
      int vno = geti(), verbose = geti();
      float n[3] = {7, 7, 7};
      computeVertexPseudoNormal(s, vno, n, verbose);
      for (float x : n) of(x);
    } else if (op == "fvi") {
      int f = geti(), vno = geti();
      oi(FACE_vertexIndex_find(&s->faces[f], vno));
    } else if (op == "fvm") {
      int a = geti(), b = geti();
      VECTOR *c = VectorAlloc(3, MATRIX_REAL);
      oi(VERTICES_commonInFaces_find(&s->faces[a], &s->faces[b], c));
      oi(FACE_vertexIndexAtMask_find(&s->faces[a], c));
      VectorFree(&c);
    } else if (op == "dil") {
      oi(MRISdilateMarked(s, geti()));
      for (int vno = 0; vno < s->nvertices; vno++) { oi(s->vertices[vno].marked); of(s->vertices[vno].tx); }
    } else if (op == "clm") {
      oi(MRISclearMarks(s));
      oi(MRIScountAllMarked(s));
    } else fail("mpd op");
  }
}

// ---- relay 20: mpe, the rest of 3.1-51 (mrisurf_metricProperties.cpp 9395-13617) ----------------

// the probe's functions of k1 and k2 for MRISusePrincipalCurvatureFunction
float upf0(float k1, float k2) { (void)k2; return k1 + 1.0f; }
float upf1(float k1, float k2) { (void)k1; return k2 * 0.5f; }
float upf2(float k1, float k2) { return k1 > k2 ? k1 : k2; }

// a file's length and FNV-1a hash (NOFILE when there is none), the file then removed
void file_hash(const char *name)
{
  FILE *f = fopen(name, "rb");
  if (!f) { out += "NOFILE "; return; }
  uint64_t h = 14695981039346656037ULL;
  long n = 0;
  int c;
  while ((c = fgetc(f)) != EOF) { h ^= (unsigned char)c; h *= 1099511628211ULL; n++; }
  fclose(f);
  char b[48];
  snprintf(b, sizeof b, "%ld:%016" PRIx64 " ", n, h);
  out += b;
  remove(name);
}

// a vertex's distances to its first vnum neighbours, or N when it has none
void dump_dist_n(MRIS *s, int vno)
{
  VERTEX *v = &s->vertices[vno];
  if (!v->dist) { out += "N "; return; }
  for (int k = 0; k < s->vertices_topology[vno].vnum; k++) of(v->dist[k]);
}

void dump_curv_only(MRIS *s)
{
  of(s->min_curv); of(s->max_curv);
  for (int vno = 0; vno < s->nvertices; vno++) of(s->vertices[vno].curv);
}

// mpe: Gdiag's bits (1 show, 2 verbose, 4 write), a surface file read by MRISread (d/lh.s0), the status
// (-1: the read's), Kmin, Kmax, Hmin, Hmax or not, for each vertex a mask (1 ripped, 2 marked: a
// number, 4 current position, 8 normal, 16 curv, 32 k1 and k2, 64 K and H, 128 area, 256 original
// position, 512 gradient) with its values, for each face a mask (1 ripped, 2 area, 4 cached
// original area, 8 cached normal) with its values, then the operations, each printing what it
// gives; the counts, status, extremes, Gdiag_no and FREESURFER_MRISaverageGradients_optimize_suppress
// come before the read
// relay 23: queue item 3.1-51's leftovers (MRIScrsLUT, MRISar1, MRISarN, MRISsmoothKernel,
// SetSurfHopList, mriSurfaceRASToVoxel) on a surface read by MRISread and volumes of one voxel per
// vertex; each operation prints its volume (or NULL) and the vertices' undefval
static void mpf_vol(MRI *m)
{
  if (!m) { out += "NULL "; return; }
  oi(m->nframes);
  for (int f = 0; f < m->nframes; f++)
    for (int s = 0; s < m->depth; s++)
      for (int r = 0; r < m->height; r++)
        for (int c = 0; c < m->width; c++) of(MRIgetVoxVal(m, c, r, s, f));
}

static void mpf_undef(MRIS *s)
{
  for (int vno = 0; vno < s->nvertices; vno++) oi(s->vertices[vno].undefval);
}

void mpf_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  setenv("SURFER_SIDEDOOR", "1", 1);
  std::string bytes = geth();
  int nv = geti(), nf = geti();
  int nrip = geti();
  std::vector<int> rips;
  for (int k = 0; k < nrip; k++) rips.push_back(geti());
  int flags = geti();
  if (flags & 1) setenv("FREESURFER_mriSurfaceRASToVoxel_both", "1", 1);
  if (flags & 2) setenv("FREESURFER_mriSurfaceRASToVoxel_old", "1", 1);
  FILE *fp = fopen("d/lh.s0", "wb");
  if (!fp) fail("the surface file");
  fwrite(bytes.data(), 1, bytes.size(), fp);
  fclose(fp);
  errno = 0;
  MRIS *s = MRISread("d/lh.s0");
  if (!s) { out += "NULL "; return; }
  if (nv != s->nvertices || nf != s->nfaces) { oi(s->nvertices); oi(s->nfaces); out += "COUNTS "; return; }
  for (int vno : rips) s->vertices[vno].ripflag = 1;
  int nvol = geti();
  std::vector<MRI *> vols;
  for (int k = 0; k < nvol; k++) {
    int type = geti(), w = geti(), h = geti(), d = geti(), fr = geti();
    MRI *v = MRIallocSequence(w, h, d, type, fr);
    for (int f = 0; f < fr; f++)
      for (int z = 0; z < d; z++)
        for (int r = 0; r < h; r++)
          for (int c = 0; c < w; c++) MRIsetVoxVal(v, c, r, z, f, getf());
    vols.push_back(v);
  }
  SURFHOPLIST **shl = NULL;
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    out += op + " ";
    fflush(stdout);
    if (op == "lut") {
      MRI *v = vols[geti()];
      int **lut = MRIScrsLUT(s, v);
      if (!lut) { out += "NULL "; continue; }
      int n = v->width * v->height * v->depth;
      for (int i = 0; i < n; i++) { oi(lut[0][i]); oi(lut[1][i]); oi(lut[2][i]); }
      oi(MRIScrsLUTFree(lut));
    } else if (op == "ar1") {
      int vi = geti(), mi = geti();
      mpf_vol(MRISar1(s, vols[vi], mi >= 0 ? vols[mi] : NULL, NULL));
    } else if (op == "arn") {
      int vi = geti(), mi = geti(), n = geti();
      mpf_vol(MRISarN(s, vols[vi], mi >= 0 ? vols[mi] : NULL, NULL, n));
      mpf_undef(s);
    } else if (op == "skn") {
      int vi = geti(), mi = geti(), kind = geti(), ki = geti();
      int rows = geti(), cols = geti();
      MATRIX *gk = MatrixAlloc(rows, cols, MATRIX_REAL);
      for (int r = 1; r <= rows; r++)
        for (int c = 1; c <= cols; c++) *MATRIX_RELT(gk, r, c) = getf();
      int reuse = geti(), ok = geti(), oi_ = geti();
      if (!reuse) shl = NULL;
      MRI *outv = ok == 0 ? NULL : ok == 1 ? vols[oi_] : vols[vi];
      MRI *r = MRISsmoothKernel(s, vols[vi], mi >= 0 ? vols[mi] : NULL, (kind & 1) ? vols[ki] : NULL,
                                (kind & 2) ? gk : NULL, &shl, outv);
      mpf_vol(r);
      mpf_undef(s);
    } else if (op == "hop") {
      int vtx = geti(), n = geti();
      SURFHOPLIST *l = SetSurfHopList(vtx, s, n);
      oi(l->cvtx);
      oi(l->nhops);
      for (int h = 0; h < l->nhops; h++) {
        oi(l->nperhop[h]);
        for (int i = 0; i < l->nperhop[h]; i++) oi(l->vtxlist[h][i]);
      }
      oi(SurfHopListFree(&l));
      mpf_undef(s);
    } else if (op == "sras") {
      VoxelFromSRASmatrix = NULL;
      if (geti()) {
        int rows = geti(), cols = geti();
        VoxelFromSRASmatrix = MatrixAlloc(rows, cols, MATRIX_REAL);
        for (int r = 1; r <= rows; r++)
          for (int c = 1; c <= cols; c++) *MATRIX_RELT(VoxelFromSRASmatrix, r, c) = getf();
      }
      int np = geti();
      for (int i = 0; i < np; i++) {
        double x = getd(), y = getd(), z = getd(), xv = 0, yv = 0, zv = 0;
        oi(mriSurfaceRASToVoxel(x, y, z, &xv, &yv, &zv));
        od(xv); od(yv); od(zv);
      }
    } else {
      fail("mpf op");
    }
  }
}

// relay 24: queue item 3.1-51's leftovers, second group: the edge and corner tables, the face,
// edge and corner metrics (no gradients), the quality statistics and printers, the overlay of the
// largest edge statistic, MRISaverageCurvatures and mrisFindMiddleOfGray, on a surface read by
// MRISread with some vertices ripped, curvatures given or not, a mask of one voxel per vertex (or
// none) and MRISfaceNormalFace's test delta
static void mpg_stats(double *st)
{
  if (!st) { out += "NULL "; return; }
  for (int k = 0; k < 5; k++) od(st[k]);
  free(st);
}

static void mpg_edges(MRIS *s)
{
  oi(s->nedges);
  if (!s->edges) { out += "N "; return; }
  for (int e = 0; e < s->nedges; e++) {
    MRI_EDGE *x = &s->edges[e];
    for (int k = 0; k < 4; k++) oi(x->vtxno[k]);
    oi(x->faceno[0]); oi(x->faceno[1]);
    for (int k = 0; k < 4; k++) { oi(x->corner[k][0]); oi(x->corner[k][1]); }
  }
  for (int vno = 0; vno < s->nvertices; vno++) {
    VERTEX_TOPOLOGY *vt = &s->vertices_topology[vno];
    if (!vt->e) { out += "N "; continue; }
    for (int k = 0; k < vt->vtotal; k++) oi(vt->e[k]);
  }
}

void mpg_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  setenv("SURFER_SIDEDOOR", "1", 1);
  std::string bytes = geth();
  int nv = geti(), nf = geti();
  int nrip = geti();
  std::vector<int> rips;
  for (int k = 0; k < nrip; k++) rips.push_back(geti());
  FILE *fp = fopen("d/lh.s0", "wb");
  if (!fp) fail("the surface file");
  fwrite(bytes.data(), 1, bytes.size(), fp);
  fclose(fp);
  errno = 0;
  MRIS *s = MRISread("d/lh.s0");
  if (!s) { out += "NULL "; return; }
  if (nv != s->nvertices || nf != s->nfaces) { oi(s->nvertices); oi(s->nfaces); out += "COUNTS "; return; }
  for (int vno : rips) s->vertices[vno].ripflag = 1;
  if (geti())
    for (int vno = 0; vno < s->nvertices; vno++) s->vertices[vno].curv = getf();
  MRI *mask = NULL;
  int mw = geti();
  if (mw > 0) {
    mask = MRIalloc(mw, 1, 1, MRI_FLOAT);
    for (int c = 0; c < mw; c++) MRIsetVoxVal(mask, c, 0, 0, 0, getf());
  }
  MRISfaceNormalFace_AddDeltaVertex = geti();
  for (int k = 0; k < 3; k++) MRISfaceNormalFace_AddDelta[k] = getd();
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    out += op + " ";
    fflush(stdout);
    if (op == "cnt") {
      oi(MRIScountEdges(s));
    } else if (op == "edg") {
      oi(MRISedges(s));
      mpg_edges(s);
    } else if (op == "cor") {
      oi(MRIScorners(s));
      oi(s->ncorners);
      for (int c = 0; s->corners && c < s->ncorners; c++) {
        MRI_CORNER *x = &s->corners[c];
        oi(x->vtxno[0]); oi(x->vtxno[1]); oi(x->vtxno[2]); oi(x->faceno);
        oi(x->edgeno[0]); oi(x->edgeno[1]); oi(x->edgedir[0]); oi(x->edgedir[1]);
      }
    } else if (op == "fm") {
      oi(MRISfaceMetric(s, 0));
      for (int f = 0; f < s->nfaces; f++) {
        DMATRIX *n = s->faces[f].norm;
        if (!n) { out += "N "; continue; }
        od(n->rptr[1][1]); od(n->rptr[2][1]); od(n->rptr[3][1]);
      }
    } else if (op == "fnf") {
      int fno = geti(), want = geti();
      DMATRIX *c = NULL;
      double cl = 0;
      oi(MRISfaceNormalFace(s, fno, (want & 1) ? &c : NULL, (want & 2) ? &cl : NULL));
      if (c) { od(c->rptr[1][1]); od(c->rptr[2][1]); od(c->rptr[3][1]); }
      od(cl);
      if (fno >= 0 && fno < s->nfaces && s->faces[fno].norm) {
        DMATRIX *n = s->faces[fno].norm;
        od(n->rptr[1][1]); od(n->rptr[2][1]); od(n->rptr[3][1]);
      }
    } else if (op == "em") {
      if (!s->edges) MRISedges(s);
      oi(MRISedgeMetric(s, 0));
      for (int e = 0; e < s->nedges; e++) {
        MRI_EDGE *x = &s->edges[e];
        od(x->len); od(x->u[0]); od(x->u[1]); od(x->u[2]);
        od(x->area[0]); od(x->area[1]); od(x->maxarea); od(x->dot); od(x->angle);
      }
    } else if (op == "cm") {
      oi(MRIScornerMetric(s, 0));
      for (int c = 0; s->corners && c < s->ncorners; c++) { od(s->corners[c].dot); od(s->corners[c].angle); }
    } else if (op == "ast") {
      int m = geti();
      mpg_stats(MRIStriangleAreaStats(s, m ? mask : NULL, NULL));
    } else if (op == "est") {
      int id = geti(), m = geti();
      mpg_stats(MRISedgeStats(s, id, m ? mask : NULL, NULL));
    } else if (op == "cst") {
      int id = geti(), m = geti();
      mpg_stats(MRIScornerStats(s, id, m ? mask : NULL, NULL));
    } else if (op == "ovl") {
      int id = geti(), m = geti();
      mpf_vol(MRISmaxEdgeStatToOverlay(s, id, m ? mask : NULL));
    } else if (op == "pq") {
      oi(MRISprintSurfQualityStats(stdout, s));
    } else if (op == "ppq") {
      oi(MRISprettyPrintSurfQualityStats(stdout, s));
    } else if (op == "avc") {
      oi(MRISaverageCurvatures(s, geti()));
      of(s->min_curv); of(s->max_curv);
      for (int vno = 0; vno < s->nvertices; vno++) of(s->vertices[vno].curv);
    } else if (op == "mid") {
      mrisFindMiddleOfGray(s);
      for (int vno = 0; vno < s->nvertices; vno++) {
        VERTEX *v = &s->vertices[vno];
        of(v->x); of(v->y); of(v->z); of(v->curv);
      }
    } else if (op == "mv") {
      int vno = geti();
      float x = getf(), y = getf(), z = getf();
      MRISsetXYZ(s, vno, x, y, z);
    } else if (op == "mp") {
      MRIScomputeMetricProperties(s);
    } else if (op == "mke") {
      // relay 33 of track D: MRISmarkEdge without its hole filling (FillHoles 0)
      int id = geti(), m = geti();
      double th = getd();
      oi(MRISmarkEdge(s, m ? mask : NULL, id, th, 0));
      for (int vno = 0; vno < s->nvertices; vno++) oi(s->vertices[vno].marked);
    } else {
      fail("mpg op");
    }
  }
}

void mpe_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  int gd = geti();
  if (gd & 1) Gdiag |= DIAG_SHOW;
  if (gd & 2) Gdiag |= DIAG_VERBOSE;
  if (gd & 4) Gdiag |= DIAG_WRITE;
  setenv("SURFER_SIDEDOOR", "1", 1);
  std::string bytes = geth();
  int nv = geti(), nf = geti(), status = geti();
  bool extremes = geti() != 0;
  float ext[4] = {0, 0, 0, 0};
  if (extremes) for (float &x : ext) x = getf();
  int gdiag_no = geti();
  if (geti()) setenv("FREESURFER_MRISaverageGradients_optimize_suppress", "1", 1);
  FILE *fp = fopen("d/lh.s0", "wb");
  if (!fp) fail("the surface file");
  fwrite(bytes.data(), 1, bytes.size(), fp);
  fclose(fp);
  errno = 0;
  MRIS *s = MRISread("d/lh.s0");
  if (!s) { out += "NULL "; return; }
  if (nv != s->nvertices || nf != s->nfaces) { oi(s->nvertices); oi(s->nfaces); out += "COUNTS "; return; }
  if (status >= 0) s->status = (MRIS_Status)status;
  if (extremes) { s->Kmin = ext[0]; s->Kmax = ext[1]; s->Hmin = ext[2]; s->Hmax = ext[3]; }
  Gdiag_no = gdiag_no;
  for (int vno = 0; vno < nv; vno++) {
    VERTEX *v = &s->vertices[vno];
    int m = geti();
    if (m & 1) v->ripflag = 1;
    if (m & 2) v->marked = geti();
    if (m & 4) { v->x = getf(); v->y = getf(); v->z = getf(); }
    if (m & 8) { v->nx = getf(); v->ny = getf(); v->nz = getf(); }
    if (m & 16) v->curv = getf();
    if (m & 32) { v->k1 = getf(); v->k2 = getf(); }
    if (m & 64) { v->K = getf(); v->H = getf(); }
    if (m & 128) v->area = getf();
    if (m & 256) { v->origx = getf(); v->origy = getf(); v->origz = getf(); }
    if (m & 512) { v->dx = getf(); v->dy = getf(); v->dz = getf(); }
    if (m & 1024) { v->val = getf(); v->val2 = getf(); v->origarea = getf(); }
  }
  for (int fno = 0; fno < nf; fno++) {
    int m = geti();
    if (m & 1) s->faces[fno].ripflag = 1;
    if (m & 2) s->faces[fno].area = getf();
    if (m & 4) setFaceOrigArea(s, fno, getf());
    if (m & 8) { float a = getf(), b = getf(), c = getf(); setFaceNorm(s, fno, a, b, c); }
  }
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    out += op + " ";
    if (op == "ssn") {
      oi(MRISsmoothSurfaceNormals(s, geti()));
      for (int vno = 0; vno < s->nvertices; vno++) {
        VERTEX *v = &s->vertices[vno];
        of(v->nx); of(v->ny); of(v->nz); of(v->tdx); of(v->tdy); of(v->tdz);
      }
    } else if (op == "cbn") {
      oi(mrisComputeBoundaryNormals(s));
    } else if (op == "cmc") {
      oi(MRIScomputeMeanCurvature(s));
      of(s->min_curv); of(s->max_curv);
      for (int vno = 0; vno < s->nvertices; vno++) {
        VERTEX *v = &s->vertices[vno];
        of(v->H); of(v->e1x); of(v->e1y); of(v->e1z); of(v->e2x); of(v->e2y); of(v->e2z);
      }
    } else if (op == "uae" || op == "ugc" || op == "umc" || op == "upc" || op == "uk1" || op == "uk2" || op == "upf") {
      int r;
      if (op == "uae") r = MRISuseAreaErrors(s);
      else if (op == "ugc") r = MRISuseGaussianCurvature(s);
      else if (op == "umc") r = MRISuseMeanCurvature(s);
      else if (op == "upc") r = MRISusePrincipalCurvature(s);
      else if (op == "uk1") r = MRISuseK1Curvature(s);
      else if (op == "uk2") r = MRISuseK2Curvature(s);
      else {
        int w = geti();
        r = MRISusePrincipalCurvatureFunction(s, w == 0 ? upf0 : w == 1 ? upf1 : upf2);
      }
      oi(r);
      dump_curv_only(s);
    } else if (op == "cci") {
      double ici = 7, fi = 7;
      MRIScomputeCurvatureIndices(s, &ici, &fi);
      od(ici); od(fi);
    } else if (op == "mxr") {
      od(MRISmaxRadius(s));
      of(s->xlo); of(s->ylo); of(s->zlo); of(s->xhi); of(s->yhi); of(s->zhi);
    } else if (op == "cac") {
      if (geti()) {
        double sigma = 7;
        od(MRIScomputeAverageCurvature(s, &sigma));
        od(sigma);
      } else {
        od(MRIScomputeAverageCurvature(s, NULL));
      }
      oi(errno);
    } else if (op == "mnv") {
      oi(MRISmarkNegativeVertices(s, geti()));
      for (int vno = 0; vno < s->nvertices; vno++) oi(s->vertices[vno].marked);
    } else if (op == "rnv") {
      oi(MRISripNegativeVertices(s));
      for (int vno = 0; vno < s->nvertices; vno++) oi(s->vertices[vno].ripflag);
      for (int fno = 0; fno < s->nfaces; fno++) oi(s->faces[fno].ripflag);
    } else if (op == "sff") {
      double pct = getd();
      oi(MRIScomputeSecondFundamentalFormThresholded(s, pct));
      of(s->Kmin); of(s->Kmax); of(s->Hmin); of(s->Hmax); od(s->Ktotal);
      for (int vno = 0; vno < s->nvertices; vno++) {
        VERTEX *v = &s->vertices[vno];
        float c[10] = {v->k1, v->k2, v->K, v->H, v->e1x, v->e1y, v->e1z, v->e2x, v->e2y, v->e2z};
        for (float x : c) of(x);
      }
      for (const char *name : {"curv.dat", "k1.plt", "k2.plt"}) file_hash(name);
    } else if (op == "agr" || op == "agf" || op == "ags") {
      int n = geti();
      oi(op == "agr" ? MRISaverageGradients(s, n) : op == "agf" ? MRISaverageGradientsFast(s, n) : mrisAverageSignedGradients(s, n));
      for (int vno = 0; vno < s->nvertices; vno++) {
        VERTEX *v = &s->vertices[vno];
        of(v->dx); of(v->dy); of(v->dz); of(v->tdx); of(v->tdy); of(v->tdz);
      }
    } else if (op == "rmp") {
      s->group_avg_surface_area = getf();
      od(MRISrescaleMetricProperties(s));
      of(s->total_area); od(s->avg_vertex_dist); od(s->std_vertex_dist); od(s->avg_vertex_area);
      for (int vno = 0; vno < s->nvertices; vno++) { of(s->vertices[vno].area); dump_dist_n(s, vno); }
      oi(errno);
    } else if (op == "nrm") {
      int dof = geti(), which = geti();
      oi(MRISnormalize(s, dof, which));
      for (int vno = 0; vno < s->nvertices; vno++) {
        VERTEX *v = &s->vertices[vno];
        of(v->origarea); of(v->curv); of(v->val); of(v->val2);
      }
    } else if (op == "ups") {
      oi(MRISupdateSurface(s));
      of(s->total_area); od(s->Ktotal);
      for (int vno = 0; vno < s->nvertices; vno++) {
        VERTEX *v = &s->vertices[vno];
        of(v->area); of(v->nx); of(v->k1); of(v->k2); of(v->K); of(v->H);
      }
    } else if (op == "moc") {
      oi(MRISmarkOrientationChanges(s));
      for (int vno = 0; vno < s->nvertices; vno++) of(s->vertices[vno].curv);
    } else if (op == "cks") {
      oi(mrisCheckSurface(s));
    } else if (op == "ssd") {
      oi(MRISsubsampleDist(s, getf()));
      for (int vno = 0; vno < s->nvertices; vno++) {
        VERTEX *v = &s->vertices[vno];
        of(v->d); of(v->val); oi(v->fixedval);
      }
    } else if (op == "xnb") {
      if (geti()) for (int vno = 0; vno < s->nvertices; vno++) s->vertices[vno].val2bak = -1;
      int targ = geti();
      double th = getd();
      int max = geti(), dist = geti();
      std::vector<int> nb(max > 0 ? max : 1);
      std::vector<double> dp(max > 0 ? max : 1);
      int n = -7;
      oi(MRISextendedNeighbors(s, targ, targ, th, nb.data(), dp.data(), &n, max, dist));
      oi(n);
      for (int k = 0; k < n; k++) { oi(nb[k]); od(dp[k]); }
      for (int vno = 0; vno < s->nvertices; vno++) of(s->vertices[vno].val2bak);
    } else if (op == "uni") {
      int mode = geti();
      MRIS *c = MRISclone(s);
      if (mode == 1) c->useRealRAS = !s->useRealRAS;
      else if (mode == 2) c->vg.c_r += 1;
      MRIS *u = MRISunion(s, c);
      if (!u) out += "NULL ";
      else {
        oi(u->nvertices); oi(u->nfaces); oi(u->useRealRAS);
        for (int vno = 0; vno < u->nvertices; vno++) { of(u->vertices[vno].x); of(u->vertices[vno].y); of(u->vertices[vno].z); }
        for (int fno = 0; fno < u->nfaces; fno++) for (int k = 0; k < 3; k++) oi(u->faces[fno].v[k]);
      }
    } else if (op == "cln") {
      MRIS *c = MRISclone(s);
      of(c->total_area); of(c->orig_area); of(c->min_curv); of(c->max_curv); of(c->xlo); of(c->zhi);
      oi(c->status); od(c->avg_vertex_dist);
      for (int vno = 0; vno < c->nvertices; vno++) {
        VERTEX *v = &c->vertices[vno];
        VERTEX_TOPOLOGY *vt = &c->vertices_topology[vno];
        float x[9] = {v->x, v->nx, v->cx, v->curv, v->area, v->origarea, v->origx, v->val, v->k1};
        for (float y : x) of(y);
        oi(v->ripflag); oi(vt->vnum); oi(vt->vtotal); oi(vt->num);
        for (int k = 0; k < vt->vtotal; k++) oi(vt->v[k]);
        dump_dist_n(c, vno);
      }
      for (int fno = 0; fno < c->nfaces; fno++) {
        FACE *f = &c->faces[fno];
        oi(f->v[0]); oi(f->v[1]); oi(f->v[2]); of(f->area); oi(f->ripflag);
      }
    } else if (op == "pvs") {
      int vno = geti(), which = geti();
      oi(MRISprintVertexStats(s, vno, stdout, which));
    } else if (op == "pvi") {
      oi(MRISprintVertexInfo(stdout, s, geti()));
    } else if (op == "sfa") {
      if (geti()) oi(mrisComputeTangentPlanes(s));
      int calls = geti();
      for (int c = 0; c < calls; c++) {
        int vno = geti(), n = geti();
        std::vector<int> nb(n > 0 ? n : 1);
        for (int j = 0; j < n; j++) nb[j] = geti();
        oi(MRIScomputeSecondFundamentalFormAtVertex(s, vno, nb.data(), n));
        of(s->Kmin); of(s->Kmax); of(s->Hmin); of(s->Hmax); od(s->Ktotal);
        VERTEX *v = &s->vertices[vno];
        float x[10] = {v->k1, v->k2, v->K, v->H, v->e1x, v->e1y, v->e1z, v->e2x, v->e2y, v->e2z};
        for (float y : x) of(y);
      }
    } else fail("mpe op");
  }
}

// relay 32 of track D: mrisurf_mri.cpp's surface-to-voxel maps and the cached conversions, and
// mriflood.cpp's surface functions (MRISshell, MRISribbon, MRISmakeBoundingVolume,
// MRISfillInterior) on one or two surfaces read by MRISread, each with a volume geometry of the
// case, and 1 to 3 volumes; each volume printed as its extent, type, frames, geometry and
// ras_good_flag, then its voxels as runs <bits>x<n>, then E
static void fli_vol(MRI *m)
{
  if (!m) { out += "NULL "; return; }
  oi(m->width); oi(m->height); oi(m->depth); oi(m->type); oi(m->nframes);
  float gs[15] = {m->xsize, m->ysize, m->zsize, m->x_r, m->x_a, m->x_s, m->y_r, m->y_a,
                  m->y_s, m->z_r, m->z_a, m->z_s, m->c_r, m->c_a, m->c_s};
  for (float x : gs) of(x);
  oi(m->ras_good_flag);
  bool have = false;
  uint32_t cur = 0;
  unsigned long long n = 0;
  char b[48];
  for (int f = 0; f < m->nframes; f++)
    for (int s = 0; s < m->depth; s++)
      for (int r = 0; r < m->height; r++)
        for (int c = 0; c < m->width; c++) {
          float x = MRIgetVoxVal(m, c, r, s, f);
          uint32_t u;
          memcpy(&u, &x, 4);
          if (have && u == cur) { n++; continue; }
          if (have) { snprintf(b, sizeof b, "%08x" "x%llu ", cur, n); out += b; }
          cur = u;
          n = 1;
          have = true;
        }
  if (have) { snprintf(b, sizeof b, "%08x" "x%llu ", cur, n); out += b; }
  out += "E ";
}

static void fli_ret(MRI *r, MRI *given)
{
  if (!r) { out += "NULL "; return; }
  if (given) {
    out += r == given ? "G " : "? ";
    fli_vol(given);
  } else {
    out += "N ";
    fli_vol(r);
    MRIfree(&r);
  }
}

static void fli_geom(VOL_GEOM *v)
{
  v->xsize = getf(); v->ysize = getf(); v->zsize = getf();
  v->x_r = getf(); v->x_a = getf(); v->x_s = getf();
  v->y_r = getf(); v->y_a = getf(); v->y_s = getf();
  v->z_r = getf(); v->z_a = getf(); v->z_s = getf();
  v->c_r = getf(); v->c_a = getf(); v->c_s = getf();
}

void fli_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  setenv("SURFER_SIDEDOOR", "1", 1);
  int two = geti();
  std::vector<std::string> names;
  std::vector<int> counts;
  for (int k = 0; k < 1 + (two != 0); k++) {
    std::string bytes = geth();
    counts.push_back(geti());
    counts.push_back(geti());
    std::string name = "d/lh.s" + std::to_string(k);
    FILE *fp = fopen(name.c_str(), "wb");
    if (!fp) fail("the surface file");
    fwrite(bytes.data(), 1, bytes.size(), fp);
    fclose(fp);
    names.push_back(name);
  }
  errno = 0;
  std::vector<MRIS *> surfs;
  for (size_t k = 0; k < names.size(); k++) {
    MRIS *s = MRISread(names[k].c_str());
    if (!s) { out += "NULL "; return; }
    if (counts[2 * k] != s->nvertices || counts[2 * k + 1] != s->nfaces) {
      oi(s->nvertices); oi(s->nfaces); out += "COUNTS ";
      return;
    }
    surfs.push_back(s);
  }
  for (MRIS *s : surfs) {
    s->vg.valid = geti();
    s->vg.width = geti(); s->vg.height = geti(); s->vg.depth = geti();
    fli_geom(&s->vg);
  }
  Gdiag = (unsigned long)(unsigned int)geti();
  Gdiag_no = geti();
  int nvol = geti();
  std::vector<MRI *> vols;
  for (int k = 0; k < nvol; k++) {
    int type = geti(), w = geti(), h = geti(), d = geti();
    MRI *v = MRIalloc(w, h, d, type);
    fli_geom(v);
    MRIreInitCache(v);
    int fill = geti();
    if (fill)
      for (int s = 0; s < d; s++)
        for (int r = 0; r < h; r++)
          for (int c = 0; c < w; c++) MRIsetVoxVal(v, c, r, s, 0, (float)((c + 2 * r + 3 * s + fill) % 4));
    vols.push_back(v);
  }
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    out += op + " ";
    fflush(stdout);
    if (op == "r2v" || op == "v2r" || op == "r2vc" || op == "v2rc") {
      MRIS *m = surfs[geti()];
      MRI *v = vols[geti()];
      double a = getd(), b = getd(), c = getd(), x = 0, y = 0, z = 0;
      if (op == "r2v") MRISsurfaceRASToVoxel(m, v, a, b, c, &x, &y, &z);
      else if (op == "v2r") MRISsurfaceRASFromVoxel(m, v, a, b, c, &x, &y, &z);
      else if (op == "r2vc") MRISsurfaceRASToVoxelCached(m, v, a, b, c, &x, &y, &z);
      else MRISsurfaceRASFromVoxelCached(m, v, a, b, c, &x, &y, &z);
      od(x); od(y); od(z);
    } else if (op == "map") {
      MRIS *m = surfs[geti()];
      MRI *v = vols[geti()];
      MRIS_SurfRAS2VoxelMap *map = MRIS_makeRAS2VoxelMap(v, m);
      for (int j = 0; j < 2; j++) {
        if (j == 1) {
          m = surfs[geti()];
          v = vols[geti()];
          MRIS_loadRAS2VoxelMap(map, v, m);
        }
        int n = geti();
        for (int i = 0; i < n; i++) {
          double a = getd(), b = getd(), c = getd(), x, y, z;
          MRIS_useRAS2VoxelMap(map, v, a, b, c, &x, &y, &z);
          od(x); od(y); od(z);
        }
      }
      MRIS_freeRAS2VoxelMap(&map);
    } else if (op == "shl") {
      MRIS *m = surfs[geti()];
      MRI *v = vols[geti()];
      int di = geti(), clear = geti();
      MRI *given = di >= 0 ? vols[di] : NULL;
      fli_ret(MRISshell(v, m, given, clear), given);
    } else if (op == "rib") {
      MRIS *inner = surfs[geti()], *outer = surfs[geti()];
      MRI *v = vols[geti()];
      int di = geti();
      MRI *given = di >= 0 ? vols[di] : NULL;
      fli_ret(MRISribbon(inner, outer, v, given), given);
    } else if (op == "bbv") {
      MRIS *m = surfs[geti()];
      double res = getd();
      MRI *r = MRISmakeBoundingVolume(m, res);
      fli_vol(r);
      if (r) MRIfree(&r);
    } else if (op == "fil") {
      MRIS *m = surfs[geti()];
      double res = getd();
      int di = geti();
      MRI *given = di >= 0 ? vols[di] : NULL;
      fli_ret(MRISfillInterior(m, res, given), given);
    } else if (op == "bbf") {
      MRIS *m = surfs[geti()];
      double res = getd();
      MRI *b = MRISmakeBoundingVolume(m, res);
      fli_ret(MRISfillInterior(m, res, b), b);
      MRIfree(&b);
    } else fail("fli op");
  }
}

// relay 32 of track D, the recorded heads (real.py): `flr fil <surface> <volume>`:
// MRISfillInterior(surface, 0, a copy of the volume) (mri_surf2volseg's call); `flr rib <inner>
// <outer> <volume>`: MRISribbon(inner, outer, volume, NULL) (mris_volmask's); `flr bbf <surface>
// <resolution>`: MRISmakeBoundingVolume, then MRISfillInterior into it (mris_place_surface's T2 and
// FLAIR steps); each result as its extent, type, frames, geometry, the count of voxels that are
// not 0 and the FNV-1a of its voxels' bytes
static void flr_put(MRI *m)
{
  if (!m) { out += "NULL "; return; }
  oi(m->width); oi(m->height); oi(m->depth); oi(m->type); oi(m->nframes);
  float gs[15] = {m->xsize, m->ysize, m->zsize, m->x_r, m->x_a, m->x_s, m->y_r, m->y_a,
                  m->y_s, m->z_r, m->z_a, m->z_s, m->c_r, m->c_a, m->c_s};
  for (float x : gs) of(x);
  long long nz = 0;
  for (int f = 0; f < m->nframes; f++)
    for (int s = 0; s < m->depth; s++)
      for (int r = 0; r < m->height; r++)
        for (int c = 0; c < m->width; c++) nz += MRIgetVoxVal(m, c, r, s, f) != 0;
  oi(nz);
  uint64_t h = 0xcbf29ce484222325ull;
  const unsigned char *b = (const unsigned char *)m->chunk;
  for (size_t i = 0; i < m->bytes_total; i++) h = (h ^ b[i]) * 0x100000001b3ull;
  char t[24];
  snprintf(t, sizeof t, "%016" PRIx64 " ", h);
  out += t;
}

void flr_case()
{
  setenv("SURFER_SIDEDOOR", "1", 1);
  // relay 35 of track D: a GIFTI file's Date (flr crv) the same on both sides
  setenv("FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING", "probe date", 1);
  std::string op = next();
  out += op + " ";
  if (op == "fil") {
    MRIS *s = MRISread(next());
    MRI *v = MRIread(next());
    if (!s || !v) fail("cannot read the inputs");
    MRI *d = MRIcopy(v, NULL);
    MRIcopyHeader(v, d);
    MRISfillInterior(s, 0, d);
    flr_put(d);
  } else if (op == "rib") {
    MRIS *inner = MRISread(next());
    MRIS *outer = MRISread(next());
    MRI *v = MRIread(next());
    if (!inner || !outer || !v) fail("cannot read the inputs");
    flr_put(MRISribbon(inner, outer, v, NULL));
  } else if (op == "bbf") {
    MRIS *s = MRISread(next());
    double res = getd();
    if (!s) fail("cannot read the inputs");
    MRI *b = MRISmakeBoundingVolume(s, res);
    MRISfillInterior(s, res, b);
    flr_put(b);
  } else if (op == "crv") {
    // relay 35 of track D: a recorded curvature file read into its surface (MRISreadCurvatureFile),
    // its values' FNV-1a, then written in five formats into the work folder (MRISwriteCurvature)
    // and read back from the MGZ and new-format files; each written file's FNV-1a (a .gii one's
    // after gii_normal) and whether the new-format file is the recorded file byte for byte
    MRIS *s = MRISread(next());
    std::string cf = next();
    if (!s) fail("cannot read the inputs");
    mkdir(workdir.c_str(), 0700);
    clear_dir(workdir, true);
    if (chdir(workdir.c_str()) != 0) fail("chdir");
    auto fnv = [](const std::string &b) {
      uint64_t h = 0xcbf29ce484222325ull;
      for (unsigned char c : b) h = (h ^ c) * 0x100000001b3ull;
      char t[24];
      snprintf(t, sizeof t, "%016" PRIx64 " ", h);
      out += t;
    };
    auto curv_fnv = [&]() {
      std::string b;
      for (int v = 0; v < s->nvertices; v++) b.append((const char *)&s->vertices[v].curv, 4);
      fnv(b);
      of(s->max_curv); of(s->min_curv);
    };
    auto slurp = [](const std::string &n) {
      std::string b;
      if (FILE *fp = fopen(n.c_str(), "rb")) {
        char buf[65536];
        size_t k;
        while ((k = fread(buf, 1, sizeof buf, fp)) > 0) b.append(buf, k);
        fclose(fp);
      }
      return b;
    };
    oi(MRISreadCurvatureFile(s, cf.c_str()));
    curv_fnv();
    // five stems: a GIFTI file beside a file of its stem would stop the read-back (FS_GII's rule)
    for (const char *nm : {"./o", "./m.mgz", "./n.nii.gz", "./g.gii", "./v.vtk"}) oi(MRISwriteCurvature(s, nm));
    std::vector<std::string> files;
    list_files(".", "", files);
    std::sort(files.begin(), files.end());
    for (const std::string &n : files) {
      oh(n.c_str());
      std::string b = slurp(n);
      bool gii = n.size() >= 4 && n.compare(n.size() - 4, 4, ".gii") == 0;
      fnv(gii ? gii_normal(b) : b);
    }
    auto newfmt = [](const std::string &n) { return n.size() >= 2 && n.compare(n.size() - 2, 2, ".o") == 0; };
    std::string written;
    for (const std::string &n : files)
      if (newfmt(n)) written = slurp(n);
    oi(written == slurp(cf));
    for (const std::string &n : files) {
      if (!newfmt(n) && n.find(".mgz") == std::string::npos) continue;
      for (int v = 0; v < s->nvertices; v++) s->vertices[v].curv = 0;
      oi(MRISreadCurvatureFile(s, ("./" + n).c_str()));
      curv_fnv();
    }
  } else if (op == "pvw") {
    // relay 36 of track D: a recorded per-vertex file read into its surface's values
    // (MRISreadValues), then the patch (MRISwritePatch), the normals (MRISwriteNormals), the values
    // into a volume given (MRISwriteIntoVolume), the normals read back (MRISreadNormals, after
    // zeroing them) and the values read from the normals' volume (frame 0): each status, the
    // values', normals' and volume's FNV-1a, and each written file's
    MRIS *s = MRISread(next());
    std::string cf = next();
    if (!s) fail("cannot read the inputs");
    mkdir(workdir.c_str(), 0700);
    clear_dir(workdir, true);
    if (chdir(workdir.c_str()) != 0) fail("chdir");
    auto fnv = [](const std::string &b) {
      uint64_t h = 0xcbf29ce484222325ull;
      for (unsigned char c : b) h = (h ^ c) * 0x100000001b3ull;
      char t[24];
      snprintf(t, sizeof t, "%016" PRIx64 " ", h);
      out += t;
    };
    auto field_fnv = [&](int which) {
      std::string b;
      for (int v = 0; v < s->nvertices; v++) {
        VERTEX *x = &s->vertices[v];
        if (which == 0) b.append((const char *)&x->val, 4);
        else { b.append((const char *)&x->nx, 4); b.append((const char *)&x->ny, 4); b.append((const char *)&x->nz, 4); }
      }
      fnv(b);
    };
    oi(MRISreadValues(s, cf.c_str()));
    field_fnv(0);
    oi(MRISwritePatch(s, "./p.patch"));
    oi(MRISwriteNormals(s, "./n.mgz"));
    MRI *given = MRIalloc(s->nvertices, 1, 1, MRI_FLOAT);
    MRI *r = MRISwriteIntoVolume(s, given, VERTEX_VALS);
    oi(r == given);
    {
      std::string b;
      for (int v = 0; v < s->nvertices; v++) { float x = MRIgetVoxVal(given, v, 0, 0, 0); b.append((const char *)&x, 4); }
      fnv(b);
    }
    for (int v = 0; v < s->nvertices; v++) { s->vertices[v].nx = 0; s->vertices[v].ny = 0; s->vertices[v].nz = 0; }
    oi(MRISreadNormals(s, "./n.mgz"));
    field_fnv(1);
    oi(MRISreadValues(s, "./n.mgz"));
    field_fnv(0);
    std::vector<std::string> files;
    list_files(".", "", files);
    std::sort(files.begin(), files.end());
    for (const std::string &n : files) {
      oh(n.c_str());
      std::string b;
      if (FILE *fp = fopen(n.c_str(), "rb")) {
        char buf[65536];
        size_t k;
        while ((k = fread(buf, 1, sizeof buf, fp)) > 0) b.append(buf, k);
        fclose(fp);
      }
      fnv(b);
    }
  } else if (op == "vps") {
    // relay 36 of track D: a recorded surface's other files' positions (`vps <surf> <sphere>
    // <smoothwm> <pial>`): MRISreadCanonicalCoordinates, MRISreadOriginalProperties,
    // MRISreadPialCoordinates, MRISreadWhiteCoordinates (the surface's own file again) and
    // MRISreadVertexPositions (the pial file), each status, then the FNV-1a of each coordinate
    // set, theta and phi, the areas, the normals, and the totals
    std::string sn = next();
    MRIS *s = MRISread(sn.c_str());
    std::string sph = next(), swm = next(), pial = next();
    if (!s) fail("cannot read the inputs");
    oi(MRISreadCanonicalCoordinates(s, sph.c_str()));
    oi(MRISreadOriginalProperties(s, swm.c_str()));
    oi(MRISreadPialCoordinates(s, pial.c_str()));
    oi(MRISreadWhiteCoordinates(s, sn.c_str()));
    oi(MRISreadVertexPositions(s, pial.c_str()));
    for (int set = 0; set < 8; set++) {
      uint64_t h = 0xcbf29ce484222325ull;
      for (int v = 0; v < s->nvertices; v++) {
        VERTEX *x = &s->vertices[v];
        float f[3];
        switch (set) {
          case 0: f[0] = x->x; f[1] = x->y; f[2] = x->z; break;
          case 1: f[0] = x->cx; f[1] = x->cy; f[2] = x->cz; break;
          case 2: f[0] = x->origx; f[1] = x->origy; f[2] = x->origz; break;
          case 3: f[0] = x->pialx; f[1] = x->pialy; f[2] = x->pialz; break;
          case 4: f[0] = x->whitex; f[1] = x->whitey; f[2] = x->whitez; break;
          case 5: f[0] = x->theta; f[1] = x->phi; f[2] = 0; break;
          case 6: f[0] = x->area; f[1] = x->origarea; f[2] = 0; break;
          default: f[0] = x->nx; f[1] = x->ny; f[2] = x->nz; break;
        }
        const unsigned char *b = (const unsigned char *)f;
        for (int k = 0; k < 12; k++) h = (h ^ b[k]) * 0x100000001b3ull;
      }
      char t[24];
      snprintf(t, sizeof t, "%016" PRIx64 " ", h);
      out += t;
    }
    oi(s->status); of(s->total_area); of(s->orig_area); od(s->radius);
  } else if (op == "lbl") {
    // relay 37 of track D: a recorded label on its white surface (`lbl <white> <label>`, the label
    // read by LabelRead): LabelArea; LabelMarkSurface, the marks' FNV-1a; LabelErode by 2 and
    // LabelDilate by 2 (CURRENT_VERTICES) of a copy, each status and the points' FNV-1a;
    // MRISlabel2Mask of the label, the mask's FNV-1a; MRISlabelInvert, its count and FNV-1a
    MRIS *s = MRISread(next());
    LABEL *a = LabelRead(NULL, next());
    if (!s || !a) fail("cannot read the inputs");
    setenv("SUBJECTS_DIR", "/", 1);
    std::string hb;
    auto put = [&hb](const void *p, size_t n) { hb.append((const char *)p, n); };
    auto flush = [&hb]() {
      uint64_t h = 0xcbf29ce484222325ull;
      for (unsigned char c : hb) h = (h ^ c) * 0x100000001b3ull;
      char t[24];
      snprintf(t, sizeof t, "%016" PRIx64 " ", h);
      out += t;
      hb.clear();
    };
    auto pts = [&](LABEL *l) {
      oi(l->n_points);
      for (int k = 0; k < l->n_points; k++) {
        LV *p = &l->lv[k];
        put(&p->vno, 4); put(&p->x, 4); put(&p->y, 4); put(&p->z, 4); put(&p->deleted, 1); put(&p->stat, 4);
      }
      flush();
    };
    od(LabelArea(a, s));
    MRISclearMarks(s);
    oi(LabelMarkSurface(a, s));
    for (int v = 0; v < s->nvertices; v++) put(&s->vertices[v].marked, 2);
    flush();
    MRISclearMarks(s);
    LABEL *e = LabelCopy(a, NULL);
    oi(LabelErode(e, s, 2));
    pts(e);
    LABEL *d = LabelCopy(a, NULL);
    oi(LabelDilate(d, s, 2, CURRENT_VERTICES));
    pts(d);
    MRI *m = MRISlabel2Mask(s, a, NULL);
    if (!m) out += "NULL ";
    else {
      for (int v = 0; v < s->nvertices; v++) { float f = MRIgetVoxVal(m, v, 0, 0, 0); put(&f, 4); }
      flush();
    }
    LABEL *inv = MRISlabelInvert(s, a);
    pts(inv);
  } else if (op == "war") {
    // relay 38 of track D: mris_place_surface --area-map on a recorded head (`war <surface>
    // <recorded>`): MRISread, MRIScomputeMetricProperties, MRISwriteArea into the work folder; the
    // status, the written file's length and FNV-1a, and whether it is the recorded file byte for
    // byte
    std::string sn = next();
    MRIS *s = MRISread(sn.c_str());
    std::string rec = next();
    if (!s) fail("cannot read the surface");
    MRIScomputeMetricProperties(s);
    mkdir(workdir.c_str(), 0700);
    // a name with a folder and the hemisphere first: MRISwriteCurvature keeps it as given (a name
    // without a folder goes beside the surface)
    std::string base = sn.substr(sn.rfind('/') + 1);
    std::string name = workdir + "/" + base.substr(0, 2) + ".w.area";
    oi(MRISwriteArea(s, name.c_str()));
    auto slurp = [](const std::string &p) {
      std::string b;
      if (FILE *f = fopen(p.c_str(), "rb")) {
        char buf[65536];
        size_t n;
        while ((n = fread(buf, 1, sizeof buf, f)) > 0) b.append(buf, n);
        fclose(f);
      }
      return b;
    };
    std::string w = slurp(name), r = slurp(rec);
    oi((long long)w.size());
    uint64_t h = 0xcbf29ce484222325ull;
    for (unsigned char c : w) h = (h ^ c) * 0x100000001b3ull;
    char t[24];
    snprintf(t, sizeof t, "%016" PRIx64 " ", h);
    out += t;
    out += w == r ? "SAME " : "DIFF ";
    unlink(name.c_str());
  } else if (op == "mki") {
    // relay 39 of track D: mrisMarkIntersections on a recorded surface (`mki <surface> <fill>`):
    // the faces counted, the vertices marked (their count and numbers); with hole filling the
    // vertices of each cluster number (1 to the largest)
    MRIS *s = MRISread(next());
    int fill = geti();
    if (!s) fail("cannot read the surface");
    oi(mrisMarkIntersections(s, fill));
    int nm = 0, mx = 0;
    for (int v = 0; v < s->nvertices; v++) {
      if (s->vertices[v].marked) nm++;
      if (s->vertices[v].undefval > mx) mx = s->vertices[v].undefval;
    }
    oi(nm);
    for (int v = 0; v < s->nvertices; v++)
      if (s->vertices[v].marked) oi(v);
    if (fill) {
      oi(mx);
      std::vector<int> c(mx + 1, 0);
      for (int v = 0; v < s->nvertices; v++)
        if (s->vertices[v].undefval > 0) c[s->vertices[v].undefval]++;
      for (int k = 1; k <= mx; k++) oi(c[k]);
    }
  } else if (op == "sgd") {
    // relay 38 of track D, the stack measurement (validation/probes/fs-surfio/stack.py): a surface
    // (`sgd <surface>`) with every value 1, one sclustMapSurfClusters (0.5, no upper bound, positive
    // sign, no area minimum): the cluster count and the first cluster's vertex count, or a crash
    MRIS *s = MRISread(next());
    if (!s) fail("cannot read the surface");
    for (int v = 0; v < s->nvertices; v++) s->vertices[v].val = 1;
    int n = 0;
    SCS *l = sclustMapSurfClusters(s, 0.5, -1, 1, 0, &n, NULL, NULL);
    oi(n);
    oi(l ? l[0].nmembers : -1);
  } else if (op == "scl") {
    // relay 38 of track D: surfcluster.cpp and the label clean-ups on a recorded head (`scl <white>
    // <label>`): LabelRemoveIslandsSurf, LabelRemoveHolesSurf, LabelRemoveHolesAndIslandsSurf of
    // the label (each label's count and FNV-1a, the vertices' cluster numbers' FNV-1a); then the
    // clusters of |x| at least 20 mm (sclustMapSurfClusters, thsign 0, minarea 10, a transform),
    // the list's count and FNV-1a, its table (DumpSurfClusterSum) on standard output, the cluster
    // numbers' FNV-1a; sclustSetSurfaceValToCWP (p-values 10^-k), the values' FNV-1a;
    // SurfClusterSummaryOld's FNV-1a; sclustGrowByDist from vertex 0 within 25 mm (flat), its count
    // and the list's FNV-1a
    MRIS *s = MRISread(next());
    LABEL *a = LabelRead(NULL, next());
    if (!s || !a) fail("cannot read the inputs");
    setenv("SUBJECTS_DIR", "/", 1);
    std::string hb;
    auto put = [&hb](const void *p, size_t n) { hb.append((const char *)p, n); };
    auto flush = [&hb]() {
      uint64_t h = 0xcbf29ce484222325ull;
      for (unsigned char c : hb) h = (h ^ c) * 0x100000001b3ull;
      char t[24];
      snprintf(t, sizeof t, "%016" PRIx64 " ", h);
      out += t;
      hb.clear();
    };
    auto pts = [&](LABEL *l) {
      oi(l->n_points);
      oi(l->max_points);
      for (int k = 0; k < l->n_points; k++) {
        LV *p = &l->lv[k];
        put(&p->vno, 4); put(&p->x, 4); put(&p->y, 4); put(&p->z, 4); put(&p->stat, 4);
      }
      flush();
    };
    auto undef = [&]() {
      for (int v = 0; v < s->nvertices; v++) put(&s->vertices[v].undefval, 4);
      flush();
    };
    auto list = [&](SCS *l, int n) {
      oi(n);
      // the 19 four-byte fields (76 bytes), then the three p-values (the 4 bytes between them are
      // padding)
      for (int k = 0; k < n; k++) { put(&l[k], 76); put(&l[k].pval_clusterwise, 24); }
      flush();
    };
    pts(LabelRemoveIslandsSurf(s, a));
    undef();
    pts(LabelRemoveHolesSurf(s, a));
    undef();
    pts(LabelRemoveHolesAndIslandsSurf(s, a));
    undef();
    for (int v = 0; v < s->nvertices; v++) s->vertices[v].val = s->vertices[v].x;
    MATRIX *T = MatrixIdentity(4, NULL);
    T->rptr[1][4] = 1.5;
    T->rptr[2][2] = 0.75;
    int n = 0;
    SCS *l = sclustMapSurfClusters(s, 20, -1, 0, 10, &n, T, NULL);
    list(l, n);
    DumpSurfClusterSum(stdout, l, n);
    undef();
    const double pv[5] = {1.0, 0.1, 0.01, 0.001, 0.0001};
    for (int k = 0; k < n; k++) l[k].pval_clusterwise = pv[k % 5];
    sclustSetSurfaceValToCWP(s, l);
    for (int v = 0; v < s->nvertices; v++) put(&s->vertices[v].val, 4);
    flush();
    for (int v = 0; v < s->nvertices; v++) s->vertices[v].val = s->vertices[v].x;
    setenv("USE_FAST_SURF_SMOOTHER", "0", 1);
    int n2 = 0;
    SCS *l2 = SurfClusterSummaryOld(s, T, &n2);
    list(l2, n2);
    std::vector<int> vl(s->nvertices, -2);
    oi(sclustGrowByDist(s, 0, 25.0, 0, -1, vl.data()));
    for (int v = 0; v < s->nvertices; v++) put(&vl[v], 4);
    flush();
  } else if (op == "mpx") {
    // relay 37 of track D: group (e) on a recorded head (`mpx <white> <sphere>`): the white
    // surface (its Talairach transform read from ../mri/transforms by MRISread) with the sphere's
    // canonical coordinates: mrisComputePrincipalCurvatureDistributions (100 bins each, a 100 x
    // 100 float volume) and mrisComputeNormalDotDistribution (100 bins), each status, bin size and
    // the FNV-1a of each histogram's bins and counts, of the volume's voxels, of k1 and k2, of the
    // positions and normals after; the sphere with its own canonical coordinates:
    // mrisSampleAshburnerTriangleEnergy (lambda 1) at 64 vertices with the point 0.5 mm along x,
    // the energies' FNV-1a; the white surface moved to Talairach space in place
    // (MRIStalairachTransform): its positions' FNV-1a, its box and centre
    std::string wn = next(), sn = next();
    MRIS *s = MRISread(wn.c_str());
    MRIS *sp = MRISread(sn.c_str());
    if (!s || !sp) fail("cannot read the inputs");
    std::string hb;
    auto put = [&hb](float f) { hb.append((const char *)&f, 4); };
    auto flush = [&hb]() {
      uint64_t h = 0xcbf29ce484222325ull;
      for (unsigned char c : hb) h = (h ^ c) * 0x100000001b3ull;
      char t[24];
      snprintf(t, sizeof t, "%016" PRIx64 " ", h);
      out += t;
      hb.clear();
    };
    oi(MRISreadCanonicalCoordinates(s, sn.c_str()));
    HISTOGRAM *h1 = HISTOalloc(100), *h2 = HISTOalloc(100), *hd = HISTOalloc(100);
    MRI *m = MRIalloc(100, 100, 1, MRI_FLOAT);
    oi(mrisComputePrincipalCurvatureDistributions(s, h1, h2, m));
    oi(mrisComputeNormalDotDistribution(s, hd));
    for (HISTOGRAM *h : {h1, h2, hd}) {
      of(h->bin_size);
      for (int b = 0; b < h->nbins; b++) { put(h->bins[b]); put(h->counts[b]); }
      flush();
    }
    of(m->xsize); of(m->ysize); of(m->xstart); of(m->ystart);
    for (int y = 0; y < 100; y++)
      for (int x = 0; x < 100; x++) put(MRIgetVoxVal(m, x, y, 0, 0));
    flush();
    for (int v = 0; v < s->nvertices; v++) { put(s->vertices[v].k1); put(s->vertices[v].k2); }
    flush();
    for (int v = 0; v < s->nvertices; v++) {
      VERTEX *x = &s->vertices[v];
      for (float f : {x->x, x->y, x->z, x->nx, x->ny, x->nz, x->area}) put(f);
    }
    flush();
    oi(MRISreadCanonicalCoordinates(sp, sn.c_str()));
    INTEGRATION_PARMS *parms = (INTEGRATION_PARMS *)calloc(1, sizeof(INTEGRATION_PARMS));
    parms->l_ashburner_lambda = 1.0f;
    for (int k = 0; k < 64; k++) {
      int vno = (int)((long long)k * sp->nvertices / 64);
      VERTEX *v = &sp->vertices[vno];
      put(mrisSampleAshburnerTriangleEnergy(sp, vno, parms, v->x + 0.5f, v->y, v->z));
    }
    flush();
    MRIStalairachTransform(s, s);
    for (int v = 0; v < s->nvertices; v++) { put(s->vertices[v].x); put(s->vertices[v].y); put(s->vertices[v].z); }
    flush();
    for (float f : {s->xlo, s->ylo, s->zlo, s->xhi, s->yhi, s->zhi, s->xctr, s->yctr, s->zctr}) of(f);
  } else if (op == "ann") {
    // relay 36 of track D: a recorded annotation read into its white surface
    // (MRISreadAnnotation), written as an annotation file, a segmentation (.mgz) and GIFTI
    // (MRISwriteAnnotation) into the work folder and read back; each status, the annotations'
    // and the colour table's FNV-1a, each written file's (a .gii one's after gii_normal)
    MRIS *s = MRISread(next());
    std::string af = next();
    if (!s) fail("cannot read the inputs");
    mkdir(workdir.c_str(), 0700);
    clear_dir(workdir, true);
    if (chdir(workdir.c_str()) != 0) fail("chdir");
    auto fnv = [](const std::string &b) {
      uint64_t h = 0xcbf29ce484222325ull;
      for (unsigned char c : b) h = (h ^ c) * 0x100000001b3ull;
      char t[24];
      snprintf(t, sizeof t, "%016" PRIx64 " ", h);
      out += t;
    };
    auto ann_fnv = [&]() {
      std::string b;
      for (int v = 0; v < s->nvertices; v++) b.append((const char *)&s->vertices[v].annotation, 4);
      fnv(b);
      std::string c;
      if (s->ct) {
        for (int k = 0; k < s->ct->nentries; k++) {
          CTE *e = s->ct->entries[k];
          if (!e) { c += "-"; continue; }
          c += e->name;
          c.append((const char *)&e->ri, 16);
        }
      }
      fnv(c);
    };
    auto slurp = [](const std::string &n) {
      std::string b;
      if (FILE *fp = fopen(n.c_str(), "rb")) {
        char buf[65536];
        size_t k;
        while ((k = fread(buf, 1, sizeof buf, fp)) > 0) b.append(buf, k);
        fclose(fp);
      }
      return b;
    };
    oi(MRISreadAnnotation(s, af.c_str()));
    ann_fnv();
    for (const char *nm : {"./a.annot", "./s.mgz", "./g.gii"}) oi(MRISwriteAnnotation(s, nm));
    std::vector<std::string> files;
    list_files(".", "", files);
    std::sort(files.begin(), files.end());
    for (const std::string &n : files) {
      oh(n.c_str());
      std::string b = slurp(n);
      bool gii = n.size() >= 4 && n.compare(n.size() - 4, 4, ".gii") == 0;
      fnv(gii ? gii_normal(b) : b);
    }
    oi(slurp("a.annot") == slurp(af));
    for (const std::string &n : files) {
      for (int v = 0; v < s->nvertices; v++) s->vertices[v].annotation = 0;
      oi(MRISreadAnnotation(s, ("./" + n).c_str()));
      ann_fnv();
    }
  } else if (op == "top") {
    // relay 42 of track D: mrisurf_topology.cpp's rest on a recorded surface (`top <surface>
    // <label>`): the Euler counts, the defect index, the neighbours in use and the surface check
    // (MRISisSurfaceValid, verbose 1); the vertices outside the label ripped with their faces and
    // removed (LabelRipRestOfSurface's steps: MRISsetRipInFacesWithRippedVertices, MRISremoveRipped),
    // the counts and the topology's FNV-1a; the distances released and the surface renumbered
    // (MRISrenumberRemovingRippedFacesAndVertices), the counts and FNV-1a; the surface read again
    // and everted (MRISevertSurface), its FNV-1a
    std::string sf = next(), lf = next();
    MRIS *s = MRISread(sf.c_str());
    if (!s) fail("cannot read the surface");
    auto counts = [&](MRIS *m) {
      int a, b, c;
      oi(MRIScomputeEulerNumber(m, &a, &b, &c)); oi(a); oi(b); oi(c);
      oi(MRIStopologicalDefectIndex(m)); oi(mrisCountTotalNeighbors(m));
    };
    auto hash = [&](MRIS *m) {
      uint64_t h = 0xcbf29ce484222325ull;
      auto put = [&](int x) {
        const unsigned char *b = (const unsigned char *)&x;
        for (int k = 0; k < 4; k++) h = (h ^ b[k]) * 0x100000001b3ull;
      };
      for (int v = 0; v < m->nvertices; v++) {
        VERTEX_TOPOLOGY *t = &m->vertices_topology[v];
        put(m->vertices[v].ripflag); put(t->num);
        for (int k = 0; k < t->num; k++) { put(t->f[k]); put(t->n[k]); }
        put(t->vnum); put(t->v2num); put(t->v3num); put(t->vtotal);
        for (int k = 0; k < t->vtotal; k++) put(t->v[k]);
      }
      for (int f = 0; f < m->nfaces; f++) {
        for (int k = 0; k < 3; k++) put(m->faces[f].v[k]);
        put(m->faces[f].ripflag);
      }
      char t[24];
      snprintf(t, sizeof t, "%016" PRIx64 " ", h);
      out += t;
    };
    counts(s);
    oi(MRISisSurfaceValid(s, 0, 1));
    LABEL *l = LabelRead(NULL, lf.c_str());
    if (!l) fail("cannot read the label");
    for (int v = 0; v < s->nvertices; v++) s->vertices[v].ripflag = 1;
    for (int n = 0; n < l->n_points; n++) {
      int v = l->lv[n].vno;
      if (v >= 0 && v < s->nvertices) s->vertices[v].ripflag = 0;
    }
    MRISsetRipInFacesWithRippedVertices(s);
    MRISremoveRipped(s);
    counts(s); hash(s);
    MRISfreeDistsButNotOrig(s);
    MRISfreeDistOrigs(s);
    MRISrenumberRemovingRippedFacesAndVertices(s);
    oi(s->nvertices); oi(s->nfaces); hash(s);
    MRIS *e = MRISread(sf.c_str());
    if (!e) fail("cannot read the surface");
    oi(MRISevertSurface(e));
    hash(e);
  } else if (op == "sbl") {
    // relay 42 of track D: the soap bubbles on a recorded surface (`sbl <surface> <label> <n>`):
    // the label's vertices free (mark 0), the others fixed (1); MRISsoapBubbleVertexPositions and
    // the weighted bubble on a second read, each with `n` passes; the positions' FNV-1a
    std::string sf = next(), lf = next();
    int n = geti();
    LABEL *l = LabelRead(NULL, lf.c_str());
    if (!l) fail("cannot read the label");
    for (int w = 0; w < 2; w++) {
      MRIS *s = MRISread(sf.c_str());
      if (!s) fail("cannot read the surface");
      for (int v = 0; v < s->nvertices; v++) s->vertices[v].marked = 1;
      for (int k = 0; k < l->n_points; k++) {
        int v = l->lv[k].vno;
        if (v >= 0 && v < s->nvertices) s->vertices[v].marked = 0;
      }
      oi(w ? MRISweightedSoapBubbleVertexPositions(s, n) : MRISsoapBubbleVertexPositions(s, n));
      uint64_t h = 0xcbf29ce484222325ull;
      for (int v = 0; v < s->nvertices; v++) {
        float c[3] = {s->vertices[v].x, s->vertices[v].y, s->vertices[v].z};
        const unsigned char *b = (const unsigned char *)c;
        for (int k = 0; k < 12; k++) h = (h ^ b[k]) * 0x100000001b3ull;
      }
      char t[24];
      snprintf(t, sizeof t, "%016" PRIx64 " ", h);
      out += t;
    }
  } else if (op == "mcl") {
    // relay 43 of track D: the closest searches on recorded surfaces (`mcl <white> <pial> <sphere.reg>
    // <other sphere.reg>`): (1) mri_surf2volseg's: a vertex table of the white surface at 16 mm, each
    // pial vertex and the same moved by 2 mm along each axis (MHTfindClosestVertexNoXYZ); (2)
    // mri_surf2surf's: a vertex table of the sphere at 16 mm, each vertex of the other sphere
    // (MHTfindClosestVertexNo2) and MHTfindClosestVertexSet2; (3) mri_relabel_hypointensities': a vertex
    // table of the white surface at 1 mm, each pial vertex (MHTfindClosestVertexInTable, no global
    // search); (4) mrisProjectOntoSurface's: a face table of the white surface (MHTcreateFaceTable),
    // each pial vertex at 8 mm, 8 shells, into its face, then at 1000 mm unprojected when none is
    // found within 0.5 mm (MHTfindClosestFaceGeneric). For each: the count found and the answers'
    // FNV-1a (the number as 4 bytes, the distance's bytes)
    MRIS *w = MRISread(next());
    MRIS *p = MRISread(next());
    MRIS *sr = MRISread(next());
    MRIS *so = MRISread(next());
    if (!w || !p || !sr || !so) fail("cannot read the surfaces");
    auto mix = [](uint64_t h, const void *b, int n) {
      const unsigned char *c = (const unsigned char *)b;
      for (int k = 0; k < n; k++) h = (h ^ c[k]) * 0x100000001b3ull;
      return h;
    };
    auto put = [&](int n, uint64_t h) {
      oi(n);
      char t[24];
      snprintf(t, sizeof t, "%016" PRIx64 " ", h);
      out += t;
    };
    MRIS_HASH_TABLE *mht = MHTcreateVertexTable_Resolution(w, CURRENT_VERTICES, 16);
    for (int pass = 0; pass < 2; pass++) {
      uint64_t h = 0xcbf29ce484222325ull;
      int found = 0;
      for (int v = 0; v < p->nvertices; v++) {
        float d = 0, o = pass ? 2.0f : 0.0f;
        int r = MHTfindClosestVertexNoXYZ(mht, w, p->vertices[v].x + o, p->vertices[v].y + o, p->vertices[v].z + o, &d);
        found += r >= 0;
        h = mix(h, &r, 4);
        h = mix(h, &d, 4);
      }
      put(found, h);
    }
    MHTfree(&mht);
    mht = MHTcreateVertexTable_Resolution(sr, CURRENT_VERTICES, 16);
    {
      uint64_t h = 0xcbf29ce484222325ull, h2 = h;
      int found = 0, found2 = 0;
      for (int v = 0; v < so->nvertices; v++) {
        float d = 0;
        int r = MHTfindClosestVertexNo2(mht, sr, so, &so->vertices[v], &d);
        found += r >= 0;
        h = mix(h, &r, 4);
        h = mix(h, &d, 4);
        VERTEX *x = MHTfindClosestVertexSet2(mht, sr, so, &so->vertices[v]);
        int r2 = x ? (int)(x - sr->vertices) : -1;
        found2 += r2 >= 0;
        h2 = mix(h2, &r2, 4);
      }
      put(found, h);
      put(found2, h2);
    }
    MHTfree(&mht);
    mht = MHTcreateVertexTable(w, CURRENT_VERTICES);
    {
      uint64_t h = 0xcbf29ce484222325ull;
      int found = 0;
      for (int v = 0; v < p->nvertices; v++) {
        VERTEX *x = MHTfindClosestVertexInTable(mht, w, p->vertices[v].x, p->vertices[v].y, p->vertices[v].z, 0);
        int r = x ? (int)(x - w->vertices) : -1;
        found += r >= 0;
        h = mix(h, &r, 4);
      }
      put(found, h);
    }
    MHTfree(&mht);
    mht = MHTcreateFaceTable(w);
    {
      uint64_t h = 0xcbf29ce484222325ull;
      int found = 0, wide = 0;
      for (int v = 0; v < p->nvertices; v++) {
        VERTEX *x = &p->vertices[v];
        FACE *face = NULL;
        int fno = -1;
        double d = 0;
        MHTfindClosestFaceGeneric(mht, w, x->x, x->y, x->z, 8.0, 8, 1, &face, &fno, &d);
        if (face == NULL || d > 0.5) {
          wide++;
          MHTfindClosestFaceGeneric(mht, w, x->x, x->y, x->z, 1000, -1, -1, &face, &fno, &d);
        }
        found += fno >= 0;
        h = mix(h, &fno, 4);
        h = mix(h, &d, 8);
      }
      put(found, h);
      oi(wide);
    }
    MHTfree(&mht);
  } else if (op == "maf") {
    // relay 41 of track D: mris_ca_label's mode filter on a recorded annotation (`maf <surface>
    // <annotation>`): the annotation read (MRISreadAnnotation), the program's table set from its
    // colour table (set_atable_from_ctable, as mris_ca_label does first), MRISmodeFilterAnnotations
    // with mris_ca_label's 10 passes; the vertices changed, their numbers' and the annotations'
    // FNV-1a
    MRIS *s = MRISread(next());
    std::string af = next();
    if (!s) fail("cannot read the surface");
    oi(MRISreadAnnotation(s, af.c_str()));
    std::vector<int> before(s->nvertices);
    for (int v = 0; v < s->nvertices; v++) before[v] = s->vertices[v].annotation;
    oi(set_atable_from_ctable(s->ct));
    oi(MRISmodeFilterAnnotations(s, 10));
    uint64_t h = 0xcbf29ce484222325ull, hv = h;
    int nch = 0;
    for (int v = 0; v < s->nvertices; v++) {
      const unsigned char *b = (const unsigned char *)&s->vertices[v].annotation;
      for (int k = 0; k < 4; k++) h = (h ^ b[k]) * 0x100000001b3ull;
      if (s->vertices[v].annotation != before[v]) {
        nch++;
        const unsigned char *c = (const unsigned char *)&v;
        for (int k = 0; k < 4; k++) hv = (hv ^ c[k]) * 0x100000001b3ull;
      }
    }
    oi(nch);
    char t[48];
    snprintf(t, sizeof t, "%016" PRIx64 " %016" PRIx64 " ", h, hv);
    out += t;
  } else if (op == "dvs") {
    // relay 41 of track D: mri_binarize --dilate-vertex (`dvs <surface> <vno> <radius>`): the disk
    // of area pi r^2 around the vertex (MRISdilateVertexToSum with the surface's areas); the mask's
    // voxels set and their numbers' FNV-1a
    MRIS *s = MRISread(next());
    int vno = geti();
    double radius = getd();
    if (!s) fail("cannot read the surface");
    MRI *m = MRISdilateVertexToSum(vno, s, NULL, M_PI * radius * radius);
    if (!m) { out += "NULL "; return; }
    uint64_t hv = 0xcbf29ce484222325ull;
    int n = 0;
    for (int c = 0; c < m->width; c++)
      if (MRIgetVoxVal(m, c, 0, 0, 0) != 0) {
        n++;
        const unsigned char *b = (const unsigned char *)&c;
        for (int k = 0; k < 4; k++) hv = (hv ^ b[k]) * 0x100000001b3ull;
      }
    oi(n);
    char t[24];
    snprintf(t, sizeof t, "%016" PRIx64 " ", hv);
    out += t;
  } else fail("flr op");
}

// relay 33 of track D: 3.1-51's rest: MRISbinarizeVolume, MRISmaskLabel, MRISmaskNotLabel and
// MRIcomputeLaminarVolumeFractions (mrisurf_metricProperties.cpp) and MRISextractOutsideDistanceMap
// (mris_fastmarching.cpp) on a surface read by MRISread: its white positions those read, its pial
// positions a second surface's (the same mesh) or the white ones; a volume geometry of the case,
// its values set from the vertex numbers, some vertices marked, a label of vertex numbers, 1 or 2
// volumes; each result printed: a volume as fli_vol does with its xstart, ystart, zstart; the
// vertices' values and marks; the vertices' curvatures; the vertices' positions
static void bvl_values(MRIS *s)
{
  for (int v = 0; v < s->nvertices; v++) {
    VERTEX *x = &s->vertices[v];
    of(x->curv); of(x->stat); of(x->val); of(x->imag_val); of(x->val2); of(x->valbak); of(x->val2bak);
    oi(x->marked);
  }
}

void bvl_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  setenv("SURFER_SIDEDOOR", "1", 1);
  int two = geti();
  std::vector<std::string> names;
  std::vector<int> counts;
  for (int k = 0; k < 1 + (two != 0); k++) {
    std::string bytes = geth();
    counts.push_back(geti());
    counts.push_back(geti());
    std::string name = "d/lh.s" + std::to_string(k);
    FILE *fp = fopen(name.c_str(), "wb");
    if (!fp) fail("the surface file");
    fwrite(bytes.data(), 1, bytes.size(), fp);
    fclose(fp);
    names.push_back(name);
  }
  errno = 0;
  std::vector<MRIS *> surfs;
  for (size_t k = 0; k < names.size(); k++) {
    MRIS *m = MRISread(names[k].c_str());
    if (!m) { out += "NULL "; return; }
    if (counts[2 * k] != m->nvertices || counts[2 * k + 1] != m->nfaces) {
      oi(m->nvertices); oi(m->nfaces); out += "COUNTS ";
      return;
    }
    surfs.push_back(m);
  }
  MRIS *s = surfs[0];
  s->vg.valid = geti();
  s->vg.width = geti(); s->vg.height = geti(); s->vg.depth = geti();
  fli_geom(&s->vg);
  MRISsaveVertexPositions(s, WHITE_VERTICES);
  if (two)
    for (int v = 0; v < s->nvertices; v++) {
      s->vertices[v].x = surfs[1]->vertices[v].x;
      s->vertices[v].y = surfs[1]->vertices[v].y;
      s->vertices[v].z = surfs[1]->vertices[v].z;
    }
  MRISsaveVertexPositions(s, PIAL_VERTICES);
  MRISrestoreVertexPositions(s, WHITE_VERTICES);
  MRIScomputeMetricProperties(s);
  for (int v = 0; v < s->nvertices; v++) {
    VERTEX *x = &s->vertices[v];
    float b = (float)(v + 1) * 0.25f;
    x->curv = b; x->stat = b + 1; x->val = b + 2; x->imag_val = b + 3; x->val2 = b + 4;
    x->valbak = b + 5; x->val2bak = b + 6;
  }
  int nmark = geti();
  for (int k = 0; k < nmark; k++) s->vertices[geti()].marked = 1;
  int npts = geti();
  LABEL *lab = LabelAlloc(npts > 0 ? npts : 1, NULL, NULL);
  lab->n_points = npts;
  for (int k = 0; k < npts; k++) lab->lv[k].vno = geti();
  Gdiag = (unsigned long)(unsigned int)geti();
  Gdiag_no = geti();
  int nvol = geti();
  std::vector<MRI *> vols;
  for (int k = 0; k < nvol; k++) {
    int type = geti(), w = geti(), h = geti(), d = geti(), nf = geti();
    MRI *v = MRIallocSequence(w, h, d, type, nf);
    fli_geom(v);
    MRIreInitCache(v);
    int fill = geti();
    if (fill > 0)
      for (int sl = 0; sl < d; sl++)
        for (int r = 0; r < h; r++)
          for (int c = 0; c < w; c++) MRIsetVoxVal(v, c, r, sl, 0, (float)((c + 2 * r + 3 * sl + fill) % 4));
    if (fill < 0) {
      // a ball of value `val` around (cx, cy, cz), radius rr (voxels)
      int cx = geti(), cy = geti(), cz = geti(), rr = geti(), val = geti();
      for (int sl = 0; sl < d; sl++)
        for (int r = 0; r < h; r++)
          for (int c = 0; c < w; c++)
            if ((c - cx) * (c - cx) + (r - cy) * (r - cy) + (sl - cz) * (sl - cz) <= rr * rr)
              MRIsetVoxVal(v, c, r, sl, 0, (float)val);
    }
    vols.push_back(v);
  }
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    out += op + " ";
    fflush(stdout);
    if (op == "bin") {
      float res = getf(), dist = getf();
      MRI_REGION reg;
      reg.x = geti(); reg.y = geti(); reg.z = geti();
      reg.dx = geti(); reg.dy = geti(); reg.dz = geti();
      MRI *r = MRISbinarizeVolume(s, &reg, res, dist);
      fli_vol(r);
      of(r->xstart); of(r->ystart); of(r->zstart);
      MRIfree(&r);
    } else if (op == "odm") {
      MRI *v = vols[geti()];
      int label = geti(), offset = geti();
      float res = getf(), maxd = getf();
      MRISextractOutsideDistanceMap(s, v, label, offset, res, maxd);
      for (int vno = 0; vno < s->nvertices; vno++) of(s->vertices[vno].curv);
    } else if (op == "msk") {
      oi(MRISmaskLabel(s, lab));
      bvl_values(s);
    } else if (op == "msn") {
      oi(MRISmaskNotLabel(s, lab));
      bvl_values(s);
    } else if (op == "lvf") {
      double res = getd();
      MRI *v = vols[geti()];
      int di = geti();
      MRI *given = di >= 0 ? vols[di] : NULL;
      fli_ret(MRIcomputeLaminarVolumeFractions(s, res, v, given), given);
      for (int vno = 0; vno < s->nvertices; vno++) {
        of(s->vertices[vno].x); of(s->vertices[vno].y); of(s->vertices[vno].z);
      }
    } else fail("bvl op");
  }
}

// ---- relay 35 of track D: crv, the curvature files by name (queue items 3.1-47/48) ----------------

// a file under the work folder as crv prints it: a .gii one as gii_normal, any other its bytes
void dump_crv_file(const std::string &name)
{
  oh(name.c_str());
  FILE *fp = fopen(name.c_str(), "rb");
  if (!fp) { out += "NOFILE "; return; }
  std::string b;
  char buf[4096];
  size_t n;
  while ((n = fread(buf, 1, sizeof buf, fp)) > 0) b.append(buf, n);
  fclose(fp);
  bool gii = name.size() >= 4 && name.compare(name.size() - 4, 4, ".gii") == 0;
  ox(gii ? gii_normal(b) : b);
}

// crv: Gdiag's bits (1 show, 2 verbose), the environment (1 FS_POSIX, 2 FS_GII <h>, 4
// MRIS_READVAL_FRAME <h>, 8 FS_LEAVE_NANS), the creation time's replacement (and always
// SURFER_SIDEDOOR), MRISsetReadFrame's frame (-1: not called), the input files, a surface from
// MRISalloc (its hemisphere, name, subject, per vertex x y z curv val, the faces' vertices), then 1
// to 4 of MRISwriteCurvature (`w`: a name, a GIFTI curvature name or none) and
// MRISreadCurvatureFile (`r`: a name, a volume of the values (case_mri) or none, the overlays' list
// given or not), each status, the volume's values and the overlays; then every vertex's x y z curv
// val, max_curv, min_curv, the counts and the folder's files
void crv_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  mkdir("d/s", 0700);
  int gd = geti();
  if (gd & 1) Gdiag |= DIAG_SHOW;
  if (gd & 2) Gdiag |= DIAG_VERBOSE;
  int envs = geti();
  if (envs & 1) setenv("FS_POSIX", "1", 1);
  if (envs & 2) { std::string v = geth(); setenv("FS_GII", v.c_str(), 1); }
  if (envs & 4) { std::string v = geth(); setenv("MRIS_READVAL_FRAME", v.c_str(), 1); }
  if (envs & 8) setenv("FS_LEAVE_NANS", "1", 1);
  std::string date = geth();
  setenv("FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING", date.c_str(), 1);
  setenv("SURFER_SIDEDOOR", "1", 1);
  int rf = geti();
  if (rf != -1) MRISsetReadFrame(rf);
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
  std::string fname = geth(), subj = geth();
  snprintf(m->fname.data(), STRLEN, "%s", fname.c_str());
  snprintf(m->subject_name.data(), STRLEN, "%s", subj.c_str());
  for (int vno = 0; vno < nv; vno++) {
    VERTEX *v = &m->vertices[vno];
    v->x = getf(); v->y = getf(); v->z = getf(); v->curv = getf(); v->val = getf();
  }
  for (int fno = 0; fno < nf; fno++) for (int k = 0; k < 3; k++) m->faces[fno].v[k] = geti();
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    out += op + " ";
    fflush(stdout);
    if (op == "w") {
      const char *nm = name_tok();
      const char *cf = opt_name();
      oi(MRISwriteCurvature(m, nm, cf));
    } else if (op == "r") {
      const char *nm = name_tok();
      MRI *mri = case_mri(nv);
      int ov = geti();
      std::vector<OverlayInfoStruct> o;
      oi(MRISreadCurvatureFile(m, nm, mri, ov ? &o : NULL));
      fflush(stdout);
      dump_mri(mri);
      if (ov) {
        oi((long long)o.size());
        for (OverlayInfoStruct &x : o) {
          if (x.__foverlay) oh(x.__foverlay); else out += "NULL ";
          oi(x.__type); oi(x.__giftiIntent); oh(x.__shapedatatype); oi(x.__format);
          oi(x.__nValsPerVertex);
          MRI *v = x.__overlaymri;
          if (!v) { out += "NULL "; continue; }
          oi(v->width); oi(v->height); oi(v->depth); oi(v->nframes);
          for (int i = 0; i < v->width; i++) of(MRIgetVoxVal(v, i, 0, 0, 0));
        }
      }
    } else fail("crv op");
    fflush(stdout);
  }
  out += "| ";
  for (int vno = 0; vno < nv; vno++) {
    VERTEX *v = &m->vertices[vno];
    of(v->x); of(v->y); of(v->z); of(v->curv); of(v->val);
  }
  of(m->max_curv); of(m->min_curv); oi(m->nvertices); oi(m->nfaces);
  out += "| ";
  std::vector<std::string> files;
  list_files(".", "", files);
  std::sort(files.begin(), files.end());
  for (const std::string &name : files) dump_crv_file(name);
}

// ---- relay 36 of track D: pvw, the patch, value and normal files (queue items 3.1-47/48) ---------

// a volume as pvw prints it: its extent, frames and type, then every voxel (frames outermost)
void dump_pvw_volume(MRI *m)
{
  oi(m->width); oi(m->height); oi(m->depth); oi(m->nframes); oi(m->type);
  for (int f = 0; f < m->nframes; f++)
    for (int s = 0; s < m->depth; s++)
      for (int r = 0; r < m->height; r++)
        for (int c = 0; c < m->width; c++) of(MRIgetVoxVal(m, c, r, s, f));
}

// pvw: Gdiag's bits (1 show, 2 verbose), the environment (1 FS_POSIX, 4 MRIS_READVAL_FRAME <h>,
// 8 FS_LEAVE_NANS; always SURFER_SIDEDOOR and USER=U), MRISsetReadFrame's frame (-1: not called),
// the input files, a surface from MRISalloc (its name; per vertex x y z val curv area, the
// annotation's bits, dx dy dz nx ny nz wnx wny wnz e1x e1y e1z e2x e2y e2z, ripflag, border; per
// face its vertices and ripflag), then 1 to 4 of MRISwritePatch (`p`), MRISwriteIntoVolume (`i`:
// the field, a volume given (type, extent, frames) or -1), MRISwriteWhiteNormals (`n`),
// MRISwriteNormals (`N`), MRISwritePrincipalDirection (`e`: the direction, a name),
// MRISreadNormals (`R`), MRISreadValues (`v`), MRISreadValuesIntoArray (`a`: a name or none, the
// size), each status (and volume or values); then every vertex's x y z nx ny nz val and the
// folder's files
void pvw_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  int gd = geti();
  if (gd & 1) Gdiag |= DIAG_SHOW;
  if (gd & 2) Gdiag |= DIAG_VERBOSE;
  int envs = geti();
  if (envs & 1) setenv("FS_POSIX", "1", 1);
  if (envs & 4) { std::string v = geth(); setenv("MRIS_READVAL_FRAME", v.c_str(), 1); }
  if (envs & 8) setenv("FS_LEAVE_NANS", "1", 1);
  setenv("SURFER_SIDEDOOR", "1", 1);
  setenv("USER", "U", 1);
  int rf = geti();
  if (rf != -1) MRISsetReadFrame(rf);
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
    v->x = getf(); v->y = getf(); v->z = getf(); v->val = getf(); v->curv = getf(); v->area = getf();
    float a = getf();
    memcpy(&v->annotation, &a, sizeof a);
    v->dx = getf(); v->dy = getf(); v->dz = getf(); v->nx = getf(); v->ny = getf(); v->nz = getf();
    v->wnx = getf(); v->wny = getf(); v->wnz = getf();
    v->e1x = getf(); v->e1y = getf(); v->e1z = getf(); v->e2x = getf(); v->e2y = getf(); v->e2z = getf();
    v->ripflag = geti();
    v->border = geti();
  }
  for (int fno = 0; fno < nf; fno++) {
    for (int k = 0; k < 3; k++) m->faces[fno].v[k] = geti();
    m->faces[fno].ripflag = geti();
  }
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    out += op + " ";
    fflush(stdout);
    if (op == "p") {
      oi(MRISwritePatch(m, name_tok()));
    } else if (op == "i") {
      int which = geti(), ty = geti();
      MRI *given = NULL;
      if (ty >= 0) {
        int w = geti(), h = geti(), d = geti(), f = geti();
        given = MRIallocSequence(w, h, d, ty, f);
      }
      MRI *r = MRISwriteIntoVolume(m, given, which);
      fflush(stdout);
      if (!r) {
        out += "NULL ";
        if (given) dump_pvw_volume(given);
      } else if (r == given) {
        out += "GIVEN ";
        dump_pvw_volume(r);
      } else {
        out += "NEW ";
        dump_pvw_volume(r);
      }
    } else if (op == "n") {
      oi(MRISwriteWhiteNormals(m, name_tok()));
    } else if (op == "N") {
      oi(MRISwriteNormals(m, name_tok()));
    } else if (op == "e") {
      int di = geti();
      oi(MRISwritePrincipalDirection(m, di, name_tok()));
    } else if (op == "R") {
      oi(MRISreadNormals(m, name_tok()));
    } else if (op == "v") {
      oi(MRISreadValues(m, name_tok()));
    } else if (op == "a") {
      const char *nm = opt_name();
      int size = geti();
      float *arr = NULL;
      int r = fssurfio_read_values_into_array(nm, size, &arr);
      fflush(stdout);
      oi(r);
      if (r == 0 && arr) {
        for (int i = 0; i < size; i++) of(arr[i]);
      } else {
        out += "NOARRAY ";
      }
    } else fail("pvw op");
    fflush(stdout);
  }
  out += "| ";
  for (int vno = 0; vno < nv; vno++) {
    VERTEX *v = &m->vertices[vno];
    of(v->x); of(v->y); of(v->z); of(v->nx); of(v->ny); of(v->nz); of(v->val);
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
    char buf[4096];
    size_t n;
    while ((n = fread(buf, 1, sizeof buf, fp)) > 0) b.append(buf, n);
    fclose(fp);
    ox(b);
  }
}

// ---- relay 36 of track D: vps, another file's vertex positions (queue items 3.1-47/49) ---------

// vps: Gdiag's bits (1 show, 2 verbose), the environment (1 FS_GII=.gii, 2 FS_FASTER_MP, 4
// TRIANGULARSURFACE_NOEXTRA_READ; always SURFER_SIDEDOOR and USER=U), the surface's file d/lh.white
// (read by MRISread), the input files, then 1 to 4 of MRISreadVertexPositions (`V`),
// MRISreadWhiteCoordinates (`W`), MRISreadPialCoordinates (`P`), MRISreadCanonicalCoordinates
// (`C`), MRISreadOriginalProperties (`O`), each with a name or none (NULL; `V` and `C` take ""),
// (before the inputs: 1 and d/lh.white.avg.area.mgh's bytes, or 0),
// each status; then every vertex's coordinate sets, theta, phi, area, origarea, normal, and the
// surface's status, total_area, orig_area, radius and counts
void vps_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  int gd = geti();
  if (gd & 1) Gdiag |= DIAG_SHOW;
  if (gd & 2) Gdiag |= DIAG_VERBOSE;
  int envs = geti();
  if (envs & 1) setenv("FS_GII", ".gii", 1);
  if (envs & 2) setenv("FS_FASTER_MP", "1", 1);
  if (envs & 4) setenv("TRIANGULARSURFACE_NOEXTRA_READ", "1", 1);
  setenv("SURFER_SIDEDOOR", "1", 1);
  setenv("USER", "U", 1);
  {
    std::string bytes = geth();
    FILE *fp = fopen("d/lh.white", "wb");
    if (!fp) fail("the surface");
    fwrite(bytes.data(), 1, bytes.size(), fp);
    fclose(fp);
  }
  if (geti()) {
    std::string bytes = geth();
    FILE *fp = fopen("d/lh.white.avg.area.mgh", "wb");
    if (!fp) fail("the average area");
    fwrite(bytes.data(), 1, bytes.size(), fp);
    fclose(fp);
  }
  int nin = geti();
  for (int k = 0; k < nin; k++) {
    std::string name = geth(), bytes = geth();
    FILE *fp = fopen(name.c_str(), "wb");
    if (!fp) fail("an input file");
    fwrite(bytes.data(), 1, bytes.size(), fp);
    fclose(fp);
  }
  errno = 0;
  MRIS *m = MRISread("d/lh.white");
  fflush(stdout);
  if (!m) { out += "NOSURFACE "; return; }
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    out += op + " ";
    fflush(stdout);
    const char *nm = opt_name();
    int r;
    if (op == "V") r = MRISreadVertexPositions(m, nm ? nm : "");
    else if (op == "W") r = MRISreadWhiteCoordinates(m, nm);
    else if (op == "P") r = MRISreadPialCoordinates(m, nm);
    else if (op == "C") r = MRISreadCanonicalCoordinates(m, nm ? nm : "");
    else if (op == "O") r = MRISreadOriginalProperties(m, nm);
    else fail("vps op");
    fflush(stdout);
    oi(r);
  }
  out += "| ";
  for (int vno = 0; vno < m->nvertices; vno++) {
    VERTEX *v = &m->vertices[vno];
    for (float x : {v->x, v->y, v->z, v->whitex, v->whitey, v->whitez, v->pialx, v->pialy, v->pialz,
                    v->cx, v->cy, v->cz, v->origx, v->origy, v->origz, v->theta, v->phi, v->area,
                    v->origarea, v->nx, v->ny, v->nz})
      of(x);
  }
  oi(m->status); of(m->total_area); of(m->orig_area); od(m->radius); oi(m->nvertices); oi(m->nfaces);
  oi(m->group_avg_vtxarea_loaded);
  for (int vno = 0; vno < m->nvertices; vno++) of(m->vertices[vno].group_avg_area);
}

// ---- relay 36 of track D: ann, the annotation files (queue items 3.1-47/48) -------------------

// a colour table from the case (gwr's form), or NULL
COLOR_TABLE *ann_ctab()
{
  int nct = geti();
  if (nct < 0) return NULL;
  COLOR_TABLE *ctab = (COLOR_TABLE *)calloc(1, sizeof(COLOR_TABLE));
  ctab->nentries = nct;
  ctab->version = 2;
  ctab->entries = (COLOR_TABLE_ENTRY **)calloc(nct + 1, sizeof(COLOR_TABLE_ENTRY *));
  strcpy(ctab->fname, "user.ctab");
  for (int k = 0; k < nct; k++) {
    if (!geti()) continue;
    CTE *e = (CTE *)calloc(1, sizeof(CTE));
    std::string name = geth();
    strncpy(e->name, name.c_str(), sizeof(e->name) - 1);
    e->ri = geti(); e->gi = geti(); e->bi = geti(); e->ai = geti();
    e->rf = e->ri / 255.0f; e->gf = e->gi / 255.0f; e->bf = e->bi / 255.0f; e->af = e->ai / 255.0f;
    ctab->entries[k] = e;
  }
  return ctab;
}

void ann_dump_ctab(const COLOR_TABLE *ct)
{
  if (!ct) { out += "NULL "; return; }
  oi(ct->nentries);
  oh(ct->fname);
  for (int k = 0; k < ct->nentries; k++) {
    CTE *e = ct->entries[k];
    if (!e) { out += "- "; continue; }
    oh(e->name); oi(e->ri); oi(e->gi); oi(e->bi); oi(e->ai);
  }
}

// ann: Gdiag's bits (1 show, 2 verbose), the environment (1 FS_POSIX, 2 FS_GII=.gii; always the
// creation time's replacement, SURFER_SIDEDOOR and USER=U), the input files, a surface from
// MRISalloc (its vertices, hemisphere, name; per vertex its annotation and ripflag), the surface's
// colour table and the caller's (or none), then 1 to 4 of MRISwriteAnnotation (`w`: a name,
// writect) and MRISreadAnnotation (`r`: a name, the GIFTI data array, the caller's table or NULL),
// each status; then the annotations, the surface's colour table and the folder's files
void ann_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("s", 0700);
  mkdir("s/surf", 0700);
  mkdir("s/label", 0700);
  int gd = geti();
  if (gd & 1) Gdiag |= DIAG_SHOW;
  if (gd & 2) Gdiag |= DIAG_VERBOSE;
  int envs = geti();
  if (envs & 1) setenv("FS_POSIX", "1", 1);
  if (envs & 2) setenv("FS_GII", ".gii", 1);
  setenv("FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING", "probe date", 1);
  setenv("SURFER_SIDEDOOR", "1", 1);
  setenv("USER", "U", 1);
  int nin = geti();
  for (int k = 0; k < nin; k++) {
    std::string name = geth(), bytes = geth();
    FILE *fp = fopen(name.c_str(), "wb");
    if (!fp) fail("an input file");
    fwrite(bytes.data(), 1, bytes.size(), fp);
    fclose(fp);
  }
  errno = 0;
  int nv = geti();
  MRIS *m = MRISalloc(nv, 0);
  m->hemisphere = geti();
  std::string fname = geth();
  snprintf(m->fname.data(), STRLEN, "%s", fname.c_str());
  for (int vno = 0; vno < nv; vno++) {
    m->vertices[vno].annotation = geti();
    m->vertices[vno].ripflag = geti();
  }
  m->ct = ann_ctab();
  COLOR_TABLE *user = ann_ctab();
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    out += op + " ";
    fflush(stdout);
    const char *nm = name_tok();
    if (op == "w") {
      int writect = geti();
      oi(MRISwriteAnnotation(m, nm, writect != 0));
    } else if (op == "r") {
      int danum = geti();
      int usect = geti();
      oi(MRISreadAnnotation(m, nm, danum, usect ? user : NULL));
    } else fail("ann op");
    fflush(stdout);
  }
  out += "| ";
  for (int vno = 0; vno < nv; vno++) oi(m->vertices[vno].annotation);
  ann_dump_ctab(m->ct);
  out += "| ";
  std::vector<std::string> files;
  list_files(".", "", files);
  std::sort(files.begin(), files.end());
  for (const std::string &name : files) dump_crv_file(name);
}

// ---- relay 37 of track D: mpx, group (e) (queue items 3.1-49/50) ------------------------------

// mpx: a surface read by MRISread (d/lh.s0), some vertices ripped, its radius, its canonical
// positions given or not, per-vertex overrides (p current, o original, w white, n normal, e the
// tangent axes), Gdiag's write and verbose bits or none (set after the read), the surface's volume
// geometry kept or the case's, one volume of the case, a Talairach transform (LTAalloc) and its
// matrix or not; then 1 to 4 of MRIStalairachTransform in place or into a clone (`tal`), one of
// the seven vertex-to-voxel conversions (`v2v`), mrisComputeNormalDotDistribution (`ndd`),
// mrisComputePrincipalCurvatureDistributions (`pcd`; `G` for a volume it would read outside),
// mrisSampleAshburnerTriangleEnergy (`ash`); with Gdiag's bits the work folder's files by name,
// length and FNV-1a hash
static uint64_t mpx_fnv64(const std::string &b)
{
  uint64_t h = 0xcbf29ce484222325ULL;
  for (unsigned char c : b) {
    h ^= c;
    h *= 0x100000001b3ULL;
  }
  return h;
}

static void mpx_ohash(uint64_t v)
{
  char b[24];
  snprintf(b, sizeof b, "%016" PRIx64 " ", v);
  out += b;
}

static void mpx_dump_surface(MRIS *m)
{
  for (int vno = 0; vno < m->nvertices; vno++) {
    VERTEX *v = &m->vertices[vno];
    of(v->x); of(v->y); of(v->z);
  }
  for (float x : {m->xlo, m->ylo, m->zlo, m->xhi, m->yhi, m->zhi, m->xctr, m->yctr, m->zctr}) of(x);
  oi(m->dist_alloced_flags);
}

static void mpx_dump_histo(HISTOGRAM *h)
{
  oi(h->nbins);
  of(h->bin_size);
  for (int b = 0; b < h->nbins; b++) { of(h->bins[b]); of(h->counts[b]); }
}

void mpx_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  setenv("SURFER_SIDEDOOR", "1", 1);
  std::string bytes = geth();
  int nv = geti(), nf = geti();
  int nrip = geti();
  std::vector<int> rips;
  for (int k = 0; k < nrip; k++) rips.push_back(geti());
  FILE *fp = fopen("d/lh.s0", "wb");
  if (!fp) fail("the surface file");
  fwrite(bytes.data(), 1, bytes.size(), fp);
  fclose(fp);
  errno = 0;
  MRIS *s = MRISread("d/lh.s0");
  if (!s) { out += "NULL "; return; }
  if (nv != s->nvertices || nf != s->nfaces) { oi(s->nvertices); oi(s->nfaces); out += "COUNTS "; return; }
  for (int vno : rips) s->vertices[vno].ripflag = 1;
  s->radius = getd();
  if (geti())
    for (int vno = 0; vno < nv; vno++) {
      VERTEX *v = &s->vertices[vno];
      v->cx = getf(); v->cy = getf(); v->cz = getf();
    }
  int nover = geti();
  for (int k = 0; k < nover; k++) {
    VERTEX *v = &s->vertices[geti()];
    std::string kind = next();
    if (kind == "p") { v->x = getf(); v->y = getf(); v->z = getf(); }
    else if (kind == "o") { v->origx = getf(); v->origy = getf(); v->origz = getf(); }
    else if (kind == "w") { v->whitex = getf(); v->whitey = getf(); v->whitez = getf(); }
    else if (kind == "n") { v->nx = getf(); v->ny = getf(); v->nz = getf(); }
    else if (kind == "e") {
      v->e1x = getf(); v->e1y = getf(); v->e1z = getf();
      v->e2x = getf(); v->e2y = getf(); v->e2z = getf();
    } else fail("mpx override");
  }
  int gd = geti();
  if (geti()) {
    s->vg.valid = geti();
    s->vg.width = geti(); s->vg.height = geti(); s->vg.depth = geti();
    fli_geom(&s->vg);
  }
  int type = geti(), w = geti(), h = geti(), d = geti();
  MRI *vol = MRIalloc(w, h, d, type);
  fli_geom(vol);
  MRIreInitCache(vol);
  if (geti()) s->lta = LTAalloc(1, NULL);
  if (geti()) {
    MATRIX *m = MatrixAlloc(4, 4, MATRIX_REAL);
    for (int r = 1; r <= 4; r++)
      for (int c = 1; c <= 4; c++) *MATRIX_RELT(m, r, c) = getf();
    s->SRASToTalSRAS_ = m;
  }
  if (gd) Gdiag |= DIAG_WRITE | DIAG_VERBOSE;
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    out += op + " ";
    fflush(stdout);
    if (op == "tal") {
      if (geti() == 0) {
        MRIStalairachTransform(s, s);
        oi(1);
        mpx_dump_surface(s);
      } else {
        MRIS *c = MRIStalairachTransform(s, NULL);
        oi(1);
        mpx_dump_surface(c);
        oi(c->lta != NULL);
      }
    } else if (op == "v2v") {
      int kind = geti(), vno = geti();
      VERTEX *v = &s->vertices[vno];
      double x = -7, y = -7, z = -7;
      if (kind == 0) MRISvertexToVoxel(s, v, vol, &x, &y, &z);
      else if (kind == 1) MRISvertexToVoxelNotCached(s, v, vol, &x, &y, &z);
      else if (kind == 2) MRISvertexNormalToVoxel(s, v, vol, &x, &y, &z);
      else if (kind == 3) oi(MRISvertexNormalToVoxelScaled(s, v, vol, &x, &y, &z));
      else if (kind == 4) { int which = geti(); MRISvertexCoordToVoxel(s, v, vol, which, &x, &y, &z); }
      else if (kind == 5) MRISorigVertexToVoxel(s, v, vol, &x, &y, &z);
      else MRISwhiteVertexToVoxel(s, v, vol, &x, &y, &z);
      od(x); od(y); od(z);
    } else if (op == "ndd") {
      HISTOGRAM *hd = HISTOalloc(geti());
      oi(mrisComputeNormalDotDistribution(s, hd));
      mpx_dump_histo(hd);
      for (int vno = 0; vno < nv; vno++) {
        VERTEX *v = &s->vertices[vno];
        for (float x : {v->x, v->y, v->z, v->tx, v->ty, v->tz, v->nx, v->ny, v->nz, v->area}) of(x);
      }
      of(s->total_area);
    } else if (op == "pcd") {
      int n1 = geti(), n2 = geti(), pw = geti(), ph = geti(), pd = geti(), pt = geti();
      if (pw < 100 || ph < 100 || pt != MRI_FLOAT) { out += "G "; continue; }
      HISTOGRAM *h1 = HISTOalloc(n1), *h2 = HISTOalloc(n2);
      MRI *m = MRIalloc(pw, ph, pd, pt);
      oi(mrisComputePrincipalCurvatureDistributions(s, h1, h2, m));
      mpx_dump_histo(h1);
      mpx_dump_histo(h2);
      of(m->xsize); of(m->ysize); of(m->xstart); of(m->ystart);
      std::string b;
      for (int z = 0; z < pd; z++)
        for (int y = 0; y < ph; y++)
          for (int x = 0; x < pw; x++) {
            float f = MRIgetVoxVal(m, x, y, z, 0);
            b.append((const char *)&f, 4);
          }
      mpx_ohash(mpx_fnv64(b));
      for (int vno = 0; vno < nv; vno++) { of(s->vertices[vno].k1); of(s->vertices[vno].k2); }
    } else if (op == "ash") {
      int vno = geti();
      INTEGRATION_PARMS *parms = (INTEGRATION_PARMS *)calloc(1, sizeof(INTEGRATION_PARMS));
      parms->l_ashburner_lambda = getf();
      float a = getf(), b = getf(), c = getf();
      of(mrisSampleAshburnerTriangleEnergy(s, vno, parms, a, b, c));
      VERTEX *v = &s->vertices[vno];
      of(v->x); of(v->y); of(v->z);
    } else fail("mpx op");
  }
  if (gd) {
    out += "| ";
    std::vector<std::string> files;
    list_files(".", "", files);
    std::sort(files.begin(), files.end());
    for (const std::string &name : files) {
      oh(name.c_str());
      FILE *f = fopen(name.c_str(), "rb");
      if (!f) { out += "NOFILE "; continue; }
      std::string b;
      char buf[4096];
      size_t n;
      while ((n = fread(buf, 1, sizeof buf, f)) > 0) b.append(buf, n);
      fclose(f);
      oi((long long)b.size());
      mpx_ohash(mpx_fnv64(b));
    }
  }
}

// ---- relay 37 of track D: lbl, label.cpp's surface-label functions --------------------------

// lbl: a surface read by MRISread (d/lh.s0), rip flags and marks, SUBJECTS_DIR set or not, a label
// (LabelAlloc, its points from the case, a subject name or none, a vertex index table with the
// surface handle, a template volume); then 1 to 4 of LabelMark (mk), LabelMark2 (mk2),
// LabelUnmark (um), LabelMarkSurface (ms), LabelArea (ar), LabelFillMarked (fm), LabelErode (er),
// LabelDilate (di), MRISlabel2Mask (l2m), MRISlabelInvert (inv); `G` where FreeSurfer would read or
// write outside the surface's vertices or the label's places; after each the label (every place),
// the marks and the index table
static bool lbl_outside(const std::string &op, LABEL *a, int nv)
{
  bool live_out = false, beyond = false, any_out = false, negative = false;
  for (int k = 0; k < a->n_points; k++) {
    int v = a->lv[k].vno;
    bool out_ = v < 0 || v >= nv;
    if (a->lv[k].deleted == 0 && out_) live_out = true;
    if (v >= nv) beyond = true;
    if (out_) any_out = true;
    if (v < 0) negative = true;
  }
  if (op == "mk" || op == "mk2") return live_out;
  if (op == "um") return beyond;
  if (op == "ar" || op == "fm") return any_out;
  if (op == "er") return live_out || beyond;
  if (op == "di") return live_out || beyond || a->max_points == 0;
  if (op == "l2m" || op == "inv" || op == "rh" || op == "rhi") return negative;
  if (op == "ri") return any_out;
  return false;
}

static void lbl_dump(LABEL *a, MRIS *s)
{
  oi(a->n_points);
  oi(a->max_points);
  for (int k = 0; k < a->max_points; k++) {
    LV *p = &a->lv[k];
    oi(p->vno);
    of(p->x); of(p->y); of(p->z); of(p->stat);
    oi(p->deleted);
    oi(p->xv); oi(p->yv); oi(p->zv);
  }
  for (int vno = 0; vno < s->nvertices; vno++) { oi(s->vertices[vno].marked); oi(s->vertices[vno].marked2); }
  if (!a->vertex_label_ind) out += "N ";
  else for (int vno = 0; vno < s->nvertices; vno++) oi(a->vertex_label_ind[vno]);
}

static void lbl_dump_mask(MRI *m)
{
  oi(m->nframes);
  for (int f = 0; f < m->nframes; f++)
    for (int c = 0; c < m->width; c++) of(MRIgetVoxVal(m, c, 0, 0, f));
}

void lbl_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  setenv("SURFER_SIDEDOOR", "1", 1);
  std::string bytes = geth();
  int nv = geti(), nf = geti();
  int nrip = geti();
  std::vector<int> rips;
  for (int k = 0; k < nrip; k++) rips.push_back(geti());
  int nmark = geti();
  std::vector<int> marks;
  for (int k = 0; k < 3 * nmark; k++) marks.push_back(geti());
  if (geti()) setenv("SUBJECTS_DIR", workdir.c_str(), 1);
  else unsetenv("SUBJECTS_DIR");
  FILE *fp = fopen("d/lh.s0", "wb");
  if (!fp) fail("the surface file");
  fwrite(bytes.data(), 1, bytes.size(), fp);
  fclose(fp);
  errno = 0;
  MRIS *s = MRISread("d/lh.s0");
  if (!s) { out += "NULL "; return; }
  if (nv != s->nvertices || nf != s->nfaces) { oi(s->nvertices); oi(s->nfaces); out += "COUNTS "; return; }
  for (int vno : rips) s->vertices[vno].ripflag = 1;
  for (int k = 0; k < nmark; k++) {
    s->vertices[marks[3 * k]].marked = marks[3 * k + 1];
    s->vertices[marks[3 * k]].marked2 = marks[3 * k + 2];
  }
  // relay 38: the vertices' stat (LabelRemoveIslandsSurf copies it)
  int nst = geti();
  for (int k = 0; k < nst; k++) {
    int vno = geti();
    s->vertices[vno].stat = getf();
  }
  int max = geti(), n = geti();
  LABEL *a = LabelAlloc(max, NULL, NULL);
  a->coords = geti();
  if (geti()) strcpy(a->subject_name, "subj");
  a->n_points = n;
  for (int k = 0; k < n; k++) {
    LV *p = &a->lv[k];
    p->vno = geti();
    p->x = getf(); p->y = getf(); p->z = getf();
    p->deleted = geti();
    p->stat = getf();
  }
  if (geti()) {
    a->vertex_label_ind = (int *)calloc(nv > 0 ? nv : 1, sizeof(int));
    a->mris = s;
  }
  MRI *tpl = NULL;
  if (geti()) {
    int w = geti(), h = geti(), d = geti();
    tpl = MRIalloc(w, h, d, MRI_UCHAR);
    fli_geom(tpl);
    MRIreInitCache(tpl);
    a->mri_template = tpl;
  }
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    out += op + " ";
    fflush(stdout);
    std::vector<int> args;
    if (op == "er") args.push_back(geti());
    else if (op == "di") { args.push_back(geti()); args.push_back(geti()); }
    else if (op == "l2m") {
      int f = geti(), st = geti(), given = geti();
      int nfr = given ? geti() : 0;
      args = {f, st, given, nfr};
    }
    if (lbl_outside(op, a, nv)) { out += "G "; continue; }
    if (op == "ri" || op == "rh" || op == "rhi") {
      // relay 38: LabelRemoveIslandsSurf, LabelRemoveHolesSurf, LabelRemoveHolesAndIslandsSurf;
      // the new label, then the vertices' cluster numbers and values
      LABEL *l = op == "ri" ? LabelRemoveIslandsSurf(s, a)
                 : op == "rh" ? LabelRemoveHolesSurf(s, a) : LabelRemoveHolesAndIslandsSurf(s, a);
      if (!l) out += "NULL ";
      else { oh(l->subject_name); lbl_dump(l, s); }
      for (int v = 0; v < nv; v++) { oi(s->vertices[v].undefval); of(s->vertices[v].val); }
    } else if (op == "mk") oi(LabelMark(a, s));
    else if (op == "mk2") oi(LabelMark2(a, s));
    else if (op == "um") oi(LabelUnmark(a, s));
    else if (op == "ms") oi(LabelMarkSurface(a, s));
    else if (op == "ar") od(LabelArea(a, s));
    else if (op == "fm") oi(LabelFillMarked(a, s));
    else if (op == "er") oi(LabelErode(a, s, args[0]));
    else if (op == "di") oi(LabelDilate(a, s, args[0], args[1]));
    else if (op == "l2m") {
      MRI *given = args[2] ? MRIallocSequence(nv, 1, 1, MRI_FLOAT, args[3]) : NULL;
      MRI *r = MRISlabel2Mask(s, a, given, args[0], args[1]);
      if (!r) out += "NULL ";
      else lbl_dump_mask(r);
    } else if (op == "inv") {
      LABEL *l = MRISlabelInvert(s, a);
      if (!l) out += "NULL ";
      else { oh(l->subject_name); lbl_dump(l, s); }
    } else fail("lbl op");
    lbl_dump(a, s);
  }
}

// ---- relay 38 of track D: scl, surfcluster.cpp whole --------------------------------------------

// scl: a surface read by MRISread (d/lh.s0), every vertex's value from the case, the group-average
// areas and their flags, now and then preset cluster numbers, Gdiag_no, FS_CLUSTER_USE_AVG_VERTEX_AREA
// and USE_FAST_SURF_SMOOTHER; then 1 to 5 of surfcluster.cpp's functions on the case's current
// cluster list; `G` where FreeSurfer would read or write outside its arrays (FS-903 to FS-905) or
// past a cluster list's end; after each the vertices' cluster numbers, values and annotations, the
// list (every field) and errno
static void scl_dump_list(SCS *l, int n)
{
  oi(n);
  for (int k = 0; k < n; k++) {
    SCS *c = &l[k];
    oi(c->clusterno); oi(c->nmembers); of(c->area); of(c->weightvtx); of(c->weightarea);
    of(c->maxval); oi(c->vtxmaxval); of(c->x); of(c->y); of(c->z); of(c->xxfm); of(c->yxfm);
    of(c->zxfm); of(c->cx); of(c->cy); of(c->cz); of(c->cxxfm); of(c->cyxfm); of(c->czxfm);
    od(c->pval_clusterwise); od(c->pval_clusterwise_low); od(c->pval_clusterwise_hi);
  }
}

static MATRIX *scl_xfm(int kind)
{
  if (kind == 0) return NULL;
  int rows = kind == 1 ? 4 : 3;
  MATRIX *m = MatrixAlloc(rows, 4, MATRIX_REAL);
  for (int r = 1; r <= rows; r++)
    for (int c = 1; c <= 4; c++) m->rptr[r][c] = getf();
  return m;
}

static MRI *scl_map(int nv)
{
  if (!geti()) return NULL;
  MRI *m = MRIallocSequence(nv, 1, 1, MRI_FLOAT, 1);
  for (int v = 0; v < nv; v++) MRIsetVoxVal(m, v, 0, 0, 0, getf());
  return m;
}

void scl_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  setenv("SURFER_SIDEDOOR", "1", 1);
  // sclustAnnot's colour table draws random colours
  setenv("FREESURFER_SEED", "1234", 1);
  std::string bytes = geth();
  int nv = geti(), nf = geti();
  Gdiag_no = geti();
  std::string avg = next(), ufss = next();
  if (avg != "-") setenv("FS_CLUSTER_USE_AVG_VERTEX_AREA", avg.c_str(), 1);
  if (ufss != "-") setenv("USE_FAST_SURF_SMOOTHER", ufss.c_str(), 1);
  FILE *fp = fopen("d/lh.s0", "wb");
  if (!fp) fail("the surface file");
  fwrite(bytes.data(), 1, bytes.size(), fp);
  fclose(fp);
  errno = 0;
  int gd = Gdiag_no;
  Gdiag_no = -1;
  MRIS *s = MRISread("d/lh.s0");
  Gdiag_no = gd;
  if (!s) { out += "NULL "; return; }
  if (nv != s->nvertices || nf != s->nfaces) { oi(s->nvertices); oi(s->nfaces); out += "COUNTS "; return; }
  for (int v = 0; v < nv; v++) s->vertices[v].val = getf();
  s->group_avg_vtxarea_loaded = geti();
  s->group_avg_surface_area = getf();
  if (geti()) s->total_area = getf();
  if (geti()) for (int v = 0; v < nv; v++) s->vertices[v].group_avg_area = getf();
  if (geti()) for (int v = 0; v < nv; v++) s->vertices[v].undefval = geti();
  SCS *list = NULL;
  int nlist = 0;
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    out += op + " ";
    fflush(stdout);
    errno = 0;
    bool outside = false, beyond = false, nanp = false;
    for (int v = 0; v < nv; v++) {
      int c = s->vertices[v].undefval;
      if (c < 0) outside = true;
      if (c > nlist) beyond = true;
    }
    for (int c = 0; c < nlist; c++)
      if (std::isnan(list[c].pval_clusterwise)) nanp = true;
    if (op == "map") {
      float thmin = getf(), thmax = getf();
      int thsign = geti();
      float minarea = getf();
      MATRIX *T = scl_xfm(geti());
      MRI *fw = scl_map(nv);
      int n = 0;
      SCS *r = sclustMapSurfClusters(s, thmin, thmax, thsign, minarea, &n, T, fw);
      oi(n);
      if (r) { free(list); list = r; nlist = n; }
      else out += "NULL ";
    } else if (op == "grow") {
      int c = geti(), seed = geti();
      float thmin = getf(), thmax = getf();
      int thsign = geti();
      oi(sclustGrowSurfCluster(c, seed, s, thmin, thmax, thsign));
    } else if (op == "area") {
      int c = geti(), n = -7;
      of(sclustSurfaceArea(c, s, &n));
      oi(n);
    } else if (op == "wt") {
      int c = geti();
      MRI *m = scl_map(nv);
      int ua = geti();
      of(sclustWeight(c, s, m, ua));
    } else if (op == "max") {
      int c = geti(), vm = -7;
      of(sclustSurfaceMax(c, s, &vm));
      oi(vm);
    } else if (op == "cen") {
      double xyz[3] = {0, 0, 0};
      oi(sclustSurfaceCentroid(geti(), s, xyz));
      od(xyz[0]); od(xyz[1]); od(xyz[2]);
    } else if (op == "zero") of(sclustZeroSurfaceClusterNo(geti(), s));
    else if (op == "zeronon") of(sclustZeroSurfaceNonClusters(s));
    else if (op == "tocn") of(sclustSetSurfaceValToClusterNo(s));
    else if (op == "cwp") {
      if (outside || beyond) { out += "G "; continue; }
      of(sclustSetSurfaceValToCWP(s, list));
    } else if (op == "count") of(sclustCountClusters(s));
    else if (op == "sum" || op == "sumold") {
      MATRIX *T = scl_xfm(geti());
      MRI *fw = op == "sum" ? scl_map(nv) : NULL;
      if (outside) { out += "G "; continue; }
      int n = -7;
      SCS *r = op == "sum" ? SurfClusterSummary(s, T, &n, fw) : SurfClusterSummaryOld(s, T, &n);
      oi(n);
      if (r) { free(list); list = r; nlist = n; }
      else out += "NULL ";
    } else if (op == "dump") oi(DumpSurfClusterSum(stdout, list, nlist));
    else if (op == "sort") {
      SCS *r = SortSurfClusterSum(list, nlist);
      free(list);
      list = r;
    } else if (op == "remap" || op == "remap0") {
      bool bad = false;
      for (int c = 0; c < nlist; c++)
        if (list[c].clusterno < 1 || list[c].clusterno > nlist) bad = true;
      if (op == "remap" && (bad || beyond)) { out += "G "; continue; }
      oi(op == "remap" ? sclustReMap(s, nlist, list) : sclustReMap0(s, nlist, list));
    } else if (op == "maxa") od(sclustMaxClusterArea(list, nlist));
    else if (op == "maxc") oi(sclustMaxClusterCount(list, nlist));
    else if (op == "maxw") of(sclustMaxClusterWeightVtx(list, nlist, geti()));
    else if (op == "pv") {
      for (int c = 0; c < 6; c++) {
        double p = getd();
        if (c < nlist) list[c].pval_clusterwise = p;
      }
    } else if (op == "prune") {
      double th = getd();
      if (outside || beyond || nanp) { out += "G "; continue; }
      int n = -7;
      SCS *r = sclustPruneByCWPval(list, nlist, th, &n, s);
      oi(n);
      free(list);
      list = r;
      nlist = n;
    } else if (op == "annot") {
      int n = geti();
      if (outside) { out += "G "; continue; }
      oi(sclustAnnot(s, n));
      COLOR_TABLE *ct = s->ct;
      oi(ct->nentries);
      for (int e = 0; e < ct->nentries; e++) {
        CTE *x = ct->entries[e];
        oh(x->name); oi(x->ri); oi(x->gi); oi(x->bi); oi(x->ai);
      }
    } else if (op == "gbd") {
      int seed = geti();
      double d = getd();
      int shape = geti(), vtxno = geti();
      std::vector<int> l(nv > 0 ? nv : 1, -2);
      oi(sclustGrowByDist(s, seed, d, shape, vtxno, l.data()));
      for (int v = 0; v < nv; v++) oi(l[v]);
    } else if (op == "pts") {
      std::string name = next();
      oi(sclustSaveAsPointSet((char *)name.c_str(), list, nlist, s));
      FILE *f = fopen(name.c_str(), "rb");
      if (!f) out += "NOFILE ";
      else {
        std::string b;
        char buf[4096];
        size_t n;
        while ((n = fread(buf, 1, sizeof buf, f)) > 0) b.append(buf, n);
        fclose(f);
        ox(b);
      }
    } else fail("scl op");
    for (int v = 0; v < nv; v++) {
      oi(s->vertices[v].undefval); of(s->vertices[v].val); oi(s->vertices[v].annotation);
    }
    scl_dump_list(list, nlist);
    oi(errno);
  }
}

// ---- relay 38 of track D: vcp, the copies between vertex fields (mrisurf_vals.cpp) and the
// fields written through the curvature (MRISwriteD, MRISwriteMarked, MRISwriteArea, MRISreadMarked)

// vcp: a surface read by MRISread (d/lh.s0), 0 to 3 vertices ripped, every vertex's 13 float and 6
// integer fields and every face's mark from the case; then 1 to 6 operations, each named by its
// FreeSurfer function; `G` where FreeSurfer would read outside the caller's vector or matrix
// (FS-908); after each the fields and the face marks; at the end every file in the folder
static void vcp_dump(MRIS *s)
{
  for (int v = 0; v < s->nvertices; v++) {
    VERTEX *x = &s->vertices[v];
    of(x->val); of(x->curv); of(x->stat); of(x->mean); of(x->mean_imag); of(x->std_error);
    of(x->imag_val); of(x->val2); of(x->val2bak); of(x->valbak); of(x->d); of(x->area);
    of(x->origarea);
    oi(x->marked); oi(x->marked2); oi(x->marked3); oi(x->cropped); oi(x->fixedval);
    oi(x->annotation);
  }
  for (int f = 0; f < s->nfaces; f++) oi(s->faces[f].marked);
}

void vcp_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  setenv("SURFER_SIDEDOOR", "1", 1);
  std::string bytes = geth();
  int nv = geti(), nf = geti();
  FILE *fp = fopen("d/lh.s0", "wb");
  if (!fp) fail("the surface file");
  fwrite(bytes.data(), 1, bytes.size(), fp);
  fclose(fp);
  errno = 0;
  MRIS *s = MRISread("d/lh.s0");
  if (!s) { out += "NULL "; return; }
  if (nv != s->nvertices || nf != s->nfaces) { oi(s->nvertices); oi(s->nfaces); out += "COUNTS "; return; }
  int nrip = geti();
  for (int k = 0; k < nrip; k++) s->vertices[geti()].ripflag = 1;
  for (int v = 0; v < nv; v++) {
    VERTEX *x = &s->vertices[v];
    x->val = getf(); x->curv = getf(); x->stat = getf(); x->mean = getf(); x->mean_imag = getf();
    x->std_error = getf(); x->imag_val = getf(); x->val2 = getf(); x->val2bak = getf();
    x->valbak = getf(); x->d = getf(); x->area = getf(); x->origarea = getf();
    x->marked = geti(); x->marked2 = geti(); x->marked3 = geti(); x->cropped = geti();
    x->fixedval = geti(); x->annotation = geti();
  }
  for (int f = 0; f < nf; f++) s->faces[f].marked = geti();
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    out += op + " ";
    fflush(stdout);
    if (op == "MRISsetCroppedToZero") oi(MRISsetCroppedToZero(s));
    else if (op == "MRISaddToValues") oi(MRISaddToValues(s, getf()));
    else if (op == "MRISimportCurvatureVector" || op == "MRISimportValVector") {
      int n = geti();
      std::vector<float> a(n > 0 ? n : 1);
      for (int i = 0; i < n; i++) a[i] = getf();
      if (n < nv) { out += "G "; continue; }
      oi(op == "MRISimportValVector" ? MRISimportValVector(s, a.data()) : MRISimportCurvatureVector(s, a.data()));
    } else if (op == "MRISimportValFromMatrixColumn") {
      int rows = geti(), cols = geti(), col = geti();
      MATRIX *m = MatrixAlloc(rows, cols, MATRIX_REAL);
      for (int r = 1; r <= rows; r++)
        for (int c = 1; c <= cols; c++) m->rptr[r][c] = getf();
      if (rows < nv || col < 1 || col > cols) { out += "G "; continue; }
      oi(MRISimportValFromMatrixColumn(s, m, col));
    } else if (op == "MRISexportValVector") {
      std::vector<float> a(nv > 0 ? nv : 1);
      oi(MRISexportValVector(s, a.data()));
      for (int i = 0; i < nv; i++) of(a[i]);
    } else if (op == "MRISexportValVectorDouble") {
      int off = geti();
      std::vector<double> a(nv + off > 0 ? nv + off : 1, -3.0);
      oi(MRISexportValVectorDouble(s, a.data(), off));
      for (int i = 0; i < nv + off; i++) od(a[i]);
    } else if (op == "MRIScopyMarkedToMarked2") oi(MRIScopyMarkedToMarked2(s));
    else if (op == "MRIScopyMarked2ToMarked") oi(MRIScopyMarked2ToMarked(s));
    else if (op == "MRIScopyMarkedToMarked3") oi(MRIScopyMarkedToMarked3(s));
    else if (op == "MRIScopyMarked3ToMarked") oi(MRIScopyMarked3ToMarked(s));
    else if (op == "MRIScopyValsToAnnotations") oi(MRIScopyValsToAnnotations(s));
    else if (op == "MRIScopyMeansToValues") oi(MRIScopyMeansToValues(s));
    else if (op == "MRIScopyImaginaryMeansToValues") oi(MRIScopyImaginaryMeansToValues(s));
    else if (op == "MRIScopyStandardErrorsToValues") oi(MRIScopyStandardErrorsToValues(s));
    else if (op == "MRIScopyStatsToValues") oi(MRIScopyStatsToValues(s));
    else if (op == "MRIScopyStatsFromValues") oi(MRIScopyStatsFromValues(s));
    else if (op == "MRIScopyValuesToImagValues") oi(MRIScopyValuesToImagValues(s));
    else if (op == "MRIScopyImagValuesToValues") oi(MRIScopyImagValuesToValues(s));
    else if (op == "MRIScopyVal2ToVal") oi(MRIScopyVal2ToVal(s));
    else if (op == "MRIScopyCurvatureToValues") oi(MRIScopyCurvatureToValues(s));
    else if (op == "MRIScopyVal2BakToVal") oi(MRIScopyVal2BakToVal(s));
    else if (op == "MRIScopyCurvatureToImagValues") oi(MRIScopyCurvatureToImagValues(s));
    else if (op == "MRIScopyValuesToCurvature") oi(MRIScopyValuesToCurvature(s));
    else if (op == "MRIScopyImagValuesToCurvature") oi(MRIScopyImagValuesToCurvature(s));
    else if (op == "MRIScopyCurvatureFromValues") oi(MRIScopyCurvatureFromValues(s));
    else if (op == "MRIScopyCurvatureFromImagValues") oi(MRIScopyCurvatureFromImagValues(s));
    else if (op == "MRIScopyValToVal2") oi(MRIScopyValToVal2(s));
    else if (op == "MRIScopyValuesToVal2Bak") oi(MRIScopyValuesToVal2Bak(s));
    else if (op == "MRIScopyValToValBak") oi(MRIScopyValToValBak(s));
    else if (op == "MRIScopyValToVal2Bak") oi(MRIScopyValToVal2Bak(s));
    else if (op == "MRIScopyFromCropped") oi(MRIScopyFromCropped(s, geti()));
    else if (op == "MRIScopyToCropped") oi(MRIScopyToCropped(s, geti()));
    else if (op == "MRISorigAreaToCurv") oi(MRISorigAreaToCurv(s));
    else if (op == "MRISareaToCurv") oi(MRISareaToCurv(s));
    else if (op == "MRISmarkedToCurv") oi(MRISmarkedToCurv(s));
    else if (op == "MRIScurvToMarked") oi(MRIScurvToMarked(s));
    else if (op == "MRISdToCurv") oi(MRISdToCurv(s));
    else if (op == "MRIScurvToD") oi(MRIScurvToD(s));
    else if (op == "MRIScopyFixedValFlagsToMarks") oi(MRIScopyFixedValFlagsToMarks(s));
    else if (op == "MRISclearFaceMarks") oi(MRISclearFaceMarks(s));
    else if (op == "MRISwriteD") oi(MRISwriteD(s, next()));
    else if (op == "MRISwriteMarked") oi(MRISwriteMarked(s, next()));
    else if (op == "MRISwriteArea") oi(MRISwriteArea(s, next()));
    else if (op == "MRISreadMarked") oi(MRISreadMarked(s, next()));
    else fail("vcp op");
    oi(errno);
    vcp_dump(s);
  }
  out += "| ";
  std::vector<std::string> files;
  list_files(".", "", files);
  std::sort(files.begin(), files.end());
  for (const std::string &name : files) {
    oh(name.c_str());
    FILE *f = fopen(name.c_str(), "rb");
    if (!f) { out += "NOFILE "; continue; }
    std::string b;
    char buf[4096];
    size_t n;
    while ((n = fread(buf, 1, sizeof buf, f)) > 0) b.append(buf, n);
    fclose(f);
    ox(b);
  }
}

// ---- relay 39 of track D: mht, the surface hash table (mrishash.cpp's tables and face tests),
// mrisMarkIntersections, MRISfillHoles, MRISmarkEdge's hole filling ----------------------------------

// a coordinate's table voxel (x / res + 1000, truncated), false when the int conversion is undefined
static bool mht_vox(double x, float res, int *v)
{
  double w = x / res + TABLE_CENTER;
  if (!(w > -2147483649.0 && w < 2147483648.0)) return false;
  *v = (int)w;
  return true;
}

// the table's buckets in the box of the vertices' voxels (every vertex's `which` position, the
// defined voxels clamped into the table, one voxel of margin): each existing bucket's voxel, count
// and list; BOX <n> for a box of more than 400,000 voxels
static void mht_dump(MRIS *s, MRIS_HASH_TABLE *mht)
{
  if (!mht) { out += "NOMHT "; return; }
  oi(mht->which()); of(mht->vres()); oi((int)mht->fno_usage());
  int lo[3] = {TABLE_SIZE, TABLE_SIZE, TABLE_SIZE}, hi[3] = {-1, -1, -1};
  for (int v = 0; v < s->nvertices; v++) {
    float c[3];
    MRISvertexCoord2XYZ_float(&s->vertices[v], mht->which(), &c[0], &c[1], &c[2]);
    for (int k = 0; k < 3; k++) {
      int x;
      if (!mht_vox(c[k], mht->vres(), &x)) continue;
      if (x < 0) x = 0;
      if (x >= TABLE_SIZE) x = TABLE_SIZE - 1;
      if (x < lo[k]) lo[k] = x;
      if (x > hi[k]) hi[k] = x;
    }
  }
  long long vol = 1;
  for (int k = 0; k < 3; k++) {
    if (hi[k] < 0) { out += "EMPTYBOX "; return; }
    lo[k] = std::max(0, lo[k] - 1);
    hi[k] = std::min(TABLE_SIZE - 1, hi[k] + 1);
    vol *= hi[k] - lo[k] + 1;
  }
  for (int k = 0; k < 3; k++) { oi(lo[k]); oi(hi[k]); }
  if (vol > 400000) { out += "BOX "; oi(vol); return; }
  for (int x = lo[0]; x <= hi[0]; x++)
    for (int y = lo[1]; y <= hi[1]; y++)
      for (int z = lo[2]; z <= hi[2]; z++) {
        MHBT *b = MHTacqBucketAtVoxIx(mht, x, y, z);
        if (!b) continue;
        oi(x); oi(y); oi(z); oi(b->nused);
        for (int i = 0; i < b->nused; i++) oi(b->bins[i].fno);
        MHTrelBucket(&b);
      }
  out += "; ";
}

static void mht_vertices(MRIS *s)
{
  for (int v = 0; v < s->nvertices; v++) {
    oi(s->vertices[v].marked); of(s->vertices[v].val); oi(s->vertices[v].undefval);
  }
}

void mht_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  setenv("SURFER_SIDEDOOR", "1", 1);
  std::string bytes = geth();
  int nv = geti(), nf = geti();
  int gdiag_no = geti();
  FILE *fp = fopen("d/lh.s0", "wb");
  if (!fp) fail("the surface file");
  fwrite(bytes.data(), 1, bytes.size(), fp);
  fclose(fp);
  errno = 0;
  Gdiag_no = -1;
  MRIS *s = MRISread("d/lh.s0");
  if (!s) { out += "NULL "; return; }
  if (nv != s->nvertices || nf != s->nfaces) { oi(s->nvertices); oi(s->nfaces); out += "COUNTS "; return; }
  Gdiag_no = gdiag_no;
  int nfrip = geti();
  for (int k = 0; k < nfrip; k++) s->faces[geti()].ripflag = 1;
  int nvrip = geti();
  for (int k = 0; k < nvrip; k++) s->vertices[geti()].ripflag = 1;
  if (geti()) for (int v = 0; v < nv; v++) s->vertices[v].marked = geti();
  if (geti())
    for (int v = 0; v < nv; v++) {
      s->vertices[v].origx = getf(); s->vertices[v].origy = getf(); s->vertices[v].origz = getf();
    }
  MRIS_HASH_TABLE *mht = NULL;
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    out += op + " ";
    fflush(stdout);
    errno = 0;
    if (op == "ft" || op == "vt") {
      int which = geti();
      float res = getf();
      MHTfree(&mht);
      mht = op == "ft" ? MHTcreateFaceTable_Resolution(s, which, res) : MHTcreateVertexTable_Resolution(s, which, res);
      mht_dump(s, mht);
    } else if (op == "ftd") {
      MHTfree(&mht);
      mht = MHTcreateFaceTable(s);
      mht_dump(s, mht);
    } else if (op == "dfi") {
      if (!mht) { out += "G "; continue; }
      for (int f = 0; f < nf; f++) oi(MHTdoesFaceIntersect(mht, s, f));
    } else if (op == "ivf") {
      int vno = geti();
      float dx = getf(), dy = getf(), dz = getf();
      if (!mht) { out += "G "; continue; }
      oi(MHTisVectorFilled(mht, vno, dx, dy, dz));
    } else if (op == "add" || op == "rem") {
      int vno = geti();
      if (!mht) { out += "G "; continue; }
      oi(op == "add" ? MHTaddAllFaces(mht, s, vno) : MHTremoveAllFaces(mht, s, vno));
      mht_dump(s, mht);
    } else if (op == "mv") {
      int vno = geti();
      // moved by the case's amounts (relative, so its faces stay small: FreeSurfer's list search is
      // quadratic in a face's voxels, and the official probe's alarm is 5 seconds)
      s->vertices[vno].x += getf(); s->vertices[vno].y += getf(); s->vertices[vno].z += getf();
    } else if (op == "tis") {
      oi(MHTtestIsMRISselfIntersecting(s, getf()));
    } else if (op == "mi") {
      oi(mrisMarkIntersections(s, geti()));
      mht_vertices(s);
    } else if (op == "fh") {
      std::string field = next();
      int usemri = geti();
      double thresh = getd();
      MRI *m = NULL;
      if (usemri) {
        m = MRIallocSequence(nv, 1, 1, MRI_FLOAT, 1);
        for (int v = 0; v < nv; v++) MRIsetVoxVal(m, v, 0, 0, 0, getf());
      }
      oi(MRISfillHoles(s, field == "-" ? NULL : (char *)field.c_str(), m, thresh));
      mht_vertices(s);
      if (m) {
        for (int v = 0; v < nv; v++) of(MRIgetVoxVal(m, v, 0, 0, 0));
        MRIfree(&m);
      }
    } else if (op == "me") {
      int id = geti();
      double th = getd();
      int fill = geti();
      oi(MRISmarkEdge(s, NULL, id, th, fill));
      mht_vertices(s);
    } else if (op == "free") {
      MHTfree(&mht);
      out += mht ? "SET " : "FREED ";
    } else fail("mht op");
    oi(errno);
  }
}

// ---- relay 37 of track D: wvl, the values written (MRISwriteValues, MRISwriteAreaErrorToValFile,
// MRIcopyMRIS) ------------------------------------------------------------------------------------

static void wvl_dump_vol(MRI *m)
{
  oi(m->nframes);
  for (int f = 0; f < m->nframes; f++)
    for (int s = 0; s < m->depth; s++)
      for (int r = 0; r < m->height; r++)
        for (int c = 0; c < m->width; c++) of(MRIgetVoxVal(m, c, r, s, f));
}

void wvl_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  setenv("SURFER_SIDEDOOR", "1", 1);
  std::string bytes = geth();
  int nv = geti(), nf = geti();
  int nrip = geti();
  std::vector<int> rips;
  for (int k = 0; k < nrip; k++) rips.push_back(geti());
  int gd = geti();
  FILE *fp = fopen("d/lh.s0", "wb");
  if (!fp) fail("the surface file");
  fwrite(bytes.data(), 1, bytes.size(), fp);
  fclose(fp);
  errno = 0;
  MRIS *s = MRISread("d/lh.s0");
  if (!s) { out += "NULL "; return; }
  if (nv != s->nvertices || nf != s->nfaces) { oi(s->nvertices); oi(s->nfaces); out += "COUNTS "; return; }
  for (int vno : rips) s->vertices[vno].ripflag = 1;
  if (gd & 1) Gdiag |= DIAG_SHOW;
  if (gd & 2) Gdiag |= DIAG_VERBOSE;
  if (geti())
    for (int vno = 0; vno < nv; vno++) s->vertices[vno].val = getf();
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    out += op + " ";
    fflush(stdout);
    if (op == "sm") {
      oi(MRISstoreMetricProperties(s));
    } else if (op == "wv" || op == "ae") {
      std::string name = geth();
      oi(op == "wv" ? MRISwriteValues(s, name.c_str()) : MRISwriteAreaErrorToValFile(s, name.c_str()));
    } else if (op == "cm") {
      int frame = geti();
      std::string field = geth();
      MRI *given = NULL;
      if (geti()) {
        int w = geti(), h = geti(), d = geti(), nfr = geti();
        given = MRIallocSequence(w, h, d, MRI_FLOAT, nfr);
      }
      MRI *r = MRIcopyMRIS(given, s, frame, field.c_str());
      if (!r) out += "NULL ";
      else wvl_dump_vol(r);
    } else fail("wvl op");
  }
  out += "| ";
  for (int vno = 0; vno < nv; vno++) of(s->vertices[vno].val);
  std::vector<std::string> files;
  list_files(".", "", files);
  std::sort(files.begin(), files.end());
  for (const std::string &name : files) {
    oh(name.c_str());
    FILE *f = fopen(name.c_str(), "rb");
    if (!f) { out += "NOFILE "; continue; }
    std::string b;
    char buf[4096];
    size_t n;
    while ((n = fread(buf, 1, sizeof buf, f)) > 0) b.append(buf, n);
    fclose(f);
    ox(b);
  }
}

// ---- relay 40 of track D: vls, mrisurf_vals.cpp's marks, settings, rip flags, face tests and
// filters (valmarks.rs, valfilter.rs)

// vls: a surface read by MRISread (d/lh.s0), now and then its neighbourhood set to 2 hops
// (MRISsetNeighborhoodSize), 0 to 3 vertices ripped, Gdiag's show and verbose bits or not, the
// extra gradients or not, every vertex's 19 float and 5 integer fields (and its extra gradients)
// and every face's mark from the case, a label of 0 to 6 points; then 1 to 6 operations, each
// named by its FreeSurfer function; `G` where FreeSurfer would write outside the vertices for a
// label point (FS-917); after each the vertices' fields, the faces' marks and the curvature extremes
static void vls_dump(MRIS *s)
{
  for (int v = 0; v < s->nvertices; v++) {
    VERTEX *x = &s->vertices[v];
    of(x->val); of(x->val2); of(x->val2bak); of(x->valbak); of(x->curv); of(x->d); of(x->stat);
    of(x->dx); of(x->dy); of(x->dz); of(x->odx); of(x->ody); of(x->odz); of(x->tx); of(x->tdx);
    oi(x->ripflag); oi(x->marked); oi(x->marked2); oi(x->flags); oi(x->fixedval); oi(x->annotation);
    if (s->dx2) { of(s->dx2[v]); of(s->dy2[v]); of(s->dz2[v]); }
  }
  for (int f = 0; f < s->nfaces; f++) oi(s->faces[f].marked);
  of(s->max_curv); of(s->min_curv);
}

// after the long-range sampling: each vertex's neighbours in use, their size and original
// distances; the surface's neighbourhood size, mean neighbour count and flag
static void vls_topo_dump(MRIS *s)
{
  for (int v = 0; v < s->nvertices; v++) {
    VERTEX_TOPOLOGY *t = &s->vertices_topology[v];
    oi(t->vtotal); oi(t->nsizeCur);
    for (int i = 0; i < t->vtotal; i++) oi(t->v[i]);
    if (s->vertices[v].dist_orig)
      for (int i = 0; i < t->vtotal; i++) of(s->vertices[v].dist_orig[i]);
    else out += "N ";
  }
  oi(s->nsize); of(s->avg_nbrs); oi(s->vtotalsMightBeTooBig);
}

void vls_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  setenv("SURFER_SIDEDOOR", "1", 1);
  std::string bytes = geth();
  int nv = geti(), nf = geti();
  FILE *fp = fopen("d/lh.s0", "wb");
  if (!fp) fail("the surface file");
  fwrite(bytes.data(), 1, bytes.size(), fp);
  fclose(fp);
  errno = 0;
  MRIS *s = MRISread("d/lh.s0");
  if (!s) { out += "NULL "; return; }
  if (nv != s->nvertices || nf != s->nfaces) { oi(s->nvertices); oi(s->nfaces); out += "COUNTS "; return; }
  int nsize = geti();
  if (nsize > 1) MRISsetNeighborhoodSize(s, nsize);
  int nrip = geti();
  for (int k = 0; k < nrip; k++) s->vertices[geti()].ripflag = 1;
  if (geti()) Gdiag |= DIAG_SHOW | DIAG_VERBOSE;
  bool extra = geti() != 0;
  if (extra) {
    s->dx2 = (float *)calloc(nv, sizeof(float));
    s->dy2 = (float *)calloc(nv, sizeof(float));
    s->dz2 = (float *)calloc(nv, sizeof(float));
  }
  for (int v = 0; v < nv; v++) {
    VERTEX *x = &s->vertices[v];
    x->val = getf(); x->val2 = getf(); x->val2bak = getf(); x->valbak = getf(); x->curv = getf();
    x->d = getf(); x->stat = getf(); x->dx = getf(); x->dy = getf(); x->dz = getf();
    x->odx = getf(); x->ody = getf(); x->odz = getf();
    x->whitex = getf(); x->whitey = getf(); x->whitez = getf();
    x->pialx = getf(); x->pialy = getf(); x->pialz = getf();
    x->marked = geti(); x->marked2 = geti(); x->annotation = geti(); x->flags = geti();
    x->fixedval = geti();
    if (extra) { s->dx2[v] = getf(); s->dy2[v] = getf(); s->dz2[v] = getf(); }
  }
  for (int f = 0; f < nf; f++) s->faces[f].marked = geti();
  // relay 40: the long-range sampling's seed, diagnostic vertex, heartbeat and vertex pair
  setenv("FREESURFER_SEED", std::to_string(geti()).c_str(), 1);
  Gdiag_no = geti();
  if (geti()) Gdiag |= DIAG_HEARTBEAT;
  int vd1 = geti(), vd2 = geti();
  if (vd1 >= 0) setenv("VDIAG1", std::to_string(vd1).c_str(), 1);
  if (vd2 >= 0) setenv("VDIAG2", std::to_string(vd2).c_str(), 1);
  int npts = geti();
  LABEL *lab = LabelAlloc(npts > 0 ? npts : 1, NULL, NULL);
  lab->n_points = npts;
  bool outside = false;
  for (int i = 0; i < npts; i++) {
    lab->lv[i].vno = geti();
    if (lab->lv[i].vno < 0 || lab->lv[i].vno >= nv) outside = true;
  }
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    out += op + " ";
    fflush(stdout);
    if (op == "MRISinvertMarks") oi(MRISinvertMarks(s));
    else if (op == "MRISnotMarked") oi(MRISnotMarked(s));
    else if (op == "MRIScountMarked") oi(MRIScountMarked(s, geti()));
    else if (op == "MRISmarkedVertices") oi(MRISmarkedVertices(s));
    else if (op == "MRISsetAllMarks") oi(MRISsetAllMarks(s, geti()));
    else if (op == "MRISthresholdValIntoMarked") oi(MRISthresholdValIntoMarked(s, getf()));
    else if (op == "MRISmarkVerticesWithValOverThresh") oi(MRISmarkVerticesWithValOverThresh(s, getf()));
    else if (op == "MRISexpandMarked") oi(MRISexpandMarked(s));
    else if (op == "MRISerodeMarked") oi(MRISerodeMarked(s, geti()));
    else if (op == "MRIScloseMarked") oi(MRIScloseMarked(s, geti()));
    else if (op == "MRISopenMarked") oi(MRISopenMarked(s, geti()));
    else if (op == "MRISclearMark2s") oi(MRISclearMark2s(s));
    else if (op == "MRISsetFlags") oi(MRISsetFlags(s, geti()));
    else if (op == "MRISclearFlags") oi(MRISclearFlags(s, geti()));
    else if (op == "mrisSetVal") mrisSetVal(s, getf());
    else if (op == "mrisSetValAndClearVal2") mrisSetValAndClearVal2(s, getf());
    else if (op == "mrisClearGradient") oi(mrisClearGradient(s));
    else if (op == "MRISclearGradient") oi(MRISclearGradient(s));
    else if (op == "mrisClearExtraGradient") oi(mrisClearExtraGradient(s));
    else if (op == "mrisClearMomentum") oi(mrisClearMomentum(s));
    else if (op == "MRISnegateValues") oi(MRISnegateValues(s));
    else if (op == "MRISsetVals") oi(MRISsetVals(s, getf()));
    else if (op == "MRISsetValues") oi(MRISsetValues(s, getf()));
    else if (op == "MRISscaleVals") oi(MRISscaleVals(s, getf()));
    else if (op == "MRISmulVal") oi(MRISmulVal(s, getf()));
    else if (op == "MRISsqrtVal") oi(MRISsqrtVal(s));
    else if (op == "MRISsetVal2") oi(MRISsetVal2(s, getf()));
    else if (op == "MRISclearFixedValFlags") oi(MRISclearFixedValFlags(s));
    else if (op == "MRISextractCurvatureVectorDouble") {
      int off = geti();
      std::vector<double> a(nv + off > 0 ? nv + off : 1, -3.0);
      oi(MRISextractCurvatureVectorDouble(s, a.data(), off));
      for (int i = 0; i < nv + off; i++) od(a[i]);
    } else if (op == "MRISextractCurvatureDoubleVector") {
      std::vector<double> a(nv > 0 ? nv : 1, -3.0);
      oi(MRISextractCurvatureDoubleVector(s, a.data()));
      for (int i = 0; i < nv; i++) od(a[i]);
    } else if (op == "MRISabsVals") oi(MRISabsVals(s));
    else if (op == "MRISabsCurvature") oi(MRISabsCurvature(s));
    else if (op == "MRISclearD") MRISclearD(s);
    else if (op == "MRISripLabel" || op == "MRISripNotLabel") {
      if (outside) { out += "G "; continue; }
      oi(op == "MRISripLabel" ? MRISripLabel(s, lab) : MRISripNotLabel(s, lab));
    } else if (op == "MRISripMarked") oi(MRISripMarked(s));
    else if (op == "MRISripUnmarked") oi(MRISripUnmarked(s));
    else if (op == "MRISripZeroThicknessRegions") oi(MRISripZeroThicknessRegions(s));
    else if (op == "mrisMarkBadEdgeVertices") oi(mrisMarkBadEdgeVertices(s, geti()));
    else if (op == "mrisAnyVertexOfFaceMarked") oi(mrisAnyVertexOfFaceMarked(s, geti()));
    else if (op == "triangleMarked") oi(triangleMarked(s, geti()));
    else if (op == "findNonMarkedFace") { int a = geti(), b = geti(); oi(findNonMarkedFace(s, a, b)); }
    else if (op == "detectContrast") oi(detectContrast(s));
    else if (op == "MRISminFilterCurvatures") oi(MRISminFilterCurvatures(s, geti()));
    else if (op == "MRISmaxFilterCurvatures") oi(MRISmaxFilterCurvatures(s, geti()));
    else if (op == "MRISaverageMarkedCurvatures") oi(MRISaverageMarkedCurvatures(s, geti()));
    else if (op == "MRISaverageMarkedVals") oi(MRISaverageMarkedVals(s, geti()));
    else if (op == "MRISaverageVals") oi(MRISaverageVals(s, geti()));
    else if (op == "MRISaverageD") oi(MRISaverageD(s, geti()));
    else if (op == "MRISaverageVal2s") oi(MRISaverageVal2s(s, geti()));
    else if (op == "MRISaverageVal2baks") oi(MRISaverageVal2baks(s, geti()));
    else if (op == "MRISaverageMarkedStats") oi(MRISaverageMarkedStats(s, geti()));
    else if (op == "MRISmedianFilterVals") oi(MRISmedianFilterVals(s, geti()));
    else if (op == "MRISmedianFilterVal2s") oi(MRISmedianFilterVal2s(s, geti()));
    else if (op == "MRISmedianFilterVal2baks") oi(MRISmedianFilterVal2baks(s, geti()));
    else if (op == "MRISmedianFilterCurvature") oi(MRISmedianFilterCurvature(s, geti()));
    else if (op == "MRISmedianFilterD") { int n = geti(), vt = geti(); oi(MRISmedianFilterD(s, n, vt)); }
    else if (op == "MRISgaussianFilterD") oi(MRISgaussianFilterD(s, getd()));
    else if (op == "MRISmodeFilterVals") oi(MRISmodeFilterVals(s, geti()));
    else if (op == "MRISmodeFilterZeroVals") oi(MRISmodeFilterZeroVals(s));
    else if (op == "MRISsoapBubbleVals") oi(MRISsoapBubbleVals(s, geti()));
    else if (op == "MRISsoapBubbleD") oi(MRISsoapBubbleD(s, geti()));
    else if (op == "MRISsampleDistances") {
      int m = geti();
      std::vector<int> nb(m + 1 > 1 ? m + 1 : 1);
      for (size_t i = 0; i < nb.size(); i++) nb[i] = geti();
      oi(MRISsampleDistances(s, nb.data(), m));
      vls_topo_dump(s);
    } else if (op == "MRISsampleAtEachDistance") {
      int a = geti(), b = geti();
      oi(MRISsampleAtEachDistance(s, a, b));
      vls_topo_dump(s);
    }
    else fail("vls op");
    vls_dump(s);
  }
  out += "| ";
  std::vector<std::string> files;
  list_files(".", "", files);
  std::sort(files.begin(), files.end());
  for (const std::string &name : files) {
    if (name == "d/lh.s0") continue;
    oh(name.c_str());
    FILE *f = fopen(name.c_str(), "rb");
    if (!f) { out += "NOFILE "; continue; }
    std::string b;
    char buf[4096];
    size_t n;
    while ((n = fread(buf, 1, sizeof buf, f)) > 0) b.append(buf, n);
    fclose(f);
    ox(b);
  }
}

// ---- relay 41 of track D: vlr, mrisurf_vals.cpp's functions of one program each (valrest.rs)

// vlr: a surface read by MRISread (d/lh.s0: vls's meshes), now and then its neighbourhood set to
// 2 hops, every vertex's rip flag, annotation, curvature and vector-registration values (VALS_VP,
// or none) and every face's rip flag from the case; the program's annotation table set or not (a
// table file in the work folder where FreeSurfer looks for its default one, or a colour table set
// as the table), the surface's colour table, Gdiag_no, a measure volume of one voxel per vertex
// (or none); then 1 to 4 operations named by their FreeSurfer functions; `G` where a value setter
// would read or write outside a vertex's arrays (FS-980); after each the vertices' rip flags,
// borders, annotations, undefvals, curvatures, tx, areas and values, the faces' rip flags, and
// for MRISdilateVertexToSum its mask
static void vlr_dump(MRIS *s)
{
  for (int v = 0; v < s->nvertices; v++) {
    VERTEX *x = &s->vertices[v];
    oi(x->ripflag); oi(x->border); oi(x->annotation); oi(x->undefval); of(x->curv); of(x->tx);
    of(x->area);
    of(x->val); of(x->val2); of(x->origarea); of(x->mean); of(x->mean_imag); of(x->std_error);
    oi(x->marked);
    VALS_VP *vp = (VALS_VP *)x->vp;
    if (!vp) { out += "N "; continue; }
    oi(vp->nvals);
    for (int k = 0; k < vp->nvals; k++) { of(vp->vals[k]); of(vp->orig_vals[k]); }
  }
  for (int f = 0; f < s->nfaces; f++) oi(s->faces[f].ripflag);
}

void vlr_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  setenv("SURFER_SIDEDOOR", "1", 1);
  std::string bytes = geth();
  int nv = geti(), nf = geti();
  FILE *fp = fopen("d/lh.s0", "wb");
  if (!fp) fail("the surface file");
  fwrite(bytes.data(), 1, bytes.size(), fp);
  fclose(fp);
  errno = 0;
  MRIS *s = MRISread("d/lh.s0");
  if (!s) { out += "NULL "; return; }
  if (nv != s->nvertices || nf != s->nfaces) { oi(s->nvertices); oi(s->nfaces); out += "COUNTS "; return; }
  int nsize = geti();
  if (nsize > 1) MRISsetNeighborhoodSize(s, nsize);
  for (int v = 0; v < nv; v++) {
    VERTEX *x = &s->vertices[v];
    x->ripflag = (char)geti();
    x->annotation = geti();
    x->curv = getf();
    x->val = getf(); x->val2 = getf(); x->origarea = getf(); x->mean = getf(); x->mean_imag = getf();
    x->std_error = getf();
    x->marked = (short)geti();
    int nvals = geti();
    if (nvals >= 0) {
      VALS_VP *vp = (VALS_VP *)calloc(1, sizeof(VALS_VP));
      vp->nvals = nvals;
      vp->vals = (float *)calloc(nvals > 0 ? nvals : 1, sizeof(float));
      vp->orig_vals = (float *)calloc(nvals > 0 ? nvals : 1, sizeof(float));
      for (int k = 0; k < nvals; k++) { vp->vals[k] = getf(); vp->orig_vals[k] = getf(); }
      x->vp = vp;
    }
  }
  for (int f = 0; f < nf; f++) s->faces[f].ripflag = (char)geti();
  // the program's annotation table: 0 none, 1 a table file where FreeSurfer looks for its
  // default one (FREESURFER_HOME unset: ./Simple_surface_labels2009.txt), 2 a colour table set as
  // the table
  int tmode = geti();
  if (tmode == 1) {
    std::string text = geth();
    FILE *tf = fopen("Simple_surface_labels2009.txt", "wb");
    if (!tf) fail("the table file");
    fwrite(text.data(), 1, text.size(), tf);
    fclose(tf);
  } else if (tmode == 2) {
    COLOR_TABLE *pre = ann_ctab();
    oi(set_atable_from_ctable(pre));
  }
  s->ct = ann_ctab();
  Gdiag_no = geti();
  setenv("FREESURFER_SEED", std::to_string(geti()).c_str(), 1);
  int vmode = geti();
  MRI *measure = NULL;
  if (vmode > 0) {
    int type = geti(), w = geti();
    measure = MRIallocSequence(w, 1, 1, type, 1);
    for (int c = 0; c < w; c++) MRIsetVoxVal(measure, c, 0, 0, 0, getf());
  }
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    out += op + " ";
    fflush(stdout);
    // a colour-table index with an annotation
    auto has = [&](int i) { return s->ct && i >= 0 && i < s->ct->nentries && s->ct->entries[i]; };
    if (op == "MRISripZeros") oi(MRISripZeros(s, geti() == 1 ? measure : NULL));
    else if (op == "MRISripUnknown") {
      if (!s->ct && nv > 0) { out += "G "; continue; }
      oi(MRISripUnknown(s));
    } else if (op == "MRISdilateConfined") {
      int annotidmask = geti(), niters = geti(), newid = geti(), ty = geti();
      MRI *mask = MRIallocSequence(nv, 1, 1, ty, 1);
      for (int c = 0; c < nv; c++) MRIsetVoxVal(mask, c, 0, 0, 0, getf());
      if ((annotidmask > -1 && !s->ct && nv > 0) || (newid > 0 && !has(newid))) { out += "G "; continue; }
      mpf_vol(MRISdilateConfined(s, mask, annotidmask, niters, newid));
    } else if (op == "MRISaccumulateMeansOnSurface") { int a = geti(), b = geti(); oi(MRISaccumulateMeansOnSurface(s, a, b)); }
    else if (op == "MRISaccumulateImaginaryMeansOnSurface") { int a = geti(), b = geti(); oi(MRISaccumulateImaginaryMeansOnSurface(s, a, b)); }
    else if (op == "MRISaccumulateStandardErrorsOnSurface") { int a = geti(), b = geti(); oi(MRISaccumulateStandardErrorsOnSurface(s, a, b)); }
    else if (op == "MRISclearWhichAndVal2") MRISclearWhichAndVal2(s, geti());
    else if (op == "MRISmarkRandomVertices") oi(MRISmarkRandomVertices(s, getf()));
    else if (op == "MRISannotation_to_label") {
      int i = geti();
      if (!has(i)) { out += "G "; continue; }
      LABEL *l = MRISannotation_to_label(s, i);
      if (!l) out += "NULL ";
      else {
        oi(l->n_points); oi(l->max_points); oh(l->name);
        for (int k = 0; k < l->n_points; k++) oi(l->lv[k].vno);
      }
    }
    else if (op == "MRISmodeFilterAnnotations") oi(MRISmodeFilterAnnotations(s, geti()));
    else if (op == "MRISdilateRipped") oi(MRISdilateRipped(s, geti()));
    else if (op == "MRISerodeRipped") oi(MRISerodeRipped(s, geti()));
    else if (op == "MRISsetRipInFacesWithRippedVertices") MRISsetRipInFacesWithRippedVertices(s);
    else if (op.compare(0, 7, "MRISset") == 0) {
      int fno = geti();
      bool outside = false;
      for (int v = 0; v < nv; v++) {
        VALS_VP *vp = (VALS_VP *)s->vertices[v].vp;
        if (!s->vertices[v].ripflag && vp && (fno < 0 || fno >= vp->nvals)) outside = true;
      }
      if (outside) { out += "G "; continue; }
      if (op == "MRISsetCurvaturesToValues") MRISsetCurvaturesToValues(s, fno);
      else if (op == "MRISsetCurvaturesToOrigValues") MRISsetCurvaturesToOrigValues(s, fno);
      else if (op == "MRISsetOrigValuesToCurvatures") MRISsetOrigValuesToCurvatures(s, fno);
      else if (op == "MRISsetOrigValuesToValues") MRISsetOrigValuesToValues(s, fno);
      else if (op == "MRISsetValuesToCurvatures") MRISsetValuesToCurvatures(s, fno);
      else fail("vlr setter");
    } else if (op == "MRISdilateVertexToSum") {
      int vno = geti(), which = geti();
      double target = getd();
      MRI *m = MRISdilateVertexToSum(vno, which == 2 ? NULL : s, which == 1 ? measure : NULL, target);
      mpf_vol(m);
    }
    else fail("vlr op");
    vlr_dump(s);
  }
}

// ---- relay 42 of track D: top, mrisurf_topology.cpp's rest (toprest.rs)

// top: a surface read by MRISread (d/lh.s0: vls's meshes), its neighbourhood set to 1 to 3 hops
// (with its distances or not), its type (3, or 0: a quadrangle surface), every vertex's rip flag
// and mark and every face's rip flag from the case, Gdiag; then 1 to 6 operations named by their
// FreeSurfer functions (`freedists`: MRISfreeDistsButNotOrig and MRISfreeDistOrigs, which the
// renumbering needs first); after each the surface's counts and every vertex's (to nvertices) rip flag,
// mark, faces with their places, counts, neighbour list (mrisVertexVSize entries) and distances,
// every face's vertices and rip flag
static void top_dump(MRIS *s)
{
  oi(s->nvertices); oi(s->nfaces); oi(s->nsize); oi(s->max_nsize); oi(s->triangle_links_removed);
  oi(s->dist_alloced_flags); oi(s->dist_nsize);
  for (int vno = 0; vno < s->nvertices; vno++) {
    VERTEX *v = &s->vertices[vno];
    VERTEX_TOPOLOGY *vt = &s->vertices_topology[vno];
    oi(v->ripflag); oi(v->marked); oi(vt->num);
    for (int k = 0; k < vt->num; k++) { oi(vt->f[k]); oi(vt->n[k]); }
    oi(vt->vnum); oi(vt->v2num); oi(vt->v3num); oi(vt->vtotal); oi(vt->nsizeMax); oi(vt->nsizeCur);
    int vs = mrisVertexVSize(s, vno);
    oi(vs);
    for (int k = 0; k < vs; k++) oi(vt->v[k]);
    oi(v->dist != NULL);
    if (v->dist) for (int k = 0; k < vt->vtotal && k < v->dist_capacity; k++) of(v->dist[k]);
  }
  for (int fno = 0; fno < s->nfaces; fno++) {
    FACE *f = &s->faces[fno];
    oi(f->v[0]); oi(f->v[1]); oi(f->v[2]); oi(f->ripflag);
  }
}

void top_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  setenv("SURFER_SIDEDOOR", "1", 1);
  std::string bytes = geth();
  int nv = geti(), nf = geti();
  FILE *fp = fopen("d/lh.s0", "wb");
  if (!fp) fail("the surface file");
  fwrite(bytes.data(), 1, bytes.size(), fp);
  fclose(fp);
  errno = 0;
  MRIS *s = MRISread("d/lh.s0");
  if (!s) { out += "NULL "; return; }
  if (nv != s->nvertices || nf != s->nfaces) { oi(s->nvertices); oi(s->nfaces); out += "COUNTS "; return; }
  int nsize = geti(), dmode = geti();
  if (dmode == 1) MRISsetNeighborhoodSizeAndDist(s, nsize);
  else if (nsize > 1) MRISsetNeighborhoodSize(s, nsize);
  s->type = geti();
  for (int v = 0; v < nv; v++) {
    s->vertices[v].ripflag = (char)geti();
    s->vertices[v].marked = (short)geti();
  }
  for (int f = 0; f < nf; f++) s->faces[f].ripflag = (char)geti();
  Gdiag = (unsigned long)(unsigned int)geti();
  // a vertex or face the surface no longer has (after a renumbering): `G`, skipped on both sides
  auto G = [&](int a) { if (a >= 0 && a < s->nvertices) return false; out += "G "; return true; };
  auto GF = [&](int f) { if (f >= 0 && f < s->nfaces) return false; out += "G "; return true; };
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    out += op + " ";
    fflush(stdout);
    if (op == "edgeExists") { int a = geti(), b = geti(); if (G(a) || G(b)) continue; oi(edgeExists(s, a, b)); }
    else if (op == "mrisCountValidLinks") { int a = geti(), b = geti(); if (G(a) || G(b)) continue; oi(mrisCountValidLinks(s, a, b)); }
    else if (op == "mrisAddEdge") { int a = geti(), b = geti(); if (G(a) || G(b)) continue; mrisAddEdge(s, a, b); }
    else if (op == "mrisRemoveEdge") { int a = geti(), b = geti(); if (G(a) || G(b)) continue; mrisRemoveEdge(s, a, b); }
    else if (op == "mrisRemoveVertexLink") { int a = geti(), b = geti(); if (G(a) || G(b)) continue; oi(mrisRemoveVertexLink(s, a, b)); }
    else if (op == "MRISremoveTriangleLinks") oi(MRISremoveTriangleLinks(s));
    else if (op == "mrisCountTotalNeighbors") oi(mrisCountTotalNeighbors(s));
    else if (op == "MRIScomputeEulerNumber") {
      int a, b, c;
      oi(MRIScomputeEulerNumber(s, &a, &b, &c)); oi(a); oi(b); oi(c);
    }
    else if (op == "MRIStopologicalDefectIndex") oi(MRIStopologicalDefectIndex(s));
    else if (op == "mrisStoreVtotalInV3num") oi(mrisStoreVtotalInV3num(s));
    else if (op == "mrisVertexReplacingNeighbors") { int a = geti(), b = geti(); if (G(a)) continue; mrisVertexReplacingNeighbors(s, a, b); }
    else if (op == "mrisForgetNeighborhoods") mrisForgetNeighborhoods(s);
    else if (op == "MRIScountTotalNeighbors") oi(MRIScountTotalNeighbors(s, geti()));
    else if (op == "MRISgetNeighborsBeginEnd") {
      int vno = geti(), a = geti(), b = geti();
      if (G(vno)) continue;
      size_t x = 7, y = 7;
      MRISgetNeighborsBeginEnd(s, vno, (size_t)a, (size_t)b, &x, &y);
      oi((long long)x); oi((long long)y);
    }
    else if (op == "MRISisSurfaceValid") { int a = geti(), b = geti(); oi(MRISisSurfaceValid(s, a, b)); }
    else if (op == "vertexInFace") { int a = geti(), b = geti(); if (G(a) || GF(b)) continue; oi(vertexInFace(s, a, b)); }
    else if (op == "mrisRemoveLink") { int a = geti(), b = geti(); if (G(a) || G(b)) continue; oi(mrisRemoveLink(s, a, b)); }
    else if (op == "mrisRemoveFace") { int f = geti(); if (GF(f)) continue; oi(mrisRemoveFace(s, f)); }
    else if (op == "MRISrenumberRemovingRippedFacesAndVertices") {
      MRISrenumberRemovingRippedFacesAndVertices(s);
      // the vertex number -1 in a face or a kept neighbour list (FS-995): the next reads go outside
      // the vertices: the state printed, the case stopped (the port does the same)
      bool minus = false;
      for (int f = 0; f < s->nfaces; f++)
        for (int n = 0; n < 3; n++) minus |= s->faces[f].v[n] < 0;
      for (int v = 0; v < s->nvertices; v++)
        for (int n = 0; n < s->vertices_topology[v].v3num; n++) minus |= s->vertices_topology[v].v[n] < 0;
      if (minus) { top_dump(s); out += "STOP "; return; }
    }
    else if (op == "MRISremoveRipped") MRISremoveRipped(s);
    else if (op == "MRISevertSurface") oi(MRISevertSurface(s));
    else if (op == "MRISgetSpatOrderedNeighbors") {
      int vno = geti();
      if (G(vno)) continue;
      std::vector<int> l = MRISgetSpatOrderedNeighbors(s, vno);
      oi((long long)l.size());
      for (int x : l) oi(x);
    }
    else if (op == "MRISsetRipInFacesWithRippedVertices") MRISsetRipInFacesWithRippedVertices(s);
    else if (op == "MRIS_setNsizeCur") { int a = geti(), b = geti(); if (G(a)) continue; MRIS_setNsizeCur(s, a, b); }
    else if (op == "rip") { int a = geti(), r = geti(); if (G(a)) continue; s->vertices[a].ripflag = (char)r; }
    else if (op == "freedists") { MRISfreeDistsButNotOrig(s); MRISfreeDistOrigs(s); }
    else fail("top op");
    top_dump(s);
  }
}

// sbl: the soap bubbles of mrisurf_deform.cpp (soap.rs): a surface read by MRISread (d/lh.s0:
// vls's meshes), now and then at 2 hops; every vertex's rip flag, mark, current, original and
// target positions from the case (each a small offset from the read position, now and then NaN);
// Gdiag, Gdiag_no; then 1 to 3 of the four bubbles with a pass count, `freedists` and `marks`
// (the marks set again from the case); after each every vertex's positions, tdx tdy tdz, mark and
// marked3
static void sbl_dump(MRIS *s)
{
  for (int v = 0; v < s->nvertices; v++) {
    VERTEX *x = &s->vertices[v];
    of(x->x); of(x->y); of(x->z); of(x->origx); of(x->origy); of(x->origz);
    of(x->targx); of(x->targy); of(x->targz); of(x->tdx); of(x->tdy); of(x->tdz);
    oi(x->marked); oi(x->marked3);
  }
}

void sbl_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  setenv("SURFER_SIDEDOOR", "1", 1);
  std::string bytes = geth();
  int nv = geti(), nf = geti();
  FILE *fp = fopen("d/lh.s0", "wb");
  if (!fp) fail("the surface file");
  fwrite(bytes.data(), 1, bytes.size(), fp);
  fclose(fp);
  errno = 0;
  MRIS *s = MRISread("d/lh.s0");
  if (!s) { out += "NULL "; return; }
  if (nv != s->nvertices || nf != s->nfaces) { oi(s->nvertices); oi(s->nfaces); out += "COUNTS "; return; }
  int nsize = geti();
  if (nsize > 1) MRISsetNeighborhoodSize(s, nsize);
  std::vector<short> marks(nv);
  for (int v = 0; v < nv; v++) {
    VERTEX *x = &s->vertices[v];
    x->ripflag = (char)geti();
    marks[v] = (short)geti();
    x->marked = marks[v];
    x->x += getf(); x->y += getf(); x->z += getf();
    x->origx = x->x + getf(); x->origy = x->y + getf(); x->origz = x->z + getf();
    x->targx = x->x + getf(); x->targy = x->y + getf(); x->targz = x->z + getf();
  }
  Gdiag = (unsigned long)(unsigned int)geti();
  Gdiag_no = geti();
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    out += op + " ";
    fflush(stdout);
    if (op == "MRISsoapBubbleVertexPositions") oi(MRISsoapBubbleVertexPositions(s, geti()));
    else if (op == "MRISweightedSoapBubbleVertexPositions") oi(MRISweightedSoapBubbleVertexPositions(s, geti()));
    else if (op == "MRISsoapBubbleOrigVertexPositions") oi(MRISsoapBubbleOrigVertexPositions(s, geti()));
    else if (op == "MRISsoapBubbleTargetVertexPositions") oi(MRISsoapBubbleTargetVertexPositions(s, geti()));
    else if (op == "freedists") { MRISfreeDistsButNotOrig(s); MRISfreeDistOrigs(s); }
    else if (op == "marks") { for (int v = 0; v < nv; v++) s->vertices[v].marked = marks[v]; }
    else fail("sbl op");
    sbl_dump(s);
  }
}

// mcl: the closest searches of the surface hash table (mhtfind.rs, relay 43 of track D): a surface
// read by MRISread (d/lh.s0) twice, the second's vertices moved by the case's offsets and their
// original positions set to the current ones; faces and vertices ripped, original positions now and
// then; then 1 to 6 operations: tables, searches at a point or at a vertex of the second surface,
// every vertex of the second surface searched, a vertex moved or ripped
void mcl_case()
{
  mkdir(workdir.c_str(), 0700);
  clear_dir(workdir, true);
  if (chdir(workdir.c_str()) != 0) fail("chdir");
  mkdir("d", 0700);
  setenv("SURFER_SIDEDOOR", "1", 1);
  std::string bytes = geth();
  int nv = geti(), nf = geti();
  int gdiag_no = geti();
  FILE *fp = fopen("d/lh.s0", "wb");
  if (!fp) fail("the surface file");
  fwrite(bytes.data(), 1, bytes.size(), fp);
  fclose(fp);
  errno = 0;
  Gdiag_no = -1;
  MRIS *s = MRISread("d/lh.s0");
  if (!s) { out += "NULL "; return; }
  if (nv != s->nvertices || nf != s->nfaces) { oi(s->nvertices); oi(s->nfaces); out += "COUNTS "; return; }
  MRIS *s2 = MRISread("d/lh.s0");
  if (!s2) { out += "NULL2 "; return; }
  Gdiag_no = gdiag_no;
  int nfrip = geti();
  for (int k = 0; k < nfrip; k++) s->faces[geti()].ripflag = 1;
  int nvrip = geti();
  for (int k = 0; k < nvrip; k++) s->vertices[geti()].ripflag = 1;
  if (geti())
    for (int v = 0; v < nv; v++) {
      s->vertices[v].origx = getf(); s->vertices[v].origy = getf(); s->vertices[v].origz = getf();
    }
  for (int v = 0; v < nv; v++) {
    VERTEX *x = &s2->vertices[v];
    x->x += getf(); x->y += getf(); x->z += getf();
    x->origx = x->x; x->origy = x->y; x->origz = x->z;
  }
  MRIS_HASH_TABLE *mht = NULL;
  int nops = geti();
  for (int k = 0; k < nops; k++) {
    std::string op = next();
    out += op + " ";
    fflush(stdout);
    errno = 0;
    if (op == "ft" || op == "vt") {
      int which = geti();
      float res = getf();
      MHTfree(&mht);
      mht = op == "ft" ? MHTcreateFaceTable_Resolution(s, which, res) : MHTcreateVertexTable_Resolution(s, which, res);
    } else if (op == "ftd") {
      MHTfree(&mht);
      mht = MHTcreateFaceTable(s);
    } else if (op == "vtd") {
      int which = geti();
      MHTfree(&mht);
      mht = MHTcreateVertexTable(s, which);
    } else if (op == "vg") {
      double px = getd(), py = getd(), pz = getd(), dmax = getd();
      int mm = geti();
      if (!mht) { out += "G "; continue; }
      int vno = -7;
      double dist = -7;
      MHTfindClosestVertexGeneric(mht, px, py, pz, dmax, mm, &vno, &dist);
      oi(vno); od(dist);
    } else if (op == "vx" || op == "vs") {
      float x = getf(), y = getf(), z = getf();
      if (!mht) { out += "G "; continue; }
      if (op == "vx") {
        float dist = -7;
        oi(MHTfindClosestVertexNoXYZ(mht, s, x, y, z, &dist));
        of(dist);
      } else oi(MHTfindClosestSetVertexNo(mht, s, x, y, z));
    } else if (op == "vi" || op == "vit") {
      float x = getf(), y = getf(), z = getf();
      int glob = geti();
      if (!mht) { out += "G "; continue; }
      if (op == "vi") oi(MHTfindVnoOfClosestVertexInTable(mht, s, x, y, z, glob));
      else {
        VERTEX *v = MHTfindClosestVertexInTable(mht, s, x, y, z, glob);
        oi(v ? (long long)(v - s->vertices) : -1);
      }
    } else if (op == "v2" || op == "vp2" || op == "vs2") {
      int vno = geti();
      if (!mht) { out += "G "; continue; }
      if (op == "v2") {
        float dist = -7;
        oi(MHTfindClosestVertexNo2(mht, s, s2, &s2->vertices[vno], &dist));
        of(dist);
      } else if (op == "vp2") {
        float dist = -7;
        VERTEX *v = MHTfindClosestVertex2(mht, s, s2, &s2->vertices[vno], &dist);
        oi(v ? (long long)(v - s->vertices) : -1);
        of(dist);
      } else {
        VERTEX *v = MHTfindClosestVertexSet2(mht, s, s2, &s2->vertices[vno]);
        oi(v ? (long long)(v - s->vertices) : -1);
      }
    } else if (op == "fg" || op == "fn") {
      double px = getd(), py = getd(), pz = getd(), dmax = getd();
      int mm = geti(), proj = geti();
      if (!mht) { out += "G "; continue; }
      int fno = -7;
      double dist = -7;
      if (op == "fg") {
        FACE *face = (FACE *)1;
        MHTfindClosestFaceGeneric(mht, s, px, py, pz, dmax, mm, proj, &face, &fno, &dist);
        if (face != (fno < 0 ? NULL : &s->faces[fno])) fail("fg face");
      } else MHTfindClosestFaceNoGeneric(mht, s, px, py, pz, dmax, mm, proj, &fno, &dist);
      oi(fno); od(dist);
    } else if (op == "bf") {
      float x = getf(), y = getf(), z = getf();
      int which = geti();
      float dist = -7;
      oi(MHTBruteForceClosestFace(s, x, y, z, which, &dist));
      of(dist);
    } else if (op == "av") {
      double dmax = getd();
      int mm = geti();
      if (!mht) { out += "G "; continue; }
      for (int v = 0; v < nv; v++) {
        VERTEX *x = &s2->vertices[v];
        int vno = -7;
        double dist = -7;
        MHTfindClosestVertexGeneric(mht, x->x, x->y, x->z, dmax, mm, &vno, &dist);
        oi(vno); od(dist);
      }
    } else if (op == "ax") {
      if (!mht) { out += "G "; continue; }
      for (int v = 0; v < nv; v++) {
        VERTEX *x = &s2->vertices[v];
        float dist = -7;
        oi(MHTfindClosestVertexNoXYZ(mht, s, x->x, x->y, x->z, &dist));
        of(dist);
      }
    } else if (op == "af") {
      double dmax = getd();
      int mm = geti(), proj = geti();
      if (!mht) { out += "G "; continue; }
      for (int v = 0; v < nv; v++) {
        VERTEX *x = &s2->vertices[v];
        int fno = -7;
        double dist = -7;
        FACE *face = NULL;
        MHTfindClosestFaceGeneric(mht, s, x->x, x->y, x->z, dmax, mm, proj, &face, &fno, &dist);
        oi(fno); od(dist);
      }
    } else if (op == "mv") {
      int vno = geti();
      s->vertices[vno].x += getf(); s->vertices[vno].y += getf(); s->vertices[vno].z += getf();
    } else if (op == "rip") {
      s->vertices[geti()].ripflag = 1;
    } else fail("mcl op");
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
  } else if (cmd == "srd") {
    srd_case();
  } else if (cmd == "mpb") {
    mpb_case();
  } else if (cmd == "mpc") {
    mpc_case();
  } else if (cmd == "mpd") {
    mpd_case();
  } else if (cmd == "mpe") {
    mpe_case();
  } else if (cmd == "mpf") {
    mpf_case();
  } else if (cmd == "mpg") {
    mpg_case();
  } else if (cmd == "fli") {
    fli_case();
  } else if (cmd == "flr") {
    flr_case();
  } else if (cmd == "bvl") {
    bvl_case();
  } else if (cmd == "crv") {
    crv_case();
  } else if (cmd == "pvw") {
    pvw_case();
  } else if (cmd == "vps") {
    vps_case();
  } else if (cmd == "ann") {
    ann_case();
  } else if (cmd == "mpx") {
    mpx_case();
  } else if (cmd == "lbl") {
    lbl_case();
  } else if (cmd == "scl") {
    scl_case();
  } else if (cmd == "vcp") {
    vcp_case();
  } else if (cmd == "mht") {
    mht_case();
  } else if (cmd == "vls") {
    vls_case();
  } else if (cmd == "vlr") {
    vlr_case();
  } else if (cmd == "top") {
    top_case();
  } else if (cmd == "sbl") {
    sbl_case();
  } else if (cmd == "mcl") {
    mcl_case();
  } else if (cmd == "wvl") {
    wvl_case();
  } else if (cmd == "gii") {
    gii_case();
  } else if (cmd == "gwr") {
    gwr_case();
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

// relay 23: MRISsmoothKernel's timings ("Done allocating shl <n>, <s> sec", "... t=<s> sec") and its
// closing value ("vtxval <x>": column 1031 of the output's first row, memory outside a small volume)
// as <t> and <v>, as the port writes them
static std::string untime(const std::string &t)
{
  std::string r;
  size_t i = 0;
  bool first = true;
  while (true) {
    size_t j = t.find('\n', i);
    std::string line = t.substr(i, j == std::string::npos ? std::string::npos : j - i);
    if (!first) r += '\n';
    first = false;
    size_t sec = line.rfind(" sec");
    if (line.rfind("Done allocating shl ", 0) == 0 && line.rfind(", ") != std::string::npos &&
        sec != std::string::npos && sec > line.rfind(", ")) {
      size_t a = line.rfind(", ");
      line = line.substr(0, a + 2) + "<t>" + line.substr(sec);
    } else if (line.rfind("Finished loop over ", 0) == 0 && line.rfind("t=") != std::string::npos &&
               sec != std::string::npos && sec > line.rfind("t=")) {
      size_t a = line.rfind("t=");
      line = line.substr(0, a + 2) + "<t>" + line.substr(sec);
    } else if (line.rfind("vtxval ", 0) == 0) {
      line = "vtxval <v>";
    } else if (line.rfind("  MRISfillInterior t = ", 0) == 0) {
      // relay 32 of track D: MRISfillInterior's elapsed time (with Gdiag_no > 0)
      line = "  MRISfillInterior t = <t>";
    } else if ((line.rfind("SurfClusterSumFast: n=", 0) == 0 || line.rfind("SurfClusterSum: n=", 0) == 0) &&
               line.find(", t = ") != std::string::npos) {
      // relay 38 of track D: the cluster summaries' elapsed times (with Gdiag_no > 0)
      line = line.substr(0, line.find(", t = ") + 6) + "<t>";
    }
    r += line;
    if (j == std::string::npos) break;
    i = j + 1;
  }
  return r;
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
      // relay 32 of track D: the recorded heads' cases (flr) run for minutes
      alarm(line.compare(0, 4, "flr ") == 0 ? 900 : 5);
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
    // relay 19: the work folder's absolute path (a volume reader's messages name it) as `<w>`,
    // as the port writes its own temporary folder's
    for (std::string *t : {&result, &text})
      for (size_t at = t->find(workdir); at != std::string::npos; at = t->find(workdir, at + 3))
        t->replace(at, workdir.size(), "<w>");
    printf("%s | %s\n", result.c_str(), escape(untime(text)).c_str());
  }
  unlink(scratch.c_str());
  clear_dir(workdir, false);
  return 0;
}
