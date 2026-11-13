// SPDX-License-Identifier: Apache-2.0
// Build: as validation/probes/fs-maths/build.sh builds fsmaths (the unit oracle's g++, its include
// folders and libraries, -fdata-sections -ffunction-sections -Wl,--gc-sections); run with a 2 GB
// address-space limit. Prints "<type> <rows> <cols> -> ok|NULL <rows> <cols>" for each size that
// returns and "CRASH <signal>" or "EXIT <status>" for the others.
// FS-13: which negative sizes FreeSurfer's MatrixAlloc makes a matrix for (each in a forked child)
#include <sys/wait.h>
#include <unistd.h>
#include <cstdio>
#include <cstdlib>
#include "matrix.h"
const char *Progname = "matrixalloc_negative";
int main()
{
  for (int type : {MATRIX_REAL, MATRIX_COMPLEX})
    for (int r = -20; r <= 3; r++)
      for (int c = -20; c <= 3; c++) {
        if (r >= 0 && c >= 0) continue;
        fflush(stdout);
        pid_t p = fork();
        if (p == 0) {
          freopen("/dev/null", "w", stderr);
          int fd = dup(1);
          freopen("/dev/null", "w", stdout);
          MATRIX *m = MatrixAlloc(r, c, type);
          char b[128];
          int n = snprintf(b, sizeof b, "%d %d %d -> %s %d %d\n", type, r, c, m ? "ok" : "NULL", m ? m->rows : 0, m ? m->cols : 0);
          write(fd, b, n);
          _exit(0);
        }
        int st;
        waitpid(p, &st, 0);
        if (!WIFEXITED(st) || WEXITSTATUS(st)) printf("%d %d %d -> %s %d\n", type, r, c, WIFSIGNALED(st) ? "CRASH" : "EXIT", WIFSIGNALED(st) ? WTERMSIG(st) : WEXITSTATUS(st));
      }
  return 0;
}
