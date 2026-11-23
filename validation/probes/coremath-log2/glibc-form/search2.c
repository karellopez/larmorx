/* SPDX-License-Identifier: Apache-2.0 */
/* Which multiply-adds of Arm's log2 main path are fused in glibc's build (every combination of
   the 7 sites against glibc's log2, a black box); and the near-1 path with no implicit fusion
   on more arguments. */
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <stdlib.h>
#include <math.h>
#include "math_config.h"
#define T __log2_data.tab
#define A __log2_data.poly
#define IH __log2_data.invln2hi
#define IL __log2_data.invln2lo
#define B __log2_data.poly1
static uint64_t sm(uint64_t i){uint64_t z=i+0x9e3779b97f4a7c15ULL;z=(z^(z>>30))*0xbf58476d1ce4e5b9ULL;z=(z^(z>>27))*0x94d049bb133111ebULL;return z^(z>>31);}
static uint64_t b(double x){uint64_t u;memcpy(&u,&x,8);return u;}
static double d(uint64_t u){double x;memcpy(&x,&u,8);return x;}
static unsigned M;
static double F(double a, double b, double c, int bit){ return (M>>bit & 1) ? fma(a,b,c) : a*b + c; }
static double mainp(double x){
  uint64_t ix = b(x), tmp = ix - 0x3fe6000000000000ULL; int i = (tmp >> 46) % 64; int k = (int64_t)tmp >> 52;
  uint64_t iz = ix - (tmp & 0xfffULL << 52); double invc = T[i].invc, logc = T[i].logc, z = d(iz), kd = k;
  double r = fma(z, invc, -1.0), t1 = r*IH, t2 = F(r, IL, fma(r, IH, -t1), 0);
  double t3 = kd + logc, hi = t3 + t1, lo = t3 - hi + t1 + t2, r2 = r*r, r4 = r2*r2;
  double p = F(r4, F(r, A[5], A[4], 3), F(r2, F(r, A[3], A[2], 2), F(r, A[1], A[0], 1), 4), 5);
  double y = F(r2, p, lo, 6) + hi;
  return y;
}
static double near1(double x){
  double r = x - 1.0, hi = r*IH, lo = r*IL + fma(r, IH, -hi), r2 = r*r, r4 = r2*r2, p = r2*(B[0] + r*B[1]);
  double y = hi + p; lo += hi - y + p;
  lo += r4 * (B[2] + r * B[3] + r2 * (B[4] + r * B[5]) + r4 * (B[6] + r * B[7] + r2 * (B[8] + r * B[9])));
  return y + lo;
}
int main(int argc, char**argv){
  long n = atol(argv[1]); static double xs[400000]; static uint64_t g[400000]; long m=0;
  for(long i=0;i<n && m<400000;i++){ uint64_t r=sm(i); double x = (i&1) ? (r>>11)*0x1p-53*1000.0 : d((r&0x000fffffffffffffULL)|((uint64_t)(1+(r>>52)%2046)<<52));
    if (fabs(x-1.0) < 0.05 || x <= 0x1p-1022) continue; xs[m]=x; g[m]=b(log2(x)); m++; }
  for(M=0; M<128; M++){ long c=0; for(long k=0;k<m;k++) if(b(mainp(xs[k]))!=g[k]) c++; if (c==0) printf("main mask %02x: 0 differ of %ld\n", M, m); }
  long c=0, t=0; for(long i=0;i<n;i++){ uint64_t r=sm(i+1000000007L); double x = 1.0+((double)(int64_t)(r>>11)-0x1p52)*0x1p-57; if (x==1.0) continue; t++; if (b(near1(x))!=b(log2(x))) c++; }
  printf("near-1 (no implicit fusion) %ld: %ld differ\n", t, c);
}
