# glibc 2.35's `qsort` and `qsort_r`, as a black box

The probe behind the **(observed)** marks of `specs/glibc-qsort.md` (the order in which the C
library's sort leaves an array, which FreeSurfer's programs depend on: medians, percentiles, ranked
labels). **Open to the clean-room implementer of that spec**: the file holds only calls of the C
library's `qsort` and `qsort_r` by name, an allocator that forwards to the C library's (refusing one
request when told to), a generator and a hash; no glibc code. The implementer may read, build and
run it (black-box runs of the original, `docs/porting-guide.md` §1).

| File | What it is |
|---|---|
| `glibc_qsort_probe.c` | reads cases `s n mode seed keyrange api fail` from standard input and prints, per case, the comparator calls, a hash of the call sequence, a hash of the sorted bytes, the requests of the temporary array's size and whether `errno` was kept; or, when the C library hands the comparator a pointer outside the array (the fallback with an inconsistent comparator), the calls and hash before that call and `outside` (the comparator then leaves `qsort` with `longjmp`, before anything is written outside the array). Its header defines the elements, the comparator kinds and the hashes (the spec's §10). Build: `cc -O2 -o glibc_qsort_probe glibc_qsort_probe.c` on a machine with glibc 2.35 (the reference machine: Ubuntu 22.04, x86-64) |

Example: `printf '4 1000 3way 1 10 qsort 0\n' | ./glibc_qsort_probe` prints
`8504 22b3708447d84162 985da63e885ae5b7 1 1` (glibc 2.35-0ubuntu3.15, 2026-11-13); the spec's §9 has
15 such reference lines.

How it forces the fallback: the executable defines `malloc` (glibc's own calls reach it: glibc
supports replacing the allocator), forwarding to `__libc_malloc` except for the one request of the
temporary array's size `T` (the spec's §3) when `fail` is 1, which it refuses; it also counts the
requests of `T` bytes, which shows the stack limit (no request for `T < 1024`).
