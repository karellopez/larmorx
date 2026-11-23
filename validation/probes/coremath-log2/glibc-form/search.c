/* SPDX-License-Identifier: Apache-2.0 */
/* Which multiply-adds of Arm's log2 near-1 path are fused in glibc's build: every combination
   of the 12 sites against glibc's log2 (a black box) on seeded arguments near 1. */
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <stdlib.h>
#include <math.h>
#include "math_config.h"
#define B __log2_data.poly1
#define IH __log2_data.invln2hi
#define IL __log2_data.invln2lo
static uint64_t sm(uint64_t i){uint64_t z=i+0x9e3779b97f4a7c15ULL;z=(z^(z>>30))*0xbf58476d1ce4e5b9ULL;z=(z^(z>>27))*0x94d049bb133111ebULL;return z^(z>>31);}
static uint64_t b(double x){uint64_t u;memcpy(&u,&x,8);return u;}
static unsigned M;
static double F(double a, double b, double c, int bit){ return (M>>bit & 1) ? fma(a,b,c) : a*b + c; }
static double v(double x){
  double r = x - 1.0, hi = r*IH;
  double lo = F(r, IL, fma(r, IH, -hi), 0);
  double r2 = r*r, r4 = r2*r2, q = F(r, B[1], B[0], 1);
  double y = F(r2, q, hi, 2);
  double t = hi - y;
  lo = lo + F(r2, q, t, 3);
  double X = F(r2, F(r, B[5], B[4], 4), F(r, B[3], B[2], 5), 6);
  double Y = F(r2, F(r, B[9], B[8], 7), F(r, B[7], B[6], 8), 9);
  lo = F(r4, F(r4, Y, X, 10), lo, 11);
  return y + lo;
}
int main(int argc, char**argv){
  long n = atol(argv[1]); static double xs[400000]; static uint64_t g[400000]; long m=0;
  for(long i=0;i<n && m<400000;i++){ uint64_t r=sm(i);
    double x = 1.0+((double)(int64_t)(r>>11)-0x1p52)*0x1p-57; if (x==1.0) continue; xs[m]=x; g[m]=b(log2(x)); m++; }
  long best=-1, bestc=1L<<62;
  for(M=0; M<4096; M++){ long c=0; for(long k=0;k<m;k++) if(b(v(xs[k]))!=g[k]) { c++; if (c > bestc) break; }
    if(c<bestc){bestc=c;best=M;} if (c==0) printf("mask %03x: 0 differ\n", M); }
  printf("args %ld best mask %03lx differ %ld\n", m, best, bestc);
}
