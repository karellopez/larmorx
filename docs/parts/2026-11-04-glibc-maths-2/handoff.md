# Handoff: the glibc maths part, second half (2026-11-04)

Branch `worktree-agent-a8216597e86536276` (larmorx), site branch `glibc-maths-2-pages` in
`<scratch>/site` (with its `evidence-blobs` commit). The exact edits that can be anchored are in
`HANDOFF.toml` (`apply_handoff.py --check --site <scratch>/site`: 2 operations apply); the rest is
below as text, because the register (`docs/findings/upstream-defects.md`) has restricted sections
that the clean-room implementer does not open, and its API page, the decision log, CLAUDE.md,
`docs/status.md` and the site's Roadmap are the orchestrator's to harmonize.

## 1. The register: GLIBC-07 (docs/findings/upstream-defects.md and docs/api/upstream-defects.md)

**Description** (as `larmorx_core::defects` now has it, changed in relay 2b):

> the mathematical functions FSL's and AFNI's programs call are not correctly rounded: the
> double-precision sin, cos, tan, asin, acos, atan, exp, log and pow (on image intensities,
> between 0.02 % (log) and 0.21 % (tan) of the arguments get a result one unit in the last place
> from the exact value) and the single-precision sinf, cosf, atan2f, expf, logf and powf; so
> fslmaths' -sin, -cos, -tan, -asin, -acos, -atan, -exp, -log and -pow of a double image (-dt
> double) differ in that last place on those voxels (a float image, the default, keeps the same
> values), its kernels, -bptf, -tfce and -roc, fslstats' -e and -E and 3dTshift's -wsinc5 and
> -wsinc9 weights (about 1 % of them) can take another last bit, and the iterative programs
> (FLIRT and its programs, mcflirt's mutual-information costs, FAST, topup's smoothing, bet2's
> powers, betsurf's blurring) another path; nor is sincos, which gives the twiddle factors of
> fslmaths' FFT convolution (through cexp): of the transform lengths 1 to 256, 80 hold a factor
> one unit in the last place from the exact one, so a few voxels of such a convolution differ in
> their last place

**What LARMORX does** (replace the clean-room originals' part of the row; the replicas' part is
unchanged until their step (b)):

> By default every function is correctly rounded (CORE-MATH's, the same bits on every platform).
> In `reproduce` mode the clean-room originals compute glibc 2.35's own bits through
> `larmorx_core::math`'s `*_glibc` functions (one `Libm` choice per call, `larmorx_core::math::Libm`):
> `exp`, `log`, `pow` and the single-precision functions translated from the permissive code glibc
> carries (Arm's Optimized Routines, fdlibm), and, since 2026-11-04, the double `sin`, `cos` and
> `sincos` written from a behavioural specification of glibc's LGPL code (`specs/glibc-dbl.md`;
> 768 of 768 blocks and 184 special arguments of the black-box record, and fresh arguments
> against the system's glibc, 11.8 billion per function for its FMA build and 5.9 billion for its
> SSE2 build, 0 differ) — in
> fslmaths (the double functions, the kernels, -bptf, -tfce, -roc), fslstats (-e, -E), FLIRT and
> its programs (the rotations, the decomposition, the blurs, the sinc kernel, the mutual
> information; avscale, makerot, convert_xfm -fixscaleskew), mcflirt's mutual-information costs,
> FAST, topup's smoothing, bet2's powers, betsurf's blurring and 3dTshift's weighted sinc. `tan`,
> `asin`, `acos` and `atan` stay correctly rounded in both modes (the user's decision of
> 2026-11-04). Named (in both modes) exactly where a value differs: fslmaths' voxelwise
> functions, fslstats -e/-E, avscale, makerot, convert_xfm -fixscaleskew and 3dTshift -wsinc5/-wsinc9;
> named nowhere in the iterative programs and fslmaths' filters, which cannot tell whether a
> result moved without running twice (the user's decision of 2026-11-04, "name exact cases only").

**D13 gap:** none left for the clean-room originals but `tan`, `asin`, `acos`, `atan` (correctly
rounded in `reproduce` too, by decision) and FSL's FFT twiddles (the original has no FFT: it sums
directly). The replicas: step (b) (next-steps, HANDOFF.toml).

**History line** (2026-11-04): "the glibc maths part, second half: `sin_glibc`, `cos_glibc`,
`sincos_glibc` (clean-room, from `specs/glibc-dbl.md`); every clean-room original's `reproduce`
path the specs name wired to the `*_glibc` functions; FAST's fMRIPrep calls and every
single-channel case, 41 FLIRT cases (the default registration among them), fslmaths' double
functions and 3dTshift's weighted sinc now the originals' bits in `reproduce`; GLIBC-07 named
exactly in the pointwise programs only (the user's decision)".

## 2. docs/validation/golden.md (HANDOFF.toml, op 0)

The five new cases, one line each (what reaches which `*_glibc` function; each call checked to
differ between the modes). golden.json: 223 → 228 cases, 1,583 → 1,623 outputs (40 added: 6, 3, 17,
8, 6); every earlier checksum unchanged. golden.md's header ("208 cases, 1283 outputs" for
`tests/golden/registry.py`) was already behind before this part: recount at the merge. The table
row "Command line, AFNI, MRI and FSL" may want the five added to its count.

## 3. docs/next-steps.md (HANDOFF.toml, op 1)

Follow-ups inserted at the top of "Follow-ups, when they fit": the three cases main's suites gained
that fail on main's code too (FAST `opt/nan-first`: FSL-203; FLIRT `programs/pointflirt-no-points`:
FSL-204; topup `opt/fsl145-one-coefficient`: FSL-145's stop), relay 2b's two spec questions
(fslmaths §10.4 and §19/§7; FLIRT §3/§8.1 step 9), the replica-side step (b), the `mri-hmc` record
with `--jobs 1`, and FLIRT's `bbr/type-local_abs` (open by the site's rule where the record
measures no band within the threshold). Also: move the part itself to "Done" in "The order from
here" (the orchestrator knows the line).

## 4. The generated tables

- README.md's and docs/validation/README.md's GENERATED blocks: regenerated in the branch by
  `python -m larmorx_validation tables` (relay 2c, from the new records and tools.toml's texts);
  nothing hand-edited. `tables --check` passes.
- The site's `data/status.yml`: regenerated in the site branch by `tables --site` (FLIRT 210 of
  217, FAST 60 of 62, topup 94 of 95, fslmaths 384 of 384 with 368 bit for bit, 3dTshift 282 of 282
  with 180 bit-identical); only the generated text changed. Re-run `tables --site` at the merge.

## 5. The site

- Branch `glibc-maths-2-pages` from the site's `main` 0dbfddb and `evidence-blobs` 206fa20 as they
  were at the start. Eight tools exported from larmorx `ab2440d` (clean): fsl-flirt, fsl-fast,
  fsl-topup, fsl-maths, fsl-stats, afni-tshift, mri-hmc, fsl-bet (the last two and fsl-stats for
  their records' provenance: every record was re-made in relay 2c; their numbers did not move).
  `static/evidence/summary.json` NOT exported (shared): re-export the summary at the merge (FAST's
  36 cases moved from *identical with correctly rounded maths* to *bit-identical*, FLIRT's 41 to
  *bit-identical*, so the Validation page's matrix changes).
- Each of the eight pages has a paragraph "The C library's last digits (since 2026-11-04)" before
  its verdicts, and its numbers follow the new records (FAST, FLIRT, topup, fslmaths, 3dTshift).
- FLIRT's page budget raised to 50 MB (`sizes.PAGE_BUDGETS`, with its reason): the BBR example
  stores the original's default-mode image in full now that it no longer equals the reproduce
  mode's (36 → about 42 MB).
- **Roadmap** (the orchestrator's; suggested row for "Completed", 2026-11-04): "**The C library's
  digits for reproduce, second half.** LARMORX's own implementation of glibc 2.35's double `sin`,
  `cos` and `sincos`, written from a behavioural specification and checked against the system's
  glibc on 11.8 billion arguments each, and the clean-room originals' `reproduce` mode computing
  the C library's functions as glibc does in fslmaths, fslstats, FLIRT, the head-motion correction,
  FAST, topup, BET and 3dTshift: FAST's fMRIPrep calls and 41 FLIRT cases, its default
  registration among them, now give FSL's bytes; the default stays correctly rounded."
- Nothing on `content/validation.md` or `site.yml` is stale.

## 6. CLAUDE.md "Where things are" and docs/status.md

The glibc maths part complete (both halves). `docs/status.md`: the clean-room originals'
`reproduce` mode computes glibc 2.35's functions (the list above); `tan`, `asin`, `acos`, `atan`
correctly rounded in both modes; GLIBC-07 named in exact cases only.

## 7. Pre-existing, for the record

The exported provenance of fsl-flirt, afni-tshift and mri-hmc carries `-dirty` stamps of older
entries and records (FLIRT's replica cache entries computed at `301fab7…-dirty`, FSL's own version
string "2412.6-dirty", one record stamp each in 3dTshift's and the head-motion correction's): the
same 17, 1 and 1 as on the published site; none from this part.
