// SPDX-License-Identifier: Apache-2.0
// The twiddle factors of Armadillo 14.2.1's Kiss FFT engine (`fft_engine_kissfft`, as fslmaths'
// FFT convolution builds it for each transform length), beside glibc's own functions on the same
// angles: one line per factor,
//   "tw <n> <dir> <i> <angle> <arma re> <arma im> <cexp re> <cexp im> <sincos cos> <sincos sin>
//    <cos> <sin>"
// as 64-bit patterns, `dir` 0 forward, 1 inverse. FSL's libfsl-newimage.so computes each factor
// with `cexp(0 + i (i k))` (its machine code calls glibc's `cexp`); glibc's `cexp` takes `sincos`.
// The functions are called through volatile pointers, so the compiler neither folds them nor
// merges a `sin` and a `cos` into one `sincos`. Compared with the replica's factors (correctly
// rounded `cos` and `sin`) by check_fft.py.
#include <armadillo>
#include <complex>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>

extern "C" {
__complex__ double cexp(__complex__ double);
void sincos(double, double*, double*);
double sin(double);
double cos(double);
}

static uint64_t bits(double x) { uint64_t b; std::memcpy(&b, &x, 8); return b; }

static __complex__ double (*volatile p_cexp)(__complex__ double) = cexp;
static void (*volatile p_sincos)(double, double*, double*) = sincos;
static double (*volatile p_sin)(double) = sin;
static double (*volatile p_cos)(double) = cos;

template <bool inverse>
static void print(arma::uword n) {
  arma::fft_engine_kissfft<std::complex<double>, inverse> e(n);
  // the engine's own angle step, as its constructor computes it
  const double k = double(inverse ? +2 : -2) * std::acos(double(-1)) / double(n);
  for (arma::uword i = 0; i < n; ++i) {
    const double y = i * k;
    const std::complex<double> a = e.coeffs_array[i];
    __complex__ double z;
    __real__ z = 0.0;
    __imag__ z = y;
    const __complex__ double c = p_cexp(z);
    double s = 0, co = 0;
    p_sincos(y, &s, &co);
    std::printf("tw %lu %d %lu %016lx %016lx %016lx %016lx %016lx %016lx %016lx %016lx %016lx\n",
                (unsigned long)n, inverse ? 1 : 0, (unsigned long)i, bits(y), bits(a.real()),
                bits(a.imag()), bits(__real__ c), bits(__imag__ c), bits(co), bits(s),
                bits(p_cos(y)), bits(p_sin(y)));
  }
}

int main(int argc, char** argv) {
  for (int a = 1; a < argc; ++a) {
    const arma::uword n = std::strtoul(argv[a], nullptr, 10);
    print<false>(n);
    print<true>(n);
  }
  return 0;
}
