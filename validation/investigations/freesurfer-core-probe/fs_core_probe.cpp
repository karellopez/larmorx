// SPDX-License-Identifier: Apache-2.0
// A black-box probe of FreeSurfer 8.2.0's core services (specs/freesurfer-core.md), linked against
// the unit oracle's libutils.a (GCC 8.4.0, FreeSurfer's flags). It holds only calls of FreeSurfer's
// functions (and of the C library's) by name, and the reading and printing of their arguments and
// results: no FreeSurfer code. The commands and their formats are in README.md beside this file.
//
// Encodings (arguments and results): a string is `x` followed by the hex digits of its bytes (`x`
// alone is the empty string), `null` is no string; a float is the 8 hex digits of its bits, a
// double the 16 hex digits of its bits; integers are decimal.
//
//   fs_core_probe batch <file>      one call per line of <file> ("<command> <arguments>"), one result
//                                   line per call on standard output (README.md, "Batch commands")
//   fs_core_probe <command> ...     one call (or one sequence) per run (README.md, "Run commands")
#include <cerrno>
#include <cinttypes>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <ctime>
#include <glob.h>
#include <stdexcept>
#include <string>
#include <sys/resource.h>
#include <unistd.h>
#include <vector>

#include "base.h"
#include "cmdargs.h"
#include "diag.h"
#include "fsenv.h"
#include "hips.h"
#include "log.h"
#include "machine.h"
#include "romp_support.h"
#include "timer.h"
#include "utils.h"
#include "version.h"
#include "xDebug.h"

typedef std::vector<std::string> Toks;

// ---- encodings ------------------------------------------------------------------------------

static bool is_null(const std::string &t) { return t == "null"; }

static std::string unhex(const std::string &t)
{
  if (t.empty() || t[0] != 'x') {
    fprintf(stderr, "probe: a string must start with x: %s\n", t.c_str());
    exit(2);
  }
  std::string s;
  for (size_t i = 1; i + 1 < t.size(); i += 2) s.push_back((char)strtol(t.substr(i, 2).c_str(), NULL, 16));
  return s;
}

static std::string hex(const char *s)
{
  if (!s) return "null";
  std::string o = "x";
  char b[3];
  for (const unsigned char *p = (const unsigned char *)s; *p; p++) {
    snprintf(b, sizeof b, "%02x", *p);
    o += b;
  }
  return o;
}

static std::string hexn(const char *s, size_t n)
{
  std::string o = "x";
  char b[3];
  for (size_t i = 0; i < n; i++) {
    snprintf(b, sizeof b, "%02x", (unsigned char)s[i]);
    o += b;
  }
  return o;
}

static float f32(const std::string &t)
{
  uint32_t u = (uint32_t)strtoul(t.c_str(), NULL, 16);
  float f;
  memcpy(&f, &u, 4);
  return f;
}

static double f64(const std::string &t)
{
  uint64_t u = strtoull(t.c_str(), NULL, 16);
  double d;
  memcpy(&d, &u, 8);
  return d;
}

static std::string sf(float f)
{
  uint32_t u;
  memcpy(&u, &f, 4);
  char b[16];
  snprintf(b, sizeof b, "%08" PRIx32, u);
  return b;
}

static std::string sd(double d)
{
  uint64_t u;
  memcpy(&u, &d, 8);
  char b[24];
  snprintf(b, sizeof b, "%016" PRIx64, u);
  return b;
}

static std::string si(long long v) { return std::to_string(v); }

static Toks split(const std::string &line)
{
  Toks t;
  size_t i = 0;
  while (i < line.size()) {
    while (i < line.size() && line[i] == ' ') i++;
    size_t j = i;
    while (j < line.size() && line[j] != ' ') j++;
    if (j > i) t.push_back(line.substr(i, j - i));
    i = j;
  }
  return t;
}

// A writable copy of a string argument (FreeSurfer's helpers take char *).
struct Buf {
  std::vector<char> v;
  explicit Buf(const std::string &s, size_t extra = 0) : v(s.begin(), s.end()) { v.resize(s.size() + 1 + extra, 0); }
  char *p() { return v.data(); }
};

