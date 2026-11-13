# glibc 2.35's qsort and qsort_r: the clean room against the C library (black box)

The clean room `larmorx_core::glibc_qsort` (`crates/larmorx-core/src/glibc_qsort/`, FreeSurfer's
foundations, the GNU family, 2026-11-13) implements [`specs/glibc-qsort.md`](../../specs/glibc-qsort.md):
the order in which the GNU C Library 2.35 (Ubuntu 22.04, x86-64) leaves an array after `qsort` or
`qsort_r`, for every comparator, with the exact sequence of comparator calls that produces it.
FreeSurfer 8.2.0's programs depend on it wherever elements the comparator calls equal differ
(medians and percentiles of floats with `±0` and NaNs, structures sorted by one field, comparators
that never return 0). Written from the spec only; checked against the C library's own functions (the
probe `glibc_qsort_probe`, a program that calls `qsort` and `qsort_r` by name) run as a black box
(`validation/investigations/freesurfer-cleanroom-random-spline/`, `compare_glibc_qsort.py`; result
`result-2026-11-13-glibc-qsort.json`). **Open to clean-room implementers**: everything here comes from
the spec and from black-box runs.

## The result

From the clean commit `5c9faa6e` (2026-11-13, the main machine, glibc 2.35-0ubuntu3.15,
`result-2026-11-13-glibc-qsort.json`; the probe built with Ubuntu's `cc` 11.4 at `-O2`, SHA-256
`bd9ba0854b35…`; the clean room's probe `3782c613ca90…`). Every case is a line `s n mode seed keyrange
api fail` (the spec's §9); both programs print the number of comparator calls, an FNV-1a hash of the
call sequence (the first 8 bytes of both arguments of every call, in order), a hash of the sorted
bytes, the requests of the temporary array and whether `errno` was kept, or `<calls> <hash> outside`
where glibc's fallback would pass the comparator memory outside the array. The clean room runs each
case twice, through its typed functions (`[u8; s]` elements) and through its byte functions:

| What (spec §9) | Cases | Identical to the C library (typed, bytes) | Where glibc leaves the array | With an allocation |
|---|---|---|---|---|
| the spec's reference lines | 15 | **15** (and equal to the spec's numbers) | 0 | 14 |
| group 1, small arrays: `n` = 0 … 64, every comparator kind (`3way`, `tie+`, `tie-`, `hash`, `f3`, `f2`, `d3`, `d2`), sizes 1, 2, 4, 8, 12, 16, 24, 32 bytes, seeds 1 to 3, key ranges 1, 2, 3 and 1000 (12 for the float kinds), the allocation granted and refused; and sizes 5, 6, 7, 20, 28 (the byte functions' element-by-element path) | 55,900 | **55,900, 55,900** | 631 | 5,401 |
| group 2, the stack limit: temporary arrays of 1000 to 1100 bytes (`n · s`, or `16 n + s` above 32 bytes), every kind, granted and refused | 1,760 | **1,760, 1,760** | 199 | 1,153 |
| group 3, large arrays: `n` = 1000, 4096, 10,007, 100,000 and 1,000,000, seven size and kind pairs (4 bytes `3way` and `f3`, 8 `hash` and `d2`, 12 `tie+`, 16 `tie-`, 40 `3way`), key ranges 3 and 1000 | 70 | **70, 70** | 0 | 70 |
| group 4, the fallback (the allocation refused, `T` ≥ 1024): `n` = 0 … 64 with 1024- and 1040-byte elements, floats and doubles with `n` = 128 … 300, arrays up to 10,007 elements | 4,546 | **4,546, 4,546** | 467 | 3,567 |
| group 5, large elements (33, 40, 72, 104, 160 bytes: `T = 16 n + s`), `qsort` and `qsort_r`, granted and refused | 10,720 | **10,720, 10,720** | 176 | 1,072 |

No difference anywhere: 72,996 cases and the 15 reference lines, every call sequence, every
arrangement, every allocation request; `qsort_r`'s context arrived unchanged in every call, and its
results are `qsort`'s. Where glibc's fallback leaves the array (1,473 cases, all with an inconsistent
comparator: `tie-`, `hash`, the float kinds with NaNs), the clean room's error comes at the same
point, after the same calls (the same count and call-sequence hash): 1,407 times at a call that would
name a position outside the array, 66 times at a range whose end is before its start (§5's last
sentence: below). The run made 22 runs of the probe (batches of up to 4,000 cases), 20 of them served
by the oracle-output cache from an earlier run of the same batches.

The unit tests check the spec's 15 reference lines, §4's example (the calls of a five-element sort),
the fallback's error on three equal elements under "ties → −1" (four calls, then position −1), §3's
choice (the stack limit, the size rule, an unknown physical memory, a refused allocation), `qsort_r`'s
context, the typed functions against the byte ones on every path, and 18 golden checksums (each
comparator kind in both orders, and the path through references above 32 bytes, `n` = 37) on every
platform.

## Speed

The same machine (shared with other work, load about 10), best of five. The sorts alone, a plain
comparator (the probe's kinds without counting or hashing), the same elements: glibc's `qsort` timed
by `qsort_bench.c` (a plain program that calls it), the clean room's merge order by `cleanroom-probe
qsort-bench`; every pair left the same arrangement (the sorted bytes' hash):

| Case (`s n mode`) | glibc's `qsort` | Clean room, typed | Clean room, bytes | glibc ÷ typed |
|---|---|---|---|---|
| 4 bytes, 10⁶, `3way` (key range 1000) | 0.0796 s | 0.0805 s | 0.0828 s | 0.99 |
| 4 bytes, 10⁶, `f3` (floats with NaNs, ±0) | 0.0448 s | 0.0506 s | 0.0511 s | 0.89 |
| 8 bytes, 10⁶, `d2` | 0.0388 s | 0.0426 s | 0.0440 s | 0.91 |
| 12 bytes, 10⁶, `tie+` | 0.1109 s | 0.0802 s | 0.0801 s | 1.38 |
| 40 bytes, 10⁶, `hash` | 0.1874 s | 0.2261 s | 0.2747 s | 0.83 |
| 104 bytes, 10⁵, `3way` | 0.0097 s | 0.0113 s | 0.0143 s | 0.86 |
| 4 bytes, 10⁷, `3way` | 0.9646 s | 0.9833 s | 1.0030 s | 0.98 |

The whole probe programs (elements made, every call counted and hashed): 0.15 s against 0.16 s for
10⁶ four-byte elements, 1.77 s against 1.99 s for 10⁷.

So the clean room sorts at glibc's speed, not faster: the spec fixes every comparator call and its
order, and these comparators' own branches (data-dependent, unpredictable for random keys) are most of
the time (about 4 ns per call). What was tried, with the same calls: the array and its copy take
turns as source and destination (no copying back at any level); the byte functions sort the common
sizes (1, 2, 4, 8, 12, 16, 24, 32 bytes) as arrays, through the typed loops (twice as fast as the
element-by-element path, which remains for other sizes); elements above 32 bytes are ordered through
references (as glibc through pointers: two per element at the peak, 16 bytes each, glibc's `16 n + s`)
and moved once, cycle by cycle, with one spare element. A merge that selects without a branch was
measured and not kept: 10 to 20 % faster on random keys, 40 to 85 % slower on keys with many ties
and through references (ties are the common case in FreeSurfer's medians of volumes). A replica's comparator, a closure inlined into the
sort, also saves glibc's indirect call per comparison, which these measurements (one comparator for
every kind, chosen per call) do not show.

## What the black-box runs added to the spec

1. **A reversed range** (§5, step 3, "a current range with `hi < lo` is outside this spec (glibc
   then computes a pivot outside the array)"): in all 66 cases where the clean room meets one, the
   probe stops at the same point after the same calls, so glibc's first call of that range (the
   pivot's `c(e[mid], e[lo])`) already passes the comparator memory outside the array. The clean room
   stops there with `QuickOrderError::ReversedRange`, before any call of that range.
2. **`n` = 0 and 1 still allocate**: with a temporary array of 1024 bytes or more (elements of 1024
   or 1040 bytes: `T = 16 n + s`), glibc asks for it even for 0 and 1 elements, though it then makes no
   call (`1040 0 3way 1 1 qsort 1` prints `0 cbf29ce484222325 cbf29ce484222325 1 1`); §3's rule as
   written gives exactly this (its size test does not look at `n`), and §2's sentence on `n` ≤ 1 is
   about calls only.
3. **The probe's elements of 3 bytes** are not defined by §10 (it gives `s = 1`, `s = 2` and
   `s ≥ 4`): the probe writes 4 key bytes into each 3-byte element (the next element's generation
   overwrites the fourth; the last one reaches the spare byte after the array) and its comparators read
   4 bytes, so the comparator's result depends on a neighbour. The comparisons use no 3-byte case;
   sizes 5, 6, 7, 20 and 28 (all `≥ 4`, defined) were added for the clean room's element-by-element
   path.

## larmorx's own limits

None found. The functions take a mutable slice and a comparator closure returning an `Ordering` or a
signed integer (`Comparison`): `merge_order` (§4; `qsort` and `qsort_r` are it under C's names, the
context being what the closure captures), `quick_order` (§5, returning `QuickOrderError` where glibc
would leave the array; the slice is then in an unspecified arrangement), `choose` and
`qsort_as_glibc` (§3, from `Machine`: the page size, the physical pages or `None`, whether the
allocation succeeds; `Machine::REFERENCE` is the spec's reference machine), each also over blocks of
bytes (`_bytes`). `merge_order` needs `T: Copy` (it copies elements into its buffer); `quick_order`
takes any `T`. `bsearch`, `lfind`, `lsearch` and C++'s sorts are outside the spec (§6) and are not
provided.
