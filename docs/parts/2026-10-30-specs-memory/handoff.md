# HANDOFF: three specs (FSL-20/21, AFNI-75, cbrtf) and the AFNI replicas' memory, 2026-10-30

Branch: the worktree agent-a67c9c36c5d6371cb (from main fb3f334). No site work (no page states the
replica's memory: the AFNI pages show the parity records, whose outputs do not change). Defect IDs
used: **FSL-65, FSL-66** (new, found by black-box runs of mcflirt 6.0.7.23); AFNI-76 to AFNI-80,
FSL-67, FSL-68 unused. Every block below is the exact text for a shared document; "replace" quotes
the current text first.

## 1. docs/findings/upstream-defects.md (the register)

1a. Summary table: after the row `| FSL-64 † | ...`, add:

| FSL-65 † | mcflirt | an output text file that cannot be opened (its name a directory, its directory missing or not writable): `-plots`' `.par` ends the run at volume 0's line with "error: unable to open output file!" and **status 0**, after the `.mat` directory's `MAT_0000` and the `.rms` files (no other matrix, no corrected series); an `.rms` file is silently not written (status 0) | e | none (fMRIPrep's `-out` is in an existing work directory) | to do, by the rule of FSL-20 (rule B, 2026-10-30): an error by default (exit 1, nothing written), mcflirt's behaviour with `reproduce`; specified in `specs/mcflirt.md` §10.6; the replica writes everything it can and ignores the failure in both modes (a follow-up) | the replica and the clean-room original |
| FSL-66 † | mcflirt | a negative `-bins`: each stage has `-bins / scale` bins (truncated toward zero), prepared for **every** cost function; with −1 bins (`-bins` −8 to −15, or −4 to −7 with two stages or more) the process is killed by `SIGFPE` (an integer division by zero), with −2 or fewer (−16 and below) it aborts with `std::bad_array_new_length`; nothing written | a | none (fMRIPrep passes no `-bins`) | to do (rule B, 2026-10-30): refused by default (exit 1, nothing written), with `reproduce` an error saying how mcflirt ends (a crash is never reproduced); specified in `specs/mcflirt.md` §12.1; the replica and the clean-room original run normally today in both modes (a follow-up) | the replica and the clean-room original |

1b. Summary table, row FSL-20: replace
"the clean-room original: to do (the spec)"
with
"the clean-room original: to do (its spec: `specs/mcflirt.md` §10.6, 2026-10-30)"

1c. Summary table, row FSL-21: replace
"the clean-room original: to do (the spec)"
with
"the clean-room original: to do (its spec: `specs/mcflirt.md` §12.1, 2026-10-30)"

1d. Summary table, row AFNI-75: replace
"| the clean-room original (its spec) |"
with
"| the clean-room original (its spec: `specs/3dUnifize.md` §9.1, 2026-10-30) |"

1e. The mcflirt replica paragraph of the gap lists: replace
"**Open:** FSL-20
and FSL-21 in the clean-room original (a spec writer adds them to `specs/mcflirt.md`, then a
clean-room implementer)."
with
"**Open:** FSL-20
and FSL-21 in the clean-room original (specified 2026-10-30: `specs/mcflirt.md` §10.6 and §12.1;
then a clean-room implementer); FSL-65 and FSL-66 (found 2026-10-30) in both the replica and the
clean-room original."

1f. "Random testing's gaps" table, row AFNI-75: replace
"| AFNI-75 | the clean-room original of 3dUnifize: the same guard (its spec, then its agent) |"
with
"| AFNI-75 | the clean-room original of 3dUnifize: the same guard (its spec, `specs/3dUnifize.md` §9.1, 2026-10-30; then its agent) |"

1g. Section "Restricted: FSL mcflirt (source-derived)", after the section "### FSL-21 † ..." (before
"### FSL-22 † ..."), add:

### FSL-65 † An output text file that cannot be opened: status 0, or silently missing — (e)
- **What:** mcflirt writes its text outputs through plain file streams after the estimation:
  `decompose_mats` (`mcflirt.cc:460-560`) opens `<out>.par` first (with `-plots`), makes the `.mat`
  directory (FSL-20), writes the `.rms` files without checking that they opened, then, volume by
  volume, the `MAT_` file and the `.par` line; at the first `.par` line it tests the stream and,
  when it failed to open, prints "error: unable to open output file!" and calls `exit(0)`
  (`mcflirt.cc:526-531`, `:542-547`). So an unopenable `.par` ends the run with status 0 after
  `MAT_0000` and the `.rms` files, without the corrected series; an unopenable `.rms` is silently
  not written.
- **Evidence:** read; verified (2026-10-30, `validation/investigations/specs-memory/mcflirt_fsl20_21.py`:
  a directory named `o.par` with `-mats -plots -rmsrel`, `-plots` alone and `-mats -plots -refvol 0`;
  directories named `o_rel.rms` and `o_abs_mean.rms`; `-plots` with a missing parent directory).
