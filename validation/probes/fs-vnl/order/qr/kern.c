// SPDX-License-Identifier: Apache-2.0
// The order in which netlib's BLAS kernels, as the FreeSurfer unit oracle's ITK 4.13.2 built them
// (libitkv3p_netlib through libfsvnl.so), pass NaN operands on: the exploration that gave
// larmorx_core::vnl_qr's tables (the probe's family `qrnan` checks them). Build and run:
//   gcc -O1 -DNMAX=64 -o kern kern.c <out>/libfsvnl.so -lm -Wl,-rpath,<out>
//   ./kern f > f.txt; ./kern d > d.txt   (float, double)
//   python3 -P infer_dot.py f.txt; python3 -P infer_axpy_scal.py f.txt
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <stdlib.h>
typedef long integer;
extern double v3p_netlib_sdot_(integer*, float*, integer*, float*, integer*);
extern double v3p_netlib_ddot_(integer*, double*, integer*, double*, integer*);
extern int v3p_netlib_saxpy_(integer*, float*, float*, integer*, float*, integer*);
extern int v3p_netlib_daxpy_(integer*, double*, double*, integer*, double*, integer*);
extern int v3p_netlib_sscal_(integer*, float*, float*, integer*);
extern int v3p_netlib_dscal_(integer*, double*, double*, integer*);
static float fnan(unsigned tag){ uint32_t b=0x7fc00000u|tag; float f; memcpy(&f,&b,4); return f;}
static unsigned ftag(float f){ uint32_t b; memcpy(&b,&f,4); return (b&0x7f800000u)==0x7f800000u && (b&0x7fffffu)? (b&0x3fffffu)|((b>>31)<<31) : 0xffffffffu;}
static double dnan(unsigned tag){ uint64_t b=0x7ff8000000000000ull|tag; double f; memcpy(&f,&b,8); return f;}
static unsigned dtag(double f){ uint64_t b; memcpy(&b,&f,8); return (b&0x7ff0000000000000ull)==0x7ff0000000000000ull && (b&0xfffffffffffffull)? (unsigned)(b&0x3fffff)|(unsigned)((b>>63)<<31) : 0xffffffffu;}
int main(int argc,char**argv){
  int dbl = argc>1 && argv[1][0]=='d';
  integer one=1;
  float *bx=aligned_alloc(64,4096), *by=aligned_alloc(64,4096);
  double *cx=aligned_alloc(64,8192), *cy=aligned_alloc(64,8192);
  for(int off=0; off<4; off++){
  printf("== dot off %d\n", off);
  for(integer n=1;n<=NMAX;n++){
    // successive winners: leaves x_i tag i, y_i tag 100+i
    int live[1024]={0}; for(int i=1;i<=n;i++){live[i]=1; live[500+i]=1;}
    printf("n=%2ld:",n);
    for(int q=0;q<2*n;q++){
      unsigned w;
      if(!dbl){ float *x=bx+off,*y=by+off;
        for(int i=1;i<=n;i++){ x[i-1]= live[i]?fnan(i):1.0f; y[i-1]= live[500+i]?fnan(500+i):1.0f; }
        w=ftag((float)v3p_netlib_sdot_(&n,x,&one,y,&one)); }
      else { double *x=cx+off,*y=cy+off;
        for(int i=1;i<=n;i++){ x[i-1]= live[i]?dnan(i):1.0; y[i-1]= live[500+i]?dnan(500+i):1.0; }
        w=dtag(v3p_netlib_ddot_(&n,x,&one,y,&one)); }
      if(w==0xffffffffu || (w&0x7fffffff)>=1024){printf(" ?%x",w);break;}
      printf(" %s%u", (w>>31)?"-":"", w&0x7fffffff); live[w&0x7fffffff]=0;
    }
    printf("\n");
  }
  printf("== axpy off %d (winner per element for NaN subsets of {a,x,y}: a=A x=X y=Y)\n", off);
  for(integer n=1;n<=NMAX;n++){
    printf("n=%2ld:",n);
    for(int s=1;s<8;s++){
      printf(" [%c%c%c]", s&1?'a':'.', s&2?'x':'.', s&4?'y':'.');
      if(!dbl){ float *x=bx+off,*y=by+off; float a = (s&1)?fnan(200):2.0f;
        for(int i=0;i<n;i++){ x[i]=(s&2)?fnan(1+i):3.0f; y[i]=(s&4)?fnan(100+i):5.0f;}
        v3p_netlib_saxpy_(&n,&a,x,&one,y,&one);
        for(int i=0;i<n;i++){ unsigned t=ftag(y[i]); t&=0x7fffffff; putchar(t==200?'A':t<100?'X':'Y'); } }
      else { double *x=cx+off,*y=cy+off; double a=(s&1)?dnan(200):2.0;
        for(int i=0;i<n;i++){ x[i]=(s&2)?dnan(1+i):3.0; y[i]=(s&4)?dnan(100+i):5.0;}
        v3p_netlib_daxpy_(&n,&a,x,&one,y,&one);
        for(int i=0;i<n;i++){ unsigned t=dtag(y[i]); t&=0x7fffffff; putchar(t==200?'A':t<100?'X':'Y'); } }
    }
    printf("\n");
  }
  printf("== scal off %d (a and x NaN: winner per element)\n", off);
  for(integer n=1;n<=NMAX;n++){
    printf("n=%2ld: ",n);
    if(!dbl){ float *x=bx+off; float a=fnan(200); for(int i=0;i<n;i++) x[i]=fnan(1+i);
      v3p_netlib_sscal_(&n,&a,x,&one); for(int i=0;i<n;i++){unsigned t=ftag(x[i])&0x7fffffff; putchar(t==200?'A':'X');} }
    else { double *x=cx+off; double a=dnan(200); for(int i=0;i<n;i++) x[i]=dnan(1+i);
      v3p_netlib_dscal_(&n,&a,x,&one); for(int i=0;i<n;i++){unsigned t=dtag(x[i])&0x7fffffff; putchar(t==200?'A':'X');} }
    printf("\n");
  }
  }
  return 0;
}
