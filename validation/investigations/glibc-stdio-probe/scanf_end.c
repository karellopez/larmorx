// SPDX-License-Identifier: Apache-2.0
// glibc's sscanf and fscanf at and near the end of their input, measured as a black box (the C
// library only; no glibc or FreeSurfer code): for each format and input, the return value and the
// errno the call leaves (5 before it); for fscanf on a file holding the same bytes also the
// position after the call and the end-of-file flag. One line per case:
//
//     <s|f> <format id> <input in hex or -> <return> <errno> [<ftell> <feof>]
//
//     gcc -O0 -o scanf_end scanf_end.c && mkdir work && ./scanf_end work
#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

static const char *inputs[] = {
    "", " ", "\n", "  \n ", "1", "1 ", "1\n", "1 2", "1 2 ", "1 2 3", "1 2 3\n", "1 2 3 4", "x",
    "1 x", "1e99", "1e99 ", "1e99 2", "2 1e99", "2 1e99 ", "nan", "nan ", "-nan", "inf", "inf ",
    "infinity", "infinity ", "nan 1", "inf 1", "infin", "ab", "ab ", "ab 1e99", "ab 1 2", "1e-50",
    "1e-50 ", "1e400", "1e400 ", "99999999999", "99999999999 ", "1e", "1e ", "0x", "0x1p3", "nan(",
    "-", "- ", ".", "AutoAlign 1 2", "Matrix", "Matrix ", "Matrix 1e99", "a\nb", "5\n\n"};

// the formats, by id; each is called with enough targets
static int call(int s, FILE *fp, const char *in, int id)
{
  int i1, i2, i3;
  float f1, f2, f3;
  double d1;
  char w[256], w2[256];
#define CALL(...) (s ? sscanf(in, __VA_ARGS__) : fscanf(fp, __VA_ARGS__))
  switch (id) {
    case 0: return CALL("%d %d %d\n", &i1, &i2, &i3);
    case 1: return CALL("%f %f   ", &f1, &f2);
    case 2: return CALL("%f  ", &f1);
    case 3: return CALL("\n");
    case 4: return CALL("%s", w);
    case 5: return CALL("%f", &f1);
    case 6: return CALL("%lf", &d1);
    case 7: return CALL("%d", &i1);
    case 8: return CALL("%f %d", &f1, &i1);
    case 9: return CALL("%s %f %f", w, &f1, &f2);
    case 10: return CALL("%*s %d", &i1);
    case 11: return CALL("%f %f %f", &f1, &f2, &f3);
    case 12: return CALL("%d %d", &i1, &i2);
    case 13: return CALL("%s %s", w, w2);
    case 14: return CALL(" %f", &f1);
    case 15: return CALL("%f\n", &f1);
    case 16: return CALL("x%d", &i1);
    case 17: return CALL("%lf %lf", &d1, &d1);
    case 18: return CALL("%*d %f", &f1);
    case 19: return CALL("%5s %d", w, &i1);
  }
  return -9;
}

int main(int argc, char **argv)
{
  if (argc != 2 || chdir(argv[1])) {
    fprintf(stderr, "usage: scanf_end <empty work directory>\n");
    return 2;
  }
  int ni = sizeof inputs / sizeof inputs[0];
  for (int id = 0; id < 20; id++) {
    for (int k = 0; k < ni; k++) {
      const char *in = inputs[k];
      char hex[128] = "-";
      for (size_t j = 0; in[j]; j++) sprintf(hex + 2 * j, "%02x", (unsigned char)in[j]);
      errno = 5;
      int r = call(1, NULL, in, id);
      printf("s %d %s %d %d\n", id, hex, r, errno);
      FILE *fp = fopen("in", "wb");
      fwrite(in, 1, strlen(in), fp);
      fclose(fp);
      fp = fopen("in", "rb");
      errno = 5;
      r = call(0, fp, in, id);
      int e = errno;
      printf("f %d %s %d %d %ld %d\n", id, hex, r, e, ftell(fp), feof(fp) != 0);
      fclose(fp);
    }
  }
  return 0;
}
