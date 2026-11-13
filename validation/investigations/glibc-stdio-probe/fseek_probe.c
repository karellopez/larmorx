// SPDX-License-Identifier: Apache-2.0
// glibc's fseek, rewind and ftell measured as a black box (the C library only; no glibc or
// FreeSurfer code): each line prints what a call returned, errno after it (5 before each
// call), ftell, the end-of-file and error flags, and what a following read or write sees.
//
//     gcc -O0 -o fseek_probe fseek_probe.c && ./fseek_probe <empty work directory>
#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

static void put(const char *name, const char *bytes, size_t n)
{
  FILE *f = fopen(name, "wb");
  fwrite(bytes, 1, n, f);
  fclose(f);
}

static void show(const char *name)
{
  FILE *f = fopen(name, "rb");
  int c;
  printf(" file=[");
  while ((c = fgetc(f)) != EOF) {
    if (c >= 0x20 && c < 0x7f) putchar(c);
    else printf("\\x%02x", c);
  }
  printf("]");
  fclose(f);
}

static long size_of(const char *name)
{
  struct stat st;
  return stat(name, &st) == 0 ? (long)st.st_size : -1;
}

static void state(FILE *f) { printf(" tell=%ld eof=%d err=%d", ftell(f), feof(f) != 0, ferror(f) != 0); }

static void seek(const char *what, FILE *f, long off, int whence)
{
  errno = 5;
  int r = fseek(f, off, whence);
  int e = errno;
  printf("%s: r=%d errno=%d", what, r, e);
  state(f);
}

