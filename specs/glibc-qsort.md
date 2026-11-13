# Spec: glibc 2.35's `qsort` and `qsort_r` (x86-64): the order it leaves, as FreeSurfer 8.2.0's programs get it (`glibc-qsort`)

(spec writer, track D of FreeSurfer's foundations, relay 3, 2026-11-13) The input for a clean-room
implementation, in the main package (`larmorx-core`, Apache-2.0), of the order in which the GNU C
Library 2.35 (Ubuntu 22.04, x86-64: the machine of the official FreeSurfer 8.2.0 build and of the
reference runs) leaves an array after `qsort` or `qsort_r`, **for every comparator**, together with the
exact sequence of comparator calls that produces it.

**Why.** A correct sort is not enough to give FreeSurfer's results. Two elements that the comparator
calls equal are often different (a structure sorted by one field; `+0.0` and `−0.0`; NaNs with
different payloads), and many comparators are not consistent (a three-way float comparison calls a NaN
equal to everything; some of FreeSurfer's comparators never return 0, others return +1 for both orders
of an equal pair, §7). The arrangement then depends on the method's every step, so a replica needs
glibc's. glibc is LGPL-2.1-or-later, so its code is never copied or translated (CLAUDE.md rule 2): this
spec describes the behaviour, from which another agent writes the functions, as for
`specs/glibc-dbl.md` and `specs/glibc-random.md`. The two methods are textbook ones (a top-down merge
sort, Knuth vol. 3 §5.2.4; a quicksort with median-of-three and a final insertion sort, Sedgewick 1978);
this spec adds the exact split, tie, pivot and threshold conventions that give glibc's arrangement.

**Whoever implements it must not open:** glibc's source (`<workspace>/tags/glibc-2.35/`, any other
release or copy; among others `stdlib/msort.c`, `stdlib/qsort.c`), its disassembly, the spec writer's
model (`validation/investigations/glibc-qsort-spec-check/`), `crates-gpl/` and `crates-nc/`,
`docs/parts/`, FreeSurfer's source and the scratch folders of FreeSurfer's parts. It may read this
spec, the publications of §11, the C standard's and POSIX's text on `qsort`, glibc's manual, and build
and run the probe in `validation/investigations/glibc-qsort-probe/` (calls of the C library's functions
only) as a black box (§9). This spec holds the method as mathematics and prose, no code; what black-box
runs confirmed is marked **(observed)**.

---

## 1. Notation

- The array holds `n` elements in **positions** `0, 1, …, n − 1`, each element a block of `s` bytes
  (`s ≥ 1`). `e[p]` is the element in position `p` at the moment it is named; "swap `p` and `q`"
  exchanges the contents of two positions.
- The **comparator** `c(x, y)` takes two elements and returns an integer; only its sign is used. It is
  treated as a black box: no property (antisymmetry, transitivity, `c(x, x) = 0`) is assumed, and every
  rule below holds for any comparator whose result depends only on the two elements' contents.
- A sort's **behaviour** is (a) the arrangement of the bytes after it and (b) the sequence of comparator
  calls, each an ordered pair (first argument, second argument). This spec fixes both. The comparator
  must not depend on the addresses it receives (they are not specified; FreeSurfer's comparators read
  only the contents).
- `⌊v / 2⌋` is the integer part for `v ≥ 0`.

## 2. The interface

`qsort(base, n, s, c)` sorts the `n` elements at `base` with the comparator `c(x, y)`.
`qsort_r(base, n, s, c, ctx)` is the same with a comparator `c(x, y, ctx)` that receives the caller's
context `ctx`, unchanged, in every call **(observed)**; its arrangement and call sequence are those of
`qsort` with the comparator `c(·, ·, ctx)` **(observed**, §9 group 5**)**. Neither returns a value or
reports an error, and neither changes `errno` **(observed**, every path, §9**)**. `n = 0` and `n = 1`
make no comparator call and leave the array as it is.

## 3. Which method: the temporary array

