/* SPDX-License-Identifier: Apache-2.0 */
#include <stdint.h>
/* Arm's math_err.c's two helpers as they compute (no errno here). */
static double xflow(uint32_t sign, double y) { y = (sign ? -y : y) * y; return y; }
double __math_divzero(uint32_t sign) { double y = (sign ? -1.0 : 1.0) / 0.0; return y; }
double __math_invalid(double x) { double y = (x - x) / (x - x); return y; }
