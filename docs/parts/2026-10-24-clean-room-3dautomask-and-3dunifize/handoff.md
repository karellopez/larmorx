# Handoff: Clean-room 3dAutomask and 3dUnifize

The agent's text for the shared documents, verbatim; integrated and harmonized by the orchestrator at the merge.

# HANDOFF: the clean-room originals of 3dAutomask and 3dUnifize

Branch `worktree-agent-a4282c736dbd9fe7c` (larmorx) and `afni-masks-original-evidence` (site
clone in `<scratch>/site`). Exact text for each shared document; the orchestrator integrates
and harmonizes. Numbers are the records' (`docs/validation/afni-automask.md`,
`afni-unifize.md`, both from clean commits).

## Things to know before integrating

1. **Record names (convention of 3dTshift):** the clean-room original now owns
   `docs/validation/afni-automask.{md,json}` and `afni-unifize.{md,json}` (`python -m
   larmorx_validation parity afni-automask` defaults to `--implementation original`); the
   replica's records were moved (`git mv`, content unchanged) to
   `afni-automask-replica.{md,json}` and `afni-unifize-replica.{md,json}` (`--implementation
   replica`). Their text still shows the old command line (`parity afni-automask --tier
   standard`); **regenerate both replica records after merging the parallel replica part**
   (which implements AFNI-41 and AFNI-48 in the replica): with this branch's suites, the
   replica's `options/nbhrs-40` and `options/T2-T2-T2` expect the default to refuse
   (`fix_refuses="AFNI-41"`/`"AFNI-48"`), as the specs decide; with main's replica (before that
   part) these two standard-tier cases fail in the replica suites, as expected (the smoke tier
   does not contain them). The pytest files follow:
   `tests/parity/test_afni_{automask,unifize}.py` (original) and `..._replica.py` (renamed).
   If the parallel part also regenerated `docs/validation/afni-automask.{md,json}` /
   `afni-unifize.{md,json}` as the replica's records, rename those to `-replica` at the merge.
