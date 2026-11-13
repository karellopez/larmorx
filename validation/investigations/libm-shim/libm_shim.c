/* SPDX-License-Identifier: Apache-2.0 */
/* An LD_PRELOAD shim that measures whether a program's results depend on the C library's last
   bits (first use: FreeSurfer 8.2.0's programs, docs/findings/freesurfer-libm.md): for each
   function below (C-library maths without a glibc-exact version in larmorx_core::math), it
   counts the calls and the calls whose C-library result (glibc 2.35, found with
   dlsym(RTLD_NEXT)) differs from the correctly rounded one (CORE-MATH's cr_*), and returns
     - the C library's value      with LARMORX_LIBM_SHIM=count (the default): the program's results
                                  are the official ones, only counted;
     - the correctly rounded one  with LARMORX_LIBM_SHIM=cr (for the functions named in
                                  LARMORX_LIBM_SHIM_FUNCS, comma-separated; every one when unset);
     - the C library's value moved by one unit in the last place away from zero (zeros, infinities
       and NaNs unchanged)        with LARMORX_LIBM_SHIM=ulp (the same choice of functions): a
                                  sensitivity test for csqrt, which has no correctly rounded
                                  reference (both parts of its result are moved).
   At exit it appends one line per function called to LARMORX_LIBM_SHIM_LOG (default
   /tmp/larmorx-libm-shim.log): program, pid, mode, function, calls, differing. csqrt has no
   correctly rounded reference: counted (differing 0), and moved in ulp mode only. Calls of the C library only; no glibc code.
   Build: build_shim.sh (README.md). */
#define _GNU_SOURCE
#include <complex.h>
#include <dlfcn.h>
#include <errno.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

extern char *program_invocation_short_name;

enum { ACOS, ASIN, ATAN, ATAN2, TAN, LOG10, LOG2, HYPOT, ERF, ERFC, EXPM1, LOG1P, TGAMMA,
       ACOSF, ASINF, LOG10F, LOG2F, TANHF, HYPOTF, CSQRT, NF };
static const char *names[NF] = {"acos", "asin", "atan", "atan2", "tan", "log10", "log2", "hypot",
  "erf", "erfc", "expm1", "log1p", "tgamma", "acosf", "asinf", "log10f", "log2f", "tanhf",
  "hypotf", "csqrt"};
static unsigned long long calls[NF], differ[NF];
static void *real[NF];
static int use_cr[NF], use_ulp[NF];
static int mode_cr, mode_ulp;

double cr_acos(double); double cr_asin(double); double cr_atan(double); double cr_atan2(double, double);
double cr_tan(double); double cr_log10(double); double cr_log2(double); double cr_hypot(double, double);
double cr_erf(double); double cr_erfc(double); double cr_expm1(double); double cr_log1p(double);
double cr_tgamma(double);
float cr_acosf(float); float cr_asinf(float); float cr_log10f(float); float cr_log2f(float);
float cr_tanhf(float); float cr_hypotf(float, float);

static void *lookup(int i)
{
  if (!real[i]) real[i] = dlsym(RTLD_NEXT, names[i]);
  return real[i];
}

__attribute__((constructor)) static void shim_init(void)
{
  const char *m = getenv("LARMORX_LIBM_SHIM");
  mode_cr = m && !strcmp(m, "cr");
  mode_ulp = m && !strcmp(m, "ulp");
  const char *f = getenv("LARMORX_LIBM_SHIM_FUNCS");
  for (int i = 0; i < NF; i++) {
    lookup(i);
    int chosen = 1;
    if (f && *f) {
      chosen = 0;
      size_t n = strlen(names[i]);
      for (const char *p = f; (p = strstr(p, names[i])); p += n)
        if ((p == f || p[-1] == ',') && (p[n] == ',' || p[n] == 0)) chosen = 1;
    }
    use_cr[i] = mode_cr && chosen && i != CSQRT;
    use_ulp[i] = mode_ulp && chosen;
  }
}

