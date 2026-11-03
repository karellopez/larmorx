// SPDX-License-Identifier: Apache-2.0
// warpfns_probe: runs FSL 6.0.7.23's own libraries (fsl-miscmaths' Splinterpolator, newimage's
// interpolation with partial derivatives, fsl-warpfns' resampling and Jacobians) on the inputs
// of one case directory and prints every result as the hexadecimal bits of its value, one line
// per value, in the order and format of the replica's side
// (crates-nc/larmorx-nc-core/tests/warpfns_probe.rs). See README.md.
//
// Usage: warpfns_probe <case dir> <nthr>
// Output volumes start at 0 (newimage leaves new volumes uninitialised).
// <case dir>/params.txt: "key value..." lines; "mode" is spl, vol, warp or jac (check.py writes
// them; the other files are raw little-endian arrays).
#define EXPOSE_TREACHEROUS  // sform, sampling_mat (as topup defines it)
#include <cstdio>
#include <cstring>
#include <fstream>
#include <iostream>
#include <map>
#include <memory>
#include <sstream>
#include <string>
#include <vector>
#include "armawrap/newmat.h"
#include "basisfield/splinefield.h"
#include "miscmaths/splinterpolator.h"
#include "newimage/newimageall.h"
#include "utils/threading.h"
#include "warpfns/fnirt_file_reader.h"
#include "warpfns/warpfns.h"

using namespace std;
using namespace NEWIMAGE;
namespace SP = SPLINTERPOLATOR;

typedef map<string, vector<string> > Params;

static Params read_params(const string& dir)
{
  Params p;
  ifstream in(dir + "/params.txt");
  string line;
  while (getline(in, line)) {
    istringstream ls(line);
    string key, v;
    ls >> key;
    while (ls >> v) p[key].push_back(v);
  }
  return p;
}

template <class T> static vector<T> slurp(const string& f)
{
  ifstream in(f, ios::binary);
  vector<char> b((istreambuf_iterator<char>(in)), istreambuf_iterator<char>());
  vector<T> v(b.size() / sizeof(T));
  memcpy(v.data(), b.data(), v.size() * sizeof(T));
  return v;
}

static void pf(const string& tag, float v) { unsigned int b; memcpy(&b, &v, 4); printf("%s %08x\n", tag.c_str(), b); }
static void pd(const string& tag, double v) { unsigned long long b; memcpy(&b, &v, 8); printf("%s %016llx\n", tag.c_str(), b); }
static void perr(const string& tag, const std::exception& e) { printf("%s error %s\n", tag.c_str(), e.what()); }
template <class T> static void pv(const string& tag, T v);
template <> void pv<float>(const string& tag, float v) { pf(tag, v); }
template <> void pv<double>(const string& tag, double v) { pd(tag, v); }

static int I(const Params& p, const string& k, int i = 0) { return atoi(p.at(k).at(i).c_str()); }
static double D(const Params& p, const string& k, int i = 0) { return strtod(p.at(k).at(i).c_str(), nullptr); }

static extrapolation ext_of(const string& s)
{
  if (s == "zeropad") return zeropad;
  if (s == "constpad") return constpad;
  if (s == "extraslice") return extraslice;
  if (s == "mirror") return mirror;
  return periodic;
}

static interpolation interp_of(const string& s)
{
  if (s == "nearestneighbour") return nearestneighbour;
  if (s == "trilinear") return trilinear;
  if (s == "sinc") return sinc;
  return spline;
}

static NEWMAT::Matrix read_mat(const string& f)
{
  vector<double> v = slurp<double>(f);
  NEWMAT::Matrix M(4, 4);
  for (int r = 0; r < 4; r++) for (int c = 0; c < 4; c++) M(r + 1, c + 1) = v[r * 4 + c];
  return M;
}

static void pmat(const string& tag, const NEWMAT::Matrix& M)
{
  for (int r = 1; r <= 4; r++) for (int c = 1; c <= 4; c++) pd(tag, M(r, c));
}

