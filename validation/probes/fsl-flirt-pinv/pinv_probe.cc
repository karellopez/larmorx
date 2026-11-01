// SPDX-License-Identifier: Apache-2.0
// FLIRT's parameter-subset pseudo-inverse (MISCMATHS::pinv = arma::pinv(A, 0), Armadillo 14.2.1
// as FSL 6.0.7.23 bundles it, on FSL's own LAPACK) for the masks FLIRT builds: the search's
// 12 x 4 mask (one common scale and three translations), its 12 x 3 mask (translations), the
// 2D schedule's paramsubset (rz, tx, ty), and masks read from standard input (one per line: the
// number of columns, then 12 values per column, as FLIRT's `setoption paramsubset` takes them).
// Prints every element of the result as a C99 hexadecimal float, row by row.
//
// Build (see README.md):
//   g++ -O2 -std=c++17 -DARMA_DONT_USE_WRAPPER -I ~/fsl-6.0.7.23/include pinv_probe.cc \
//       -L ~/fsl-6.0.7.23/lib -llapack -lblas -Wl,-rpath,$HOME/fsl-6.0.7.23/lib -o pinv_probe
#include <armadillo>
#include <cstdio>
#include <iostream>
#include <sstream>
#include <string>

static void show(const char* name, const arma::mat& m) {
  arma::mat p = arma::pinv(m, 0);
  std::printf("%s %llu %llu\n", name, (unsigned long long)p.n_rows, (unsigned long long)p.n_cols);
  for (arma::uword r = 0; r < p.n_rows; ++r) {
    for (arma::uword c = 0; c < p.n_cols; ++c) std::printf(" %a", p(r, c));
    std::printf("\n");
  }
}

int main(int argc, char** argv) {
  if (argc > 1 && std::string(argv[1]) == "--stdin") {
    std::string line;
    int k = 0;
    while (std::getline(std::cin, line)) {
      std::istringstream is(line);
      int n = 0;
      if (!(is >> n) || n <= 0) continue;
      arma::mat m(12, n, arma::fill::zeros);
      for (int c = 0; c < n; ++c)
        for (int r = 0; r < 12; ++r) is >> m(r, c);
      std::string name = "stdin" + std::to_string(k++);
      show(name.c_str(), m);
    }
    return 0;
  }
  arma::mat m4(12, 4, arma::fill::zeros);
  m4(6, 0) = m4(7, 0) = m4(8, 0) = 1.0;
  m4(3, 1) = 1.0;
  m4(4, 2) = 1.0;
  m4(5, 3) = 1.0;
  show("search12x4", m4);
  arma::mat m3(12, 3, arma::fill::zeros);
  m3(3, 0) = m3(4, 1) = m3(5, 2) = 1.0;
  show("search12x3", m3);
  arma::mat m2(12, 3, arma::fill::zeros);
  m2(2, 0) = m2(3, 1) = m2(4, 2) = 1.0;
  show("sched2d", m2);
  arma::mat id(12, 12, arma::fill::eye);
  show("identity12", id);
  return 0;
}
