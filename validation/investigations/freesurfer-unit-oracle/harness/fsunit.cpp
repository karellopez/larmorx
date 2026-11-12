// SPDX-License-Identifier: Apache-2.0
// CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: it calls FreeSurfer 8.2.0's library functions by
// name, from its headers.
//
// fsunit: FreeSurfer 8.2.0's unit oracle. One call of one library function of the official
// build's `utils` (and the ITK 4.13.2 VNL it links) on given inputs; its outputs written to files
// and, on standard output, one JSON object with every floating-point value both as C99's
// hexadecimal form ("%a": exact) and as "%.17g". Built by build.py with FreeSurfer's own flags,
// as one of FreeSurfer's programs (top.cmake). Called from Python through the oracle-output
// cache by larmorx_validation.freesurfer_unit; the commands: README.md.
//
// Written for LARMORX (Apache-2.0); the binary it builds links FreeSurfer's code and stays on
// the machine that built it.

#include <cerrno>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>

#include "diag.h"
#include "error.h"
#include "matrix.h"
#include "mri.h"
#include "utils.h"
#include "version.h"

#include "vnl/vnl_random.h"

const char *Progname = "fsunit";

namespace {

// ---------------------------------------------------------------- output
std::string hexf(double v)
{
  char b[64];
  snprintf(b, sizeof b, "%a", v);
  return b;
}

std::string decf(double v)
{
  char b[64];
  snprintf(b, sizeof b, "%.17g", v);
  return b;
}

// a JSON number pair: {"hex": "...", "dec": "..."} as a string field
std::string num(double v) { return "{\"hex\": \"" + hexf(v) + "\", \"dec\": \"" + decf(v) + "\"}"; }

std::string quote(const char *s)
{
  std::string out = "\"";
  for (const char *p = s; *p; ++p) {
    unsigned char c = (unsigned char)*p;
    if (c == '"' || c == '\\') {
      out += '\\';
      out += (char)c;
    }
    else if (c < 0x20) {
      char b[8];
      snprintf(b, sizeof b, "\\u%04x", c);
      out += b;
    }
    else
      out += (char)c;
  }
  return out + "\"";
}

[[noreturn]] void fail(const std::string &what)
{
  fprintf(stderr, "fsunit: %s\n", what.c_str());
  exit(2);
}

long to_long(const char *s)
{
  char *end = nullptr;
  errno = 0;
  long v = strtol(s, &end, 0);
  if (errno || !end || *end) fail(std::string("not an integer: ") + s);
  return v;
}

double to_double(const char *s)
{
  char *end = nullptr;
  errno = 0;
  double v = strtod(s, &end);  // decimal or C99 hexadecimal
  if (errno || !end || *end) fail(std::string("not a number: ") + s);
  return v;
}

// ---------------------------------------------------------------- matrices
// Text form: "rows cols" on the first line, then the rows, each value in any form strtof reads
// (the hexadecimal "%a" form keeps a float exact).
MATRIX *read_matrix(const char *path)
{
  FILE *f = fopen(path, "r");
  if (!f) fail(std::string("cannot open ") + path);
  int rows = 0, cols = 0;
  if (fscanf(f, "%d %d", &rows, &cols) != 2 || rows < 1 || cols < 1) fail("bad matrix header");
  MATRIX *m = MatrixAlloc(rows, cols, MATRIX_REAL);
  char tok[128];
  for (int r = 1; r <= rows; r++)
    for (int c = 1; c <= cols; c++) {
      if (fscanf(f, "%127s", tok) != 1) fail("short matrix");
      char *end = nullptr;
      *MATRIX_RELT(m, r, c) = strtof(tok, &end);
      if (!end || *end) fail(std::string("not a number: ") + tok);
    }
  fclose(f);
  return m;
}

void write_matrix(const char *path, const MATRIX *m)
{
  FILE *f = fopen(path, "w");
  if (!f) fail(std::string("cannot write ") + path);
  fprintf(f, "%d %d\n", m->rows, m->cols);
  for (int r = 1; r <= m->rows; r++) {
    for (int c = 1; c <= m->cols; c++) fprintf(f, "%s%a", c > 1 ? " " : "", (double)*MATRIX_RELT(m, r, c));
    fprintf(f, "\n");
  }
  fclose(f);
}

std::string matrix_json(const MATRIX *m)
{
  std::string s = "[";
  for (int r = 1; r <= m->rows; r++) {
    s += r > 1 ? ", [" : "[";
    for (int c = 1; c <= m->cols; c++) s += (c > 1 ? ", " : "") + num((double)*MATRIX_RELT(m, r, c));
    s += "]";
  }
  return s + "]";
}

// ---------------------------------------------------------------- volumes
std::string header_json(MRI *mri)
{
  std::string s = "{";
  s += "\"type\": " + std::to_string(mri->type);
  s += ", \"width\": " + std::to_string(mri->width);
  s += ", \"height\": " + std::to_string(mri->height);
  s += ", \"depth\": " + std::to_string(mri->depth);
  s += ", \"nframes\": " + std::to_string(mri->nframes);
  s += ", \"ras_good_flag\": " + std::to_string(mri->ras_good_flag);
  const struct {
    const char *name;
    double v;
  } fields[] = {{"xsize", mri->xsize}, {"ysize", mri->ysize}, {"zsize", mri->zsize},
                {"x_r", mri->x_r},     {"x_a", mri->x_a},     {"x_s", mri->x_s},
                {"y_r", mri->y_r},     {"y_a", mri->y_a},     {"y_s", mri->y_s},
                {"z_r", mri->z_r},     {"z_a", mri->z_a},     {"z_s", mri->z_s},
                {"c_r", mri->c_r},     {"c_a", mri->c_a},     {"c_s", mri->c_s},
                {"tr", mri->tr},       {"te", mri->te},       {"ti", mri->ti},
                {"flip_angle", mri->flip_angle}};
  for (const auto &f : fields) s += std::string(", \"") + f.name + "\": " + num(f.v);
  MATRIX *v2r = MRIgetVoxelToRasXform(mri);
  s += ", \"vox2ras\": " + matrix_json(v2r);
  MatrixFree(&v2r);
  return s + "}";
}

// ---------------------------------------------------------------- the commands
int cmd_version(int, char **)
{
  printf("{\"function\": \"version\", \"fs_version\": %s, \"build_stamp\": %s, \"compiler\": %s}\n",
         quote(getVersion().c_str()).c_str(), quote(getBuildStamp().c_str()).c_str(),
         quote(__VERSION__).c_str());
  return 0;
}

// MRIread then MRIwrite: FreeSurfer's readers and writers, chosen by the names' suffixes
int cmd_mri_read_write(int argc, char **argv)
{
  if (argc != 2) fail("usage: mri-read-write <in> <out>");
  MRI *mri = MRIread(argv[0]);
  if (!mri) fail(std::string("MRIread failed: ") + argv[0]);
  std::string header = header_json(mri);
  int status = MRIwrite(mri, argv[1]);
  printf("{\"function\": \"MRIread+MRIwrite\", \"write_status\": %d, \"header\": %s}\n", status,
         header.c_str());
  MRIfree(&mri);
  return status == NO_ERROR ? 0 : 1;
}

int cmd_mri_header(int argc, char **argv)
{
  if (argc != 1) fail("usage: mri-header <in>");
  MRI *mri = MRIread(argv[0]);
  if (!mri) fail(std::string("MRIread failed: ") + argv[0]);
  printf("{\"function\": \"MRIread\", \"header\": %s}\n", header_json(mri).c_str());
  MRIfree(&mri);
  return 0;
}

int cmd_matrix_inverse(int argc, char **argv)
{
  if (argc != 2) fail("usage: matrix-inverse <in> <out>");
  MATRIX *m = read_matrix(argv[0]);
  MATRIX *inv = MatrixInverse(m, NULL);
  if (!inv) {
    printf("{\"function\": \"MatrixInverse\", \"result\": null}\n");
    return 1;
  }
  write_matrix(argv[1], inv);
  printf("{\"function\": \"MatrixInverse\", \"result\": %s}\n", matrix_json(inv).c_str());
  MatrixFree(&m);
  MatrixFree(&inv);
  return 0;
}

int cmd_matrix_multiply(int argc, char **argv)
{
  if (argc != 3) fail("usage: matrix-multiply <a> <b> <out>");
  MATRIX *a = read_matrix(argv[0]);
  MATRIX *b = read_matrix(argv[1]);
  MATRIX *c = MatrixMultiply(a, b, NULL);
  if (!c) fail("MatrixMultiply returned NULL");
  write_matrix(argv[2], c);
  printf("{\"function\": \"MatrixMultiply\", \"result\": %s}\n", matrix_json(c).c_str());
  return 0;
}

int cmd_matrix_determinant(int argc, char **argv)
{
  if (argc != 1) fail("usage: matrix-determinant <in>");
  MATRIX *m = read_matrix(argv[0]);
  float det = MatrixDeterminant(m);
  printf("{\"function\": \"MatrixDeterminant\", \"result\": %s}\n", num((double)det).c_str());
  return 0;
}

// VXL's vnl_random (ITK 4.13.2), seeded as vnl_random(seed), then <count> draws of one method
int cmd_vnl_random(int argc, char **argv)
{
  if (argc != 3) fail("usage: vnl-random <seed> <count> lrand32|drand32|drand64|normal|normal64");
  unsigned long seed = (unsigned long)to_long(argv[0]);
  long count = to_long(argv[1]);
  std::string method = argv[2];
  vnl_random rng(seed);
  std::string s = "[";
  for (long i = 0; i < count; i++) {
    if (i) s += ", ";
    if (method == "lrand32")
      s += std::to_string(rng.lrand32());
    else if (method == "drand32")
      s += num(rng.drand32());
    else if (method == "drand64")
      s += num(rng.drand64());
    else if (method == "normal")
      s += num(rng.normal());
    else if (method == "normal64")
      s += num(rng.normal64());
    else
      fail("unknown method " + method);
  }
  printf("{\"function\": \"vnl_random::%s\", \"seed\": %lu, \"values\": %s]}\n", method.c_str(), seed,
         s.c_str());
  return 0;
}

// FreeSurfer's own generator: setRandomSeed(seed), then randomNumber(low, high) <count> times
int cmd_fs_random(int argc, char **argv)
{
  if (argc != 4) fail("usage: fs-random <seed> <count> <low> <high>");
  long seed = to_long(argv[0]);
  long count = to_long(argv[1]);
  double low = to_double(argv[2]), high = to_double(argv[3]);
  setRandomSeed(seed);
  std::string s = "[";
  for (long i = 0; i < count; i++) s += (i ? ", " : "") + num(randomNumber(low, high));
  printf("{\"function\": \"setRandomSeed+randomNumber\", \"seed\": %ld, \"values\": %s]}\n", seed,
         s.c_str());
  return 0;
}

struct Command {
  const char *name;
  int (*run)(int, char **);
  const char *what;
};

const Command COMMANDS[] = {
    {"version", cmd_version, "the build's version, build stamp and compiler"},
    {"mri-read-write", cmd_mri_read_write, "<in> <out>: MRIread, then MRIwrite (header as read)"},
    {"mri-header", cmd_mri_header, "<in>: MRIread, the header and vox2ras"},
    {"matrix-inverse", cmd_matrix_inverse, "<in> <out>: MatrixInverse (float MATRIX)"},
    {"matrix-multiply", cmd_matrix_multiply, "<a> <b> <out>: MatrixMultiply"},
    {"matrix-determinant", cmd_matrix_determinant, "<in>: MatrixDeterminant"},
    {"vnl-random", cmd_vnl_random, "<seed> <count> <method>: vnl_random(seed), count draws"},
    {"fs-random", cmd_fs_random, "<seed> <count> <low> <high>: setRandomSeed, randomNumber"},
};

}  // namespace

int main(int argc, char *argv[])
{
  ErrorInit(NULL, NULL, NULL);
  DiagInit(NULL, NULL, NULL);
  if (argc < 2 || !strcmp(argv[1], "--help") || !strcmp(argv[1], "-h")) {
    fprintf(stderr, "usage: fsunit <command> [arguments]\n");
    for (const auto &c : COMMANDS) fprintf(stderr, "  %-20s %s\n", c.name, c.what);
    return argc < 2 ? 2 : 0;
  }
  for (const auto &c : COMMANDS)
    if (!strcmp(argv[1], c.name)) return c.run(argc - 2, argv + 2);
  fail(std::string("unknown command ") + argv[1]);
}
