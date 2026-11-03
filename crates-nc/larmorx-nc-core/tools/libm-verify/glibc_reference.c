/* SPDX-License-Identifier: LicenseRef-FSL */
/* Copyright 2026 Karel Lopez Vilaret. */
/* The reference side of libm-verify: reads "<op> <a> <b>" lines (operands as 64-bit hex bit
   patterns of doubles) and prints each result's bits in hex, computed by this machine's C
   library and x87 unit: pow, tanh, exp, log, logf, expf, powf (glibc), and the long double operations of FSL's NIfTI layer
   (x87: the exact result rounded to 64 bits, then to the stored type).
   Build: gcc -O2 -o glibc_reference glibc_reference.c -lm */
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

/* The symbols FSL's binaries bind (built against an older C library): the SVID-compatible
   wrappers of GLIBC_2.2.5, named explicitly so that any compiler binds them (a compiler whose
   sysroot is the system's binds exp@GLIBC_2.29, powf@GLIBC_2.27, ... otherwise). */
double exp_compat(double);
double log_compat(double);
double pow_compat(double, double);
float expf_compat(float);
float logf_compat(float);
float powf_compat(float, float);
__asm__(".symver exp_compat,exp@GLIBC_2.2.5");
__asm__(".symver log_compat,log@GLIBC_2.2.5");
__asm__(".symver pow_compat,pow@GLIBC_2.2.5");
__asm__(".symver expf_compat,expf@GLIBC_2.2.5");
__asm__(".symver logf_compat,logf@GLIBC_2.2.5");
__asm__(".symver powf_compat,powf@GLIBC_2.2.5");

static double d(uint64_t b) { double x; memcpy(&x, &b, 8); return x; }
static uint64_t bits(double x) { uint64_t b; memcpy(&b, &x, 8); return b; }
static uint64_t fbits(float x) { uint32_t b; memcpy(&b, &x, 4); return b; }

int main(void) {
  char op[16];
  unsigned long long a, b;
  while (scanf("%15s %llx %llx", op, &a, &b) == 3) {
    volatile double x = d(a), y = d(b);
    uint64_t r = 0;
    if (!strcmp(op, "pow")) r = bits(pow_compat(x, y));
    else if (!strcmp(op, "tanh")) r = bits(tanh(x));
    else if (!strcmp(op, "exp")) r = bits(exp_compat(x));
    else if (!strcmp(op, "log")) r = bits(log_compat(x));
    else if (!strcmp(op, "logf")) r = fbits(logf_compat((float) x));
    else if (!strcmp(op, "expf")) r = fbits(expf_compat((float) x));
    else if (!strcmp(op, "powf")) r = fbits(powf_compat((float) x, (float) y));
    else if (!strcmp(op, "lddiv")) { volatile long double l = (long double) x / y; r = bits((double) l); }
    else if (!strcmp(op, "ldadd")) { volatile long double l = (long double) x + y; r = bits((double) l); }
    else if (!strcmp(op, "ldsub")) { volatile long double l = (long double) x - y; r = bits((double) l); }
    else if (!strcmp(op, "ldmulf")) { volatile long double l = (long double) x * y; r = fbits((float) l); }
    else if (!strcmp(op, "ldmul")) { volatile long double l = (long double) x * y; r = bits((double) l); }
    printf("%016llx\n", (unsigned long long) r);
  }
  return 0;
}
