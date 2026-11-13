// SPDX-License-Identifier: Apache-2.0
// A black-box measurement of glibc 2.35's scanf (validation/probes/fs-maths/scanf/README.md).
#include <errno.h>
#include <stdio.h>
int main(int argc, char **argv) {
  FILE *fp = fopen(argv[1], "r"); errno = 0;
  for (int i = 0; i < 4; i++) { float f; int r = fscanf(fp, "%f", &f); printf(" [r=%d errno=%d]", r, errno); }
  printf("\n"); return 0;
}
