/* SPDX-License-Identifier: Apache-2.0 */
/* The probe's IJG 6b entry points, each run in a forked child (the library's error_exit ends the
 * process with status 1, as in FreeSurfer's programs), its standard output and standard error
 * captured. Compiled once against IJG 6b's release (libjp_up.so) and once against FreeSurfer
 * 8.2.0's libjpeg.a as the unit oracle built it (libjp_fs.so); calls only libjpeg's public API.
 *
 *   jp_decode: the calls of a reader that wants RGB (jpeg_std_error, jpeg_create_decompress,
 *              fopen "rb", jpeg_stdio_src, jpeg_read_header(TRUE), out_color_space = JCS_RGB,
 *              jpeg_start_decompress, jpeg_read_scanlines one row at a time,
 *              jpeg_finish_decompress, jpeg_destroy_decompress); output "ok W H\n", "--\n", the
 *              rows' bytes as they come (top row first);
 *   jp_header: jpeg_read_header(TRUE) only; output "W H\n";
 *   jp_gray:   the compressor of one grey component with jpeg_set_defaults (fopen "wb",
 *              jpeg_stdio_dest, jpeg_start_compress(TRUE), jpeg_write_scanlines one row at a time,
 *              jpeg_finish_compress, fclose);
 *   jp_make:   IJG's compressor with the parameters p[] (for diverse inputs; see main.rs). */
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

#include "jpeglib.h"

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

typedef struct {
  int mode;
  const char *path;
  int w, h, incomp;
  const unsigned char *pix;
  const int *p;
} Job;

static void child_decode(const Job *j, int fd) {
  FILE *infile;
  struct jpeg_decompress_struct cinfo;
  struct jpeg_error_mgr jerr;
  JSAMPROW ptr;
  char line[64];
  cinfo.err = jpeg_std_error(&jerr);
  jpeg_create_decompress(&cinfo);
  if ((infile = fopen(j->path, "rb")) == NULL) _exit(9);
  jpeg_stdio_src(&cinfo, infile);
  jpeg_read_header(&cinfo, TRUE);
  cinfo.out_color_space = JCS_RGB;
  jpeg_start_decompress(&cinfo);
  ptr = malloc((size_t)cinfo.output_width * 3 + 16);
  while (cinfo.output_scanline < cinfo.output_height) {
    jpeg_read_scanlines(&cinfo, &ptr, 1);
    wall(fd, ptr, (size_t)cinfo.output_width * 3);
  }
  jpeg_finish_decompress(&cinfo);
  snprintf(line, sizeof line, "ok %u %u\n", cinfo.image_width, cinfo.image_height);
  jpeg_destroy_decompress(&cinfo);
  wall(fd, line, strlen(line));
}

static void child_header(const Job *j, int fd) {
  FILE *fp;
  struct jpeg_decompress_struct cinfo;
  struct jpeg_error_mgr jerr;
  char line[64];
  cinfo.err = jpeg_std_error(&jerr);
  jpeg_create_decompress(&cinfo);
  if ((fp = fopen(j->path, "rb")) == NULL) _exit(9);
  jpeg_stdio_src(&cinfo, fp);
  jpeg_read_header(&cinfo, TRUE);
  snprintf(line, sizeof line, "%u %u\n", cinfo.image_width, cinfo.image_height);
  jpeg_destroy_decompress(&cinfo);
  wall(fd, line, strlen(line));
}

static void child_gray(const Job *j, int fd) {
  FILE *outf;
  struct jpeg_compress_struct cinfo;
  struct jpeg_error_mgr jerr;
  JSAMPROW ptr;
  (void)fd;
  cinfo.err = jpeg_std_error(&jerr);
  jpeg_create_compress(&cinfo);
  if ((outf = fopen(j->path, "wb")) == NULL) _exit(9);
  jpeg_stdio_dest(&cinfo, outf);
  cinfo.image_width = (JDIMENSION)j->w;
  cinfo.image_height = (JDIMENSION)j->h;
  cinfo.input_components = 1;
  cinfo.in_color_space = JCS_GRAYSCALE;
  jpeg_set_defaults(&cinfo);
  jpeg_start_compress(&cinfo, TRUE);
  while (cinfo.next_scanline < cinfo.image_height) {
    ptr = (JSAMPROW)(j->pix + (size_t)cinfo.next_scanline * (size_t)j->w);
    jpeg_write_scanlines(&cinfo, &ptr, 1);
  }
  jpeg_finish_compress(&cinfo);
  jpeg_destroy_compress(&cinfo);
  fclose(outf);
}