// A volume<float> of the case's prefix ("f", "o", "d"): size, voxel sizes and settings.
static void setup(volume<float>& v, const Params& p, const string& pre)
{
  if (p.count(pre + "interp")) v.setinterpolationmethod(interp_of(p.at(pre + "interp")[0]));
  if (p.count(pre + "order")) v.setsplineorder(I(p, pre + "order"));
  if (p.count(pre + "extrap")) v.setextrapolationmethod(ext_of(p.at(pre + "extrap")[0]));
  if (p.count(pre + "pad")) v.setpadvalue(float(D(p, pre + "pad")));
  if (p.count(pre + "valid")) v.setextrapolationvalidity(I(p, pre + "valid", 0), I(p, pre + "valid", 1), I(p, pre + "valid", 2));
}

// ---------------------------------------------------------------------------------------------
// spl: the Splinterpolator itself

template <class T> static void spl(const string& dir, const Params& p, const string& pfx, int nthr)
{
  int nd = I(p, "ndim");
  vector<unsigned int> dim(nd);
  vector<SP::ExtrapolationType> et(nd);
  unsigned int n = 1;
  for (int i = 0; i < nd; i++) { dim[i] = I(p, "dims", i); et[i] = SP::ExtrapolationType(I(p, "et", i)); n *= dim[i]; }
  unsigned int order = I(p, "order");
  vector<float> raw = slurp<float>(dir + "/data.bin");
  vector<T> data(raw.begin(), raw.begin() + n);
  vector<double> pts = slurp<double>(dir + "/points.bin");
  vector<unsigned int> grid = slurp<unsigned int>(dir + "/grid.bin");
  try {
    SP::Splinterpolator<T> s(data.data(), dim, et, order, false, Utilities::NoOfThreads(nthr));
    vector<T> c; s.Copy(c);
    for (auto v : c) pv<T>(pfx + "coef", v);
    for (size_t q = 0; q + 5 <= pts.size(); q += 5) {
      vector<float> co(nd);
      for (int i = 0; i < nd; i++) co[i] = float(pts[q + i]);
      try { pv<T>(pfx + "val", s(co)); } catch (const std::exception& e) { perr(pfx + "val", e); }
      for (int dd = 0; dd < nd; dd++) {
        try { T dv; T v = s(co, dd, &dv); pv<T>(pfx + "vd", v); pv<T>(pfx + "vd", dv); }
        catch (const std::exception& e) { perr(pfx + "vd", e); }
      }
      try { vector<T> r(nd); T v = s.ValAndDerivs(co, r); pv<T>(pfx + "vads", v); for (auto x : r) pv<T>(pfx + "vads", x); }
      catch (const std::exception& e) { perr(pfx + "vads", e); }
      if (nd <= 4) {
        double x = pts[q], y = nd > 1 ? pts[q + 1] : 0.0, z = nd > 2 ? pts[q + 2] : 0.0, t = nd > 3 ? pts[q + 3] : 0.0;
        try { pv<T>(pfx + "at", s(x, y, z, t)); } catch (const std::exception& e) { perr(pfx + "at", e); }
      }
      if (nd <= 3) {
        double x = pts[q], y = nd > 1 ? pts[q + 1] : 0.0, z = nd > 2 ? pts[q + 2] : 0.0;
        for (int dd = 0; dd < nd; dd++) {
          try { T dv; T v = s(x, y, z, dd, &dv); pv<T>(pfx + "atd", v); pv<T>(pfx + "atd", dv); }
          catch (const std::exception& e) { perr(pfx + "atd", e); }
        }
      }
      if (nd == 3) {
        try { vector<T> r(3); T v = s.ValAndDerivs(pts[q], pts[q + 1], pts[q + 2], r); pv<T>(pfx + "vad3", v); for (auto x : r) pv<T>(pfx + "vad3", x); }
        catch (const std::exception& e) { perr(pfx + "vad3", e); }
      }
    }
    for (size_t q = 0; q + 5 <= grid.size(); q += 5) {
      vector<unsigned int> ix(grid.begin() + q, grid.begin() + q + nd);
      try { vector<T> g(nd); s.Grad(ix, g); for (auto x : g) pv<T>(pfx + "grad", x); }
      catch (const std::exception& e) { perr(pfx + "grad", e); }
      for (int dd = 0; dd < nd; dd++) {
        try { pv<T>(pfx + "deriv", s.Deriv(ix, dd)); } catch (const std::exception& e) { perr(pfx + "deriv", e); }
      }
      if (nd == 3) {
        try { T a, b, cc; s.Grad3D(ix[0], ix[1], ix[2], &a, &b, &cc); pv<T>(pfx + "g3", a); pv<T>(pfx + "g3", b); pv<T>(pfx + "g3", cc); }
        catch (const std::exception& e) { perr(pfx + "g3", e); }
      }
    }
  }
  catch (const std::exception& e) { perr(pfx + "new", e); }
}

