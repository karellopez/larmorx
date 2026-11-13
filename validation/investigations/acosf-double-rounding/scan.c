/* SPDX-License-Identifier: Apache-2.0 */
/* Every float x in [-1, 1]: d = CORE-MATH's cr_acos((double)x) (correctly rounded double); prints
   the x whose d is exactly halfway between two floats (where rounding d to float may differ from
   the correctly rounded float acos), and counts them. */
#include <stdio.h>
#include <stdint.h>
#include <string.h>
double cr_acos(double);
int main(void) {
  uint64_t n = 0, mid = 0;
  for (uint64_t u = 0; u <= 0xffffffffu; u++) {
    uint32_t b = (uint32_t)u; float x; memcpy(&x, &b, 4);
    if (!(x >= -1.0f && x <= 1.0f)) continue;
    n++;
    double d = cr_acos((double)x);
    uint64_t db; memcpy(&db, &d, 8);
    /* d's significand below a float's: 29 bits; a midpoint is 1 followed by 28 zeros (normal floats; acos >= 0 and < 4) */
    if ((db & 0x1fffffffu) == 0x10000000u) { mid++; printf("MID %08x %a %.17g\n", b, d, d); }
    /* d exactly a float: rounding is exact too (also fine) */
  }
  printf("N %llu MIDPOINTS %llu\n", (unsigned long long)n, (unsigned long long)mid);
  return 0;
}
