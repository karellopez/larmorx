<!-- Draft of an issue for github.com/freesurfer/freesurfer. Status: draft, not yet filed. When filed, add the issue's URL here and in README.md, and the cause and fix from docs/findings/freesurfer-io.md Part 21 (FS-680, FS-685: source-derived, kept out of this open file). Register entry: FS-680 in docs/findings/upstream-defects.md; larmorx's replica reads both series by default and reproduces the crash with upstream_defects="reproduce". Reproducer: validation/investigations/freesurfer-ge-genesis/ (make_series.py, run.sh). -->
# mri_convert crashes (segmentation fault) on every GE Genesis and GE Signa LX series

*Draft, not yet filed.*

FreeSurfer 8.2.0 cannot read GE Genesis files (`I.001`, `I.002`, ... or `<stem>NNN.MR`) or GE Signa LX
files (`e<n>s<n>i<n>`) at all: `mri_convert` ends with a segmentation fault before it opens the first
slice, for any series, whatever its contents.

## To reproduce

Three 2 x 2 slices of each format, written by a short script from the formats' layout
(`make_series.py` in the reproducer folder; any real series behaves the same, since the crash comes
before the first slice is opened):

```
python3 make_series.py out
cd out
mri_convert -it genesis genesis/I.002 out.mgz
mri_convert lx/e1s1i2 out.mgz
mri_convert -it gelx lx/e1s1i2 out.mgz
```

FreeSurfer 8.2.0 (the official `freesurfer_ubuntu22-8.2.0_amd64.deb`, Ubuntu 22.04, x86-64):

```
$ mri_convert -it genesis genesis/I.002 out.mgz
Segmentation fault                        (exit status 139)
$ mri_convert lx/e1s1i2 out.mgz
reading from lx/e1s1i2...
Segmentation fault                        (exit status 139)
$ mri_convert -it gelx lx/e1s1i2 out.mgz
Segmentation fault                        (exit status 139)
```

With standard output unbuffered, the Genesis reader's own lines appear before the crash
(`fname_format  : genesis/I.%03d`); nothing more.

## Where

The crash happens in FreeSurfer's GE Genesis and GE LX readers, after they have worked out the series'
file names and before they open the first slice; the same code is in FreeSurfer's development branch
(2026-10-07). The faulty line and a one-line correction are in larmorx's findings (FS-680; with FS-685, a
second crash multi-echo Genesis series meet once the first is corrected), closed to larmorx's clean-room
implementers: they go into this issue when it is filed.

Found while porting FreeSurfer's volume readers to Rust for larmorx (a validation harness runs
FreeSurfer's own functions, compiled from the 8.2.0 tag, against the port on generated files).
