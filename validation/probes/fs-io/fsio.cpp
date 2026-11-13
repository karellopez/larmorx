// SPDX-License-Identifier: Apache-2.0
// fs.io's probe: FreeSurfer 8.2.0's file helpers (fio.cpp's readers and writers of one number on
// C streams and znzlib files, its name helpers and file tests, TSV::read and TSV::write;
// bfileio.cpp's byte swaps; MRIio_old.cpp's lcalloc) called case by case, linked with the unit
// oracle's libutils as the official programs are (build.sh). CLOSED TO FREESURFER CLEAN-ROOM
// IMPLEMENTERS: it names FreeSurfer's functions (running it is a black-box run of FreeSurfer's
// code).
//
//     fsio <cases>     one output line per case line (absolute path; see check.py)
//
// Every case runs in a child process, in a fixture directory made beside the cases (a file `f`,
// a directory `d` with a file `d/i`, an unreadable file `nr`, a read-only directory `ro`, links
// `ln` -> f, `dln` -> d, `bad` -> nowhere); its standard output and error go to a scratch file;
// the parent prints `<result> | <text>`, the text C-escaped (\xHH for bytes outside printable
// ASCII and for the backslash), or `EXIT <status> | <text>` / `CRASH <signal> | <text>` when
// FreeSurfer's code ended the child (each child is limited to 1 GiB of address space and 5
// seconds). Numbers read are printed as their bytes in hex; strings as `N` (NULL) or `x<hex>`.

#include <fcntl.h>
#include <ftw.h>
#include <signal.h>
#include <sys/resource.h>
#include <sys/stat.h>
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

#include "bfileio.h"
#include "diag.h"
#include "error.h"
#include "fio.h"
#include "MRIio_old.h"
#include "znzlib.h"

const char *Progname = "fsio";

