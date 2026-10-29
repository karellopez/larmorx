# HANDOFF: version update, part 2 (FSL 6.0.7.17 → 6.0.7.23), 2026-10-29

Branch: `worktree-agent-afdb6eadb1ef500f6` (from main `fa4dd75`). Each block is the exact text for one shared document,
with where it goes. Defect IDs used: FSL-59, FSL-60, FSL-61, FSL-62, FSL-63 (of FSL-59 to
FSL-68). Updated after the coordinator's four items (the site, the mcflirt replica's full
record, the five BET differences, mri-hmc's threads): sections 1, 2, 6, 7, 9, 10, 11, 12.

## 1. `docs/findings/upstream-defects.md`

### 1a. Table: replace the FSL-41 and FSL-43 rows; add five rows after FSL-58

```
| FSL-41 † | bet2 | no voxel strictly between the 2nd and 98th percentiles inside the initial sphere (a constant or binary image): the median read from an empty list, crash (FSL 6.0.7.17; **fixed in FSL 6.0.7.23**: the robust maximum) | a | none | refused by default, `reproduce` took the median as 0 (2026-10-25); since FSL 6.0.7.23 (2026-10-29) both modes take 6.0.7.23's robust maximum and name nothing; a constant image then meets FSL-43's crash, which the default refuses, naming FSL-43 | – |
| FSL-59 | mcflirt | FSL 6.0.7.23 only: `-sinc_final` and `-nn_final` resample with trilinear interpolation (newimage 2601.0's cached ShadowVolume keeps each volume's interpolation method from its first access) | c | none (fMRIPrep uses the default) | corrected by default (the method asked for, FSL 6.0.7.17's result); `reproduce` trilinear; named where the two series differ (2026-10-29) | – |
| FSL-60 | FSL's NIfTI layer (NewNifti 5.0.0) | FSL 6.0.7.23 only: an image name holding `.img` before its extension (a directory, the file's stem) is opened with its last `.img` made `.hdr`: another file is read or written, or the program aborts | c | none | corrected by default (the name given); `reproduce` renames as FSL 6.0.7.23 does; named by the program (2026-10-29) | – |
| FSL-61 | bet2 | FSL 6.0.7.23 only: `--binaryMask` with a mask on another grid than the input: the brain mask is applied over the input's dimensions, reading and writing outside it | a | none | refused in both modes (rule B; FSL's result depends on memory), named (2026-10-29) | – |
| FSL-62 | bet2 | FSL 6.0.7.23 only: `--ct` with `--binaryMask` converts the mask instead of the CT image and writes the mask, masked, as the brain image | c | none | corrected by default (the CT input masked; rule A); `reproduce` gives FSL's; named (2026-10-29) | – |
| FSL-63 | bet (script) | `-R` passes bet2's options through bash's `echo`, which takes leading words of `-n`, `-e`, `-E` as its own options: a leading `-e` (the mesh) is dropped, and backslashes after it read as escapes (FSL 6.0.7.17 and 6.0.7.23) | a | none (fMRIPrep's calls do not combine `-R` and `-e`) | corrected by default (the options as given); `reproduce` passes them through bash's `echo` as FSL does; named where they differ (2026-10-29) | – |
```

The FSL-43 row (replace):
```
| FSL-43 † | bet2, betsurf | a mask's fill starts at the surface's centroid without a bounds check: outside the image FSL reaches each point by its linear index (other voxels of the volume, or memory beyond it, which it reads and writes, and may crash on); at a NaN centre it writes far from the volume and crashes | a | none | the fill starts at the nearest voxel inside (rule A, 2026-10-24); `reproduce` does FSL's fill point by point by the linear index, memory beyond the volume read as not part of the mask and never written, and names the number of those reads (2026-10-29; before: FSL's defined part one voxel outside only); at a NaN centre it skips the fill (the mask holds the drawn surface) and names FSL-43 (2026-10-25) | – |
```

### 1b. Section "FSL-41": replace its "larmorx" and "D13 / gap" bullets

```
- **larmorx:** refused by default, after FSL's messages; `reproduce` took the median as 0 (FSL's
  defined part: everything before and after the read), named FSL-41 and wrote bet2's outputs
  (D13's rule for failure cases, 2026-10-11; done 2026-10-25).
- **FSL 6.0.7.23** (2026-10-29): fixed upstream: fsl-bet2 2111.9 takes the robust maximum as the
  median of an empty list ("Safe option here is to set the median to the robust max"; it lets
  `--binaryMask` work on binary masks). The replica follows it in both modes and names FSL-41 no
  more. A constant image still crashes FSL 6.0.7.23, later, in the mask's fill from the surface's
  NaN centre (FSL-43): the default refuses there, naming FSL-43; `reproduce` does not fill.
- **D13 / gap:** done (fixed upstream).
```

