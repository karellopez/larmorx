/* SPDX-License-Identifier: Apache-2.0 */
/* The probe's NrrdIO entry point: nrrdLoad in a forked child, its result written as a dump (the
 * format of src/dump.rs) on a pipe, its standard error captured. Compiled once against NrrdIO
 * r3991's header (libup.so) and once against FreeSurfer 8.2.0's (libfs.so); calls only NrrdIO's
 * public names. */
#include <errno.h>
#include <poll.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#include "NrrdIO.h"

typedef struct {
  unsigned char *p;
  size_t n, cap;
} Buf;

static void bput(Buf *b, const void *d, size_t n) {
  if (b->n + n + 1 > b->cap) {
    b->cap = (b->n + n + 1) * 2;
    b->p = realloc(b->p, b->cap);
  }
  memcpy(b->p + b->n, d, n);
  b->n += n;
}
static void bstr(Buf *b, const char *s) { bput(b, s, strlen(s)); }
static void bhex(Buf *b, const unsigned char *d, size_t n) {
  static const char H[] = "0123456789abcdef";
  size_t i;
  for (i = 0; i < n; i++) {
    char c[2] = {H[d[i] >> 4], H[d[i] & 15]};
    bput(b, c, 2);
  }
}
static void bcs(Buf *b, const char *s) { /* a C string or "-" */
  if (!s) {
    bstr(b, "-");
  } else {
    bstr(b, "s");
    bhex(b, (const unsigned char *)s, strlen(s));
  }
}
static void bnum(Buf *b, const char *fmt, ...) __attribute__((format(printf, 2, 3)));
#include <stdarg.h>
static void bnum(Buf *b, const char *fmt, ...) {
  char t[128];
  va_list ap;
  va_start(ap, fmt);
  vsnprintf(t, sizeof t, fmt, ap);
  va_end(ap);
  bstr(b, t);
}
static void bdbl(Buf *b, double v) {
  uint64_t u;
  memcpy(&u, &v, 8);
  bnum(b, " %016llx", (unsigned long long)u);
}

static void dump(Buf *b, int ret, Nrrd *n, char *err) {
  unsigned int i, j;
  bnum(b, "ret %d\nbiff ", ret);
  bcs(b, err);
  bstr(b, "\n");
  if (ret) return;
  bnum(b, "dim %u type %d blockSize %lu space %d spaceDim %u\n", n->dim, n->type,
       (unsigned long)n->blockSize, n->space, n->spaceDim);
  bstr(b, "content ");
  bcs(b, n->content);
  bstr(b, "\nsampleUnits ");
  bcs(b, n->sampleUnits);
  bstr(b, "\nspaceUnits");
  for (i = 0; i < NRRD_SPACE_DIM_MAX; i++) {
    bstr(b, " ");
    bcs(b, n->spaceUnits[i]);
  }
  bstr(b, "\nspaceOrigin");
  for (i = 0; i < NRRD_SPACE_DIM_MAX; i++) bdbl(b, n->spaceOrigin[i]);
  bstr(b, "\nmf");
  for (i = 0; i < NRRD_SPACE_DIM_MAX; i++)
    for (j = 0; j < NRRD_SPACE_DIM_MAX; j++) bdbl(b, n->measurementFrame[i][j]);
  bstr(b, "\nold");
  bdbl(b, n->oldMin);
  bdbl(b, n->oldMax);
  bstr(b, "\n");
  for (i = 0; i < NRRD_DIM_MAX; i++) {
    NrrdAxisInfo *a = n->axis + i;
    bnum(b, "axis %u %lu", i, (unsigned long)a->size);
    bdbl(b, a->spacing);
    bdbl(b, a->thickness);
    bdbl(b, a->min);
    bdbl(b, a->max);
    for (j = 0; j < NRRD_SPACE_DIM_MAX; j++) bdbl(b, a->spaceDirection[j]);
    bnum(b, " %d %d ", a->center, a->kind);
    bcs(b, a->label);
    bstr(b, " ");
    bcs(b, a->units);
    bstr(b, "\n");
  }
  bnum(b, "cmt %u\n", n->cmtArr->len);
  for (i = 0; i < n->cmtArr->len; i++) {
    bcs(b, n->cmt[i]);
    bstr(b, "\n");
  }
  bnum(b, "kvp %u\n", n->kvpArr->len);
  for (i = 0; i < n->kvpArr->len; i++) {
    bcs(b, n->kvp[2 * i]);
    bstr(b, " ");
    bcs(b, n->kvp[2 * i + 1]);
    bstr(b, "\n");
  }
  bstr(b, "data ");
  if (n->data) {
    bhex(b, n->data, nrrdElementNumber(n) * nrrdElementSize(n));
  } else {
    bstr(b, "-");
  }
  bstr(b, "\n");
}