namespace {

std::vector<std::string> toks;
size_t pos;
std::string out;
std::string dat;  // the data file's absolute path

[[noreturn]] void fail(const char *what)
{
  fprintf(stderr, "fsio: %s\n", what);
  exit(2);
}

const char *next()
{
  if (pos >= toks.size()) fail("missing token");
  return toks[pos++].c_str();
}
long long geti() { return strtoll(next(), NULL, 10); }
uint32_t getx32() { return (uint32_t)strtoul(next(), NULL, 16); }

std::vector<unsigned char> unhex(const char *s)
{
  std::vector<unsigned char> b;
  if (strcmp(s, "-") == 0) return b;
  for (size_t i = 0; s[i] && s[i + 1]; i += 2) {
    char t[3] = {s[i], s[i + 1], 0};
    b.push_back((unsigned char)strtoul(t, NULL, 16));
  }
  return b;
}
// `N` or `x<hex>`; `isnull` set for N
std::string getstr(bool *isnull)
{
  const char *t = next();
  *isnull = (t[0] == 'N');
  std::string s;
  if (*isnull) return s;
  std::vector<unsigned char> b = unhex(t[1] ? t + 1 : "-");
  s.assign(b.begin(), b.end());
  return s;
}

void oi(long long v) { out += std::to_string(v) + " "; }
void ox(const void *p, size_t n)
{
  // the value's bytes most significant first (x86-64 memory is little-endian)
  const unsigned char *b = (const unsigned char *)p;
  char t[4];
  for (size_t i = n; i-- > 0;) {
    snprintf(t, sizeof t, "%02x", b[i]);
    out += t;
  }
  out += " ";
}
void obytes(const std::vector<unsigned char> &b)
{
  char t[4];
  if (b.empty()) out += "-";
  for (unsigned char c : b) {
    snprintf(t, sizeof t, "%02x", c);
    out += t;
  }
  out += " ";
}
void ostr(const char *s)
{
  if (!s) { out += "N "; return; }
  out += "x";
  obytes(std::vector<unsigned char>(s, s + strlen(s)));
  if (!*s) { out.pop_back(); out.pop_back(); out += " "; }
}

void write_file(const std::vector<unsigned char> &b)
{
  FILE *fp = fopen(dat.c_str(), "wb");
  if (!fp) fail("cannot write the data file");
  if (!b.empty()) fwrite(b.data(), 1, b.size(), fp);
  fclose(fp);
}
std::vector<unsigned char> read_file(const std::string &name)
{
  std::vector<unsigned char> b;
  FILE *fp = fopen(name.c_str(), "rb");
  if (!fp) return b;
  int c;
  while ((c = fgetc(fp)) != EOF) b.push_back((unsigned char)c);
  fclose(fp);
  return b;
}
// the decompressed bytes of a znz file
std::vector<unsigned char> read_znz(const std::string &name, int gz)
{
  std::vector<unsigned char> b;
  znzFile z = znzopen(name.c_str(), "rb", gz);
  if (znz_isnull(z)) return b;
  unsigned char c;
  while (znzread(&c, 1, 1, z) == 1) b.push_back(c);
  znzclose(z);
  return b;
}

void run_case()
{
  std::string cmd = next();
  if (cmd == "rd") {
    std::string fn = next();
    Gdiag_no = (int)geti();
    int count = (int)geti();
    int32_t init = (int32_t)getx32();
    write_file(unhex(next()));
    FILE *fp = fopen(dat.c_str(), "rb");
    if (!fp) fail("cannot open the data file");
    for (int k = 0; k < count; k++) {
      if (fn == "r1") { int v = 0x5a5a5a5a; oi(fread1(&v, fp)); ox(&v, 4); }
      else if (fn == "r2") { int v = 0x5a5a5a5a; oi(fread2(&v, fp)); ox(&v, 4); }
      else if (fn == "r3") { int v = 0x5a5a5a5a; oi(fread3(&v, fp)); ox(&v, 4); }
      else if (fn == "rd") { double d = freadDouble(fp); ox(&d, 8); }
      else if (fn == "ri") { int i = freadInt(fp); ox(&i, 4); }
      else if (fn == "rl") { long long l = freadLong(fp); ox(&l, 8); }
      else if (fn == "rix") { int i = init; oi(freadIntEx(&i, fp)); ox(&i, 4); }
      else if (fn == "rf") { float f = freadFloat(fp); ox(&f, 4); }
      else fail("unknown reader");
    }
    oi(feof(fp) != 0);
    fclose(fp);
  } else if (cmd == "zr") {
    std::string fn = next();
    Gdiag_no = (int)geti();
    int mode = (int)geti();
    int count = (int)geti();
    uint32_t init = getx32();
    std::vector<unsigned char> b = unhex(next());
    if (mode == 1) {
      znzFile w = znzopen(dat.c_str(), "wb", 1);
      if (znz_isnull(w)) fail("cannot write the data file");
      if (!b.empty()) znzwrite(b.data(), 1, b.size(), w);
      znzclose(w);
    } else {
      write_file(b);
    }
    znzFile z = znzopen(dat.c_str(), "rb", mode != 0);
    if (znz_isnull(z)) fail("cannot open the data file");
    for (int k = 0; k < count; k++) {
      if (fn == "z1") { int v = 0x5a5a5a5a; oi(znzread1(&v, z)); ox(&v, 4); }
      else if (fn == "zs") { short s = znzreadShort(z); ox(&s, 2); }
      else if (fn == "zi") { int i = znzreadInt(z); ox(&i, 4); }
      else if (fn == "zl") { long long l = znzreadLong(z); ox(&l, 8); }
      else if (fn == "zf") { float f = znzreadFloat(z); ox(&f, 4); }
      else if (fn == "zd") { double d = znzreadDouble(z); ox(&d, 8); }
      else if (fn == "zix") { int i = (int)init; oi(znzreadIntEx(&i, z)); ox(&i, 4); }
      else if (fn == "zfx") { float f; memcpy(&f, &init, 4); oi(znzreadFloatEx(&f, z)); ox(&f, 4); }
      else fail("unknown reader");
    }
    oi(znzeof(z) != 0);
    znzclose(z);
  } else if (cmd == "wr") {
    std::string fn = next();
    const char *v = next();
    FILE *fp = fopen(dat.c_str(), "wb");
    if (!fp) fail("cannot write the data file");
    int r;
    if (fn == "w2") r = fwrite2((int)strtoll(v, NULL, 10), fp);
    else if (fn == "w3") r = fwrite3((int)strtoll(v, NULL, 10), fp);
    else if (fn == "wi") r = fwriteInt((int)strtoll(v, NULL, 10), fp);
    else if (fn == "wl") r = fwriteLong(strtoll(v, NULL, 10), fp);
    else if (fn == "wf") { uint32_t u = (uint32_t)strtoul(v, NULL, 16); float f; memcpy(&f, &u, 4); r = fwriteFloat(f, fp); }
    else fail("unknown writer");
    fclose(fp);
    oi(r);
    obytes(read_file(dat));
  } else if (cmd == "zw") {
    std::string fn = next();
    int gz = (int)geti();
    const char *v = next();
    znzFile z = znzopen(dat.c_str(), "wb", gz);
    if (znz_isnull(z)) fail("cannot write the data file");
    int r;
    if (fn == "zws") r = znzwriteShort((short)strtoll(v, NULL, 10), z);
    else if (fn == "zwus") r = znzwriteUShort((unsigned short)strtoll(v, NULL, 10), z);
    else if (fn == "zwi") r = znzwriteInt((int)strtoll(v, NULL, 10), z);
    else if (fn == "zwl") r = znzwriteLong(strtoll(v, NULL, 10), z);
    else if (fn == "zwf") { uint32_t u = (uint32_t)strtoul(v, NULL, 16); float f; memcpy(&f, &u, 4); r = znzwriteFloat(f, z); }
    else if (fn == "zwd") { uint64_t u = strtoull(v, NULL, 16); double d; memcpy(&d, &u, 8); r = znzwriteDouble(d, z); }
    else fail("unknown writer");
    znzclose(z);
    oi(r);
    obytes(read_znz(dat, gz));
  } else if (cmd == "dirname") {
    bool n;
    std::string s = getstr(&n);
    char *r = fio_dirname(n ? NULL : s.c_str());
    ostr(r);
    free(r);
  } else if (cmd == "basename") {
    bool n, ne;
    std::string s = getstr(&n);
    std::string e = getstr(&ne);
    char *r = fio_basename(n ? NULL : s.c_str(), ne ? NULL : e.c_str());
    ostr(r);
    free(r);
  } else if (cmd == "ext") {
    bool n;
    std::string s = getstr(&n);
    char *r = fio_extension(n ? NULL : s.c_str());
    ostr(r);
    free(r);
  } else if (cmd == "fsx") {
    std::string fn = next();
    bool n;
    std::string s = getstr(&n);
    int r, e;
    errno = 0;
    if (fn == "exists") { r = fio_FileExistsReadable(s.c_str()); e = errno; }
    else if (fn == "isdir") { r = fio_IsDirectory(s.c_str()); e = errno; }
    else if (fn == "dirw0" || fn == "dirw1") {
      int f = fn == "dirw1";
      r = fio_DirIsWritable(s.c_str(), f);
      e = errno;
      std::string junk = f ? s + ".junk.54_-_sdfklj" : s + "/.junk.54_-_sdfklj";
      struct stat st;
      int left = stat(junk.c_str(), &st) == 0;
      oi(r);
      oi(left);
      r = -1;
    } else fail("unknown test");
    if (r != -1) oi(r);
    oi(e);
  } else if (cmd == "tsv") {
    int debug = (int)geti();
    bool nofmt;
    std::string fmt = getstr(&nofmt);
    const char *content = next();
    unlink("t.tsv");
    if (strcmp(content, "N") != 0) {
      std::vector<unsigned char> b = unhex(content);
      FILE *fp = fopen("t.tsv", "wb");
      if (!fp) fail("cannot write t.tsv");
      if (!b.empty()) fwrite(b.data(), 1, b.size(), fp);
      fclose(fp);
    }
    TSV t;
    t.debug = debug;
    errno = 0;
    oi(t.read("t.tsv"));
    int e = errno;
    oi(t.ncols());
    for (const std::string &h : t.headers) {
      out += "x";
      obytes(std::vector<unsigned char>(h.begin(), h.end()));
      if (h.empty()) { out.pop_back(); out.pop_back(); out += " "; }
    }
    oi(t.nrows());
    for (const std::vector<double> &row : t.data) {
      oi((long long)row.size());
      for (double v : row) ox(&v, 8);
    }
    oi(e);
    if (!nofmt) {
      oi(t.write("o.tsv", fmt));
      obytes(read_file("o.tsv"));
      unlink("o.tsv");
    }
    unlink("t.tsv");
  } else if (cmd == "swap") {
    std::string fn = next();
    long count = (long)geti();
    int buflen = (int)geti();
    std::vector<unsigned char> b = unhex(next());
    // room after the buffer, so that FreeSurfer's writes past it stay in the probe's memory
    std::vector<unsigned char> room(b);
    room.resize(b.size() + 64, 0xee);
    if (fn == "s2") byteswapbufshort(room.data(), count);
    else if (fn == "s4") byteswapbuffloat(room.data(), count);
    else if (fn == "s8") byteswapbufdouble(room.data(), count);
    else fail("unknown swap");
    room.resize(buflen);
    obytes(room);
  } else if (cmd == "calloc") {
    unsigned long long nmemb = strtoull(next(), NULL, 10);
    unsigned long long size = strtoull(next(), NULL, 10);
    char *p = lcalloc((size_t)nmemb, (size_t)size);
    unsigned long long n = nmemb * size;
    int nonzero = 0;
    for (unsigned long long i = 0; i < n && i < 64; i++) nonzero |= p[i] != 0;
    out += "ok ";
    oi(nonzero);
    free(p);
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

int rm_entry(const char *p, const struct stat *, int, struct FTW *)
{
  chmod(p, 0700);
  return remove(p);
}

void make_fixture(const std::string &fix)
{
  nftw(fix.c_str(), rm_entry, 16, FTW_DEPTH | FTW_PHYS);
  if (mkdir(fix.c_str(), 0700)) fail("cannot make the fixture");
  auto file = [&](const std::string &n, const char *text) {
    FILE *f = fopen((fix + "/" + n).c_str(), "w");
    if (!f) fail("cannot write a fixture file");
    fputs(text, f);
    fclose(f);
  };
  file("f", "abc\n");
  mkdir((fix + "/d").c_str(), 0700);
  file("d/i", "x");
  file("nr", "secret");
  chmod((fix + "/nr").c_str(), 0);
  mkdir((fix + "/ro").c_str(), 0500);
  if (symlink("f", (fix + "/ln").c_str()) || symlink("d", (fix + "/dln").c_str()) ||
      symlink("nowhere", (fix + "/bad").c_str()))
    fail("cannot make the fixture's links");
}

}  // namespace

int main(int argc, char *argv[])
{
  if (argc != 2) {
    fprintf(stderr, "usage: fsio <cases>\n");
    return 2;
  }
  if (argv[1][0] != '/') fail("the cases must be given by an absolute path");
  std::string scratch = std::string(argv[1]) + ".out";
  dat = std::string(argv[1]) + ".dat";
  std::string fix = std::string(argv[1]) + ".fix";
  make_fixture(fix);
  FILE *fp = fopen(argv[1], "r");
  if (!fp) fail("cannot open the cases");
  std::vector<std::string> lines;
  static char buf[1 << 20];
  while (fgets(buf, sizeof buf, fp)) lines.push_back(buf);
  fclose(fp);
  for (const std::string &line : lines) {
    if (line.size() >= sizeof buf) fail("a case longer than the buffer");
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
      if (chdir(fix.c_str())) _exit(4);
      // a case that reads for ever (TSV::read, FS-172) stops at 1 GiB of memory or 5 seconds
      struct rlimit rl = {1ul << 30, 1ul << 30};
      setrlimit(RLIMIT_AS, &rl);
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
    std::string text;
    if (FILE *r = fopen(scratch.c_str(), "rb")) {
      int ch;
      while ((ch = fgetc(r)) != EOF) text += (char)ch;
      fclose(r);
    }
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
  unlink(dat.c_str());
  nftw(fix.c_str(), rm_entry, 16, FTW_DEPTH | FTW_PHYS);
  return 0;
}
