// SPDX-License-Identifier: Apache-2.0
// A black-box measurement of glibc 2.35's scanf (validation/probes/fs-maths/scanf/README.md).
#include <errno.h>
#include <stdio.h>
#include <string.h>
static void f(const char *in, const char *fmt) {
  float a, b; int x, y; FILE *fp = fmemopen((void *)in, strlen(in) ? strlen(in) : 1, "r"); if (!strlen(in)) fgetc(fp);
  errno = 7; int r = strchr(fmt, 'd') ? fscanf(fp, fmt, &x, &y) : fscanf(fp, fmt, &a, &b);
  printf("fscanf %-8s on %-6s r=%2d errno=%d   ", fmt, in, r, errno);
  errno = 7; r = strchr(fmt, 'd') ? sscanf(in, fmt, &x, &y) : sscanf(in, fmt, &a, &b);
  printf("sscanf r=%2d errno=%d\n", r, errno); fclose(fp);
}
int main(void) {
  const char *ins[] = {"", " ", "5", "5 ", "5  ", "5 6", NULL};
  const char *fmts[] = {"%f", "%f %f", "%f%f", "%d %d", "%d%d", NULL};
  for (int j = 0; fmts[j]; j++) for (int i = 0; ins[i]; i++) f(ins[i], fmts[j]);
  return 0;
}
