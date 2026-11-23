/* SPDX-License-Identifier: Apache-2.0 */
/* Arm's log2 (this folder's build) against the C library's log2 (a black box): bit for bit. */
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <math.h>
#include <stdlib.h>
double arm_log2(double);
static uint64_t sm(uint64_t i){uint64_t z=i+0x9e3779b97f4a7c15ULL;z=(z^(z>>30))*0xbf58476d1ce4e5b9ULL;z=(z^(z>>27))*0x94d049bb133111ebULL;return z^(z>>31);}
static uint64_t b(double x){uint64_t u;memcpy(&u,&x,8);return u;}
static double d(uint64_t u){double x;memcpy(&x,&u,8);return x;}
int main(int argc,char**argv){
  long n = argc>1 ? atol(argv[1]) : (1L<<26); long bad=0, tot=0;
  for(long i=0;i<n;i++){
    uint64_t r=sm(i); double xs[6];
    xs[0]=d(r);                                   /* random bits */
    xs[1]=(r>>11)*0x1p-53*1000.0;                 /* uniform (0,1000) */
    xs[2]=1.0+((double)(int64_t)(r>>11)-0x1p52)*0x1p-57; /* near 1 */
    xs[3]=d(r&0x000fffffffffffffULL);             /* subnormal */
    xs[4]=d((r&0x800fffffffffffffULL)|((uint64_t)(1+(r>>52)%2046)<<52)); /* normal, both signs */
    xs[5]=(r>>11)*0x1p-53;                        /* (0,1): probabilities */
    for(int k=0;k<6;k++){tot++; if(b(arm_log2(xs[k]))!=b(log2(xs[k]))){ if(bad<5) printf("DIFF %a arm %a glibc %a\n",xs[k],arm_log2(xs[k]),log2(xs[k])); bad++;}}
  }
  double sp[]={0.0,-0.0,1.0,-1.0,INFINITY,-INFINITY,NAN,-NAN,d(0x7ff0000000000001ULL),d(0xfff4000000000000ULL),d(1),0x1p-1022,0x1.fffffffffffffp1023,2.0,0.5};
  for(unsigned k=0;k<sizeof sp/sizeof sp[0];k++){tot++; if(b(arm_log2(sp[k]))!=b(log2(sp[k]))){printf("SPECIAL DIFF %a: %016llx %016llx\n",sp[k],(unsigned long long)b(arm_log2(sp[k])),(unsigned long long)b(log2(sp[k])));bad++;}}
  printf("compared %ld differ %ld\n",tot,bad); return bad!=0;
}
