/* SPDX-License-Identifier: Apache-2.0 */
/* glibc's log2 main path against Arm's two forms (fused r, or the tab2 split without fma), every
   combination of the implicit fusion sites: the fewest differences. */
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <stdlib.h>
#include <math.h>
#define HAVE_FAST_FMA 0
#include "math_config.h"
#define T __log2_data.tab
#define T2 __log2_data.tab2
#define A __log2_data.poly
#define IH __log2_data.invln2hi
#define IL __log2_data.invln2lo
static uint64_t sm(uint64_t i){uint64_t z=i+0x9e3779b97f4a7c15ULL;z=(z^(z>>30))*0xbf58476d1ce4e5b9ULL;z=(z^(z>>27))*0x94d049bb133111ebULL;return z^(z>>31);}
static uint64_t b(double x){uint64_t u;memcpy(&u,&x,8);return u;}
static double d(uint64_t u){double x;memcpy(&x,&u,8);return x;}
static unsigned M;
static double F(double a, double b, double c, int bit){ return (M>>bit & 1) ? fma(a,b,c) : a*b + c; }
static double mainp(double x, int form){
  uint64_t ix = b(x), tmp = ix - 0x3fe6000000000000ULL; int i = (tmp >> 46) % 64; int k = (int64_t)tmp >> 52;
  uint64_t iz = ix - (tmp & 0xfffULL << 52); double invc = T[i].invc, logc = T[i].logc, z = d(iz), kd = k;
  double r, t1, t2;
  if (form == 0) { r = fma(z, invc, -1.0); t1 = r*IH; t2 = F(r, IL, fma(r, IH, -t1), 0); }
  else { r = (z - T2[i].chi - T2[i].clo) * invc; double rhi = d(b(r) & -1ULL << 32), rlo = r - rhi; t1 = rhi*IH;
         t2 = (M & 1) ? fma(rlo, IH, r*IL) : ((M>>7 & 1) ? fma(r, IL, rlo*IH) : rlo*IH + r*IL); }
  double t3 = kd + logc, hi = t3 + t1, lo = t3 - hi + t1 + t2, r2 = r*r, r4 = r2*r2;
  double p = F(r4, F(r, A[5], A[4], 3), F(r2, F(r, A[3], A[2], 2), F(r, A[1], A[0], 1), 4), 5);
  return F(r2, p, lo, 6) + hi;
}
int main(int argc, char**argv){
  long n = atol(argv[1]); static double xs[200000]; static uint64_t g[200000]; long m=0;
  for(long i=0;i<n && m<200000;i++){ uint64_t r=sm(i); double x = (i&1) ? (r>>11)*0x1p-53*1000.0 : d((r&0x000fffffffffffffULL)|((uint64_t)(1+(r>>52)%2046)<<52));
    if (fabs(x-1.0) < 0.05 || x <= 0x1p-1022) continue; xs[m]=x; g[m]=b(log2(x)); m++; }
  for (int form = 0; form < 2; form++) { long best = 1L<<60; unsigned bm = 0;
    for(M=0; M<256; M++){ long c=0; for(long k=0;k<m;k++) if(b(mainp(xs[k], form))!=g[k]) c++; if (c<best){best=c;bm=M;} }
    printf("form %d: best mask %02x: %ld differ of %ld\n", form, bm, best, m); }
}
