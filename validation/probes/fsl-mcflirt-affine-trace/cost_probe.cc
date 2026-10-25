// SPDX-License-Identifier: Apache-2.0
// A probe of FSL's cost functions (not FSL code: it calls FSL 6.0.7.17's libfsl-newimage, as
// mcflirt does): mcflirt's Costfn for one volume of a series against a reference resampled to
// a scale, evaluated at the affine matrices the larmorx-nc replica printed (its debug lines),
// with FSL's cost and the replica's side by side.
//
// Usage: cost_probe <series> <reference> <volume> <scale> <bins> <cost: normcorr|...> <debug lines>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <iostream>
#include <sstream>
#include <string>

#include "newimage/newimageall.h"
#include "newimage/costfns.h"

using namespace NEWIMAGE;

static double from_hex64(const std::string& h) {
  uint64_t u = std::strtoull(h.c_str(), nullptr, 16);
  double d;
  std::memcpy(&d, &u, 8);
  return d;
}

static uint32_t bits32(float f) {
  uint32_t u;
  std::memcpy(&u, &f, 4);
  return u;
}

int main(int argc, char** argv) {
  if (argc < 8) {
    std::cerr << "usage: cost_probe series reference volume scale bins cost debuglines\n";
    return 2;
  }
  volume4D<float> ts;
  read_volume4D(ts, argv[1]);
  volume<float> ext;
  read_volume(ext, argv[2]);
  int vol = std::atoi(argv[3]);
  float scale = std::atof(argv[4]);
  int bins = std::atoi(argv[5]);
  std::string costname = argv[6];
  volume<float> refvol = isotropic_resample(ext, scale);
  volume<float> testvol = ts[vol];
  Costfn c(refvol, testvol);
  c.set_no_bins(bins / (int)scale);
  c.smoothsize = 1.0;
  costfns ct = NormCorr;
  if (costname == "leastsq") ct = LeastSq;
  c.set_costfn(ct);
  std::ifstream in(argv[7]);
  std::string line;
  int n = 0, differ = 0;
  while (std::getline(in, line)) {
    if (line.rfind("DBG ", 0) != 0) continue;
    std::istringstream ss(line);
    std::string tag, v, sc, mats, costhex, coste;
    ss >> tag >> v >> sc >> mats >> costhex >> coste;
    NEWMAT::Matrix aff(4, 4);
    std::stringstream ms(mats);
    std::string h;
    for (int k = 0; k < 16 && std::getline(ms, h, ','); k++) aff(k % 4 + 1, k / 4 + 1) = from_hex64(h);
    float f = c.cost(aff);
    uint32_t rb = (uint32_t)std::strtoul(costhex.c_str(), nullptr, 16);
    bool same = bits32(f) == rb;
    differ += !same;
    std::printf("%d fsl %08x %.9g replica %08x %s\n", n, bits32(f), f, rb, same ? "same" : "DIFFER");
    n++;
  }
  std::printf("%d evaluations, %d differ\n", n, differ);
  return 0;
}
