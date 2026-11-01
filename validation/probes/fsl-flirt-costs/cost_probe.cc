// SPDX-License-Identifier: Apache-2.0
// One FLIRT cost function of FSL 6.0.7.23's newimage library (libfsl-newimage), evaluated on a
// reference and a test volume for matrices read from standard input (16 numbers per line, row
// by row), each printed as a C99 hexadecimal float: to compare larmorx-nc-core's translation
// with FSL's own code bit for bit (larmorx-nc-flirt's `cost_probe` test prints the same).
//
//   cost_probe <ref.nii> <test.nii> <labeldiff|labeldiff_smoothed|leastsq_smoothed> [smooth]
//
// Build (see README.md):
//   g++ -O2 -std=c++17 -DARMA_DONT_USE_WRAPPER -I ~/fsl-6.0.7.23/include cost_probe.cc \
//     -L ~/fsl-6.0.7.23/lib -lfsl-newimage -lfsl-miscmaths -lfsl-NewNifti -lfsl-znz \
//     -lfsl-cprob -lfsl-utils -llapack -lblas -lz -Wl,-rpath,$HOME/fsl-6.0.7.23/lib -o cost_probe
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <iostream>
#include <string>

#include "armawrap/newmat.h"
#include "newimage/newimageall.h"

namespace NEWIMAGE {
float p_labeldiff(const volume<float>& vref, const volume<float>& vtest, const NEWMAT::Matrix& aff);
float p_labeldiff_smoothed(const volume<float>& vref, const volume<float>& vtest,
                           const NEWMAT::Matrix& aff, const float smoothsize);
float p_leastsquares_smoothed(const volume<float>& vref, const volume<float>& vtest,
                              const NEWMAT::Matrix& aff, const float smoothsize);
}  // namespace NEWIMAGE

int main(int argc, char** argv) {
  if (argc < 4) {
    std::fprintf(stderr, "usage: cost_probe ref test cost [smooth]\n");
    return 2;
  }
  NEWIMAGE::volume<float> ref, test;
  NEWIMAGE::read_volume(ref, argv[1]);
  NEWIMAGE::read_volume(test, argv[2]);
  std::string cost = argv[3];
  float smooth = argc > 4 ? std::stof(argv[4]) : 1.0f;
  double m[16];
  while (true) {
    for (int k = 0; k < 16; ++k)
      if (!(std::cin >> m[k])) return 0;
    NEWMAT::Matrix aff(4, 4);
    for (int r = 0; r < 4; ++r)
      for (int c = 0; c < 4; ++c) aff(r + 1, c + 1) = m[4 * r + c];
    float v = 0;
    if (cost == "labeldiff")
      v = NEWIMAGE::p_labeldiff(ref, test, aff);
    else if (cost == "labeldiff_smoothed")
      v = NEWIMAGE::p_labeldiff_smoothed(ref, test, aff, smooth);
    else if (cost == "leastsq_smoothed")
      v = NEWIMAGE::p_leastsquares_smoothed(ref, test, aff, smooth);
    uint32_t bits;
    std::memcpy(&bits, &v, 4);
    std::printf("%08x %a\n", bits, v);
  }
}