### 1c. New sections, after FSL-58's

```
### FSL-59 Final interpolation ignored (FSL 6.0.7.23) — (c)
- **What:** in FSL 6.0.7.23 `volume4D[t]` returns a ShadowVolume cached at its first access,
  with its own copy of the interpolation method (trilinear, during the registration); mcflirt's
  final loop sets the series' method to sinc or nearest neighbour, but `interpolate` reads the
  cached copy: `-sinc_final` and `-nn_final` resample trilinearly. `-spline_final` builds its own
  shadow from the series and is unaffected. FSL 6.0.7.17 made a fresh shadow at every access.
- **Evidence:** read (newimage 2601.0 `newimage.{h,cc}`); verified
  (`validation/investigations/version-fsl/final_interp_probe.sh`: every output of
  `-sinc_final` and `-nn_final` equals the default run's, byte for byte, in 6.0.7.23; the
  recordings' `opt-nn_final`, `small/nn_final`, `small/sinc_final`).
- **fMRIPrep:** none (its call uses the default interpolation).
- **larmorx:** corrected by default (the interpolation asked for, as FSL 6.0.7.17); `reproduce`
  resamples trilinearly; both name FSL-59 where the two series differ (rule A, 2026-10-29).
- **D13 / gap:** No gap.

### FSL-60 Image names holding `.img` (FSL 6.0.7.23) — (c)
- **What:** NewNifti 5.0.0 opens every image under `replaceSuffix(name, ".img", ".hdr")`, which
  replaces the last `.img` anywhere in the name: a directory (`a.img.d/in.nii.gz` →
  `a.hdr.d/in.nii.gz`) or the file's own stem (`sub.img2.nii.gz`). Another file is read or
  written, or FSL aborts with `Error: cant open file <renamed>`.
- **Evidence:** read (`NewNifti.cc`); verified (`path_suffix_probe.sh`: mcflirt `-out a.img.d/out`
  writes `a.hdr.d/out.nii.gz`; a pair into `x.hdr.y.img.d/` aborts); the replica's record's
  `edge/img-in-directory` and `edge/img-in-output-directory`.
- **fMRIPrep:** none (its names hold no `.img`).
- **larmorx:** corrected by default (the name given); `reproduce` renames as FSL 6.0.7.23
  does; the program names FSL-60 once (rule A, 2026-10-29). bet2's own name handling (FSL-40)
  aborts on such names before NewNifti is reached.
- **D13 / gap:** No gap.

### FSL-61 `--binaryMask` on another grid (FSL 6.0.7.23) — (a)
- **What:** bet2's new `--binaryMask` finds the surface on the mask and makes the brain mask on
  the mask's grid, then applies it over the input's dimensions: with a mask of other dimensions
  FSL reads and writes outside the brain mask's memory.
- **Evidence:** read (`bet2.cpp`).
- **fMRIPrep:** none.
- **larmorx:** refused in both modes, naming FSL-61 (rule B: the input makes no sense, and what
  FSL computes depends on memory). Gap: where the input is smaller than the mask in every
  dimension FSL's reads stay inside the buffer and are defined; `reproduce` refuses them too.
- **D13 / gap:** the smaller-input case above.

### FSL-62 `--ct --binaryMask` cuts the brain from the mask (FSL 6.0.7.23) — (c)
- **What:** with `--binaryMask` the mask is read first, so `--ct` converts the mask to Cormack
  units and keeps the unconverted mask as "the CT image": the brain image written is the mask,
  masked, instead of the CT input.
- **Evidence:** read (`bet2.cpp`); verified (the BET record's `defects/binary-mask-ct`:
  `reproduce` bit-identical to FSL; the default's brain image differs by up to 1e3, the CT
  values).
- **fMRIPrep:** none.
- **larmorx:** corrected by default (the brain cut from the CT input; rule A); `reproduce` gives
  FSL's; both name FSL-62.
- **D13 / gap:** No gap (the surface is still found on the converted mask, as FSL does).
```

### 1c'. Section "FSL-43": replace its "larmorx" bullet, add an "Evidence" line