// The lists of the list commands sit between two guard values, so a read just outside the list
// shows as the guard: 7777 for integers, 7777.0f for floats, unless the variable
// FS_CORE_PROBE_GUARD gives another integer (the float guard is that integer as a float): two runs
// with different guards tell a result that depends on memory outside the list.
static int guard_i()
{
  const char *g = getenv("FS_CORE_PROBE_GUARD");
  return g ? atoi(g) : 7777;
}
#define GUARD_I guard_i()
#define GUARD_F ((float)guard_i())

// ---- batch commands -------------------------------------------------------------------------

static std::string batch_call(const Toks &a)
{
  const std::string &c = a[0];
  if (c == "swapshort") return si(swapShort((short)atol(a[1].c_str())));
  if (c == "swapushort") return si(swapUShort((unsigned short)atol(a[1].c_str())));
  if (c == "swapint") return si(swapInt((int)atol(a[1].c_str())));
  if (c == "swaplong32") return si(swapLong32((long32)atol(a[1].c_str())));
  if (c == "swaplong64") return si(swapLong64((long64)strtoll(a[1].c_str(), NULL, 10)));
  if (c == "swapfloat") return sf(swapFloat(f32(a[1])));
  if (c == "swapdouble") return sd(swapDouble(f64(a[1])));
  if (c == "byteswapbuf") {
    // byteswapbuf <bytes per item> <items> <buffer>
    std::string b = unhex(a[3]);
    std::vector<char> v(b.begin(), b.end());
    v.resize(b.size() + 16, 0);
    int r = ByteSwapBuf(v.data(), atol(a[2].c_str()), atoi(a[1].c_str()));
    fflush(stdout);
    return si(r) + " " + hexn(v.data(), b.size());
  }
  if (c == "arch486") return si(Arch486());
  if (c == "isflag") return si(CMDisFlag(Buf(unhex(a[1])).p()));
  if (c == "singledash") return si(CMDsingleDash(Buf(unhex(a[1])).p()));
  if (c == "stringmatch") {
    std::string s1 = is_null(a[1]) ? "" : unhex(a[1]), s2 = is_null(a[2]) ? "" : unhex(a[2]);
    if (is_null(a[1]) && is_null(a[2])) return "skip";  // both absent: not a defined call
    return si(CMDstringMatch(is_null(a[1]) ? NULL : s1.c_str(), is_null(a[2]) ? NULL : s2.c_str()));
  }
  if (c == "nthisarg") {
    // nthisarg <nth> <argument>... (nargc = the number of arguments given)
    std::vector<Buf> bufs;
    for (size_t i = 2; i < a.size(); i++) bufs.emplace_back(unhex(a[i]));
    std::vector<char *> argv;
    for (auto &b : bufs) argv.push_back(b.p());
    argv.push_back(NULL);
    return si(CMDnthIsArg((int)bufs.size(), argv.data(), atoi(a[1].c_str())));
  }
  if (c == "fgetl") {
    // fgetl <n> <file content>: every line fgetl returns until it returns nothing
    std::string content = unhex(a[2]);
    int n = atoi(a[1].c_str());
    FILE *fp = content.empty() ? fopen("/dev/null", "r") : fmemopen((void *)content.data(), content.size(), "r");
    std::vector<char> buf(n + 8, 0);
    std::string out;
    char *cp;
    while ((cp = fgetl(buf.data(), n, fp)) != NULL) out += hex(cp) + " ";
    fclose(fp);
    return out + "end";
  }
  if (c == "strupper") return hex(StrUpper(Buf(unhex(a[1])).p()));
  if (c == "strlower") return hex(StrLower(Buf(unhex(a[1])).p()));
  if (c == "str_toupper") {
    Buf b(unhex(a[1]));
    str_toupper(b.p());
    return hex(b.p());
  }
  if (c == "filenameonly") {
    std::string s = unhex(a[1]);
    std::vector<char> out(s.size() + 8, 0);
    return hex(FileNameOnly(s.c_str(), out.data()));
  }
  if (c == "filename") {
    // the result, then the argument as the call left it
    Buf b(unhex(a[1]));
    char *r = FileName(b.p());
    return hex(r) + " " + hex(b.p());
  }
  if (c == "filenamepath") {
    std::string s = unhex(a[1]);
    std::vector<char> out(s.size() + 8, 0);
    return hex(FileNamePath(s.c_str(), out.data()));
  }
  if (c == "removeext") {
    std::string s = unhex(a[1]);
    std::vector<char> out(s.size() + 8, 0);
    return hex(FileNameRemoveExtension(s.c_str(), out.data()));
  }
  if (c == "extension") {
    std::string s = unhex(a[1]);
    std::vector<char> out(s.size() + 8, 0);
    return hex(FileNameExtension(s.c_str(), out.data()));
  }
  if (c == "fileexists") {
    // the result, then whether errno is as before the call (1) or changed (0)
    errno = 1234;
    int r = FileExists(unhex(a[1]).c_str());
    int same = errno == 1234;
    errno = 0;
    return si(r) + " " + si(same);
  }
  if (c == "fromwildcard") {
    std::string s = unhex(a[1]);
    std::vector<char> out(s.size() + 4096, 0);
    return hex(FileNameFromWildcard(s.c_str(), out.data()));
  }
  if (c == "stricmp") return si(stricmp(unhex(a[1]).c_str(), unhex(a[2]).c_str()));
  if (c == "itemsinstring") return si(ItemsInString(unhex(a[1]).c_str()));
  if (c == "countitems") return si(CountItemsInString(unhex(a[1]).c_str()));
  if (c == "nthitem") {
    // nthitem <n> <string>
    char *r = GetNthItemFromString(unhex(a[2]).c_str(), atoi(a[1].c_str()));
    std::string o = hex(r);
    free(r);
    return o;
  }
  if (c == "deblank") {
    char *r = deblank(unhex(a[1]).c_str());
    std::string o = hex(r);
    free(r);
    return o;
  }
  if (c == "deltaangle") return sf(deltaAngle(f32(a[1]), f32(a[2])));
  if (c == "devisinf") return si(devIsinf(f32(a[1])));
  if (c == "devisnan") return si(devIsnan(f32(a[1])));
  if (c == "devfinite") return si(devFinite(f32(a[1])));
  if (c == "nint") return si(nint(f64(a[1])));
  if (c == "nint2") return si(nint2(f64(a[1])));
  if (c == "sum2stddev") return sd(sum2stddev(f64(a[1]), f64(a[2]), atoi(a[3].c_str())));
  if (c == "cmpints") {
    int x = atoi(a[1].c_str()), y = atoi(a[2].c_str());
    return si(compare_ints(&x, &y));
  }
  if (c == "cmpfloats") {
    float x = f32(a[1]), y = f32(a[2]);
    return si(compare_floats(&x, &y));
  }
  if (c == "nunique" || c == "unique" || c == "mostfrequent") {
    // <command> <integer>...: the result, then the list as the call left it
    int n = (int)a.size() - 1;
    std::vector<int> v(n + 2, GUARD_I);
    for (int i = 0; i < n; i++) v[i + 1] = atoi(a[i + 1].c_str());
    std::string o;
    if (c == "nunique")
      o = si(nunqiue_int_list(v.data() + 1, n));
    else if (c == "unique") {
      int nu = 0;
      int *u = unqiue_int_list(v.data() + 1, n, &nu);
      o = si(nu);
      for (int i = 0; i < nu; i++) o += " " + si(u[i]);
      free(u);
    }
    else {
      int nmax = 0;
      int m = most_frequent_int_list(v.data() + 1, n, &nmax);
      o = si(m) + " " + si(nmax);
    }
    o += " |";
    for (int i = 0; i < n; i++) o += " " + si(v[i + 1]);
    return o;
  }
  if (c == "kth" || c == "qselect" || c == "median" || c == "mad") {
    // kth|qselect <k> <float>...; median|mad <float>...: the result, then the list as left
    size_t first = (c == "kth" || c == "qselect") ? 2 : 1;
    int n = (int)(a.size() - first);
    std::vector<float> v(n + 2, GUARD_F);
    for (int i = 0; i < n; i++) v[i + 1] = f32(a[first + i]);
    float r;
    if (c == "kth")
      r = kth_smallest(v.data() + 1, n, atoi(a[1].c_str()));
    else if (c == "qselect")
      r = quick_select(v.data() + 1, n, atoi(a[1].c_str()));
    else if (c == "median")
      r = median(v.data() + 1, n);
    else
      r = mad(v.data() + 1, n);
    std::string o = sf(r) + " |";
    for (int i = 0; i < n; i++) o += " " + sf(v[i + 1]);
    return o;
  }
  if (c == "atan2f") return sf(fastApproxAtan2f(f32(a[1]), f32(a[2])));
  if (c == "dliststats") {
    // dliststats <n> <double>... | dliststats <n> null
    int n = atoi(a[1].c_str());
    std::vector<double> v;
    bool none = a.size() > 2 && is_null(a[2]);
    for (size_t i = 2; !none && i < a.size(); i++) v.push_back(f64(a[i]));
    v.push_back(0.0);
    double st[5] = {0, 0, 0, 0, 0};
    double *r = DListStats(none ? NULL : v.data(), n, st);
    fflush(stdout);
    if (!r) return "null";
    std::string o;
    for (int i = 0; i < 5; i++) o += (i ? " " : "") + sd(st[i]);
    return o;
  }
  if (c == "findfirst") {
    // findfirst <value> <integer>...
    std::vector<int> v;
    for (size_t i = 2; i < a.size(); i++) v.push_back(atoi(a[i].c_str()));
    return si(FindFirstMatch(v, atoi(a[1].c_str())));
  }
  if (c == "argv2cmdline") {
    std::vector<Buf> bufs;
    for (size_t i = 1; i < a.size(); i++) bufs.emplace_back(unhex(a[i]));
    std::vector<char *> argv;
    for (auto &b : bufs) argv.push_back(b.p());
    char *r = argv2cmdline((int)argv.size(), argv.data());
    std::string o = hex(r);
    free(r);
    return o;
  }
  if (c == "checkfinite") {
    // checkfinite <place> <double>
    errno = 0;
    int r = check_finite(unhex(a[1]).c_str(), f64(a[2]));
    return si(r);
  }
  if (c == "rompsum") {
    // rompsum <initial double> <double>...: the values summed by a reproducible ROMP loop
    double total = f64(a[1]);
    std::vector<double> vals;
    for (size_t i = 2; i < a.size(); i++) vals.push_back(f64(a[i]));
    int nvals = (int)vals.size();
#define ROMP_VARIABLE i
#define ROMP_LO 0
#define ROMP_HI nvals
#define ROMP_SUMREDUCTION0 total
#define ROMP_FOR_LEVEL ROMP_level_assume_reproducible
#include "romp_for_begin.h"
    ROMP_for_begin
#define total ROMP_PARTIALSUM(0)
      total += vals[i];
#undef total
#include "romp_for_end.h"
    return sd(total);
  }
  if (c == "rompparts") {
    // rompparts <lo> <hi>: the pieces ROMP_Distributor_begin cuts [lo, hi) into
    static ROMP_Distributor d;
    double s0 = 0;
    ROMP_Distributor_begin(&d, atoi(a[1].c_str()), atoi(a[2].c_str()), &s0, NULL, NULL);
    std::string o = si(d.partialSize);
    for (int i = 0; i < d.partialSize; i++) o += " " + si(d.partials[i].lo) + ":" + si(d.partials[i].hi);
    return o;
  }
  if (c == "rompparallel") {
    // whether a loop of each ROMP level (serial, experimental, fast, assume_reproducible,
    // shown_reproducible) may run in parallel
    std::string o;
    for (int l = 0; l < (int)ROMP_level__size; l++)
      o += si(ROMP_if_parallel1((ROMP_level)l)) + (l + 1 < (int)ROMP_level__size ? " " : "");
    return o;
  }
  // ---- the C library, as FreeSurfer's helpers use it
  if (c == "strerror") return hex(strerror(atoi(a[1].c_str())));
  if (c == "atoi") return si(atoi(unhex(a[1]).c_str()));
  if (c == "atof") return sd(atof(unhex(a[1]).c_str()));
  if (c == "scanlx") {
    unsigned long v = 0;
    int r = sscanf(unhex(a[1]).c_str(), "0x%lx", &v);
    return si(r) + " " + std::to_string(v);
  }
  if (c == "scand") {
    int v = -999999;
    int r = sscanf(unhex(a[1]).c_str(), "%d", &v);
    return si(r) + " " + (r == 1 ? si(v) : std::string("-"));
  }
  if (c == "stol") {
    try {
      return si(std::stol(unhex(a[1])));
    } catch (const std::invalid_argument &) {
      return "invalid_argument";
    } catch (const std::out_of_range &) {
      return "out_of_range";
    }
  }
  if (c == "glob") {
    glob_t g;
    int r = glob(unhex(a[1]).c_str(), 0, NULL, &g);
    std::string o = si(r);
    if (r == 0) {
      for (size_t i = 0; i < g.gl_pathc; i++) o += " " + hex(g.gl_pathv[i]);
      globfree(&g);
    }
    return o;
  }
  if (c == "gmtime" || c == "localtime") {
    time_t t = (time_t)strtoll(a[1].c_str(), NULL, 10);
    struct tm tmv;
    struct tm *p = c == "gmtime" ? gmtime_r(&t, &tmv) : localtime_r(&t, &tmv);
    if (!p) return "null";
    char b[256];
    snprintf(b, sizeof b, "%d %d %d %d %d %d %d %d %d %ld %s", p->tm_year, p->tm_mon, p->tm_mday, p->tm_hour,
             p->tm_min, p->tm_sec, p->tm_wday, p->tm_yday, p->tm_isdst, p->tm_gmtoff, hex(p->tm_zone).c_str());
    return b;
  }
  if (c == "strftimec") {
    time_t t = (time_t)strtoll(a[1].c_str(), NULL, 10);
    struct tm tmv;
    if (!localtime_r(&t, &tmv)) return "null";
    char b[1000];
    strftime(b, sizeof b, "%c", &tmv);
    return hex(b);
  }
  fprintf(stderr, "probe: unknown batch command %s\n", c.c_str());
  exit(2);
}