glibc's `qsort` uses the **merge order** (§4), which needs a temporary array of `T` bytes, and falls back
to the **quicksort order** (§5), which needs none, when it cannot have that array:

- `T = n · s` for `s ≤ 32`; `T = 16 n + s` for `s > 32` (two 8-byte pointers per element and one
  spare element: larger elements are sorted through pointers, which changes neither the arrangement nor
  the call sequence, §4) **(observed**: the probe's allocator sees exactly one request of `T` bytes
  during every sort with `T ≥ 1024`, and none for `T < 1024`, §9 group 4**)**.
- `T < 1024`: the merge order, with the temporary array on the stack (no allocation).
- `T ≥ 1024`: let `P` be the page size and `Φ` the number of physical pages, as the operating system
  reports them (`sysconf`; read once per process, at its first sort with `T ≥ 1024`; when the system
  cannot report `Φ`, `Φ = 2⁶³ − 1`). If `⌊T / P⌋ > ⌊Φ / 4⌋` (the temporary array would exceed a quarter
  of the physical memory), the quicksort order, with no allocation. Otherwise one allocation of `T`
  bytes: the merge order if it succeeds, the quicksort order if it fails **(observed**: a refused
  request gives §5's arrangement and calls, §9 group 4**)**. `errno` is kept across the allocation.

On the reference machine `P = 4096` and `Φ = 32,763,452` (124 GiB): the size rule takes the quicksort
order only for `T > 31.2 GiB`. FreeSurfer's largest sorts on its default paths hold one volume (§7: at
most about 128 MiB for a 256³ volume of doubles), so on any machine that can run its programs they
take the merge order. **What larmorx does** (D13: a result that depends on the machine's memory or on
an allocation failure is never reproduced by default): the replicas use the merge order; the quicksort
order is provided as its own function (§8), so that a caller can reproduce a small-memory machine on
request.

## 4. The merge order

The sort of positions `lo, …, lo + m − 1` (`m` elements), written `M(lo, m)`; the whole sort is
`M(0, n)`:

1. If `m ≤ 1`: nothing (no call).
2. `m₁ = ⌊m / 2⌋`, `m₂ = m − m₁`: the **left part** is positions `lo, …, lo + m₁ − 1`, the **right
   part** `lo + m₁, …, lo + m − 1` (for odd `m` the right part is the larger).
3. `M(lo, m₁)`, then `M(lo + m₁, m₂)`: every call of the left part's sort comes before every call of
   the right part's.
4. **Merge.** Let `L₀ … L_{m₁−1}` be the left part's elements in their order after step 3 and
   `R₀ … R_{m₂−1}` the right part's. With `i = j = 0`, while `i < m₁` and `j < m₂`: call `c(Lᵢ, Rⱼ)`
   (the first argument always from the left part); if the result is **≤ 0** the next element of the
   output is `Lᵢ` (`i` increases by one), otherwise `Rⱼ` (`j` increases by one). Then the remaining
   `Lᵢ, …` and then the remaining `Rⱼ, …`, with no further call. The output fills positions
   `lo, …, lo + m − 1` in order.

**(observed**: §9 groups 1 to 5, every element size, comparator and count tried**)**. Properties that
follow:

