// SPDX-License-Identifier: Apache-2.0
// glibc 2.35 measured as a black box: what sscanf(input, "%*s %s"), "%*s %f" and "%f %f %f %f" return
// and leave in errno (7 before) at the end of the input (an ERANGE at the end included). Build and run: gcc -O0 scanf_word_errno.c && ./a.out
// (relay 15 of track C: the fs-volume probe's ltaread found the port keeping errno where glibc sets 0).
#include <stdio.h>
#include <errno.h>
static void words(void){
  const char *in[] = {"fscale", "fscale ", "fscale\n", "fscale 1e40", "fscale 1e40\n", "subject", "subject ", "subject x", "subject x\n", "", " ", "fscale x"};
  for (int i = 0; i < 12; i++) {
    char s[100]; float f; int r1, r2, e1, e2;
    errno = 7; r1 = sscanf(in[i], "%*s %s", s); e1 = errno;
    errno = 7; r2 = sscanf(in[i], "%*s %f", &f); e2 = errno;
    printf("%-14s %%s: %2d errno %2d   %%f: %2d errno %2d\n", in[i][0] ? in[i] : "(empty)", r1, e1, r2, e2);
  }
}

static void numbers(void){
  const char *in[] = {"-1877   1e40", "-1877   1e40 ", "1e40", "1e40 ", "5 1e40", "1e-50", "1e-50 3", "fscale 1e40", "1 2"};
  for (int i = 0; i < 9; i++) {
    float a,b,c,d; int r;
    errno = 7; r = sscanf(in[i], "%f %f %f %f", &a,&b,&c,&d);
    printf("%-16s ffff: %2d errno %2d", in[i], r, errno);
    errno = 7; r = sscanf(in[i], "%f", &a);
    printf("   f: %2d errno %2d", r, errno);
    errno = 7; r = sscanf(in[i], "%*s %f", &a);
    printf("   *s f: %2d errno %2d\n", r, errno);
  }
}

int main(void) { words(); numbers(); return 0; }
