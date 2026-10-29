# HANDOFF: antsAI and antsAffineInitializer, memory and speed (2026-10-28/29)

Branch: the agent worktree `agent-a876beb406db40b5c`, last commit `4e286e4` (from `e50fcf9`).
Records re-made at the clean commit `9b9fe9aa5232`; benchmark at `ec2910367e8d` (same code).

## Already in the branch (no action, for awareness)

- **Generated tables** (`README.md`, `docs/validation/README.md`): regenerated with
  `python -m larmorx_validation tables` in commit `4e286e4`, because `validation/tests/test_tables.py`
  fails otherwise; the only change is the ants-ai counts (131 -> 132 cases; 75 -> 76 of 76 compared,
  74 -> 75 as built). At the merge, run `python -m larmorx_validation tables --site ../larmorx_documentation`
  so the site's `data/status.yml` gets the same counts.
- `docs/benchmarks/README.md` lists the new report and its command (index of the benchmark reports).
- The shared core function `larmorx_core::itk_real::round_to_i64` (ITK's floor and round half up for
  every tool) now rounds below 2^51 by one addition of 1.5*2^52 (the general method above); tested
  equal on 600,000 values; golden checksums unchanged, smoke tier 2,549 passed.
- `ImageMetricv4.fixed/moving` are now `metric::MetricImage<R, D>` (`Real(Arc<Image<R,D>>)` for
  antsRegistration, unchanged arithmetic; `Float(Arc<Image<f32,D>>)` for antsAI and
  antsAffineInitializer). antsRegistration's code paths are the same operations (construction sites
  `.into()`, `m.fixed.real()` for its gradient images).

## docs/next-steps.md

1. In "The order from here", replace item 1:

```
1. **antsAI and antsAffineInitializer: memory and speed** (a small part: their metric's images in
   float, the initializer's speed; decision log 2026-10-27).
```

with:

```
1. **antsAI and antsAffineInitializer: memory and speed: done** (2026-10-28): the metric's images
   in float as in ANTs (peaks at or below ANTs': 95 and 91 MiB against 92 to 96 and 89 on the
   memory test's head, 676 against 688 on the 7 T; the two strict xfails removed); Mattes on the
   sampled points with each point's fixed side once and the moving side in batches; ITK-21's
   second search sharing what it can, the two at once with threads. antsAI 1.8 to 2 times as fast
   as ANTs at one thread; antsAffineInitializer at ANTs' time in `reproduce` on the 7 T, 1.2 times
   it on the head, half of it by default; 4 to 10 times as fast at eight threads; bit for bit as
   before ([benchmark](benchmarks/ants-ai.md), [findings](findings/ants-ai.md), "Memory and speed
   (2026-10-28)").
```

2. In the follow-ups, the "Memory, the rest" item: replace

```
  **antsAI and antsAffineInitializer still above** (127 and 110 MiB against 92 and 91 on a
  128 × 128 × 96 head; +40 and +156 MiB on the 7 T): their metric holds the images in double, ANTs
  in float; store them in float (the next small part; strict-xfail cases in
  `tests/parity/test_memory_peak.py`); the AFNI programs, mcflirt and `resample_series` remain to
```

with

```
  **antsAI and antsAffineInitializer: done** (2026-10-28: the images in float, 95 and 91 MiB on
  the head against ANTs' 92 to 96 and 89, 676 against 688 on the 7 T; the test's strict xfails
  removed); the AFNI programs, mcflirt and `resample_series` remain to
```

3. Replace the item

```
- **antsAffineInitializer's speed**: larmorx 277 s against ANTs' 73 s at one thread on a
  128 × 128 × 96 head, 1212 s against 332 s on the 7 T (the D13 notice of ITK-21 runs every local
  search twice; the rest is the metric's speed): the next small part.
```

with

```
- **antsAffineInitializer's speed: done** (2026-10-28; on the new machine, where ANTs takes 33.5 s
  on the head and 252 s on the 7 T): `reproduce` at one thread 40.0 s and 254.5 s (the ITK-21
  notice's second search; the search itself takes 0.6 of ANTs' time), the default mode 20.9 s and
  124.7 s, eight threads 8.7 s and 51.6 s (`reproduce`). A further gain at one thread would need
  SIMD over a batch's points (each point's operations unchanged): not attempted.
- **The sampled Mattes path for antsRegistration** (a possible follow-up): `evaluate_sampled` is
  taken only for float-stored images (antsAI, antsAffineInitializer); antsRegistration's sampled
  Mattes stages (`Regular`/`Random`, no fixed transform, a linear stage) could take it too, with
  its own records re-made.
```

4. In "Figures": `the site's pages for antsRegistration, antsAI and the HDF5 files say their benchmark is pending` -> `the site's pages for antsRegistration and the HDF5 files say their benchmark is pending` (antsAI's now exists; see the site text below).

## CLAUDE.md, "Where things are"

In the **Next:** paragraph, replace `a small part for antsAI and antsAffineInitializer (memory and speed), the clean-room fixes, the`
with `the clean-room fixes (antsAI and antsAffineInitializer's memory and speed: done, 2026-10-28), the`.

In the "In place" bullet **Memory:**, after `` `tests/parity/test_memory_peak.py` `` add:
`, antsAI and antsAffineInitializer included since 2026-10-28: their images in float`.

## Decision log

No new decision (the rules settled everything). Optionally, under 2026-10-27 "Memory similar to the
original's", one line: `2026-10-28: antsAI and antsAffineInitializer meet it (the metric's images in
float); the memory test's last two strict xfails removed.`

## The defect register (docs/findings/upstream-defects.md, docs/api/upstream-defects.md)

No new defect (none of ANTS-190 to 194 or ITK-116 to 120 used). No register change.

## docs/validation/golden.md

No change: the golden checksums are unchanged (`pytest tests/golden` passed; no case added: no new
public function).

## The site (larmorx_documentation, `content/tools/ants-ai.md`, section "Speed")

Replace `No benchmark report yet: the benchmark of antsAI and antsAffineInitializer is pending.` with:

```
antsAI takes about half of ANTs' time at one thread (17.5 s against 31.2 s on a 128 × 128 × 96 head, 79 s against 156 s on the 7 T MP2RAGE and the MNI template) and a sixth or less at eight threads, with ANTs' memory; antsAffineInitializer takes ANTs' time at one thread when it reproduces ANTs (its D13 check of ITK-21 searches twice), half of it in the default mode, and a quarter to a tenth at eight threads. Every transform ANTs' byte for byte: [the benchmark](repo:docs/benchmarks/ants-ai.md).
```

(The page's header line of links may also get `benchmark <a href="{{ repo('docs/benchmarks/ants-ai.md') }}">ants-ai</a>` as the N4 page has.)

## docs/parts

The brief, this handoff and the final report, as usual.
