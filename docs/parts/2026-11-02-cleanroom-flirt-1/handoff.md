# Handoff: FLIRT's clean-room original, part 1 (2026-11-02)

Branch `worktree-agent-a083655161a925b75` (from main 3f07df7). The exact texts for the shared
documents follow; the orchestrator integrates and harmonises them. Numbers in `{...}` are the
tables' keys; `<N>` are filled from the record (`docs/validation/fsl-flirt.md`) at the end of this
file's "Record" section.

## Record (from the final clean commit)

`docs/validation/fsl-flirt.md` (commit b244d758acba): both-error 35, expected-divergence 4, fail 22, pass 89, skipped 7; compared cases 111.

Golden: `tests/golden/golden.json` recorded from 13012f3 (`record_golden.py --skip-parity`): the
nine FLIRT cases added, every other checksum unchanged; the parity smoke tier (2653 tests, 16
workers then the memory and thread tests serially, all passed) ran on b244d75, which differs from
13012f3 only in `tests/golden/registry.py` (two cases' outputs split into status and streams).
The orchestrator's re-recording after the merge replaces the `skipped` stamp.

mri-hmc: its standard tier re-run from b244d75 is identical to `docs/validation/mri-hmc.md`
(`compare_records.py`: 169/169 cases, no status or detail differs, summary equal).

## README.md (the tool table is generated: see validation/tools.toml below)

No hand edit beyond `python -m larmorx_validation tables`.

## validation/tools.toml

In `[[tool]] id = "fsl-flirt"`, replace `how` and `validation` with:

```toml
how = "clean-room original of `flirt` and `convert_xfm` (part 1), `experimental`; exact replica of every program in `larmorx-nc` (FSL Licence, non-commercial), `experimental` (parts 1 to 3: every program and script of fsl-flirt but `-warp`, `standard_space_roi`, `flirt_average`, `epi_reg`)"
validation = "original: [{fsl-flirt.pass} of {fsl-flirt.compared}](record:fsl-flirt) compared cases within thresholds against FSL 6.0.7.23 (fMRIPrep's three boundary-based calls within 0.1 mm, `-applyxfm` FSL's images bit for bit with trilinear and nearest neighbour, `convert_xfm`, the schedules' and the command line's messages FSL's), {fsl-flirt.both} more rejected by both alike, {fsl-flirt.expected} where FLIRT crashes or never finishes; the cases it misses are in its record (costs equal to FSL's printed digits, paths that part in the last bits; BBR's normals from `-wmseg`; ill-posed registrations); replica: [{fsl-flirt-replica.pass} of {fsl-flirt-replica.compared}](record:fsl-flirt-replica) compared cases bit-identical to FSL 6.0.7.23 (matrices, images, printed results, exit status and messages): fMRIPrep's boundary-based calls on three oracle runs (their matrices also identical as text to fMRIPrep's own), every cost function, the search options, every shipped schedule and every schedule command (`alignpaxes`, any parameter-subset mask), `-anglerep quaternion`, `-debug`, `-applyxfm` 3D and 4D with every interpolation and data type, BBR with its types, coordinates and a field map, avscale, the coordinate programs, rmsdiff, makerot, midtrans, pointflirt, applyxfm4D in every data type and interpolation, pairreg, extracttxt and aff2rigid; {fsl-flirt-replica.both} more rejected by both, alike; {fsl-flirt-replica.expected} where FLIRT crashes or never finishes (FSL-71, FSL-72, FSL-75)"
```

Add a record row before `[[record]] id = "fsl-flirt-replica"`:

```toml
[[record]]
id = "fsl-flirt"
larmorx = "`lx.fsl.flirt(implementation=\"original\")`, `lx.fsl.convert_xfm(implementation=\"original\")`, `larmorx fsl flirt`, `convert_xfm` (clean-room, `crates/larmorx-fsl`)"
reference = "flirt and convert_xfm (FSL 6.0.7.23); the replica suite's cases"
status = "`experimental` (part 1: {fsl-flirt.pass} of {fsl-flirt.compared} compared cases within thresholds, fMRIPrep's three calls included; {fsl-flirt.fail} miss them (listed in the record, explained in docs/findings/fsl-flirt-original.md); {fsl-flirt.both} rejected by both, with the same messages and status; {fsl-flirt.expected} where FLIRT crashes or never finishes)"
```

## docs/findings/upstream-defects.md (summary rows, behaviour only)

No new defect (FSL-90 to FSL-95 unused). In the "what larmorx does" column of these rows, after
the replica's text, add the original's:

- FSL-09: "; the clean-room original of FLIRT the same (2026-11-02)"
- FSL-11: "; FLIRT's clean-room original writes `.nii.gz`; `reproduce` prints FSL's message and exits 1 (2026-11-02)"
- FSL-14: "; FLIRT's clean-room original: non-finite values left out of the costs by default (2026-11-02)"
- FSL-21, FSL-66: "; FLIRT's clean-room original: refused in both modes where a histogram cost is evaluated at a scale with no bins, or for negative bins at any cost (2026-11-02)"
- FSL-22, FSL-75: "; FLIRT's clean-room original: the same (the whole border by default, FSL's buffer with `reproduce`, a single voxel refused in both modes, also a 1 x 1 x 1 level of the pyramid; NaNs sorted last), named (2026-11-02)"
- FSL-67: "; FLIRT's clean-room original: kept (2026-11-02)"
- FSL-68: "; FLIRT's clean-room original: exit 1 by default, FSL's 0 with `reproduce`, named (2026-11-02)"
- FSL-71, FSL-72: "; FLIRT's clean-room original: refused in both modes (2026-11-02)"

## docs/next-steps.md (item 7, FLIRT's clean-room original)

Status: "**Part 1 done (2026-11-02):** `flirt` and `convert_xfm` (crate `larmorx-fsl`, module
`flirt`, on `larmorx_mri::hmc`'s shared engine; record `fsl-flirt`: 89 of 111 compared
cases within thresholds, fMRIPrep's three BBR calls within 0.1 mm; `-anglerep quaternion` and
`alignpaxes` included). **Part 2:** `-debug`, the verbose levels above 1, FLIRT's other programs
(avscale, the coordinate programs, rmsdiff, makerot, midtrans, pointflirt, applyxfm4D, pairreg,
extracttxt, aff2rigid), the site's clean-room panels, the benchmark report and speed; the record's
open cases once the spec writer answers the spec questions below (BBR's normal masks and
smoothing, `local_abs`, the correlation ratio's last bits, sinc's arithmetic, `-usesqform`'s and
`-fixscaleskew`'s precision, the spline with `-setbackground`)."

Follow-up for `lx.mri.hmc` (its own part; not changed here, its record stays identical): the
correlation ratio's sparse-bin rule found here (a reference bin with at most two points is left
out of the counts, the sums and the total variance; `docs/findings/fsl-flirt-original.md`, "The
costs") probably holds for mcflirt's `-cost corratio` too (`specs/mcflirt.md` appendix A leaves the
sparse bins in the total); and the search findings (a subset's convergence test over its own
tolerances) for its schedule-free optimiser if it uses subsets. A spec-writer question first.

## docs/validation/golden.md

Add to the list of covered tools: "`larmorx fsl flirt` and `convert_xfm`, `lx.fsl.flirt` and
`lx.fsl.convert_xfm` (the clean-room originals, 2026-11-02): `cli.fsl.flirt` (the default schedule,
`-v`, `-omat`, `-out`), `cli.fsl.flirt.costs` (every cost, weights, `-2D`), `cli.fsl.flirt.bbr`
(`-wmseg` with `local_abs`, `-wmcoords`/`-wmnorms`), `cli.fsl.flirt.schedule` (most commands),
`cli.fsl.flirt.applyxfm` (every interpolation, `-paddingsize`, `-setbackground`, `-datatype`,
`-applyisoxfm`), `cli.fsl.flirt.errors` (the scan's messages, FSL-68 in both modes, FSL-21's
refusal), `cli.fsl.convert_xfm`, `py.fsl.flirt`, `py.fsl.convert_xfm`; `lx.fsl.flirt` and
`lx.fsl.convert_xfm` leave `REPLICA_ONLY_FUNCTIONS`."

## CLAUDE.md, "Where things are", FSL row

After "...`specs/flirt.md` for the clean-room original (§14: only what is left by decision)" add:
"; FLIRT's **clean-room original**, part 1 (`crates/larmorx-fsl`: `larmorx fsl flirt`,
`convert_xfm`, `lx.fsl.flirt`, `lx.fsl.convert_xfm` with `implementation=\"original\"`;
`experimental`; 2026-11-02): on the registration engine it shares with `lx.mri.hmc`
(`larmorx_mri::hmc`), 89 of 111 compared cases within thresholds, fMRIPrep's three BBR
calls within 0.1 mm, `-anglerep quaternion` and `alignpaxes`; part 2: `-debug`, verbose levels
above 1, FLIRT's other programs". And in the table's Records column add `fsl-flirt`.

## The site's Roadmap row

FLIRT's clean-room original: "In progress: part 1 done (2026-11-02: `flirt` and `convert_xfm`
on the shared registration engine, fMRIPrep's boundary-based calls within 0.1 mm of FSL's);
part 2 next (`-debug`, FLIRT's other programs, the site's panels)."

