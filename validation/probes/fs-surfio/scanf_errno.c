// SPDX-License-Identifier: Apache-2.0
// glibc 2.35's sscanf and fscanf as a black box (relay 12 of fs.surfio): the return value and
// errno (5 before each call) for the directives the surface library's text readers use (text
// that must match, white space, %d, %f, %s, the suppressed forms), at and near the end of the
// input; the model in larmorx-freesurfer-surf's src/scanf.rs reproduces every line (its test
// glibc_measurements). Calls glibc only; no glibc code.
//
//     gcc -O0 -o scanf_errno scanf_errno.c && ./scanf_errno
#include <errno.h>
#include <stdio.h>
#include <string.h>

static void t(const char *in, const char *fmt)
{
  int a = -7, b = -7;
  char s[64] = "unset";
  float f = -7;
  errno = 5;
  int r;
  if (strstr(fmt, "%s")) r = sscanf(in, fmt, s, &a, &b);
  else if (strstr(fmt, "%f")) r = sscanf(in, fmt, &f, &a);
  else r = sscanf(in, fmt, &a, &b);
  printf("[%s] [%s] r=%d errno=%d a=%d b=%d s=%s f=%g\n", in, fmt, r, errno, a, b, s, f);
}

static void t3(const char *in, const char *fmt)
{
  float f = -7, g = -7, h = -7;
  errno = 5;
  int r = sscanf(in, fmt, &f, &g, &h);
  printf("[%s] [%s] r=%d errno=%d f=%g g=%g h=%g\n", in, fmt, r, errno, f, g, h);
}

int main(void)
{
  t("POINTS", "POINTS %d float\n");
  t("POINTS ", "POINTS %d float\n");
  t("POINT", "POINTS %d float\n");
  t("POINTSX", "POINTS %d float\n");
  t("POINTS 5", "POINTS %d float\n");
  t("POINTS 99999999999 float", "POINTS %d float\n");
  t("12", "%*d %*f %*f %*f %f\n");
  t("12 1 2 3", "%*d %*f %*f %*f %f\n");
  t("99999999999 1e99 2 3 4", "%*d %*f %*f %*f %f\n");
  t("1 2 3 4 1e99", "%*d %*f %*f %*f %f\n");
  t("x 2 3 4 5", "%*d %*f %*f %*f %f\n");
  t("FIELD", "FIELD %*s %d\n");
  t("FIELD a", "FIELD %*s %d\n");
  t("FIELD a 3", "FIELD %*s %d\n");
  t("curv", "%s %d %d %*s\n");
  t("curv 1", "%s %d %d %*s\n");
  t("curv 1 2", "%s %d %d %*s\n");
  t("", "%d");
  t("", "x%d");
  t(" ", "%*s");
  t("1e99", "%f %d");
  t("1e99", "%f x");
  t("1e99", "%f\n");
  t("1e99 ", "%f %d");
  t("1e99 x", "%f %d");
  t("inf", "%f %d");
  t("nan", "%f %d");
  t("-nan", "%f %d");
  t("infinity", "%f %d");
  t("1", "%f %d");
  t("nan(1", "%f %d");
  t("in", "%f %d");
  t3("1e99", "%f %f %f");
  t3("1 2", "%f %f %f");
  // fscanf on a file: each call starts afresh at the end of the file
  FILE *fp = fopen("scanf_errno.tmp", "w");
  fputs("1e99", fp);
  fclose(fp);
  fp = fopen("scanf_errno.tmp", "r");
  float f = -7, g = -7;
  errno = 5;
  int r = fscanf(fp, "%f", &f);
  printf("fscanf %%f r=%d errno=%d f=%g\n", r, errno, f);
  r = fscanf(fp, "%f", &g);
  printf("fscanf %%f at the end r=%d errno=%d g=%g\n", r, errno, g);
  errno = 5;
  r = fscanf(fp, "x");
  printf("fscanf x at the end r=%d errno=%d\n", r, errno);
  errno = 5;
  r = fscanf(fp, "\n");
  printf("fscanf \\n at the end r=%d errno=%d\n", r, errno);
  fclose(fp);
  remove("scanf_errno.tmp");
  return 0;
}
