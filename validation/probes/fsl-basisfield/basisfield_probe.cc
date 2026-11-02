// SPDX-License-Identifier: Apache-2.0
// basisfield_probe: runs FSL 6.0.7.23's own BASISFIELD::splinefield (libfsl-basisfield) on the
// inputs of one case directory and prints every result as the hexadecimal bits of its value, one
// line per value, in the order and format of the replica's side
// (crates-nc/larmorx-nc-core/tests/basisfield_probe.rs). See README.md.
//
// Usage: basisfield_probe <case dir> <nthr> [energy]   (energy: only the sizes, the energies and
// their gradients)
// <case dir>/params.txt: nx ny nz vx vy vz kx ky kz order
// coef.bin (double, CoefSz), ima1.bin, ima2.bin (float, FieldSz), mask.bin (char, FieldSz),
// points.txt (x y z per line: Peek), wide.txt (i j k per line: PeekWide).
#include <cstdio>
#include <cstring>
#include <fstream>
#include <iostream>
#include <memory>
#include <string>
#include <vector>
#include "armawrap/newmat.h"
#include "basisfield/splinefield.h"
#include "miscmaths/bfmatrix.h"
#include "newimage/newimageall.h"
#include "utils/threading.h"

using namespace std;

static vector<char> slurp(const string& f)
{
  ifstream in(f, ios::binary);
  return vector<char>((istreambuf_iterator<char>(in)), istreambuf_iterator<char>());
}

static void pd(const char* tag, double v)
{
  unsigned long long b; memcpy(&b, &v, 8);
  printf("%s %016llx\n", tag, b);
}

static void pf(const char* tag, float v)
{
  unsigned int b; memcpy(&b, &v, 4);
  printf("%s %08x\n", tag, b);
}

static void pvec(const char* tag, const NEWMAT::ColumnVector& v)
{
  printf("%s n=%d\n", tag, int(v.Nrows()));
  for (int i = 1; i <= v.Nrows(); i++) pd(tag, v(i));
}

static void pcsc(const char* tag, const std::shared_ptr<MISCMATHS::BFMatrix>& m)
{
  const MISCMATHS::SparseBFMatrix<double>* s = dynamic_cast<const MISCMATHS::SparseBFMatrix<double>*>(m.get());
  MISCMATHS::SpMat<double> sp = s->AsSpMat();
  printf("%s m=%u n=%u nz=%u\n", tag, sp.Nrows(), sp.Ncols(), sp.NZ());
  for (unsigned int c = 1; c <= sp.Ncols(); c++) {
    for (MISCMATHS::SpMat<double>::ColumnIterator it = sp.begin(c); it != sp.end(c); ++it) {
      unsigned long long b; double v = *it; memcpy(&b, &v, 8);
      printf("%s %u %u %016llx\n", tag, c - 1, it.Row() - 1, b);
    }
  }
}

