# FS-680: FreeSurfer 8.2.0 cannot read GE Genesis or GE Signa LX series

Open (behaviour only): black-box runs of FreeSurfer's own `mri_convert` on files made from the formats'
layout as `specs/freesurfer-io.md` §110-111 give it. The mechanism is in `docs/findings/freesurfer-io.md`
Part 21 (closed to FreeSurfer clean-room implementers).

- `make_series.py <out>`: two series of three 2 x 2 slices, `<out>/genesis/I.001` to `I.003` (GE Genesis)
  and `<out>/lx/e1s1i1` to `e1s1i3` (GE Signa LX).
- `run.sh <workspace> <out>`: FreeSurfer 8.2.0's official `mri_convert` (the Ubuntu 22.04 package unpacked
  in `<workspace>/oracles/freesurfer-8.2.0/root`) on each series, by name alone and with its input type
  (`-it genesis`, `-it gelx`).

Outcome (2026-11-14, Ubuntu 22.04 x86-64; `<out>` and `<workspace>` for the paths):

```
== FreeSurfer 8.2.0: mri_convert genesis/I.002 out.mgz
WARNING: searching dicom file genesis/I.002 for Manufacturer tag 0x8, 0x70
WARNING: the result could be a mess.
reading from genesis/I.002...
...
ERROR: genesis/I.002 is not a dicom file or some other problem
exit status 1
== FreeSurfer 8.2.0: mri_convert -it genesis genesis/I.002 out.mgz
Segmentation fault
exit status 139
== FreeSurfer 8.2.0: mri_convert lx/e1s1i2 out.mgz
reading from lx/e1s1i2...
Segmentation fault
exit status 139
== FreeSurfer 8.2.0: mri_convert -it gelx lx/e1s1i2 out.mgz
Segmentation fault
exit status 139
```

The Genesis series by name alone is taken for DICOM (these files parse as a bare DICOM data set;
larmorx's format detection says the same). With its type, and the LX series either way, the program crashes
before it opens the first slice. The probe's piece 19 (`validation/probes/fs-io/`, `grd`/`grh`) shows the
lines FreeSurfer prints before the crash (with an unbuffered standard output: `fname_format  : ...`), and
`gfd`/`gfh` the readers with the one faulty line corrected. The upstream report draft:
`docs/upstream-reports/freesurfer-ge-genesis-null-header.md`.
