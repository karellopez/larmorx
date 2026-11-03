// SPDX-License-Identifier: Apache-2.0
// Armadillo 14.2.1's fft and ifft (Kiss FFT, as fslmaths' FFT convolution runs them through
// armawrap's FFT and FFTI) of seeded complex vectors of the lengths given: one line per output
// value, "<n> <dir> <index> <re bits> <im bits>". Compared with larmorx-nc-core's KissFft by
// crates-nc/larmorx-nc-core/tests/fft_probe.rs (the same inputs) through check_fft.py.
#include <armadillo>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>

static uint32_t state = 12345u;
static double next() {
  state = state * 1664525u + 1013904223u;
  return (double)((float)(state >> 8) / 16777216.0f - 0.5f);
}

static uint64_t bits(double x) { uint64_t b; std::memcpy(&b, &x, 8); return b; }

int main(int argc, char** argv) {
  for (int a = 1; a < argc; ++a) {
    const arma::uword n = std::strtoul(argv[a], nullptr, 10);
    for (int rep = 0; rep < 3; ++rep) {
      arma::cx_vec x(n);
      for (arma::uword i = 0; i < n; ++i) {
        const double re = next();  // the real part drawn first (argument order is unspecified)
        const double im = rep == 0 ? 0.0 : next();
        x(i) = std::complex<double>(re, im);
      }
      arma::cx_vec f = arma::fft(x);
      arma::cx_vec g = arma::ifft(x);
      for (arma::uword i = 0; i < n; ++i)
        std::printf("%lu %d fwd %lu %016lx %016lx\n", n, rep, i, bits(f(i).real()), bits(f(i).imag()));
      for (arma::uword i = 0; i < n; ++i)
        std::printf("%lu %d inv %lu %016lx %016lx\n", n, rep, i, bits(g(i).real()), bits(g(i).imag()));
    }
  }
  return 0;
}