int main(int argc, char* argv[])
{
  if (argc < 3) { cerr << "usage: basisfield_probe <case dir> <nthr>" << endl; return 2; }
  string dir(argv[1]);
  int nthr = atoi(argv[2]);
  bool energy_only = argc > 3 && string(argv[3]) == "energy";
  unsigned int nx, ny, nz, kx, ky, kz; double vx, vy, vz; int order;
  { ifstream p(dir + "/params.txt"); p >> nx >> ny >> nz >> vx >> vy >> vz >> kx >> ky >> kz >> order; }
  try {
    vector<unsigned int> sz{nx, ny, nz}, ksp{kx, ky, kz};
    vector<double> vxs{vx, vy, vz};
    BASISFIELD::splinefield f(sz, vxs, ksp, order, Utilities::NoOfThreads(nthr));
    printf("csz %u %u %u\n", f.CoefSz_x(), f.CoefSz_y(), f.CoefSz_z());
    vector<char> cb = slurp(dir + "/coef.bin");
    NEWMAT::ColumnVector coef(f.CoefSz());
    memcpy(coef.Store(), cb.data(), sizeof(double) * f.CoefSz());
    f.SetCoef(coef);
    if (energy_only) {
      pd("memen", f.MemEnergy());
      pd("bendeng", f.BendEnergy());
      pvec("memen_grad", f.MemEnergyGrad());
      pvec("bend_grad", f.BendEnergyGrad());
      return 0;
    }
    unsigned int n = nx * ny * nz;
    NEWIMAGE::volume<float> ima1(nx, ny, nz), ima2(nx, ny, nz), ones(nx, ny, nz);
    NEWIMAGE::volume<char> mask(nx, ny, nz);
    ima1.setdims(vx, vy, vz); ima2.setdims(vx, vy, vz); ones.setdims(vx, vy, vz); mask.setdims(vx, vy, vz);
    vector<char> b1 = slurp(dir + "/ima1.bin"), b2 = slurp(dir + "/ima2.bin"), bm = slurp(dir + "/mask.bin");
    const float* p1 = reinterpret_cast<const float*>(b1.data());
    const float* p2 = reinterpret_cast<const float*>(b2.data());
    unsigned int vi = 0;
    for (unsigned int k = 0; k < nz; k++) for (unsigned int j = 0; j < ny; j++) for (unsigned int i = 0; i < nx; i++, vi++) {
      ima1(i, j, k) = p1[vi]; ima2(i, j, k) = p2[vi]; mask(i, j, k) = bm[vi]; ones(i, j, k) = 1.0;
    }
    // The field and its derivatives, double and as float volumes.
    for (int fi = 0; fi < 4; fi++) {
      std::shared_ptr<NEWMAT::ColumnVector> g = f.Get(BASISFIELD::FieldIndex(fi));
      char tag[32]; snprintf(tag, 32, "field%d", fi);
      pvec(tag, *g);
      NEWIMAGE::volume<float> v(nx, ny, nz); v.setdims(vx, vy, vz);
      f.AsVolume(v, BASISFIELD::FieldIndex(fi));
      snprintf(tag, 32, "asvol%d", fi);
      for (unsigned int k = 0; k < nz; k++) for (unsigned int j = 0; j < ny; j++) for (unsigned int i = 0; i < nx; i++) pf(tag, v(i, j, k));
    }
    { ifstream pts(dir + "/points.txt"); double x, y, z;
      while (pts >> x >> y >> z) for (int fi = 0; fi < 4; fi++) pd("peek", f.Peek(x, y, z, BASISFIELD::FieldIndex(fi))); }
    { ifstream pts(dir + "/wide.txt"); int i, j, k;
      while (pts >> i >> j >> k) for (int fi = 0; fi < 4; fi++) pd("wide", f.PeekWide(i, j, k, BASISFIELD::FieldIndex(fi))); }
    vector<unsigned int> d0{0, 0, 0}, dx{1, 0, 0}, dy{0, 1, 0}, dz{0, 0, 1};
    pvec("jte_ima_mask", f.Jte(ima1, &mask));
    pvec("jte_ima", f.Jte(ima1, 0));
    pvec("jte_dx_ima_mask", f.Jte(dx, ima1, &mask));
    pvec("jte_prod_mask", f.Jte(ima1, ima2, &mask));
    pvec("jte_dy_prod", f.Jte(dy, ima1, ima2, 0));
    pvec("jte_dz_prod_mask", f.Jte(dz, ima1, ima2, &mask));
    pcsc("jtj_sym_mask", f.JtJ(ima1, ima2, &mask, MISCMATHS::BFMatrixDoublePrecision));
    pcsc("jtj_sym_ones", f.JtJ(ones, 0, MISCMATHS::BFMatrixDoublePrecision));
    pcsc("jtj_dx_sym", f.JtJ(dx, ima1, ones, &mask, MISCMATHS::BFMatrixDoublePrecision));
    pcsc("jtj_dx_dy", f.JtJ(dx, ima1, dy, ima2, &mask, MISCMATHS::BFMatrixDoublePrecision));
    pcsc("jtj_0_dz", f.JtJ(d0, ima1, dz, ones, 0, MISCMATHS::BFMatrixDoublePrecision));
    pd("memen", f.MemEnergy());
    pd("bendeng", f.BendEnergy());
    pvec("memen_grad", f.MemEnergyGrad());
    pvec("bend_grad", f.BendEnergyGrad());
    pcsc("memen_hess", f.MemEnergyHess(MISCMATHS::BFMatrixDoublePrecision));
    pcsc("bend_hess", f.BendEnergyHess(MISCMATHS::BFMatrixDoublePrecision));
    f.ScaleField(-1.5);
    pvec("scaled_field", *f.Get(BASISFIELD::FIELD));
  }
  catch (const std::exception& e) { printf("exception %s\n", e.what()); return 1; }
  return 0;
}