## validation/difftest/triage.toml (behaviour only)

The trial: `difftest --tools fsl-flirt-original --budget 0.45` (27 minutes, seed 20261102, 8 cases
at a time on one thread each, realistic cases only, a 24 GiB box with 3 GiB per case), on WIP 3
(`0231f5b`): 5,668 cases, 2,166 agree, 3,242 differ, 260 skipped (part 2's options), 41 rejected
by both. The differences, by kind (the trial's categories), and what became of them; a stratified
sample of 282 findings (12 per category) was replayed (`difftest --replay`, 8 at a time, a 32 GiB
box) on WIP 7 (`b5bd9ab`): 154 agree, 128 differ; the fixes after the replay are named.

```toml
# Proposed triage entries for fsl-flirt-original (behaviour only). Each `fixed` entry names the
# original's change; `open` entries name the reason and the spec question.

[[entry]]
tool = "fsl-flirt-original"
signature = "stderr: WARNING::in calculating COG, total = 0.0 (a count of warnings)"
status = "fixed"
reason = "the search measures both centres of mass again (one more warning for an empty level)"
cases = "~1,300 in the trial; 11 of 12 replayed agree"

[[entry]]
tool = "fsl-flirt-original"
signature = "out.mat: displacement (registrations), every cost"
status = "fixed"
reason = "the correlation ratio's and Woods' sparse bins; the pair's centre kept at a reset; the grids' dof, sampled parameters, minima order and start costs; a subset's convergence count (docs/findings/fsl-flirt-original.md)"
cases = "~2,000 in the trial; of the replayed registrations most agree; what remains is listed below"

[[entry]]
tool = "fsl-flirt-original"
signature = "exit status: flirt killed by heap corruption (malloc), the original exits 0"
status = "register"
defect = "FSL-75"
reason = "FSL's background-value buffer overflow on small levels (5 or 6 voxels along x or y); the original takes every border value and names FSL-75 (specs/flirt.md §10a); the suite now accepts exactly this"
cases = "~370 in the trial; 84 of 86 replayed agree (the 2 others: the degenerate shrunk input below)"

[[entry]]
tool = "fsl-flirt-original"
signature = "out.<image>: the headers differ (srow, quatern) with -applyisoxfm"
status = "fixed"
reason = "an unset sform keeps the reference's rows through the two grids' x flips; the qform's fields only when set"
cases = "~210 in the trial; the replayed cases agree but one: quatern_b/c in their last float bit (-0.07943016 against -0.07943015) on an oblique qform (open, spec question 9)"

[[entry]]
tool = "fsl-flirt-original"
signature = "convert_xfm -inverse: the text differs"
status = "fixed"
reason = "the inverse in LAPACK's order (dgetrf, dgetri through dtrti2): FSL's -0"
cases = "21 in the trial; 12 of 12 replayed differed on WIP 7, the sign of a zero; agree since WIP 8"

[[entry]]
tool = "fsl-flirt-original"
signature = "files written: only the original out.mat (-applyxfm with FSLOUTPUTTYPE=ANALYZE)"
status = "fixed"
reason = "-applyxfm writes the image before the matrix: an output type FSL cannot write ends the run first"

[[entry]]
tool = "fsl-flirt-original"
signature = "zstd and bzip2 outputs: the comparer could not read them"
status = "fixed"
reason = "the suite's comparer reads them through FSL's own zstd (fsl_bet.content); harness only"

[[entry]]
tool = "fsl-flirt-original"
signature = "out.mat: displacement on degenerate shrunk inputs (1 x 1 x 1 images, tripled), FSL's own p95 0"
status = "open"
reason = "the shrinker's minimum of many findings: a single voxel registered (an arbitrary matrix in both); the whole recipes are the realistic cases; candidate for a rule-B refusal of images with fewer than 2 voxels along every axis (FSL-90 if the orchestrator registers it)"

[[entry]]
tool = "fsl-flirt-original"
signature = "out.mat: displacement beyond FSL's own variability with an all-zero -refweight or -inweight"
status = "open"
reason = "the random weights (blobs about the world origin) fall outside many images: every weighted cost is 0/0, the registration is undefined in both; candidate for a rule-B refusal (a weight of zeros), with the defect ID from the orchestrator"

[[entry]]
tool = "fsl-flirt-original"
signature = "out.mat: displacement within or near FSL's own variability (ill-posed registrations), out.<image>: NCC"
status = "open"
reason = "the costs agree with FSL's printed digits; the paths part in the last bits (spec question 3) and ill-posed registrations amplify it; the image check is reported, not held, when the matrix passes by FSL's own variability"

[[entry]]
tool = "fsl-flirt-original"
signature = "stderr: ERROR::set_bbr_seg / WARNING:: Found NaN in BBR cost (2) order, -bbrtype local_abs, BBR's normals"
status = "open"
reason = "BBR's boundary points and normals from -wmseg (spec question 4); the messages' order on an empty segmentation"

[[entry]]
tool = "fsl-flirt-original"
signature = "stderr: Erroneous dof 3 : using 6 instead (one line fewer), Parameters do not form a valid axis (quaternions), WARNING: Woods ... (counts)"
status = "open"
reason = "per-evaluation messages whose count follows the optimisation's path (the last bits of spec question 3)"

[[entry]]
tool = "fsl-flirt-original"
signature = "out.<image>: a few voxels of -applyxfm (trilinear, nearest) differ"
status = "open"
reason = "2 of 16 voxels on a tiny 4D case with -setbackground and similar; not reproduced on the suite's images (bit-identical); to trace in part 2"
```

## Spec questions (for the spec writer)

Observed by the clean-room implementer (black box, `docs/findings/fsl-flirt-original.md`); each
could be stated in `specs/flirt.md`, or corrected where the observation is wrong:

1. **`corratio` and `woods`: sparse bins** (§6.2, `specs/mcflirt.md` appendix A): the points of a
   reference bin with at most two (weighted) points are left out of the counts, the sums **and**
   the total variance (`corratio`), and of the count (`woods`). With it the original's costs are
   FSL's printed values at every pair, `-noresample` and weights included (the weighted value
   above 1 at the 8 mm pair, `1.016221`, is this rule). Please confirm.
2. **`woods`**: `Σ_b n_b·σ_b/μ_b / n` (σ with `n_b − 1`; appendix A's `n_b²` is not FLIRT's):
   please confirm.
3. **The correlation ratio's last bits**: with single-precision running sums per bin in the
   sampling order and the final formula in double, the 9th evaluation of `-verbose 7`'s trace
   differs from FSL's in the 5th decimal of a parabolic step; neither double sums, nor a
   three-level (row, slice, volume) summation as §12 describes, nor single-precision final
   arithmetic gives FSL's 9th and 10th steps both. Which sums and precisions?
4. **BBR's normals (§6.5)**: "the gradient masks of `specs/bet.md`'s smoothing step" are not in
   `specs/bet.md`. Which 27 weights per component, which Gaussian (kernel and width for σ = 2 mm),
   and the boundary voxels' order? The cost of the initial matrix of `bbr/default` with `bbrstep`
   1: FSL 0.537036, the original 0.537732 (variants in the findings).
5. **`-bbrtype local_abs`**: `min(u, 2 − u)` per term leaves the original 1.4 mm from FSL.
6. **The search (§8.1)**, observed with `-verbose 7`: (a) the grids' subset and scale rule follow
   the `search` command's dof (12 by default), not `-dof` (with `-dof 6` the coarse grid optimises
   the common scale, the fine grid keeps the median scale); `-dof` applies to the free
   optimisations; (b) the grid points and the minima start from their sampled parameters, not a
   decomposition of their matrices; (c) the minima are listed z outer, then y, then x; (d) `P`
   holds each start with its own cost; (e) both centres of mass are measured again (their
   warnings).
