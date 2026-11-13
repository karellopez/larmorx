// SPDX-License-Identifier: Apache-2.0
// glibc's qsort and qsort_r as a black box (specs/glibc-qsort.md, the (observed) marks): calls of
// the C library's functions by name, a generator, a hash; no glibc code.
//
// Build: cc -O2 -o glibc_qsort_probe glibc_qsort_probe.c   (on the reference machine: glibc 2.35)
//
// Reads cases from standard input, one per line:
//     <s> <n> <mode> <seed> <keyrange> <api> <fail>
//   s         element size in bytes (1, 2, or 4 and more; 4 or 8 for the float modes)
//   n         number of elements
//   mode      the comparator: 3way, tie+, tie-, hash (s >= 8), f3, f2 (s = 4: floats), d3, d2 (s = 8)
//   seed      the elements' and the hash comparator's seed (splitmix64, the spec's section 9)
//   keyrange  keys are draws mod keyrange (small: many ties); for f*/d*: the size of the value set used
//   api       qsort or qsort_r (qsort_r passes a context pointer the comparator checks)
//   fail      1: the probe's malloc returns NULL for the one request of the temporary array's size
//             (the spec's section 4: forces the C library's fallback), 0: no interference
// and prints one line per case:
//     <comparisons> <hash of the comparison sequence> <hash of the sorted bytes> <malloc> <errno>
//   comparisons  how many times the comparator was called
//   sequence     (hexadecimal) FNV-1a 64 over, per call, the first min(s, 8) bytes of the first argument, then of
//                the second
//   sorted       (hexadecimal) FNV-1a 64 over the n * s bytes of the array after the sort
//   malloc       how many times malloc was asked for exactly the temporary array's size during the sort
//   errno        1 if errno kept the value set before the call (12345), else 0
// or, when the C library passed the comparator a pointer outside the array (the spec's section 5:
// possible only in the fallback, with an inconsistent comparator), the line
//     <comparisons> <hash of the comparison sequence> outside
// counting and hashing the calls before that one: the comparator then leaves qsort at once
// (longjmp), before the C library could write outside the array.
// Element i (splitmix64 draw r_i): key k_i = r_i mod keyrange; bytes 0..s-1 = 0; then for s = 1 the
// byte k_i, s = 2 the 16-bit k_i, s >= 4 the 32-bit k_i (little-endian) in bytes 0..3, and for s >= 8
// the 32-bit index i in bytes 4..7 and, from byte 8, the bytes of further draws (8 per draw, in order).
// The float modes take value number k_i of the table FVALS (f*) or DVALS (d*) instead.
#define _GNU_SOURCE
#include <errno.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern void *__libc_malloc(size_t);

static size_t fail_size, watch_size;
static int fail_on, malloc_seen;

// The executable's malloc is the one the C library's own calls reach (glibc supports replacing
// malloc); it forwards to the C library's allocator except for the one request it is told to refuse.
void *malloc(size_t n) {
  if (watch_size && n == watch_size) {
    malloc_seen++;
    if (fail_on && n == fail_size) return NULL;
  }
  return __libc_malloc(n);
}

static uint64_t sm_state;
static uint64_t splitmix(void) {
  uint64_t z = (sm_state += 0x9E3779B97F4A7C15ull);
  z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9ull;
  z = (z ^ (z >> 27)) * 0x94D049BB133111EBull;
  return z ^ (z >> 31);
}

#define FNV0 14695981039346656037ull
#define FNVP 1099511628211ull
static uint64_t fnv_bytes(uint64_t h, const unsigned char *p, size_t n) {
  for (size_t i = 0; i < n; i++) h = (h ^ p[i]) * FNVP;
  return h;
}

static const uint32_t FVALS[] = {0x7fc00000u, 0x80000000u, 0x00000000u, 0x3f800000u, 0xbf800000u,
                                 0x7f800000u, 0xff800000u, 0x7fc00001u, 0xffc00000u, 0x40000000u,
                                 0x00000001u, 0x3f800000u};
static const uint64_t DVALS[] = {0x7ff8000000000000ull, 0x8000000000000000ull, 0x0000000000000000ull,
                                 0x3ff0000000000000ull, 0xbff0000000000000ull, 0x7ff0000000000000ull,
                                 0xfff0000000000000ull, 0x7ff8000000000001ull, 0xfff8000000000000ull,
                                 0x4000000000000000ull, 0x0000000000000001ull, 0x3ff0000000000000ull};

static size_t S;
static int MODE;
static uint64_t HSEED, NCMP, HCMP;
static int CTX_OK;
static int CTX_TOKEN;
static const unsigned char *ARR_LO, *ARR_HI;
static jmp_buf OUT_ENV;

enum { M3WAY, MTIEP, MTIEM, MHASH, MF3, MF2, MD3, MD2 };

static uint32_t key_of(const unsigned char *p) {
  if (S == 1) return p[0];
  if (S == 2) return (uint32_t)p[0] | ((uint32_t)p[1] << 8);
  uint32_t k;
  memcpy(&k, p, 4);
  return k;
}

