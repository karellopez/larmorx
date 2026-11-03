// SPDX-License-Identifier: Apache-2.0
// The FSL side of fslmaths' library probe: runs FSL 6.0.7.23's own code (Armadillo 14.2.1's
// headers, libfsl-cprob.so) on the values of one input file and prints every result as its
// 64-bit pattern (16 hexadecimal digits), through C's stdio, so that Cephes' error reports
// (mtherr, printed with printf) appear where FSL prints them.
//
//   probe arma  <file>   each line a vector (bit patterns separated by blanks; an empty line
//                        the empty vector): "max <arma::max(v)> min <arma::min(v)>
//                        mmax <Mat.max()> mmin <Mat.min()>"
//   probe cprob <file>   each line one value x: "ndtr <ndtr(x)>", "ndtri <ndtri(x)>",
//                        "erf <erf(x)>", "erfc <erfc(x)>", each after what it reported
#include <armadillo>
#include <cinttypes>
#include <cstdio>
#include <cstring>
#include <fstream>
#include <sstream>
#include <string>
#include <vector>

#include "cprob/cprob.h"

static double from_bits(const std::string& s) {
  uint64_t b = std::stoull(s, nullptr, 16);
  double d;
  std::memcpy(&d, &b, 8);
  return d;
}

static void put(const char* tag, double d) {
  uint64_t b;
  std::memcpy(&b, &d, 8);
  std::printf("%s %016" PRIx64 "\n", tag, b);
}

int main(int argc, char** argv) {
  if (argc != 3) {
    std::fprintf(stderr, "usage: probe arma|cprob <file>\n");
    return 2;
  }
  std::string mode(argv[1]);
  std::ifstream in(argv[2]);
  std::string line;
  while (std::getline(in, line)) {
    std::istringstream ls(line);
    std::vector<double> v;
    std::string w;
    while (ls >> w) v.push_back(from_bits(w));
    if (mode == "arma") {
      arma::vec a(v);
      arma::mat m(v.data(), v.size(), 1);
      // max()/min() of an empty object throw in Armadillo: not a case
      if (v.empty()) continue;
      put("max", arma::max(a));
      put("min", arma::min(a));
      put("mmax", m.max());
      put("mmin", m.min());
    } else {
      double x = v.at(0);
      put("ndtr", CPROB::ndtr(x));
      std::fflush(stdout);
      put("ndtri", CPROB::ndtri(x));
      put("erf", CPROB::erf(x));
      put("erfc", CPROB::erfc(x));
    }
  }
  return 0;
}
