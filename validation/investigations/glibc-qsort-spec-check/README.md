# The spec writer's check of `specs/glibc-qsort.md`

**Closed to the clean-room implementer of that spec** (GNU's family in the clean-room audit): its
author read glibc 2.35's source to write the spec. `model.py` follows the spec's text section by
section (§3's temporary-array size, §4's merge order, §5's quicksort order with its out-of-range
stop, §10's elements, comparators and hashes); `check.py` runs the open probe
(`../glibc-qsort-probe/`) on the spec's §9 groups and compares every output line with the model's.

    cc -O2 -o <s>/glibc_qsort_probe ../glibc-qsort-probe/glibc_qsort_probe.c
    python -P check.py <s>/glibc_qsort_probe [--quick]

Groups (the spec's §9): 1 small arrays (`n` = 0 to 64, sizes 1 to 32 bytes, every comparator kind,
key ranges 1, 2, 3, 1000, two seeds), 2 the stack limit (`T` around 1024 bytes, merge and fallback),
3 large arrays (1,000 to 100,000 elements), 4 the fallback (`fail` = 1: 1024- and 1040-byte elements
for `n` = 0 to 64, floats and doubles at `n` = 128 to 300, large arrays to 10,007), 5 large elements
(33 to 160 bytes, `T = 16 n + s`) and `qsort_r`. A line compares the calls, the call-sequence hash,
the sorted-bytes hash, the requests of `T` and `errno`; where both leave the array (§5), the calls
and the hash up to that point.

Results (2026-11-13, the reference machine, glibc 2.35-0ubuntu3.15, 40 s; log
`<workspace>/oracles/logs/glibc-qsort-spec-check.log`): **all identical**, 16,544 cases:

    group 1: 13780 cases, 13780 identical (0 of them leaving the array at the same call)
    group 2: 232 cases, 232 identical (16 of them leaving the array at the same call)
    group 3: 40 cases, 40 identical (0 of them leaving the array at the same call)
    group 4: 1202 cases, 1202 identical (269 of them leaving the array at the same call)
    group 5: 1290 cases, 1290 identical (1 of them leaving the array at the same call)

The cases that leave the array are the fallback's with the `tie-` and `hash` comparators (the
spec's §5).
