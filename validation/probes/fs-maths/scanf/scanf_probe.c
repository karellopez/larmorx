// SPDX-License-Identifier: Apache-2.0
// A black-box measurement of glibc 2.35's scanf (validation/probes/fs-maths/scanf/README.md).
#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <stdint.h>
int main(void) {
  const char *in[] = {"1", "-1.5", "+.5", "5.", ".", "-", "+", "1e", "1e5", "1e+", "1e-3x", "100ergs", "0x", "0x1p3", "0x1.8", "0xg", "0x.p1",
    "inf", "INF", "infinity", "infin", "infx", "-inf", "nan", "-nan", "nan(123)", "nan(0x7)", "nan(", "nan(abc", "nanx", "na", "i",
    "1e40", "-1e40", "1e-50", "1e-40", "3.4028235e38", "3.40282357e38", "1.17549435e-38", "0x1p-149", "0x1p-150", "0x1.000001p0", "  \t\n7", "", "   ", "e5", "1.2.3", "1,5", "00012", "1e0005", ".e1", "0.000000000000000000000000000000000000000000001",
    "1e+x", "5.e3", "-.5", "0x1p", "0x1p+", "0x1.8p1x", "0x.8", "0x.", "0X1P2", "infinityx", "infinit", "+nan", "0.", "-0", ".5e-1", "1e-", "1ee5", "1e5e5", "0x1.fffffffp127", "0xp1", "0x+1", "-0x1p0", "0x1p-126", "0x0.8p-149", "0x1.8p-149", "0x1.4p-149", "0x1.cp-149", "1.4e-45", "0.7e-45", "0x123456789abcdef0123p0", "0x1p99999999999", "1e99999999999", "1e-99999999999", "0e99999999999", "0x0p99999999999", "0x1.fffffep-127", "0x1.ffffffp-127", "0x1.fffffcp-127", "1.40129846432481707092372958328991613128026194187651577175706828388979108268586060148663818836212158203125e-45", "1.1754942e-38", "1.17549428e-38", "0x1.fffffep127", "0x1.ffffffp127", "-0x", "-.", "-e", "0x1.", "0xa.", "1.e", "infi", "-infin", "NaN", "nAN", "Inf", "-NAN", NULL};
  for (int i = 0; in[i]; i++) {
    FILE *fp = fmemopen((void *)in[i], strlen(in[i]) ? strlen(in[i]) : 1, "r");
    if (!strlen(in[i])) { fclose(fp); fp = fmemopen((void*)"x", 0 + 1, "r"); fgetc(fp); }
    float f = 12345.0f; errno = 0;
    int r = fscanf(fp, "%f", &f);
    int e = errno;
    long pos = ftell(fp);
    int next = fgetc(fp);
    uint32_t u; memcpy(&u, &f, 4);
    printf("%-50s r=%2d f=%08x errno=%d pos=%ld next=%d\n", in[i], r, u, e, pos, next);
    fclose(fp);
  }
  int a=7,b=7,c=7; const char *d[] = {"1 2 3", "1 2", "-0 +5 3x", "2147483648 1 1", "99999999999 1 1", " 3\t4\n5", "1 2 3 4", "a 1 2", NULL};
  for (int i = 0; d[i]; i++) { a=b=c=7; errno=0; int r = sscanf(d[i], "%d %d %d\n", &a, &b, &c); printf("%-20s r=%d %d %d %d errno=%d\n", d[i], r, a, b, c, errno); }
  return 0;
}
