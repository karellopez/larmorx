/* SPDX-License-Identifier: Apache-2.0 */
/* Variants of Arm's near-1 path (explicit fma only, -ffp-contract=off) against glibc's log2. */
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
static double v(int var, double x){
  double r = x - 1.0, hi = r*IH, lo = fma(r, IL, fma(r, IH, -hi));
  double r2 = r*r, r4 = r2*r2, q = fma(r, B[1], B[0]), y, p;
  if (var == 0) { y = fma(r2, q, hi); lo = lo + fma(r2, q, hi - y); }      /* my build */
  else if (var == 1) { p = r2*q; y = hi + p; lo = lo + ((hi - y) + p); }  /* p rounded */
  else if (var == 2) { p = r2*q; y = hi + p; lo = lo + fma(r2, q, hi - y); }
  else { p = r2*q; y = fma(r2, q, hi); lo = lo + ((hi - y) + p); }
  double X = fma(r2, fma(r, B[5], B[4]), fma(r, B[3], B[2]));
  double Y = fma(r2, fma(r, B[9], B[8]), fma(r, B[7], B[6]));
  lo = fma(r4, fma(r4, Y, X), lo);
  return y + lo;
}
int main(int argc, char**argv){
  long n = atol(argv[1]); long bad[4]={0}, tot=0;
  for(long i=0;i<n;i++){ uint64_t r=sm(i);
    double x = 1.0+((double)(int64_t)(r>>11)-0x1p52)*0x1p-57; if (x==1.0) continue; tot++;
    double g = log2(x); for(int k=0;k<4;k++) if(b(v(k,x))!=b(g)) bad[k]++; }
  printf("near-1 %ld: variants differ %ld %ld %ld %ld\n", tot, bad[0], bad[1], bad[2], bad[3]);
}
