// SPDX-License-Identifier: Apache-2.0
// A black-box measurement of glibc 2.35's scanf (validation/probes/fs-maths/scanf/README.md).
#include <stdio.h>
#include <string.h>
int main(void) {
  const char *in[] = {"infin\n", "infinx", "infix", "infinix", "inx", "ix", "i\n", "nax", "nx", "n\n", "infinit\n", "0x\n", "0xg", "-x", "-\n", ".x", ".\n", "+.x", "abc", "1,5", "e5", "-0x", "-0xz", "0x.", "0x.z", "1e\n", "1ex", NULL};
  for (int i = 0; in[i]; i++) {
    FILE *fp = fmemopen((void *)in[i], strlen(in[i]), "r");
    float f = 0; int r = fscanf(fp, "%f", &f);
    long pos = ftell(fp); int next = fgetc(fp);
    printf("%-10s r=%2d pos=%ld next=%d\n", in[i][strlen(in[i])-1]=='\n' ? "(nl)" : in[i], r, pos, next);
    fclose(fp);
  }
  return 0;
}
