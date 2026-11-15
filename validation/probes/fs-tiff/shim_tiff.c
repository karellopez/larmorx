/* SPDX-License-Identifier: Apache-2.0 */
/* The probe's libtiff 3.6.1 entry points, each run in a forked child (a crash or a hang of the
 * library is seen as the child's signal or timeout), its standard output and standard error
 * captured. Compiled once against libtiff 3.6.1's release (libtf_up.so) and once against
 * FreeSurfer 8.2.0's libtiff.a as the unit oracle built it (libtf_fs.so); calls only libtiff's
 * public API, the transcripts of larmorx_io::tiff::cases::port_read and port_write.
 *
 *   tf_read:  TIFFOpen "r", TIFFNumberOfDirectories, for each of the first six directories
 *             TIFFSetDirectory, TIFFGetFieldDefaulted of the queried tags, TIFFGetField of the
 *             tile size, TIFFIsTiled, TIFFScanlineSize, TIFFTileSize, every scanline (or tile)
 *             of up to five samples; TIFFSetDirectory(0) and TIFFReadDirectory up to 8 times;
 *             TIFFClose;
 *   tf_write: FreeSurfer's TiffWriteImage: TIFFOpen "w", its TIFFSetField calls,
 *             TIFFWriteScanline per row, TIFFWriteDirectory, TIFFClose. */
#include <errno.h>
#include <poll.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#include "tiffio.h"

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

static void wall(int fd, const void *d, size_t n) {
  size_t at = 0;
  while (at < n) {
    ssize_t w = write(fd, (const char *)d + at, n - at);
    if (w <= 0) return;
    at += (size_t)w;
  }
}

static int g_fd;
static void say(const char *fmt, ...) __attribute__((format(printf, 1, 2)));
#include <stdarg.h>
static void say(const char *fmt, ...) {
  char line[512];
  va_list ap;
  va_start(ap, fmt);
  vsnprintf(line, sizeof line, fmt, ap);
  va_end(ap);
  wall(g_fd, line, strlen(line));
}

typedef struct {
  int mode;
  const char *path;
  /* writing: cols rows spp bps sample_format compression; xres, yres; the rows */
  const int *p;
  const double *d;
  const unsigned char *data;
} Job;

static unsigned long long fnv(const unsigned char *b, size_t n) {
  unsigned long long h = 0xcbf29ce484222325ULL;
  size_t i;
  for (i = 0; i < n; i++) h = (h ^ b[i]) * 0x100000001b3ULL;
  return h;
}

static const struct { ttag_t tag; char ty; } QUERIED[] = {
  {256, 'L'}, {257, 'L'}, {277, 'S'}, {258, 'S'}, {262, 'S'}, {266, 'S'}, {259, 'S'},
  {296, 'S'}, {282, 'F'}, {283, 'F'}, {274, 'S'}, {284, 'S'}, {278, 'L'}, {339, 'S'},
  {280, 'S'}, {281, 'S'}, {254, 'L'}, {263, 'S'}, {32997, 'L'}, {32998, 'L'}, {32995, 'S'},
  {32996, 'S'}, {530, 'P'}, {297, 'P'}, {338, 'X'},
};

