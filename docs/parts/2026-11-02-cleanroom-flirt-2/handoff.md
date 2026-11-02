# Handoff: FLIRT's clean-room original, part 2 (relays 1 to 7, 2026-11-02)

Branch: the worktree `<workspace>/larmorx/.claude/worktrees/agent-a9232cf1150982f92` (from main fb1de31, main
0bed438 merged in relay 4), HEAD `7a294b5` (relay 7; the records' commit be3f502, golden 3086fe0).
Site: `<scratch>/site`, branch `fsl-flirt-cleanroom-page` (from site main 94729d5), commit `741ba2c`; its local
`evidence-blobs` branch `af4e28b` (606 blobs, manifest in the commit). The exact edits to the shared documents
are `HANDOFF.toml` beside this file (20 operations; `apply_handoff.py --check` passes on the branch).

## The handoff's operations, and why

1. **Register, FSL-111 (new entry, summary row).** applyxfm4D reads an image stored as uint8 as signed bytes
   (x86-64's `char`): its values 128 to 255 enter the interpolation, the background value and the spline's
   coefficients as v − 256 (profile 100, 140 at 0.4 gives 13, not 116); `flirt -applyxfm` reads the same
   image as stored (probed). Class c, rule A (the input makes sense, the result is wrong), fMRIPrep impact
   none (fMRIPrep resamples with its own resampler). Evidence: relay 6's black-box probes
   (`validation/investigations/cleanroom-flirt/relay6_u8probe.py`, every FSL run through the oracle cache),
   the 29 uint8 applyxfm4D findings of the trial, all FSL's bytes now in `reproduce`. Gap **replica**: the
   replica reads uint8 as signed bytes in both modes (black box, 3 probes), so its default does not correct
   it yet. The row is inserted after FSL-93's; if FAST's replica (FSL-96 to FSL-110) merges first, move it
   after FSL-110. FSL-112 to FSL-115 are not used.
   - **Not an op (I may not search the register):** the entry's full section and its history line, for the
     orchestrator to place in the non-restricted FSL part and the register's history:
     > ### FSL-111 — applyxfm4D reads uint8 images as signed bytes (FSL 6.0.7.23)
     > **What:** an image stored as uint8 is held as signed bytes: values 128 to 255 read v − 256 (255 as −1).
     > Interpolating across 127/128 (a profile 100, 140 at 0.4 of the way gives 13, not 116), the background
     > value (the border's 10th percentile: 180 for a border of 10s and 140 to 250s) and the spline's
     > coefficients use the negative values; nearest neighbour writes the stored bytes back (the output's
     > conversion wraps them) but for the padding voxels. `flirt -applyxfm` reads the same image as stored.
     > **Evidence:** verified (black-box runs of FSL 6.0.7.23's applyxfm4D and flirt on uint8 and int16
     > profiles, every run through the oracle cache: `validation/investigations/cleanroom-flirt/relay6_u8probe.py`;
     > the 29 uint8 applyxfm4D findings of the clean-room trial of 2026-11-02); validated in the
     > clean-room record's `programs/applyxfm4D-*` cases. **fMRIPrep users:** none. **larmorx:** the
     > clean-room original reads the values as stored by default and as FSL with `reproduce`, naming FSL-111
     > where a written value changes (2026-11-02, rule A); the replica reads them as FSL in both modes.
     > **D13 gap:** the replica's default and its notice.
     > History line: "2026-11-02: FSL-111 (applyxfm4D's uint8 images read as signed bytes), found by FLIRT's
     > clean-room original's black-box runs (part 2, relay 6); rule A; the replica's switch to do."
2. **`docs/api/upstream-defects.md`.** FSL-111's row; and the rows of FSL-68, FSL-75, FSL-76, FSL-78, FSL-79,
   FSL-81 and FSL-90 now name both implementations (the clean-room original names and corrects each of them,
   with its `reproduce` switch: `larmorx_core::defects` constants used in `crates/larmorx-fsl`; FSL-77, FSL-80,
   FSL-91 to FSL-93 rows say "the same", so they follow).
3. **`validation/tools.toml`** (the README's tool table, the validation index and the site's status table are
   generated from it: run `python -m larmorx_validation tables` after applying, which also regenerates
   `data/status.yml` in the site): the `fsl-flirt` tool row (every program now has its clean-room original;
   the speed) and the `fsl-flirt` record row (part 2's programs and cases). **Note:** on the branch
   `tables --check` fails now only on the two `fsl-flirt` rows of README.md and docs/validation/README.md,
   because the record re-made in be3f502 changed their counts; applying the ops and running `tables`
   regenerates them (checked: no other row changes).
4. **`docs/validation/golden.md`:** part 2's 24 new cases and 8 re-recorded on purpose (relay 6, from be3f502
   with the smoke tier's marker), the two family rows (41 command-line cases, 17 `lx.fsl` cases).
5. **`docs/next-steps.md`:** item 4 (parts 1 and 2 done), item 7's FLIRT clean-room part 2 (done, with the
   numbers), the `lx.mri.hmc` follow-up (done in relay 4), two new follow-ups: "FLIRT's clean-room original,
   what is left" (the sinc/spline float last bits; SIMD sampling and a lighter parallel pass for the
   single-thread 12-dof run, 0.9 times FSL's speed; the trial's paths, -debug headers, avscale's sign, the
   record's 8) and "FSL-111 in the replica".
6. **`validation/difftest/triage.toml`** (an `append`; I never opened the file): 11 `[[difference]]` entries of
   `fsl-flirt-original` (implementation `original`), signatures the trial's check names: registrations'
   matrices and their images (open), pairreg (open), the streams whose counts follow the path (open; relay
   5's BBR boundary messages and relay 6's `-dof 3` rule noted), images from a given matrix (fixed: relays 5
   and 6), applyxfm4D's uint8 sinc/spline (register, FSL-111), the float sinc/spline last bits (open),
   `-debug`'s volumes' headers (open), the singular matrix with non-finite voxels (open), the text programs
   (fixed), the D13 notices (fixed), aff2rigid's one case (open). Four findings replayed with the final build
   (relay 7: #2905, #704, #170, #2424): all still differ, as the open entries say.

## The site (not ops: the shared site files)

- `site.yml`, FLIRT's tool summary:
  `summary: "Affine and boundary-based registration (flirt) with convert_xfm and FLIRT's other programs: a clean-room original (experimental) and an exact replica in larmorx-nc."`
- `data/status.yml`: generated (item 3's `tables`).
- `static/evidence/summary.json`: re-export with `python -m larmorx_validation evidence summary --site <site>`
  after the merge: `evidence/summary.py` (branch 9c39f61) now lists FLIRT's clean-room original (record
  `fsl-flirt`) beside its replica. Observation: the summary lists BET's two replica records but not BET's
  clean-room original (`fsl-bet`), whose page shows it; not changed here (not my tool).
- `content/roadmap.md` (the orchestrator's): row 1's status, suggested:
  "The replica complete (2026-11-01/02, parts 1 to 3: ... as now ...). The clean-room original complete
  (2026-11-02, parts 1 and 2): every program and script of the replica, `-debug` and every verbose level,
  206 of 214 compared cases within thresholds, fMRIPrep's three boundary-based calls within 0.011 mm of FSL's
  matrices, 12 to 14 times FSL's speed on 14 threads; [its page](/tools/fsl-flirt/)." and a Completed row:
  "| 2026-11-02 | **FLIRT's clean-room original, part 2.** FLIRT's other programs (avscale, the coordinate
  programs, rmsdiff, makerot, midtrans, pointflirt, applyxfm4D) and scripts (pairreg, extracttxt,
  aff2rigid), `-debug` and every verbose level, written from the specification and black-box runs; the
  specification's answers closed 22 of its 30 open cases; 12 to 14 times FSL's speed on fMRIPrep's call;
  FSL-111 found (applyxfm4D's uint8 images); the page shows it beside FSL and the replica, with FSL's own
  variability; [the record](repo:docs/validation/fsl-flirt.md): 206 of 214 compared cases within thresholds |".
  The orchestrator noted the spec answers belong in the roadmap at this merge: the replica's six new cases
  (FSL-90 to FSL-93, FSL-22 in FLIRT's padding) are on the page now (the replica's record: 302 cases).
- The page (`content/tools/fsl-flirt.md`, mine) also corrects what part 2 made stale: the replica's numbers
  (302 / 209 / 87 / 6 from its current record), "the clean-room original is still to be written", "there is no
  benchmark report yet". Its evidence: before/after inventory of all 19 pages, after >= before on every one
  (only fsl-flirt.json changed: five examples, panels 8→11, comparisons 4→8 and 3→7, blobs and bytes up;
  cases 296→315; implementations replica 296→302 plus original 305; speed rows 0→7); page 36.5 MB (budget 40);
  number check 318 of 318; viewer check 38 of 38; build 0 problems with the main repository's links checked;
  screenshots 1440 and 390, light and dark (`<scratch>/shots/`). The case commands now name FSL's installation
  `$FSLDIR` (the export had the account's home path in 19 commands, pre-existing; none left in the JSON).
- Push order: `evidence-blobs` (af4e28b, force) before the branch's main merge.

## For the orchestrator and the replica side

- **The replica and FSL-111:** `larmorx-nc fsl applyxfm4D` reads uint8 as signed bytes in both modes (relay 6,
  3 black-box probes): its default does not correct FSL-111 (next-steps follow-up in the ops).
- **`docs/api/fsl-flirt.md`** gets the spec writer's operation after this merge: part 2 kept its sections'
  headings as they were (relay 7 did not edit the page).
- **CLAUDE.md's FSL row / README:** README through `tools.toml`; CLAUDE.md's "Next" paragraph: FLIRT's clean-room
  original part 2 done.
- **Rules broken in relay 5** (reported then, repeated here): 16 × 8 GiB boxes beside a 24 GiB trial once
  (over the 32 GiB limit), and the trial ran with the oracle cache off. Both rules were added to the briefs.
- **Relay 1 and relay 6 slips** (titles or file names only; below).
- **GNU sed's usage text** (GPL) is not reproduced by the Apache original (extracttxt prints one larmorx line;
  the suite cuts that block from FSL's standard error).
- **Benchmark:** timed at 04f5462 (relay 3), before the specification's answers changed the arithmetic; not
  re-timed (no repeated benchmarks in this part); the page and the chart say so.

## Spec questions (for the spec writer; relays 1 to 7; part 1's 25 were answered in §14)

1. makerot: §15.4 cites "§8.5's rotation" (stale). Observed: B = [u, v, n], n = w/θ with θ = |w| in single
   precision (n not renormalised), u = (−n_y, n_x, 0)/|.| or (1,0,0) when exactly 0, v = (n × u)/|.|, core
   [[c,s,0],[−s,c,0],[0,0,1]] with cosf/sinf(θ); identity when θ < 1e-8; axis and centre in single precision.
2. avscale: scales and skews in single precision, rotations and translations in double; with §3's
   decomposition (relay 6) 11 of 12 sampled trial findings agree; one residual's sign in the check
   `M⁻¹·R·K·S` (−0.000000 against 0.000000) is not reproduced: how FSL forms rotmat's last bits.
3. (superseded by 27.)
4. img2imgcoord: `-` is not an option (as the last word: "Lacking argument to option -"); an unknown option as
   the last word: "Lacking argument"; `-flirt`'s warning on standard error.
5. Standard input: the prompt before the terminal refusal (stderr, the bare program name); std2imgcoord reads a
   terminal; its repeated-triple test is exact and never applies to the first triple.
6. Numbers read as libstdc++ reads doubles: nan, inf, out of range end the input.
7. `-warp` of a readable image that is not a warp field: FSL prints "An error occured while reading file:
   <name>", exit 1; the original refuses every readable `-warp` (its own message, exit 1).
8. Matrix files (§2, §15): libstdc++'s extraction; inf/nan read 0 and fail the line (later values repeat the last
   stored one), 1e400 the largest double, the first numeric line's word count gives the columns.
9. midtrans: `Pxx_av` (--debug) is stage 1's avg(P_n(rI + x_n x_nᵀ)), printed before A; the centre prints 4
   values (90 110 90 0 / zeros) without a template, 3 with one; x_n computed after "Initialising averages"; an
   unreadable template is caught (FSL-78); a non-finite determinant counts as unreadable.
10. pointflirt: the transposition decided by the input file alone (both transposed); a missing file gives the
    count error (exit 255), not an abort; a NaN coordinate does not abort (FSL's SVD returns an arbitrary matrix).
11. applyxfm4D: no blur before resampling (§15.7 says "as §9.2"); the reference's sform written as stored unless
    neurological (its zeros then positive); the input's read error prints only #63 (also with -3D), the
    reference's the #22 lines too; matrices inverted first; `-v` prints the method as given.
12. extracttxt: a negative relative start is subtracted without dc's message (only `-0`/`+0` and signed counts
    print it).
13. aff2rigid: one argument goes on to the NameError (no usage); missing, non-square or singular inputs stop with
    NumPy tracebacks.
14. pairreg: the texts of pairreg1.sch to pairreg3.sch, as behaviour, so the original need not read $FSLDIR.
15. (orchestrator, resolved in main 0bed438: the cache no longer serves exit 126/127.)
16. §2.3: the minima's free optimisations print at the search's level; `-debugsave` writes its grids from level
    6; `Using subset cost function` from level 4; the coarse grid's `Starting with`, `and tolerance`,
    ` dearranged:` at the search's 4, its evaluations at 5; measurecost and gridmeasurecost print no
    evaluation lines; `Previous scale used` only when a setscale makes a new pair.
17. §2.3 level 2's exact lines (the findings section lists them); -noclamp's "intensity between", -noresample's
    missing subsampling lines, -2D's missing sform messages.
18. §9.4: global_refweight1 a 1-D one-voxel image at the minimum sampling's voxel size; testvol keeps the input's
    sform −0; the levels' sforms follow W·F·D·F'; `MJ DEBUG OUTPUT` and `testvol DEBUG::` at each setscale;
    BBR's five set-up lines; with an empty U the final call before "Failed to calculate any transformation
    matrix". (See 38.)
19. §7.3: `setoption` echoes `setoptions: <name> <values>` at level 3; a tolerance list shorter than 12 errors
    at the next measurecost as well as optimise.
20. §8.1 step 9: the free optimisation starts from the start's decomposition composed again (under gimbal
    lock the translations' 6th digit); with -dof 6 and FSL-90's reproduce the message prints twice per start.
21. §6.4: the edge factors multiply as (((w_t·w_r)·w_x)·w_y)·w_z.
22. §9.2's sinc: with the table and lookup as stated, 1.6 % of real/interp-sinc's voxels still differ in their
    last bit (and applyxfm4D's spline of float images in 1.7 to 23 % of voxels in five trial findings): the
    per-axis weights' or the blend's precision?
23. §6.2 leastsq: its exact sums and order (real/cost-leastsq's trace parts at a free optimisation's start).
24. FSL-91/FSL-92 with reproduce: every cost constant; the matrix follows the ties of the line search, sorts and
    list after the 8 mm search: their tie rules.
25. §8.1 step 3 blocks every centre of mass of FLIRT "and specs/mcflirt.md §6.1's"; specs/mcflirt.md §6.1 still
    says one double sum and `lx.mri.hmc` was left so: should mcflirt's be blocked too?
26. §9.3: a registration's -out and applyxfm4D write an unset sform as the qform (its code) and an unset qform as
    the sform's quaternion (its code); -applyxfm keeps the unset form.
27. §15.2: an image with neither sform nor qform has V = S (x not reversed) as source and as destination or
    standard image; the standard programs warn for such a `-std` image too.
28. §6.5: without boundary voxels the error is printed at each pair's set-up (twice per setscale, once at
    setoption costfunction bbr) and each evaluation prints it then "Found NaN in BBR cost (2)".
29. (answered by 30.)
30. §15.7: applyxfm4D's char group is signed char (x86-64): uint8 128..255 read v−256 (FSL-111); FSL-79's
    conversions are x86-64's (through a 32-bit integer, wrapped), not kept in the range.
31. §9.2/§9.3/§15.7: a reference with neither form: -out and applyxfm4D write the input's form through
    W·D_in⁻¹·A⁻¹·D_ref (products left to right, single precision) as both forms, the input's form's code
    (applyxfm4D: the first volume's matrix); -applyxfm keeps the unset forms.
32. §9.3/§4.1: a quaternion FSL computes from its matrix (a completed qform, -applyisoxfm's) is written as
    computed, not converted to its matrix and back.
33. §8.2: `Erroneous dof` once per row (optimise and measurecost) and once per free start, after the start's own
    messages; FSL-90's starts at dof < 6 composed once.
34. §15.2: the qform as FSL holds it (§4.1's conversion) also in the coordinate programs.
35. §9.2: nearest neighbour outside the input takes the nearest index first, then the extraslice rule.
36. §12: the vector solves (dgesv) use the same left-looking LU as the inverse? (the original does; no case
    differs).
37. (new, relay 7) A matrix FLIRT inverts during a registration that is singular (inputs with −inf or NaN voxels:
    trial #2905, #704, #8361): FSL goes on and writes its matrix; the original stops with "inv(): matrix is
    singular" (exit 0 in reproduce, FSL-68). Which inversion, and what does FSL use instead?
38. (new, relay 7) `-debug`'s volumes on oblique and anisotropic images: global_refweight1's pixdim and the
    pyramid levels' qoffset/srow differ (114 trial findings, #170 replayed); and with BBR and no boundary point,
    FSL writes the debug and out images where the original stops after its set-up lines (#2424): what does
    FLIRT write after the set_bbr_seg error with -debug?

## The clean-room statement of every relay (from NOTES.md, the slips as stated)

- **Relay 1:** opened only CLAUDE.md (loaded), the brief, part 1's NOTES.md and bin/ scripts, specs/flirt.md
  (§1-§3, §8.5-§9.1, §10-§15.4 header to §15.5's start), specs/mcflirt.md §4.1, §4.2, §4.4, §6.1, §10.4,
  larmorx's own code (crates/larmorx-fsl, larmorx-mri hmc rigid/volume/image/files/report, larmorx-io nifti
  read_header, larmorx-core math/defects, larmorx-cli, larmorx-py fsl.rs, python/larmorx/fsl, _replica.py,
  _core.pyi), the suites parity/fsl_flirt.py and fsl_flirt_replica.py (cases and mechanics), oracle_cache
  core.run, parity/defects.py, tests/golden registry/test_golden, docs/api/fsl-flirt.md,
  docs/findings/fsl-flirt-original.md, part 1's validation/investigations/cleanroom-flirt/. Never opened FSL's
  source, crates-nc/, the replica crates, the restricted findings, docs/parts/, other investigations or probes,
  triage.toml, the replica's permanent cases, or git history beyond `git log --oneline -3` at start. **One
  slip:** a `grep -n` for FSL-14|FSL-76|FSL-77 over docs/findings/upstream-defects.md as a whole showed the
  summary table's rows (allowed) and two headings of † sections (lines 4337, 4804: titles only, no content
  read); not repeated.
- **Relay 2:** opened only CLAUDE.md, the brief, NOTES.md, relay 1's helpers, specs/flirt.md (§2 lines 51-135,
  §7.1, §7.4, §8.3-§8.5, §9-§9.4, §10a-§15 header, §15.5-§15.8), larmorx's own code (larmorx-fsl cli/ and
  flirt/, bet/image.rs, larmorx-mri hmc kernels/interp/volume, larmorx-core linalg/vnl_svd signatures,
  larmorx-cli, larmorx-py fsl.rs grep, python/larmorx/fsl, _replica.select, _core.pyi), the suites
  fsl_flirt.py, fsl_flirt_replica.py, fsl_mcflirt_replica.py helpers, tests/golden/registry.py,
  docs/api/fsl-flirt.md, docs/findings/fsl-flirt-original.md, crates/larmorx-fsl/PROVENANCE.md, part 1's
  investigation README. Never opened FSL's source or FSL's scripts in ~/fsl-6.0.7.23/bin (only ran them),
  crates-nc/ (only ran its built binary), the replica crates, the restricted findings, docs/parts/, other
  investigations or probes, triage.toml, the replica's permanent cases, the register, or git history beyond
  this branch's own `git log --oneline`.
- **Relay 3:** opened only CLAUDE.md, the brief, NOTES.md, relays 1-2's helpers, part 1's NOTES.md and its trace
  directory (FSL's own -verbose 7 output and part 1's), specs/flirt.md (§2.3-§2.5, §9.4, §12-§14, §6.5's
  field-map paragraph, §7.3's tolerance lines), larmorx's own code, the suites fsl_flirt.py and
  fsl_flirt_replica.py, harness in_tier, docs/api/fsl-flirt.md, docs/findings/fsl-flirt-original.md,
  docs/validation/fsl-flirt.md, docs/benchmarks/fsl-bet.md and cleanroom-bet/bench.py (models), FSL's bbr.sch
  (schedule data). Never opened FSL's source, crates-nc/ (only ran its built binary for timing), the replica
  crates, the restricted findings, docs/parts/, other investigations or probes, triage.toml, the replica's
  permanent cases, the register, or git history beyond this branch's own `git log --oneline`.
- **Relay 4:** opened only CLAUDE.md, the brief, NOTES.md, part 1's bin/records.sh, specs/flirt.md (§3,
  §4.1-§4.3, §5, §6.1-§6.5, §7.1 sort row, §7.2, §8.1-§8.6, §9.2, §9.3, §10a FSL-22 lines, §11, §12, §14 with its
  2026-11-02 table), specs/mcflirt.md (§4.4, §6.1-§6.3, appendix A), larmorx's own code, the suites
  fsl_flirt.py and fsl_flirt_replica.py (Run fields, the new cases, _undefined_case/_replica_undefined
  mechanics, cases()), mri_hmc.py (_run_larmorx), defects.py notices_in, compare_records.py usage, the
  findings fsl-flirt-original.md and mri-hmc-original.md, the records fsl-flirt.md and mri-hmc.md,
  docs/api/fsl-flirt.md, the PROVENANCE files, FSL's bbr.sch. Never opened FSL's source, crates-nc/ or
  crates-gpl/ sources (only `cargo check` of them, exit status only), the replica crates, the restricted
  findings, validation/investigations/flirt-spec-answers/ or other investigations, probes, triage.toml, the
  replica's permanent cases, docs/parts/, the register, or any git history (the merge of main brought its
  commits; no log or diff of them read).
- **Relay 5:** opened only CLAUDE.md, the brief, NOTES.md, part 1's NOTES.md, its bin/replay.sh, records.sh,
  golden.sh, its own triage notes and helpers cats.py, repsum.py; specs/flirt.md (§4.1's neurological
  paragraph, §6.5's boundary points and NaN lines, the FSL-91 lines of §6.4, §8.2's dof line, §8.4, §9.3, §12's
  first bullets, §15.7's type paragraph); docs/validation/difftest.md; the register's summary rows FSL-09 and
  FSL-76 to FSL-93 (by their `| FSL-` lines); larmorx's own code; the difftest descriptor and harness; the
  suites; thread_pools.py, test_memory_peak.py, test_thread_pools.py, tests/golden/registry.py, FSL's bbr.sch.
  **One slip:** a grep for "fsl-flirt-original" over validation/difftest/ printed lines of triage.toml (link
  lines of fsl-bet-original entries and part 1's own fsl-flirt-original entry text); not opened otherwise.
  Never opened FSL's source, crates-nc/ or crates-gpl/ sources (their gates ran with the output in files not
  read), the replica crates, the restricted findings, docs/parts/, other investigations or probes, the
  replica's permanent cases, or git history beyond this branch's own.
- **Relay 6:** opened only CLAUDE.md, the brief, NOTES.md, the relays' helpers, specs/flirt.md (§4.1, §9.2,
  §9.3, §12, §15.1, §15.2's mapping, §15.7, §8.2's dof line), the register's head (lines 1-40) and its summary
  rows FSL-09, FSL-70..81, FSL-90..93 (line range 40-522, `| FSL-` rows only), larmorx's own code,
  docs/findings/fsl-flirt-original.md, docs/api/fsl-flirt.md, crates/larmorx-fsl/PROVENANCE.md, the suites
  (run_original, oracle_run, _environment), oracle_cache core.run's signature, the difftest descriptor's
  make_recipe/case, the trial's results.jsonl, scripts' usage lines, the investigation README; ran the
  larmorx-nc replica's applyxfm4D as a black box (3 probes, both modes). **One slip:** a `grep -rln "FSL-93"`
  over the worktree printed file NAMES only (among them a docs/parts/ handoff and two crates-nc/ files); none
  was opened. Never opened FSL's source, crates-nc/ or crates-gpl/ sources, the replica crates, the restricted
  findings, docs/parts/, other investigations or probes, triage.toml, the replica's permanent cases, or git
  history beyond this branch's own.
- **Relay 7:** opened only CLAUDE.md (loaded), the brief, NOTES.md and the relays' scratch logs and helpers;
  the worktree's docs/figures.md; in the site clone its CLAUDE.md, README (the evidence section),
  content/tools/fsl-flirt.md and fsl-bet.md (a model), data/verdicts.yml, data/status.yml's FLIRT rows,
  site.yml's and content/roadmap.md's FLIRT lines, tools/blobs.py, tools/build.py's options,
  tools/evidence.py's metrics grouping and tools/review/shoot.mjs; larmorx's evidence code
  (evidence/fsl_flirt.py, fsl_bet.py and mri_hmc.py as models, common.py, core.py's compare and register
  signatures, summary.py, difftest.py's triage reading, figures/cache.py, figures/records.py), the suites
  parity/fsl_flirt.py (runner, comparison, band, threshold, cases) and fsl_flirt_replica.py (oracle_run,
  _environment), oracle_cache's run signature and Policy, the records fsl-flirt.md/.json and
  fsl-flirt-replica.md (summary, expected divergences), docs/benchmarks/fsl-flirt.md/.json,
  docs/findings/fsl-flirt-original.md (relays 5 and 6's sections), docs/api/upstream-defects.md's rows,
  the register's head (lines 1-40) and `| FSL-` summary rows of lines 40-522, validation/tools.toml's
  fsl-flirt blocks, docs/next-steps.md (the order, item 4, item 7's FLIRT part, the follow-ups near FLIRT),
  docs/validation/golden.md's FLIRT lines, docs/validation/difftest.md's triage section,
  scripts/orchestration/apply_handoff.py and boxed.sh, larmorx_core's FSL_111 line and
  crates/larmorx-fsl's search.rs minima lines, the trial's results.jsonl and report.md, and
  validation/investigations/cleanroom-bet/site_inventory.py (a clean-room part's tool). Ran: the evidence
  export (the original through `python -m larmorx`; FSL on four perturbed inputs through the oracle cache in
  use mode, never cached), four `difftest --replay`s of trial findings (FSL through the cache, use mode), the
  site's build and checks. **One slip:** a `grep -n "^| FSL-93 †"` over docs/findings/upstream-defects.md as
  a whole (to anchor the handoff's insert) printed the first 20 characters of that summary-table row (allowed
  content); nothing else. Never opened FSL's source, crates-nc/, crates-gpl/, the replica crates, the
  restricted findings (the site page links to docs/findings/fsl-flirt.md; not opened), docs/parts/, other
  investigations or probes, triage.toml (only `apply_handoff.py --check` read it in memory), the replica's
  permanent cases, or git history beyond this branch's own log.
