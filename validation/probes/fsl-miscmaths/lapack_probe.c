// SPDX-License-Identifier: Apache-2.0
// lapack_probe: FSL 6.0.7.23's own OpenBLAS (liblapack.so.3, libblas.so) on the systems of one
// case file: dgesv (the LU factors, the pivots and the solutions), dsyrk('U', 'T'), dgemm, the
// symmetric indefinite solve as Armadillo 14.2.1's solve_sym_fast calls it (dsytrf('L') after its
// workspace query, then dsytrs('L')) and the BLAS routines that solve calls, each value printed
// as its hexadecimal bits, as the replica's side (crates-nc/larmorx-nc-core/tests/lapack_probe.rs)
// prints them. See README.md.
//
// Usage: lapack_probe <case file>
// The case file: u32 kind (0 gesv, 1 syrk, 2 gemm, 3 sym, 4 blas); gesv: u32 n, u32 nrhs, A (n*n
// f64, column-major), B (n*nrhs f64); syrk: u32 k, u32 n, A (k*n f64); gemm: u32 m, u32 n, u32 k,
// A (m*k f64), B (k*n f64): C = A*B; sym: u32 n, u32 nrhs, A (n*n), B (n*nrhs); blas: u32 op,
// u32 m, u32 n, then op's arrays (see blas_op below).
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern void dgesv_(int* n, int* nrhs, double* a, int* lda, int* ipiv, double* b, int* ldb, int* info);
extern void dgemm_(const char* ta, const char* tb, int* m, int* n, int* k, double* alpha,
                   double* a, int* lda, double* b, int* ldb, double* beta, double* c, int* ldc);
extern void dsyrk_(const char* uplo, const char* trans, int* n, int* k, double* alpha, double* a,
                   int* lda, double* beta, double* c, int* ldc);
extern void dsytrf_(const char* uplo, int* n, double* a, int* lda, int* ipiv, double* work,
                    int* lwork, int* info);
extern void dsytrs_(const char* uplo, int* n, int* nrhs, double* a, int* lda, int* ipiv, double* b,
                    int* ldb, int* info);
extern void daxpy_(int* n, double* da, double* x, int* incx, double* y, int* incy);
extern void dger_(int* m, int* n, double* alpha, double* x, int* incx, double* y, int* incy,
                  double* a, int* lda);
extern void dsyr_(const char* uplo, int* n, double* alpha, double* x, int* incx, double* a,
                  int* lda);
extern void dgemv_(const char* t, int* m, int* n, double* alpha, double* a, int* lda, double* x,
                   int* incx, double* beta, double* y, int* incy);

static void pd(const char* tag, double v)
{
  unsigned long long b; memcpy(&b, &v, 8);
  printf("%s %016llx\n", tag, b);
}

static double* readd(FILE* f, size_t n)
{
  double* p = malloc(sizeof(double) * (n ? n : 1));
  if (fread(p, 8, n, f) != n) exit(2);
  return p;
}

