// SPDX-License-Identifier: Apache-2.0
// A black-box measurement of glibc 2.35's scanf (validation/probes/fs-maths/scanf/README.md).
#include <errno.h>
#include <stdio.h>
#include <string.h>
static void run(const char *text, int n) {
  FILE *fp = fmemopen((void *)text, strlen(text), "r");
  errno = 0;
  printf("%-24s:", text);
  for (int i = 0; i < n; i++) { float f; int r = fscanf(fp, "%f", &f); printf(" [r=%d errno=%d]", r, errno); }
  printf("\n");
  fclose(fp);
}
int main(void) {
  run("1e40 5", 3);
  run("1e40", 2);
  run("1e40 x", 2);
  run("1e40 1e-50", 2);
  run("5 1e40", 3);
  run("1e40 . ", 2);
  run("1e40 -", 2);
  run("1e40 inf", 2);
  run("1e40 nan", 2);
  run("1e40 0x1p3", 2);
  run("1e40 1", 2);
  run("1e40\n1\n", 3);
  return 0;
}
