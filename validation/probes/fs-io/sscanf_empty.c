// SPDX-License-Identifier: Apache-2.0
// glibc's sscanf as a black box (track A relay 27, FS-658): the return value and errno (set to 2 first)
// for an empty, blank or short text under %f, %d, %lf, %*s %d, %*s %f and %s; an end met before the first
// conversion gives EOF with errno 0. Build: gcc -O2 sscanf_empty.c -o sscanf_empty; run it.
#include <stdio.h>
#include <errno.h>
int main(void){
  const char *in[] = {"", "  ", "\n", "abc", "1.5", "x 1", "1e999", "-", ".", "nan"};
  const char *fm[] = {"%f", "%d", "%lf", "%*s %d", "%*s %f", "%s"};
  for (int j = 0; j < 6; j++) for (int i = 0; i < 10; i++) {
    float f = 0; int d = 0; double lf = 0; char s[64];
    errno = 2; int r;
    if (j == 0) r = sscanf(in[i], fm[j], &f);
    else if (j == 1) r = sscanf(in[i], fm[j], &d);
    else if (j == 2) r = sscanf(in[i], fm[j], &lf);
    else if (j == 3) r = sscanf(in[i], fm[j], &d);
    else if (j == 4) r = sscanf(in[i], fm[j], &f);
    else r = sscanf(in[i], fm[j], s);
    printf("%-7s %-6s r=%2d errno=%d\n", fm[j], in[i][0]=='\n'?"\\n":in[i], r, errno);
  }
}
