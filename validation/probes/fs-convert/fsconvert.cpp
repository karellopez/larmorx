// SPDX-License-Identifier: Apache-2.0
// fs-convert's probe: FreeSurfer 8.2.0's mri_convert (mri_convert.cpp with its main renamed and the
// two print hooks of build.sh, compiled into this file) and the stats-table functions of the unit
// oracle's libutils (stats.cpp, with csv.h compiled here in a namespace of its own), called case by
// case, linked as the official program is (build.sh). CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS:
// it names FreeSurfer's functions and variables (running it is a black-box run of FreeSurfer's code).
//
//     fsconvert <cases>     one output line per case line (absolute path; see check.py)
//
// Every case runs in a child process in the folder <cases>.d (emptied first), its standard output
// (unbuffered) and error to one scratch file; the parent prints `<result> | <text>`, the text
// C-escaped, or `EXIT <status> | <text>` / `CRASH <signal> | <text>`, the result followed by the
// files the case wrote (names not starting with `in_`, sorted): `F <name> <FNV-1a>`, a `.mgz` read
// back and printed. Floats in and out are their bits in hex; a voxel token `v*k` stands for k
// voxels of the value v; a file's contents and an argument are hex (an argument `@` is empty).
//
// Commands:
//   csv <hex>                      readCSV of the file: rows, each row's fields
//   lst <name> <hex|->             LoadStatTable(name), the file written first unless `-`
//   ist <vol> <name|-|@> <hex|->   InitStatTableFromMRI(vol, name; `-` NULL, `@` "")
//   wst <vol> <name|-|@> <hex|-> <out>   the same, then WriteStatTable(out, table) when there is one
//   opt <nfiles> [<name> <hex>]... <nargs> <arg>...   mri_convert's main with those arguments: its
//       output up to the dump hook, which prints every variable its options set (build.sh)
//   cnv <nfiles> [<name> <hex>]... <nargs> <arg>...   mri_convert's main with those arguments, run
//       to its end (the dump hook off; its getAllInfo text with the time, user, machine and kernel
//       version as `X`, probe::canon_info): `EXIT <status>`, then every file written as
//       `F <name> <bytes> <FNV-1a>` of its contents read through zlib's gzread (a compressed file
//       decompressed), each command line's text from `TimeStamp: ` to its NUL masked with `X`

#include <climits>
#include <cstdint>
#include <cstring>
#include <fstream>
#include <zlib.h>
#include <istream>
#include <string>
#include <vector>

namespace pcsv {
#include "csv.h"
}

#include "matrix.h"
#include "mri.h"
#include "stats.h"
#include "version.h"

