// SPDX-License-Identifier: Apache-2.0
// A black-box probe of FreeSurfer 8.2.0's random generator and distributions (utils/numerics.cpp's
// sc_* functions, RANLUX389 and its distributions), linked against the unit oracle's libutils.a
// (GCC 8.4.0, FreeSurfer's flags). It holds only calls: no FreeSurfer or GSL code.
//
// Usage (one command per run; every value on its own line):
//   fs_rng_probe raw <seed> <count>                 the generator's 24-bit integers, decimal
//   fs_rng_probe dist <name> <seed> <count> [p1 [p2]]  draws as the 16 hex digits of their bits
//       names: uni uni_pos flat(a,b) gaussian(sigma) ugauss gamma(a,b) gamma_int(n) gamma_large(a)
//              gamma_frac(a) chisq(nu) fdist(nu1,nu2) tdist(nu) exponential(mu)
//   fs_rng_probe rf <name> <seed> <count> [p1 [p2]]    RFspecInit(seed) + RFdrawVal, as mri_coreg
//   fs_rng_probe mixed <seed> <count>               one stream, the draws of a fixed rotation of
//       every distribution (checks that each consumes the stream as specified): draw k is, by k mod 14,
//       uni, uni_pos, flat(-3.5, 7.25), gaussian(2.5), ugauss, gamma(3, 2), gamma(0.4, 1.5),
//       gamma(7.3, 0.5), gamma(25, 1), gamma(31.6, 1), chisq(5), fdist(4, 9),
//       tdist(1.5 if k mod 28 = 12, else 7), exponential(0.75)
//   fs_rng_probe crafted <name> <a> <b> [p1 [p2]]   one draw from a state whose next integers are a, b
//   fs_rng_probe zero-seeds <first> <last>          the 32-bit seeds whose seeded state is all zero
// <seed> is parsed as an unsigned 64-bit integer, or as a signed one when it starts with '-'
// (converted as C does: modulo 2^64).
#include <cinttypes>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>

#include "numerics.h"
#include "randomfields.h"

// Defined in utils/numerics.cpp with external linkage, not declared in its header.
double sc_uni(const sc_rng *r);
double sc_uni_pos(const sc_rng *r);
double sc_gamma_int(const sc_rng *r, const unsigned int a);
double sc_gamma_large(const sc_rng *r, const double a);
double sc_gamma_frac(const sc_rng *r, const double a);
double sc_ugauss(const sc_rng *r);

static unsigned long parse_seed(const char *s)
{
  if (s[0] == '-') return (unsigned long)strtoll(s, NULL, 10);
  return strtoull(s, NULL, 10);
}

static void put(double v)
{
  uint64_t b;
  memcpy(&b, &v, sizeof b);
  printf("%016" PRIx64 "\n", b);
}

static double draw(const char *name, const sc_rng *r, double p1, double p2)
{
  if (!strcmp(name, "uni")) return sc_uni(r);
  if (!strcmp(name, "uni_pos")) return sc_uni_pos(r);
  if (!strcmp(name, "flat")) return sc_ran_flat(r, p1, p2);
  if (!strcmp(name, "gaussian")) return sc_ran_gaussian(r, p1);
  if (!strcmp(name, "ugauss")) return sc_ugauss(r);
  if (!strcmp(name, "gamma")) return sc_ran_gamma(r, p1, p2);
  if (!strcmp(name, "gamma_int")) return sc_gamma_int(r, (unsigned int)p1);
  if (!strcmp(name, "gamma_large")) return sc_gamma_large(r, p1);
  if (!strcmp(name, "gamma_frac")) return sc_gamma_frac(r, p1);
  if (!strcmp(name, "chisq")) return sc_ran_chisq(r, p1);
  if (!strcmp(name, "fdist")) return sc_ran_fdist(r, p1, p2);
  if (!strcmp(name, "tdist")) return sc_ran_tdist(r, p1);
  if (!strcmp(name, "exponential")) return sc_ran_exponential(r, p1);
  fprintf(stderr, "unknown distribution %s\n", name);
  exit(2);
}