static int batch(const char *path)
{
  FILE *fp = strcmp(path, "-") ? fopen(path, "r") : stdin;
  if (!fp) {
    fprintf(stderr, "probe: cannot open %s\n", path);
    return 2;
  }
  std::string line;
  int ch;
  while (true) {
    line.clear();
    while ((ch = fgetc(fp)) != EOF && ch != '\n') line.push_back((char)ch);
    if (line.empty() && ch == EOF) break;
    Toks t = split(line);
    if (!t.empty()) {
      std::string r = batch_call(t);
      printf("%s\n", r.c_str());
      fflush(stdout);
      fflush(stderr);
    }
    if (ch == EOF) break;
  }
  return 0;
}

// ---- run commands ---------------------------------------------------------------------------

static void print_globals()
{
  printf("Gdiag=%lx Gdiag_no=%d IMAGE_SIZE=%d Gx=%d Gy=%d Gz=%d Gvx=%d Gvy=%d Gvz=%d Gprofile=%d\n", Gdiag, Gdiag_no,
         IMAGE_SIZE, Gx, Gy, Gz, Gvx, Gvy, Gvz, Gprofile);
}

static std::vector<char *> argv_of(std::vector<Buf> &bufs)
{
  std::vector<char *> av;
  for (auto &b : bufs) av.push_back(b.p());
  av.push_back(NULL);
  return av;
}

