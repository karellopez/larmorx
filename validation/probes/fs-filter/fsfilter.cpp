// SPDX-License-Identifier: Apache-2.0
// fs.filter's probe: FreeSurfer 8.2.0's volume filters (the fast-marching distance maps
// MRIextractDistanceMap and MRIdistanceTransform, ...) called case by case, linked with the unit
// oracle's libutils as the official programs are (build.sh). CLOSED TO FREESURFER CLEAN-ROOM
// IMPLEMENTERS: it names FreeSurfer's functions (running it is a black-box run of FreeSurfer's code).
//
//     fsfilter <cases>     one output line per case line (absolute path; see check.py)
//
// Every case runs in a child process whose standard output and error go to a scratch file beside
// the cases; the parent prints `<result> | <text>`, the text C-escaped (\xHH for bytes outside
// printable ASCII and for the backslash), or `EXIT <status> | <text>` / `CRASH <signal> | <text>`
// when FreeSurfer's code ended the child. Floats in and out are their bits in hex; a voxel token
// `v*k` stands for k voxels of the value v.

#include <fcntl.h>
#include <signal.h>
#include <sys/wait.h>
#include <unistd.h>

#include <cerrno>
#include <cinttypes>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>

#include "diag.h"
#include "error.h"
#include "fastmarching.h"
#include "mri.h"

MRI *MRIstdNonzero(MRI *mri_src, MRI *mri_dst, MRI *mri_mean, int wsize);

const char *Progname = "fsfilter";

