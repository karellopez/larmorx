/* SPDX-License-Identifier: GPL-3.0-or-later
   Copyright 2026 Karel Lopez Vilaret. Links AFNI 25.2.09 (libmri.a), whose csfft.c is
   Copyright (C) 1994-2000 Medical College of Wisconsin, GPL version 2 or any later version. */
/* Oracle harness: runs AFNI 25.2.09's compiled csfft_cox (libmri.a) on every length that
   csfft_cox computes itself (2..32768), both modes, several input vectors, and writes
   records  [int32 mode][int32 n][int32 vec][2n float input][2n float output]  to argv[1].
   argv[2] receives csfft_nextup / csfft_nextup_one35 / csfft_nextup_even (exported) for
   n = 1..40000 as int32 triplets. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <math.h>

typedef struct { float r, i; } cplx; /* same layout as AFNI's complex */
void csfft_cox(int mode, int idim, cplx *xc);
int csfft_nextup(int idim);
int csfft_nextup_one35(int idim);
int csfft_nextup_even(int idim);

static uint64_t rs;
static uint64_t next(void) { /* splitmix64 */
  uint64_t z = (rs += 0x9E3779B97F4A7C15ull);
  z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9ull;
  z = (z ^ (z >> 27)) * 0x94D049BB133111EBull;
  return z ^ (z >> 31);
}
static float unif(double lo, double hi) { return (float)(lo + (hi - lo) * ((next() >> 11) * (1.0 / 9007199254740992.0))); }
static float fbits(uint32_t b) { float f; memcpy(&f, &b, 4); return f; }

static float special(void) {
  uint64_t u = next();
  float s = (u & 1) ? -1.0f : 1.0f;
  switch ((u >> 1) % 8) {
    case 0: return 0.0f;
    case 1: return -0.0f;
    case 2: return fbits((uint32_t)((u >> 8) & 0x007FFFFF) | ((u & 1) ? 0x80000000u : 0)); /* subnormal */
    case 3: return s * ldexpf(unif(1.0, 2.0), -126 + (int)((u >> 40) % 20));               /* tiny normal */
    case 4: return s * unif(1e30, 1e33);                                                    /* huge */
    case 5: return (float)((int)((u >> 20) % 17) - 8);                                      /* small int */
    case 6: return s * unif(1e-20, 1e-19);
    default: return unif(-1000.0, 1000.0);
  }
}

#define NVEC 10
static void fill(int vec, int n, cplx *x) {
  int j;
  int ntt = n - n / 8 - 4; if (ntt < 1) ntt = 1;
  switch (vec) {
    case 0: case 1: case 2: case 3: case 4:
      for (j = 0; j < n; j++) { x[j].r = unif(-1000, 1000); x[j].i = unif(-1000, 1000); } break;
    case 5:
      for (j = 0; j < n; j++) { x[j].r = special(); x[j].i = special(); } break;
    case 6: /* 3dTshift-like: two real series packed as (f,g), zero-filled tail */
      for (j = 0; j < n; j++) {
        if (j < ntt) { x[j].r = unif(0, 3000); x[j].i = unif(0, 3000); }
        else { x[j].r = 0.0f; x[j].i = 0.0f; }
      } break;
    case 7: /* one real integer series (g == NULL), zero-filled tail */
      for (j = 0; j < n; j++) {
        x[j].r = (j < ntt) ? (float)((int)(next() % 2001) - 1000) : 0.0f; x[j].i = 0.0f;
      } break;
    case 8: /* signed zeros only */
      for (j = 0; j < n; j++) { uint64_t u = next(); x[j].r = (u & 1) ? -0.0f : 0.0f; x[j].i = (u & 2) ? -0.0f : 0.0f; } break;
    case 9: /* impulse among signed zeros */
      for (j = 0; j < n; j++) { uint64_t u = next(); x[j].r = (u & 1) ? -0.0f : 0.0f; x[j].i = (u & 2) ? -0.0f : 0.0f; }
      j = (int)(next() % (uint64_t)n); x[j].r = (next() & 1) ? 1.0f : -1.0f; break;
  }
}

static int nextup_even_internal(int idim) { /* csfft_nextup_even with internal_check = 1 */
  int jj = idim;
  do { jj = csfft_nextup(jj); if (jj % 2 == 1) jj++; else return jj; } while (1);
}

int main(int argc, char **argv) {
  FILE *fo = fopen(argv[1], "wb"), *fn = fopen(argv[2], "wb");
  static int lens[40000]; int nl = 0, n, v, li, nrec = 0; long long nval = 0;
  cplx *in = malloc(sizeof(cplx) * 32768), *out = malloc(sizeof(cplx) * 32768);
  cplx **keep, **keep_in;
  int modes_all[2] = {-1, 1}, modes_extra[2] = {0, 2};

  for (n = 1; n <= 40000; n++) {
    int t[3] = {csfft_nextup(n), csfft_nextup_one35(n), csfft_nextup_even(n)};
    fwrite(t, 4, 3, fn);
  }
  fclose(fn);

  for (n = 2; n <= 32768; n++) if (nextup_even_internal(n) == n) lens[nl++] = n;
  fprintf(stderr, "Cox-routed lengths in 2..32768: %d\n", nl);

  rs = 20261009;
  keep = calloc(nl, sizeof(cplx *)); keep_in = calloc(nl, sizeof(cplx *));
  for (li = 0; li < nl; li++) {
    n = lens[li];
    for (v = 0; v < NVEC; v++) {
      int nm = (v == 0 || v == 7) ? 4 : 2, mi;
      fill(v, n, in);
      for (mi = 0; mi < nm; mi++) {
        int mode = mi < 2 ? modes_all[mi] : modes_extra[mi - 2];
        int hdr[3] = {mode, n, v};
        memcpy(out, in, sizeof(cplx) * n);
        csfft_cox(mode, n, out);
        fwrite(hdr, 4, 3, fo); fwrite(in, sizeof(cplx), n, fo); fwrite(out, sizeof(cplx), n, fo);
        nrec++; nval += 2LL * n;
        if (v == 0 && mode == -1) { keep[li] = malloc(sizeof(cplx) * n); memcpy(keep[li], out, sizeof(cplx) * n);
                                      keep_in[li] = malloc(sizeof(cplx) * n); memcpy(keep_in[li], in, sizeof(cplx) * n); }
      }
    }
  }
  fclose(fo);
  fprintf(stderr, "records: %d, output values: %lld\n", nrec, nval);

  /* state independence: redo vector 0 / mode -1 in a shuffled length order */
  {
    int *perm = malloc(sizeof(int) * nl), i, diff = 0;
    for (i = 0; i < nl; i++) perm[i] = i;
    for (i = nl - 1; i > 0; i--) { int j = (int)(next() % (uint64_t)(i + 1)), t = perm[i]; perm[i] = perm[j]; perm[j] = t; }
    for (i = 0; i < nl; i++) {
      int k;
      li = perm[i]; n = lens[li];
      memcpy(out, keep_in[li], sizeof(cplx) * n);
      csfft_cox(-1, n, out);
      for (k = 0; k < n; k++) if (memcmp(&out[k], &keep[li][k], sizeof(cplx))) diff++;
    }
    fprintf(stderr, "shuffled-order rerun: %d differing complex values\n", diff);
  }
  return 0;
}