7. **The convergence test (§8.2, §8.3)**: in the subset form `n_t` is the subset's number of
   tolerances (4 for the search's), not 12.
8. **`setscale` that only resets the input (§7.2)**: the existing pair keeps its centre (the
   previous input's centre of mass).
9. **`-applyisoxfm` (§9.3)**: an unset sform (code 0) keeps the reference's rows but for the x
   flips of the two grids (`W·F_n·F_n'`, no scaling); the qform's fields follow only when set.
10. **The output of `-applyxfm`**: the image is written before `-omat` (an output type FSL cannot
    write ends the run before the matrix).
11. **`inv(A)`'s zeros**: LAPACK's `dgetri` order (via `dtrti2`) gives FSL's `-0` in
    `convert_xfm -inverse`; the original now follows it; are OpenBLAS's kernels' fused
    multiply-adds part of it (§12)?
12. **`alignpaxes` with a non-finite moment**: FSL prints an empty line and `warning: eig_sym():
    given matrix is not symmetric` before `Mat::operator(): index out of bounds`.
13. **The windowed sinc of `-interp sinc` (§9.2)**: a third of the voxels differ from FSL's in
    the last bit: the table's precision and lookup, the products' order, the sums' precision.
14. **The spline with `-setbackground`** (`real/applyisoxfm-4d-blur-setbackground`): the spline's
    coefficients outside the grid with a constant padding.
15. **`-usesqform`'s matrix (§8.5)**: FSL's translation is 4.5e-6 mm from the double products.
16. **`convert_xfm -fixscaleskew`**: FSL's matrix differs in the 8th digit from §3's decomposition
    and composition with single-precision sines and cosines.
17. **The default schedule's first pair**: a schedule with no `setscale` runs on a pair made before
    the schedule (the 8 mm level, the input blurred to 8 mm): please state it in §7.
18. **The base scale** is decided before single slices are tripled (FSL-75's case): §4.2 could say
    so.
19. **Powell's direction's tolerance (§8.2)**: the line search along `x_end − x_start` needs a
    tolerance; the original uses one tolerance unit along the direction, `1/‖d ⊘ t‖`.
20. **Paddings**: with `-paddingsize p` the voxels up to `p` voxels outside are interpolated by the
    extraslice rule; with a field map the output has no padding test.

## Clean-room statement

I wrote FLIRT's clean-room original (`crates/larmorx-fsl/src/flirt/`, `cli/flirt.rs`,
`cli/convert_xfm.rs`) and its changes to the shared engine (`crates/larmorx-mri/src/hmc/`) from
`specs/flirt.md`, the parts of `specs/mcflirt.md` and `specs/bet.md` it refers to, larmorx's own
Apache-2.0 code and black-box runs of FSL 6.0.7.23's `flirt` and `convert_xfm`.

**Read:** CLAUDE.md and the brief (not `docs/porting-guide.md` or `docs/architecture.md` beyond
CLAUDE.md's summary of them); `specs/flirt.md` in full; `specs/mcflirt.md` (§4, §5.3, §6, §7, §9,
§11's option table, §12, §13, appendices A and B); `specs/bet.md` (short passages, looking for the
gradient masks the FLIRT spec names); larmorx's own Apache code (`crates/larmorx-mri` hmc and its
PROVENANCE, `crates/larmorx-fsl` bet/betsurf/cli, `crates/larmorx-cli`, `crates/larmorx-py`'s FSL
bindings, `larmorx-core`'s defects list, `larmorx-io`'s header fields); `docs/api/fsl-flirt.md`;
`docs/validation/fsl-flirt-replica.md` and `docs/validation/mri-hmc.md` (headers and case tables);
the validation code the suite builds on: `parity/fsl_flirt_replica.py` (its cases and mechanics,
which the original's suite reuses; it holds behaviour descriptions, no FSL code),
`fsl_bet.py` (its zstd/bzip2 readers), `harness.py`, `__main__.py`, `thread_pools.py`,
`tests/parity/test_memory_peak.py`, `tests/golden/registry.py`, the difftest descriptor
`difftest/tools/fsl_flirt.py`, `difftest/cli.py` and `difftest/inputs.py`.

**Exposures to report.** (1) A `grep` of `docs/findings/upstream-defects.md` for the FLIRT rows
displayed the summary rows FSL-67 to FSL-81, which are marked † (restricted), truncated to 400
characters; FSL-75's, FSL-79's and FSL-80's rows name FSL routines (`calc_bval`,
`subsample_by_2`, `configure_interpolation`, `SplineColumn::Set`, a kernel cache). An earlier read
of the register's summary rows FSL-09 to FSL-75 by line range (before this session's summary) may
have shown † rows too. Nothing from them went into the code: what the code does for FSL-67, 68,
69, 71, 72, 75 follows `specs/flirt.md` (§6.5, §7.2, §10a, §11), which states the same behaviour,
and FSL-79/80 concern applyxfm4D (part 2). (2) A `grep` over `crates/` printed two lines of
`crates/larmorx-afni-replica/src/environ.rs` and one of its PROVENANCE (AFNI environment-variable
names, `AFNI_TRACE`); unrelated to FLIRT, nothing used.

