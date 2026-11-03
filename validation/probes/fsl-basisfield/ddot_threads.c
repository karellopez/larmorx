/* SPDX-License-Identifier: Apache-2.0
 * ddot_threads: FSL 6.0.7.23's BLAS ddot (OpenBLAS 0.3.33) on n seeded elements; prints the
 * result's bits. Run it with OPENBLAS_NUM_THREADS set to several values to see whether the
 * thread count changes the dot products topup's energies and optimisers compute (README.md).
 * Build: gcc -O1 ddot_threads.c -L ~/fsl-6.0.7.23/lib -lblas -Wl,-rpath,$HOME/fsl-6.0.7.23/lib */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
double ddot_(const int*, const double*, const int*, const double*, const int*);
int main(int argc, char** argv)
{
  int n = atoi(argv[1]), one = 1;
  double *x = malloc(n * 8), *y = malloc(n * 8);
  unsigned long long s = 88172645463325252ULL;
  for (int i = 0; i < n; i++) {
    s ^= s << 13; s ^= s >> 7; s ^= s << 17; x[i] = (double)(s % 100000) / 977.0 - 50.0;
    s ^= s << 13; s ^= s >> 7; s ^= s << 17; y[i] = (double)(s % 100000) / 1013.0 - 40.0;
  }
  double d = ddot_(&n, x, &one, y, &one);
  unsigned long long b;
  memcpy(&b, &d, 8);
  printf("%016llx\n", b);
  return 0;
}