// ---------------------------------------------------------------------------------------------
// vol: newimage's interpolation of one volume

static void vol(const string& dir, const Params& p)
{
  int nx = I(p, "fsize", 0), ny = I(p, "fsize", 1), nz = I(p, "fsize", 2);
  volume<float> f(nx, ny, nz);
  f.setdims(float(D(p, "fvxs", 0)), float(D(p, "fvxs", 1)), float(D(p, "fvxs", 2)));
  vector<float> data = slurp<float>(dir + "/f.bin");
  for (int k = 0, n = 0; k < nz; k++) for (int j = 0; j < ny; j++) for (int i = 0; i < nx; i++, n++) f(i, j, k) = data[n];
  setup(f, p, "f");
  vector<float> pts = slurp<float>(dir + "/points.bin");
  volume<float>::extrapparams ep2(ext_of(p.at("ep2extrap")[0]), I(p, "ep2valid", 0), I(p, "ep2valid", 1), I(p, "ep2valid", 2));
  const volume<float>& cf = f;
  for (int e = 0; e < 2; e++) {
    const volume<float>::extrapparams& ep = e ? ep2 : volume<float>::extrapparams();
    string t = e ? "e2" : "e1";
    for (size_t q = 0; q + 3 <= pts.size(); q += 3) {
      float x = pts[q], y = pts[q + 1], z = pts[q + 2];
      try { pf(t + "ip", cf.interpolate(x, y, z, ep)); } catch (const std::exception& ex) { perr(t + "ip", ex); }
      for (int d = 0; d < 3; d++) {
        try { float dv; float v = cf.interp1partial(x, y, z, d, &dv, ep); pf(t + "i1", v); pf(t + "i1", dv); }
        catch (const std::exception& ex) { perr(t + "i1", ex); }
      }
      try { float a, b, c; float v = cf.interp3partial(x, y, z, &a, &b, &c, ep); pf(t + "i3", v); pf(t + "i3", a); pf(t + "i3", b); pf(t + "i3", c); }
      catch (const std::exception& ex) { perr(t + "i3", ex); }
      printf("%svf %d\n", t.c_str(), int(cf.valid(x, y, z, 1e-8, ep)));
    }
  }
}

// ---------------------------------------------------------------------------------------------
// warp: fsl-warpfns' resampling

static void pout(const string& tag, const volume<float>& o)
{
  for (int k = 0; k < o.zsize(); k++) for (int j = 0; j < o.ysize(); j++) for (int i = 0; i < o.xsize(); i++) pf(tag, o(i, j, k));
  printf("%s forms %d %d\n", tag.c_str(), o.sform_code(), o.qform_code());
  pmat(tag + "s", o.sform_mat());
  pmat(tag + "q", o.qform_mat());
}

static void pderiv(const string& tag, const volume4D<float>& d)
{
  for (int t = 0; t < d.tsize(); t++)
    for (int k = 0; k < d.zsize(); k++) for (int j = 0; j < d.ysize(); j++) for (int i = 0; i < d.xsize(); i++) pf(tag, d(i, j, k, t));
  printf("%s forms %d %d\n", tag.c_str(), d.sform_code(), d.qform_code());
}

static void pmask(const string& tag, const volume<char>& m)
{
  for (int k = 0; k < m.zsize(); k++) for (int j = 0; j < m.ysize(); j++) for (int i = 0; i < m.xsize(); i++) printf("%s %d\n", tag.c_str(), int(m(i, j, k)));
}

