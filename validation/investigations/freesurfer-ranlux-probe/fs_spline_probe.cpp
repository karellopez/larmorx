// SPDX-License-Identifier: Apache-2.0
// A black-box probe of FreeSurfer 8.2.0's cubic spline (utils/numerics.cpp's SplineCubicSet,
// SplineCubicValue, d3_np_fs and their wrappers OpenSpline and OpenSplint), linked against the unit
// oracle's libutils.a (GCC 8.4.0, FreeSurfer's flags). It holds only calls: no FreeSurfer or
// Burkardt code. For specs/burkardt-spline.md.
//
// Usage:
//   fs_spline_probe cases < cases.txt
//     one case per input line, every float as the 8 hex digits of its bits:
//       <n> <ibcbeg> <ybcbeg> <ibcend> <ybcend> <m> t[0..n-1] y[0..n-1] tval[0..m-1]
//     prints per case: "ypp" and the n second derivatives, then one line per tval:
//       "val" yval ypval yppval (ypval and yppval start as 7fc00bad: an output left unwritten keeps it);
//     then "wrap" and OpenSpline(t, y, n, ybcbeg, ybcend)'s n values and OpenSplint's m values
//     (the wrappers always use first-derivative ends, so ibcbeg and ibcend do not apply there).
//   fs_spline_probe d3 < systems.txt
//     one system per line: <n> a[0..3n-1] b[0..n-1] (hex floats); prints "x" and the solution, or
//     "null" when d3_np_fs refuses (a zero diagonal entry). Entries of a it never reads may be 0.
//   fs_spline_probe set1 <n> <ibcbeg> <ibcend> t... y...   (decimal; one call, for the error messages)
#include <cinttypes>
#include <cstdio>
#include <cstdlib>
#include <cstring>

#include "numerics.h"

float *d3_np_fs(int n, float a[], float b[]);

static float hexf(const char *s)
{
  uint32_t b = (uint32_t)strtoul(s, NULL, 16);
  float f;
  memcpy(&f, &b, 4);
  return f;
}

static void putf(float f)
{
  uint32_t b;
  memcpy(&b, &f, 4);
  printf(" %08" PRIx32, b);
}

static char *tok(char **p)
{
  return strtok_r(NULL, " \t\n", p);
}

int main(int argc, char **argv)
{
  if (argc < 2) return 2;
  static char line[1 << 20];
  if (!strcmp(argv[1], "cases")) {
    while (fgets(line, sizeof line, stdin)) {
      char *save = NULL;
      char *first = strtok_r(line, " \t\n", &save);
      if (!first) continue;
      int n = atoi(first);
      int ibcbeg = atoi(tok(&save));
      float ybcbeg = hexf(tok(&save));
      int ibcend = atoi(tok(&save));
      float ybcend = hexf(tok(&save));
      int m = atoi(tok(&save));
      float *t = new float[n], *y = new float[n], *tv = new float[m];
      for (int i = 0; i < n; i++) t[i] = hexf(tok(&save));
      for (int i = 0; i < n; i++) y[i] = hexf(tok(&save));
      for (int i = 0; i < m; i++) tv[i] = hexf(tok(&save));
      float *ypp = SplineCubicSet(n, t, y, ibcbeg, ybcbeg, ibcend, ybcend);
      printf("ypp");
      for (int i = 0; i < n; i++) putf(ypp[i]);
      printf("\n");
      for (int i = 0; i < m; i++) {
        float yp = hexf("7fc00bad"), ypp2 = hexf("7fc00bad");
        float v = SplineCubicValue(n, t, tv[i], y, ypp, &yp, &ypp2);
        printf("val");
        putf(v);
        putf(yp);
        putf(ypp2);
        printf("\n");
      }
      float *w = new float[n];
      OpenSpline(t, y, n, ybcbeg, ybcend, w);
      printf("wrap");
      for (int i = 0; i < n; i++) putf(w[i]);
      for (int i = 0; i < m; i++) {
        float v;
        OpenSplint(t, y, w, n, tv[i], &v);
        putf(v);
      }
      printf("\n");
      fflush(stdout);
      delete[] t;
      delete[] y;
      delete[] tv;
      delete[] w;
      delete[] ypp;
    }
    return 0;
  }
  if (!strcmp(argv[1], "d3")) {
    while (fgets(line, sizeof line, stdin)) {
      char *save = NULL;
      char *first = strtok_r(line, " \t\n", &save);
      if (!first) continue;
      int n = atoi(first);
      float *a = new float[3 * n], *b = new float[n];
      for (int i = 0; i < 3 * n; i++) a[i] = hexf(tok(&save));
      for (int i = 0; i < n; i++) b[i] = hexf(tok(&save));
      float *x = d3_np_fs(n, a, b);
      if (!x) {
        printf("null\n");
      }
      else {
        printf("x");
        for (int i = 0; i < n; i++) putf(x[i]);
        printf("\n");
        delete[] x;
      }
      delete[] a;
      delete[] b;
    }
    return 0;
  }
  if (!strcmp(argv[1], "set1") && argc >= 5) {
    int n = atoi(argv[2]);
    int ibcbeg = atoi(argv[3]), ibcend = atoi(argv[4]);
    if (n < 0 || argc < 5 + 2 * n) return 2;
    float *t = new float[n > 0 ? n : 1], *y = new float[n > 0 ? n : 1];
    for (int i = 0; i < n; i++) t[i] = strtof(argv[5 + i], NULL);
    for (int i = 0; i < n; i++) y[i] = strtof(argv[5 + n + i], NULL);
    float *ypp = SplineCubicSet(n, t, y, ibcbeg, 0.0f, ibcend, 0.0f);
    printf("ypp");
    for (int i = 0; i < n; i++) putf(ypp[i]);
    printf("\n");
    return 0;
  }
  return 2;
}
