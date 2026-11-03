// SPDX-License-Identifier: Apache-2.0
// lapack_probe: FSL 6.0.7.23's own OpenBLAS (liblapack.so.3, libblas.so) on the systems of one
// case file: dgesv (the LU factors, the pivots and the solutions) and dsyrk('U', 'T'), each value
// printed as its hexadecimal bits, as the replica's side
// (crates-nc/larmorx-nc-core/tests/lapack_probe.rs) prints them. See README.md.
//
// Usage: lapack_probe <case file>
// The case file: u32 kind (0 gesv, 1 syrk); gesv: u32 n, u32 nrhs, A (n*n f64, column-major),
// B (n*nrhs f64); syrk: u32 k, u32 n, A (k*n f64).
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern void dgesv_(int* n, int* nrhs, double* a, int* lda, int* ipiv, double* b, int* ldb, int* info);
extern void dsyrk_(const char* uplo, const char* trans, int* n, int* k, double* alpha, double* a,
                   int* lda, double* beta, double* c, int* ldc);

static void pd(const char* tag, double v)
{
  unsigned long long b; memcpy(&b, &v, 8);
  printf("%s %016llx\n", tag, b);
}

int main(int argc, char* argv[])
{
  if (argc < 2) { fprintf(stderr, "usage: lapack_probe <case file>\n"); return 2; }
  FILE* f = fopen(argv[1], "rb");
  if (!f) return 2;
  unsigned int kind, p, q;
  if (fread(&kind, 4, 1, f) != 1 || fread(&p, 4, 1, f) != 1 || fread(&q, 4, 1, f) != 1) return 2;
  if (kind == 0) {
    int n = (int)p, nrhs = (int)q, info = 0;
    double* a = malloc(sizeof(double) * n * n);
    double* b = malloc(sizeof(double) * n * nrhs);
    int* ipiv = malloc(sizeof(int) * n);
    if (fread(a, 8, (size_t)n * n, f) != (size_t)n * n) return 2;
    if (fread(b, 8, (size_t)n * nrhs, f) != (size_t)n * nrhs) return 2;
    dgesv_(&n, &nrhs, a, &n, ipiv, b, &n, &info);
    printf("info %d\n", info);
    for (int i = 0; i < n; i++) printf("ipiv %d\n", ipiv[i]);
    for (int i = 0; i < n * n; i++) pd("lu", a[i]);
    for (int i = 0; i < n * nrhs; i++) pd("x", b[i]);
  } else {
    int k = (int)p, n = (int)q;
    double alpha = 1.0, beta = 0.0;
    double* a = malloc(sizeof(double) * k * n);
    double* c = calloc((size_t)n * n, sizeof(double));
    if (fread(a, 8, (size_t)k * n, f) != (size_t)k * n) return 2;
    dsyrk_("U", "T", &n, &k, &alpha, a, &k, &beta, c, &n);
    for (int j = 0; j < n; j++)
      for (int i = 0; i <= j; i++) pd("c", c[i + j * n]);
  }
  return 0;
}