static void child(const char *fname, int fd) {
  Buf b = {0};
  Nrrd *nin = nrrdNew();
  NrrdIoState *nio = nrrdIoStateNew();
  int ret = nrrdLoad(nin, fname, nio);
  char *err = ret ? biffGetDone(NRRD) : NULL;
  dump(&b, ret, nin, err);
  {
    size_t at = 0;
    while (at < b.n) {
      ssize_t w = write(fd, b.p + at, b.n - at);
      if (w <= 0) break;
      at += (size_t)w;
    }
  }
  fflush(stdout);
  fflush(stderr);
  _exit(0);
}

/* The outcome: 0 the child returned (dump in *out), 1 it exited with *status, 2 it died of
 * signal *status, 3 it ran past timeout_ms (killed). *err holds its standard error. */
int fm_nrrd_load(const char *fname, int timeout_ms, unsigned char **out, size_t *outlen,
                 unsigned char **err, size_t *errlen, int *status) {
  int pd[2], pe[2], st = 0, done = 0, res;
  pid_t pid;
  Buf bo = {0}, be = {0};
  struct timespec t0, t1;
  if (pipe(pd) || pipe(pe)) return -1;
  fflush(NULL);
  pid = fork();
  if (pid == 0) {
    close(pd[0]);
    close(pe[0]);
    dup2(pe[1], 2);
    child(fname, pd[1]);
  }
  close(pd[1]);
  close(pe[1]);
  clock_gettime(CLOCK_MONOTONIC, &t0);
  {
    struct pollfd fds[2] = {{pd[0], POLLIN, 0}, {pe[0], POLLIN, 0}};
    int open_n = 2;
    while (open_n > 0) {
      long ms;
      int k;
      clock_gettime(CLOCK_MONOTONIC, &t1);
      ms = (t1.tv_sec - t0.tv_sec) * 1000 + (t1.tv_nsec - t0.tv_nsec) / 1000000;
      if (ms >= timeout_ms) {
        kill(pid, SIGKILL);
        done = 3;
        break;
      }
      if (poll(fds, 2, (int)(timeout_ms - ms)) < 0 && errno != EINTR) break;
      for (k = 0; k < 2; k++) {
        if (fds[k].fd >= 0 && (fds[k].revents & (POLLIN | POLLHUP))) {
          unsigned char tmp[65536];
          ssize_t r = read(fds[k].fd, tmp, sizeof tmp);
          if (r > 0) {
            bput(k ? &be : &bo, tmp, (size_t)r);
          } else {
            close(fds[k].fd);
            fds[k].fd = -1;
            open_n--;
          }
        }
      }
    }
    if (fds[0].fd >= 0) close(fds[0].fd);
    if (fds[1].fd >= 0) close(fds[1].fd);
  }
  waitpid(pid, &st, 0);
  if (done == 3) {
    res = 3;
    *status = 0;
  } else if (WIFSIGNALED(st)) {
    res = 2;
    *status = WTERMSIG(st);
  } else if (WEXITSTATUS(st) != 0) {
    res = 1;
    *status = WEXITSTATUS(st);
  } else {
    res = 0;
    *status = 0;
  }
  *out = bo.p;
  *outlen = bo.n;
  *err = be.p;
  *errlen = be.n;
  return res;
}

void fm_free(void *p) { free(p); }
