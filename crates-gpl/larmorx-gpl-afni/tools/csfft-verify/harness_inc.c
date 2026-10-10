/* SPDX-License-Identifier: GPL-3.0-or-later
   Copyright 2026 Karel Lopez Vilaret. Includes AFNI 25.2.09 csfft.c and a verbatim copy of its
   general power-of-two loop: Copyright (C) 1994-2000 Medical College of Wisconsin (Robert W.
   Cox, Andrzej Jesmanowicz), GPL version 2 or any later version. */
/* Second oracle harness: compiles AFNI 25.2.09's csfft.c itself (with AFNI's flags) to reach
   its static parts.
   argv[1]: csfft_nextup_even(n) with internal_check = 1 (as csfft_cox calls it), n = 1..40000.
   argv[2]: records [mode][n][vec][in][out] of csfft_cox's general power-of-two loop (copied
            verbatim below; unreachable through csfft_cox), n = 2..65536.
   argv[3]: (arg, cos, sin) doubles for every argument the twiddle tables of the 139
            supported lengths pass to sincos. */
#include "csfft.c"
#include <stdint.h>

/* ---- verbatim copy of csfft_cox's general power-of-2 path (csfft.c lines 309-351) ---- */
static void generic_cox( int mode , int idim , complex *xc )
{
   register unsigned int  m, n, i0, i1, i2, i3, k;
   register complex       *r0, *r1, *csp;
   register float         co, si, f0, f1, f2, f3, f4;

   if( nold != idim ) csfft_trigconsts( idim ) ;

   n   = idim;
   i2  = idim >> 1;
   i1  = 0;
   csp = (mode > 0) ? csplus : csminus ;  /* choose const array */

   for (i0=0; i0 < n; i0 ++) {
      if ( i1 > i0 ) {
         r0    = xc + i0; r1    = xc + i1;
         f1    = r0->r;   f2    = r0->i;
         r0->r = r1->r;   r0->i = r1->i;
         r1->r = f1;      r1->i = f2;
      }
      m = i2;
      while ( m && !(i1 < m) ) { i1 -= m; m >>= 1; }
     i1 += m;
   }

   m = 1; k = 0;
   while (n > m) {
      i3 = m << 1;
      for (i0=0; i0 < m; i0 ++) {
         co = (csp + k)->r; si = (csp + k)->i;
         for (i1=i0; i1 < n; i1 += i3) {
            r0    = xc + i1;    r1    = r0 + m;
            f1    = r1->r * co; f2    = r1->i * si;
            f3    = r1->r * si; f4    = r1->i * co;
            f1   -= f2;         f3   += f4;
            f2    = r0->r;      f4    = r0->i;
            r1->r = f2 - f1;    r1->i = f4 - f3;
            r0->r = f2 + f1;    r0->i = f4 + f3;
         }
         k++;
      }
      m = i3;
   }
}

static uint64_t rs = 77;
static uint64_t nxt(void) {
  uint64_t z = (rs += 0x9E3779B97F4A7C15ull);
  z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9ull;
  z = (z ^ (z >> 27)) * 0x94D049BB133111EBull;
  return z ^ (z >> 31);
}
static float unif(double lo, double hi) { return (float)(lo + (hi - lo) * ((nxt() >> 11) * (1.0 / 9007199254740992.0))); }

/* ---- the sincos arguments of every table ---- */
static double ths[4096]; static int lens[4096]; static int nth = 0;
static void add_table(double th, int len) {
  int i;
  for (i = 0; i < nth; i++) if (ths[i] == th) { if (len > lens[i]) lens[i] = len; return; }
  ths[nth] = th; lens[nth] = len; nth++;
}
static void add_trig(int n) { (void)n; } /* covered by the csfft_trigconsts loop in main */
static void walk(int n);
static void walk_pow2(int n) {
  if (n <= 4) return;
  if (n <= 32) { add_trig(n); return; }
  if (n == 64) { add_table(PI/32.0, 32); walk_pow2(32); return; }
  add_table(2.0*PI/n, 3*(n/4)); walk_pow2(n/4);
}
static void walk(int n) {
  if ((n & (n - 1)) == 0) { walk_pow2(n); return; }
  if (n % 3 == 0) { add_table(2.0*PI/n, 2*(n/3)); if (n/3 > 1) walk(n/3); return; }
  if (n % 5 == 0) { add_table(2.0*PI/n, 4*(n/5)); if (n/5 > 1) walk(n/5); return; }
  fprintf(stderr, "unexpected length %d\n", n); exit(1);
}

int main(int argc, char **argv) {
  FILE *f1 = fopen(argv[1], "wb"), *f2 = fopen(argv[2], "wb"), *f3 = fopen(argv[3], "wb");
  int n, v, mi, k; long long nargs = 0;
  complex *in = malloc(sizeof(complex) * 65536), *out = malloc(sizeof(complex) * 65536);

  internal_check = 1;
  for (n = 1; n <= 40000; n++) { int t = csfft_nextup_even(n); fwrite(&t, 4, 1, f1); }
  internal_check = 0;
  fclose(f1);

  for (n = 2; n <= 65536; n *= 2) {
    for (v = 0; v < 5; v++) {
      for (k = 0; k < n; k++) {
        if (v < 3) { in[k].r = unif(-1000, 1000); in[k].i = unif(-1000, 1000); }
        else if (v == 3) { in[k].r = (k < n - n/8) ? (float)((int)(nxt() % 2001) - 1000) : 0.0f; in[k].i = 0.0f; }
        else { uint64_t u = nxt(); in[k].r = (u & 1) ? -0.0f : 0.0f; in[k].i = (u & 2) ? -0.0f : 0.0f; }
      }
      if (v == 4) in[nxt() % (uint64_t)n].r = 1.0f;
      for (mi = 0; mi < 2; mi++) {
        int hdr[3] = {mi ? 1 : -1, n, v};
        memcpy(out, in, sizeof(complex) * n);
        generic_cox(hdr[0], n, out);
        fwrite(hdr, 4, 3, f2); fwrite(in, sizeof(complex), n, f2); fwrite(out, sizeof(complex), n, f2);
      }
    }
  }
  fclose(f2);

  internal_check = 1;
  for (n = 2; n <= 32768; n++) if (csfft_nextup_even(n) == n) walk(n);
  internal_check = 0;
  for (k = 0; k < nth; k++) {
    int j;
    for (j = 1; j < lens[k]; j++) {
      double a = j * ths[k], s, c, rec[3];
      sincos(a, &s, &c); rec[0] = a; rec[1] = c; rec[2] = s;
      fwrite(rec, 8, 3, f3); nargs++;
    }
  }
  /* csfft_trigconsts(n), n <= 65536: al = f1*PI/f2 with float f1 = +-1, float f2 = m < n */
  {
    unsigned int m;
    float fa, fb;
    for (m = 1; m < 65536u; m <<= 1) {
      for (fa = 1.0f; fa >= -1.0f; fa -= 2.0f) {
        double a, s, c, rec[3];
        fb = m; a = fa*PI/fb;
        sincos(a, &s, &c); rec[0] = a; rec[1] = c; rec[2] = s;
        fwrite(rec, 8, 3, f3); nargs++;
      }
    }
  }
  fclose(f3);
  fprintf(stderr, "tables: %d distinct th; sincos arguments written: %lld\n", nth, nargs);
  return 0;
}