namespace {

std::vector<std::string> toks;
size_t pos;
std::string out;
std::string scratch;

[[noreturn]] void fail(const char *what)
{
  fprintf(stderr, "fsfilter: %s\n", what);
  exit(2);
}

const char *next()
{
  if (pos >= toks.size()) fail("missing token");
  return toks[pos++].c_str();
}
int geti() { return (int)strtol(next(), NULL, 10); }
float getf()
{
  uint32_t u = (uint32_t)strtoul(next(), NULL, 16);
  float f;
  memcpy(&f, &u, 4);
  return f;
}

void oi(long long v) { out += std::to_string(v) + " "; }
void of(float f)
{
  uint32_t u;
  memcpy(&u, &f, 4);
  char b[16];
  snprintf(b, sizeof b, "%08x ", u);
  out += b;
}
void od(double d)
{
  uint64_t u;
  memcpy(&u, &d, 8);
  char b[24];
  snprintf(b, sizeof b, "%016" PRIx64 " ", u);
  out += b;
}

// a volume of the case: w h d type nframes xsize ysize zsize, then its stored voxels (every frame;
// floats as their bits, the integer types as decimal numbers; `v*k` for k equal voxels)
MRI *get_vol()
{
  int w = geti(), h = geti(), d = geti(), type = geti(), nf = geti();
  MRI *m = MRIallocSequence(w, h, d, type, nf);
  m->xsize = getf();
  m->ysize = getf();
  m->zsize = getf();
  size_t n = (size_t)w * h * d * nf, i = 0;
  unsigned char *b = (unsigned char *)m->chunk;
  while (i < n) {
    std::string t = next();
    size_t star = t.find('*');
    size_t k = star == std::string::npos ? 1 : strtoul(t.c_str() + star + 1, NULL, 10);
    std::string v = t.substr(0, star);
    for (size_t j = 0; j < k && i < n; j++, i++) {
      switch (type) {
        case MRI_UCHAR: b[i] = (unsigned char)strtol(v.c_str(), NULL, 10); break;
        case MRI_SHORT: { short s = (short)strtol(v.c_str(), NULL, 10); memcpy(b + 2 * i, &s, 2); break; }
        case MRI_USHRT: { unsigned short s = (unsigned short)strtol(v.c_str(), NULL, 10); memcpy(b + 2 * i, &s, 2); break; }
        case MRI_INT: { int s = (int)strtol(v.c_str(), NULL, 10); memcpy(b + 4 * i, &s, 4); break; }
        case MRI_LONG: { long s = strtol(v.c_str(), NULL, 10); memcpy(b + 8 * i, &s, 8); break; }
        case MRI_FLOAT: { uint32_t u = (uint32_t)strtoul(v.c_str(), NULL, 16); memcpy(b + 4 * i, &u, 4); break; }
        default: fail("a voxel type the cases do not write");
      }
    }
  }
  return m;
}

// an optional volume: 0 for none, 1 and a volume
MRI *get_opt() { return geti() ? get_vol() : NULL; }

double getd()
{
  uint64_t u = (uint64_t)strtoull(next(), NULL, 16);
  double d;
  memcpy(&d, &u, 8);
  return d;
}

// a volume with its centre: get_vol's tokens, then c_r c_a c_s (float bits)
MRI *get_volc()
{
  MRI *m = get_vol();
  m->c_r = getf();
  m->c_a = getf();
  m->c_s = getf();
  MRIreInitCache(m);
  return m;
}

// a kernel: its length, then its weights (float bits)
std::vector<float> get_kernel()
{
  int n = geti();
  std::vector<float> k(n > 0 ? n : 0);
  for (int i = 0; i < n; i++) k[i] = getf();
  return k;
}

// the volume: extent, type, frames, outside_val, xsize, then its stored voxels as get_vol reads them
void put_vol(const MRI *m)
{
  if (!m) { out += "NULL "; return; }
  oi(m->width); oi(m->height); oi(m->depth); oi(m->type); oi(m->nframes);
  od(m->outside_val);
  of(m->xsize);
  size_t n = (size_t)m->width * m->height * m->depth * m->nframes;
  const unsigned char *b = (const unsigned char *)m->chunk;
  std::string prev;
  size_t run = 0;
  auto flush = [&]() {
    if (!run) return;
    out += prev;
    if (run > 1) out += "*" + std::to_string(run);
    out += " ";
  };
  for (size_t i = 0; i < n; i++) {
    char t[32];
    switch (m->type) {
      case MRI_UCHAR: snprintf(t, sizeof t, "%d", b[i]); break;
      case MRI_SHORT: { short s; memcpy(&s, b + 2 * i, 2); snprintf(t, sizeof t, "%d", s); break; }
      case MRI_USHRT: { unsigned short s; memcpy(&s, b + 2 * i, 2); snprintf(t, sizeof t, "%d", s); break; }
      case MRI_INT: { int s; memcpy(&s, b + 4 * i, 4); snprintf(t, sizeof t, "%d", s); break; }
      case MRI_LONG: { long s; memcpy(&s, b + 8 * i, 8); snprintf(t, sizeof t, "%ld", s); break; }
      case MRI_FLOAT: { uint32_t u; memcpy(&u, b + 4 * i, 4); snprintf(t, sizeof t, "%08x", u); break; }
      default: fail("a voxel type the cases do not print");
    }
    if (run && prev == t) {
      run++;
    } else {
      flush();
      prev = t;
      run = 1;
    }
  }
  flush();
}

// the header fields fs.filter's functions set: the voxel sizes, direction cosines, centre, start
// and end coordinates, thickness, pixel size (float bits), scale, imnr0, imnr1
void put_hdr(const MRI *m)
{
  if (!m) return;
  for (float f : {m->xsize, m->ysize, m->zsize, m->x_r, m->x_a, m->x_s, m->y_r, m->y_a, m->y_s, m->z_r,
                  m->z_a, m->z_s, m->c_r, m->c_a, m->c_s, m->xstart, m->xend, m->ystart, m->yend,
                  m->zstart, m->zend, m->thick, m->ps})
    of(f);
  oi(m->scale); oi(m->imnr0); oi(m->imnr1);
}

void put_full(const MRI *m)
{
  put_vol(m);
  put_hdr(m);
}

// the destination of a function F(src, ..., dst): 0 none, 1 a given volume, 2 the source itself
MRI *get_dst(int mode, MRI *src) { return mode == 1 ? get_vol() : mode == 2 ? src : NULL; }

// the result r of a call given dst: whether it is dst, then r's volume (NULL when NULL), then dst's
// when one was given and r is another
void put_result(MRI *r, MRI *dst)
{
  oi(r == dst);
  put_full(r);
  if (dst && r != dst) put_full(dst);
}

bool filter_case(const std::string &cmd)
{
  if (cmd == "g1d") {
    float sigma = getf();
    int max_len = geti();
    put_full(MRIgaussian1d(sigma, max_len));
  } else if (cmd == "cv1" || cmd == "cvb" || cmd == "cvs" || cmd == "cvi") {
    // cv1|cvb|cvs|cvi <src> <dst?> <kernel> axis src_frame dst_frame
    MRI *src = get_vol();
    MRI *dst = get_opt();
    std::vector<float> k = get_kernel();
    int axis = geti(), sf = geti(), df = geti();
    float *kp = k.data();
    int len = (int)k.size();
    MRI *r = cmd == "cv1" ? MRIconvolve1d(src, dst, kp, len, axis, sf, df)
           : cmd == "cvb" ? MRIconvolve1dByte(src, dst, kp, len, axis, sf, df)
           : cmd == "cvs" ? MRIconvolve1dShort(src, dst, kp, len, axis, sf, df)
                          : MRIconvolve1dInt(src, dst, kp, len, axis, sf, df);
    put_result(r, dst);
  } else if (cmd == "cvg") {
    // cvg <src> <dst mode> [<dst>] <kernel volume>
    MRI *src = get_vol();
    MRI *dst = get_dst(geti(), src);
    MRI *kv = get_vol();
    MRI *r = MRIconvolveGaussian(src, dst, kv);
    put_result(r, dst);
  } else if (cmd == "gsn" || cmd == "gs") {
    // gsn <src> <dst mode> [<dst>] cstd rstd sstd (double bits); gs <src> <dst mode> [<dst>] std norm
    MRI *src = get_vol();
    MRI *dst = get_dst(geti(), src);
    MRI *r;
    if (cmd == "gsn") {
      double c = getd(), rr = getd(), s = getd();
      r = MRIgaussianSmoothNI(src, c, rr, s, dst);
    } else {
      double s = getd();
      int norm = geti();
      r = MRIgaussianSmooth(src, s, norm, dst);
    }
    put_result(r, dst);
  } else if (cmd == "mgs") {
    // mgs <src> <mask?> <dst mode> [<dst>] std
    MRI *src = get_vol();
    MRI *mask = get_opt();
    MRI *dst = get_dst(geti(), src);
    double s = getd();
    put_result(MRImaskedGaussianSmooth(src, mask, s, dst), dst);
  } else if (cmd == "mean") {
    MRI *src = get_vol();
    MRI *dst = get_opt();
    int wsize = geti();
    put_result(MRImean(src, dst, wsize), dst);
  } else if (cmd == "std" || cmd == "stdnz") {
    MRI *src = get_vol();
    MRI *dst = get_opt();
    MRI *mean = get_vol();
    int wsize = geti();
    MRI *r = cmd == "std" ? MRIstd(src, dst, mean, wsize) : MRIstdNonzero(src, dst, mean, wsize);
    put_result(r, dst);
  } else if (cmd == "red" || cmd == "red2") {
    MRI *src = get_volc();
    MRI *dst = get_opt();
    put_result(cmd == "red" ? MRIreduce(src, dst) : MRIreduce2D(src, dst), dst);
    put_hdr(src);
  } else if (cmd == "r1d" || cmd == "rsl") {
    MRI *src = get_vol();
    MRI *dst = get_opt();
    std::vector<float> k = get_kernel();
    int axis = geti();
    MRI *r = cmd == "r1d" ? MRIreduce1d(src, dst, k.data(), (int)k.size(), axis)
                          : MRIreduceSlice(src, dst, k.data(), (int)k.size(), axis);
    put_result(r, dst);
  } else if (cmd == "xsob" || cmd == "ysob" || cmd == "zsob") {
    MRI *src = get_vol();
    MRI *dst = get_opt();
    int frame = geti();
    MRI *r = cmd == "xsob" ? MRIxSobel(src, dst, frame) : cmd == "ysob" ? MRIySobel(src, dst, frame)
                                                         : MRIzSobel(src, dst, frame);
    put_result(r, dst);
  } else if (cmd == "sob" || cmd == "sobf") {
    MRI *src = get_vol();
    MRI *grad = get_opt();
    MRI *mag = get_opt();
    MRI *r = cmd == "sob" ? MRIsobel(src, grad, mag) : MRIsobelFrame(src, grad, mag, geti());
    put_result(r, grad);
    if (mag) put_full(mag);
  } else if (cmd == "div" || cmd == "lap") {
    MRI *src = get_vol();
    MRI *dst = get_opt();
    put_result(cmd == "div" ? MRIdivergence(src, dst) : MRIlaplacian(src, dst), dst);
  } else {
    return false;
  }
  return true;
}

void run_case()
{
  std::string cmd = next();
  if (filter_case(cmd)) return;
  if (cmd == "edm" || cmd == "dtr") {
    // edm|dtr <src> <dst?> label max_distance mode <mask?>
    MRI *src = get_vol();
    MRI *dst = get_opt();
    int label = geti();
    float maxd = getf();
    int mode = geti();
    MRI *mask = get_opt();
    MRI *r = cmd == "edm" ? MRIextractDistanceMap(src, dst, label, maxd, mode, mask)
                          : MRIdistanceTransform(src, dst, label, maxd, mode, mask);
    oi(r == dst);
    put_vol(r);
  } else if (cmd == "edmf" || cmd == "dtrf") {
    // edmf|dtrf <volume file> label max_distance mode: a recorded head's volume (MRIread), the
    // result's extent, type, frames, outside_val, xsize and the FNV-1a (64 bits) of its voxels
    MRI *src = MRIread(next());
    if (!src) fail("cannot read the volume");
    int label = geti();
    float maxd = getf();
    int mode = geti();
    MRI *r = cmd == "edmf" ? MRIextractDistanceMap(src, NULL, label, maxd, mode, NULL)
                           : MRIdistanceTransform(src, NULL, label, maxd, mode, NULL);
    oi(r->width); oi(r->height); oi(r->depth); oi(r->type); oi(r->nframes);
    od(r->outside_val);
    of(r->xsize);
    uint64_t h = 0xcbf29ce484222325ull;
    const unsigned char *b = (const unsigned char *)r->chunk;
    for (size_t i = 0; i < r->bytes_total; i++) h = (h ^ b[i]) * 0x100000001b3ull;
    char t[24];
    snprintf(t, sizeof t, "%016" PRIx64 " ", h);
    out += t;
  } else {
    fail("unknown command");
  }
}

std::string slurp(const std::string &path)
{
  std::string r;
  if (FILE *f = fopen(path.c_str(), "rb")) {
    int ch;
    while ((ch = fgetc(f)) != EOF) r += (char)ch;
    fclose(f);
  }
  return r;
}

std::string escape(const std::string &s)
{
  std::string r;
  char b[8];
  for (unsigned char c : s) {
    if (c < 0x20 || c > 0x7e || c == '\\') {
      snprintf(b, sizeof b, "\\x%02x", c);
      r += b;
    } else {
      r += (char)c;
    }
  }
  return r;
}

const char *MARK = "\n@@RESULT ";

// the recorded heads' cases run for minutes
bool toks_hint_long(const std::string &line) { return line.compare(0, 5, "edmf ") == 0 || line.compare(0, 5, "dtrf ") == 0; }

}  // namespace

