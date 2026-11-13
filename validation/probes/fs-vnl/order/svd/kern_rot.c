// SPDX-License-Identifier: Apache-2.0
// The order in which netlib's `srot` (unit strides), as the FreeSurfer unit oracle's ITK 4.13.2 built
// it (libitkv3p_netlib through libfsvnl.so), passes NaN operands on: the exploration behind
// larmorx_core::vnl_svd_f32's rotation tables (the probe's family `svdnan` checks them).
// `x_i' = c*x_i + s*y_i`, `y_i' = c*y_i - s*x_i`: for every subset of {c, s, x, y} given NaNs with
// distinct payloads (c 200, s 300, x_i 1 + i, y_i 100 + i; the others the numbers 2, 3, 5, 7), the
// NaN each element ends with (C, S, X, Y; '.' a number). The two vectors are columns of one array
// (as LINPACK passes them): y after or before x by `d` elements, x at four alignments.
//   gcc -O1 -DNMAX=40 -o kern_rot kern_rot.c <out>/libfsvnl.so -lm -Wl,-rpath,<out>
//   ./kern_rot > rot.txt; python3 -P infer_rot.py rot.txt
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <stdlib.h>
typedef long integer;
extern int v3p_netlib_srot_(integer *, float *, integer *, float *, integer *, float *, float *);
static float fnan(unsigned tag) { uint32_t b = 0x7fc00000u | tag; float f; memcpy(&f, &b, 4); return f; }
static char who(float f) {
  uint32_t b; memcpy(&b, &f, 4);
  if ((b & 0x7f800000u) != 0x7f800000u || !(b & 0x7fffffu)) return '.';
  unsigned t = b & 0x3fffffu;
  if (t == 200) return 'C';
  if (t == 300) return 'S';
  if (t >= 1 && t < 100) return 'X';
  if (t >= 100 && t < 200) return 'Y';
  return '?';
}
int main(void) {
  integer one = 1;
  float *buf = aligned_alloc(64, 4 * 4096);
  int ds[] = {0, 1, 2, 3, 5, 8, 17};
  for (integer n = 1; n <= NMAX; n++)
    for (int off = 0; off < 4; off++)
      for (int k = 0; k < 7; k++)
        for (int dir = 0; dir < 2; dir++) {
          integer d = n + ds[k];
          float *x = buf + 256 + off, *y = dir ? x - d : x + d;
          printf("n=%ld off=%d d=%ld dir=%d:", n, off, d, dir);
          for (int sub = 1; sub < 16; sub++) {
            float c = (sub & 1) ? fnan(200) : 2.0f, s = (sub & 2) ? fnan(300) : 3.0f;
            for (int i = 0; i < n; i++) {
              x[i] = (sub & 4) ? fnan(1 + i) : 5.0f;
              y[i] = (sub & 8) ? fnan(100 + i) : 7.0f;
            }
            v3p_netlib_srot_(&n, x, &one, y, &one, &c, &s);
            putchar(' ');
            for (int i = 0; i < n; i++) putchar(who(x[i]));
            putchar('/');
            for (int i = 0; i < n; i++) putchar(who(y[i]));
          }
          putchar('\n');
        }
  return 0;
}