```
- **Evidence:** ... ; random testing of the version update (2026-10-29, cases 661, 1297, 1308,
  80): a real centroid several voxels outside does not make FSL crash as a rule; FSL's fill
  checks each neighbour's coordinate along its own axis only and reaches a point by `value`'s
  linear index, so a point outside the image is another voxel of the volume (the start `1 -4 0`
  before a 3 × 6 × 4 image reaches every voxel through its neighbour one slice up: FSL 6.0.7.17
  and 6.0.7.23 fill the whole image) or memory beyond it (case 80: 6.0.7.17 crashes, 6.0.7.23
  writes values that depend on that memory).
- **larmorx:** the fill starts at the nearest voxel inside the image (rule A), named where that
  changes FSL's mask; `reproduce` does FSL's fill point by point (FSL's stack and order, its
  per-axis bounds, its linear index): inside the volume exactly, beyond it no write, a place the
  fill wrote read back as its value and any other read as not part of the mask; it names FSL-43
  with the number of reads beyond the volume (FSL's outputs may depend on them, and the suite
  judges them undefined there). At a NaN centroid (`INT_MIN`), where FSL crashes, `reproduce`
  skips the fill (the mask holds the drawn surface only) and the default refuses. Inside the
  `bet` script the same (its `reproduce` had corrected the fill: case 1297). Permanent cases
  `bet2-fill-by-index`, `bet-fill-by-index`, `bet-robust-crash-filled`, `bet2-fill-memory-beyond`.
```

### 1c''. New section, after FSL-62's

```
### FSL-63 `bet -R` loses a leading `-e` through `echo` — (a)
- **What:** under `-R` the script rebuilds bet2's options with ``bet2opts=`echo $bet2opts | sed
  's/-n//g'` `` (and `-B` with `-s`). bash's builtin `echo` takes the leading words made only of
  `-n`, `-e` and `-E` as its own options: with `bet <in> <out> -R -e` the mesh option is
  dropped and no `_mesh.vtk` is written (bet2's verbose lines are those without a mesh); after a
  leading `-e`, backslashes in the other options are read as escapes (`\c` ends the text). A
  leading `-n` is dropped too, as the `sed` meant.
- **Evidence:** read (`bet`, lines 246 and 388); verified (random testing of the version update,
  case 141: FSL 6.0.7.17 and 6.0.7.23 write no mesh; the permanent case `bet-robust-mesh-echo`).
- **fMRIPrep:** none (its calls do not combine `-R` with `-e`).
- **larmorx:** the options are passed as given (the mesh written); `reproduce` runs them through
  bash's `echo` as FSL does; both name FSL-63 where the two differ (rule A, 2026-10-29).
- **D13 / gap:** No gap.
```

### 1d. History (append)

```
**2026-10-29, the version update (FSL 6.0.7.17 → 6.0.7.23):** FSL-41 is fixed upstream
(fsl-bet2 2111.9: the robust maximum as the median of an empty list); the replica follows it in
both modes. Four new entries, all of FSL 6.0.7.23 only: FSL-59 (mcflirt's `-sinc_final` and
`-nn_final` resample trilinearly), FSL-60 (NewNifti 5.0.0 renames names holding `.img`),
FSL-61 and FSL-62 (bet2's new `--binaryMask` on another grid, and with `--ct`). No other FSL
entry changed. Not a defect, recorded in `findings/fsl-mcflirt.md`: FSL 6.0.7.23's Armadillo
14.2.1 inverts 4 × 4 matrices through LAPACK; the replica keeps Armadillo 10.5.1's closed form
for now (the last bits of 5 of 203 standard-tier cases; a follow-up). Random testing of the
update (2026-10-29) found FSL-63 (`bet -R` loses a leading `-e` through bash's `echo`), and
corrected FSL-43's `reproduce`: FSL's fill from outside the image reaches voxels by their
linear index (and memory beyond the volume), which the replica now does point by point.
```

## 2. `docs/api/upstream-defects.md`

Replace the FSL-41 row:
```
| FSL-41 | bet2 | `larmorx fsl bet2`, `bet`, `lx.fsl.bet` | FSL 6.0.7.17: a constant or binary image crashed reading the median of an empty list. **Fixed in FSL 6.0.7.23** (the robust maximum; the replica follows it in both modes) | the robust maximum, as FSL 6.0.7.23 |
```
Add after FSL-58:
```
| FSL-59 | mcflirt | `larmorx fsl mcflirt -sinc_final`, `-nn_final`, `lx.mri.hmc(implementation="replica", final="sinc"|"nearest")` | FSL 6.0.7.23 resamples trilinearly whatever the option | the interpolation asked for; `reproduce` trilinear |
| FSL-60 | FSL's NIfTI layer | every image name of `larmorx fsl ...` holding `.img` before its extension | FSL 6.0.7.23 opens the name with its last `.img` made `.hdr` | the name given; `reproduce` FSL's renamed name |
| FSL-61 | bet2 | `larmorx fsl bet2 --binaryMask=<mask>` with a mask on another grid | FSL reads and writes outside the mask | an error in both modes |
| FSL-62 | bet2 | `larmorx fsl bet2 --ct --binaryMask=<mask>` | FSL writes the mask, masked, as the brain image | the CT input masked; `reproduce` FSL's |
| FSL-63 | bet | `larmorx fsl bet <in> <out> -R -e ...`, `lx.fsl.bet(..., variation="robust", mesh=True)` | the script's `echo` drops a leading `-e`: no mesh | the options as given (the mesh written); `reproduce` FSL's |
```
And in the FSL-43 row, "what larmorx does": "the fill from the nearest voxel inside; `reproduce` FSL's fill by the image's linear index (memory beyond the volume read as not part of the mask)".

