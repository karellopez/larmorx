# HANDOFF: BET's -S/-B/-A/-A2 replica part

The exact changes are in `HANDOFF.toml` beside this file (`apply_handoff.py --check` passes);
this file gives their reasons. Relays append their sections.

## Relay 1: sub-task 1, the replicas' `reproduce` on glibc's double `sin`, `cos`, `sincos`

**What changed in the branch** (commits 36ccf0c..d834ab9 on main 4e1f7fc: 36ccf0c the wiring,
f840a80 and 8f6b2f5 two replica fixes, 9a6a77a and 95a34e2 golden, 4b198b0 the investigation
tools, fa92495 and d834ab9 the record):
every place the FSL and AFNI replicas' sources call glibc's double `sin`, `cos` or `sincos` (found by
reading FSL's sources in `crates-nc/fsl-source/` and, for AFNI, by `objdump` of the 26.3.05 oracle's
binaries: GCC merges each `sin`/`cos` pair into `sincos`) takes `larmorx_core::math`'s `sin_glibc`,
`cos_glibc` or `sincos_glibc` with `reproduce` and the correctly rounded functions by default:

| Site | Replica | Notice (the user's decision "exact cases only") | Evidence |
|---|---|---|---|
| fslmaths `-sin`, `-cos` (double, any `-dt`) | `larmorx-nc-avwutils` | GLIBC-07 named in both modes where a stored voxel differs | unit test; record `fn/double-sin`, `-cos` pass |
| Kiss FFT twiddle factors (`cexp` -> `sincos`) of fslmaths' FFT convolution | `larmorx-nc-core::kissfft` | none (a filter) | `check_fft.py --mode reproduce`: 0 of 65,792 factors, 0 of 197,376 transform values differ from Armadillo's; unit test: the modes' factors differ on exactly the 220 factors in 80 lengths the probe found; record `difftest/fft-twiddles-glibc-sincos` expected divergence -> pass |
| sinc kernels' Hanning and Blackman windows (FLIRT `-interp sinc`, applyxfm4D, mcflirt `-sinc_final`) | `larmorx-nc-core::kernel` | none (an interpolation) | unit test: the same float kernels in both modes at every width 1 to 1001 |
| avscale's half transforms (`sqrtaff`'s `cos`) | `larmorx-nc-flirt::avscale`, `larmorx-nc-core::miscmaths` | GLIBC-07 named in both modes where the two reports differ | unit test (sqrtaff's float rotation hides the last bit on every case tried) |
| FLIRT's stage-4 cost sinc table (`q_hanning`) | `larmorx-nc-core::costfns` | none | unit test: the 201 entries are the same with either `cos`, one table serves both modes |
| AFNI `csfft` tables (`fft64` ... `fft_5dec`, `csfft_trigconsts`) | `larmorx-afni-replica::csfft` (`CsfftPlan::with_mode`) | none | unit test: the same tables in both modes for every length `csfft_cox` computes |
| 3dTshift Fourier phase factors (`CEXPIT`) | `larmorx-afni-replica::shift` (`ShiftWork::with_mode`) | none | probe over all 2^32 floats: 0 give another float pair |

Not ported, so not wired (recorded for completeness): warpfns' `calc_sigloss` (`Sinc`, double
`sin`), the warp fields' float `cos`/`sin` and newimage's `real`/`imag` (float functions). topup's
`exact_movement_derivative` is larmorx's own correction (FSL-201), not FSL's code: it keeps the
correctly rounded functions.