- **fMRIPrep:** none (it writes no `.par` through mcflirt into a blocked name; its `-out` is in an
  existing work directory).
- **larmorx:** to do. By FSL-20's rule (rule B: the run cannot honour its command and reports
  success): an error by default (exit 1, nothing more written), mcflirt's behaviour with
  `reproduce` (`specs/mcflirt.md` §10.6). The `larmorx-nc` replica today writes every other output
  and exits 0 in both modes where `.par` fails (mcflirt writes only `MAT_0000` and the `.rms`
  files), and skips an unopenable `.rms` silently in both modes (mcflirt's behaviour).
- **D13 / gap:** the replica's `reproduce` (mcflirt's exit point) and default (the error); the
  clean-room original's (its spec).

### FSL-66 † A negative `-bins` crashes mcflirt, whatever the cost — (a)
- **What:** `-bins` is read with `atoi` (FSL-07) and each stage calls `set_no_bins(no_bins /
  scale)` (`mcflirt.cc:350`, integer division toward zero) for every cost function;
  `Costfn::set_no_bins` (`costfns.cc:3622-3661`) allocates `new int[(no_bins+1)*(no_bins+1)]` and
  `new int[no_bins+1]` and sizes a table with `10*N/(no_bins+1)`. With −1 bins the division by 0
  raises `SIGFPE`; with −2 or fewer `new int[no_bins+1]` of a negative length throws
  `std::bad_array_new_length`, uncaught (abort). So `-bins` −8 to −15 (stage 1 has −1), −4 to −7
  with two stages or more (stage 2 has −1), and −16 or less end the run with a signal before any
  output; −1 to −3 give 0 bins in every stage (FSL-21's case for the histogram costs, harmless for
  `normcorr` and `leastsquares`).
- **Evidence:** read; verified (2026-10-30, `mcflirt_fsl20_21.py`: `-bins -8` and `-bins -4 -stages
  2` killed by signal 8, `-bins -16` with `normcorr` and `mutualinfo` signal 6 with
  `bad_array_new_length`, `-bins -1` runs).
- **fMRIPrep:** none.
- **larmorx:** to do: refused by default before any work (rule B: a negative number of bins makes
  no sense, and mcflirt crashes on most of them), exit 1; with `reproduce`, an error saying how
  mcflirt ends where it crashes, mcflirt's run where it does not (−1 to −3 with the costs that use
  no bins) (`specs/mcflirt.md` §12.1). The replica and the clean-room original today run every
  negative value (they take a negative count as none, or one).
- **D13 / gap:** the replica's and the clean-room original's.

1h. The history paragraph / counts: two new entries (FSL-65 class e, FSL-66 class a), both "to do",
neither **decide**. Recount with `register_counts.py`.

## 2. docs/api/upstream-defects.md

If it lists every FSL defect with its default, add (in the mcflirt rows, after FSL-21):
"| FSL-65 | an output text file (`.par`, `.rms`) that cannot be opened: mcflirt exits with status 0 after part of its outputs (`.par`), or silently skips the file (`.rms`) | to do: an error (exit 1); `reproduce` mcflirt's behaviour | the command line |"
"| FSL-66 | a negative `-bins` crashes mcflirt (signal 8 or 6) for every cost | to do: refused (exit 1); `reproduce` an error where mcflirt crashes | the command line |"

## 3. docs/next-steps.md

3a. Follow-up "**mcflirt's FSL-20 and FSL-21 in the clean-room original:**": replace the bullet
"- **mcflirt's FSL-20 and FSL-21 in the clean-room original:** a spec writer adds them to
  `specs/mcflirt.md` (behaviour only; the replica's record shows mcflirt's black-box outputs),
  then a clean-room implementer adds them to `lx.mri.hmc`'s original."
with
"- **mcflirt's FSL-20, FSL-21, FSL-65 and FSL-66 in the clean-room original: ready for its
  implementer** (specified 2026-10-30, `specs/mcflirt.md` §10.6, §12.1, the cases in §14.1; the
  black-box probe `validation/investigations/specs-memory/mcflirt_fsl20_21.py`): a clean-room
  implementer adds them to `lx.mri.hmc`'s original (today it errors on FSL-20 without naming it,
  runs FSL-21's cases with one bin in both modes, and runs FSL-66's).
- **The mcflirt replica's FSL-65 and FSL-66** (found 2026-10-30): its `reproduce` should end where
  mcflirt does (an unopenable `.par`: status 0 after `MAT_0000` and the `.rms` files; a negative
  `-bins`: mcflirt's crash, as an error) and its default refuse both (`specs/mcflirt.md` §10.6,
  §12.1 are the target); a small replica-author part with a command-line test each; no record
  changes (no recorded case uses them)."