## 3. `docs/licensing.md` (append to the FSL notes)

```
- **FSL 6.0.7.23** (checked 2026-10-29): the replicas follow it; `crates-nc/fsl-source/` holds
  its original files of everything the translation now comes from (68 files, `MANIFEST.tsv`
  with each package's version and SHA-256; fsl-znzlib 2511.3's C++ files replace 2111.0's
  `znzlib.c`, which leaves the directory with the version it belonged to, kept in git history
  and in `~/fsl`; FSL 6.0.7.17's copies also in `<workspace>/archive/2026-10-29-fsl-source-6.0.7.17/`).
  Nothing is still translated from 6.0.7.17 alone: the files 6.0.7.23 did not change are the same
  bytes. Armadillo's matrix rules (`arma.rs`) are still Armadillo 10.5.1's (fsl-armawrap 0.6.0,
  Apache-2.0, not FSL code: notice in `crates-nc/NOTICE`). New pure-Rust dependencies of
  `larmorx-nc-core` for zstd and bzip2: `ruzstd` 0.9.1 (MIT), `bzip2` 0.6.1 (MIT OR Apache-2.0)
  with `libbz2-rs-sys` 0.2.5 (the bzip2-1.0.6 licence: permissive, the copyright notice kept in
  source). No C library.
```

## 4. `UPSTREAM.md`

Policy paragraph: replace "FSL (newest 6.0.7.23) waits for part 2" with "FSL moved to 6.0.7.23
on 2026-10-29 (part 2: the replicas; the clean-room head-motion correction stays validated
against 6.0.7.17 until a clean-room agent aligns it with FSL-59's behaviour)".

FSL section: the replicas' source is FSL 6.0.7.23 (`~/fsl-6.0.7.23`, FSL's official installation;
6.0.7.17 kept in `~/fsl`); its table gains a 6.0.7.23 column or rows: fsl-mcflirt 2111.0,
fsl-newimage 2601.0, fsl-miscmaths 2412.6, fsl-newnifti 5.0.0, fsl-utils 2412.6, fsl-znzlib
2511.3, fsl-bet2 2111.9, fsl-avwutils 2209.6, fsl-meshclass 2111.0, fsl-armawrap 0.7.0 (Armadillo
14.2.1), fslpy 3.27.0, OpenBLAS 0.3.33 (build hashes in `~/fsl-6.0.7.23/conda-meta/`).

## 5. `upstream.tsv`

Comment line 8: "an FSL conda package installed with FSL 6.0.7.23 under ~/fsl-6.0.7.23 (6.0.7.17
under ~/fsl)". Rows: fsl-newimage `2601.0`, fsl-miscmaths `2412.6`, fsl-newnifti `5.0.0`,
fsl-utils `2412.6`, fsl-znzlib `2511.3`, fsl-armawrap `0.7.0`; fsl-mcflirt and fsl-flirt unchanged;
add `fsl-bet2 https://git.fmrib.ox.ac.uk/fsl/bet2.git 2111.9 FSL-Licence fsl`,
`fsl-meshclass ... 2111.0`, `fsl-avwutils ... 2209.6` if the table should list every package the
replicas translate.

## 6. `CLAUDE.md`