int main(int argc, char **argv)
{
  if (argc < 4) {
    fprintf(stderr, "usage: fs_rng_probe raw|dist|rf|mixed ...\n");
    return 2;
  }
  const char *cmd = argv[1];
  if (!strcmp(cmd, "raw")) {
    sc_rng *r = sc_rng_alloc(&intern_rng_type);
    sc_rng_set(r, parse_seed(argv[2]));
    long n = atol(argv[3]);
    for (long k = 0; k < n; k++) printf("%lu\n", sc_rng_get(r));
    return 0;
  }
  if (!strcmp(cmd, "dist") || !strcmp(cmd, "rf")) {
    if (argc < 5) return 2;
    const char *name = argv[2];
    unsigned long seed = parse_seed(argv[3]);
    long n = atol(argv[4]);
    double p1 = argc > 5 ? strtod(argv[5], NULL) : 0.0;
    double p2 = argc > 6 ? strtod(argv[6], NULL) : 0.0;
    if (!strcmp(cmd, "dist")) {
      sc_rng *r = sc_rng_alloc(&intern_rng_type);
      sc_rng_set(r, seed);
      for (long k = 0; k < n; k++) put(draw(name, r, p1, p2));
    }
    else {
      RFS *rfs = RFspecInit(seed, NULL);
      rfs->name = strdup(name);
      rfs->params[0] = p1;
      rfs->params[1] = p2;
      for (long k = 0; k < n; k++) put(RFdrawVal(rfs));
    }
    return 0;
  }
  if (!strcmp(cmd, "mixed")) {
    sc_rng *r = sc_rng_alloc(&intern_rng_type);
    sc_rng_set(r, parse_seed(argv[2]));
    long n = atol(argv[3]);
    for (long k = 0; k < n; k++) {
      switch (k % 14) {
        case 0: put(sc_uni(r)); break;
        case 1: put(sc_uni_pos(r)); break;
        case 2: put(sc_ran_flat(r, -3.5, 7.25)); break;
        case 3: put(sc_ran_gaussian(r, 2.5)); break;
        case 4: put(sc_ugauss(r)); break;
        case 5: put(sc_ran_gamma(r, 3.0, 2.0)); break;
        case 6: put(sc_ran_gamma(r, 0.4, 1.5)); break;
        case 7: put(sc_ran_gamma(r, 7.3, 0.5)); break;
        case 8: put(sc_ran_gamma(r, 25.0, 1.0)); break;
        case 9: put(sc_ran_gamma(r, 31.6, 1.0)); break;
        case 10: put(sc_ran_chisq(r, 5.0)); break;
        case 11: put(sc_ran_fdist(r, 4.0, 9.0)); break;
        case 12: put(sc_ran_tdist(r, k % 28 == 12 ? 1.5 : 7.0)); break;
        case 13: put(sc_ran_exponential(r, 0.75)); break;
      }
    }
    return 0;
  }
  if (!strcmp(cmd, "crafted")) {
    // crafted <name> <a> <b> [p1 [p2]]: a generator state whose next two integers are a and b
    // (then 0 until the 24th call: every other value of the state is 0, the carry 0, the decimation
    // counter at its start), then one draw of the distribution. Shows what a distribution does on
    // a given pair of uniforms (e.g. gaussian 0 8388608: X = -1, Y = 0, R = 1).
    if (argc < 5) return 2;
    sc_rng *r = sc_rng_alloc(&intern_rng_type);
    sc_rng_set(r, 1);
    sc_status_t *st = (sc_status_t *)r->status;
    for (int i = 0; i < 24; i++) st->u[i] = 0;
    st->carry = 0;
    st->n = 0;
    st->i = 23;
    st->j = 9;
    st->u[9] = strtoul(argv[3], NULL, 10) & 0xffffffUL;
    st->u[8] = strtoul(argv[4], NULL, 10) & 0xffffffUL;
    double p1 = argc > 5 ? strtod(argv[5], NULL) : 0.0;
    double p2 = argc > 6 ? strtod(argv[6], NULL) : 0.0;
    put(draw(argv[2], r, p1, p2));
    return 0;
  }
  if (!strcmp(cmd, "zero-seeds")) {
    // Every 32-bit int seed (as mri_coreg's --seed passes it: sign-extended to unsigned long) in
    // [first, last], and the seeds whose seeded state holds 24 zeros (printed with the carry).
    long first = atol(argv[2]), last = atol(argv[3]);
    sc_rng *r = sc_rng_alloc(&intern_rng_type);
    sc_status_t *st = (sc_status_t *)r->status;
    long found = 0;
    for (long s = first; s <= last; s++) {
      sc_rng_set(r, (unsigned long)(long)(int)s);
      int all0 = 1;
      for (int i = 0; i < 24 && all0; i++)
        if (st->u[i] != 0) all0 = 0;
      if (all0) {
        printf("%ld carry=%u\n", s, st->carry);
        found++;
      }
    }
    printf("found %ld in [%ld, %ld]\n", found, first, last);
    return 0;
  }
  fprintf(stderr, "unknown command %s\n", cmd);
  return 2;
}
