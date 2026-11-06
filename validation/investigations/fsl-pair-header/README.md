# fsl-pair-header: what FSL writes as a NIfTI pair's header file

A black-box probe (no FSL source read; FSL 6.0.7.23's programs run through the oracle-output
cache, larmorx's clean-room originals as a program with `LARMORX_IMPLEMENTATION=original`).
Written in PRELUDE's clean-room part (2026-11-06) to settle the pair header of the clean-room
FSL image writers: `larmorx_fsl::bet::image::write` (BET, bet2, betsurf, FAST, FLIRT and its
programs, topup, applytopup, fslmaths, fslroi, standard_space_roi, prelude) and
`larmorx_mri::hmc::image::write_like_files` (mcflirt and `lx.mri.hmc`).

| File | What it shows | How to run |
|---|---|---|
| `probe.py` | for fslmaths, fslroi, bet2, fast, flirt, mcflirt, prelude, topup and applytopup, each of the eight pair types of `FSLOUTPUTTYPE` (`NIFTI_PAIR`, `NIFTI2_PAIR`, each also `_GZ`, `_ZST`, `_BZ2`), optionally two single-file types, and with `FSL_LOAD_NIFTI_EXTENSIONS=1` inputs carrying one extension (fslmaths, bet2, fast, prelude) or two of different sizes (fslmaths): the length of every header file FSL writes (decompressed), the four bytes after its header block, and every file the original writes compared with FSL's, decompressed, byte for byte (`descrip` apart, which the suites compare separately; data files and other files reported too) | `boxed.sh 8 python -P probe.py <work dir> --larmorx <larmorx program> [--only fslmaths,prelude] [--singles] [--mode fix\|reproduce] [--json out.json]` (the workspace's `.venv`, `PYTHONPATH=<checkout>/validation/src`) |
| `time_step.py` | the time step (`pixdim[4]`) FSL writes for inputs whose time step is 0 or negative, against the originals: fslmaths (`-mul`, `-Tmean`), fslroi (a series, one volume) and prelude (a series, one volume, a 3D phase) | `boxed.sh 4 python -P time_step.py <work dir> <larmorx program>` |

What `time_step.py` showed (2026-11-06, 14 runs): FSL reads a time step as its magnitude, and as
1 for an image with a time axis (`dim[0]` 4 or more) whose time step is 0; its outputs keep that
(`specs/flirt.md` §4.1 states the same reading). prelude's original follows it since this relay
(found by its random-testing trial); **fslmaths' and fslroi's originals keep the stored value**
(8 of the 14 runs differ: a series of 0 written 0 where FSL writes 1, of -2 written -2 where FSL
writes 2): open, for a shared fix in the FSL readers.

What `probe.py` showed (2026-11-06, 106 runs; inputs generated in `<workspace>/oracles/inputs/fsl-pair-header/`):

- **FSL writes a pair's header file as the header block followed by NIfTI's four extension
  bytes, for every pair type and every program probed**: 352 bytes for NIfTI-1, 544 for NIfTI-2,
  the four bytes `00 00 00 00`; with extensions kept, `01 00 00 00` and the extensions in full
  (one extension of 48 bytes: 400 and 592 bytes; two of 48 and 96 bytes: 496 and 688). A pair's
  `vox_offset` is 0; FSL-56 (the data offset of a single file counting the first extension's
  size once per extension) does not touch pairs.
- **Before the fix** the clean-room writers wrote the header block alone (348 or 540 bytes) for
  every pair type, except prelude and FLIRT's uncompressed `.hdr`, which patched it afterwards;
  a `_ZST` or `_BZ2` pair dropped the input's extensions entirely (`FSL_LOAD_NIFTI_EXTENSIONS=1`).
  mcflirt's original also wrote ANALYZE's `extents` and `regular` as the input held them (0 in
  the probe's input), where FSL writes 16384 and `r` into every NIfTI-1 header (single files and
  pairs). 59 of 102 runs differed from FSL's header files.
- **After the fix** (the shared writers write the four bytes and the extensions into every
  pair's header file; mcflirt's writes `extents` 16384 and `regular` `r` into NIfTI-1 headers):
  105 of 106 runs with every header file identical to FSL's; the other is FSL-56 in a single
  `NIFTI_GZ` file with two extensions (the default corrects the data offset, D13), identical
  with `--mode reproduce` (15 of 15 runs of fslmaths' two-extension and prelude's cases).
- The parity suites did not see the difference: they compare header fields as nibabel reads
  them (the 348- or 540-byte block), not the header file's length. FAST, FLIRT, mcflirt, topup and
  applytopup's data on these small synthetic inputs differ from FSL's as their records' thresholds
  allow (iterative fits); the probe reports it, it does not judge it.

The unit tests `bet::image::tests::pair_headers_hold_the_extension_bytes_for_every_type`
(`crates/larmorx-fsl`) and `hmc::image::tests::pair_headers_hold_the_extension_bytes_for_every_type`
(`crates/larmorx-mri`) keep the rule.