namespace probe {

int dump_on = 0;
std::string d;
const char *MARK = "\n@@RESULT ";
const int SENT_I = INT_MIN + 12345;
const uint32_t SENT_F = 0x7fa5a5a5u;

void unset_i(int *v, int n)
{
  for (int i = 0; i < n; i++) v[i] = SENT_I;
}
void unset_f(float *v, int n)
{
  for (int i = 0; i < n; i++) memcpy(&v[i], &SENT_F, 4);
}
void unset_s(char *s)
{
  s[0] = 1;
  s[1] = 0;
}

std::string hex(const char *s)
{
  std::string r;
  char b[4];
  for (const unsigned char *p = (const unsigned char *)s; *p; p++) {
    snprintf(b, sizeof b, "%02x", *p);
    r += b;
  }
  return r;
}
std::string fbits(float v)
{
  uint32_t u;
  memcpy(&u, &v, 4);
  if (u == SENT_F) return "U";
  char b[16];
  snprintf(b, sizeof b, "%08x", u);
  return b;
}
std::string ival(long long v) { return v == SENT_I ? std::string("U") : std::to_string(v); }

void di(const char *n, long long v) { d += std::string(n) + "=" + ival(v) + " "; }
void dia(const char *n, const int *v, int k)
{
  d += std::string(n) + "=";
  for (int j = 0; j < k; j++) d += (j ? "," : "") + ival(v[j]);
  d += " ";
}
void df(const char *n, float v) { d += std::string(n) + "=" + fbits(v) + " "; }
void dfa(const char *n, const float *v, int k)
{
  d += std::string(n) + "=";
  for (int j = 0; j < k; j++) d += (j ? "," : "") + fbits(v[j]);
  d += " ";
}
void ds(const char *n, const char *s)
{
  d += std::string(n) + "=" + (s[0] == 1 && s[1] == 0 ? std::string("U") : hex(s)) + " ";
}
void dp(const char *n, const char *s) { d += std::string(n) + "=" + (s ? hex(s) : std::string("N")) + " "; }
void dm(const char *n, MATRIX *m)
{
  d += std::string(n) + "=";
  if (!m) {
    d += "N ";
    return;
  }
  d += std::to_string(m->rows) + "x" + std::to_string(m->cols) + ":";
  for (int r = 1; r <= m->rows; r++)
    for (int c = 1; c <= m->cols; c++) d += fbits(*MATRIX_RELT(m, r, c)) + (r == m->rows && c == m->cols ? "" : ",");
  d += " ";
}
void de(const char *name)
{
  const char *v = getenv(name);
  d += std::string(name) + "=" + (v ? hex(v) : std::string("N")) + " ";
}
void finish_dump()
{
  if (!d.empty() && d.back() == ' ') d.pop_back();
  fputs(MARK, stdout);
  fputs(d.c_str(), stdout);
  fflush(NULL);
  _exit(0);
}

// getAllInfo's text with the values that change from run to run and from machine to machine (the
// time, the user, the machine and its kernel's version) as `X`, so that the command lines the
// written files keep compare on every platform (the port's probe does the same to its own text)
std::string canon_info(const std::string &s)
{
  std::string r = s;
  auto blank = [&](const std::string &key, const std::string &next) {
    size_t a = r.find(key);
    if (a == std::string::npos) return;
    a += key.size();
    size_t b = r.find(next, a);
    if (b == std::string::npos) return;
    r.replace(a, b - a, "X");
  };
  blank("TimeStamp: ", "  BuildTime: ");
  blank("User: ", "  Machine: ");
  blank("Machine: ", "  Platform: ");
  blank("PlatformVersion: ", "  CompilerName: ");
  return r;
}

std::string all_info(int argc, char **argv, const std::string &progname)
{
  return canon_info(getAllInfo(argc, argv, progname));
}

}  // namespace probe

#define main mri_convert_main
#define getAllInfo(a, b, c) probe::all_info(a, b, c)
#include "mri_convert_hooked.cpp"
#undef getAllInfo
#undef main

#include <dirent.h>
#include <fcntl.h>
#include <sys/resource.h>
#include <signal.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <unistd.h>

#include <algorithm>
#include <cerrno>
#include <cinttypes>

namespace probe {

std::vector<std::string> toks;
size_t pos;
std::string out;
std::string scratch, workdir;

[[noreturn]] void fail(const char *what)
{
  fprintf(stderr, "fsconvert: %s\n", what);
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
void od(double v)
{
  uint64_t u;
  memcpy(&u, &v, 8);
  char b[24];
  snprintf(b, sizeof b, "%016" PRIx64 " ", u);
  out += b;
}
void oh(const char *s) { out += (s ? (*s ? hex(s) : std::string("-")) : std::string("N")) + " "; }

std::string unhex(const char *h)
{
  std::string r;
  if (strcmp(h, "@") == 0) return r;
  for (const char *p = h; p[0] && p[1]; p += 2) {
    unsigned v;
    sscanf(p, "%2x", &v);
    r += (char)v;
  }
  return r;
}

void put_file(const char *name, const char *h)
{
  std::string b = unhex(h);
  FILE *f = fopen(name, "wb");
  if (!f) fail("cannot write a case file");
  fwrite(b.data(), 1, b.size(), f);
  fclose(f);
}

// a volume: w h d type nframes xsize ysize zsize, then its stored voxels
MRI *get_vol()
{
  int w = geti(), h = geti(), dd = geti(), type = geti(), nf = geti();
  MRI *m = MRIallocSequence(w, h, dd, type, nf);
  m->xsize = getf();
  m->ysize = getf();
  m->zsize = getf();
  size_t n = (size_t)w * h * dd * nf, i = 0;
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
        case MRI_INT: { int s = (int)strtol(v.c_str(), NULL, 10); memcpy(b + 4 * i, &s, 4); break; }
        case MRI_FLOAT: { uint32_t u = (uint32_t)strtoul(v.c_str(), NULL, 16); memcpy(b + 4 * i, &u, 4); break; }
        default: fail("a voxel type the cases do not write");
      }
    }
  }
  return m;
}

