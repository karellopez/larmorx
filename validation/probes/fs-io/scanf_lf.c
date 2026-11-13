// SPDX-License-Identifier: Apache-2.0
// glibc's `sscanf("%lf%n")` as a black box, for larmorx-freesurfer-core's cscan (`%lf`, the
// TSV reader's fscanf): for each argument, the conversion's count, the double's bits, errno and
// the bytes read. Build and run: cc -O0 scanf_lf.c -o scanf_lf && ./scanf_lf 1 0.1 1e400 ...
#include <errno.h>
#include <inttypes.h>
#include <stdio.h>
#include <string.h>

int main(int argc, char **argv)
{
  for (int i = 1; i < argc; i++) {
    double d = 0;
    int n = -1;
    errno = 0;
    int r = sscanf(argv[i], "%lf%n", &d, &n);
    uint64_t u;
    memcpy(&u, &d, 8);
    printf("%s\t%d\t%016" PRIx64 "\t%d\t%d\n", argv[i], r, u, errno, n);
  }
  return 0;
}