int main(int argc, char *argv[])
{
  if (argc != 2) {
    fprintf(stderr, "usage: fsfilter <cases>\n");
    return 2;
  }
  if (argv[1][0] != '/') fail("the cases must be given by an absolute path");
  scratch = std::string(argv[1]) + ".out";
  FILE *fp = fopen(argv[1], "r");
  if (!fp) fail("cannot open the cases");
  std::vector<std::string> lines;
  static char buf[1 << 22];
  while (fgets(buf, sizeof buf, fp)) lines.push_back(buf);
  fclose(fp);
  for (const std::string &line : lines) {
    if (line.size() >= sizeof buf - 1) fail("a case longer than the buffer");
    fflush(stdout);
    pid_t pid = fork();
    if (pid < 0) fail("fork");
    if (pid == 0) {
      int fd = open(scratch.c_str(), O_WRONLY | O_CREAT | O_TRUNC, 0600);
      if (fd < 0) _exit(3);
      dup2(fd, 1);
      dup2(fd, 2);
      close(fd);
      setvbuf(stdout, NULL, _IONBF, 0);
      alarm(toks_hint_long(line) ? 900 : 20);
      memcpy(buf, line.c_str(), line.size() + 1);
      toks.clear();
      pos = 0;
      for (char *t = strtok(buf, " \t\n"); t; t = strtok(NULL, " \t\n")) toks.push_back(t);
      if (!toks.empty()) {
        out.clear();
        Gerror = NO_ERROR;
        errno = 0;
        run_case();
        if (!out.empty() && out.back() == ' ') out.pop_back();
        fputs(MARK, stdout);
        fputs(out.c_str(), stdout);
      }
      fflush(stdout);
      _exit(0);
    }
    int status = 0;
    waitpid(pid, &status, 0);
    std::string text = slurp(scratch);
    std::string result;
    size_t at = text.rfind(MARK);
    bool clean = WIFEXITED(status) && WEXITSTATUS(status) == 0 && at != std::string::npos;
    if (clean) {
      result = text.substr(at + strlen(MARK));
      text = text.substr(0, at);
    } else if (WIFSIGNALED(status)) {
      result = "CRASH " + std::to_string(WTERMSIG(status));
    } else {
      result = "EXIT " + std::to_string(WEXITSTATUS(status));
    }
    printf("%s | %s\n", result.c_str(), escape(text).c_str());
  }
  unlink(scratch.c_str());
  return 0;
}
