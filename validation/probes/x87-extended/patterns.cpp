// SPDX-License-Identifier: Apache-2.0
// The arithmetic patterns of the FreeSurfer 8.2.0 functions that compute in long double
// (docs/freesurfer/variation.md section 1), written for this probe as plain loops (no FreeSurfer
// code): long double sums of double products, sums and squares with a mean, variance and
// standard deviation, edge vectors and cross products from float coordinates, unit vectors and
// their gradients, float values normalised by a long double sum, squared length differences.
// Compiled by GCC 8.4.0 with FreeSurfer's C++ flags (build.sh); larmorx_core::x87::cases::Pattern
// computes the same with larmorx_core::x87::F80.
#include <math.h>

typedef long double ld;

extern "C" double pat_dot(const double *a, const double *b, int n) {
  ld s = 0;
  for (int i = 0; i < n; i++) s += ((ld)a[i] * b[i]);
  return s;
}

extern "C" void pat_stats(const double *x, int n, double *out) {
  ld sum = 0, sum2 = 0;
  for (int i = 0; i < n; i++) {
    sum += x[i];
    sum2 += (ld)x[i] * x[i];
  }
  ld mean = sum / n;
  ld var = (sum2 - n * mean * mean) / (n - 1);
  ld sd = sqrt(var);
  out[0] = mean;
  out[1] = var;
  out[2] = sd;
}

extern "C" void pat_cross(const float *v, double *out) {
  ld ax = (ld)v[3] - v[0], ay = (ld)v[4] - v[1], az = (ld)v[5] - v[2];
  ld bx = (ld)v[6] - v[3], by = (ld)v[7] - v[4], bz = (ld)v[8] - v[5];
  ld cx = ay * bz - az * by;
  ld cy = az * bx - ax * bz;
  ld cz = ax * by - ay * bx;
  ld cl = sqrt(cx * cx + cy * cy + cz * cz);
  out[0] = cx;
  out[1] = cy;
  out[2] = cz;
  out[3] = cl;
}

extern "C" void pat_unit(const double *v, double *out) {
  ld w[3], L = 0;
  for (int k = 0; k < 3; k++) {
    w[k] = (ld)v[3 + k] - v[k];
    L += (w[k] * w[k]);
  }
  L = sqrt(L);
  out[0] = L;
  for (int k = 0; k < 3; k++) out[1 + k] = w[k] / L;
  ld iL3 = 1.0 / (L * L * L);
  ld iL = 1.0 / L;
  for (int k = 0; k < 3; k++)
    for (int m = 0; m < 3; m++) {
      ld A = (w[k] * w[m]) * iL3;
      if (k == m) A -= iL;
      out[4 + 3 * k + m] = A;
    }
}

extern "C" void pat_smooth(const float *v, int n, float *out, double *out2) {
  ld scale = 0, vmf = 0;
  for (int i = 0; i < n; i++) {
    float val = v[i];
    scale += val;
    vmf += val * val;
  }
  for (int i = 0; i < n; i++) out[i] = v[i] / scale;
  out2[0] = vmf;
  out2[1] = 1.0 / vmf;
}

extern "C" void pat_edgecost(const double *x, int n, double *out) {
  double L0 = x[0];
  ld total = 0;
  for (int i = 0; i < n; i++) {
    double len = x[1 + i];
    ld dL = len - L0;
    total += dL * dL;
    ld a = 2.0 * dL / len;
    out[i] = a;
  }
  out[n] = total;
}