Rule 2: add "   - FSL 6.0.7.23 (since 2026-10-29: the replicas; 6.0.7.17 kept for the clean-room
head-motion correction until it is aligned, and for the step between the two)". "Where things
are", the FSL row: "the mcflirt replica (...), bit-identical to mcflirt 6.0.7.23 as built (203 cases on the standard tier: 176 of 176 compared cases bit-identical, 22 rejected by both with the same messages, 5 expected divergences (Armadillo 14.2.1's LAPACK last bits), 0 failures; FSL's own step: 179 of 197 cases identical, 18 changed, every one explained)"
and "the BET replica (...): every file FSL 6.0.7.23 writes byte for byte in every compared case
(128 of 128 compared cases, 315 files; `--binaryMask` and the zstd and bzip2 types included;
31 rejected by both, 17 expected divergences)". The records table's FSL row gains
`fsl-mcflirt-replica-full` (the 6.0.7.17 full tier, restored from fa4dd75).

## 7. `docs/next-steps.md`

Item 3, FSL half: "**part 2, FSL: done** (2026-10-29, `docs/parts/2026-10-29-version-fsl/`):
the replicas follow FSL 6.0.7.23 (FSL-59 to FSL-62 new, FSL-41 fixed upstream; the zstd and
bzip2 formats; bet2 `--binaryMask`); records against 6.0.7.23 with FSL's own step from 6.0.7.17
in every case; Armadillo 14.2.1's LAPACK inverses not followed yet (below)."

New follow-ups:
```
- **The replicas follow Armadillo 14.2.1's LAPACK inverses** (FSL 6.0.7.23; replica author):
  4 × 4 `inv()` (diagonal `1/x`; triangular `dtrtri`; symmetric `dsytrf`/`dsytri`; else
  `dgetrf` + `dgetri`), `inv(A)·B` (`dgesv`; symmetric `dsysv`), `det()` by LU, in OpenBLAS
  0.3.33's Haswell order (the same results under every core tested), checked by a ctypes probe
  against `~/fsl-6.0.7.23/lib/liblapack.so.3`; then the mcflirt record's 5 expected divergences
  (`ARMA_LAPACK` in the suite) become passes. `docs/findings/fsl-mcflirt.md`, "FSL 6.0.7.23".
  It reaches BET too (`docs/findings/fsl-bet.md`, the version trial's triage):
  `niftivox2newimagevox_mat()` inverts the sampling matrix for an image stored neurologically,
  in bet2's `-c` (case 80: 18 of 15,360 voxels differ between 6.0.7.17 and 6.0.7.23) and in
  `fslstats -C` (case 141 shrunk: `-0.000000`, which `bet -R`'s `dc` reads as a subtraction);
  `bet2_versions.sh` and `cog_sign_probe.py` reproduce both. This part joins the
  SciPy/NumPy/OpenBLAS part (coordinator, 2026-10-29). Recommendation: port the four LAPACK
  routines Armadillo 14.2.1 calls for 4 × 4 (`dgetrf`/`dgetri`, `dgesv`, `dsytrf`/`dsytri`,
  `dtrtri`) as reference LAPACK's algorithms with OpenBLAS 0.3.33's Haswell kernels' summation
  order (the kernels matter only through `dgemm`/`dtrsm` blocking, which 4 × 4 does not reach:
  the unblocked `dgetf2`/`dtrti2` paths), and check them bit for bit against
  `~/fsl-6.0.7.23/lib/liblapack.so.3` through a ctypes probe on random and FSL-typical matrices
  (swap, sampling, rigid, affine), before switching `arma.rs`.
- **The clean-room head-motion correction follows FSL 6.0.7.23** (a clean-room agent; from
  `specs/mcflirt.md`, lines marked "(FSL 6.0.7.23)"): (1) FSL-59: `reproduce` resamples
  `-sinc_final` and `-nn_final` trilinearly, the default keeps the method asked for, both name
  FSL-59 where they differ (§9.3); (2) the zstd and bzip2 output types and their message (§10.1),
  reading by content (§4.1); (3) FSL-60 (§4.1); (4) its record against 6.0.7.23 and
  `DEFAULT["original"]` in `validation/src/larmorx_validation/parity/fsl_versions.py` set to
  6.0.7.23; (5) `-cost normmi` and `mutualinfo` from appendix B (the follow-up of 2026-10-29).
  Against 6.0.7.23 today: the standard tier (136 cases) against mcflirt 6.0.7.23 gives 120 pass, 11 rejected by both, 3 expected divergences, 2 failures: the known `real/ds003345/default` and `syn/motion/small/nn_final` (FSL-59); every other case as against 6.0.7.17 (`validation/investigations/version-fsl/mri-hmc-6.0.7.23.md`); its record and default stay on 6.0.7.17.
```

## 8. `docs/decision-log.md` (append)

```
## 2026-10-29: FSL 6.0.7.23, the version update's second part (orchestrator, by the rules)
- **The replicas follow FSL 6.0.7.23.** mcflirt's source is unchanged; its libraries and BET's
  bet2 changed: FSL-41 fixed upstream (followed in both modes, as AFNI-03 and AFNI-05); four new
  defects of 6.0.7.23 decided by the rules (FSL-59 and FSL-62 rule A, FSL-60 rule A, FSL-61
  rule B); bet2 `--binaryMask` and the zstd and bzip2 formats ported.
- **Not followed yet:** Armadillo 14.2.1's LAPACK 4 × 4 inverses (last bits of 5 of 203
  mcflirt cases), a follow-up for a replica author.
- **Two FSL installations, one choice:** `--fsl-version` / `LARMORX_FSL_VERSION` (6.0.7.23 by
  default; `LARMORX_FSLDIR` names any installation, `LARMORX_FSL_DIR` still read); every replica
  case also runs or reads 6.0.7.17 and its record names FSL's own step, with the reason. The
  clean-room head-motion correction stays on 6.0.7.17 until aligned (FSL-59).
- **The version trial's BET differences** (by the rules, 2026-10-29): FSL-63 new (`bet -R`
  loses a leading `-e` through bash's `echo`: rule A); FSL-43's `reproduce` follows FSL's fill
  by the linear index (a replica mistake: it had treated any start more than a voxel outside as
  a crash); FSL's reads of memory beyond the volume there are undefined (judged so by the suite);
  the Armadillo part also covers BET's `niftivox2newimagevox_mat` (bet2 `-c`, `fslstats -C`).
- **The FSL pages of the site** show FSL 6.0.7.23 beside 6.0.7.17 in figures-cache entries of
  their own, keyed with the version (`<case>-fsl-6.0.7.23`); 6.0.7.17's entries are kept.
```

## 9. `validation/difftest/triage.toml`

The trial (`difftest --tools fsl-mcflirt,fsl-bet,mri-hmc --budget 0.5 --jobs 8 --fsl-version
6.0.7.23 --out <workspace>/oracles/difftest/2026-10-29-version-fsl-2`, larmorx `15cd710`): 5,817
cases in 41.8 min. (A first trial, `2026-10-29-version-fsl`, is kept but void: larmorx's `_core`
library was rebuilt and replaced while it ran, and its later larmorx runs failed with
`SystemError`.) Sorted by `validation/investigations/version-fsl/triage_trial.py`:
- **mcflirt replica:** 2,222 cases, every one agrees.
- **BET replica:** 1,368 agree, 2 notices, 3 timeouts (300 s: `bet -Z`, `bet -F`, `bet -r` on
  large random heads), 6 differ. Replayed against FSL 6.0.7.17 (`difftest --replay ...
  --fsl-version 6.0.7.17`): `00080` agrees there (FSL 6.0.7.17 crashes in the mask's fill,
  FSL-43, which the replica fills; FSL 6.0.7.23 does not crash on the same writes outside the
  volume and writes a mask that depends on memory: FSL-43's undefined part, not reproduced);
  the five others (`00051`, `00141`, `00661`, `01297`, `01308`) differ against 6.0.7.17 alike:
  differences the update did not cause, found because the new output types changed the draws.
  **Triaged (the coordinator's item 3):** two replica mistakes (FSL-43's fill by the linear
  index, 661 and 1297: fixed), two of the suite's (51, 1308: fixed), one new defect of FSL's
  (FSL-63, 141), and case 80's memory reads (FSL-43, undefined) and Armadillo 14.2.1's
  arithmetic (80 as drawn, 141 shrunk: the Armadillo part). Each replayed against both versions
  after the fix (`validation/investigations/version-fsl/replay_bet_findings.sh`): 51, 661, 1297,
  1308, 80 (shrunk) and 141 (as drawn) agree with FSL 6.0.7.23; against 6.0.7.17 only the
  images' build string differs. Six permanent cases (`make_permanent_cases.sh`).
- **The clean-room head-motion correction** (against 6.0.7.23): 634 agree, 1,582 differ: the
  classes of the clean-room trial's entry (thresholds on random phantoms, `-cost normmi`) and
  about 260 more with `-sinc_final` or `-nn_final` (FSL-59: mcflirt 6.0.7.23 resamples
  trilinearly, the original follows the option), for the clean-room alignment.

```toml
# ---------------------------------------------------------------- the version update (FSL, 2026-10-29)

[[difference]]
tool = "fsl-bet"
implementation = "replica"
signature = ["out.nii.gz", "out_mask.nii.gz"]
status = "register"
explanation = "FSL 6.0.7.23 does not crash on FSL-43's writes far outside the volume where 6.0.7.17 did (SIGSEGV), and writes a mask that depends on memory; the replica's reproduce fills deterministically, as for 6.0.7.17's crash (FSL-43's undefined part; the trial's case 80 agrees against 6.0.7.17)"
link = "<workspace>/oracles/difftest/2026-10-29-version-fsl-2/findings/fsl-bet/20261029-00080-replica.json"

[[difference]]
tool = "fsl-bet"
implementation = "replica"
signature = ["outputs written"]
status = "harness"
explanation = "bet2 -n alone where FSL crashes in FSL-43's NaN fill (a constant image): the replica's filled run wrote nothing, which -n alone asks; the suite required an output (trial case 51; the check now asks for outputs only where the call does: asks_outputs)"
link = "validation/difftest/cases/fsl-bet/bet2-n-crash-nothing-asked.json"

[[difference]]
tool = "fsl-bet"
implementation = "replica"
signature = ["files written"]
status = "register"
explanation = "bet -R -e: the script passes bet2's options through bash's echo, which takes the leading -e as its own option, so FSL writes no mesh (FSL-63, new, rule A; FSL 6.0.7.17 and 6.0.7.23 alike; trial case 141). Its shrunk form, a 1 x 1 x 2 image stored neurologically, also meets FSL 6.0.7.23's fslstats -C printing -0.000000 (dc: stack empty), Armadillo 14.2.1's inverse: open for the Armadillo part"
link = "validation/difftest/cases/fsl-bet/bet-robust-mesh-echo.json"

[[difference]]
tool = "fsl-bet"
implementation = "replica"
signature = ["out.nii.gz", "out_mask.nii.gz"]
status = "fixed"
explanation = "FSL-43's fill from a start outside the image: FSL reaches each point by its linear index (other voxels of the volume), so it fills where the replica's reproduce had skipped the fill as for a crash (trial case 661, 72 of 72 values), and inside bet the replica's reproduce had corrected it (case 1297); reproduce now does FSL's fill point by point (fill_by_index), FSL's bytes in both versions"
link = "validation/difftest/cases/fsl-bet/bet2-fill-by-index.json"

[[difference]]
tool = "fsl-bet"
implementation = "replica"
signature = ["out.nii.gz", "out_mask.nii.gz", "out_overlay.nii.gz"]
status = "fixed"
explanation = "inside the bet script, reproduce corrected FSL-43's fill from outside the image instead of reproducing it (the fix flag passed to keep the script's failure for crashes; trial case 1297, 2,160 of 8,316 values): the same fill by the linear index now"
link = "validation/difftest/cases/fsl-bet/bet-fill-by-index.json"

[[difference]]
tool = "fsl-bet"
implementation = "replica"
signature = ["messages before FSL's crash"]
status = "harness"
explanation = "bet -R where FSL's second bet2 crashes (FSL-43's fill far above the image): the script's ERR trap prints 'bet failed during command:...' after the crash, which the suite had counted among what FSL printed before it (trial case 1308; _BET_FAILED); reproduce goes on, judged as a filled run"
link = "validation/difftest/cases/fsl-bet/bet-robust-crash-filled.json"

[[difference]]
tool = "fsl-bet"
implementation = "replica"
signature = ["out.nii.gz", "out_mask.nii.gz"]
status = "register"
explanation = "FSL-43's fill reads memory beyond the volume (bet2 -c 22 19 8 on a 3 x 4 x 8 ramp, trial case 80 shrunk: FSL 6.0.7.17 crashes, 6.0.7.23 writes 96 values that depend on that memory): undefined, judged so by the suite (reads_memory); the replica's reproduce reads it as not part of the mask and names the reads. Case 80 as drawn (a 30 x 32 x 16 image of 1.2 x 0.8 x 1.2 mm stored neurologically, -c): 18 voxels differ between FSL 6.0.7.17 and 6.0.7.23 from -c's centre through Armadillo 14.2.1's inverse (bet2_versions.sh): open for the Armadillo part"
link = "validation/difftest/cases/fsl-bet/bet2-fill-memory-beyond.json"

[[difference]]
tool = "mri-hmc"
implementation = "original"
signature = ["out_mcf.nii.gz: voxel values"]
status = "open"
explanation = "FSL-59: mcflirt 6.0.7.23 resamples -sinc_final and -nn_final trilinearly; the clean-room original follows the option (about 260 findings of the version trial against 6.0.7.23); its alignment is the clean-room agent's (specs/mcflirt.md §9.3, marked FSL 6.0.7.23); its default stays on 6.0.7.17 until then"
link = "<workspace>/oracles/difftest/2026-10-29-version-fsl-2/report.md"
```

## 10. `docs/validation/golden.md`

Line 18 (the `larmorx-nc` column): "36 cases, 261 outputs" (it said 33 and 242). Line 59
(`larmorx-nc fsl mcflirt`): 24 cases; add "and FSL-59 reproduced (`-nn_final` resampled
trilinearly, as FSL 6.0.7.23) with zstd outputs (`FSLOUTPUTTYPE=NIFTI_ZST`)". Line 60 (`larmorx-nc
fsl bet`, `bet2`, `betsurf`): 12 cases; add "`--binaryMask` (FSL 6.0.7.23) with bzip2 outputs
(`NIFTI_BZ2`), and `bet` with zstd outputs (`NIFTI_ZST`)". A paragraph after line 32:
"`larmorx-nc`'s checksums were re-recorded on 2026-10-29 from c58a6f6 (the version update to FSL
6.0.7.23): 3 cases added; 28 outputs of 10 BET cases changed, every one an image header (`descrip`:
newimage 2601.0's build string, `2601.0 2025-12-08T15:20:57Z`); no data checksum changed
(`validation/investigations/version-fsl/golden_changes.py`)."
The golden test also checks the defects each case names: `mcflirt.reffile.sinc.stages4.float32`
and `mcflirt.mutualinfo.nn.bins64` name FSL-59 now (corrected). The BET triage (FSL-43's fill,
FSL-63) changed no golden checksum (`cargo test -p larmorx-nc-cli --test golden`: every case
passes; no golden case starts a fill outside the image or uses `-R -e`).

## 11. The site

Branch `version-fsl-6.0.7.23` of the clone the coordinator prepared
(`<scratch>/version-fsl/site`): committed, 60a16ec and cbbac85 (on dc64b69, the clone's starting state; the sandbox allowed the commits). Exported from larmorx at the branch's head (the
evidence modules committed there): `fsl-bet.json`, `mri-hmc.json`, `summary.json`, and
`data/status.yml` (`tables --site`); the pages' prose `content/tools/fsl-bet.md` and
`mri-hmc.md`. No new image blob: FSL 6.0.7.23's panels and the replica's against it hold the
same values as 6.0.7.17's (content-addressed, the same blobs).
- **What the pages gain:** every example (BET: fMRIPrep's call, the T1w with `-R`, betsurf, the
  data panel's child; head motion: fMRIPrep's mcflirt) gains FSL 6.0.7.23's panel, the
  replica's run following it, "replica − FSL 6.0.7.23" (bit-identical on every example, headers
  too) and "FSL 6.0.7.23 − FSL 6.0.7.17" as a reference row (the same values on every call
  shown; the images' headers differ in the build string only; mcflirt: 240 of 240 matrices
  identical as text), and a fact naming which version each panel ran. The case explorers show
  both records of each replica side by side (BET: the full tier against 6.0.7.17, the standard
  tier against 6.0.7.23; head motion: `fsl-mcflirt-replica-full` against 6.0.7.17, restored, and
  the standard record against 6.0.7.23), each with its agreement chart; the head-motion panels
  now name FSL 6.0.7.17 where they said 6.0.7. The figures cache keys the new entries with the
  version (`<case>-fsl-6.0.7.23`, `evidence/fsl_step.py`); 6.0.7.17's entries are untouched and
  still served.
- **Summary** (`evidence/summary.py`): the Validation page's status table gains the BET standard
  record (FSL 6.0.7.23) and the head-motion replica's two records.
- **Inventory** (`validation/investigations/panel-ants/site_inventory.py <before> <after> fsl-bet
  mri-hmc`, and the built pages by `difftest/site_inventory.py`): no loss on either page: BET's examples 6→8 panels and 6→8 comparisons (the T1w 4→6 and 3→5, betsurf 6→8 and 5→7), cases 173→190, shown numbers 65→97; head motion's example 10→12 panels and 9→11 comparisons, cases 196→209, shown numbers 202→239; the Validation page's shown numbers 350→362; no shared shown number decreased (the two changed are the pages' case totals, which grew); the three re-exported evidence files changed, none removed, no blob added (the new panels' values are the old ones')
- **Screenshots and the number check** (`validation/investigations/version-fsl/site_shots.sh`,
  before and after, desktop and phone, light and dark): number check: BET 96 of 96, head motion 238 of 238, Validation 362 of 362 (before: 64 and 201); 33 screenshots in `<scratch>/version-fsl/shots/{before,after}/`; the build: 0 problems, 113 links into the repository checked against the worktree
- **Hand-written numbers** updated in the prose: BET's standard tier (176 cases against 6.0.7.23,
  315 of 315 files), the version trial's six findings, the corrections (FSL-60 to FSL-63, FSL-41
  fixed upstream), the Armadillo limit; head motion: 176 of 176 against 6.0.7.23, the four cases
  of the full tier only, FSL-59 and FSL-60.

## 12. `docs/next-steps.md`: follow-ups (replaces the earlier section 12)

The two follow-ups of the first report are done in this part (the five BET differences: section 9;
the FSL pages of the site: section 11). Add instead, for the final validation phase: "the replicas'
full tiers against FSL 6.0.7.23 (`fsl-mcflirt-replica-full` and `fsl-bet-replica-full` are
6.0.7.17's: the four mcflirt cases only the full tier runs, among them fMRIPrep's 280-volume
multiband run of ds005454, and BET's real T1w and betsurf cases)". The Armadillo follow-up of
section 7 now names BET's two places (bet2 `-c`, `fslstats -C`).

## 13. The harness's threads (the coordinator's item 4; no shared document)

`parity/mri_hmc.py` runs larmorx at `LARMORX_PARITY_LX_THREADS` threads (4 by default, as the
ANTs suites; its `OMP_NUM_THREADS` set around each in-process run), `mri_hmc_count.py` likewise,
and random testing's `limit_threads` sets it; with `--jobs 6` the suite stays within 24 threads.
`docs/validation/README.md` could say so beside the other suites' thread settings.