__attribute__((destructor)) static void shim_fini(void)
{
  const char *path = getenv("LARMORX_LIBM_SHIM_LOG");
  FILE *out = fopen(path && *path ? path : "/tmp/larmorx-libm-shim.log", "a");
  if (!out) return;
  for (int i = 0; i < NF; i++)
    if (calls[i])
      fprintf(out, "%s %d %s %s %llu %llu\n", program_invocation_short_name, (int)getpid(),
              use_cr[i] ? "cr" : use_ulp[i] ? "ulp" : "count", names[i], calls[i], differ[i]);
  fclose(out);
}

static int same64(double a, double b)
{
  uint64_t x, y;
  memcpy(&x, &a, 8);
  memcpy(&y, &b, 8);
  return x == y || (isnan(a) && isnan(b));
}
static int same32(float a, float b)
{
  uint32_t x, y;
  memcpy(&x, &a, 4);
  memcpy(&y, &b, 4);
  return x == y || (isnan(a) && isnan(b));
}

/* one unit in the last place away from zero; zeros, infinities and NaNs unchanged */
static double bump64(double a)
{
  return (a == 0 || !isfinite(a)) ? a : nextafter(a, a > 0 ? INFINITY : -INFINITY);
}
static float bump32(float a)
{
  return (a == 0 || !isfinite(a)) ? a : nextafterf(a, a > 0 ? INFINITY : -INFINITY);
}
#define BUMP_double bump64
#define BUMP_float bump32

/* errno as the C library left it, so a correctly rounded value does not change it */
#define WRAP1(T, NAME, IDX, SAME)                                                       \
  T NAME(T x)                                                                           \
  {                                                                                     \
    T (*g)(T) = (T(*)(T))lookup(IDX);                                                   \
    T a = g(x);                                                                         \
    int e = errno;                                                                      \
    T c = cr_##NAME(x);                                                                 \
    errno = e;                                                                          \
    __atomic_fetch_add(&calls[IDX], 1, __ATOMIC_RELAXED);                               \
    if (!SAME(a, c)) __atomic_fetch_add(&differ[IDX], 1, __ATOMIC_RELAXED);             \
    return use_cr[IDX] ? c : use_ulp[IDX] ? BUMP_##T(a) : a;                             \
  }
#define WRAP2(T, NAME, IDX, SAME)                                                       \
  T NAME(T x, T y)                                                                      \
  {                                                                                     \
    T (*g)(T, T) = (T(*)(T, T))lookup(IDX);                                             \
    T a = g(x, y);                                                                      \
    int e = errno;                                                                      \
    T c = cr_##NAME(x, y);                                                              \
    errno = e;                                                                          \
    __atomic_fetch_add(&calls[IDX], 1, __ATOMIC_RELAXED);                               \
    if (!SAME(a, c)) __atomic_fetch_add(&differ[IDX], 1, __ATOMIC_RELAXED);             \
    return use_cr[IDX] ? c : use_ulp[IDX] ? BUMP_##T(a) : a;                             \
  }

WRAP1(double, acos, ACOS, same64)
WRAP1(double, asin, ASIN, same64)
WRAP1(double, atan, ATAN, same64)
WRAP2(double, atan2, ATAN2, same64)
WRAP1(double, tan, TAN, same64)
WRAP1(double, log10, LOG10, same64)
WRAP1(double, log2, LOG2, same64)
WRAP2(double, hypot, HYPOT, same64)
WRAP1(double, erf, ERF, same64)
WRAP1(double, erfc, ERFC, same64)
WRAP1(double, expm1, EXPM1, same64)
WRAP1(double, log1p, LOG1P, same64)
WRAP1(double, tgamma, TGAMMA, same64)
WRAP1(float, acosf, ACOSF, same32)
WRAP1(float, asinf, ASINF, same32)
WRAP1(float, log10f, LOG10F, same32)
WRAP1(float, log2f, LOG2F, same32)
WRAP1(float, tanhf, TANHF, same32)
WRAP2(float, hypotf, HYPOTF, same32)

double complex csqrt(double complex z)
{
  double complex (*g)(double complex) = (double complex(*)(double complex))lookup(CSQRT);
  __atomic_fetch_add(&calls[CSQRT], 1, __ATOMIC_RELAXED);
  double complex r = g(z);
  if (!use_ulp[CSQRT]) return r;
  int e = errno;
  r = CMPLX(bump64(creal(r)), bump64(cimag(r)));
  errno = e;
  return r;
}
