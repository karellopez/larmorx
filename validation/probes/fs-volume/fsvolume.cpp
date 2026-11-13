// SPDX-License-Identifier: Apache-2.0
// fs.volume's probe: FreeSurfer 8.2.0's volume functions (transform.cpp's VOL_GEOM text block,
// copies and matrices; mri.cpp's voxel arithmetic, coordinates and geometry; cma.cpp) called case
// by case, linked with the unit oracle's libutils as the official programs are (build.sh). CLOSED
// TO FREESURFER CLEAN-ROOM IMPLEMENTERS: it names FreeSurfer's functions (running it is a
// black-box run of FreeSurfer's code).
//
//     fsvolume <cases>     one output line per case line (absolute path; see check.py)
//
// Every case runs in a child process whose standard output and error go to a scratch file beside
// the cases; the parent prints `<result> | <text>`, the text C-escaped (\xHH for bytes outside
// printable ASCII and for the backslash), or `EXIT <status> | <text>` / `CRASH <signal> | <text>`
// when FreeSurfer's code ended the child. Floats and doubles in and out are their bits in hex;
// byte strings (files) are hex, `-` for none.

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
#include "matrix.h"
#include "mri.h"
#include "transform.h"
#include "utils.h"

extern double vg_isEqual_Threshold;

const char *Progname = "fsvolume";

namespace {

std::vector<std::string> toks;
size_t pos;
std::string out;
std::string scratch;

[[noreturn]] void fail(const char *what)
{
  fprintf(stderr, "fsvolume: %s\n", what);
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
double getd()
{
  uint64_t u = (uint64_t)strtoull(next(), NULL, 16);
  double d;
  memcpy(&d, &u, 8);
  return d;
}
std::string gets() { std::string s = next(); return s == "-" ? std::string() : s; }
std::string gethex()
{
  std::string h = next(), r;
  if (h == "-") return r;
  for (size_t i = 0; i + 1 < h.size(); i += 2) r += (char)strtoul(h.substr(i, 2).c_str(), NULL, 16);
  return r;
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
void os(const char *s) { out += (s && *s) ? std::string(s) : std::string("-"); out += " "; }
void ohex(const std::string &s)
{
  if (s.empty()) { out += "- "; return; }
  char h[4];
  for (unsigned char c : s) { snprintf(h, sizeof h, "%02x", c); out += h; }
  out += " ";
}

void om(const MATRIX *m)
{
  if (!m) { out += "NULL "; return; }
  oi(m->rows);
  oi(m->cols);
  for (int r = 1; r <= m->rows; r++)
    for (int c = 1; c <= m->cols; c++) of(m->rptr[r][c]);
}

// the geometry fields, in the order the cases give them
void set_geom(VOL_GEOM *v)
{
  v->valid = geti();
  v->width = geti();
  v->height = geti();
  v->depth = geti();
  v->xsize = getf(); v->ysize = getf(); v->zsize = getf();
  v->ras_good_flag = geti();
  v->x_r = getf(); v->x_a = getf(); v->x_s = getf();
  v->y_r = getf(); v->y_a = getf(); v->y_s = getf();
  v->z_r = getf(); v->z_a = getf(); v->z_s = getf();
  v->c_r = getf(); v->c_a = getf(); v->c_s = getf();
  v->s_r = getf(); v->s_a = getf(); v->s_s = getf();
  std::string f = gets();
  strcpy(v->fname, f.c_str());
}
// the fields the VOL_GEOM functions here read or write (not the cached matrices)
void put_geom(const VOL_GEOM *v)
{
  oi(v->valid);
  oi(v->width); oi(v->height); oi(v->depth);
  of(v->xsize); of(v->ysize); of(v->zsize);
  oi(v->ras_good_flag);
  of(v->x_r); of(v->x_a); of(v->x_s);
  of(v->y_r); of(v->y_a); of(v->y_s);
  of(v->z_r); of(v->z_a); of(v->z_s);
  of(v->c_r); of(v->c_a); of(v->c_s);
  of(v->s_r); of(v->s_a); of(v->s_s);
  ohex(std::string(v->fname));
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
void spill(const std::string &path, const std::string &bytes)
{
  FILE *f = fopen(path.c_str(), "wb");
  if (!f) fail("cannot write the case's file");
  fwrite(bytes.data(), 1, bytes.size(), f);
  fclose(f);
}

void run_case()
{
  std::string cmd = next();
  if (cmd == "vgwrite") {
    VOL_GEOM vg;
    set_geom(&vg);
    std::string path = scratch + ".vg";
    FILE *fp = fopen(path.c_str(), "w");
    writeVolGeom(fp, &vg);
    fclose(fp);
    ohex(slurp(path));
  } else if (cmd == "vgread") {
    VOL_GEOM vg;
    set_geom(&vg);
    std::string path = scratch + ".vg";
    spill(path, gethex());
    FILE *fp = fopen(path.c_str(), "r");
    errno = 0;
    readVolGeom(fp, &vg);
    int e = errno;
    put_geom(&vg);
    oi(ftell(fp));
    oi(feof(fp) != 0);
    oi(ferror(fp) != 0);
    oi(e);
    std::string rest;
    int ch;
    while (rest.size() < 64 && (ch = fgetc(fp)) != EOF) rest += (char)ch;
    ohex(rest);
    fclose(fp);
  } else if (cmd == "vgprint0") {
    VOL_GEOM vg;
    set_geom(&vg);
    vg_print(&vg);
    oi(0);
  } else if (cmd == "getvg" || cmd == "copyvg") {
    VOL_GEOM a, b;
    set_geom(&a);
    set_geom(&b);
    if (cmd == "getvg") getVolGeom(&a, &b);
    else copyVolGeom(&a, &b);
    put_geom(&b);
  } else if (cmd == "vgi2r") {
    VOL_GEOM vg;
    set_geom(&vg);
    om(vg_i_to_r(&vg));
  } else if (cmd == "vgr2i") {
    VOL_GEOM vg;
    set_geom(&vg);
    om(vg_r_to_i(&vg));
  } else if (cmd == "tkrv2r") {
    VOL_GEOM vg;
    set_geom(&vg);
    om(TkrVox2RASfromVolGeom(&vg));
  } else if (cmd == "vgeq0") {
    VOL_GEOM a, b;
    set_geom(&a);
    set_geom(&b);
    double thresh = getd();
    vg_isEqual_Threshold = getd();
    oi(vg_isNotEqualThresh(&a, &b, thresh));
    oi(vg_isEqual(&a, &b));
  } else {
    fail("unknown command");
  }
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

}  // namespace

int main(int argc, char *argv[])
{
  if (argc != 2) {
    fprintf(stderr, "usage: fsvolume <cases>\n");
    return 2;
  }
  if (argv[1][0] != '/') fail("the cases must be given by an absolute path");
  scratch = std::string(argv[1]) + ".out";
  FILE *fp = fopen(argv[1], "r");
  if (!fp) fail("cannot open the cases");
  std::vector<std::string> lines;
  static char buf[1 << 20];
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
      alarm(5);
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
  unlink((scratch + ".vg").c_str());
  return 0;
}
