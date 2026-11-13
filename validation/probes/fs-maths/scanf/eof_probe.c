// SPDX-License-Identifier: Apache-2.0
// A black-box measurement of glibc 2.35's scanf (validation/probes/fs-maths/scanf/README.md).
#include <errno.h>
#include <stdio.h>
#include <string.h>
int main(void) {
  int a, b, c; float f;
  errno = 7; int r = sscanf("", "%d %d %d\n", &a, &b, &c); printf("sscanf empty r=%d errno=%d\n", r, errno);
  errno = 7; r = sscanf("  ", "%f", &f); printf("sscanf blank r=%d errno=%d\n", r, errno);
  errno = 7; r = sscanf("1 2", "%d %d %d\n", &a, &b, &c); printf("sscanf short r=%d errno=%d\n", r, errno);
  FILE *fp = fmemopen((void *)"  \n ", 4, "r");
  errno = 7; r = fscanf(fp, "%f", &f); printf("fscanf blank r=%d errno=%d\n", r, errno);
  errno = 7; r = fscanf(fp, "\n"); printf("fscanf nl at eof r=%d errno=%d\n", r, errno);
  fclose(fp);
  fp = fmemopen((void *)"5", 1, "r");
  errno = 7; r = fscanf(fp, "%f  ", &f); printf("fscanf %%f-space at eof r=%d errno=%d\n", r, errno);
  errno = 7; r = fscanf(fp, "\n"); printf("fscanf nl at eof r=%d errno=%d\n", r, errno);
  fclose(fp);
  fp = fmemopen((void *)"5 ", 2, "r");
  errno = 7; r = fscanf(fp, "%f %f   ", &f, &f); printf("fscanf 2 at eof r=%d errno=%d\n", r, errno);
  fclose(fp);
  return 0;
}