static void warp(const string& dir, const Params& p, int nthr)
{
  int fx = I(p, "fsize", 0), fy = I(p, "fsize", 1), fz = I(p, "fsize", 2);
  int ox = I(p, "osize", 0), oy = I(p, "osize", 1), oz = I(p, "osize", 2);
  int dx = I(p, "dsize", 0), dy = I(p, "dsize", 1), dz = I(p, "dsize", 2);
  volume<float> f(fx, fy, fz);
  f.setdims(float(D(p, "fvxs", 0)), float(D(p, "fvxs", 1)), float(D(p, "fvxs", 2)));
  vector<float> fd = slurp<float>(dir + "/f.bin");
  for (int k = 0, n = 0; k < fz; k++) for (int j = 0; j < fy; j++) for (int i = 0; i < fx; i++, n++) f(i, j, k) = fd[n];
  setup(f, p, "f");
  if (I(p, "fsform")) f.set_sform(I(p, "fsform"), read_mat(dir + "/fsform.bin"));
  volume4D<float> d(dx, dy, dz, 3);
  d.setdims(float(D(p, "dvxs", 0)), float(D(p, "dvxs", 1)), float(D(p, "dvxs", 2)), 1.0);
  vector<float> dd = slurp<float>(dir + "/d.bin");
  for (int t = 0, n = 0; t < 3; t++) for (int k = 0; k < dz; k++) for (int j = 0; j < dy; j++) for (int i = 0; i < dx; i++, n++) d(i, j, k, t) = dd[n];
  if (I(p, "dsform")) d.set_sform(I(p, "dsform"), read_mat(dir + "/dsform.bin"));
  NEWMAT::Matrix A = read_mat(dir + "/A.bin"), TT = read_mat(dir + "/TT.bin"), M = read_mat(dir + "/M.bin");
  int dir1 = I(p, "dir1");
  Utilities::NoOfThreads nt(nthr);
  auto mkout = [&]() { volume<float> o(ox, oy, oz); o.setdims(float(D(p, "ovxs", 0)), float(D(p, "ovxs", 1)), float(D(p, "ovxs", 2))); o = 0.0f; return o; };
  auto mkder = [&](int n) { volume4D<float> o(ox, oy, oz, n); o.setdims(float(D(p, "ovxs", 0)), float(D(p, "ovxs", 1)), float(D(p, "ovxs", 2)), 1.0); o = 0.0f; return o; };
  auto mkmask = [&]() { volume<char> m(ox, oy, oz); m = 0; return m; };
  try { volume<float> o = mkout(); volume4D<float> de = mkder(3); volume<char> m = mkmask();
        general_transform_3partial(f, A, d, o, de, m, nt); pout("g3p", o); pderiv("g3pd", de); pmask("g3pm", m); }
  catch (const std::exception& e) { perr("g3p", e); }
  try { volume<float> o = mkout(); general_transform(f, A, d, o, nt); pout("g", o); }
  catch (const std::exception& e) { perr("g", e); }
  try { volume<float> o = mkout(); volume<char> m = mkmask(); general_transform(f, A, d, o, m, nt); pout("gm", o); pmask("gmm", m); }
  catch (const std::exception& e) { perr("gm", e); }
  try { volume<float> o = mkout(); volume<char> m = mkmask(); affine_transform(f, A, o, m, nt); pout("a", o); pmask("am", m); }
  catch (const std::exception& e) { perr("a", e); }
  try { volume<float> o = mkout(); volume4D<float> de = mkder(3); affine_transform_3partial(f, A, o, de, nt); pout("a3p", o); pderiv("a3pd", de); }
  catch (const std::exception& e) { perr("a3p", e); }
  try { volume<float> o = mkout(); volume4D<float> de = mkder(3); volume<float> d0 = d[0];
        displacement_transform_1D_3partial(f, A, d0, dir1, o, de, nt); pout("d1", o); pderiv("d1d", de); }
  catch (const std::exception& e) { perr("d1", e); }
  try { volume<float> o = mkout(); volume4D<float> de = mkder(1); volume<char> m = mkmask();
        vector<int> defdir = {0, 1, 2}, derivdir = {dir1};
        raw_general_transform(f, A, d, defdir, derivdir, o, de, m, nt); pout("r1", o); pderiv("r1d", de); pmask("r1m", m); }
  catch (const std::exception& e) { perr("r1", e); }
  try { volume<float> o = mkout(); volume<char> m = mkmask(); apply_warp(f, A, d, TT, M, o, m, nt); pout("aw", o); pmask("awm", m); }
  catch (const std::exception& e) { perr("aw", e); }
  try { volume<float> o = mkout(); volume<char> m = mkmask(); vector<unsigned int> sl = {static_cast<unsigned int>(oz - 1), 0u};
        affine_transform(f, A, sl, o, m, nt); pout("slc", o); pmask("slcm", m); }
  catch (const std::exception& e) { perr("slc", e); }
}