3b. Under the random-testing gaps bullet "AFNI-75's guard is in the GPL replica of 3dUnifize only: the
clean-room original needs it through `specs/3dUnifize.md` (a spec writer), then a clean-room agent.":
replace with
"AFNI-75's guard is in the replica of 3dUnifize only: the clean-room original's is **ready for its
implementer** (specified 2026-10-30, `specs/3dUnifize.md` §9.1: the prediction, the limit, the
messages, `reproduce`; the cases in §10; the probe
`validation/investigations/specs-memory/unifize_epi_ball.py`)."

3c. Follow-up "**3dUnifize's replica memory**": replace the bullet with
"- **3dUnifize's replica memory: done** (2026-10-30): the replica holds what AFNI holds (`-T2` on
  160×192×160: 115.2 MiB through `python -m larmorx`, 103.8 through the binary, against AFNI's 99.1;
  `-EPI` on 64×64×40×160: 120.8 and 110.4 against 109.2; it was 164.8 and 320.9), results unchanged
  bit for bit (the standard record re-made, compare_records: no change); the two strict expected
  failures removed (`validation/investigations/specs-memory/README.md`)."

3d. Follow-up "**The Python entry point's own memory**": replace the bullet with
"- **The Python entry point's own memory: a decision** (2026-10-30, measured with
  `validation/investigations/specs-memory/entry_memory.py`): numpy is not imported on the command
  line (it never was); its fixed cost is the interpreter (9.0 MiB) and the Rust core it loads
  (4.0 MiB). The one import left, `larmorx._replica` (2.3 MiB), is now deferred, so 3dAutomask on the
  64×64×40 mean takes 19.6 MiB (21.3 before; `_core.cli_main` called alone 18.5; the standalone
  binary 8.4) against AFNI's 7.7 and the rule's 16.5. No import is left to defer: the rule's 8 MiB is
  below the entry point's fixed 13 MiB, which no change of larmorx's Python reaches. **For the user**
  (recommended first): (1) count the entry point's fixed cost apart in the memory test (its
  allowance `1.10 × original + the peak of python -m larmorx --version`, 14.0 MiB here), keeping the
  8 MiB rule for the computation, which the binary meets; (2) measure the small cases through the
  standalone binary; (3) keep the strict expected failure."

3e. Follow-up "**3dUnifize `-EPI`'s `cbrtf`**": after "Alternatives: a table of the 9,346 changed volumes from 0.001 mm³ (exact on realistic voxels
  only), or keeping the warning." add
"  **The spec is ready** (2026-10-30, `specs/cbrtf.md`: the method as mathematics, its constants as
  binary64 bits, every rounding, the special values; the validation plan: the block hashes of all
  2^32 inputs from the system's `cbrtf` as a black box, committed as
  `validation/investigations/specs-memory/cbrtf-glibc-2.35-x86_64-blocks.json`, the whole-sequence
  hash `c679cb92d4402e33`, and an input-by-input comparison with the earlier translation by someone
  allowed to read it): if the user agrees, a clean-room implementer can start at once (about an
  hour)."

## 4. docs/decision-log.md

One entry, 2026-10-30 (decided by the rules, no user decision needed): "FSL-65 and FSL-66 (found by
the spec writer's black-box runs of mcflirt 6.0.7.23): refused by default, mcflirt's behaviour (or,
for FSL-66's crashes, an error saying how it ends) with `reproduce`, by rule B, as FSL-20 and
FSL-21." And the open question of 3d (the memory test's allowance for the Python entry point's
fixed cost), for the user.

## 5. The site's Roadmap (`content/roadmap.md`, the orchestrator's rule of 2026-10-29)

No tool page states the replica's memory (checked: `content/tools/afni-unifize.md` and the
Validation page name only the rule), so no page changes. The Roadmap's row 2, "**Two specified
clean-room additions** | mcflirt's FSL-20 and FSL-21 corrections and 3dUnifize's memory guard
(AFNI-75) in the clean-room originals | Planned", may become "| Specified (2026-10-30), ready for
the clean-room implementer |", with FSL-65 and FSL-66 added to its description ("mcflirt's FSL-20,
FSL-21, FSL-65 and FSL-66 corrections"), and a Completed line: "| 2026-10-30 | **3dUnifize's replica:
memory as AFNI's.** Peak memory within the rule with `-T2` and `-EPI` (it held 1.5 and 2.8 times
AFNI's); the specs of FSL-20/21, AFNI-75 and a glibc-exact `cbrtf` written |".

## 6. Nothing else shared

No change to README, the validation index, golden.md, licensing, CLAUDE.md. The 3dUnifize
replica's record (`docs/validation/afni-unifize-replica.md`, `.json`) is re-made in the branch
(5d0a982, from the clean 1c4d4f6): compare_records 116 of 116 cases, 0 status and 0 detail changes;
its tables entry unchanged (`tables --check` passes).
