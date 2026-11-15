/* SPDX-License-Identifier: Apache-2.0 */
/* The probe's MINC entry point: the volume input sequence FreeSurfer 8.2.0's mincRead runs
 * (start_volume_input with the dimension names xspace, yspace, zspace, time, the file's own type,
 * no voxel range, a new volume, no options; input_more_of_volume until done; every voxel value),
 * in a forked child. Its results are written as a dump on a pipe (every double's bits), its
 * standard output and standard error captured. Compiled once against MINC 1.5.1's volume_io
 * header (libminc_up.so) and once against FreeSurfer 8.2.0's packages/minc header (libminc_fs.so);
 * calls only volume_io's public names and reads the volume's public fields. */
#include <errno.h>
#include <poll.h>
#include <signal.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#ifdef UPSTREAM
#include <internal_volume_io.h>
#include <minc.h>
#else
#include "minc_volume_io.h"
#endif

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
static void bnum(Buf *b, const char *fmt, ...) __attribute__((format(printf, 2, 3)));
static void bnum(Buf *b, const char *fmt, ...) {
  char t[256];
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
static void bcs(Buf *b, const char *s) { /* " -" or " s<hex>" */
  static const char H[] = "0123456789abcdef";
  size_t i;
  if (!s) {
    bstr(b, " -");
    return;
  }
  bstr(b, " s");
  for (i = 0; s[i]; i++) {
    char c[2] = {H[(unsigned char)s[i] >> 4], H[(unsigned char)s[i] & 15]};
    bput(b, c, 2);
  }
}

static void child(const char *fname, int fd) {
  Buf b = {0};
  VIO_Status status;
  Volume vol = NULL;
  volume_input_struct info;
  char *dim_names[4];
  int i, n, sizes[5], idx[5], total;
  double f;
  char *tmp;
  dim_names[0] = MIxspace;
  dim_names[1] = MIyspace;
  dim_names[2] = MIzspace;
  dim_names[3] = MItime;
  tmp = strdup(fname);
  status = start_volume_input(tmp, 0, dim_names, NC_UNSPECIFIED, 0, 0, 0, 1, &vol, NULL, &info);
  free(tmp);
  bnum(&b, "status %d\n", (int)status);
  if (status == OK) {
    double vmin, vmax, rmin, rmax, sep[5], voxel[5], wx, wy, wz;
    General_transform *g;
    char *space;
    n = get_volume_n_dimensions(vol);
    get_volume_sizes(vol, sizes);
#ifdef UPSTREAM
    /* FreeSurfer's copy has no volume cache (its volume has no is_cached_volume). */
    bnum(&b, "ndims %d type %d signed %d cached %d rgba %d\n", n, (int)vol->nc_data_type,
         (int)vol->signed_flag, (int)vol->is_cached_volume, (int)vol->is_rgba_data);
#else
    bnum(&b, "ndims %d type %d signed %d cached %d rgba %d\n", n, (int)vol->nc_data_type,
         (int)vol->signed_flag, 0, (int)vol->is_rgba_data);
#endif
    bstr(&b, "names");
    for (i = 0; i < n; i++) bcs(&b, vol->dimension_names[i]);
    bstr(&b, "\nsizes");
    for (i = 0; i < n; i++) bnum(&b, " %d", sizes[i]);
    get_volume_separations(vol, sep);
    bstr(&b, "\nsep");
    for (i = 0; i < n; i++) bdbl(&b, sep[i]);
    bstr(&b, "\nstarts");
    for (i = 0; i < n; i++) bdbl(&b, vol->starts[i]);
    bstr(&b, "\ndircos");
    for (i = 0; i < n; i++) {
      bdbl(&b, vol->direction_cosines[i][0]);
      bdbl(&b, vol->direction_cosines[i][1]);
      bdbl(&b, vol->direction_cosines[i][2]);
    }
    bnum(&b, "\nspatial %d %d %d\n", vol->spatial_axes[0], vol->spatial_axes[1],
         vol->spatial_axes[2]);
    get_volume_voxel_range(vol, &vmin, &vmax);
    get_volume_real_range(vol, &rmin, &rmax);
    bstr(&b, "vrange");
    bdbl(&b, vmin);
    bdbl(&b, vmax);
    bstr(&b, "\nrrange");
    bdbl(&b, rmin);
    bdbl(&b, rmax);
    bstr(&b, "\nscale");
    bdbl(&b, vol->real_value_scale);
    bdbl(&b, vol->real_value_translation);
    space = vol->coordinate_system_name;
    bstr(&b, "\nspace");
    bcs(&b, space);
    g = get_voxel_to_world_transform(vol);
    bnum(&b, "\nxform %d", (int)g->type);
    if (g->type == LINEAR) {
      Transform *t = get_linear_transform_ptr(g);
      int r, c;
      for (r = 0; r < 4; r++)
        for (c = 0; c < 4; c++) bdbl(&b, Transform_elem(*t, r, c));
    }
    for (i = 0; i < 5; i++) voxel[i] = 0.0;
    if (n >= 3) {
      voxel[0] = sizes[0] / 2.0;
      voxel[1] = sizes[1] / 2.0;
      voxel[2] = sizes[2] / 2.0;
    }
    convert_voxel_to_world(vol, voxel, &wx, &wy, &wz);
    bstr(&b, "\nworld_c");
    bdbl(&b, wx);
    bdbl(&b, wy);
    bdbl(&b, wz);
    for (i = 0; i < 5; i++) voxel[i] = 0.0;
    convert_voxel_to_world(vol, voxel, &wx, &wy, &wz);
    bstr(&b, "\nworld_0");
    bdbl(&b, wx);
    bdbl(&b, wy);
    bdbl(&b, wz);
    bstr(&b, "\nmore");
    {
      int k = 0;
      while (input_more_of_volume(vol, &info, &f) && k < 100000) {
        bdbl(&b, f);
        k++;
      }
      bdbl(&b, f);
    }
#ifdef UPSTREAM
    /* volume_io's cache is decided when the data are allocated (at the first read). */
    bnum(&b, "\ncached_after %d", (int)vol->is_cached_volume);
#else
    bstr(&b, "\ncached_after 0");
#endif
    bstr(&b, "\ndata");
    total = 1;
    for (i = 0; i < n; i++) total *= sizes[i];
    for (i = 0; i < 5; i++) idx[i] = 0;
    if (n >= 1 && total > 0 && total <= (1 << 22)) {
      for (;;) {
        bdbl(&b, get_volume_voxel_value(vol, idx[0], idx[1], idx[2], idx[3], idx[4]));
        for (i = n - 1; i >= 0; i--) {
          if (++idx[i] < sizes[i]) break;
          idx[i] = 0;
        }
        if (i < 0) break;
      }
    }
    bstr(&b, "\n");
    delete_volume_input(&info);
    delete_volume(vol);
  }
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

/* Runs fn(arg, 200) in a forked child whose descriptors 3 and up are closed (so that the first
 * file it opens is 3); its dump pipe is descriptor 200. The outcome: 0 the child returned (dump in
 * *out), 1 it exited with *status, 2 it died of signal *status, 3 it ran past timeout_ms (killed).
 * *sout and *serr hold its standard output and error. cache_threshold >= 0 sets
 * VOLUME_CACHE_THRESHOLD in the child. */
static int run_child(void (*fn)(void *, int), void *arg, int timeout_ms, int cache_threshold,
                     unsigned char **out, size_t *outlen, unsigned char **sout, size_t *soutlen,
                     unsigned char **serr, size_t *serrlen, int *status) {
  int pd[2], po[2], pe[2], st = 0, done = 0, res;
  pid_t pid;
  Buf bb[3] = {{0}, {0}, {0}};
  struct timespec t0, t1;
  if (pipe(pd) || pipe(po) || pipe(pe)) return -1;
  fflush(NULL);
  pid = fork();
  if (pid == 0) {
    int k;
    dup2(pd[1], 200);
    dup2(po[1], 1);
    dup2(pe[1], 2);
    for (k = 3; k < 200; k++) close(k);
    if (cache_threshold >= 0) {
      char v[32];
      snprintf(v, sizeof v, "%d", cache_threshold);
      setenv("VOLUME_CACHE_THRESHOLD", v, 1);
    } else {
      unsetenv("VOLUME_CACHE_THRESHOLD");
    }
    fn(arg, 200);
  }
  close(pd[1]);
  close(po[1]);
  close(pe[1]);
  clock_gettime(CLOCK_MONOTONIC, &t0);
  {
    struct pollfd fds[3] = {{pd[0], POLLIN, 0}, {po[0], POLLIN, 0}, {pe[0], POLLIN, 0}};
    int open_n = 3;
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
      if (poll(fds, 3, (int)(timeout_ms - ms)) < 0 && errno != EINTR) break;
      for (k = 0; k < 3; k++) {
        if (fds[k].fd >= 0 && (fds[k].revents & (POLLIN | POLLHUP))) {
          unsigned char tmp[65536];
          ssize_t r = read(fds[k].fd, tmp, sizeof tmp);
          if (r > 0) {
            bput(&bb[k], tmp, (size_t)r);
          } else {
            close(fds[k].fd);
            fds[k].fd = -1;
            open_n--;
          }
        }
      }
    }
    {
      int k;
      for (k = 0; k < 3; k++)
        if (fds[k].fd >= 0) close(fds[k].fd);
    }
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
  *out = bb[0].p;
  *outlen = bb[0].n;
  *sout = bb[1].p;
  *soutlen = bb[1].n;
  *serr = bb[2].p;
  *serrlen = bb[2].n;
  return res;
}

static void load_child(void *arg, int fd) { child((const char *)arg, fd); }

/* mincRead's sequence on fname (above), in a forked child. */
int fm_minc_load(const char *fname, int timeout_ms, int cache_threshold, unsigned char **out,
                 size_t *outlen, unsigned char **sout, size_t *soutlen, unsigned char **serr,
                 size_t *serrlen, int *status) {
  return run_child(load_child, (void *)fname, timeout_ms, cache_threshold, out, outlen, sout,
                   soutlen, serr, serrlen, status);
}

/* A write case (the probe's src/minc_write.rs has the same layout): the volume_io sequence of
 * FreeSurfer 8.2.0's mincWrite with every argument chosen by the probe. create_volume(n_dims,
 * names or NULL, nc_type, signed_flag, vmin, vmax); optionally set_volume_real_range;
 * set_volume_sizes; alloc_volume_data (do_alloc); set_volume_separations;
 * set_volume_direction_cosine for each listed axis (or set_volume_direction_unit_cosine when
 * cos_unit); set_volume_translation (do_translation); set_volume_voxel_value of every voxel in
 * C order with values[k % n_values]; output_volume(fname, out_type, out_signed, out_min,
 * out_max, volume, history, NULL); delete_volume. */
typedef struct {
  int n_dims;
  int names_null;
  const char *names[5];
  int nc_type;
  int signed_flag;
  double vmin, vmax;
  int set_real;
  double rmin, rmax;
  int sizes[5];
  int do_alloc;
  double sep[5];
  int n_cos;
  int cos_unit;
  int cos_axis[5];
  double cos[5][3];
  int do_translation;
  double tvoxel[5];
  double tworld[3];
  long n_values;
  const double *values;
  const char *fname;
  int out_type;
  int out_signed;
  double out_min, out_max;
  const char *history;
  int set_space;
  const char *space;
} WriteCase;

/* A line of the write child's dump, written at once (a later exit keeps what came before). */
static void wline(int fd, const char *fmt, ...) __attribute__((format(printf, 2, 3)));
static void wline(int fd, const char *fmt, ...) {
  char t[256];
  size_t at = 0, n;
  va_list ap;
  va_start(ap, fmt);
  vsnprintf(t, sizeof t, fmt, ap);
  va_end(ap);
  n = strlen(t);
  fflush(stdout);
  fflush(stderr);
  while (at < n) {
    ssize_t wr = write(fd, t + at, n - at);
    if (wr <= 0) break;
    at += (size_t)wr;
  }
}

static void write_child(void *arg, int fd) {
  const WriteCase *w = (const WriteCase *)arg;
  VIO_Status status;
  Volume vol;
  char *names[5];
  int i, idx[5], sizes[5];
  long k = 0, total = 1;
  double sep[5], dir[3], tv[5], tw[3];
  for (i = 0; i < 5; i++) names[i] = w->names[i] ? strdup(w->names[i]) : NULL;
  vol = create_volume(w->n_dims, w->names_null ? NULL : names, (nc_type)w->nc_type,
                      w->signed_flag, w->vmin, w->vmax);
  wline(fd, "created %d\n", vol != NULL);
  if (vol != NULL) {
    if (w->set_space) set_volume_space_type(vol, (char *)w->space);
    if (w->set_real) set_volume_real_range(vol, w->rmin, w->rmax);
    for (i = 0; i < 5; i++) sizes[i] = w->sizes[i];
    set_volume_sizes(vol, sizes);
    if (w->do_alloc) alloc_volume_data(vol);
    wline(fd, "allocated\n");
    for (i = 0; i < 5; i++) sep[i] = w->sep[i];
    set_volume_separations(vol, sep);
    for (i = 0; i < w->n_cos; i++) {
      dir[0] = w->cos[i][0];
      dir[1] = w->cos[i][1];
      dir[2] = w->cos[i][2];
      if (w->cos_unit)
        set_volume_direction_unit_cosine(vol, w->cos_axis[i], dir);
      else
        set_volume_direction_cosine(vol, w->cos_axis[i], dir);
    }
    wline(fd, "cosines\n");
    if (w->do_translation) {
      for (i = 0; i < 5; i++) tv[i] = w->tvoxel[i];
      for (i = 0; i < 3; i++) tw[i] = w->tworld[i];
      set_volume_translation(vol, tv, tw);
    }
    wline(fd, "translated\n");
    if (w->do_alloc && w->n_values > 0) {
      for (i = 0; i < w->n_dims && i < 5; i++) total *= w->sizes[i];
      for (i = 0; i < 5; i++) idx[i] = 0;
      if (w->n_dims >= 1 && total > 0) {
        for (;;) {
          set_volume_voxel_value(vol, idx[0], idx[1], idx[2], idx[3], idx[4],
                                 w->values[k % w->n_values]);
          k++;
          for (i = w->n_dims - 1; i >= 0; i--) {
            if (++idx[i] < w->sizes[i]) break;
            idx[i] = 0;
          }
          if (i < 0) break;
        }
      }
    }
    wline(fd, "set\n");
    {
      char *fname = strdup(w->fname);
      char *hist = w->history ? strdup(w->history) : NULL;
      status = output_volume(fname, (nc_type)w->out_type, w->out_signed, w->out_min,
                             w->out_max, vol, hist, NULL);
      wline(fd, "status %d\n", (int)status);
    }
    delete_volume(vol);
  }
  fflush(stdout);
  fflush(stderr);
  _exit(0);
}

/* The write case w in a forked child (outcomes as fm_minc_load's). */
int fm_minc_write(const WriteCase *w, int timeout_ms, unsigned char **out, size_t *outlen,
                  unsigned char **sout, size_t *soutlen, unsigned char **serr, size_t *serrlen,
                  int *status) {
  return run_child(write_child, (void *)w, timeout_ms, -1, out, outlen, sout, soutlen, serr,
                   serrlen, status);
}

void fm_minc_free(void *p) { free(p); }
