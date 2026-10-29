# The clean-room fixes of 2026-10-29: black-box tools

The tools the clean-room implementer of the 2026-10-29 fixes (3dTshift, 3dAutomask and
3dUnifize's originals in `crates/larmorx-afni`, the head-motion correction in
`crates/larmorx-mri`) used to settle what the specs left open. Black-box only: AFNI 25.2.09,
FSL 6.0.7.17's mcflirt, larmorx's original and the replica programs are run and their outputs
read; nothing here reads AFNI's, FSL's or the replicas' source. What they showed is in the specs
(lines marked observed by the clean-room implementer, 2026-10-29),
[docs/findings/afni-masks-original.md](../../../docs/findings/afni-masks-original.md) and
[docs/findings/mri-hmc-original.md](../../../docs/findings/mri-hmc-original.md).

Run them from the repository with `PYTHONPATH=python:validation/src` and `LARMORX_WORKSPACE` set
(the AFNI binaries are `<workspace>/oracles/afni-25.2.09/bin`; larmorx is
`target/release/larmorx`, the replica `crates-gpl/target/release/larmorx-gpl`).

| Tool | What it shows | Behind |
|---|---|---|
| `probe.py show\|run <finding.json> <work> [--mode fix\|reproduce] [--brief]` | a random-testing recipe through AFNI, the original and the replica side by side: exit status, messages (or their differences), every header field and value digest where they differ | every item: reproducing each finding, and the replica's notices and default for the same call |
| `geometry_probe.py quat <work> [N]` | AFNI's quaternions as doubles (NIfTI-2 outputs) against candidate computations: only the polar decomposition's inverse with `1/det` in extended precision matches all (1,200 of 1,200) | headers of tiny oblique images (3dAutomask, 3dUnifize) |
| `geometry_probe.py orient <work> [N]` | the orientation letters 3dAutomask prints against the NIfTI library's closest-orientation method (double: 1,604 of 1,604) and each column's largest entry | `[from ?]`, `-SI` on a one-slice oblique image |
| `geometry_probe.py angle <work>` | the obliquity angle comes from the matrix AFNI reads (the sform), computed as the spec says | the data panel's angles (AFNI-10, below) |
| `geometry_probe.py qform-units <work> [N]` | the replica's default (AFNI-07 corrected) on qform-only headers in metres and micrometres | 3dTshift's metre-unit default |
| `tshift_times.py <work>` | every slice time of 16 headers (seconds, ms, µs) read back with `-slice k`: a float32 sum of a double step | `toffset` 0.36000001 |
| `signed_zero.py <work>` | a float64 −0 through `-apply_prefix` with no, NaN and finite slopes | the sign of zero |
| `hmc_alignment.py <night> <work> <out.jsonl> [--jobs N]`, `--summary <out.jsonl>` | every `mri-hmc` finding: mcflirt's and larmorx's corrected series' temporal standard deviation, by option and cost | the head-motion correction's differences |

The data panel's 3dAutomask angles: `strip_extensions.py` of
[../afni-masks-original/](../afni-masks-original/) copies an input without AFNI's header
extension; AFNI on the copy prints the original's angle (AFNI-10).
