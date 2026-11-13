// SPDX-License-Identifier: Apache-2.0
// The C library's qsort timed alone (a black-box run: calls of qsort by name, no C library code):
// the elements and comparator kinds of glibc_qsort_probe (specs/glibc-qsort.md section 10), a plain
// comparator (no counting or hashing), the best of <repeats> sorts of a fresh copy, timed with
// CLOCK_MONOTONIC. Prints the best time and the FNV-1a 64 hash of the sorted bytes, so that the clean
// room's `cleanroom-probe qsort-bench` (the same case) can be checked to leave the same arrangement.
//
// Build: cc -O2 -o qsort_bench qsort_bench.c
// Usage: qsort_bench <s> <n> <mode> <seed> <keyrange> <repeats>
#define _GNU_SOURCE
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static uint64_t sm_state;
static uint64_t mix(uint64_t z) {
  z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9ull;
  z = (z ^ (z >> 27)) * 0x94D049BB133111EBull;
  return z ^ (z >> 31);
}
static uint64_t splitmix(void) { return mix(sm_state += 0x9E3779B97F4A7C15ull); }

static const uint32_t FVALS[] = {0x7fc00000u, 0x80000000u, 0x00000000u, 0x3f800000u, 0xbf800000u,
                                 0x7f800000u, 0xff800000u, 0x7fc00001u, 0xffc00000u, 0x40000000u,
                                 0x00000001u, 0x3f800000u};
static const uint64_t DVALS[] = {0x7ff8000000000000ull, 0x8000000000000000ull, 0x0000000000000000ull,
                                 0x3ff0000000000000ull, 0xbff0000000000000ull, 0x7ff0000000000000ull,
                                 0xfff0000000000000ull, 0x7ff8000000000001ull, 0xfff8000000000000ull,
                                 0x4000000000000000ull, 0x0000000000000001ull, 0x3ff0000000000000ull};

static size_t S;
static int MODE;
static uint64_t HSEED;
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
  switch (MODE) {
    case M3WAY: {
      uint32_t x = key_of(pa), y = key_of(pb);
      return x < y ? -1 : x > y ? 1 : 0;
    }
    case MTIEP: return key_of(pa) < key_of(pb) ? -1 : 1;
    case MTIEM: return key_of(pa) > key_of(pb) ? 1 : -1;
    case MHASH: {
      uint32_t ta, tb;
      memcpy(&ta, pa + 4, 4);
      memcpy(&tb, pb + 4, 4);
      uint64_t z = mix(HSEED ^ ((uint64_t)ta * 0x9E3779B97F4A7C15ull) ^ ((uint64_t)tb * 0xC2B2AE3D27D4EB4Full));
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

int main(int argc, char **argv) {
  if (argc != 7) {
    fprintf(stderr, "usage: qsort_bench <s> <n> <mode> <seed> <keyrange> <repeats>\n");
    return 2;
  }
  const char *names[] = {"3way", "tie+", "tie-", "hash", "f3", "f2", "d3", "d2"};
  MODE = -1;
  for (int i = 0; i < 8; i++)
    if (!strcmp(argv[3], names[i])) MODE = i;
  S = strtoull(argv[1], 0, 10);
  size_t n = strtoull(argv[2], 0, 10);
  uint64_t seed = strtoull(argv[4], 0, 10), keyrange = strtoull(argv[5], 0, 10);
  int repeats = atoi(argv[6]);
  if (MODE < 0 || S == 0 || keyrange == 0 || repeats < 1) return 2;
  unsigned char *arr = calloc(n * S + 1, 1), *work = malloc(n * S + 1);
  sm_state = seed;
  for (size_t i = 0; i < n; i++) {
    unsigned char *p = arr + i * S;
    uint64_t k = splitmix() % keyrange;
    if (MODE == MF3 || MODE == MF2) { memcpy(p, &FVALS[k % 12], 4); continue; }
    if (MODE == MD3 || MODE == MD2) { memcpy(p, &DVALS[k % 12], 8); continue; }
    if (S == 1) p[0] = (unsigned char)k;
    else if (S == 2) { p[0] = (unsigned char)k; p[1] = (unsigned char)(k >> 8); }
    else {
      uint32_t k32 = (uint32_t)k, i32 = (uint32_t)i;
      memcpy(p, &k32, 4);
      if (S >= 8) {
        memcpy(p + 4, &i32, 4);
        for (size_t off = 8; off < S; off += 8) {
          uint64_t r = splitmix();
          size_t m = S - off < 8 ? S - off : 8;
          memcpy(p + off, &r, m);
        }
      }
    }
  }
  HSEED = seed ^ 0x5DEECE66Dull;
  double best = 1e30;
  for (int r = 0; r < repeats; r++) {
    memcpy(work, arr, n * S);
    struct timespec t0, t1;
    clock_gettime(CLOCK_MONOTONIC, &t0);
    qsort(work, n, S, compare);
    clock_gettime(CLOCK_MONOTONIC, &t1);
    double t = (t1.tv_sec - t0.tv_sec) + 1e-9 * (t1.tv_nsec - t0.tv_nsec);
    if (t < best) best = t;
  }
  uint64_t h = 14695981039346656037ull;
  for (size_t i = 0; i < n * S; i++) h = (h ^ work[i]) * 1099511628211ull;
  printf("%.4f s %016llx\n", best, (unsigned long long)h);
  free(arr);
  free(work);
  return 0;
}