2. **CORE-MATH preload:** `scripts/build_crmath_preload.sh` now also builds `tanhf` and `cbrtf`
   (3dUnifize's squash and `-EPI`). The shared `oracles/core-math-preload` was **not** rebuilt
   (the parallel agent may be using it); this branch's unifize record used a build of the same
   script in `oracles/core-math-preload-tanhf` (`LARMORX_CRMATH_PRELOAD`). Rebuild the shared
   one at the merge (`scripts/build_crmath_preload.sh`); then the unifize suite's explanatory
   pass (AFNI with correctly rounded maths) finds it by default; without `tanhf` in its
   `BUILD_INFO.txt` the pass reports "not run". `oracles/core-math-preload-tanhf` can then be
   archived.
3. **`larmorx_core::defects`** gained `AFNI_41` and `AFNI_48` (the parallel replica part may add
   the same constants: keep one of each).
4. **AFNI-10 in the original's record:** fMRIPrep's 3dAutomask call on ds005454's BOLD reference
   reads 3dUnifize's output, which carries AFNI's header extension; AFNI takes the geometry from
   it (7 digits). The original ignores it (as 3dTshift's original does): data bit-identical,
   header differs in the 7th digit; the harness verifies by running AFNI on the input without
   its extension (the header is then the original's byte for byte). Verdict on the site:
   *deliberate correction (D13)*.
5. **Golden checksums:** the 8 new cases recorded by `scripts/record_golden.py` from 12cad72
   after the parity smoke tier passed (2231 tests, 1 h 28 min; ddba1c9); every other case
   unchanged. After the refusal alignment (4cc2835), `record_golden.py --check`: 149 of 149
   agree. Merging with the decisions part (which re-recorded its own cases) needs
   `scripts/orchestration/merge_golden.py` or a re-record from the merge.
6. **Black-box tools committed** (rule of 2026-10-23): `validation/investigations/afni-masks-original/`
   (README, `blackbox.py` to run and compare AFNI, the original and the replica, the phantoms,
   `strip_extensions.py` for AFNI-10, `coremath_checksums.c`, `timing.py` and its
   `timings-2026-10-24.json`). `validation/investigations/README.md` has no index to update.
7. **`crates-gpl/LICENSES/NOTICE-larmorx.txt`** refreshed as a byte copy of `NOTICE` (which gained
   CORE-MATH's `tanhf` and `cbrtf`): crates-gpl's test `the_apache_notices_are_current_copies`
   asked for it. Nothing else under `crates-gpl/` was opened or changed.
8. **Informal timings** (not a benchmark report; a shared machine, load 17 to 22): in
   `docs/findings/afni-masks-original.md` and on both site pages as speed charts (the evidence's
   `speed` from the timings JSON, provenance under `benchmarks`). The benchmark reports remain a
   follow-up.
9. **The site's T1w example** of 3dUnifize: its default comparisons named one without shown
   panels, which stopped its viewer (fixed, faa0046; the follow-up for `common.finish` is in the
   site section below).
10. **The rule-B refusals as the replica words them** (the coordinator's spec update, main
   60c963d, copied into this branch's specs unchanged): AFNI-41 (the last `-nbhrs`, whether or
   not `-NN1/2/3` follows; a non-number reads as 0; refused before any input is read; in
   `reproduce` the notice after the outputs, "acts as with 17" or the `-NN` value) and AFNI-48
   (three or more `-T2` without `-EPI`; refused after the scan; in `reproduce` the notice after
   the output) now give the corrected replica's messages and notices byte for byte, checked by
   black-box runs of `.claude/worktrees/agent-af2255d42fc4f41d1/crates-gpl/target/release/larmorx-gpl`
   (25 command lines, both modes). The same comparison found the AFNI-45 and `-EPI` AFNI-01
   notices worded differently from the replica's (already in main's replica): matched too. One
   difference remains, outside this part and shared with 3dTshift: when the input does not
   exist, the replica's default mode adds the system's error (`Can't open dataset '<f>': <f>: No
   such file or directory (os error 2)`), the originals (`crate::input`) print AFNI's line in
   both modes; the records compare `reproduce` only, where both print AFNI's.
11. **Harness changes shared with 3dTshift's suites** (`validation/.../parity/`): `afni_replica.py`
   (an `implementation` per suite, a float tolerance for an original, the AFNI-10 explainer, the
   CORE-MATH preload pass, fMRIPrep's own outputs held to an original's threshold) and
   `defects.py` (a refusal's quoted message, which may contain "; ", is no longer read as a list of
   "other changes" in the D13 summary: the replica records regenerated at the merge get the clean
   line too). 3dTshift's suites run through the same code; the parity smoke tier passed at 12cad72
   (2231 tests) and the two tools' standard records were made from the final harness (2a009d1).
12. **Site:** branch `afni-masks-original-evidence`, one commit `cf87fef` on `a53145d` (fetch from
   `<scratch>/site`). Built with `--main-repo` this worktree: 0 problems; number check 109 of 109
   (3dAutomask) and 153 of 153 (3dUnifize); viewer readouts 32 and 30, all agree; a re-export is
   byte-identical; review screenshots in `<scratch>/shots/` (desktop 1440 and phone 390, light and
   dark).

## README.md

Replace the two rows of the tools table:

```markdown
| 3dAutomask (brain mask) | `larmorx afni 3dAutomask`, `lx.afni.automask` | clean-room original; exact replica in `larmorx-gpl` | original: [105 cases](docs/validation/afni-automask.md), 80 of 81 compared cases that write outputs bit-identical to AFNI (the other: AFNI-10's header, its data bit-identical), fMRIPrep's call on every oracle run's BOLD reference included; replica: [81 of 81](docs/validation/afni-automask-replica.md) bit-identical |
| 3dUnifize (intensity uniformization) | `larmorx afni 3dUnifize`, `lx.afni.unifize` | clean-room original; exact replica in `larmorx-gpl` | original: [97 cases](docs/validation/afni-unifize.md), none failing: 32 of 72 compared cases that write outputs bit-identical to AFNI, the other 40 within 1e-5 (at most 8.7e-8 of the largest value) and bit-identical to AFNI run with correctly rounded maths (glibc's `tanhf` in AFNI's squash), fMRIPrep's call on every oracle run included; 4 to 25 times faster than AFNI (informal timings); replica: [72 of 72](docs/validation/afni-unifize-replica.md) bit-identical at 1, 2 and 6 threads |
```

In the roadmap, item 1, replace "the clean-room originals of 3dAutomask and 3dUnifize (their
replicas are done), then 3dQwarp" by:

```markdown
1. **AFNI:** 3dQwarp, 3dSkullStrip,
```

## docs/validation/README.md (the validation index)

Replace the two rows `afni-automask` and `afni-unifize` by:

```markdown
| [afni-automask](afni-automask.md) | `lx.afni.automask`, `larmorx afni 3dAutomask` (clean-room original, `crates/larmorx-afni`) | 3dAutomask (AFNI 25.2.09 binary; fMRIPrep 25.2.6's AFNI as a second build) | `validated` |
| [afni-automask-replica](afni-automask-replica.md) | `larmorx-gpl afni 3dAutomask` (GPL replica, `crates-gpl/`), run by `larmorx afni 3dAutomask` and `lx.afni.automask` | 3dAutomask (AFNI 25.2.09 binary; fMRIPrep 25.2.6's AFNI as a second build) | `validated` |
| [afni-unifize](afni-unifize.md) | `lx.afni.unifize`, `larmorx afni 3dUnifize` (clean-room original) | 3dUnifize (AFNI 25.2.09 binary, as built and with CORE-MATH's correctly rounded functions preloaded; 1 and 6 OpenMP threads; fMRIPrep 25.2.6's AFNI) | `validated` |
| [afni-unifize-replica](afni-unifize-replica.md) | `larmorx-gpl afni 3dUnifize` (GPL replica), run by `larmorx afni 3dUnifize` and `lx.afni.unifize` | 3dUnifize (AFNI 25.2.09 binary, 1, 2 and 6 OpenMP threads; fMRIPrep 25.2.6's AFNI) | `validated` |
```

and the commands (lines `parity afni-automask` and `parity afni-unifize`) by:

```bash
python -m larmorx_validation parity afni-automask --tier standard --out docs/validation   # the clean-room original; the AFNI oracle
python -m larmorx_validation parity afni-automask --tier standard --implementation replica --out docs/validation   # the larmorx-gpl build
LARMORX_CRMATH_PRELOAD=oracles/core-math-preload/libcrmath.so python -m larmorx_validation parity afni-unifize --tier standard --out docs/validation   # a preload with tanhf, cbrtf
python -m larmorx_validation parity afni-unifize --tier standard --implementation replica --out docs/validation
```

## docs/validation/golden.md

In the table of cases, add a row (and keep the replica's row):

```markdown
| `larmorx afni 3dAutomask`, `3dUnifize`, `lx.afni.automask`, `lx.afni.unifize` (clean-room originals) | 8 | 3dAutomask: fMRIPrep's call (`-apply_prefix -dilate 1`) with `-depth` on the oblique float32 volume; a 4D series averaged with `-nograd -eclip -clfrac 0.3 -peels 2 -NN3 -dilate 2 -erode 1 -SI 20`, and the 2D method on a plane; `-peels 0 -nbhrs 12` on a series with a negative scale factor in both modes (AFNI-43, AFNI-01); the Python API in memory with `exterior_clip`, `erode`, `nn=1`, the masked input and the depth. 3dUnifize: the defaults with `-ssave` and `-amsave`; fMRIPrep's `-T2` call; `-T2 -T2 -GM`; `-EPI` on a series; `-T2 -T2up 100` in both modes (AFNI-45); the Python API in memory with `t2`, the scale and the automasked input. The half-size path (more than a million voxels) is covered by the parity suites and the Rust unit tests of `unifize::duplo` |
```

Replace the paragraph "**Tools with only a replica** (3dAutomask and 3dUnifize, until their
clean-room originals ...)" by:

```markdown
- **Tools with a replica and an original** (3dTshift, 3dAutomask, 3dUnifize): the cases in
  `tests/golden/registry.py` run the clean-room original (`implementation="original"`); the
  replica has its own in `crates-gpl/larmorx-gpl-cli/tests/golden.rs`. No tool has only a
  replica now (`REPLICA_ONLY_FUNCTIONS` is empty). 3dUnifize's half-size path (more than a
  million voxels) and AFNI-47 are beyond the golden tests' 32³ limit: the parity suites and
  the Rust unit tests (`unifize::duplo`, `unifize::wmi` against the spec's method) cover them.
```

## docs/findings/README.md (the index of findings)

Add a row:

```markdown
| [afni-masks-original.md](afni-masks-original.md) | the clean-room originals of 3dAutomask and 3dUnifize: what black-box runs showed (AFNI's own header extension in fMRIPrep's input, AFNI-10; glibc's `tanhf` and 3dUnifize's squash; messages), and how the original computes the same results faster (unrestricted) |
```

## docs/findings/upstream-defects.md (the register)

In the summary table, replace the rows AFNI-10, AFNI-41 and AFNI-48 (their other columns as
they are):

```markdown
| AFNI-10 † | 3dTshift, 3dAutomask, 3dUnifize | AFNI's own header extension (7 digits) wins over the NIfTI header | d | AFNI-written inputs (fMRIPrep's 3dAutomask reads 3dUnifize's output: ds005454's BOLD reference, a header 7th digit) | replicas reproduce; originals ignore (the 3dAutomask original since 2026-10-23: data bit-identical, header as AFNI's without the extension) | – |
| AFNI-41 † | 3dAutomask | `-nbhrs` outside 6..26 silently becomes 17 (the range check never runs); 19..26 act as 18 | c? | none | refused by default (exit 1, named), `reproduce` keeps AFNI's: the clean-room original since 2026-10-23 (the replica: its part) | – (decided 2026-10-23, rule B) |
| AFNI-48 † | 3dUnifize | `-T2` three times: neither inverted back nor edge-clipped (the output stays inverted); the help documents once and twice | c? | none | refused by default (exit 1, named), `reproduce` keeps AFNI's: the clean-room original since 2026-10-23 (the replica: its part) | – (decided 2026-10-23, rule B) |
```

In the rows AFNI-01, -02, -07, -11, -12, -16, -18, -23, -26, -40, -43, -45, -46, -47 (the
3dAutomask and 3dUnifize ones), "corrected by default ... (replica ...)" holds for both
implementations now: add "; the clean-room originals of 3dAutomask and 3dUnifize since
2026-10-23". History paragraph, add:

```markdown
The clean-room originals of 3dAutomask and 3dUnifize (2026-10-23, written from the specs by an
implementer who never read AFNI's source) correct the same defects by default with the same
switch, and implement the user's decisions of 2026-10-23: AFNI-41 and AFNI-48 refused by default
(rule B), AFNI-42 and AFNI-44 kept (rule C). The **yes** of AFNI-41 and AFNI-48 is done for the
originals.
```

## docs/api/upstream-defects.md

Line with the records ("in the records: [afni-tshift.md] ..."), replace the 3dAutomask and
3dUnifize links by:

```markdown
[afni-automask.md](../validation/afni-automask.md) and [afni-automask-replica.md](../validation/afni-automask-replica.md), [afni-unifize.md](../validation/afni-unifize.md) and [afni-unifize-replica.md](../validation/afni-unifize-replica.md),
```

Add two rows to the table of AFNI defects (after AFNI-47):

```markdown
| AFNI-41 | 3dAutomask | `larmorx afni 3dAutomask -nbhrs n`, `lx.afni.automask(neighbours=n)` with n outside 6..26 | 17 used silently | an error (exit 1, `ValueError`); `reproduce` uses 17 (decided 2026-10-23) |
| AFNI-48 | 3dUnifize | `larmorx afni 3dUnifize -T2 -T2 -T2` | neither the inversion back nor the edge clipping: the output stays inverted | an error (exit 1); `reproduce` gives AFNI's output (decided 2026-10-23); `lx.afni.unifize` accepts `t2` 0 to 2 only |
```

After the paragraph "The 3dAutomask and 3dUnifize replicas (`larmorx-gpl`, 2026-10-23) have the
switch for every ...", add:

```markdown
Their clean-room originals (2026-10-23) have the same switch and the same defaults; their parity
records' fix passes show each correction where it applies.
```

## docs/licensing.md

Where the section on 3dAutomask and 3dUnifize says their clean-room originals are to be written
(the orchestrator knows the place; the clean-room implementer did not read that section), add:

```markdown
The clean-room originals exist since 2026-10-23 (`crates/larmorx-afni`, Apache-2.0: `automask`,
`unifize`; provenance in `crates/larmorx-afni/PROVENANCE.md`); the main package uses
CORE-MATH's correctly rounded `tanhf` and `cbrtf` (MIT, `larmorx_core::math`) where AFNI calls the C
library's.
```

## docs/next-steps.md

Table of parts, add:

```markdown
| AFNI masks original | `worktree-agent-a4282c736dbd9fe7c` (10-23) | the clean-room originals of 3dAutomask and 3dUnifize (from the specs; built for speed: a sliding window of ranked values for 3dUnifize's ball, union-find clusters, breadth-first exterior clip and depth, parallel passes), `implementation=` for both, AFNI-41 and AFNI-48 refused by default; CORE-MATH's `tanhf` and `cbrtf`; 8 golden cases; both evidence pages with the original | a benchmark report (on a quiet machine); the replica records to regenerate under `-replica`; the shared CORE-MATH preload to rebuild |
```

Item 5 of "The order from here": replace "**Next for them: the clean-room originals**, ... as
3dTshift." by:

```markdown
   **The clean-room originals: done** (2026-10-23): `larmorx afni 3dAutomask` / `3dUnifize` and
   `lx.afni.automask` / `unifize` have `implementation="auto"|"replica"|"original"` as 3dTshift
   (records `afni-automask.md`, `afni-unifize.md`; the replica's moved to `*-replica.md`).
```

In the follow-ups, replace "**The clean-room originals of 3dAutomask and 3dUnifize** (from
`specs/3dAutomask.md`, `specs/3dUnifize.md`); a benchmark report for each once the original
exists." by:

```markdown
- **Benchmark reports of 3dAutomask and 3dUnifize** (AFNI, the replica, the original; a quiet
  machine). Informal timings of 2026-10-23 are in the originals' part report.
```

## CLAUDE.md, "Where things are"

AFNI row of the table:

```markdown
| AFNI | 3dTshift, 3dAutomask, 3dUnifize, complete: clean-room originals (`crates/larmorx-afni`) and GPL replicas (`crates-gpl/`) | `afni-tshift`, `afni-tshift-replica`, `afni-automask`, `afni-automask-replica`, `afni-unifize`, `afni-unifize-replica` |
```

and after "**AFNI 3dAutomask and 3dUnifize merged** (2026-10-23): ...":

```markdown
**The clean-room originals of 3dAutomask and 3dUnifize merged** (2026-10-23): from the specs,
built for speed; `implementation=` as 3dTshift.
```

## The site (`afni-masks-original-evidence`)

- `content/tools/afni-automask.md`, `content/tools/afni-unifize.md`, `static/evidence/afni-automask.json`,
  `afni-unifize.json` and their blobs: in the branch.
- `data/status.yml`, the rows of 3dAutomask and 3dUnifize:

```yaml
  - eco: afni
    tool: 3dAutomask (brain mask)
    page: /tools/afni-automask/
    use: ["larmorx afni 3dAutomask", "lx.afni.automask"]
    how: clean-room original; exact replica in `larmorx-gpl`
    validation: "original: [105 cases](repo:docs/validation/afni-automask.md), 80 of 81 compared cases that write outputs bit-identical to AFNI (the other: AFNI-10's header, its masks bit-identical); replica: [81 of 81](repo:docs/validation/afni-automask-replica.md) bit-identical; fMRIPrep's call on every oracle run included"
  - eco: afni
    tool: 3dUnifize (intensity uniformization)
    page: /tools/afni-unifize/
    use: ["larmorx afni 3dUnifize", "lx.afni.unifize"]
    how: clean-room original; exact replica in `larmorx-gpl`
    validation: "original: [97 cases](repo:docs/validation/afni-unifize.md), 72 of 72 compared cases that write outputs agree, 32 bit-identical and 40 bit-identical with correctly rounded maths, 4–25× faster (informal timings); replica: [72 of 72](repo:docs/validation/afni-unifize-replica.md) bit-identical; fMRIPrep's call on every oracle run included"
```

  and in `planned`, the AFNI item's text: `3dQwarp, 3dSkullStrip, 3dcalc and an analysis track (3dTproject, 3dDespike, 3dReHo, 3dRSFC, 3dDeconvolve/3dREMLfit, 3dFWHMx/3dClustSim, ...).`
- `site.yml`, the navigation summaries:

```yaml
  - { title: 3dAutomask, url: "/tools/afni-automask/", eco: afni,
      summary: "A brain mask from intensities: a clean-room original, and an exact replica in larmorx-gpl." }
  - { title: 3dUnifize, url: "/tools/afni-unifize/", eco: afni,
      summary: "White-matter intensity uniformization: a clean-room original, and an exact replica in larmorx-gpl." }
```

- `content/validation.md`, line 14: drop the last sentence ("A tool may have its replica before its
  clean-room original (3dAutomask and 3dUnifize, for now).") or replace it by "Every AFNI tool
  so far has both."
- The verdict *identical with correctly rounded maths* (`identical-cr`) is used by the 3dUnifize
  original's 40 cases; it is in `data/verdicts.yml` already (N4 and others use it).
- Follow-up for the evidence code (`validation/src/larmorx_validation/evidence/common.py`, not
  changed here): a worked example whose `default.diffs` names a comparison without displayable
  panels (one of its runs only compared, `full=`) stops that example's viewer (`needed()` reads
  `panelById[...]` of a panel that does not exist: the panels stay empty, no error shown). It
  happened to 3dUnifize's T1w example in this part and is fixed there (faa0046); a check in
  `common.finish` (every default diff must have `display`) would catch it for every tool.
- `static/evidence/summary.json`: re-export (`lxv.sh . evidence summary --site ...`); the
  exporter's `summary.py` lists both implementations of both tools now.
- `content/validation.md`: wherever it says 3dAutomask and 3dUnifize have only a replica, say they
  have both implementations, as 3dTshift.
