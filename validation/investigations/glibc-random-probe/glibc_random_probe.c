/* SPDX-License-Identifier: Apache-2.0 */
/* A black-box probe of the C library's rand, srand, random and srandom (glibc 2.35 on the
   reference machine), for specs/glibc-random.md. Calls of the system's functions only.

   Seeds are read as signed 64-bit integers and converted to unsigned int as C converts a long
   (modulo 2^32), the conversion FreeSurfer's setRandomSeed(long) makes when it calls srand.

   Commands:
     rand <seed> <count>             srand(seed), then <count> values of rand()
     random <seed> <count>           srandom(seed), then <count> values of random()
     default <count>                 <count> values of rand() with no seeding call before
     shared <seed> <count>           srand(seed), then <count> values alternating rand() and random();
                                     then srand(seed); srandom(seed) (as setRandomSeed) and <count>
                                     values of rand(); then srandom(seed) and <count> of rand()
     hash <seed0> <nseeds> <values>  for the <nseeds> seeds seed0, seed0 + 1, ... (modulo 2^32):
                                     srand(seed), then <values> values of rand(); prints the FNV-1a
                                     64-bit hash of all of them in that order (h = (h XOR v) * P mod
                                     2^64 per value v, h0 = 14695981039346656037, P = 1099511628211)
     long <seed> <count>             srand(seed), then the same hash over <count> values of rand()
     raw <seed0> <nseeds> <values>   as hash, but writes the values to standard output as unsigned
                                     32-bit little-endian binary integers (for a comparison program)
   Values are printed in decimal, one per line. */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static unsigned int seed_arg(const char *s)
{
  long long v = strtoll(s, NULL, 0);
  return (unsigned int)v;
}

static uint64_t fnv(uint64_t h, uint32_t v)
{
  return (h ^ (uint64_t)v) * 1099511628211ULL;
}

int main(int argc, char **argv)
{
  if (argc < 3) {
    fprintf(stderr, "usage: see the header of glibc_random_probe.c\n");
    return 2;
  }
  const char *cmd = argv[1];
  if (!strcmp(cmd, "rand") && argc == 4) {
    long n = atol(argv[3]);
    srand(seed_arg(argv[2]));
    for (long i = 0; i < n; i++) printf("%d\n", rand());
  } else if (!strcmp(cmd, "random") && argc == 4) {
    long n = atol(argv[3]);
    srandom(seed_arg(argv[2]));
    for (long i = 0; i < n; i++) printf("%ld\n", random());
  } else if (!strcmp(cmd, "default") && argc == 3) {
    long n = atol(argv[2]);
    for (long i = 0; i < n; i++) printf("%d\n", rand());
  } else if (!strcmp(cmd, "shared") && argc == 4) {
    unsigned int s = seed_arg(argv[2]);
    long n = atol(argv[3]);
    srand(s);
    for (long i = 0; i < n; i++) printf("%ld\n", (i % 2) ? random() : (long)rand());
    srand(s);
    srandom(s);
    for (long i = 0; i < n; i++) printf("%d\n", rand());
    srandom(s);
    for (long i = 0; i < n; i++) printf("%d\n", rand());
  } else if (!strcmp(cmd, "hash") && argc == 5) {
    uint32_t s0 = seed_arg(argv[2]);
    unsigned long long ns = strtoull(argv[3], NULL, 0);
    long nv = atol(argv[4]);
    uint64_t h = 14695981039346656037ULL;
    for (unsigned long long k = 0; k < ns; k++) {
      srand((unsigned int)(s0 + (uint32_t)k));
      for (long i = 0; i < nv; i++) h = fnv(h, (uint32_t)rand());
    }
    printf("%016llx\n", (unsigned long long)h);
  } else if (!strcmp(cmd, "raw") && argc == 5) {
    uint32_t s0 = seed_arg(argv[2]);
    unsigned long long ns = strtoull(argv[3], NULL, 0);
    long nv = atol(argv[4]);
    uint32_t buf[4096];
    size_t m = 0;
    for (unsigned long long k = 0; k < ns; k++) {
      srand((unsigned int)(s0 + (uint32_t)k));
      for (long i = 0; i < nv; i++) {
        buf[m++] = (uint32_t)rand();
        if (m == 4096) {
          if (fwrite(buf, 4, m, stdout) != m) return 1;
          m = 0;
        }
      }
    }
    if (m && fwrite(buf, 4, m, stdout) != m) return 1;
  } else if (!strcmp(cmd, "long") && argc == 4) {
    long long n = atoll(argv[3]);
    uint64_t h = 14695981039346656037ULL;
    srand(seed_arg(argv[2]));
    for (long long i = 0; i < n; i++) h = fnv(h, (uint32_t)rand());
    printf("%016llx\n", (unsigned long long)h);
  } else {
    fprintf(stderr, "unknown command or wrong number of arguments: %s\n", cmd);
    return 2;
  }
  return 0;
}