- For a consistent comparator the order is **stable**: equal elements keep their input order (a tie
  takes the left part's element first).
- A comparator that returns a positive value for both orders of an equal pair (the "ties → +1" kind)
  takes the right part's element first at every merge; one that returns a negative value for both
  takes the left's. Neither is a permutation rule that a stable sort reproduces: the split of
  step 2 decides which elements meet.
- The number of calls depends on the data: at most `m₁ + m₂ − 1` per merge.
- The element size does not matter: elements of more than 32 bytes are ordered through pointers to
  them and placed at the end, with the same calls on the same contents.

**Example** (`n = 5`, elements `a b c d e` in positions 0 to 4): `M(0, 5)` splits into `M(0, 2)` (`a b`)
and `M(2, 3)` (`c d e`), the latter into `M(2, 1)` (`c`) and `M(3, 2)` (`d e`). The calls begin
`c(a, b)`, then `c(d, e)`, then the merge of `c` with the sorted `d e`, then the merge of the sorted
`a b` with the sorted `c d e`.

## 5. The quicksort order (the fallback)

A quicksort phase (only for `n > 4`), then an insertion phase (for every `n ≥ 2`). The threshold is
`τ = 4` elements. Positions are whole numbers; the rules below are exact for every comparator **as
long as every position they name lies in `0, …, n − 1`**. An inconsistent comparator can drive a scan
past the ends of the array, where glibc passes the comparator memory outside the array and may then
write outside it (a known defect of glibc's fallback, Qualys 2024, §11): that case is outside this
spec, and an implementation stops with an error at the first call that would name such a position,
having made exactly the calls before it (D13: never reproduced) **(observed**: the probe stops glibc at
the first call with a pointer outside the array; the calls before it and that point agree with these
rules in every case tried, §9 group 4**)**. The common case: a comparator that returns a negative value
for both orders of an equal pair ("ties → −1") leaves the array in the insertion phase whenever an
element equal to the first minimum is inserted (the scan of step 2 does not stop at position 0), in
243 of the 266 such cases of §9 group 4.

**Quicksort phase** (`n > 4`). A stack of pending ranges, empty at the start; the current range
`lo = 0`, `hi = n − 1`. Repeat:

1. **Pivot.** `mid = lo + ⌊(hi − lo) / 2⌋`. If `c(e[mid], e[lo]) < 0`: swap `mid` and `lo`. Then if
   `c(e[hi], e[mid]) < 0`: swap `mid` and `hi`, and then if `c(e[mid], e[lo]) < 0`: swap `mid` and `lo`
   (this third call is made only when the second swap was made). `mid` is a position; it names the
   pivot's current place and follows it in step 2's swaps.
2. **Partition.** `l = lo + 1`, `r = hi − 1`. Repeat:
   - while `c(e[l], e[mid]) < 0`: `l` increases by one;
   - while `c(e[mid], e[r]) < 0`: `r` decreases by one;
   - if `l < r`: swap `l` and `r`; if `mid = l` then `mid` becomes `r`, else if `mid = r` then `mid`
     becomes `l`; then `l` increases and `r` decreases by one;
   - else if `l = r`: `l` increases and `r` decreases by one, and the partition ends;
   until `l > r` (tested after the first three items).
3. **Next range.** The left part `lo … r` is **small** when `0 ≤ r − lo ≤ τ`, the right part `l … hi`
   when `0 ≤ hi − l ≤ τ` (a negative difference, possible only with an inconsistent comparator, is not
   small):
   - both small: if the stack is empty the quicksort phase ends; otherwise the range last pushed is
     removed from the stack and becomes the current range;
   - only the left part small: `lo = l` (the current range becomes the right part);
   - only the right part small: `hi = r`;
   - neither: if `r − lo > hi − l` (signed), push the left part `(lo, r)` and continue with the right
     part (`lo = l`); otherwise push the right part `(l, hi)` and continue with the left part
     (`hi = r`).

   A current range with `hi < lo` is outside this spec (glibc then computes a pivot outside the array).

**Insertion phase** (every `n ≥ 2`, on the whole array):

1. **The first minimum.** `t = min(n − 1, τ)`; `k = 0`; for `p = 1, …, t` in order: if
   `c(e[p], e[k]) < 0` then `k = p`. If `k ≠ 0`: swap `k` and `0`.
2. **Insertion.** For `p = 2, …, n − 1` in order: `q = p − 1`; while `c(e[p], e[q]) < 0`: `q`
   decreases by one (the element in position `p` stays there during the scan; `q < 0` is outside this
   spec: glibc reads before the array, the Qualys case); then `q` increases by one, and if `q ≠ p` the
   element in position `p` moves to position `q`, the elements of positions `q, …, p − 1` each moving
   up by one.

(For `n ≤ 4` only the insertion phase runs; for `n = 1` it makes no call.) **(observed**: §9 group 4,
the probe refusing the temporary array**)**.

## 6. What is not specified here

`bsearch`, `lfind`, `lsearch` and the C++ library's `std::sort` and `std::stable_sort` (a different
library: FreeSurfer's two `std::sort` calls on its default paths sort keys with a strict total order,
where every correct sort gives the same arrangement; FreeSurfer's one `bsearch` on its default paths
only asks whether a value is present in a sorted list of integers, which every correct binary search
answers alike). glibc releases after 2.35 changed `qsort` (this spec is 2.35's).

## 7. FreeSurfer's calls (what the replicas pass)

About 30 functions on the default paths of FreeSurfer 8.2.0's programs (recon-all's two streams and
fMRIPrep's calls) call `qsort` (none of them `qsort_r`), with elements of 1 to 104 bytes. Their
comparators are of these kinds; each is a black box to this spec, and the replicas pass FreeSurfer's
own:

- **three-way on numbers** (`x < y → −1`, `x > y → +1`, else 0) over floats, doubles, bytes and
  integers: median filters and percentiles of surfaces and volumes, normalisation of curvatures,
  sorted lists of vertex and face numbers, prime factors. With floats the order of `+0.0` and `−0.0`,
  and of NaNs (equal to everything under this comparator), follows §4: a median can be `−0.0` or
  `+0.0` by it;
- the same with the last test reversed (`x < y → −1`, `x = y → 0`, else +1: a NaN against anything is
  +1);
- **three-way on one field of a structure**, ties between different structures common: label
  probabilities, voxels sorted by value, segmentation indices, cluster summaries (104-byte elements:
  the pointer path of §4);
- **comparators that never return 0**: ties give −1 for both orders (eigenvalues sorted by
  magnitude; edges of equal length and equal first vertex) or +1 for both orders (samples of equal
  prior).

## 8. What the clean room provides

In `larmorx-core` (a clean-room file of the GNU family, listed in the clean-room audit's tables):

- **the merge order** (§4): sorts a mutable slice of elements in place with a comparator closure
  returning an ordering or a signed integer, making exactly §4's calls in §4's order; elements of any
  size; a temporary buffer of the slice's size;
- **the quicksort order** (§5): the same interface, no buffer; it returns an error instead of leaving
  the slice (§5's out-of-range cases), with the slice then in an unspecified arrangement;
- **the choice of §3** as a third entry point that takes `T`'s inputs, `P`, `Φ` and whether the
  allocation succeeds, for a caller that wants glibc's rule; the replicas call the merge order;
- no threads, no global state, the same result on every platform (no arithmetic beyond indices).

## 9. Validation plan (black box)

The probe `validation/investigations/glibc-qsort-probe/glibc_qsort_probe.c` (calls of the system's
`qsort` and `qsort_r` only, and an allocator that forwards to the C library's except for the one
request it is told to refuse; build with any C compiler on a machine with glibc 2.35:
`cc -O2 -o glibc_qsort_probe glibc_qsort_probe.c`) reads cases `s n mode seed keyrange api fail` and
prints, per case, the number of comparator calls, a hash of the call sequence, a hash of the sorted
bytes, the number of requests of `T` bytes and whether `errno` was kept (or, when glibc passes the
comparator a pointer outside the array, the calls and hash before that call and the word `outside`). Its header defines the
elements (from a seeded generator, §10), the comparator kinds (`3way`, `tie+` (ties → +1), `tie-`
(ties → −1), `hash` (an arbitrary, inconsistent result from the two elements' indices), `f3`, `f2`
(floats including NaNs with three payloads, `±0`, `±∞`, a subnormal), `d3`, `d2` (the same in
double)) and both hashes. The clean room passes when, for the same cases, its counts and hashes equal
the probe's:

1. **Small arrays, every kind:** `n = 0 … 64`, each comparator kind, element sizes 1, 2, 4, 8, 12, 16,
   24, 32 (and 4 / 8 for the float kinds), several seeds, key ranges 1, 2, 3 and 1000.
2. **Sizes around the stack limit:** `T` from 1000 to 1100 bytes (`n · s` on both sides of 1024).
3. **Large arrays:** `n` = 1000, 4096, 10,007, 100,000 and 1,000,000 with key ranges 3 and 1000.
4. **The fallback:** cases with `fail` = 1 and `T ≥ 1024` against the quicksort order: `n` = 0 to 64
   with 1024- and 1040-byte elements (so that `T ≥ 1024` for every `n`), floats and doubles with
   `n` = 128 to 300, and the large arrays up to 10,007 elements. Where §5 leaves the array, the probe
   prints `<calls> <hash> outside` (it stops glibc at the first call with a pointer outside the array,
   §9's probe header): the clean room's error must come at the same point, after the same calls (the
   same count and call-sequence hash).
5. **Large elements and `qsort_r`:** sizes 33, 40, 72, 104, 160 (`T = 16 n + s`), and `api` =
   `qsort_r` for a subset, which must give the `qsort` results.
6. **Golden cases** (CLAUDE.md rule 10), once the functions are reachable from a registered tool: one
   case per comparator kind with `n` = 37, both orders.

**Reference lines (observed**, the probe, glibc 2.35-0ubuntu3.15, 2026-11-13): input line, then output
line (`calls`, call-sequence hash, sorted-bytes hash, requests of `T`, `errno` kept):

| Case | Output |
|---|---|
| `4 1000 3way 1 10 qsort 0` | `8504 22b3708447d84162 985da63e885ae5b7 1 1` |
| `4 1000 3way 1 10 qsort 1` | `9540 1b768952e1c84670 985da63e885ae5b7 1 1` |
| `12 1000 3way 2 10 qsort 0` | `8465 cd3c5a72a7733a84 1331cd34ebdbf954 1 1` |
| `12 1000 tie+ 2 10 qsort 0` | `8494 4bcd1b66e61a1fa1 3c92d2e1fd018aac 1 1` |
| `16 1000 tie- 3 10 qsort 0` | `8544 c4bcc9a6525d3b2e 31c543dbc89ca695 1 1` |
| `40 1000 hash 4 3 qsort 0` | `6916 f672409666b84a2b 25ec899552189d77 1 1` |
| `40 1000 hash 4 3 qsort_r 0` | `6916 f672409666b84a2b 25ec899552189d77 1 1` |
| `40 1000 hash 4 3 qsort 1` | `9621 2ea5d1118facf2b7 2b1e1211fa2aad7f 1 1` |
| `4 1000 f3 5 12 qsort 0` | `5997 a8107a0175eee3f5 563fcc1dcef7a859 1 1` |
| `4 1000 f2 5 12 qsort 0` | `6127 5669dc3121f908a8 62a8788fd4a06369 1 1` |
| `8 1000 d3 6 12 qsort 0` | `5879 8cc18a31990e2485 edf4496b7e528b61 1 1` |
| `8 1000 d2 6 12 qsort 1` | `9262 c90bb93c4a7bba71 d7f7923ba5e9ce51 1 1` |
| `104 100 3way 7 5 qsort 0` | `531 0461360aad912fdb 84af1b838617c27b 1 1` |
| `1 255 3way 8 4 qsort 0` | `1612 5923cd99459aacc0 d1fe3ddfe7376da5 0 1` |
| `4 100000 3way 9 1000 qsort 0` | `1536229 8bdb84d427f725aa cd915035a4583feb 1 1` |

The spec writer's check (`validation/investigations/glibc-qsort-spec-check/`, closed) ran groups 1 to
5 against a model written from this text **(observed**, 2026-11-13**)**: 16,544 cases (13,780 small
arrays of every kind and size, 232 around the stack limit, 40 large arrays up to 100,000 elements,
1,202 fallback cases, 1,290 with large elements and `qsort_r`), **all identical**: the call counts,
the call sequences and the arrangements; in the 286 cases where §5 leaves the array, both stop at the
same call after the same calls.

## 10. The probe's elements and hashes

- **Generator** (splitmix64, Steele, Lea and Flood 2014): a 64-bit state `σ` (the case's seed); each
  draw sets `σ = σ + 0x9E3779B97F4A7C15`, then `z = σ`, `z = (z XOR (z >> 30)) · 0xBF58476D1CE4E5B9`,
  `z = (z XOR (z >> 27)) · 0x94D049BB133111EB`, and returns `z XOR (z >> 31)` (all modulo 2⁶⁴).
- **Element `i`** (`i = 0, …, n − 1`, one draw `rᵢ` each, then the draws for its filler bytes): key
  `kᵢ = rᵢ mod keyrange`; all `s` bytes 0, then: `s = 1`: byte 0 is `kᵢ mod 256`; `s = 2`: bytes 0 and
  1 hold `kᵢ mod 65536` (little-endian); `s ≥ 4`: bytes 0 to 3 hold `kᵢ mod 2³²` (little-endian) and,
  for `s ≥ 8`, bytes 4 to 7 hold `i`, and from byte 8 on, in steps of 8 bytes, the little-endian bytes
  of one further draw each (the last step truncated to the bytes left). The float kinds take instead
  the value of entry `kᵢ mod 12` of the table (bit patterns, binary32 for `f*`, binary64 for `d*`):
  quiet NaN, `−0`, `+0`, 1, −1, `+∞`, `−∞`, NaN with payload 1, negative quiet NaN, 2, the smallest
  subnormal, 1 (`7fc00000 80000000 00000000 3f800000 bf800000 7f800000 ff800000 7fc00001 ffc00000
  40000000 00000001 3f800000`, and in double `7ff8000000000000 8000000000000000 0 3ff0000000000000
  bff0000000000000 7ff0000000000000 fff0000000000000 7ff8000000000001 fff8000000000000
  4000000000000000 1 3ff0000000000000`).
- **Keys read by the comparators:** byte 0 (`s = 1`), the 16-bit (`s = 2`) or 32-bit (`s ≥ 4`)
  little-endian unsigned integer at byte 0. `3way`: `−1, +1, 0` for `<, >, =`; `tie+`: `−1` if `<`,
  else `+1`; `tie-`: `+1` if `>`, else `−1`. `hash` (s ≥ 8): with `a`, `b` the indices in bytes 4 to 7
  of the first and second argument, `z = h ⊕ (a · 0x9E3779B97F4A7C15) ⊕ (b · 0xC2B2AE3D27D4EB4F)`
  (mod 2⁶⁴, `h` = seed XOR `0x5DEECE66D`), then splitmix64's two mixing steps and final XOR as above,
  and the result `(z mod 3) − 1`. `f3`/`d3`: `x < y → −1`, `x > y → +1`, else 0 (IEEE comparisons:
  false with a NaN); `f2`/`d2`: `x < y → −1`, `x = y → 0`, else +1.
- **Hashes** (FNV-1a 64): `h₀ = 14695981039346656037`, then for each byte `β`,
  `h = (h XOR β) · 1099511628211 mod 2⁶⁴`. The call sequence: per call, the first `min(s, 8)` bytes of
  the first argument, then those of the second. The arrangement: the `n · s` bytes after the sort.

## 11. References

- D. E. Knuth, *The Art of Computer Programming*, vol. 3 (2nd ed.), §5.2.4 (merging, merge sort) and
  exercise 5.2-10 (rearranging records in place from a sorted table of pointers).
- R. Sedgewick, "Implementing Quicksort programs", Comm. ACM 21 (1978) 847–857: median-of-three, the
  explicit stack with the larger part pushed, the final insertion sort below a threshold.
- Qualys Security Advisory, "qsort() memory corruption in the GNU C Library" (2024-01-30): glibc's
  fallback with a nontransitive comparator, every release from 1992 to 2.38.
- ISO/IEC 9899 (C), §7.22.5.2 (`qsort`; no stability or comparator-error requirement); the GNU C
  Library manual, "Array Sort Function"; POSIX.1-2017 `qsort`.
- G. L. Steele, D. Lea and C. H. Flood, "Fast splittable pseudorandom number generators", OOPSLA 2014
  (splitmix64).
