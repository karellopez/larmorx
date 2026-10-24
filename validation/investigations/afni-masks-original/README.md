# The clean-room originals of 3dAutomask and 3dUnifize: the black-box tools

The tools with which the clean-room implementer of `crates/larmorx-afni`'s `automask` and
`unifize` (2026-10-23) established what [`specs/3dAutomask.md`](../../../specs/3dAutomask.md)
and [`specs/3dUnifize.md`](../../../specs/3dUnifize.md) did not say (their lines marked
*observed by the clean-room implementer*) and the findings in
[`docs/findings/afni-masks-original.md`](../../../docs/findings/afni-masks-original.md). They
run AFNI 25.2.09's programs, the original and the replica as black boxes: nothing here reads
AFNI's source or the replica's, so a clean-room implementer may use them.

Run from the repository with `PYTHONPATH=python:validation/src` (the phantoms are the parity
suites'), AFNI in `<workspace>/oracles/afni-25.2.09/bin` (`scripts/build_afni_oracle.sh`), the
original in `target/release/larmorx` (`cargo build --release -p larmorx-cli`), the replica in
`crates-gpl/target/release/larmorx-gpl`; `blackbox.py`'s docstring has the variables that
override these paths.

| Tool | What it shows | How to run it |
|---|---|---|
| `blackbox.py phantoms <work>` | writes the inputs: the parity suites' synthetic heads, series, planes and edge cases, small heads and series for 3dUnifize (AFNI takes about 96 s on the full 64 × 64 × 40 phantom at one thread), negative values, unit scale factors, odd shapes | once, before the others |
| `blackbox.py compare <work> <tool> <outputs> -- <args>` | AFNI and the original (`reproduce` mode by default, `--mode fix` for the default) on one command line: `SAME`, or the first difference in exit status, messages and each output's header bytes and values. Every *observed* spec line was settled this way, and the code changed until every comparison said `SAME` | e.g. `compare <work> 3dAutomask m.nii -- -eclip -prefix m.nii @/data/head-f32.nii` |
| `blackbox.py unifize-batch <work>` | `compare` on 34 3dUnifize command lines (every option, `-EPI` with one and with 150 volumes, negative values, planes, empty and constant images, odd shapes): all `SAME`, except that values squashed above 1300 can differ in their last bit (glibc's `tanhf`, below) | a few minutes at three in parallel |
| `blackbox.py run <work> <prog> <args>` | one program's exit status, standard output and error; `<prog>` is AFNI's name, `lx:3dX` (the original) or `gpl:3dX` (the replica: the wording of larmorx's own notices, so both implementations name a defect in the same words) | |
| `compare_programs.py <work> [replica]` | the replica's and the original's transcripts on 24 command lines where larmorx speaks for itself (the AFNI-41 and AFNI-48 refusals: the last `-nbhrs`, `-NN1` after it, a non-number, `-T2` with `-EPI`; the notices of AFNI-01, -41, -43, -45, -48; a missing input), both modes, diffed. On 2026-10-24, against the corrected replica: identical but for a missing input in the default mode, where the replica adds the system's error (shared with 3dTshift) | a few minutes |
| `strip_extensions.py <in> <out>` | AFNI-10: copies a NIfTI-1 file without its header extensions (the header unchanged but `vox_offset`) and prints the extensions' codes. AFNI's 3dAutomask on fMRIPrep's `fixhdr_unifize/uni_xform.nii.gz` of ds005454 writes a header that differs from the original's in the 7th digit; on the stripped copy it writes the original's header byte for byte, so AFNI took the geometry from its own extension (code 4). The parity harness does the same in every case whose input has an extension (`afni_replica.afni_10_explainer`) | `strip_extensions.py uni_xform.nii.gz /tmp/x/noext.nii`, then `compare` on both |
| `coremath_checksums.c` | the ports of CORE-MATH's `tanhf` and `cbrtf` in `larmorx_core::math` are bit-identical to the C on all 2^32 floats: the C prints the checksums the Rust tests assert | its header comment (about 100 s) |
| `timing.py <out.json>` | informal timings of AFNI, the replica and the original on real inputs (fMRIPrep's calls on the oracle runs, a T1w, a BOLD series) at 1 and 6 threads | on a quiet machine; not a benchmark report |

**glibc's `tanhf`.** Where the original's 3dUnifize output differs from AFNI's in the last bit,
AFNI run with CORE-MATH's correctly rounded `tanhf` and `cbrtf` preloaded
(`scripts/build_crmath_preload.sh` builds them since 2026-10-23; `run --env
LD_PRELOAD=<preload>/libcrmath.so`) writes the original's bytes. The parity suite repeats this in
every case whose float values differ (`LARMORX_CRMATH_PRELOAD`).
