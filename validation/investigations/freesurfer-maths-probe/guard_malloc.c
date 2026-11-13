// SPDX-License-Identifier: Apache-2.0
// A guard allocator for black-box runs (LD_PRELOAD): once a program calls guard_malloc_enable()
// (the probe does in each child process, after the fork), every allocation gets pages of its own,
// its bytes surrounded by 64 bytes of a fill byte on each side and by inaccessible pages beyond
// them. A program that reads a little outside an array it allocated reads the fill byte (so two
// runs with different fill bytes differ), and one that reads further dies of a segmentation
// fault: the comparison loop beside this file (compare.py) finds the calls whose results depend on
// memory the program does not own by running them so. The fill byte is MALLOC_PERTURB_'s value
// (glibc's own variable, 0 to 255; 0x5a when it is unset); new memory from malloc holds the fill
// byte's complement, as glibc's MALLOC_PERTURB_ does; calloc's is zero. Freed memory is unmapped.
// Before the switch, and for memory allocated before it, glibc's allocator serves. For
// single-threaded runs (OMP_NUM_THREADS=1). Written for LARMORX; it holds no other program's code.
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

#define PAD 64
#define PAGE 4096
#define SLOTS (1u << 21)

static int on;
static void *slot[SLOTS];  // the live guarded pointers (open addressing)
static void *const GONE = (void *)1;

void guard_malloc_enable(void) { on = 1; }

static size_t hash(const void *p) { return (size_t)(((uintptr_t)p >> 4) * 0x9E3779B97F4A7C15ULL) & (SLOTS - 1); }

static void remember(void *p)
{
  for (size_t i = hash(p);; i = (i + 1) & (SLOTS - 1))
    if (slot[i] == NULL || slot[i] == GONE) {
      slot[i] = p;
      return;
    }
}

static int forget(void *p)
{
  for (size_t i = hash(p); slot[i] != NULL; i = (i + 1) & (SLOTS - 1))
    if (slot[i] == p) {
      slot[i] = GONE;
      return 1;
    }
  return 0;
}

static int known(void *p)
{
  for (size_t i = hash(p); slot[i] != NULL; i = (i + 1) & (SLOTS - 1))
    if (slot[i] == p) return 1;
  return 0;
}

struct meta {
  void *base;     // the mapping
  size_t length;  // its length
  size_t size;    // the bytes asked for
  size_t spare;
};

static int fill_byte(void)
{
  static int fill = -1;
  if (fill < 0) {
    const char *s = getenv("MALLOC_PERTURB_");
    fill = s && *s ? (int)(strtol(s, NULL, 0) & 0xff) : 0x5a;
  }
  return fill;
}

static void *guarded(size_t n, size_t align, int zero)
{
  if (n == 0) n = 1;
  if (align < 16) align = 16;
  size_t need = sizeof(struct meta) + PAD + n + PAD + align;
  size_t body = (need + PAGE - 1) / PAGE * PAGE;
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
  uintptr_t u = (uintptr_t)(end - PAD - n);
  u -= u % align;
  char *user = (char *)u;
  struct meta *m = (struct meta *)(user - PAD - sizeof(struct meta));
  m->base = base;
  m->length = length;
  m->size = n;
  memset(user, zero ? 0 : (f ^ 0xff), n);
  remember(user);
  return user;
}

static struct meta *meta_of(void *p) { return (struct meta *)((char *)p - PAD - sizeof(struct meta)); }

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
  if (on && forget(p)) {
    struct meta *m = meta_of(p);
    munmap(m->base, m->length);
    return;
  }
  __libc_free(p);
}

void *realloc(void *p, size_t n)
{
  if (!p) return malloc(n);
  if (!(on && known(p))) return __libc_realloc(p, n);
  if (n == 0) {
    free(p);
    return NULL;
  }
  void *q = guarded(n, 16, 0);
  if (!q) return NULL;
  size_t old = meta_of(p)->size;
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
  if (on && known(p)) return meta_of(p)->size;
  static size_t (*libc_usable)(void *);
  if (!libc_usable) libc_usable = (size_t (*)(void *))dlsym(RTLD_NEXT, "malloc_usable_size");
  return libc_usable ? libc_usable(p) : 0;
}