// ---------------------------------------------------------------------------------------------
// jac: Jacobians of spline displacement fields

static void jac(const string& dir, const Params& p)
{
  vector<unsigned int> sz{(unsigned)I(p, "fsize", 0), (unsigned)I(p, "fsize", 1), (unsigned)I(p, "fsize", 2)};
  vector<double> vxs{D(p, "fvxs", 0), D(p, "fvxs", 1), D(p, "fvxs", 2)};
  vector<unsigned int> ksp{(unsigned)I(p, "ksp", 0), (unsigned)I(p, "ksp", 1), (unsigned)I(p, "ksp", 2)};
  int order = I(p, "order");
  int jx = I(p, "jsize", 0), jy = I(p, "jsize", 1), jz = I(p, "jsize", 2);
  vector<double> cb = slurp<double>(dir + "/coef.bin");
  NEWMAT::Matrix aff = read_mat(dir + "/aff.bin");
  auto field = [&](int set) {
    BASISFIELD::splinefield fl(sz, vxs, ksp, order);
    NEWMAT::ColumnVector c(fl.CoefSz());
    for (unsigned int i = 0; i < fl.CoefSz(); i++) c(i + 1) = cb[set * fl.CoefSz() + i];
    fl.SetCoef(c);
    return fl;
  };
  auto mkjac = [&]() { volume<float> j(jx, jy, jz); j.setdims(float(vxs[0]), float(vxs[1]), float(vxs[2]));
                       if (p.count("jextrap")) j.setextrapolationmethod(ext_of(p.at("jextrap")[0])); j = 0.0f; return j; };
  auto pj = [&](const string& tag, const volume<float>& j) {
    for (int k = 0; k < j.zsize(); k++) for (int jj = 0; jj < j.ysize(); jj++) for (int i = 0; i < j.xsize(); i++) pf(tag, j(i, jj, k)); };
  double sx = D(p, "scale", 0), sy = D(p, "scale", 1);
  for (int fi = 1; fi <= 3; fi++) {
    string tag = "j1_" + to_string(fi);
    try { BASISFIELD::splinefield fl = field(0); fl.ScaleField(fi == 2 ? sy : sx); volume<float> j = mkjac();
          deffield2jacobian(fl, BASISFIELD::FieldIndex(fi), j); pj(tag, j); }
    catch (const std::exception& e) { perr(tag, e); }
  }
  try { BASISFIELD::splinefield x = field(0), y = field(0), z = field(0); x.ScaleField(sx); y.ScaleField(sy); z.ScaleField(0.0);
        volume<float> j = mkjac(); deffield2jacobian(x, y, z, j); pj("j3t", j); }
  catch (const std::exception& e) { perr("j3t", e); }
  try { BASISFIELD::splinefield x = field(0), y = field(1), z = field(2); volume<float> j = mkjac();
        deffield2jacobian(x, y, z, aff, j); pj("j3a", j); }
  catch (const std::exception& e) { perr("j3a", e); }
  try { BASISFIELD::splinefield x = field(0), y = field(1), z = field(2); volume4D<float> jm(jx, jy, jz, 9);
        jm.setdims(float(vxs[0]), float(vxs[1]), float(vxs[2]), 1.0); jm = 0.0f;
        deffield2jacobian_matrix(x, y, z, aff, jm);
        for (int t = 0; t < 9; t++) for (int k = 0; k < jz; k++) for (int jj = 0; jj < jy; jj++) for (int i = 0; i < jx; i++) pf("jm", jm(i, jj, k, t)); }
  catch (const std::exception& e) { perr("jm", e); }
}

int main(int argc, char* argv[])
{
  if (argc < 3) { cerr << "usage: warpfns_probe <case dir> <nthr>" << endl; return 2; }
  string dir(argv[1]);
  int nthr = atoi(argv[2]);
  Params p = read_params(dir);
  string mode = p.at("mode")[0];
  if (mode == "spl") { spl<float>(dir, p, "f_", nthr); spl<double>(dir, p, "d_", nthr); }
  else if (mode == "vol") vol(dir, p);
  else if (mode == "warp") warp(dir, p, nthr);
  else if (mode == "jac") jac(dir, p);
  return 0;
}