**Never opened:** FSL's source (`~/fsl*/src`, `crates-nc/fsl-source/`), the replica crates
(`crates-nc/`, `crates/larmorx-afni-replica`, `crates-gpl/`) beyond the grep lines above, the
restricted findings (`docs/findings/fsl-flirt.md`, `fsl-bet.md`, `fsl-mcflirt.md`, the AFNI
source-derived ones), `docs/parts/`, the replica parts' investigations and probes,
`validation/probes/`, `validation/difftest/triage.toml`, the replica authors' permanent cases, the
git history of the replica parts, `fsl_versions.py`.

**Black boxes:** FSL 6.0.7.23's `flirt` and `convert_xfm` (`~/fsl-6.0.7.23/bin`, FSL off the
`PATH`): through the oracle cache (the suite, the probes, the trial) and about 61 direct runs in
memory boxes (probes of messages and warnings, `-verbose 7` traces of three registrations, one
`-debug` run, the speed table's three); FSL's shipped schedule files (`etc/flirtsch/*.sch`) were
passed to FLIRT as inputs, never read; the default schedules' text was taken from `flirt -v`'s
output. The replica's program (`larmorx-nc`, the main checkout's build) ran for the speed table
only; `crates-nc` and `crates-gpl` were compiled by the gates without being read.