void put_vol(const MRI *m)
{
  if (!m) { out += "NULL "; return; }
  oi(m->width); oi(m->height); oi(m->depth); oi(m->type); oi(m->nframes);
  od(m->outside_val);
  of(m->xsize); of(m->ysize); of(m->zsize);
  const float *g[12] = {&m->x_r, &m->x_a, &m->x_s, &m->y_r, &m->y_a, &m->y_s,
                        &m->z_r, &m->z_a, &m->z_s, &m->c_r, &m->c_a, &m->c_s};
  for (const float *p : g) of(*p);
  oi(m->ras_good_flag);
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
      case MRI_INT: { int s; memcpy(&s, b + 4 * i, 4); snprintf(t, sizeof t, "%d", s); break; }
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

void put_table(STAT_TABLE *st)
{
  if (!st) { out += "NULL "; return; }
  out += "T ";
  oh(st->filename); oh(st->measure); oi(st->nrows); oi(st->ncols);
  for (int c = 0; c < st->ncols; c++) oh(st->colnames[c]);
  for (int r = 0; r < st->nrows; r++) oh(st->rownames[r]);
  for (int r = 0; r < st->nrows; r++)
    for (int c = 0; c < st->ncols; c++) od(st->data[r][c]);
  put_vol(st->mri);
}

uint64_t fnv(const std::string &s)
{
  uint64_t h = 0xcbf29ce484222325ULL;
  for (unsigned char c : s) h = (h ^ c) * 0x100000001b3ULL;
  return h;
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

// a file's bytes through gzread (a gzip file decompressed, any other read as it is), each command
// line's text from `TimeStamp: ` (the time, user, machine, platform) to its NUL masked
std::string slurp_gz_masked(const std::string &path)
{
  std::string r;
  gzFile f = gzopen(path.c_str(), "rb");
  if (!f) return r;
  char b[65536];
  int n;
  while ((n = gzread(f, b, sizeof b)) > 0) r.append(b, n);
  gzclose(f);
  const std::string key = "TimeStamp: ";
  for (size_t at = r.find(key); at != std::string::npos; at = r.find(key, at + 1)) {
    for (size_t k = at + key.size(); k < r.size() && r[k] != '\0'; k++) r[k] = 'X';
  }
  return r;
}
bool full_files = false;
// the files the case wrote (not the ones it was given: names starting with `in_`), sorted: a
// volume read back and printed, any other file's FNV-1a (cnv: every file's size and FNV-1a
// through slurp_gz_masked)
std::string written()
{
  std::vector<std::string> names;
  if (DIR *dd = opendir(workdir.c_str())) {
    while (struct dirent *e = readdir(dd)) {
      std::string n = e->d_name;
      if (n == "." || n == ".." || n.compare(0, 3, "in_") == 0) continue;
      names.push_back(n);
    }
    closedir(dd);
  }
  std::sort(names.begin(), names.end());
  out.clear();
  for (const std::string &n : names) {
    std::string path = workdir + "/" + n;
    out += "F " + n + " ";
    if (full_files) {
      std::string b = slurp_gz_masked(path);
      char h[48];
      snprintf(h, sizeof h, "%zu %016" PRIx64 " ", b.size(), fnv(b));
      out += h;
    } else if (n.size() > 4 && n.compare(n.size() - 4, 4, ".mgz") == 0) {
      // read back quietly: MRIread's own messages would go into the parent's output
      fflush(stdout);
      fflush(stderr);
      int keep1 = dup(1), keep2 = dup(2), null = open("/dev/null", O_WRONLY);
      dup2(null, 1);
      dup2(null, 2);
      int gd = Gdiag;
      Gdiag = 0;
      MRI *m = MRIread(path.c_str());
      Gdiag = gd;
      fflush(stdout);
      fflush(stderr);
      dup2(keep1, 1);
      dup2(keep2, 2);
      close(keep1);
      close(keep2);
      close(null);
      put_vol(m);
      if (m) MRIfree(&m);
    } else {
      char b[24];
      snprintf(b, sizeof b, "%016" PRIx64 " ", fnv(slurp(path)));
      out += b;
    }
  }
  if (!out.empty() && out.back() == ' ') out.pop_back();
  return out;
}

void clear_dir(const std::string &dir)
{
  if (DIR *dd = opendir(dir.c_str())) {
    while (struct dirent *e = readdir(dd)) {
      std::string n = e->d_name;
      if (n != "." && n != "..") unlink((dir + "/" + n).c_str());
    }
    closedir(dd);
  }
}

// the template name of ist and wst: `-` NULL, `@` "", else the file (written unless its hex is `-`)
const char *get_template(std::string &keep)
{
  keep = next();
  const char *h = next();
  if (keep == "-") return NULL;
  if (keep == "@") return "";
  if (strcmp(h, "-") != 0) put_file(keep.c_str(), h);
  return keep.c_str();
}

void run_case()
{
  std::string cmd = next();
  if (cmd == "csv") {
    put_file("in_t.csv", next());
    std::ifstream f("in_t.csv");
    std::vector<std::vector<std::string>> t = pcsv::readCSV(f);
    oi((long long)t.size());
    for (auto &row : t) {
      oi((long long)row.size());
      for (auto &x : row) oh(x.c_str());
    }
  } else if (cmd == "lst") {
    std::string name = next();
    const char *h = next();
    if (strcmp(h, "-") != 0) put_file(name.c_str(), h);
    put_table(LoadStatTable(name.c_str()));
  } else if (cmd == "ist" || cmd == "wst") {
    MRI *m = get_vol();
    std::string keep;
    const char *tmpl = get_template(keep);
    STAT_TABLE *st = InitStatTableFromMRI(m, tmpl);
    put_table(st);
    if (cmd == "wst") {
      std::string o = next();
      if (st) WriteStatTable(o.c_str(), st);
    }
  } else if (cmd == "opt" || cmd == "cnv") {
    int nfiles = geti();
    for (int i = 0; i < nfiles; i++) {
      std::string name = next();
      put_file(name.c_str(), next());
    }
    int n = geti();
    static std::vector<std::string> keep;
    keep.assign(1, "mri_convert");
    for (int i = 0; i < n; i++) keep.push_back(unhex(next()));
    std::vector<char *> av;
    for (std::string &s : keep) av.push_back(&s[0]);
    av.push_back(NULL);
    av.push_back(NULL);
    dump_on = cmd == "opt";
    mri_convert_main((int)keep.size(), av.data());
    out += "RETURNED ";
  } else {
    fail("unknown command");
  }
}

}  // namespace probe

int main(int argc, char *argv[])
{
  using namespace probe;
  if (argc != 2) {
    fprintf(stderr, "usage: fsconvert <cases>\n");
    return 2;
  }
  if (argv[1][0] != '/') fail("the cases must be given by an absolute path");
  scratch = std::string(argv[1]) + ".out";
  workdir = std::string(argv[1]) + ".d";
  Progname = "mri_convert";
  setenv("SURFER_SIDEDOOR", "1", 1);  // the licence key check (none in the replica)
  FILE *fp = fopen(argv[1], "r");
  if (!fp) fail("cannot open the cases");
  std::vector<std::string> lines;
  static char buf[1 << 23];
  while (fgets(buf, sizeof buf, fp)) lines.push_back(buf);
  fclose(fp);
  mkdir(workdir.c_str(), 0700);
  for (const std::string &line : lines) {
    if (line.size() >= sizeof buf - 1) fail("a case longer than the buffer");
    fflush(stdout);
    clear_dir(workdir);
    pid_t pid = fork();
    if (pid < 0) fail("fork");
    if (pid == 0) {
      int fd = open(scratch.c_str(), O_WRONLY | O_CREAT | O_TRUNC, 0600);
      if (fd < 0) _exit(3);
      dup2(fd, 1);
      dup2(fd, 2);
      close(fd);
      setvbuf(stdout, NULL, _IONBF, 0);
      if (chdir(workdir.c_str()) != 0) _exit(4);
      // every case under 4 GiB of address space (a reader that never ends grows without bound)
      struct rlimit rl = {4ull << 30, 4ull << 30};
      setrlimit(RLIMIT_AS, &rl);
      alarm(60);
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
      fflush(NULL);
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
    full_files = line.compare(0, 4, "cnv ") == 0;
    std::string files = written();
    if (!files.empty()) result += (result.empty() ? "" : " ") + files;
    std::string esc;
    char b[8];
    for (unsigned char c : text) {
      if (c < 0x20 || c > 0x7e || c == '\\') {
        snprintf(b, sizeof b, "\\x%02x", c);
        esc += b;
      } else {
        esc += (char)c;
      }
    }
    printf("%s | %s\n", result.c_str(), esc.c_str());
  }
  clear_dir(workdir);
  rmdir(workdir.c_str());
  unlink(scratch.c_str());
  return 0;
}