/* p: [0] jpeg colour space (-1: the default for the input), [1] quality, [2] progressive,
 * [3] optimize_coding, [4] restart_interval, [5] restart_in_rows, [6] dct_method, [7] smoothing,
 * [8] write_JFIF_header (-1 default), [9] write_Adobe_marker (-1 default), [10..] (h, v) per
 * component (0: the default), [30] in colour space. */
static void child_make(const Job *j, int fd) {
  FILE *outf;
  struct jpeg_compress_struct cinfo;
  struct jpeg_error_mgr jerr;
  JSAMPROW ptr;
  int ci;
  (void)fd;
  cinfo.err = jpeg_std_error(&jerr);
  jpeg_create_compress(&cinfo);
  if ((outf = fopen(j->path, "wb")) == NULL) _exit(9);
  jpeg_stdio_dest(&cinfo, outf);
  cinfo.image_width = (JDIMENSION)j->w;
  cinfo.image_height = (JDIMENSION)j->h;
  cinfo.input_components = j->incomp;
  cinfo.in_color_space = (J_COLOR_SPACE)j->p[30];
  jpeg_set_defaults(&cinfo);
  if (j->p[0] >= 0) jpeg_set_colorspace(&cinfo, (J_COLOR_SPACE)j->p[0]);
  jpeg_set_quality(&cinfo, j->p[1], FALSE);
  cinfo.optimize_coding = j->p[3] ? TRUE : FALSE;
  cinfo.restart_interval = (unsigned int)j->p[4];
  cinfo.restart_in_rows = j->p[5];
  cinfo.dct_method = (J_DCT_METHOD)j->p[6];
  cinfo.smoothing_factor = j->p[7];
  if (j->p[8] >= 0) cinfo.write_JFIF_header = j->p[8] ? TRUE : FALSE;
  if (j->p[9] >= 0) cinfo.write_Adobe_marker = j->p[9] ? TRUE : FALSE;
  for (ci = 0; ci < cinfo.num_components && ci < 10; ci++) {
    if (j->p[10 + 2 * ci] > 0) {
      cinfo.comp_info[ci].h_samp_factor = j->p[10 + 2 * ci];
      cinfo.comp_info[ci].v_samp_factor = j->p[11 + 2 * ci];
    }
  }
  if (j->p[2]) jpeg_simple_progression(&cinfo);
  jpeg_start_compress(&cinfo, TRUE);
  while (cinfo.next_scanline < cinfo.image_height) {
    ptr = (JSAMPROW)(j->pix + (size_t)cinfo.next_scanline * (size_t)j->w * (size_t)j->incomp);
    jpeg_write_scanlines(&cinfo, &ptr, 1);
  }
  jpeg_finish_compress(&cinfo);
  jpeg_destroy_compress(&cinfo);
  fclose(outf);
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
      case 0: child_decode(job, pd[1]); break;
      case 1: child_header(job, pd[1]); break;
      case 2: child_gray(job, pd[1]); break;
      default: child_make(job, pd[1]); break;
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

int jp_run(int mode, const char *path, int w, int h, int incomp, const unsigned char *pix,
           const int *p, int timeout_ms, int mem_mb, unsigned char **out, size_t *outlen,
           unsigned char **err, size_t *errlen, int *status) {
  Job job = {mode, path, w, h, incomp, pix, p};
  return run(&job, timeout_ms, mem_mb, out, outlen, err, errlen, status);
}

void jp_free(void *p) { free(p); }
