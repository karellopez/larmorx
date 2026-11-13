// SPDX-License-Identifier: Apache-2.0
// A guard allocator for black-box runs (LD_PRELOAD): once a program calls guard_malloc_enable()
// (the probe does in each child process, after the fork), every allocation gets pages of its own,
// with an inaccessible page right after its last byte (MALLOC_GUARD_ unset or "over": a read or
// write past the end dies of a segmentation fault, but for the at most 15 bytes that keep the
// start 16-byte aligned) or right before its first byte (MALLOC_GUARD_=under: a read before the
// start dies). Every other byte of its pages holds a fill byte, MALLOC_PERTURB_'s value (glibc's
// own variable, 0 to 255; 0x5a when it is unset); new memory from malloc holds the fill byte's
// complement, as glibc's MALLOC_PERTURB_ does; calloc's is zero; freed memory is unmapped. The
// comparison loop beside this file (compare.py) runs each call in both modes: a call whose result
// depends on memory the program does not own (outside an array, never written, freed) gives
// another result, or dies, in one of them. Before the switch, and for memory allocated before it,
// glibc's allocator serves. For single-threaded runs (OMP_NUM_THREADS=1). Written for LARMORX; it
// holds no other program's code.
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>

extern void *__libc_malloc(size_t);
extern void *__libc_calloc(size_t, size_t);
extern void *__libc_realloc(void *, size_t);
extern void *__libc_memalign(size_t, size_t);
extern void __libc_free(void *);

#define PAGE 4096
#define SLOTS (1u << 21)

struct block {
  void *base;     // the mapping
  size_t length;  // its length
  size_t size;    // the bytes asked for
};

static int on;
static void *key[SLOTS];  // the live guarded pointers (open addressing)
static struct block blk[SLOTS];
static void *const GONE = (void *)1;

void guard_malloc_enable(void) { on = 1; }

static size_t hash(const void *p) { return (size_t)(((uintptr_t)p >> 4) * 0x9E3779B97F4A7C15ULL) & (SLOTS - 1); }

static void remember(void *p, struct block b)
{
  for (size_t i = hash(p);; i = (i + 1) & (SLOTS - 1))
    if (key[i] == NULL || key[i] == GONE) {
      key[i] = p;
      blk[i] = b;
      return;
    }
}

static struct block *find(void *p)
{
  for (size_t i = hash(p); key[i] != NULL; i = (i + 1) & (SLOTS - 1))
    if (key[i] == p) return &blk[i];
  return NULL;
}

static int fill_byte(void)
{
  static int fill = -1;
  if (fill < 0) {
    const char *s = getenv("MALLOC_PERTURB_");
    fill = s && *s ? (int)(strtol(s, NULL, 0) & 0xff) : 0x5a;
  }
  return fill;
}

static int under(void)
{
  static int mode = -1;
  if (mode < 0) {
    const char *s = getenv("MALLOC_GUARD_");
    mode = s && strcmp(s, "under") == 0;
  }
  return mode;
}

static void *guarded(size_t n, size_t align, int zero)
{
  if (n == 0) n = 1;
  if (align < 16) align = 16;
  size_t body = (n + align + PAGE - 1) / PAGE * PAGE;
  size_t length = body + 2 * PAGE;
  char *base = mmap(NULL, length, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
  if (base == MAP_FAILED) {
    errno = ENOMEM;
    return NULL;
  }
  mprotect(base, PAGE, PROT_NONE);
  mprotect(base + PAGE + body, PAGE, PROT_NONE);
  char *start = base + PAGE, *end = start + body;
  int f = fill_byte();
  memset(start, f, body);
  char *user;
  if (under())
    user = start;  // page-aligned: any alignment up to a page
  else {
    uintptr_t u = (uintptr_t)(end - n);
    u -= u % align;
    user = (char *)u;
  }
  memset(user, zero ? 0 : (f ^ 0xff), n);
  struct block b = {base, length, n};
  remember(user, b);
  return user;
}

void *malloc(size_t n) { return on ? guarded(n, 16, 0) : __libc_malloc(n); }

void *calloc(size_t k, size_t n)
{
  if (!on) return __libc_calloc(k, n);
  if (n && k > (size_t)-1 / n) {
    errno = ENOMEM;
    return NULL;
  }
  return guarded(k * n, 16, 1);
}

void free(void *p)
{
  if (!p) return;
  struct block *b = on ? find(p) : NULL;
  if (b) {
    munmap(b->base, b->length);
    for (size_t i = hash(p); key[i] != NULL; i = (i + 1) & (SLOTS - 1))
      if (key[i] == p) {
        key[i] = GONE;
        break;
      }
    return;
  }
  __libc_free(p);
}

void *realloc(void *p, size_t n)
{
  if (!p) return malloc(n);
  struct block *b = on ? find(p) : NULL;
  if (!b) return __libc_realloc(p, n);
  if (n == 0) {
    free(p);
    return NULL;
  }
  size_t old = b->size;
  void *q = guarded(n, 16, 0);
  if (!q) return NULL;
  memcpy(q, p, old < n ? old : n);
  free(p);
  return q;
}

void *reallocarray(void *p, size_t k, size_t n)
{
  if (n && k > (size_t)-1 / n) {
    errno = ENOMEM;
    return NULL;
  }
  return realloc(p, k * n);
}

void *memalign(size_t align, size_t n) { return on ? guarded(n, align, 0) : __libc_memalign(align, n); }

int posix_memalign(void **out, size_t align, size_t n)
{
  if (align == 0 || (align & (align - 1)) || align % sizeof(void *)) return EINVAL;
  void *p = memalign(align, n);
  if (!p) return ENOMEM;
  *out = p;
  return 0;
}

void *aligned_alloc(size_t align, size_t n) { return memalign(align, n); }
void *valloc(size_t n) { return memalign(PAGE, n); }
void *pvalloc(size_t n) { return memalign(PAGE, (n + PAGE - 1) / PAGE * PAGE); }

size_t malloc_usable_size(void *p)
{
  if (!p) return 0;
  struct block *b = on ? find(p) : NULL;
  if (b) return b->size;
  static size_t (*libc_usable)(void *);
  if (!libc_usable) libc_usable = (size_t (*)(void *))dlsym(RTLD_NEXT, "malloc_usable_size");
  return libc_usable ? libc_usable(p) : 0;
}