int main(int argc, char **argv)
{
  if (argc != 2 || chdir(argv[1])) {
    fprintf(stderr, "usage: fseek_probe <empty work directory>\n");
    return 2;
  }
  char b[16];
  FILE *f;

  put("s", "abcdef", 6);
  f = fopen("s", "r");
  fread(b, 1, 3, f);
  seek("set 2 after 3 read", f, 2, SEEK_SET);
  printf(" getc=%c\n", getc(f));
  seek("cur 1", f, 1, SEEK_CUR);
  printf(" getc=%c\n", getc(f));
  seek("end -2", f, -2, SEEK_END);
  printf(" getc=%c\n", getc(f));
  seek("set 100", f, 100, SEEK_SET);
  printf(" getc=%d", getc(f));
  state(f);
  printf("\n");
  seek("set 0 after eof", f, 0, SEEK_SET);
  printf(" getc=%c\n", getc(f));
  while (getc(f) != EOF) {}
  seek("set -1 at eof", f, -1, SEEK_SET);
  printf("\n");
  seek("cur -100 at eof", f, -100, SEEK_CUR);
  printf("\n");
  seek("whence 3 at eof", f, 0, 3);
  printf("\n");
  seek("end 0", f, 0, SEEK_END);
  printf(" getc=%d", getc(f));
  state(f);
  printf("\n");
  seek("end -7", f, -7, SEEK_END);
  printf("\n");
  seek("end 3", f, 3, SEEK_END);
  printf("\n");
  errno = 5;
  rewind(f);
  printf("rewind: errno=%d", errno);
  state(f);
  printf(" getc=%c\n", getc(f));
  fclose(f);

  // the error flag: a write on a stream open for reading
  f = fopen("s", "r");
  errno = 5;
  int w = fputc('x', f);
  printf("fputc on r: r=%d errno=%d", w, errno);
  state(f);
  printf("\n");
  seek("set 1 with error", f, 1, SEEK_SET);
  printf(" getc=%c\n", getc(f));
  errno = 5;
  rewind(f);
  printf("rewind with error: errno=%d", errno);
  state(f);
  printf("\n");
  fclose(f);

  // writing: a gap after the end, an overwrite, pending output and a failed seek
  f = fopen("w", "w");
  fwrite("abc", 1, 3, f);
  seek("w: set 5 after abc", f, 5, SEEK_SET);
  fwrite("x", 1, 1, f);
  printf(" size before close=%ld", size_of("w"));
  fclose(f);
  show("w");
  printf("\n");
  f = fopen("w", "w");
  fwrite("abcdef", 1, 6, f);
  seek("w: set 2 after abcdef", f, 2, SEEK_SET);
  printf(" size=%ld", size_of("w"));
  fwrite("X", 1, 1, f);
  seek("w: cur 0", f, 0, SEEK_CUR);
  fclose(f);
  show("w");
  printf("\n");
  f = fopen("w", "w");
  fwrite("abc", 1, 3, f);
  seek("w: set -1 pending abc", f, -1, SEEK_SET);
  printf(" size=%ld", size_of("w"));
  seek(" w: whence 7 pending", f, 0, 7);
  printf(" size=%ld", size_of("w"));
  seek(" w: cur -9 pending", f, -9, SEEK_CUR);
  printf(" size=%ld", size_of("w"));
  seek(" w: end 0", f, 0, SEEK_END);
  printf(" size=%ld", size_of("w"));
  fclose(f);
  printf("\n");

  // r+: read, seek, write; write, seek, read
  put("rw", "abcdef", 6);
  f = fopen("rw", "r+");
  fread(b, 1, 2, f);
  seek("r+: cur 0 after 2 read", f, 0, SEEK_CUR);
  fputc('Z', f);
  seek(" r+: cur 0 after write", f, 0, SEEK_CUR);
  printf(" getc=%c", getc(f));
  seek(" r+: set 0", f, 0, SEEK_SET);
  memset(b, 0, sizeof b);
  printf(" read=%zu [%s]", fread(b, 1, 10, f), b);
  state(f);
  fclose(f);
  printf("\n");

  // append: a seek does not move the writes
  put("ap", "abc", 3);
  f = fopen("ap", "a");
  seek("a: set 0", f, 0, SEEK_SET);
  fputc('Q', f);
  seek(" a: cur 0", f, 0, SEEK_CUR);
  fclose(f);
  show("ap");
  printf("\n");
  put("ap", "abc", 3);
  f = fopen("ap", "a+");
  seek("a+: set 0", f, 0, SEEK_SET);
  printf(" getc=%c", getc(f));
  fputc('Q', f);
  state(f);
  seek(" a+: set 1", f, 1, SEEK_SET);
  printf(" getc=%c", getc(f));
  fclose(f);
  show("ap");
  printf("\n");

  // a directory opened for reading
  mkdir("dir", 0700);
  f = fopen("dir", "r");
  if (f) {
    seek("dir: set 344", f, 344, SEEK_SET);
    errno = 5;
    size_t n = fread(b, 1, 4, f);
    printf(" fread=%zu errno=%d", n, errno);
    state(f);
    seek(" dir: end 0", f, 0, SEEK_END);
    seek(" dir: set 0", f, 0, SEEK_SET);
    fclose(f);
  } else {
    printf("dir: fopen failed errno=%d", errno);
  }
  printf("\n");

  // an empty file
  put("e", "", 0);
  f = fopen("e", "r");
  seek("empty: set 5", f, 5, SEEK_SET);
  printf(" getc=%d", getc(f));
  state(f);
  seek(" empty: cur -5", f, -5, SEEK_CUR);
  printf(" getc=%d", getc(f));
  state(f);
  fclose(f);
  printf("\n");

  // whether a failed seek writes the pending output (the size read before ftell)
  for (int k = 0; k < 3; k++) {
    f = fopen("w", "w");
    fwrite("abc", 1, 3, f);
    errno = 5;
    int r = k == 0 ? fseek(f, -1, SEEK_SET) : k == 1 ? fseek(f, 0, 7) : fseek(f, -9, SEEK_CUR);
    int e = errno;
    printf("%sw: failed seek %d: r=%d errno=%d size=%ld", k ? " " : "", k, r, e, size_of("w"));
    fclose(f);
  }
  printf("\n");

  // ungetc and a seek
  f = fopen("s", "r");
  getc(f);
  getc(f);
  ungetc('Q', f);
  printf("ungetc: tell=%ld", ftell(f));
  seek(" ungetc: cur 0", f, 0, SEEK_CUR);
  printf(" getc=%c", getc(f));
  fclose(f);
  printf("\n");
  return 0;
}