static void fields_text(TIFF *t) {
  size_t k;
  for (k = 0; k < sizeof QUERIED / sizeof QUERIED[0]; k++) {
    ttag_t tg = QUERIED[k].tag;
    int r;
    switch (QUERIED[k].ty) {
      case 'L': { uint32 v = 0; r = TIFFGetFieldDefaulted(t, tg, &v);
        if (r) say(" f %u 1 %u\n", tg, v); else say(" f %u 0\n", tg); break; }
      case 'S': { uint16 v = 0; r = TIFFGetFieldDefaulted(t, tg, &v);
        if (r) say(" f %u 1 %u\n", tg, v); else say(" f %u 0\n", tg); break; }
      case 'F': { float v = 0; r = TIFFGetFieldDefaulted(t, tg, &v);
        if (r) say(" f %u 1 %.9g\n", tg, (double)v); else say(" f %u 0\n", tg); break; }
      case 'P': { uint16 a = 0, b = 0; r = TIFFGetFieldDefaulted(t, tg, &a, &b);
        if (r) say(" f %u 1 %u %u\n", tg, a, b); else say(" f %u 0\n", tg); break; }
      default: { uint16 n = 0; uint16 *v = NULL; r = TIFFGetFieldDefaulted(t, tg, &n, &v);
        if (r) {
          char line[400]; int at, i;
          at = snprintf(line, sizeof line, " f %u 1 %u", tg, n);
          for (i = 0; i < n && v && at < 360; i++) at += snprintf(line + at, sizeof line - at, " %u", v[i]);
          say("%s\n", line);
        } else say(" f %u 0\n", tg);
        break; }
    }
  }
  {
    uint32 v = 0;
    int r = TIFFGetField(t, TIFFTAG_TILEWIDTH, &v);
    if (r) say(" g %u 1 %u\n", TIFFTAG_TILEWIDTH, v); else say(" g %u 0\n", TIFFTAG_TILEWIDTH);
    v = 0;
    r = TIFFGetField(t, TIFFTAG_TILELENGTH, &v);
    if (r) say(" g %u 1 %u\n", TIFFTAG_TILELENGTH, v); else say(" g %u 0\n", TIFFTAG_TILELENGTH);
  }
}

static void data_text(TIFF *t) {
  int tiled = TIFFIsTiled(t);
  tsize_t sls = TIFFScanlineSize(t), ts = TIFFTileSize(t), size;
  uint32 length = 0, width = 0, tw = 0, th = 0;
  uint16 spp = 0, planar = 0;
  unsigned nsamp, sample;
  unsigned char *buf;
  unsigned reads = 0;
  say(" tiled %d sls %d ts %d\n", tiled ? 1 : 0, (int)sls, (int)ts);
  size = tiled ? ts : sls;
  if (size <= 0 || size > (16 << 20)) { say(" skip\n"); return; }
  TIFFGetFieldDefaulted(t, TIFFTAG_IMAGELENGTH, &length);
  TIFFGetFieldDefaulted(t, TIFFTAG_IMAGEWIDTH, &width);
  TIFFGetFieldDefaulted(t, TIFFTAG_SAMPLESPERPIXEL, &spp);
  TIFFGetFieldDefaulted(t, TIFFTAG_PLANARCONFIG, &planar);
  nsamp = planar == 2 ? (spp < 1 ? 1 : spp > 5 ? 5 : spp) : 1;
  buf = malloc((size_t)size);
  if (!tiled) {
    for (sample = 0; sample < nsamp; sample++) {
      uint32 row;
      for (row = 0; row < length && row < 400; row++) {
        int r;
        memset(buf, 0xab, (size_t)size);
        r = TIFFReadScanline(t, buf, row, (tsample_t)sample);
        say(" r %u %u %d %016llx\n", row, sample, r, fnv(buf, (size_t)size));
      }
    }
  } else {
    TIFFGetField(t, TIFFTAG_TILEWIDTH, &tw);
    TIFFGetField(t, TIFFTAG_TILELENGTH, &th);
    if (tw < 1) tw = 1;
    if (th < 1) th = 1;
    for (sample = 0; sample < nsamp && reads < 200; sample++) {
      uint32 y, x;
      for (y = 0; y < length && reads < 200; y = (y > 0xffffffffu - th) ? 0xffffffffu : y + th) {
        for (x = 0; x < width && reads < 200; x = (x > 0xffffffffu - tw) ? 0xffffffffu : x + tw) {
          tsize_t r;
          memset(buf, 0xab, (size_t)size);
          r = TIFFReadTile(t, buf, x, y, 0, (tsample_t)sample);
          say(" t %u %u %u %d %016llx\n", x, y, sample, (int)r, fnv(buf, (size_t)size));
          reads++;
        }
      }
    }
  }
  free(buf);
}