// The BLAS routines alone, in the shapes dsytrf/dsytrs call them (m, n from the case):
// 0 daxpy unit strides: alpha, x (m), y (m) -> y;
// 1 dger as dsytrs: x (m), B ((m+1) x n, ld m+1): B(1.., :) -= x * B(0, :) -> B;
// 2 dsyr('L') unit x: alpha, x (m), A (m x m, ld m+3) -> A;
// 3 dgemv('T') as dsytrs: x (m), B ((m+1) x n, ld m+1): B(0, :) -= B(1.., :)^T x -> B(0, :);
// 4 dgemv('N') as dlasyf: A (m x n, ld m), X (n*5: x strided by 5), y (m) -> y;
// 5 dgemm('N', 'T') as dlasyf: u32 k, A (m x k, ld m), W ((n+2) x k, ld n+2), C (m x n, ld m):
//   C -= A W(0..n, :)^T -> C.
static void blas_op(FILE* f, unsigned int op, int m, int n)
{
  int one = 1;
  double mone = -1.0, pone = 1.0;
  if (op == 0) {
    double alpha; if (fread(&alpha, 8, 1, f) != 1) exit(2);
    double* x = readd(f, m); double* y = readd(f, m);
    daxpy_(&m, &alpha, x, &one, y, &one);
    for (int i = 0; i < m; i++) pd("y", y[i]);
  } else if (op == 1) {
    int ld = m + 1;
    double* x = readd(f, m); double* b = readd(f, (size_t)ld * n);
    dger_(&m, &n, &mone, x, &one, b, &ld, b + 1, &ld);
    for (int i = 0; i < ld * n; i++) pd("b", b[i]);
  } else if (op == 2) {
    int ld = m + 3;
    double alpha; if (fread(&alpha, 8, 1, f) != 1) exit(2);
    double* x = readd(f, m); double* a = readd(f, (size_t)ld * m);
    dsyr_("L", &m, &alpha, x, &one, a, &ld);
    for (int i = 0; i < ld * m; i++) pd("a", a[i]);
  } else if (op == 3) {
    int ld = m + 1;
    double* x = readd(f, m); double* b = readd(f, (size_t)ld * n);
    dgemv_("T", &m, &n, &mone, b + 1, &ld, x, &one, &pone, b, &ld);
    for (int j = 0; j < n; j++) pd("y", b[j * ld]);
  } else if (op == 4) {
    int inc = 5;
    double* a = readd(f, (size_t)m * n); double* x = readd(f, (size_t)n * 5); double* y = readd(f, m);
    dgemv_("N", &m, &n, &mone, a, &m, x, &inc, &pone, y, &one);
    for (int i = 0; i < m; i++) pd("y", y[i]);
  } else if (op == 5) {
    unsigned int kk; if (fread(&kk, 4, 1, f) != 1) exit(2);
    int k = (int)kk, ldw = n + 2;
    double* a = readd(f, (size_t)m * k); double* w = readd(f, (size_t)ldw * k);
    double* c = readd(f, (size_t)m * n);
    dgemm_("N", "T", &m, &n, &k, &mone, a, &m, w, &ldw, &pone, c, &m);
    for (int i = 0; i < m * n; i++) pd("c", c[i]);
  }
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
  } else if (kind == 2) {
    int m = (int)p, n = (int)q, k;
    unsigned int kk;
    if (fread(&kk, 4, 1, f) != 1) return 2;
    k = (int)kk;
    double alpha = 1.0, beta = 0.0;
    double* a = malloc(sizeof(double) * m * k);
    double* b = malloc(sizeof(double) * k * n);
    double* c = calloc((size_t)m * n, sizeof(double));
    if (fread(a, 8, (size_t)m * k, f) != (size_t)m * k) return 2;
    if (fread(b, 8, (size_t)k * n, f) != (size_t)k * n) return 2;
    dgemm_("N", "N", &m, &n, &k, &alpha, a, &m, b, &k, &beta, c, &m);
    for (int i = 0; i < m * n; i++) pd("c", c[i]);
  } else if (kind == 3) {
    // Armadillo 14.2.1's auxlib::solve_sym_fast: lwork = max(16, n), the workspace query for
    // n > 16, dsytrf('L'), then (info == 0) dsytrs('L').
    int n = (int)p, nrhs = (int)q, info = 0;
    double* a = readd(f, (size_t)n * n);
    double* b = readd(f, (size_t)n * nrhs);
    int* ipiv = malloc(sizeof(int) * n);
    int lwork = n > 16 ? n : 16;
    if (n > 16) {
      double wq[2] = {0, 0};
      int lq = -1;
      dsytrf_("L", &n, a, &n, ipiv, wq, &lq, &info);
      if ((int)wq[0] > lwork) lwork = (int)wq[0];
      printf("lwork %d\n", lwork);
    }
    double* work = malloc(sizeof(double) * lwork);
    dsytrf_("L", &n, a, &n, ipiv, work, &lwork, &info);
    printf("info %d\n", info);
    for (int i = 0; i < n; i++) printf("ipiv %d\n", ipiv[i]);
    for (int j = 0; j < n; j++)
      for (int i = j; i < n; i++) pd("f", a[i + j * n]);
    if (info == 0) {
      dsytrs_("L", &n, &nrhs, a, &n, ipiv, b, &n, &info);
      printf("info2 %d\n", info);
      for (int i = 0; i < n * nrhs; i++) pd("x", b[i]);
    }
  } else if (kind == 4) {
    unsigned int nn;
    if (fread(&nn, 4, 1, f) != 1) return 2;
    blas_op(f, p, (int)q, (int)nn);
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