static int compare(const void *a, const void *b) {
  const unsigned char *pa = a, *pb = b;
  size_t m = S < 8 ? S : 8;
  if (pa < ARR_LO || pa >= ARR_HI || pb < ARR_LO || pb >= ARR_HI) longjmp(OUT_ENV, 1);
  NCMP++;
  HCMP = fnv_bytes(fnv_bytes(HCMP, pa, m), pb, m);
  switch (MODE) {
    case M3WAY: {
      uint32_t x = key_of(pa), y = key_of(pb);
      return x < y ? -1 : x > y ? 1 : 0;
    }
    case MTIEP: {
      uint32_t x = key_of(pa), y = key_of(pb);
      return x < y ? -1 : 1;
    }
    case MTIEM: {
      uint32_t x = key_of(pa), y = key_of(pb);
      return x > y ? 1 : -1;
    }
    case MHASH: {
      uint32_t ta, tb;
      memcpy(&ta, pa + 4, 4);
      memcpy(&tb, pb + 4, 4);
      uint64_t z = HSEED ^ ((uint64_t)ta * 0x9E3779B97F4A7C15ull) ^ ((uint64_t)tb * 0xC2B2AE3D27D4EB4Full);
      z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9ull;
      z = (z ^ (z >> 27)) * 0x94D049BB133111EBull;
      z ^= z >> 31;
      return (int)(z % 3) - 1;
    }
    case MF3: case MF2: {
      float x, y;
      memcpy(&x, pa, 4);
      memcpy(&y, pb, 4);
      if (MODE == MF3) return x < y ? -1 : x > y ? 1 : 0;
      return x < y ? -1 : x == y ? 0 : 1;
    }
    default: {
      double x, y;
      memcpy(&x, pa, 8);
      memcpy(&y, pb, 8);
      if (MODE == MD3) return x < y ? -1 : x > y ? 1 : 0;
      return x < y ? -1 : x == y ? 0 : 1;
    }
  }
}

static int compare_r(const void *a, const void *b, void *ctx) {
  if (ctx != (void *)&CTX_TOKEN) CTX_OK = 0;
  return compare(a, b);
}

static int mode_of(const char *m) {
  const char *names[] = {"3way", "tie+", "tie-", "hash", "f3", "f2", "d3", "d2"};
  for (int i = 0; i < 8; i++)
    if (!strcmp(m, names[i])) return i;
  return -1;
}

int main(void) {
  char line[256], mode[16], api[16];
  unsigned long long s, n, seed, keyrange;
  int fail;
  while (fgets(line, sizeof line, stdin)) {
    if (sscanf(line, "%llu %llu %15s %llu %llu %15s %d", &s, &n, mode, &seed, &keyrange, api, &fail) != 7)
      continue;
    MODE = mode_of(mode);
    S = s;
    if (MODE < 0 || s == 0 || keyrange == 0 || (MODE == MHASH && s < 8) ||
        ((MODE == MF3 || MODE == MF2) && s != 4) || ((MODE == MD3 || MODE == MD2) && s != 8)) {
      printf("bad case\n");
      continue;
    }
    unsigned char *arr = __libc_malloc(n * s + 1);
    memset(arr, 0, n * s + 1);
    sm_state = seed;
    for (size_t i = 0; i < n; i++) {
      unsigned char *p = arr + i * s;
      uint64_t k = splitmix() % keyrange;
      if (MODE == MF3 || MODE == MF2) {
        memcpy(p, &FVALS[k % 12], 4);
        continue;
      }
      if (MODE == MD3 || MODE == MD2) {
        memcpy(p, &DVALS[k % 12], 8);
        continue;
      }
      if (s == 1) p[0] = (unsigned char)k;
      else if (s == 2) { p[0] = (unsigned char)k; p[1] = (unsigned char)(k >> 8); }
      else {
        uint32_t k32 = (uint32_t)k, i32 = (uint32_t)i;
        memcpy(p, &k32, 4);
        if (s >= 8) {
          memcpy(p + 4, &i32, 4);
          for (size_t off = 8; off < s; off += 8) {
            uint64_t r = splitmix();
            size_t m = s - off < 8 ? s - off : 8;
            memcpy(p + off, &r, m);
          }
        }
      }
    }
    HSEED = seed ^ 0x5DEECE66Dull;
    NCMP = 0;
    HCMP = FNV0;
    CTX_OK = 1;
    watch_size = s > 32 ? 2 * n * sizeof(void *) + s : n * s;
    fail_size = watch_size;
    fail_on = fail;
    malloc_seen = 0;
    ARR_LO = arr;
    ARR_HI = arr + n * s;
    if (setjmp(OUT_ENV)) {
      watch_size = 0;
      fail_on = 0;
      printf("%llu %016llx outside\n", (unsigned long long)NCMP, (unsigned long long)HCMP);
      free(arr);
      continue;
    }
    errno = 12345;
    if (!strcmp(api, "qsort_r")) qsort_r(arr, n, s, compare_r, &CTX_TOKEN);
    else qsort(arr, n, s, compare);
    int keep = errno == 12345;
    watch_size = 0;
    fail_on = 0;
    printf("%llu %016llx %016llx %d %d%s\n", (unsigned long long)NCMP, (unsigned long long)HCMP,
           (unsigned long long)fnv_bytes(FNV0, arr, n * s), malloc_seen, keep,
           CTX_OK ? "" : " context-lost");
    free(arr);
  }
  return 0;
}
