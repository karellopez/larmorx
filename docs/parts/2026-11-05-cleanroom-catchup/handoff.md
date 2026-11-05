# The clean-room catch-up: handoff (the reasons; the exact changes in HANDOFF.toml)

Each relay appends its tool's section. `python -P scripts/orchestration/apply_handoff.py
<scratch>/HANDOFF.toml --check` checks every operation; afterwards `python -m larmorx_validation
tables` and `register_counts.py`.

## topup (relay 1, commits a1074a7, 6bb746f, 72b69ff)

What changed in topup's clean-room original (`crates/larmorx-fsl/src/topup/`, `cli/topup.rs`,
`cli/applytopup.rs`), each from `specs/topup.md`'s answers of 2026-11-03 and black-box runs
(`docs/findings/fsl-topup-original.md`, findings 37 to 44):

- **FSL-201's `reproduce`**: FSL's single-precision rotation matrices (glibc's `cosf`/`sinf` from
  `larmorx_core::math`) and their forward differences; `--rbmout` writes FSL's matrices in that
  mode. The default names FSL-201 on every run that estimates the movements (it did not before).
  The first gradients equal FSL's to 2e-5 (`debug/movements`) and 1e-6 (four volumes). FSL's own
  path is not determined beyond its inputs' last bits (one bit of a read-out time moves FSL's
  corrected images by up to 5e-3, and by 2e-2 on four volumes; `fsl201_spread.py`): six movement
  cases of the record stay expected divergences, now with FSL's measured spread beside the
  original's in their reasons; the default's own spread is 1e-7 to 1.4e-5 (proportional to the
  change). A point for the site's FSL-201 text and the register: FSL-201 also makes FSL's
  results ill-determined, which the correction removes.
- **FSL-145's `reproduce` exactly**: FSL's counts (1 when `n < k`, 2 when `k <= n < 2k` at
  `k >= 3`), FSL's errors at every refit of a single coefficient, the outputs' refit included
  (`Error occured while writing output from topup`, a black-box run); applytopup reads FSL's count
  only in `reproduce` (FSL aborts on the general count), either count by default, naming FSL-145
  when it reads the general count on a short axis.
- **FSL-133's `reproduce`** (FSL's `(k + 2^32) mod n` read) and **FSL-134's field of another
  shape** in `reproduce`; applytopup's `lsr` matrices row by row by FSL's walk (both modes, a
  rule-C convention of the spec).
- **Both modes**: the spline prefilter's truncated start (§4.5, axes shorter than 15 voxels), the
  regulariser's sampling of quadratic B-splines at an even spacing (§4.6).
- **Two mistakes of the original found and fixed** (no register entry: larmorx's): the mask's
  frame followed the displacement `t p`, so a volume with a read-out time of 0 was framed as
  encoded along no axis (FSL-146's trigger likewise: brief item 8); the moved volumes' mask lost
  the frame and checked the target grid instead of the image's bounds.

The record `fsl-topup` re-made from 6bb746f (clean): 183 cases, 102 within the thresholds, 67
rejected by both, 14 expected divergences, 0 failures (main's: 94, 66, 22, 1). Against main's
(`compare_records.py`): 10 expected divergences now pass (`opt/scg-movements`,
`opt/zero-readout`, `cfg/b02b0`, `cfg/b02b0_2`, `at/lsr-long`, `at/vb2d-long`, `at/vb3d-long`,
`at/vb4d-long`, `at/jac-other-shape`, `difftest/fsl145-short-axis`); the failure
`opt/fsl145-one-coefficient` is rejected by both alike; `opt/int16` and `opt/interp-linear`
(passes before, by the exact derivatives' smooth path) are now expected divergences of FSL-201's
path, with FSL's spread measured; 66 cases' details differ (metrics; the fix pass now counts the
values FSL-201's correction changes).

The shared documents (HANDOFF.toml): next-steps' topup items closed (with the outcome of each of
the eight), the "originals' suites hold the spec answers' new cases" item (topup's part done),
the record-notes item; `validation/tools.toml`'s `fsl-topup` tool and record blocks (no failure
left; the expected divergences' reasons); the register's summary rows of FSL-133, FSL-134,
FSL-145, FSL-146 and FSL-201 (the original's `reproduce`; FSL-145's and FSL-201's gap
`reproduce` closed: two gaps fewer, `register_counts.py`); `docs/api/upstream-defects.md`'s rows
of FSL-133, FSL-134, FSL-145 and FSL-201 (both implementations).

Not in the handoff, for later relays of this part (sub-task 6): the topup golden cases'
checksums move (all 6: the truncated prefilter on their small images, the walk, the masks):
re-record with `record_golden.py` once; the random-testing trial for topup; the site's topup page.

## FAST (relay 2, commits 0ab7292, 4a1ba04, ccdbb34, 980dd20)

What changed in FAST's clean-room original (`crates/larmorx-fsl/src/fast/`, `cli/fast.rs`), each
from `specs/fast.md`'s answers of 2026-11-03 (§12) and black-box runs
(`docs/findings/fsl-fast-original.md`, "The catch-up of 2026-11-05"):

- **FSL-202 by default** (the variance with the double-precision mean's square, in the class
  statistics and the partial volumes'), `reproduce` FAST's; named in both modes. With glibc's
  functions (`validation/investigations/cleanroom-fast/glibc_default.patch`, a development switch
  never committed into the crate) the original's default is the replica's default bit for bit,
  standard output and every image, on every single-channel case of the suite (fMRIPrep's three
  calls among them: `glibc_default.py`); several channels differ there only in their partial
  volumes (larmorx's own LU, §8.6). So the two implementations' corrections are the same
  arithmetic.
- **FSL-203 by default** (the extrema of the values that are not NaN), `reproduce` FAST's; named
  in both modes; FSL-103's count now uses the corrected maximum (`opt/nan-first` named FSL-103
  before: now rejected by both alike, its default naming FSL-203).
- `_restore` keeps a NaN voxel NaN unless the rule for negative values' second branch clears it
  (§4.1.1); standard output written as FAST buffers it (an abort loses the unflushed k-means
  lines and partial-volume heading, §7); three orders observed and now the original's (the
  heading before the images are read; FSL-100's and fewer than 0 classes' aborts after the image
  lines) and `-n 0`'s loops without a class; several channels' class without weight (case 1840:
  FSL's `MEAN -nan`, permanent case `multi-class-without-weight`).
- **The suite** (`parity/fsl_fast.py`): the default is now also compared with the replica's
  default (`larmorx-nc`, `specs/fast.md` §11), with no band for one channel and FSL's own band for
  several channels that correct nothing compared; 13 cases of the original's own (`original/`)
  with two derived inputs (`<workspace>/oracles/inputs/fsl-fast/`).

The record `fsl-fast` re-made from ccdbb34 (clean): 102 cases, 64 within the thresholds, 35
rejected by both, 2 expected divergences, 1 failure (main's: 88 cases; 60, 25, 1, 2). Against
main's (`compare_records.py`): the comparison with FSL (`reproduce`) unchanged on every one of
main's 88 cases; `opt/nan-first` fail -> rejected by both alike (FSL-203); `multi/s2-manualseg-cycle`
pass -> expected divergence (the new comparison: both defaults correct FSL-98's cycle, and the
correctly rounded functions move `_mixeltype` to Dice 0.99880 from the replica's default; with
glibc's functions the labels are the replica's exactly; FSL's own band 0.99941 on FAST's
uncorrected order); 14 new cases (13 `original/`, 1 permanent), all pass or rejected by both
alike; the fix pass counts of 34 single-channel cases change (FSL-202 in the default).
`multi/s5` stays the one failure (explained). fMRIPrep's three calls: the default's labels the
replica's default's exactly, partial volumes 0 to 3.5e-7 apart.

The shared documents (HANDOFF.toml, operations 15 to 23): next-steps' FAST items closed (the
answers' six, case 1840's, FSL-202's "follows in its next part", `opt/nan-first`);
`validation/tools.toml`'s `fsl-fast` tool and record blocks; the register's summary rows of
FSL-202 and FSL-203 (the original's gap closed: two gaps fewer, `register_counts.py`);
`docs/api/upstream-defects.md`'s FSL-202 row (FSL-203's says "the same").

For a spec writer (`specs/fast.md`, not edited): §11's "the default against the replica's
default, a narrow band" holds for one channel only (several channels have no FSL-202: their
defaults lie FSL's own last-bit band apart, `multi/s3` and `multi/s2-n4` to every digit FSL
against FSL with correctly rounded maths); §7 and §4.1.4 could state what black-box runs showed:
`Starting ... Image Segmentation` printed before the images are read (and before `-S 0`'s
error), FSL-100's abort after the image lines, fewer than 0 classes' abort after them and after
FSL-100's, and `-n 0`'s lines (the findings' items 1 to 4).

Not in the handoff, for later relays of this part (sub-task 6): FAST's golden cases move
(`cli.fsl.fast`, `.bias`, `.reproduce`, `py.fsl.fast`: FSL-202 in the default; `.multi` and
`.priors` unchanged): re-record with `record_golden.py` once; FAST's random-testing trial; the
site's FAST page (the default-vs-replica comparison is new evidence).

## FLIRT, programs and messages (relay 3: items L1 to L8; commits 5fe9831, 62d177c)

What changed in FLIRT's clean-room original (`crates/larmorx-fsl/src/flirt/`, `cli/`), from
`specs/flirt.md` §14's answers of 2026-11-03 and black-box runs
(`docs/findings/fsl-flirt-original.md`, "The clean-room catch-up: programs and messages"):

- (1) §2.3 confirmed (no change). (2) §8.1 step 9: an invalid quaternion composition keeps only its
  scales and skews (no translations), each start composed once; the free optimisations' messages
  in FSL's order, start by start (each optimisation's own log, passed on in order: deterministic at
  any thread count). (3) avscale confirmed: the trial's 82 avscale findings agree. (4) midtrans:
  `|det| < 1e-6` is FSL's test; averages beyond double precision refused in both modes, **FSL-241**
  (new; rule B; the replica writes FSL's NaN rows: gap `replica`). (5) FSL-204 refused after FSL's
  lines; `nan` words read 0; an overflow stops as FSL's SVD (signal 6, exit 1). (6) `-warp`: exactly
  three volumes. (7) §12: the 4 × 4 inverse fails only on an exact zero (NaN and infinite matrices
  inverted; with `reproduce` FLIRT writes its NaN matrices); `convert_xfm -fixscaleskew` aborts as
  avscale where its decomposition does not solve; pairreg prints the shell's `Aborted` after an
  aborted step: 37 of the 57 trial findings that stopped at `inv(): matrix is singular` agree
  (three permanent cases). (8) pairreg's three schedules carried (from FSL's `-v` echo): no
  `$FSLDIR`.
- Found on the way (FSL's lines): the `-verbose 3` centres' COG warning, the levels' TR, the
  sampling and initial matrices in scaled mm, a registration's 4D images warned twice each.
- The suite (`parity/fsl_flirt.py`): 11 cases of the original's own (`verbose/2-basescale-tr`,
  `verbose/3-empty-reference`, `real/anglerep-quaternion-dof3`, `programs/pointflirt-*` 3,
  `programs/*-warp-*` 2, `programs/pairreg-verbose`) and 2 refusal cases of FSL-241 (a new
  payload kind, an expected divergence: FSL finishes, the original refuses in both modes);
  `difftest/permanent.py` maps the suite `fsl-flirt` to `fsl-flirt-original`'s permanent cases.
  A development run of the standard tier (not the record): 319 cases, 214 within the thresholds,
  88 rejected by both, 10 expected divergences, 7 failures (the six of items 9 to 13, and
  `verbose/2-basescale-tr`, fixed since: the 4D warnings); every status of main's record's cases
  unchanged but `programs/pointflirt-no-points` (fail -> expected divergence).

The shared documents (HANDOFF.toml, operations 24 to 31): next-steps' FLIRT items 1 to 8 closed
with their outcomes (and the two places that named `programs/pointflirt-no-points`), the "FLIRT's
NaN signs" follow-up's new observation; the register's FSL-204 row (the original's gap closed) and
the new FSL-241 row (gap `replica`: `register_counts.py`); `docs/api/upstream-defects.md`'s FSL-204
and FSL-241 rows. `validation/tools.toml`'s `fsl-flirt` blocks are relay 4's (the record).

For a spec writer (`specs/flirt.md`, not edited): (a) §15.5 could say what FSL does with averages
beyond double precision (NaN rows; the failed decomposition reported as `incompatible matrix
dimensions: 0x0 and 3x1`), so a `reproduce` of FSL-241 can be written; (b) §3 or §12: how FSL's
compositions keep a matrix's 3 × 3 part finite when the centre is NaN (finding 5009 of the
2026-11-02 trial: free optimisations ending at rotations 0, translations NaN; the original's
compositions spread the NaN into every element), and the NaN signs (§14's question 2); (c) §2.3
could name the 4D warning's two readings, the levels' TR and the scaled sampling and initial
matrices; (d) §15.8: the schedules' exact lines are those relay 3 took from FSL's echo (the
spec's "one comment before each identity row" of `pairreg2.sch` is not FSL's text: it has none).

Not in the handoff, for later relays: the golden cases `cli.fsl.pairreg` and `py.fsl.pairreg` move
(the carried schedules replace the case's stand-in ones): re-record with `record_golden.py` once,
with topup's 6 and FAST's 4; the random-testing trial and the triage entries of FLIRT (sub-task 6).

## FLIRT, numerics, `-debug` and the record (relay 4: items L9 to L13; commits 87cc8d9, b63d962, 23710fe, 7863c30, 78014e5)

What changed in FLIRT's clean-room original (`crates/larmorx-fsl/src/flirt/`, `cli/applyxfm4d.rs`),
from `specs/flirt.md`'s answers of 2026-11-03 and black-box runs of FSL 6.0.7.23 through the oracle
cache (`docs/findings/fsl-flirt-original.md`, "The clean-room catch-up: numerics, `-debug` and NaN
centres"):

- **L12** `leastsq`'s three-level single-precision sums (§6.2): `real/cost-leastsq` passes (82 mm
  before). FLIRT's own accumulator: `larmorx_mri::hmc::cost::least_squares_weighted` (mcflirt's,
  double sums) is unchanged; whether mcflirt's `leastsquares` has the same three levels is not in
  `specs/mcflirt.md` (a question for its spec writer, for `lx.mri.hmc`'s sub-task).
- **L11** applyxfm4D's spline is FLIRT's own (§9.3, §15.7): float passes in single, a double image
  read and filtered in double, FSL-79's integer passes; the five modelled findings' 5,177 voxels
  are FSL's now (two permanent cases).
- **L9** `-debug`: the levels' headers from the resampling's steps (one function with
  `-applyisoxfm`'s header), `global_refweight1`'s qfac and `pixdim[4..7]` 1; observed beyond the
  spec: `-noresample`'s levels (the reference's own forms, no `global_refweight1`), `testvol`'s held
  voxel sizes, the first pair's BBR set-up after the verbose centres; BBR without boundary points
  repeats the set-up's lines and error at every evaluation (one evaluation's lines pushed together:
  the grids evaluate in parallel), writes `debug` (zeros) and goes on to `-omat` and `-out`. 123 of
  the trial's 161 `-debug` findings agree (52 before).
- **L10** wired since the glibc maths part; the spline horizon's `ln` is now `math::log`.
- **L13** equal and NaN costs move nothing (the line search was right); what moved the NaN-centre
  runs was elsewhere, found by black-box probes: FSL starts an optimisation from the identity's
  parameters where its own are NaN; a NaN pre-image is outside (the background value); a NaN
  fine-grid scale is kept; the grids' `Starting with :` is the identity's parameters composed. Two
  observations of ordinary runs: a perturbed row is composed again with all twelve parameters
  (`measurecost 6` of a scaled, skewed row keeps them; the original dropped them), and the
  search's `P` keeps each start composed once (§8.1 step 9). 52 of the 57 non-finite findings of
  the two 2026-11-02 trials agree (37 after relay 3).

The record `fsl-flirt` re-made from `23710fe` (clean): 333 cases, 229 within the thresholds, 89
rejected by both, 10 expected divergences, 5 failures (main's, from `be3db91`: 308 cases, 210 pass,
7 fail). `compare_records.py`: `real/cost-leastsq` fail -> pass; `programs/pointflirt-no-points` fail
-> expected divergence (relay 3's FSL-204); `real/cost-leastsq`, `real/bins64` (0.0692 mm) and
`real/weights` (0.3059 mm) now FSL's matrices as text; fMRIPrep's ds005040 call 0.0065 -> 0.0113 mm
(a boundary-based path, whose costs are not FSL's to the last bit, parting elsewhere since the
perturbed rows keep their scales; the other two calls unchanged, 0.0000 and 0.0002); 25 new cases
(relay 3's 14; relay 4's `debug/bbr-no-boundary`, `debug/noresample`, `schedule/perturb-scaled` and 8
permanent cases). The 5 failures: FSL-91's and FSL-92's three `reproduce` runs (traces FSL's to line
66,124 of 140,494; parted at a zero skew's sign under gimbal lock) and `real/weights-normcorr`,
`real/weights-woods` (parted at the 4 mm stage; weighted costs' last bits): spec questions (a) and (b)
of the next-steps operation.

The shared documents (HANDOFF.toml, operations 31 to 35): next-steps' FLIRT item (relay 3's "Left for
relay 4" replaced by items 9 to 13's outcomes and the record), the 2026-11-02 "what is left" item
(the sinc and spline last bits, `-debug`'s headers and avscale's sign closed; the spec writer's
questions (a) to (e)), `validation/tools.toml`'s `fsl-flirt` tool and record blocks (FSL-204 now in
the original, FSL-241's refusals among the expected divergences, fMRIPrep's calls within 0.012 mm,
the failures' new reasons). No new defect: no register or API defect operation. After applying:
`python -m larmorx_validation tables` (the counts move).

Not in the handoff, for later relays: golden `cli.fsl.flirt`, `cli.fsl.flirt.costs`,
`cli.fsl.flirt.reproduce`, `cli.fsl.flirt.schedule` and `cli.fsl.applyxfm4D` move (the perturbations'
twelve parameters, `leastsq`'s sums, the spline): re-record once with topup's 6, FAST's 4 and
pairreg's 2 (sub-task 6); no triage entry added (sub-task 6's FLIRT trial triages).

## standard_space_roi, fslroi and FSL's standard images (relay 5: sub-task 4 part a, items B2, B3 and B4 for the two programs; commits on the branch, see NOTES.md)

The clean-room originals of FLIRT's `standard_space_roi` script and of `fslroi`
(`crates/larmorx-fsl/src/cli/ssroi.rs`, `cli/fslroi.rs`), written from `specs/standard_space_roi.md`
and black-box runs of FSL 6.0.7.23 through the oracle cache; FSL's standard images found as
`specs/bet.md` §5.4 says (`crates/larmorx-fsl/src/data.rs`); `original: true` for both in
`crates/larmorx-cli/src/replica.rs`; `lx.fsl.standard_space_roi` and `lx.fsl.fslroi` run the original
with `implementation="original"` and with `"auto"` when larmorx-nc is not found. FSL-211 to FSL-215
and fslroi's FSL-217 in both modes, by the rules the spec gives (the default: names as given, only
the run's files removed, `maskthr` ignored, the first failure stops the script, fslroi's errors exit 1,
int8 kept; `reproduce`: FSL's, with bash's splitting and `[`'s messages and every program's own
failures).

**The record.** Not re-made (relay 6 makes `fsl-bet` after BET's options). A development run of the
original's BET suite on every case of the two programs (`ssroi/`, `fslroi/` and the six `defects/`
cases of FSL-211 to FSL-214; standard tier): 57 cases, 45 pass, 12 refused by both, 0 fail; every
`standard_space_roi` case FSL's bit for bit, the six real heads included (the original's FLIRT gives
FSL's matrices on every one). `REPLICA_ONLY_TOOLS` is empty; the suite holds the original's
`standard_space_roi` to `specs/standard_space_roi.md` §8 (bit for bit where the matrix is FSL's,
else the matrices within FLIRT's 12-dof threshold, the masks' Dice, the output's box within a voxel,
and a stage-by-stage run with FSL's matrix given to both: `_compare_ssroi`, `_ssroi_stage`), and runs
the replica's pending `ssroi/phantom-none-convert` as a comparison for the original (it passes).

**FSL's standard images.** The search: the named directory alone (`fsl_data=`, `LARMORX_FSL_DATA`);
larmorx-nc's copy (its program from `LARMORX_NC_BIN`, the CLI's replica directories or `PATH`; each
file checked against the spec's SHA-256); `$FSLDIR/data/standard`; larmorx's data cache, filled once
by a download of FSL's `fsl-data_standard-2208.0-0.tar.bz2` (checked by size and SHA-256, only the
three images kept, each checked, with a `MANIFEST.tsv`). The transport is the platform's `curl`
(else `wget`) as a child process: larmorx keeps pure-Rust dependencies and links no TLS library; the
checksums make the files trustworthy. Tested without the network: Rust unit tests build a package of
their own (a bzip2 tar with a pax header) and serve it as a local file; a changed byte, another size
and a wrong member are refused; the order of the places; the refusal's message. The real download was
made once on the main machine (17 s, 13.6 MB of memory, the three files FSL's byte for byte), never in
CI (the golden case and the Python test that reach the search name an empty directory or a missing
local package).

**For the orchestrator to decide (a proposal, not in HANDOFF.toml):** the decision log of 2026-11-04
asks for "the place and the command that fetches ahead" of every fetched data set, and names neither;
this relay chose, for FSL's standard images: the cache `LARMORX_DATA_DIR`, else the platform's user
cache directory with `larmorx` in it (`$XDG_CACHE_HOME` or `~/.cache` on Linux, `~/Library/Caches` on
macOS, `%LOCALAPPDATA%\larmorx\cache` on Windows), FSL's under `fsl/fsl-data_standard-2208.0-0/`; the
command `larmorx fetch fsl-standard [<package file or URL>]` (a top-level command, not a tool: no
golden case needed; it prints the directory), and `LARMORX_FSL_DATA_PACKAGE` for a local copy of the
package. If the user prefers other names, they are in `larmorx_fsl::data` (`CACHE_ENV`,
`PACKAGE_ENV`) and `larmorx_cli::fetch`. A decision-log entry would record them for the later data
sets (FreeSurfer's atlases, TemplateFlow).

**Found on the way (the FLIRT original):** FSL's FLIRT writes `-out` with the display range 0 and 0
whatever the reference's (registration, `-applyxfm`, `-applyisoxfm`, every interpolation); the
original kept the reference's (MNI152's 3000 and 8000). Corrected in `flirt::engine`'s two `-out`
writers, with FLIRT's suite case `out/display-range` (passes). The `fsl-flirt` record (relay 4's)
does not move: no case of it has a reference with a display range (its header comparison would have
failed). No golden case moves for it (the FSL golden cases checked: only the 17 known and the 4 new).

**fslroi's matrices** (`docs/findings/fsl-bet-original.md`): FSL's held-frame arithmetic, fitted on
every row of both matrices of two neurologically stored real images (`fit_translate.py`); the
direct sum in double was a unit in the last place away on one of them. A question for a spec writer
(§3.2 says only "translated so that every kept voxel keeps its world position"): state it.

**Spec questions** (for a spec writer): (1) §3.2's matrices' arithmetic, as above; (2) §2.1/FSL-211:
the forms of the script's `[` tests (inferred from the messages as `[ X$2 = X ]` at line 112 and `[
_$1 != _ ]` at line 133; whether other unquoted tests exist, e.g. on option values with blanks); (3)
§2.5: fslpy's `imcp` message for a source that does not exist (not observed; the original prints
`No image files match: <name>` on standard output, exit 1); (4) `specs/flirt.md` §9.2: `-out`'s
display range 0 and 0 (observed).

The shared documents (HANDOFF.toml, operations 37 to 51): next-steps' follow-up "For the clean-room
catch-up" (the two programs and the data done; what stays for relay 6) and the `imcp` item; the
register's FSL-211 to FSL-215 rows (the clean-room original, gap `–`) and FSL-217's (fslroi's done,
`bet -F`/`-Z` still to do, gap kept `original`); the API register's FSL-211, FSL-215, FSL-217 rows;
`validation/tools.toml`'s `fsl-bet` tool (`how` and its last clause) and the `fsl-flirt` tool's
`standard_space_roi` phrase (after operation 34's block); `docs/licensing.md`'s sentence on the
original's search. After applying: `python -m larmorx_validation tables`.

Not in the handoff, for later relays: golden `cli.fsl.fslroi`, `cli.fsl.standard_space_roi`,
`py.fsl.fslroi`, `py.fsl.standard_space_roi` are new (no checksums yet: recorded once in sub-task 6
with the 17 that move; each runs in under 0.2 s); `docs/validation/golden.md` then names them. No
triage entry (sub-task 6's BET trial covers the two programs now that the original's descriptor
draws them as comparisons). `docs/api/fsl-bet.md` still says the options' programs are the
replica's: relay 6 rewrites it with the options.

## bet -S, -B, -A, -A2, FSL-217 inside bet, and the fsl-bet record (relay 6: sub-task 4 part b, items B1, B4's rest, B5's rest, B6; commits b774406, 6907f6c, 1a731a8, 7c303d3)

`bet -S`, `-B`, `-A`, `-A2` in the clean-room original (`crates/larmorx-fsl/src/cli/bet_script.rs`,
written from `specs/bet.md` §5.3, §5.4, §7, §9 to §11.1): each the script's sequence of the
original's own programs run in one process with the arguments the spec names (bet2,
`standard_space_roi`, FLIRT, `fslmaths`, `fslstats`, FAST with its streams discarded but larmorx's
lines, betsurf, the bet script itself for `-B`'s robust run); FSL's standard images found before
anything runs (relay 5's search); bash's `Aborted` report (line 399, the command as the script writes
it) and the `ERR` trap's line where a program fails; FSL-216's eye threshold in dc's exact decimal
arithmetic; FSL-63 for `-B -s`; FSL-46's cleanup by the files the run's programs created or changed;
FSL-217 inside `-F` and `-Z` (fslroi's types; an int8 input as uint8 bytes in `reproduce`). Built for
speed: the original's programs, the temporary images uncompressed when they are not kept; the steps
one after the other (two at once would hold both programs' images: FSL's peak is its largest
program's, and the memory rule holds larmorx to it). `lx.fsl.bet` runs the four variations in the
original (`implementation="original"`, `"auto"` without larmorx-nc; `tanh="correctly-rounded"`).
`REPLICA_ONLY_VARIATIONS`, `ORIGINAL_PENDING_CASES` (`parity/fsl_bet.py`) and
`REPLICA_ONLY_BET_CASES` (`tests/parity/test_memory_peak.py`) are empty; `original: true` was already
set for `bet` (and relay 5 set it for the two programs).

**The record** (`fsl-bet`, from 6907f6c; the run before it, from b774406, was stamped `-dirty`
because documents were edited during it, and gave the same 252 results, `compare_records.py`: 0
differences): 252 cases, 198 pass, 46 rejected by both, 6 expected divergences, 2 failures. Against
main's record (2026-10-31, 168 cases): the 164 cases kept are unchanged in status and details; the
4 pending cases of `-S`/`-B`/`-A`/`-A2` are gone (the BET options part replaced them with real
cases); 88 new: relay 5's 57 (`ssroi/`, `fslroi/`, the six `defects/` cases of FSL-211 to FSL-214:
45 pass, 12 rejected by both) and relay 6's 31 (the 23 option cases of the BET part's merge
verification, the six real heads among them, plus `bet/surfaces-t2-no-value`, the three FSL-217
cases of `ORIGINAL_PENDING_CASES` with `bet/fmri-int8-positive`, and the new
`defects/eyes-negative-mean`): 26 pass, 4 rejected by both (`bet/eyes-no-brain` and
`bet/surfaces-t2-missing`, the two that failed at the merge verification because the original
refused before running: now FSL's messages, files and status, FSL failing midway;
`bet/surfaces-t2-no-value`; `defects/eyes-nan`), 1 failure. `-S` and `-B` are FSL's bit for bit on
every case but one (every output and every `-d` file); `-A` and `-A2` have bet2's brain and mesh and
FLIRT's matrices FSL's and betsurf's masks within betsurf's thresholds (as its own record). The two
failures: `real/philips-surf-t2` (ds002603's scalp, the record's earlier failure, sub-task 5) and
`real/child-3y-B` (a 3.5-year-old's `bet -B`: every `-d` file FSL's, the robust run's second bet2
pass with `-c` on a neurologically stored image differs: Dice 0.9983, boundary distance p95 1.0,
max 2.24 voxels against bet2's 2; the open spec question of random-testing cases 622 and 677).
The suite holds the options to §11.1 (`_compare_options`: bit for bit, else stage by stage from both
programs' `-d` runs: when the first differing temporary file is a FLIRT matrix, FAST's bias field or
an image FLIRT resampled, §11.1's thresholds; otherwise the step that made it fails).

**Memory** (`tests/parity/test_memory_peak.py`, the original's ten BET calls pass): `-S -m` 62 MiB
against FSL's 67, `-B` 62 against 115, `-A2` 58 against 72; `bet -R` 93 against 103 (the robust run's
centre is now the original's `fslstats -l 0.001 -C`, whose centre computes each weight as it sums
it: it held a double copy of the volume, 130 MiB, over the allowance). **Speed**: on the main machine (FSL's single-threaded time from the oracle cache's record, the replica on one thread, the original on 1, 4 and 28): `-S -m` on ds000005's adult T1w 27.6, 20.8, 15.6, 5.7, 3.4 s; `-B -m` 85.8, 57.5, 29.2, 10.3, 6.2 s; `-A` 19.0, 18.6, 13.5, 4.9, 3.0 s; `-A2 -m` on ds000256's child 108.3, 91.6, 79.1, 23.4, 11.0 s (1.4 to 2.9 times FSL's speed on one thread, 6 to 14 times on 28)

**Found on the way** (black-box runs, `docs/findings/fsl-bet-original.md`, "bet -S, -B, -A, -A2"):
FSL 6.0.7.23's `fslstats -M`/`-S` print six decimals, so FSL-216's exponent kind never reaches dc; a
negative mean prints one `dc: stack empty` and uses the magnitude (reproduced bit for bit); FSL's
script leaves its temporary files when a step fails; `-A2` with a missing T2 goes on past FLIRT
(FSL-68) to betsurf's abort; bet2's zero minimum keeps the first voxel's sign (`min 0`; the
original printed `min -0`: corrected); `standard_space_roi -d` ends with status 0 in `reproduce`
and fslpy's `imcp` of a missing source prints `Could not find a supported file with prefix
"<name>"` (relay 5's spec question (3), answered by observation: corrected in the original).

**Spec questions** (for a spec writer): (1) §5.3 step 9: the dc expression's exact words, for
`reproduce` of `nan`, `inf` and an exponent (refused in both modes now; unreachable with FSL
6.0.7.23's fslstats and FLIRT); and the spec's "a mean of 1,000,000 or more prints `2.1e+06`" (FSL
6.0.7.23 prints `1951833.117037`); (2) §5.3's "a NaN voxel reaches FLIRT inside step 2 first (FSL-14
of FLIRT: the default refuses there)" against `specs/flirt.md`'s FSL-14 (non-finite values left out
of the costs by default): the original follows flirt.md, so its default goes on and refuses at the
threshold (FSL-216's rule B; the suite's `ORIGINAL_FIX_REFUSES`); (3) the bash reports' command
texts of the steps whose abort was not observed (written in the observed form); (4) the existing
question of `-c` on a neurologically stored image (cases 622, 677; now `real/child-3y-B` too).

**For FLIRT's next part** (not done here, it would move FLIRT's record): the original's FLIRT leaves
NaN and infinite voxels out of its costs by default without naming FSL-14 (it names it for the
centres of gravity only).

The shared documents (HANDOFF.toml, the operations after the 51 already there): next-steps' catch-up
follow-up (the options done, with the record, memory, speed and what is left); the register's FSL-216
and FSL-217 rows (the clean-room original, gap `–`; FSL-216's observation of six decimals);
`docs/api/upstream-defects.md`'s FSL-216 and FSL-217 rows; `validation/tools.toml`'s `fsl-bet` tool
(`how`, the validation's last clause) and its `fsl-bet` record block (the second failure; nothing
pending). After applying: `python -m larmorx_validation tables`.

Not in the handoff, for later relays: golden cases of `bet -S`, `-B`, `-A`, `-A2` (sub-task 6: the
inputs generated with stand-in standard images, as `tests/python/test_fsl_bet.py` makes them; `-B`
needs a larger head than the Python test's); the random-testing trial of BET's original now draws the
four variations as comparisons (sub-task 6); `-Z -d` of a float64 input writes its temporary images
float32 (FSL float64; bet2 reads them in single precision either way).

## BET's open differences, `lx.mri.hmc`'s centres and the `libm` crate's last uses (relay 7: sub-task 5, items H1, H2, H3; commits 0842fbd, 2b3db9e, 22d397e, 0b3f17c, 679dc6b)

**H1, BET's random-testing differences** (`oracles/difftest/2026-11-04-bet-options-6b`; the "190" of
next-steps and the brief is the BET part's count, the trial kept 176 differing cases of the original:
164 betsurf, 11 `bet`, 1 `bet2`). Judged again as black-box cases (`replay_trial.py`: each finding's
recipe and shrunk reproducer, FSL from the oracle cache, each case in its own 3 GiB leaf of the
memory box): 172 still differed with relay 6's build. betsurf's share of profiles set aside
(`betsurf_shares.py`, `-v` added) was FSL's on only 74 of 164 images, 82 of the 90 others `--t1only`.
Single profiles probed one at a time (`profile_oracle_cached.py`, FSL through the cache; the
original's profiles from a temporary dump build, never committed; 5,290 profiles of 11 images) found
four readings of `specs/bet.md` §6.4 that were the original's mistakes, now corrected (rule A, none
a defect of FSL): (1) the backward searches never read a profile's first sample; (2) the T1-only scan
for `b` starts at the second sample when `a` is the first; (3) step 5's first bright sample must lie
strictly within 10 mm of the inner skull; (4) step 5's searches each start after the previous one's
sample, and an outer-skull search range with no sample (step 7's `a` none) counts the profile's
scalp only, FSL running on: **FSL-50's stop was never seen** (43 images of the two trials), so the
original no longer stops in `reproduce` (the register's FSL-50 row and `docs/api/upstream-defects.md`'s
in HANDOFF.toml). Also `bet -Z -d` of a float64 input keeps the doubles in its temporary images (case
330; FSL's float64). After: shares FSL's on 162 of 164 images; 25 of the 176 cases agree; five
permanent cases (`original-betsurf-t1only-first-sample`, `-gap-strictly-within-10mm`,
`-flat-region-gaps`, `-empty-range-scalp-only`, `original-bet-Z-d-float64-tmp`). What still differs,
explained with numbers in `docs/findings/fsl-bet-original.md` and triaged "open" (52 entries appended
to `triage.toml`, one per signature group: 7 fixed, 45 open): betsurf's surfaces after the profiles
(inner skull median 0.047 mm, outer 0.82, scalp 3.6; the scalp's deformation, §6.5, on four images
whose skulls are FSL's exactly 0.24 to 3.4 mm; the masks of these 10- to 32-voxel heads miss Dice
0.99 by a voxel or two), `-p` on a mesh collapsed to a point (8: FSL writes 782 points, one midpoint
per edge 1,026), and `bet -R`/`-c` on neurologically stored images (7: cases 622 and 677's question).
**ds002603** (`real/philips-surf-t2`): its inner skull is now FSL's to 0.026 mm (95th percentile; was
0.95, up to 11 mm: part 2b's "few profiles vote" region), the outer skull 0.20 mm (0.56); the scalp
2.86 mm (3.18), the mask's Dice 0.9935 and boundary p95 2.0 voxels: still the record's failure, the
scalp's deformation.

**For the orchestrator:** the BET options part's earlier triage entry for the original's 190
differences (status open; not read by this clean-room relay) precedes relay 7's entries in
`triage.toml` and wins the site's ties of equal signatures: retire it or point it at relay 7's.

**A regression of the branch, corrected** (commit 2b3db9e): the part 2b permanent cases
`original-bet-R-centre-zero` and `original-bet-R-negative-image-bet2-crash` agreed on main and
differed with relay 6's build: `bet -R`'s centre, now `fslstats -C`'s text, is `nx−1 0 0` for an empty
brain of a neurologically stored image, and FSL's bet2 takes a centre of zeros *in its radiological
frame* (after `nx − 1 − x`) as not given (black box: `c-of-g` the centre of gravity). The original now
tests the zeros after the conversion (`cli::bet2::centre_mm`); every BET and head-motion permanent
case agrees (the nine `original-*` and the replica's 17, the two `mri-hmc` ones). For the spec writer:
§4.1 to say so (in HANDOFF.toml's next-steps op).

**The fsl-bet record** (from 2b3db9e; an earlier one from 0842fbd gave the same 252 results, 0
details apart): 252 cases, 198 pass, 46 rejected by both, 6 expected, 2 fail,
as relay 6's (`compare_records.py`: 0 statuses differ, 8 cases' details): betsurf's surface metrics of
the -A, -A2 and real surface cases and `defects/t2-other-grid` (the profile rules: e.g. the 82-year-
old's scalp 0.55 → 0.30 mm, the adult's 0.66 → 1.36 mm; the 75-year-old's `-A` scalp mask Dice
0.99867 → 0.99927), ds002603 as above. FSL from the oracle cache (329 runs served, 14 run).

**H2, `lx.mri.hmc`'s centres of mass**: both centres (each test volume's, the `.par` file's
reference) are FLIRT's blocked sums (`specs/mcflirt.md` §6.1, `specs/flirt.md` §8.1 step 3), and a
centre whose total is below 1e-5 prints mcflirt's `WARNING::in calculating COG, total = 0.0` as
often as mcflirt (black box, `cog_warning_probe.py`: 8 of 8 calls alike: once per registration of
such a test volume, once per `.par` line of such a reference). The `mri-hmc` record (from 0842fbd, `--jobs 1`): 169 cases, 145 pass, 20 rejected by both, 3 expected divergences, 1 failure (`real/ds003345/default`, as main's); against main's record (`compare_records.py`) no status and no detail differs. Below the record's printed precision, `centre_moves.py` ran fMRIPrep's call on four real runs (ds000005, ds000117, ds005040, ds006010) and `-cost normmi`, `-meanvol` and `-dof 12` on ds000005 with main's build, the blocks alone and the final build (`reproduce`, the `.par` file added): the 1,344 matrices and `.par` lines are the same text in all three. The golden cases `py.mri.hmc` and `py.mri.hmc.options` move with `atan2`, not with the blocks (each build's golden run).

**H3, the `libm` crate's last uses** (`docs/findings/platform-math.md`; measured first by
`validation/investigations/libm-last-uses`): `acos` (AFNI's oblique angle; `Affine::obliquity`),
`atan2` (the hmc's decomposition) and `pow` (the B-spline prefilter) are `larmorx_core::math`'s now;
AFNI's angle is glibc's on every float32 cosine with either function (no record moves); the pow is
glibc's, so ANTs', on all 91 pairs that reach it (the libm crate's differed on 6): the B-spline cases
of `ants-apply-transforms` (54) and `ants-resample-image` (8) re-run, unchanged; `atan2` moves the
hmc's last bits (in the record above, with the centres). The `libm` dependency left `larmorx-afni`,
`-core` and `-mri` (`larmorx-interp`'s is a dev-dependency); the three `Cargo.lock` refreshed. Left:
mcflirt's arctangent is glibc's `atan2f` (single precision), for the hmc's `reproduce` later.

**Golden** (sub-task 6, one `record_golden.py` run): relay 7 moves `cli.fsl.betsurf.t1only`,
`py.mri.hmc` and `py.mri.hmc.options` (24 failing in all with the 21 known).

**Spec questions** (for a spec writer; in HANDOFF.toml's next-steps op): §6.4's four readings to
state; case 192 (a constant int16 image: four profiles whose outer searches both find nothing in a
two-sample region at the profile's end, kept by FSL, set aside by `OS2 ≥ IS`); step 3's "within
10 mm" strict or not (no case met it); FSL-50 (when, if ever, FSL stops); §6.5's scalp phase (the
four clean images); `-p` on a collapsed mesh.

The shared documents (HANDOFF.toml, ops 60 to 67 after the 59 already there): `docs/next-steps.md`
(the BET part's follow-up: its trial judged; relay 6's "left" item of `-Z -d`'s float64 done; BET's
open spec questions (2) to (4) brought up to date, with bet2's zero centre; the `libm` crate's last
uses done; the B-spline prefilter's `pow` done; `lx.mri.hmc`'s centres of mass done), the register's
FSL-50 row (`docs/findings/upstream-defects.md`) and `docs/api/upstream-defects.md`'s FSL-50 row. No
`validation/tools.toml` change: the two records keep their counts (the generated tables read them).
`validation/difftest/triage.toml`: relay 7's 52 entries appended in the branch.

## Random testing, golden, gates (relay 8: sub-task 6; commits c798a29, 5d35983, d11b2ae, 5a44c8f, 6b0eb8e, 0b11e92, e0138ce, 4a8f0f8, 0483e0c, 2a978ad, 9209800)

**The trials** (each boxed, 30 minutes, one thread a case, the oracle cache in `use` mode,
`oracles/difftest/2026-11-05-catchup-<tool>`; seeds the earlier trials', so their cases are drawn
again and FSL's runs served): topup 17,535 cases (13,300 agree, 4,223 differ, 12 notices; of the
2026-11-03 trial's 6,999 differences 3,695 agree now); FAST 19,695 (19,645 agree, 50 differ); FLIRT
13,907 (12,334 agree, 1,560 differ, 13 notices; of the 2026-11-02 trial's 1,140, 696 agree now);
BET 909 (the original 792 agree, 114 differ; the replica 896 agree, 13 differ); the head-motion
correction 83,244 cases (31,552 agree, 51,681 differ, 11 notices: registrations on small phantoms ending elsewhere, as well aligned as mcflirt's on 400 sampled, median residual ratio 1.000; 6,363 of them the oracle cache's missing empty directory, below). Every difference is in `validation/difftest/triage.toml` (27 entries appended at
its end: topup 10, FAST 4, FLIRT 6, BET 5, the head-motion correction 2), with numbers, reproducers
and the probes behind them (`validation/investigations/cleanroom-topup/trial_*.py`,
`cleanroom-flirt/catchup8_int_nn.py`).

**Relay 1's open item** (topup findings 6 and 27 differing on `--replay` only): `difftest --replay`
runs a finding's *shrunk* recipe, relay 1's direct runs took its *drawn* one; on the same recipe
both agree. Shrunk 27 (one voxel) was the suite's mistake (fixed, with a test); shrunk 6 (1 x 1 x 3
voxels, the default 8 mm smoothing) is the original's smoothing on an axis shorter than its kernel,
3 % (open, for a spec writer).

**Fixed** (each with a permanent case): topup's scaled conjugate gradients count every iteration
toward `--miter`, the rejected ones too (observed with `-v`), and **FSL-242** (new, rule A, class c):
a level whose last iteration is rejected ends at that rejected, worse point in FSL; the original
ends there with `reproduce`, at the lowest cost by default, named where it happens; FLIRT's search
lists a fallback minimum only on its `Cost minima` line; FLIRT's `-out` keeps the negative zeros of
a sform completed from a qform-only reference; applyxfm4D writes an int32 image's padding exactly in
its type; the topup suite's one-voxel grids and NIfTI-2 quaternions (harness). The records re-made
from 5a44c8f: `fsl-topup` 185 cases (104 pass, 67 both, 14 expected, 0 fail) and `fsl-flirt` 337
(233, 89, 10, 5): against the branch's, no status and no detail moves, only the new permanent
cases (and FLIRT's `out/display-range`); the `fsl-bet` suite run again from it: 252 cases, every
status and detail the same (not committed: nothing moved).

**Golden** (recorded from the clean commit with `record_golden.py`, the parity smoke tier passing (2,786
tests): 236 cases, 1,726 outputs (228 and 1,623 before); 8 new: `cli.fsl.bet.cleanup` (bet `-S -m -s
-v` and `-B -m`), `cli.fsl.bet.surfaces` (`-A`, `-A2`), both on stand-in standard images that the
case writes from `anat2` into the directory `LARMORX_FSL_DATA` names (never FSL's data),
`cli.mri.hmc.cog` (a volume of zeros: mcflirt's COG warning counted, 3), `cli.fetch` (a file that is
not FSL's package refused by its size, nothing kept; the usage), and relay 5's `cli.fsl.fslroi`,
`cli.fsl.standard_space_roi`, `py.fsl.fslroi`, `py.fsl.standard_space_roi`; 20 re-recorded on
purpose, exactly the catch-up's named ones (topup 6, FAST 4, pairreg 2, FLIRT 5,
`cli.fsl.betsurf.t1only`, `py.mri.hmc`, `py.mri.hmc.options`), every other checksum unchanged. The
gates' CI-like run (without `FSLDIR` and `LARMORX_NC_BIN`) found `cli.fsl.standard_space_roi`'s
usage naming `$FSLDIR`'s standard image: the case now unsets both, and the checksums were recorded
once more from that commit (from 0483e0c, the smoke tier passing again, 2,786 tests: only that case's `usage:stdout` moved; committed as 2a978ad). The cases took 12.9 s on this machine (quiet); BET's four
variations about 5.7 s of one-thread CPU of it. `docs/validation/golden.md`'s family rows ("Command
line, AFNI, MRI and FSL" 41, `lx.fsl` 17) were behind the registry (66 and 25 before this part): the
ops set 72 and 27.)

**The shared documents** (HANDOFF.toml ops 67 to 76): the register's FSL-242 row
(`docs/findings/upstream-defects.md`, after FSL-241's; gap "replica": the topup replica's default is
its author's to check) and `docs/api/upstream-defects.md`'s; `validation/tools.toml`'s topup "how"
(FSL-242; the counts come from the records); `docs/next-steps.md`'s follow-ups of the trials (what is
left per tool, four spec questions); `docs/validation/golden.md`'s counts and the recording.

**For the orchestrator:**
- The BET replica's 13 trial differences are every one where FSL's FLIRT crashes inside
  standard_space_roi (FSL-75's corrupted heap; never reproduced): for its author (not triaged here,
  replica entries).
- The head-motion correction's first trial run overlapped two rebuilds of the extension
  (`install_core.sh` copies over a loaded `_core.abi3.so`; the trial's process ended with a
  segmentation fault after writing its report), so it was run again with the final build: 83,244 cases, exit status 0; the first run's numbers (48,582 cases) are kept in `oracles/difftest/2026-11-05-catchup-hmc-overlapped-rebuild` and not used.
- CI's golden time: the run of 2026-11-04 (41922352582) took 42 to 110 s for its "Test wheel (Python
  3.12)" step (golden and Python tests; macOS x64 the 110 s); this part adds about 6 s of one-thread
  CPU on this machine (BET's four variations 5.7 s, the COG and fetch cases 0.03 s).
  `docs/validation/golden.md`'s "Time budget" is from 2026-10-11 (81 tests, 0.8 s): for the
  orchestrator to re-measure.
- The oracle-output cache keeps a run's files, not the empty directories it makes: mcflirt's `.mat`
  directory for `-rmsrel`/`-rmsabs` without `-mats` (FSL-13) is missing from every replay, which made
  6,363 "files written" differences in the hmc trial drawn again from the cache (the first run, live,
  had none). The hmc descriptor now runs such mcflirt calls live (`makes_empty_directory`, with a
  test); whether the cache should record and restore empty directories (`oracle_cache/core.py`'s
  `_snapshot` sees files only) is the orchestrator's.
- The head-motion correction's default (FSL-01, -03 and the other corrections on) is aligned worse
  than mcflirt on random small phantoms more often than `reproduce` (above 1.05 times mcflirt's
  residual in 102 of 400 sampled findings, `reproduce` in 18): for its next part.
- `tables --check` passes with the handoff applied in a scratch copy (checked; the worktree untouched).

**Gates at the final commit (9209800):** the Rust gates of the three workspaces (fmt, clippy
`-D warnings`, tests) at 0b11e92, every later commit Python, documents and golden only;
`gates-python.sh --no-build`: ruff, format, SPDX and the collection without `larmorx_validation`
pass; `pytest tests/python tests/golden validation/tests` 810 passed, 1 failed (`test_tables`, until
the handoff's `tools.toml` blocks are applied and the tables regenerated: with the handoff applied
in a scratch copy `tables --check` and `test_tables` pass); CI's environment (without `FSLDIR` and
`LARMORX_NC_BIN`) 645 passed. `apply_handoff.py --check`: 77 operations apply (the 67 before and 10 of
relay 8).

## The site, part 1: topup, FAST and FLIRT (relay 9: sub-task 7, split; larmorx commits ca06f7d, 32d1dac, 81ef967; site branch `catchup-pages`)

Relay 9 did topup's, FAST's and FLIRT's pages; relay 10 does BET's (the options' clean-room panels)
and the head-motion correction's, then sub-task 8. The site clone is `<scratch>/site`, branch
`catchup-pages` from the site's `main` 54eb60f, `evidence-blobs` from a1dfe8b (fetch both from it;
the blobs commit's manifest SHA-256 is in its message). Relay 9's commits there: `catchup-pages`
4362f6b (the three pages, their JSON, the manifest); `evidence-blobs` 95f020a (1,092 blobs, 302.5 MB,
the manifest's SHA-256 2a9e375aa772a9e9).

**What changed on the three pages** (exported from larmorx `81ef967`, clean; the exporter's tree in
each JSON's provenance):
- The clean-room original's runs of every worked example made again (`refresh_original` in
  `evidence/fsl_topup.py`, `fsl_fast.py`, `fsl_flirt.py`, with the reason recorded in each cache
  entry's `rerun`; FSL's and the replicas' runs kept), and the cases' classification
  (`cases-original`) of topup and FAST computed again. The original's cache entries as they were
  before are kept in `<scratch>/r9/cache-backup` (hard links).
- The case explorers from the re-made records (fsl-topup 185, fsl-fast 102, fsl-flirt 337).
- Each page a new table pair: the catch-up's random-testing trial and its differences, each in one
  category with its verdict (a column of its own) and a reproducer
  (`validation/investigations/cleanroom-topup/trial_catchup_triage.json`, from relay 8's
  `trial_categories.py`; `cleanroom-fast/trial_catchup_categories.py` and
  `cleanroom-flirt/trial_catchup_categories.py`, new, with their JSON outputs). The fixed classes'
  findings replayed with the final build (`cleanroom-flirt/trial_catchup_replay_fixed.py`): every
  one agrees but two FLIRT header findings whose messages still differ (counted with the messages).
- The original's texts where its reproduce mode came to follow FSL (FSL-201's forward differences,
  FSL-145's exact counts and error, FSL-133's index, FSL-134's other shape; FAST's FSL-202 and
  FSL-203; FLIRT's FSL-204), FSL-241 and FSL-242; FAST's examples gain the row *clean-room original,
  default − replica, default* (the record holds the one to the other): within FSL's own last-bit
  band on five examples, the same bits on the priors, just beyond the band on the three channels
  (976 partial-volume values against FSL's 973: open).
- A fix of FAST's evidence: the original's own cases (`original/`) were classified with the replica
  suite's preparation, which leaves their `%name` images out (FSL aborted, `original/nan-no-negative`
  and `nan-offset` showed as open): prepared with the original suite's `_prepare` now, both
  bit-identical.
- Honest moves: FLIRT's fMRIPrep call on ds005040 is now 0.0113 mm from FSL's matrix (0.0065
  before; FSL's own 0.0086 mm): its verdict moved from *within the original's own variability* to
  *open*, as relay 4's findings record. topup's movement example: the original's reproduce mode
  (FSL's forward differences) ends 4.9e-4 mm and 1.3e-4 rad from FSL, its default 6.4e-6 mm and
  3.3e-7 rad from the replica's default (the page says so).
- Prose: each page's lede, the implementations' dates, a "clean-room catch-up" paragraph (topup,
  FAST), the verdict counts, the random-testing section, the corrections and the known limits.

**The site's checks** (relay 9): `tools/build.py` 0 problems; the number check (`LX_EVIDENCE.check`)
topup 1,028 of 1,028, FAST 2,826 of 2,826, FLIRT 350 of 350; the viewer check 182, 650 and 38 viewers, every one ok; the inventory
(`validation/investigations/cleanroom-glibc-dbl/site_inventory.py`): 25 pages and 23 evidence files,
0 counts fell; screenshots before and after at 1440 and 390, light and dark, full page
(`<scratch>/r9/shots-before`, `shots-after`); no machine path in the three JSONs or pages.

**For the orchestrator, the site's shared files:**
- `data/status.yml`: generated: after the larmorx ops (the tools.toml blocks of fsl-topup, fsl-fast,
  fsl-flirt, fsl-bet), `python -m larmorx_validation tables --site <site>`.
- `static/evidence/summary.json`: generated from the tools' JSON: `python -m larmorx_validation
  evidence summary --site <site>` after merging the pages (relay 9 did not commit it).
- `site.yml` (ops 78, 79: FAST's summary no longer says "bit for bit FSL's with correctly rounded
  maths"; FLIRT's says the catch-up), `content/roadmap.md` (ops 80 to 82: last updated, step 1's
  status, the Completed row of the whole part; relay 10 checks its BET and head-motion numbers when
  their pages are exported). `content/validation.md`: no change (its matrix comes from summary.json).
- Pre-existing, not this part's: the published JSON of ANTs' pages hold machine paths (the home
  directory in `ants-n4.json`'s commands; agent scratch paths in `ants-ai.json`, and in
  `ants-atropos.json`, `ants-denoise.json`, `ants-apply-transforms.json`): a re-export of those pages
  with `_short` applied, for the orchestrator.

## The BET trial's last findings, FLIRT's search, the site part 2 and the final handoff (relay 10: sub-tasks 7 part 2 and 8; larmorx commits a835ddd, e4253cc, 3bdec51, 73c3baf, 6bd4959, ed52250, 57ef90b, 7db850a; site `catchup-pages` dfff9f4, 846beef, 8f321ef; `evidence-blobs` 123c0e6)

**A mistake of FLIRT's original, found through BET's trial and fixed (a835ddd).** The 18 BET trial
findings without a triage entry were mostly standard_space_roi's FLIRT matrices 37 to 303 mm from
FSL's on small heads. FSL's own `flirt` measured both final matrices (`trial_ssroi_matrices.py`, a
`measurecost` schedule of our own): every cost 1 at every scale (no information, the identity's too),
so the search's starting points tied. FSL sorts the search's list by cost with libstdc++'s
`std::sort` (not stable) before appending it to `S` and `P`; the original kept the order found.
Observed with a schedule `search; print S; print P` on a pair whose 829 starting points all cost 1:
FSL's `S` and `P` are exactly the introsort's permutation of the printed list; with 20 rows in `S`
before the search those stay first. Fixed in `flirt/search.rs` (`schedule::std_sort` after the
`-verbose` print); unit test of the observed permutation; four permanent cases
(`fsl-bet/original-ssroi-search-equal-costs`, `original-bet-B-search-equal-costs`,
`fsl-flirt-original/search-equal-costs-sorted`, `pairreg-search-equal-costs-sorted`; the FLIRT ones
fail with the build before). Not an FSL defect (FSL's sort is a design: nothing to register).

**What it changed, measured.** FLIRT's catch-up trial: the 1,471 findings of its three path
classes replayed with the final build and with the build before the fix (`catchup_replay.py`): 873
agree now and differed before (755 registrations, 101 pairreg runs, 17 message counts). BET's trial:
every one of the original's 114 findings replayed (`replay_trial.py`): 20 agree (15 matrices of
standard_space_roi and `bet -B`, 5 output boxes). Records from a835ddd: `fsl-flirt` re-made and
committed (e4253cc): 339 cases, 238 pass, 89 both, 10 expected, 2 fail (3 fail to pass:
`real/weights-empty-refweight`, `-inweight`, `edge/one-voxel-input`; 2 cases' details:
`difftest/debug-neurological-levels`, `debug-noresample`; 2 new permanent cases); `fsl-bet` re-run:
252 cases, every status and detail as the branch's (not committed); `fsl-fast` re-run (its `-a`
priors use this FLIRT): 102, nothing moved. Golden: 238 cases pass with the fix (none moved, nothing
re-recorded).

**Triage** (`validation/difftest/triage.toml`, 9 entries appended, 466 in all, `tomllib` checks):
FLIRT's fix (fixed); BET: standard_space_roi's and `bet -B`'s matrices (fixed, two entries); the
output headers given FSL's matrix by kind (open, with numbers: 9 one float unit, 4 a form's
translation whole voxels where qform and sform differ, 2 NIfTI-2 outputs in single precision and a
sform of code 0 moved, 2 fslpy's conversion); 88 (ends at a lower cost than FSL's by FSL's measure),
114 (a higher one), 545. Every finding of the BET trial now shares a check with a clean-room entry.
New defects: none (FSL-243 to FSL-260 unused).

**The site, part 2** (`<scratch>/site`, branch `catchup-pages`):
- BET: the variations' seven worked examples (the replica authors' `fsl_bet_options`, used as a black
  box, not read) gain the clean-room original's two runs and four comparisons each
  (`evidence/fsl_bet_options_original.py`: its own cache entries `options-<case>-original`, the
  panels added before the replica's module finishes each example, every output's file checked
  against the replica's own panel). The original's reproduce mode gives FSL's values on `-S` (both
  heads), the 82-year-old's `-B` and standard_space_roi; on the 3.5-year-old's `-B` its brain
  differs in 2,439 values (the record's failure `real/child-3y-B`, the `-c` centre question); on
  `-A`/`-A2` betsurf's masks at Dice 0.9993 to 0.9996. Its default differs where it names
  corrections (FSL-09, FSL-182, FSL-202) and by the correctly rounded maths (GLIBC-07). The
  original's three earlier entries made again (`refresh_original`); the case explorer from the
  `fsl-bet` record; the trial's tables (summary of both implementations; the original's 114 in 12
  categories with verdicts). Texts: lede, facts, the original's record paragraph (198 of 200),
  the variations, standard_space_roi and fslroi in the original, its data search, known limits.
- Head-motion correction: the original's three runs made again (they dated from 2026-10-21); the
  catch-up trial's tables (83,244 cases; 49,383 registrations as well aligned on the 400 sampled,
  2,298 harness-only).
- FLIRT again: the original's runs made again after the fix; the case explorer from the re-made
  record; the trial table's new fixed class (873); texts (238 of 240, the fixed class).
- Checks: `tools/build.py --main-repo <worktree>` 0 problems; number check BET 1,051/1,051, hmc
  259/259, FLIRT 349/349; viewer check 272, 12 and 38 viewers ok; inventory against the site's main:
  0 counts fell (359 grew); against relay 9's state one count fell on FLIRT's page (`data-ev` 351 to
  350: two numbers of the original's agreement chart are gone, its *open* and *within the
  original's variability* segments of two categories now empty because their cases moved to
  bit-identical, and the triage table gained one: the cases improved, nothing was removed); screenshots before and after, 1440 and 390, light
  and dark, full page (`<scratch>/r10/shots-before`, `shots-after`; element shots in
  `shots-select`); no machine path in the JSONs or pages. Blobs: 1,133, 302.8 MB, the manifest's
  SHA-256 6ca9d3a050157ca6 (evidence-blobs 123c0e6).

**For the orchestrator:** push `evidence-blobs` (123c0e6) before `catchup-pages`' merge into the
site's `main`; then `python -m larmorx_validation evidence summary --site <site>` and `tables --site`
(status.yml), after the larmorx ops. Shared documents changed by this part (all in HANDOFF.toml, 84
operations, `apply_handoff.py --check --site <scratch>/site` passes; applied to a scratch copy,
`tables --check` and `test_tables` pass and the register counts 395 entries, 393 before: FSL-241
class d, FSL-242 class c): `docs/next-steps.md` (every tool's items closed, the follow-ups left and
the spec writer's questions gathered in one list, op 70), `docs/findings/upstream-defects.md` and
`docs/api/upstream-defects.md` (the rows of the defects this part touched: FSL-133, -134, -145, -201 to -204, -211 to
-217, -241, -242, -50), `validation/tools.toml` (fsl-topup, fsl-fast, fsl-flirt, fsl-bet blocks and texts),
`docs/licensing.md` (the original's data), `docs/validation/golden.md` (relay 8's recording), and in
the site `site.yml` (FAST, FLIRT, BET, hmc summaries) and `content/roadmap.md` (the Completed row:
FLIRT 238 of 240, BET 198 of 200, hmc 145 of 146). The decision-log entry for the data cache is the
orchestrator's (its note of 2026-11-05). Pre-existing, for the orchestrator: ANTs pages' JSON with
machine paths (relay 9's note); the oracle cache keeps no empty directories (relay 8's).
