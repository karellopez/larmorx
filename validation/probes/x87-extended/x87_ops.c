/* SPDX-License-Identifier: Apache-2.0 */
/* The machine's x87 on arrays of long double (x86-64: 16-byte slots, the value in the first ten
 * bytes): the hardware side of validation/probes/x87-extended. Written for this probe; no upstream
 * code. Compiled by GCC 8.4.0 with FreeSurfer 8.2.0's C flags (build.sh). */
#include <math.h>

typedef long double ld;

#define BINARY(name, op)                                                  \
  void x87_##name(const ld *a, const ld *b, ld *r, long n) {              \
    for (long i = 0; i < n; i++) r[i] = a[i] op b[i];                     \
  }
BINARY(add, +)
BINARY(sub, -)
BINARY(mul, *)
BINARY(div, /)

/* The fsqrt instruction itself. */
void x87_sqrt_insn(const ld *a, ld *r, long n) {
  for (long i = 0; i < n; i++) {
    ld x = a[i];
    __asm__("fsqrt" : "+t"(x));
    r[i] = x;
  }
}

/* glibc's sqrtl (a library call: -fno-builtin). */
void x87_sqrtl(const ld *a, ld *r, long n) {
  for (long i = 0; i < n; i++) r[i] = sqrtl(a[i]);
}

/* __builtin_sqrtl as GCC expands it (fsqrt, then sqrtl for errno when the argument is below
 * zero): what C++'s std::sqrt(long double) compiles to. */
void x87_builtin_sqrt(const ld *a, ld *r, long n) {
  for (long i = 0; i < n; i++) r[i] = __builtin_sqrtl(a[i]);
}

void x87_cmp(const ld *a, const ld *b, unsigned char *r, long n) {
  for (long i = 0; i < n; i++) {
    ld x = a[i], y = b[i];
    r[i] = (unsigned char)((x < y) | (x <= y) << 1 | (x == y) << 2 | (x != y) << 3 |
                           (x > y) << 4 | (x >= y) << 5);
  }
}

#define CONVERT(name, from, to)                                           \
  void x87_##name(const from *a, to *r, long n) {                         \
    for (long i = 0; i < n; i++) r[i] = (to)a[i];                         \
  }
CONVERT(to_f64, ld, double)
CONVERT(to_f32, ld, float)
CONVERT(to_i64, ld, long)
CONVERT(to_i32, ld, int)
CONVERT(to_i16, ld, short)
CONVERT(to_u64, ld, unsigned long)
CONVERT(to_u32, ld, unsigned int)
CONVERT(from_f64, double, ld)
CONVERT(from_f32, float, ld)
CONVERT(from_i64, long, ld)
CONVERT(from_i32, int, ld)
CONVERT(from_u64, unsigned long, ld)
CONVERT(from_u32, unsigned int, ld)

/* glibc's llrintl (a library call). */
void x87_llrintl(const ld *a, long long *r, long n) {
  for (long i = 0; i < n; i++) r[i] = llrintl(a[i]);
}

/* The x87 control word this process runs under (Linux: 0x037f). */
unsigned short x87_control_word(void) {
  unsigned short cw;
  __asm__ volatile("fnstcw %0" : "=m"(cw));
  return cw;
}