int main(int argc, char **argv)
{
  if (argc < 2) {
    fprintf(stderr, "usage: fs_core_probe batch <file> | <command> ... (README.md)\n");
    return 2;
  }
  std::string c = argv[1];
  Toks a;
  for (int i = 2; i < argc; i++) a.push_back(argv[i]);
  if (c == "batch") return batch(argc > 2 ? argv[2] : "-");
  if (c == "single") {
    // single <batch command> <arguments>: one batch call alone (for the calls that end a run)
    printf("%s\n", batch_call(a).c_str());
    return 0;
  }
  if (c == "init") {
    // init: DiagInit(NULL, NULL, NULL) with the environment as given, then the state it set
    unsigned long r = DiagInit(NULL, NULL, NULL);
    printf("return=%lx\n", r);
    print_globals();
    return 0;
  }
  if (c == "errorprintf" || c == "errorexit") {
    // errorprintf|errorexit <errno> <code> <message> [log <file>] [throw] [twice]: the message
    // passed through "%s"; then Gerror and errno (errorexit: what reached the caller)
    int e = atoi(a[0].c_str()), code = atoi(a[1].c_str());
    std::string msg = unhex(a[2]);
    bool thr = false, twice = false;
    for (size_t i = 3; i < a.size(); i++) {
      if (a[i] == "log" && i + 1 < a.size())
        setErrorLog(unhex(a[++i]));
      else if (a[i] == "throw")
        thr = true;
      else if (a[i] == "twice")
        twice = true;
    }
    throwExceptions(thr);
    errno = e;
    if (c == "errorprintf") {
      ErrorPrintf(code, "%s", msg.c_str());
      if (twice) ErrorPrintf(code, "%s", msg.c_str());
      printf("Gerror=%d errno=%d\n", Gerror, errno);
      return 0;
    }
    try {
      ErrorExit(code, "%s", msg.c_str());
      printf("returned Gerror=%d\n", Gerror);
    } catch (int t) {
      printf("caught %d Gerror=%d\n", t, Gerror);
    }
    return 0;
  }
  if (c == "fatal" || c == "error" || c == "warning" || c == "debug") {
    // fatal <code> <message> [throw] | error <message> [log <file>] | warning <message> |
    // debug <Gdiag in hex> <message>
    if (c == "fatal") {
      throwExceptions(a.size() > 2 && a[2] == "throw");
      try {
        fs::fatal(atoi(a[0].c_str())) << unhex(a[1]);
        printf("returned\n");
      } catch (int t) {
        printf("caught %d\n", t);
      }
    }
    else if (c == "error") {
      if (a.size() > 2 && a[1] == "log") setErrorLog(unhex(a[2]));
      fs::error() << unhex(a[0]);
    }
    else if (c == "warning")
      fs::warning() << unhex(a[0]);
    else {
      Gdiag = strtoul(a[0].c_str(), NULL, 16);
      fs::debug() << unhex(a[1]);
    }
    return 0;
  }
  if (c == "argnerr") {
    CMDargNErr(Buf(unhex(a[0])).p(), atoi(a[1].c_str()));
    return 0;
  }
  if (c == "usageexit") {
    CMDusageExit(Buf(std::string("p")).p());
    return 0;
  }
  if (c == "assert") {
    // assert <file> <line> <test>: assertFailed(file, line, test)
    assertFailed(unhex(a[0]).c_str(), atoi(a[1].c_str()), unhex(a[2]).c_str());
    return 0;
  }
  if (c == "diagprintf") {
    // diagprintf <Gdiag in hex> <bits in hex> <message>
    Gdiag = strtoul(a[0].c_str(), NULL, 16);
    int r = DiagPrintf(strtoul(a[1].c_str(), NULL, 16), "%s", unhex(a[2]).c_str());
    printf("\nreturn=%d\n", r);
    return 0;
  }
  if (c == "heartbeat" || c == "pctdone") {
    // heartbeat <Gdiag in hex> <float>... | pctdone <Gdiag in hex> <nprints> <float>...
    Gdiag = strtoul(a[0].c_str(), NULL, 16);
    if (c == "heartbeat")
      for (size_t i = 1; i < a.size(); i++) DiagHeartbeat(f32(a[i]));
    else
      for (size_t i = 2; i < a.size(); i++) DiagShowPctDone(f32(a[i]), atoi(a[1].c_str()));
    return 0;
  }
  if (c == "random") {
    // random <step>...: seed=<long> | draw=<low double>,<high double> | perm=<n> | state
    for (auto &s : a) {
      if (s.compare(0, 5, "seed=") == 0)
        setRandomSeed(strtol(s.c_str() + 5, NULL, 10));
      else if (s.compare(0, 5, "draw=") == 0) {
        size_t k = s.find(',');
        double v = randomNumber(f64(s.substr(5, k - 5)), f64(s.substr(k + 1)));
        printf("%s\n", sd(v).c_str());
      }
      else if (s.compare(0, 5, "perm=") == 0) {
        int n = atoi(s.c_str() + 5);
        int *p = compute_permutation(n, NULL);
        std::string o = "perm";
        for (int i = 0; i < n; i++) o += " " + si(p[i]);
        printf("%s\n", o.c_str());
        free(p);
      }
      else if (s == "state")
        printf("seed=%ld calls=%ld\n", getRandomSeed(), getRandomCalls());
      fflush(stdout);
    }
    return 0;
  }
  if (c == "version" || c == "allinfo") {
    // version|allinfo <progname> <argv[0]> <argument>...: handleVersionOption, then its result and
    // the arguments left (the first argc - result); allinfo: getAllInfo alone
    std::string prog = unhex(a[0]);
    std::vector<Buf> bufs;
    for (size_t i = 1; i < a.size(); i++) bufs.emplace_back(unhex(a[i]));
    std::vector<char *> av = argv_of(bufs);
    if (c == "allinfo") {
      printf("%s\n", getAllInfo((int)bufs.size(), av.data(), prog).c_str());
      return 0;
    }
    int n = handleVersionOption((int)bufs.size(), av.data(), prog.c_str());
    std::string o = "n=" + si(n);
    for (int i = 0; i < (int)bufs.size() - n; i++) o += " " + hex(av[i]);
    printf("%s\n", o.c_str());
    return 0;
  }
  if (c == "versionstrings") {
    printf("%s\n%s\n", getVersion().c_str(), getBuildStamp().c_str());
    return 0;
  }
  if (c == "curstamp") {
    printf("%s\n", VERcurTimeStamp());
    return 0;
  }
  if (c == "filestamp") {
    char *s = VERfileTimeStamp(Buf(unhex(a[0])).p());
    printf("%s\n", s ? s : "NULL");
    return 0;
  }
  if (c == "user") {
    printf("%s\n", VERuser());
    return 0;
  }
  if (c == "datetime") {
    printf("%s\n", currentDateTime(atoi(a[0].c_str()) != 0).c_str());
    return 0;
  }
  if (c == "fsenv") {
    FSENV *e = FSENVgetenv();
    if (!e) {
      printf("\nNULL\n");
      return 0;
    }
    printf("\nFREESURFER_HOME=%s\nSUBJECTS_DIR=%s\nuser=%s\ncwd=%s\nsysname=%s\nmachine=%s\nhostname=%s\ndate=%s\n"
           "desired_bvec_space=%d\nctab=%s\n",
           hex(e->FREESURFER_HOME).c_str(), hex(e->SUBJECTS_DIR).c_str(), hex(e->user).c_str(), hex(e->cwd).c_str(),
           hex(e->sysname).c_str(), hex(e->machine).c_str(), hex(e->hostname).c_str(), e->date, e->desired_bvec_space,
           e->ctab ? "read" : "none");
    return 0;
  }
  if (c == "getsubjectsdir") {
    printf("%s\n", hex(FSENVgetSUBJECTS_DIR()).c_str());
    return 0;
  }
  if (c == "setsubjectsdir") {
    FSENVsetSUBJECTS_DIR(Buf(unhex(a[0])).p());
    printf("%s\n", hex(getenv("SUBJECTS_DIR")).c_str());
    return 0;
  }
  if (c == "logic") {
    // logic <file> <line> <calls>: spendTimeCheckingForLogicProblem's answers, one digit per call
    std::string file = unhex(a[0]);
    int line = atoi(a[1].c_str()), n = atoi(a[2].c_str());
    std::string o;
    for (int i = 0; i < n; i++) o += spendTimeCheckingForLogicProblem(file.c_str(), line) ? '1' : '0';
    printf("%s\n", o.c_str());
    return 0;
  }
  if (c == "cope") {
    // cope <fixer variable | null> <message> <file> <line> <function> <calls>: each call's
    // response (0 old, 1 fix) and whether it reported
    std::string fixer = is_null(a[0]) ? "" : unhex(a[0]), msg = unhex(a[1]), file = unhex(a[2]), fn = unhex(a[4]);
    int line = atoi(a[3].c_str()), n = atoi(a[5].c_str());
    for (int i = 0; i < n; i++) {
      bool rep = true;
      LogicProblemResponse r =
          copeWithLogicProblem2(&rep, is_null(a[0]) ? NULL : fixer.c_str(), msg.c_str(), file.c_str(), line, fn.c_str());
      printf("response=%d reported=%d\n", (int)r, (int)rep);
      fflush(stdout);
    }
    return 0;
  }
  if (c == "tempdir") {
    printf("%s\n", getBaseTempDir().c_str());
    return 0;
  }
  if (c == "tempfile") {
    printf("%s\n", makeTempFile(a.empty() ? "" : unhex(a[0])).c_str());
    return 0;
  }
  if (c == "mktempdir") {
    printf("%s\n", makeTempDir().c_str());
    return 0;
  }
  if (c == "dirwritable") {
    printf("%d %d\n", (int)directoryExists(unhex(a[0])), (int)directoryIsWritable(unhex(a[0])));
    return 0;
  }
  if (c == "memused") {
    int m = getMemoryUsed();
    printf("\nmemused=%d\n", m);
    return 0;
  }
  if (c == "memusage") {
    int u[5] = {-1, -1, -1, -1, -1};
    GetMemUsage(u);
    printf("%d %d %d %d %d peak=%d\n", u[0], u[1], u[2], u[3], u[4], GetVmPeak());
    return 0;
  }
  if (c == "rusage") {
    int r = PrintRUsage(RUSAGE_SELF, unhex(a[0]).c_str(), stdout);
    printf("return=%d\n", r);
    return 0;
  }
  if (c == "writerusage") {
    int r = WriteRUsage(RUSAGE_SELF, unhex(a[0]).c_str(), Buf(unhex(a[1])).p());
    printf("return=%d\n", r);
    return 0;
  }
  if (c == "xdbg") {
    // xdbg <step>...: push=<title>,<note> | pop | enter=<title> | exit | desc=<text> | note=<text> |
    // cur (prints the current function, note and description)
    for (auto &s : a) {
      if (s.compare(0, 5, "push=") == 0) {
        size_t k = s.find(',');
        Buf t(unhex(s.substr(5, k - 5))), n(unhex(s.substr(k + 1)));
        xDbg_PushStack(t.p(), n.p());
      }
      else if (s == "pop")
        xDbg_PopStack();
      else if (s.compare(0, 6, "enter=") == 0) {
        std::string t = unhex(s.substr(6));
        DebugEnterFunction(("%s", t.c_str()));
      }
      else if (s == "exit")
        DebugExitFunction;
      else if (s.compare(0, 5, "desc=") == 0)
        xDbg_SetStackDesc("%s", unhex(s.substr(5)).c_str());
      else if (s.compare(0, 5, "note=") == 0)
        xDbg_SetCurrentNote("%s", unhex(s.substr(5)).c_str());
      else if (s == "cur")
        printf("%s %s %s\n", hex(xDbg_GetCurrentFunction()).c_str(), hex(xDbg_GetCurrentNote()).c_str(),
               hex(xDbg_sStackDesc).c_str());
      fflush(stdout);
    }
    return 0;
  }
  if (c == "maxthreads") {
    printf("%d\n", omp_get_max_threads());
    return 0;
  }
  if (c == "hips") {
    // hips: one HIPS function with a result and one without
    double r = h_entropy(NULL, 0, 0);
    thin(NULL, 0, 0);
    printf("%s\n", sd(r).c_str());
    return 0;
  }
  if (c == "threadnum") {
    // threadnum <n>: n threads each ask their number (FreeSurfer's thread-number check)
    omp_set_num_threads(atoi(a[0].c_str()));
    int maxseen = -1;
#pragma omp parallel
    {
      int t = omp_get_thread_num();
#pragma omp critical
      if (t > maxseen) maxseen = t;
    }
    printf("max thread number %d\n", maxseen);
    return 0;
  }
  fprintf(stderr, "probe: unknown command %s\n", c.c_str());
  return 2;
}
