// SPDX-License-Identifier: Apache-2.0
// rg_ on one matrix (float bits, row-major, as OpenNonSymmetricEigenSystem passes it), wr/wi/z
// preset to a sentinel to show which entries rg_ writes; then vnl_real_eigensystem itself.
#include <cstdio>
#include <cstring>
#include <cstdint>
#include <cstdlib>
#include <vnl/vnl_matrix.h>
#include <vnl/algo/vnl_real_eigensystem.h>
#include <vnl/algo/vnl_netlib.h>
static void pd(const char *k, double v) { uint64_t b; memcpy(&b, &v, 8); printf("%s %016llx %.17g\n", k, (unsigned long long)b, v); }
int main(int argc, char **argv) {
  int n = atoi(argv[1]);
  double d[64];
  for (int i = 0; i < n * n; i++) { uint32_t b = (uint32_t)strtoul(argv[2 + i], 0, 16); float f; memcpy(&f, &b, 4); d[i] = f; }
  // the Fortran copy: column-major
  double a[64], wr[8], wi[8], z[64], fv1[8]; long iv1[8];
  for (int r = 0; r < n; r++) for (int c = 0; c < n; c++) a[c * n + r] = d[r * n + c];
  double sent; uint64_t sb = 0x7ff4dead0000beefull; memcpy(&sent, &sb, 8);
  for (int i = 0; i < n; i++) { wr[i] = sent; wi[i] = sent; }
  for (int i = 0; i < n * n; i++) z[i] = sent;
  long nn = n, ierr = 0, matz = 1;
  v3p_netlib_rg_(&nn, &nn, a, wr, wi, &matz, z, iv1, fv1, &ierr);
  printf("ierr %ld\n", ierr);
  char k[32];
  for (int i = 0; i < n; i++) { snprintf(k, 32, "wr%d", i); pd(k, wr[i]); snprintf(k, 32, "wi%d", i); pd(k, wi[i]); }
  for (int c = 0; c < n; c++) for (int r = 0; r < n; r++) { snprintf(k, 32, "z(%d,%d)", r, c); pd(k, z[c * n + r]); }
  vnl_matrix<double> M(d, n, n);
  vnl_real_eigensystem es(M);
  for (int r = 0; r < n; r++) for (int c = 0; c < n; c++) { snprintf(k, 32, "Vreal(%d,%d)", r, c); pd(k, es.Vreal(r, c)); }
  for (int c = 0; c < n; c++) { snprintf(k, 32, "Dre%d", c); pd(k, es.D(c, c).real()); snprintf(k, 32, "Dim%d", c); pd(k, es.D(c, c).imag()); }
  return 0;
}
