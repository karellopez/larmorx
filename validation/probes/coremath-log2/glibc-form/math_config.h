/* SPDX-License-Identifier: Apache-2.0 */
/* A minimal configuration for compiling Arm's log2.c and log2_data.c alone (larmorx's probe). */
#include <stdint.h>
#define LOG2_TABLE_BITS 6
#define LOG2_POLY_ORDER 7
#define LOG2_POLY1_ORDER 11
#ifndef HAVE_FAST_FMA
#define HAVE_FAST_FMA 1
#endif
#define WANT_ROUNDING 1
#define USE_GLIBC_ABI 0
#define unlikely(x) __builtin_expect((x), 0)
static inline uint64_t asuint64(double f) { union { double f; uint64_t i; } u = {f}; return u.i; }
static inline double asdouble(uint64_t i) { union { uint64_t i; double f; } u = {i}; return u.f; }
static inline double eval_as_double(double x) { return x; }
double __math_divzero(uint32_t sign);
double __math_invalid(double x);
extern const struct log2_data {
  double invln2hi;
  double invln2lo;
  double poly[LOG2_POLY_ORDER - 1];
  double poly1[LOG2_POLY1_ORDER - 1];
  struct { double invc, logc; } tab[1 << LOG2_TABLE_BITS];
#if !HAVE_FAST_FMA
  struct { double chi, clo; } tab2[1 << LOG2_TABLE_BITS];
#endif
} __log2_data;
