// SPDX-License-Identifier: Apache-2.0
// An LD_PRELOAD probe for x86-64 Linux: every SSE instruction that raises the invalid-operation
// exception (a NaN made from operands that are not NaN: 0/0, inf - inf, 0 * inf, sqrt of a
// negative number; also float-to-integer conversions out of range) is recorded once per
// address, with a count, without changing the program's results: the exception is unmasked,
// the SIGFPE handler records the instruction's address, masks the exception and single-steps
// the instruction (the trap flag), and the SIGTRAP handler unmasks it again. At exit the sites
// are written to $NAN_TRAP_DIR/<pid>.txt as "<offset in the executable> <count>", after the
// command line, for addr2line.
#define _GNU_SOURCE
#include <fenv.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ucontext.h>
#include <unistd.h>

#define MAX_SITES 8192
static uintptr_t sites[MAX_SITES];
static unsigned long counts[MAX_SITES];
static unsigned char arith[MAX_SITES];
static int last_site;
static int nsites;
static uintptr_t exe_lo, exe_hi;
// NAN_TRAP_ARM=1: after each SSE add, sub, mul, div or sqrt that made a NaN, the destination's
// lanes that hold x86's default NaN (sign set) are given AArch64's default NaN (sign clear), so
// the program computes as an ARM CPU would with this binary's instruction order.
static int arm_mode;
static __thread int pend_reg = -1, pend_wide;
static unsigned long flips;

static void record(uintptr_t ip) {
  for (int i = 0; i < nsites; i++)
    if (sites[i] == ip) {
      counts[i]++;
      last_site = i;
      return;
    }
  last_site = -1;
  if (nsites < MAX_SITES) {
    sites[nsites] = ip;
    last_site = nsites;
    counts[nsites++] = 1;
  }
}

static void on_fpe(int sig, siginfo_t *si, void *ctx) {
  (void)sig;
  (void)si;
  ucontext_t *uc = ctx;
  uintptr_t ip = (uintptr_t)uc->uc_mcontext.gregs[REG_RIP];
  record(ip);
  pend_reg = -1;
  if (arm_mode) {
    const unsigned char *b = (const unsigned char *)ip;
    int pre = 0, rex = 0, i = 0;
    while (b[i] == 0x66 || b[i] == 0xF2 || b[i] == 0xF3) pre = b[i++];
    if ((b[i] & 0xF0) == 0x40) rex = b[i++];
    if (b[i] == 0x0F) {
      unsigned char op = b[i + 1], modrm = b[i + 2];
      if (op == 0x51 || op == 0x58 || op == 0x59 || op == 0x5C || op == 0x5E) {
        pend_reg = ((modrm >> 3) & 7) | ((rex & 4) ? 8 : 0);
        pend_wide = (pre == 0xF2 || pre == 0x66);  // sd, pd: 64-bit lanes
        if (last_site >= 0) arith[last_site] = 1;
      }
    }
  }
  uc->uc_mcontext.fpregs->mxcsr |= 0x80;   // mask invalid
  uc->uc_mcontext.fpregs->mxcsr &= ~0x01u; // clear its flag
  uc->uc_mcontext.gregs[REG_EFL] |= 0x100; // single-step the instruction
}

static void on_trap(int sig, siginfo_t *si, void *ctx) {
  (void)sig;
  (void)si;
  ucontext_t *uc = ctx;
  if (pend_reg >= 0) {
    uint32_t *e = uc->uc_mcontext.fpregs->_xmm[pend_reg].element;
    for (int l = 0; l < 4; l += pend_wide ? 2 : 1) {
      if (pend_wide) {
        if (e[l] == 0 && e[l + 1] == 0xFFF80000u) { e[l + 1] = 0x7FF80000u; flips++; }
      } else if (e[l] == 0xFFC00000u) { e[l] = 0x7FC00000u; flips++; }
    }
    pend_reg = -1;
  }
  uc->uc_mcontext.fpregs->mxcsr &= ~0x80u; // unmask invalid again
  uc->uc_mcontext.fpregs->mxcsr &= ~0x01u;
  uc->uc_mcontext.gregs[REG_EFL] &= ~0x100;
}

static void exe_range(void) {
  char exe[4096];
  ssize_t n = readlink("/proc/self/exe", exe, sizeof exe - 1);
  if (n <= 0) return;
  exe[n] = 0;
  FILE *f = fopen("/proc/self/maps", "r");
  if (!f) return;
  char line[8192];
  while (fgets(line, sizeof line, f)) {
    unsigned long lo, hi;
    if (strstr(line, exe) && sscanf(line, "%lx-%lx", &lo, &hi) == 2) {
      if (!exe_lo || lo < exe_lo) exe_lo = lo;
      if (hi > exe_hi) exe_hi = hi;
    }
  }
  fclose(f);
}

static void dump(void) {
  const char *dir = getenv("NAN_TRAP_DIR");
  if (!dir || !nsites) return;
  char path[4096];
  snprintf(path, sizeof path, "%s/%d.txt", dir, (int)getpid());
  FILE *f = fopen(path, "w");
  if (!f) return;
  FILE *c = fopen("/proc/self/cmdline", "r");
  if (c) {
    int ch;
    while ((ch = fgetc(c)) != EOF) fputc(ch ? ch : ' ', f);
    fclose(c);
  }
  fprintf(f, "\nflips %lu\n", flips);
  for (int i = 0; i < nsites; i++) {
    if (sites[i] >= exe_lo && sites[i] < exe_hi)
      fprintf(f, "0x%lx %lu%s\n", (unsigned long)(sites[i] - exe_lo), counts[i], arith[i] ? " arith" : "");
    else
      fprintf(f, "abs:0x%lx %lu\n", (unsigned long)sites[i], counts[i]);
  }
  fclose(f);
}

__attribute__((constructor)) static void init(void) {
  if (!getenv("NAN_TRAP_DIR")) return;
  exe_range();
  arm_mode = getenv("NAN_TRAP_ARM") != NULL;
  struct sigaction sa;
  memset(&sa, 0, sizeof sa);
  sa.sa_flags = SA_SIGINFO;
  sa.sa_sigaction = on_fpe;
  sigaction(SIGFPE, &sa, 0);
  sa.sa_sigaction = on_trap;
  sigaction(SIGTRAP, &sa, 0);
  atexit(dump);
  feenableexcept(FE_INVALID);
}