**A replica regression found and fixed (f840a80):** the re-made record failed `stat/ztop`,
`stat/ztop-double`, `stat/ztop-ptoz` and `d13/ztop-inf`, and so does main's own build at 4e1f7fc:
`-ztop` of a positive NaN voxel gave `0xffc00000` where FSL gives `0x7fc00000`. cprob's `expx2` wrote
its products as plain Rust arithmetic, whose NaN sign the optimiser does not keep (it folded
`-(m * m)` into `(-m) * m` once the shared maths copies of 2026-11-04 changed the inlining); its
operations are now `sse`'s (x86-64's NaN rules explicit), with a unit test in debug and release. It
is a larmorx mistake (no register entry). It suggests that main's other replica records, last made
before the glibc maths copy (7958e0e), may hold more such latent NaN-sign changes: the orchestrator
may want each replica record re-run once (this relay re-ran fslmaths', FLIRT's, mcflirt's and
3dTshift's).

**Records.** `fsl-maths-replica` re-made from 8f6b2f5 (commit d834ab9): 422 cases, 360 pass, 61
both reject, 1 expected divergence (`fn/double-tan`), 0 fail; against main's record (a51517a): one
status change, `difftest/fft-twiddles-glibc-sincos` expected divergence -> pass, the new permanent
case `difftest/dilall-nan-order` a pass, and `fn/double-tan`'s reason (only `tan`, `asin`,
`acos`, `atan` listed). (A first re-make at 36ccf0c failed four `-ztop` cases: the expx2
regression below, fixed before this record.) **Records not re-made** (they did not move): the FLIRT,
mcflirt and 3dTshift replica suites were re-run at 5d80c86 (to scratch) and compared with main's
records by `compare_records.py --against`: `fsl-flirt-replica` 305 of 305 cases, status and
details identical; `fsl-mcflirt-replica` 237 of 237 identical; `afni-tshift-replica` 315 of 315
identical, plus the 2 cases main's suite gained after that record (`methods/glibc-sinf-wsinc5`,
`-wsinc9`, from the glibc maths part): both pass with the replica (the record lags its suite by
those two; re-making it is the orchestrator's choice). Note: the 3dTshift replica suite with 24
workers in a 24 GiB box lost a worker (BrokenProcessPool, memory); it ran with 12 workers in 40 GiB.

**Golden.** `larmorx-nc` (125 cases, 688 outputs): `avscale.allparams.reproduce`,
`flirt.applyxfm-sinc.reproduce` (the default's checksums: the float rotation and windows hide the
last bit), `fslmaths.filters-fft-twiddles` and `.reproduce` (a new generated input `{chk}`, a
30 x 1 x 3 checkerboard: a 60-point transform, the two modes differ in one voxel; a reproduce case
on `{t2}` gave the default's output and was replaced), `fslmaths.unary-double.reproduce` moved.
AFNI replicas (43 cases, 96 outputs): `3dTshift.fourier.int16.reproduce` (the default's output).
Recorded from 9a6a77a (95a34e2).

**Random testing.** The permanent case `fft-twiddles-glibc-sincos` expects a pass (its triage entry
stays `register`, GLIBC-07, its explanation extended); the fslmaths descriptor now draws `-sin` and
`-cos` in double precision (only `tan`, `asin`, `acos`, `atan` stay out there). A short trial
(2026-11-04, seed `bet-options-1-sincos`, 9 minutes, 24 jobs, 69,962 cases,
`oracles/difftest/2026-11-04-bet-options-1-fsl-maths`): one difference, a replica mistake fixed
(8f6b2f5): `-dilall` of an image with NaN, `inf` and `-inf` gave 42 voxels a NaN of the other sign
(FSL's build adds each neighbour first, the replica added the sum first; main's build too);
permanent case `dilall-nan-order`, triage entry `fixed`. Notices without a visible change as in the
trial of 2026-11-03 (GLIBC-07 in 77 cases: a double function's value that a later operation or a
float output removes, by design: the notice is about the operation's stored values).

**Shared documents** (HANDOFF.toml): the register's GLIBC-07 row (the replicas' part and the D13
gap: none for `sin`, `cos`, `sincos`), the API defects page's GLIBC-07 row, next-steps item 7's
step (b) marked done and its follow-up bullet removed; golden.md's case counts and its recording note.

**The site** (not touched by this relay; for sub-task 6's site relay or the orchestrator): the
fslmaths page's replica evidence still shows `difftest/fft-twiddles-glibc-sincos` as an expected
divergence (the record of 2026-11-03). A re-export of fslmaths' evidence from the new record
(`fsl-maths-replica`, d834ab9) makes it a bit-identical case and adds `dilall-nan-order`; the
evidence module's reason for the FFT case is removed and its trial text updated
(`validation/src/larmorx_validation/evidence/fsl_maths.py`).

## Relay 2: sub-task 2, `standard_space_roi` and `fslroi` (replicas)

Commits 37d16b4, c2487ec, 4db356d (branch worktree-agent-a70a1c0824b674bea).

- **What is built.** `larmorx-nc fsl standard_space_roi` (FSL 6.0.7.23's fsl-flirt 2111.4 script,
  `crates-nc/larmorx-nc-flirt/src/ssroi.rs`, every option, message, exit status and failure case;
  its programs in-process; bash's `[` on split words in `shtest.rs`), `larmorx-nc fsl fslroi`
  (fsl-avwutils `fslroi.cc` as a program; the core's region now takes all three argument forms,
  the time index's bounds and the allocation), fslpy's `imcp`/`immv` (`larmorx-nc-core/src/fslpy.rs`,
  Apache-2.0 code in the nc package, its notice in `crates-nc/NOTICE`), and the data lookup
  (`larmorx-nc-core/src/fsldata.rs`: `LARMORX_FSL_DATA`, `$FSLDIR/data`, larmorx-nc's copy at
  `<prefix>/share/larmorx-nc/fsl-data`, the refusal naming the image and every place looked).
  Registry: both `original: false` (`crates/larmorx-cli`), listed after fslstats; `lx.fsl.fslroi`
  and `lx.fsl.standard_space_roi` (`python/larmorx/fsl/regions.py`), replica-only functions in
  `tests/golden/registry.py`. FSL's script copied into `crates-nc/fsl-source/` (MANIFEST.tsv).
- **Evidence (not a record yet):** the BET replica suite's 55 new cases (`fslroi/`, `ssroi/`, six
  `defects/`), standard tier, against FSL 6.0.7.23 through the oracle cache: 42 identical, 12
  refused alike, 1 expected divergence (fslpy's nibabel conversion in `imcp`, pending), 0 fail.
  The record `fsl-bet-replica` is re-made in sub-task 6 and holds them (standard_space_roi's
  record is inside BET's).
- **A change to the FLIRT replica:** its `descrip` was `larmorx-nc fsl flirt`; FLIRT writes
  newimage's build string, as the bet2 and fslmaths replicas already did; it now writes FSL's.
  FLIRT's golden header checksums move (data unchanged): `record_golden.py` in sub-task 6.
  Suggested follow-up (not done): `applyxfm4D` and `mcflirt` still write `larmorx-nc fsl ...`.
- **Shared documents** (HANDOFF.toml, relay 2): the register's rows FSL-211 to FSL-215 after
  FSL-204 (the counts line: the orchestrator's recount). For README/validation index: through
  `validation/tools.toml` in sub-task 6 (fslroi and standard_space_roi rows). For
  `docs/licensing.md`: larmorx-nc now carries fslpy's `imcp`/`immv` rules too (Apache-2.0,
  notice in crates-nc/NOTICE). For `docs/next-steps.md` follow-ups: imcp's file-type conversion
  (fslpy with nibabel 5.4.2) is pending in the replica (refused clearly); the bet replica's
  in-process `fslroi` uses the core's `ToolFailure` status 1 after an exception where FSL's
  fslroi exits 0 (unreachable in bet: its regions are always valid).

## Relay 3: sub-task 3, `bet -S`, `-B`, `-A`, `-A2` (replica)

Commits 3cbbacf, d8d2ef6, d244efc, 403e9cf, 99c060f, dce7494, c30ba30 (branch worktree-agent-a70a1c0824b674bea).

- **What is built.** The BET replica's `bet` runs every variation of FSL 6.0.7.23's script: `-S`
  (SIENA's eye cleanup), `-B` (bias field and neck cleanup, then the script again with `-R`), `-A`
  and `-A2 <T2>` (FLIRT, bet2 `-e`, betsurf), every program in-process (FLIRT, standard_space_roi,
  FAST, fslmaths, fslstats, betsurf, fslpy's `immv`, GNU dc), the `ERR` trap and bash's report of an
  aborted program; it refuses before running anything when a standard image is nowhere.
  `lx.fsl.bet(variation="eyes" | "bias" | "surfaces" | "surfaces_t2", t2=..., fsl_data=...)` runs them
  through the replica (`BetResult.surfaces` for -A/-A2); with `implementation="original"` (or no
  larmorx-nc) they raise `NotImplementedError`: the clean-room original is the catch-up part's.
- **New defect FSL-216** (bet script, rule A; NaN/infinity rule B): `-S`'s eye threshold through `dc`
  misreads what `fslstats` prints (an exponent, a minus sign, `nan`, `inf`). Register row: HANDOFF.toml
  (after FSL-215). FSL-63's row widened to `-B -s` (the same `echo | sed`, `bet:388`).
- **Evidence (not a record yet):** the 26 new suite cases (17 `bet/`, 3 `defects/`, 6 `real/`: an adult
  `-S`, a tumour `-S`, a 3.5-year-old `-B`, an 82-year-old `-B -s`, a 75-year-old at 1.5 T `-A`, a
  7.5-year-old `-A2` with its T2w), standard tier, against FSL 6.0.7.23 through the oracle cache: 192
  of 192 files byte for byte, exit statuses and messages FSL's; the whole standard tier (255 cases)
  agrees after the suite's fixes. The record `fsl-bet-replica` is re-made in sub-task 6.
- **Fixes of earlier relays' work found here:** the golden case `fslstats.fsl182.reproduce` had been
  dropped (restored, checksum unchanged); the BET suite's crash pattern did not match bash's report
  once its process ID was masked (two permanent difftest cases failed; fixed); five probe scripts
  lacked SPDX headers (fixed).
- **For `docs/next-steps.md` follow-ups:** the clean-room original of `-S`/`-B`/`-A`/`-A2` (catch-up
  part, from `specs/bet.md`'s sections the spec writer adds in sub-task 5); a known messages-only gap:
  bet2 or betsurf aborting on an uncaught exception inside `bet` print the replica's own line instead
  of the C++ runtime's text and bash's report (compared up to the crash, as before).

## Relay 4: sub-task 4, FSL's standard images in larmorx-nc

Commit 9e048b6 (branch worktree-agent-a70a1c0824b674bea).

- **What is done (the user's decisions of 2026-11-04).** The three images the options and
  standard_space_roi read (`MNI152_T1_2mm`, `MNI152_T1_2mm_brain_mask_dil`, `MNI152_T1_2mm_eye_mask`,
  `.nii.gz`, 1,420,897 bytes), copied byte for byte from FSL 6.0.7.23's fsl-data_standard 2208.0 (the
  oracle's install; each size and SHA-256 equal to the conda package's own record), are committed in
  `crates-nc/wheel-data/data/share/larmorx-nc/fsl-data/standard/` with `LICENSE` (FSL's Licence, a
  copy of `crates-nc/LICENSE`), `NOTICE` (the package's origin and checksum, MNI's notice),
  `MANIFEST.tsv` and a README. Nothing else image-like. `.gitignore` gains the one negation for that
  directory; `.gitattributes` marks `*.nii.gz` binary.
- **The wheel.** `crates-nc/pyproject.toml`: `data = "wheel-data"` (maturin's wheel data: pip installs
  `wheel-data/data/` under the prefix), and the data's NOTICE and MANIFEST among the licence files.
  Linux x86-64 release wheel: 6,209,575 bytes, of which the images are 1,419,464 (about 30 % more than
  without). Installed into a fresh venv (`pip install --no-deps`): `<venv>/share/larmorx-nc/fsl-data/`,
  and `standard_space_roi -b` ran without FSL (exit 0).
- **The search** (`larmorx_nc_core::fsldata`): `bundled_for(exe)` also covers Windows' per-user scheme
  (program in `<userbase>\Python3XY\Scripts`, data in `<userbase>`); `BUNDLED_IMAGES` names the copy's
  images. Tests: `crates-nc/larmorx-nc-cli/tests/fsl_data.rs` (the manifest: exactly those files,
  unchanged, licence identical, notice lines, the wheel's data setting; every image found in the copy;
  the program end to end in an installation's layout with a blank and `~` in the prefix: refused
  without the copy, exit 0 with it; 0.7 s), `fsldata::tests::the_bundled_copy_beside_the_program`.
- **Shared document:** `docs/licensing.md` (HANDOFF.toml op 10: a bullet after the Armadillo/LAPACK
  one). Also for the orchestrator: `docs/next-steps.md`'s follow-up for the clean-room original's data
  (larmorx-nc's copy, else `$FSLDIR/data`, else a checksum-verified download from FSL's channel:
  `https://fsl.fmrib.ox.ac.uk/fsldownloads/fslconda/public/noarch/fsl-data_standard-2208.0-0.tar.bz2`,
  196,312,969 bytes, SHA-256 `2faefb4d...828a`, from which only the three files are kept; the download
  is the catch-up part's, the facts are in `specs/bet.md`).
- **Facts for the user (no decision needed):** FSL's data package names no licence (empty licence
  field, no file), so FSL's Licence applies to the files; MNI's notice (TemplateFlow's
  tpl-MNI152NLin6Sym LICENSE text, the 1993-2009 version) is reproduced, not a licence FSL ships. The
  whole download for the clean-room original is 196 MB for 1.4 MB used: a smaller official source
  (TemplateFlow's `tpl-MNI152NLin6Asym` res-02 T1w appears to be FSL's MNI152_T1_2mm: its header's
  `descrip` FSL5.0 and FSL's grid, larmorx-testdata's notes; not compared byte for byte; FSL's two
  masks are not there) is a later choice.

## Relay 4 (continued): sub-task 5, the specs for the clean-room original

Commits 4ceb259, de02c72, c29768a.

- **`specs/bet.md`**: §2 and §5.2 no longer say "pending"; **§5.3** `-S`, `-B`, `-A`, `-A2` step by
  step (each program and its arguments, the temporary files `-d` keeps, the observed data types,
  the eye threshold's decimal rule as GNU dc computes it with scale 10, checked against dc: the
  product truncated to SD's own places beyond 9; `-S -n`'s abort and bash's report; `-B -s`;
  standard_space_roi's status inside bet and FSL-214; the last variation; `-v`; the Python API);
  **§5.4** FSL's standard images (sizes, SHA-256s, types, licence) and where the original finds
  them, in the user's order: the named directory (`fsl_data=`, `LARMORX_FSL_DATA`), larmorx-nc's
  copy (`<prefix>/share/larmorx-nc/fsl-data`, read as data only), `$FSLDIR/data`, a download of FSL's
  `fsl-data_standard-2208.0-0.tar.bz2` (196,312,969 bytes, both checksums), cached; refused before
  running when none has it; §7's messages table, §9's rows (FSL-216, FSL-217; FSL-44, FSL-46 and
  FSL-63 widened), §10 speed, **§11.1** the validation plan.
- **`specs/standard_space_roi.md`** (new): standard_space_roi and fslroi complete (command line,
  both usage texts exactly, every option and message, the work step by step with the files and
  types, exit statuses, bash's abort reports per step, fslpy's `imcp`, the Python APIs, the data,
  numerics, FSL-211 to FSL-215 and FSL-217, speed, the validation plan).
- **New defect FSL-217** (rule A, found by the spec writer from the source and the replica, which
  gives −64 → 192 for an int8 input in both modes): fslroi and fslmerge write int8 as uint8 with the
  same bytes (newimage's closest templated type), so `bet -F` and `-Z` run bet2 on wrapped values.
  Register row: HANDOFF.toml op 11 (after FSL-216). **Still to do in this part (sub-task 6):** the
  replica's default correction (int8 kept, named; `larmorx_core::defects::FSL_217`), parity cases
  (`fslroi/int8`, `bet -F` and `-Z` of an int8 input) against FSL through the oracle cache.
- **For `docs/next-steps.md`** (the clean-room catch-up part): the original of `bet -S`, `-B`,
  `-A`, `-A2` from `specs/bet.md` §5.3-5.4, and of `standard_space_roi` and `fslroi` from
  `specs/standard_space_roi.md` (then `original: true` in the registry, `crates/larmorx-cli/src/
  replica.rs`), with the data search and the checksum-verified download of §5.4.

## Relay 5: sub-task 6 (tests, records, golden, memory, random testing; not the site)

- **FSL-217 corrected in the replica** (6314512): `larmorx_nc_core::avwutils::fslroi` and
  `fslmerge_z` keep an int8 image as int8 with its values by default (written with datatype 256)
  and give FSL's uint8 with the same bytes in `reproduce`; `Int8Output` tells the caller. Named:
  fslroi's own output whenever its input is int8 (the type differs from FSL's), with the number of
  negative values; inside `bet -F`/`-Z` where the result differs (a negative value: bet2 runs on
  other values; `-d` keeping the int8 temporary files); in `reproduce` where a value is negative.
  `larmorx_core::defects::FSL_217`. Parity: `fslroi/int8-negative`, `int8-positive`,
  `type-head-i8`, `bet/fmri-int8`, `fmri-int8-positive`, `small-fov-int8`, `small-fov-int8-debug`
  identical to FSL 6.0.7.23 in `reproduce` (FSL's datatype 2 confirmed); the default changes them
  exactly where FSL-217 is named (`bet/fmri-int8`: 8,066 voxels). The register row (HANDOFF.toml
  op 11) rewritten accordingly. **The register's class column of FSL-211 to FSL-217 corrected**
  (they all said `a`): FSL-211 `e` (a parsing quirk), FSL-212 to FSL-217 `c`.
- **The clean-room original's suite** leaves out fslroi and standard_space_roi (no original yet)
  and FSL-217's three bet cases (the original keeps int8's values and writes float temporary
  files, black-box: they fail its thresholds); a random case drawn for them with the original
  expects larmorx's refusal ("no clean-room original", exit 127). For the catch-up part's brief:
  put them back when their original exists (`REPLICA_ONLY_TOOLS`, `ORIGINAL_PENDING_CASES` in
  `parity/fsl_bet.py`; `REPLICA_ONLY_BET_CASES` in `tests/parity/test_memory_peak.py`).
- **Golden** (8604b84; recorded in this relay, see below): `larmorx-nc` +5 cases, `bet.eyes`
  (`-S -m`), `bet.bias` (`-B -s`), `bet.surfaces-t2` (`-A2`) on generated standard images
  (`LARMORX_FSL_DATA`: a template head of other noise on the head's 32 x 32 x 24 grid, int16, its
  dilated brain mask and an eye block, uint8; no FSL data and not larmorx-nc's own 91 x 109 x 91
  copy), `fslroi.int8` and `.reproduce` (FSL-217). Left out for CI's time: `-A` (its betsurf is
  `betsurf.t1only`'s, its other steps `-A2`'s) and `-S` in `reproduce` (GNU dc's reading differs
  only for FSL-216's numbers: unit tests). CI: the nc golden test grows from about 45 s to about
  59 s in the tests' build on the main machine (+14 s per platform, 6 jobs: under 2 minutes in all).
- **Memory** (a03d83b): `tests/parity/test_memory_peak.py`'s `REPLICA_ONLY_BET_CASES`: `bet -S -m`
  37.6 MiB (FSL 66.9), `bet -B` 104.7 (114.6), `bet -A2` 46.0 (71.9), `standard_space_roi -b` 35.4
  (66.9), on the suite's 96 x 112 x 84 head, every output FSL's byte for byte in `reproduce`; not in
  the original's list until its catch-up.
- **Random testing** (a03d83b, 3a795ff): the BET descriptor draws `-S`, `-B`, `-A`, `-A2`, fslroi
  and standard_space_roi; the trial `oracles/difftest/2026-11-04-bet-options-6b` (0.5 h, 2,699
  cases): the replica 2,580 agree, 70 notices, 47 differences, 1 timeout, all triaged
  (`triage.toml`): two replica mistakes fixed (FSL-63 named for `bet -R -e` when bet2 refused first;
  FSL-45 for `bet -Z -n`, only temporary files differing: notices only) with permanent cases;
  intermediate corrections and FSL-75 (FLIRT's buffer overrun: FSL segfaults or not by its heap)
  explained; fslpy's imcp conversion open (the replica refuses it clearly). The clean-room
  original's 190 differences (betsurf on tiny heads) are its own, open for the catch-up.
- **For `docs/validation/difftest.md`**: its fsl-bet row edited in the branch (not a shared
  document).
- **Records** (87e1bb8, from the clean commit 3a795ff): **`fsl-bet-replica`** (standard tier; it
  holds standard_space_roi's and fslroi's cases too, no separate record): 263 cases, 201
  bit-identical to FSL 6.0.7.23 (exit status, messages, every file: 600 of 600 byte for byte), 47
  rejected by both alike, 15 expected divergences (13 where FSL crashes and `reproduce` fills, 1
  where FSL's fill reads memory beyond the volume, 1 pending: fslpy's imcp conversion), 0 fail.
  Against main's record (178 cases): no common case moved; the 4 `bet/pending-*` cases are gone
  (replaced by the real `-S`, `-B`, `-A`, `-A2` cases, all pass). **`afni-tshift-replica`**
  re-made (the orchestrator's point (3)): 317 cases, the 2 that main's suite had gained
  (`methods/glibc-sinf-wsinc5`, `-wsinc9`) pass, nothing else moved. The generated tables
  (README's tool table, docs/validation/README.md) regenerated in the branch from
  `validation/tools.toml` (BET's entry now names standard_space_roi and fslroi, the options in
  the replica; FLIRT's "how" no longer lists standard_space_roi as missing from the replica).
- **The orchestrator's point (2), the older replica suites re-run** (oracle cache in use mode,
  scratch `rec-rerun/`, not records): `fsl-fast-replica` 87, `fsl-topup-replica` 176,
  `fsl-stats-replica` 208, `afni-automask-replica` 125, `afni-unifize-replica` 127 cases: every
  one identical to main's record (status and details; `compare_records.py`). No further NaN-sign
  or other mistake of the shared maths copy found there.
- **Golden recorded** (47f52a8, from 87e1bb8 whose smoke tier passed: 2,780 tests, 20 workers then
  the serial memory and thread tests): `larmorx-nc` 134 cases, 746 outputs (+9 cases; FLIRT's
  written headers moved in 8 FLIRT cases and `fast.priors`: `descrip` = newimage's build string,
  relay 2's change; data unchanged); `larmorx` re-recorded from the same commit, every checksum
  unchanged (228 cases). HANDOFF.toml ops 12 to 15: golden.md's counts, the BET row, the
  recording note.
- **Gates** at 47f52a8: `gates-rust.sh all` (fmt, clippy -D warnings, tests: root, crates-gpl,
  crates-nc) OK; `gates-python.sh --no-build` OK (792 and 630 passed); `tables --check` OK;
  `check_spdx.py` OK; `fsl_source.py --check` OK (146 files).
- **Commits of relay 5:** 6314512 (FSL-217), 8604b84 (golden cases), a03d83b (random testing's
  descriptor, the memory test), 6ac3547 and ac5b7b7 (the original's suite leaves out the
  replica-only programs and FSL-217's cases), 3a795ff (the trial's two notice fixes, permanent
  cases, triage, findings), 87e1bb8 (records, tables, API page), 47f52a8 (golden checksums).

## Relay 6: sub-task 6, the site

- **Site branch** `bet-options-pages` in `<scratch>/site` (4dddba7, on main 51d5465); **evidence-blobs**
  a1dfe8b (1073 blobs, manifest SHA-256 f3f4bc4edc3fd3ea; was 67aa66e, 998): push evidence-blobs
  (force) before main. Exported from larmorx 65543af7 (clean): `evidence fsl-bet` and `evidence
  fsl-maths` only; **not** exported here (shared, the orchestrator's at the merge): `summary`
  (`static/evidence/summary.json`) and `tables --site` (`data/status.yml`: BET's row still says
  "`bet -S`, `-B`, `-A`, `-A2` pending" and fslmaths' "2 one unit in the last place ... an FFT's
  twiddle factors"; both come from tools.toml/records), the Roadmap row.
- **BET's page**: seven new worked examples (slices view, the whole field of view of defaced
  data-panel heads): `bet -S` (ds000005, ds005003), `-B` (ds000228, ds003592 with `-s`), `-A`
  (ds002422), `-A2` (ds000256 with its T2w), `standard_space_roi -b` (ds000228); FSL 6.0.7.23
  through the oracle cache (the record's runs), the replica in both modes, FSL's `bet -m [-s]`
  without the option beside -S and -B (no verdict: the option's effect). Every replica (reproduce)
  output bit-identical to FSL's; the default differs only in the two -B examples (FSL-202, plus
  FSL-09 on ds003592), measured in each example. New sections #options, #ssroi, #data; the
  corrections FSL-211..217 and the known limits updated; the case explorer shows the 263-case
  standard record.
- **fslmaths' page**: the replica's entries recomputed (`fsl_maths.refresh_replica`) and exported
  from its record of 422 cases; the text: 360/61/1, sin/cos and the FFT's twiddle factors glibc's
  in reproduce, the two NaN-sign fixes (`-ztop`, `-dilall`).
- **Checks**: build 0 problems (153 repo links checked); numbers 416/416 (BET), 450/450
  (fslmaths); 332 viewers, 0 bad; inventory 594 counts grew, 1 fell, explained (fslmaths' agreement
  chart: the random-testing category's "expected divergence: 1" is now 0);
  `validation/investigations/bet-options-site/`.

## Relay 7: sub-task 7, the final pass

**`HANDOFF.toml` now holds 40 operations on 12 files** (ops 0-15 from relays 1-5, unchanged but for
relay 1's next-steps text, whose placeholder part path is now `docs/parts/2026-11-04-bet-options/`:
change it if the part's folder gets another name). Checked with `apply_handoff.py --check --site`
against a scratch merge of main `960c559` with this branch (one conflict: `tests/golden/golden.json`'s
`recorded` line, main's 4e1f7fc against the branch's 87e1bb8, which `merge_golden.py` settles: the
branch's recording is the newer and its checksums include main's unchanged ones) and of the site's
main `51d5465` with `bet-options-pages` (no conflict): every operation applies. Then, in those
scratch copies with the handoff applied: `tables --site` and `tables --check --site` exit 0,
`evidence summary --site` exit 0, the site's `tools/build.py --main-repo <the merge>`: 26 pages,
**0 problems** (153 repository links checked), both YAML files parse, and `register_counts.py`
gives 393 entries (a 91, b 13, c 191, d 24, e 59, f 13; no decide, no yes), the numbers op 34-36 write.

What the new operations (16-39) do, and why:

- **README.md** (16), **data/status.yml's planned FSL text** (31): the roadmap's FSL item was already
  stale at main (it still named fslmaths' and fslstats' clean-room originals, done 2026-11-04): it now
  names the user's order of 2026-11-04 (the clean-room catch-up with the options' original, then
  PRELUDE) and lists what is done, BET with every option, standard_space_roi and fslroi included.
- **CLAUDE.md** (17): "Where things are"'s Next line: this part complete, next the catch-up, then
  PRELUDE.
- **docs/status.md** (18-20): the FSL row's BET replica (every variation, standard_space_roi, fslroi,
  201 of 201 and 600 files from 130 and 318), the FSL bullet (the standard images), the Next line.
- **docs/next-steps.md** (21-24): item 4's list; item 7's sentence on the options; the part's
  paragraph after the glibc paragraph (record, defects, golden, memory, random testing, re-runs,
  specs, site); a follow-ups group: the catch-up's list (the original's specs, data search and
  download, `original: true`, the three lists to put back, the 190 open differences), imcp's
  conversion, applyxfm4D's and mcflirt's `descrip`, the messages-only abort gap, TemplateFlow as a
  smaller download, FSL's own step's 6 unclassified standard_space_roi cases, the record's stale
  notes (below), and the oracle-cache tests under xdist (below).
- **docs/licensing.md** (25): standard_space_roi, fslroi and fslpy's `imcp`/`immv` in larmorx-nc, and
  that GNU dc and bash's `[`/`echo`/word splitting are emulated from their manuals and observed runs
  (no GNU code), as the crates' PROVENANCE.md say. *To confirm at the merge:*
  `crates-nc/larmorx-nc-flirt/src/shtest.rs`'s header reads "bash 5.1's `test.c` as documented and
  as it behaves"; if its author read bash's `test.c` (GPL-3.0), only behaviour was taken (no code
  was copied or translated, by its PROVENANCE row), but the header could say "bash 5.1's `test`
  builtin" to leave no doubt.
- **docs/api/upstream-defects.md** (26, 27): rows FSL-211 to FSL-217 after FSL-204 (the page lists
  every FSL defect; the earlier relays had only the register), FSL-63's row widened to `-B -s`.
- **docs/findings/upstream-defects.md** (28, 29, 34-36): the D13-gaps paragraph of the BET replica
  (it still said "Open: BET's clean-room original": now the options, their rules, and what the
  original lacks); a restricted section "FSL-211 to FSL-217" (source lines, as the FSL-201 to
  FSL-204 section has); the counts (386 -> 393; c 185 -> 191, e 58 -> 59).
- **docs/decision-log.md** (30): an entry for the part's decisions by the rules (FSL-211 to FSL-217
  with A/B, fslroi's other templated types kept by rule C, imcp's conversion refused).
- **site.yml** (32): BET's summary names the options and the two programs. **content/roadmap.md**
  (33, 37-39): row 1 (fslmaths and fslstats, complete) replaced by the next step (the catch-up,
  BET's options' original, PRELUDE); the row numbers 6 and 7 after 4 corrected to 5 and 6; a
  Completed row for this part (linked to `/tools/fsl-bet/#options`). "Last updated" stays
  2026-11-04.

**At the merge, generated (not in the TOML):**
- `python -m larmorx_validation tables --site <site>`: BET's row (the use list with
  standard_space_roi and fslroi, "replica: 201 of 201"), FLIRT's "how" (standard_space_roi the
  replica's), fslmaths' replica 360 of 360, 3dTshift's replica 282 of 282 (its record re-made).
- `python -m larmorx_validation evidence summary --site <site>`: BET's replica 178 -> 263 cases
  (bit-identical 127 -> 198, both-reject 28 -> 41, divergence 20 -> 21), fslmaths' replica 421 -> 422
  (358 -> 360, divergence 2 -> 1); nothing else moves. Run it from the merge (it reads the site's
  evidence files, so after fetching `bet-options-pages`).
- Push `evidence-blobs` a1dfe8b (force) before the site's main; the site branch is on the site's
  main 51d5465 as it is now.
- *Optional:* the 3dTshift page's evidence was not exported again: its replica record gained 2
  passing cases (317), so the page's case explorer shows 315 while the status table says 282 of
  282 compared (`evidence afni-tshift` from the merge, if wanted: nothing shown is removed).

**Changed in the branch by this relay** (5dc17067, b5d4bc13; HEAD b5d4bc13, tree clean): the BET suite's texts. The replica record's
"Validated" line named only bet, bet2 and betsurf, its reference likewise, and its notes still said
that `-S`, `-B`, `-A`, `-A2` are refused for want of standard_space_roi, FLIRT and FAST; the clean-room
original's notes said larmorx lacks FLIRT and FAST. Both corrected in the suite; **the committed record
`fsl-bet-replica` (from 3a795ff) keeps the old sentences until its next re-make** (not re-made here:
each record once; its cases, statuses and numbers are right). Same for the clean-room original's record
`fsl-bet` (main's, not re-made by this part).

**The oracle-cache tests under xdist (the orchestrator's check):** yes, they fail at main `0f10327`
too. One run of `pytest -n 16 validation/tests` there: 2 failed,
`test_oracle_cache.py::test_a_rewritten_input_is_replayed_onto_this_runs_input` and
`::test_a_command_not_found_is_never_stored_nor_served` (159 passed, 1 skipped: the scratch checkout has no
workspace with `oracles/` above it; serially there 161 passed, the same 1 skipped). One run on the branch: 1 failed, `::test_stdout_bytes_are_what_subprocess_text_mode_gives`
(161 passed). Relay 6's: `::test_a_peak_is_measured_kept_and_served` and
`::test_a_command_not_found_is_never_stored_nor_served`; serially all pass. Each failure: a run the
test expects stored is not. Not this part's; recorded in
`validation/investigations/oracle-cache-xdist/` (README, `run_n16.sh`) and as a follow-up.