static void child_read(const Job *j, int fd) {
  TIFF *t;
  tdir_t n, d;
  int k;
  g_fd = fd;
  t = TIFFOpen(j->path, "r");
  if (!t) { say("open 0\n"); return; }
  say("open 1\n");
  n = TIFFNumberOfDirectories(t);
  say("ndirs %u\n", (unsigned)n);
  for (d = 0; d < n && d < 6; d++) {
    int r = TIFFSetDirectory(t, d);
    say("setdir %u %d\n", (unsigned)d, r ? 1 : 0);
    fields_text(t);
    data_text(t);
  }
  {
    int r = TIFFSetDirectory(t, 0);
    say("setdir 0 %d\n", r ? 1 : 0);
  }
  for (k = 0; k < 8; k++) {
    int r = TIFFReadDirectory(t);
    say("readdir %d\n", r ? 1 : 0);
    if (!r) break;
  }
  TIFFClose(t);
  say("close\n");
}

static void child_write(const Job *j, int fd) {
  TIFF *t;
  uint32 cols = (uint32)j->p[0], rows = (uint32)j->p[1], k, bad = 0;
  int spp = j->p[2], bps = j->p[3], sf = j->p[4], comp = j->p[5];
  size_t row = ((size_t)cols * (size_t)bps * (size_t)spp + 7) / 8;
  unsigned char *b;
  int r;
  g_fd = fd;
  t = TIFFOpen(j->path, "w");
  if (!t) { say("open 0\n"); return; }
  say("open 1\n");
#define SET(tg, v) do { r = TIFFSetField(t, tg, v); say("set %u %d\n", (unsigned)(tg), r ? 1 : 0); } while (0)
  SET(TIFFTAG_SAMPLEFORMAT, sf);
  SET(TIFFTAG_IMAGEWIDTH, cols);
  SET(TIFFTAG_IMAGELENGTH, rows);
  SET(TIFFTAG_ORIENTATION, ORIENTATION_BOTLEFT);
  SET(TIFFTAG_SAMPLESPERPIXEL, spp);
  SET(TIFFTAG_SAMPLEFORMAT, SAMPLEFORMAT_INT);
  SET(TIFFTAG_BITSPERSAMPLE, bps);
  SET(TIFFTAG_PLANARCONFIG, PLANARCONFIG_CONTIG);
  SET(TIFFTAG_PHOTOMETRIC, PHOTOMETRIC_MINISBLACK);
  SET(TIFFTAG_RESOLUTIONUNIT, RESUNIT_INCH);
  if (j->d[0] != 0) SET(TIFFTAG_XRESOLUTION, j->d[0]);
  if (j->d[1] != 0) SET(TIFFTAG_YRESOLUTION, j->d[1]);
  SET(TIFFTAG_COMPRESSION, comp);
  b = malloc(row + 64);
  for (k = 0; k < rows; k++) {
    memcpy(b, j->data + (size_t)k * row, row);
    memset(b + row, 0, 64);
    r = TIFFWriteScanline(t, b, k, 0);
    if (r != 1) {
      bad++;
      if (bad <= 3) say("row %u %d\n", k, r);
    }
  }
  say("rows %u bad %u\n", rows, bad);
  r = TIFFWriteDirectory(t);
  say("writedir %d\n", r ? 1 : 0);
  TIFFClose(t);
  say("close\n");
  free(b);
}

/* The outcome: 0 the child ended normally, 1 it exited with *status, 2 it died of signal
 * *status, 3 it ran past timeout_ms (killed). *out and *err hold its two streams. Each child's
 * address space is capped at mem_mb MiB (0: no cap). */
static int run(const Job *job, int timeout_ms, int mem_mb, unsigned char **out, size_t *outlen,
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
    if (mem_mb > 0) {
      struct rlimit rl;
      rl.rlim_cur = rl.rlim_max = (rlim_t)mem_mb << 20;
      setrlimit(RLIMIT_AS, &rl);
    }
    switch (job->mode) {
      case 0: child_read(job, pd[1]); break;
      default: child_write(job, pd[1]); break;
    }
    fflush(stdout);
    fflush(stderr);
    _exit(0);
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

int tf_run(int mode, const char *path, const int *p, const double *d, const unsigned char *data,
           int timeout_ms, int mem_mb, unsigned char **out, size_t *outlen, unsigned char **err,
           size_t *errlen, int *status) {
  Job job;
  job.mode = mode;
  job.path = path;
  job.p = p;
  job.d = d;
  job.data = data;
  return run(&job, timeout_ms, mem_mb, out, outlen, err, errlen, status);
}

void tf_free(void *p) { free(p); }
