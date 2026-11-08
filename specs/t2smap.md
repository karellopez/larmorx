# Spec: `lx.tedana.t2smap` (behaviour of tedana 26.0.3's `t2smap`)

(spec writer, 2026-11-08, relay 8a of the t2smap part) The input for the clean-room **original**
of tedana's `t2smap` (TE-Dependent ANAlysis's T2* and S0 maps of multi-echo fMRI and their
optimal combination), to be written in the main package as `crates/larmorx-tedana` (Apache-2.0)
and built as a next-generation implementation: the same results, designed for speed (the user,
2026-11-06; `docs/porting-guide.md`, "Clean-room originals: the same behaviour, built for speed").
tedana is licensed under the GNU LGPL version 2.1 (only), so larmorx has two implementations
(CLAUDE.md rule 1, `docs/licensing.md`):
- the bit-exact **replica** in `larmorx-gpl` (`crates-gpl/larmorx-gpl-tedana`, GPL-3.0-only under
  the LGPL 2.1's section 3; done 2026-11-08: `larmorx-gpl tedana t2smap`,
  `lx.tedana.t2smap(..., implementation="replica")`; the record
  `docs/validation/tedana-t2smap-replica.md`);
- the **original**, in the main package (`larmorx tedana t2smap`, `lx.tedana.t2smap(...,
  implementation="original")`), written from this file, tedana's documentation
  (https://tedana.readthedocs.io, `t2smap -h`), the papers below and black-box runs of tedana's
  program only.

**Whoever writes the original must not open** tedana's source: the workspace clone `tedana/`,
`tags/tedana-26.0.3*/` and `tags/tedana-25.0.1*/`, and every installed copy (`site-packages/tedana`
of `.venv`, of `oracles/python-newest` and inside fMRIPrep's image); the replica crates
(`crates-gpl/`, whose `larmorx-gpl-tedana` and `larmorx-gpl-cli` translate tedana); the findings
written from the source (`docs/findings/tedana-t2smap.md`; its last section's black-box facts are
copied into this spec); the replica author's probes (`validation/investigations/t2smap/`); the
replica's parity suite, random-testing descriptor and permanent cases
(`validation/src/larmorx_validation/parity/tedana_t2smap_replica.py`,
`validation/src/larmorx_validation/difftest/tools/tedana_t2smap.py`,
`validation/difftest/cases/tedana-t2smap/`); or `docs/parts/` (porting guide §1). It may read and
run `validation/investigations/t2smap-spec/` (this spec's black-box probe and its model: §18,
written from runs of tedana's program and from this text), run tedana's program as an oracle
(the launchers `oracles/inputs/tedana-t2smap-replica/launcher-{newest,shared}/t2smap`, which only
start it), and read and use the permissive ports of numpy, SciPy, nilearn, nibabel and CPython in
the main package (§13.9): they are translations of those libraries, not of tedana.

This spec describes behaviour: what goes in, what comes out, the maths and the conventions that
decide the results. It holds no code. Statements marked **(observed)** were confirmed by running
tedana's program (`validation/investigations/t2smap-spec/blackbox.py`, and the replica's suite,
which compares 69 cases with tedana bit for bit); the rest come from the analysis of the
program's behaviour, and §§3–11's arithmetic was confirmed by a model written from this text,
whose files equal tedana's bit for bit on the probe's cases and fMRIPrep's calls (§18). Message texts are tedana's output, quoted as it prints them: their prefixes
name tedana's module, function and line because tedana's log format prints them (§12.1).

**The method and its sources.** Posse et al., "Enhancement of BOLD-contrast sensitivity by
single-shot multi-echo functional MR imaging", Magnetic Resonance in Medicine 42:87–97, 1999 (the
T2*-weighted combination); Poser et al., "BOLD contrast sensitivity enhancement and artifact
reduction with multiecho EPI: parallel-acquired inhomogeneity-desensitized fMRI", MRM
55:1227–1235, 2006 (PAID); Kundu et al., "Differentiating BOLD and non-BOLD signals in fMRI time
series using multi-echo EPI", NeuroImage 60:1759–1770, 2012, and "Multi-echo fMRI: a review of
applications in fMRI denoising and analysis of BOLD signals", NeuroImage 154:59–80, 2017 (the
monoexponential decay model and its log-linear fit); DuPre et al., "TE-dependent analysis of
multi-echo fMRI with tedana", Journal of Open Source Software 6:3669, 2021 (the program). tedana's
documentation: "Multi-echo fMRI" and "Outputs of tedana" (the files and their contents), the
`t2smap` command-line page.

**Exact where it is cheap, thresholds where an order decides the last bits.** t2smap draws no
random numbers. Its results are decided by (a) integer and comparison logic (the masks, the
volumes, the echo counts, the file layout), which an original reproduces exactly; (b) elementwise
float32 and float64 arithmetic and the elementary functions named in §13.3, exact when done in
the precisions and with the functions this spec names; and (c) three computations whose last bits
depend on the order of their sums: numpy's sums and means (§13.2), numpy's least-squares solution
of every voxel of an echo count at once (§13.4) and SciPy's bounded nonlinear least squares
(§13.5). tedana's own results change in (c) with its library releases, its CPU path and its
threads (§17.3, "tedana's own spread"). The original may compute (c) its own way, within the
thresholds of §17.3, or exactly, with the main package's permissive ports (§13.9); §16 says what
must stay and what may change.

**Defects (D13).** By default larmorx corrects tedana's defects on this path and names each one
where it changed a result; `upstream_defects="reproduce"` (`LARMORX_UPSTREAM_DEFECTS=reproduce`)
gives tedana's behaviour: TED-01 to TED-18 (TED-06 kept as tedana does it: rule C), and the
library entries NP-03 (numpy's float32 logarithm), NP-04 (numpy's AVX-512 kernels: a register
row, the reference runs with them off) and GLIBC-07 (the C library's float64 `exp`, `log` and
`pow`). §15 gives each with what the default and `reproduce` do; §§2–13 describe tedana, that is
`reproduce`, unless they say otherwise.

**Versions.** tedana 26.0.3 (2026-04-03), the newest release, with the reference library
environment numpy 2.5.4 (its OpenBLAS 0.3.34, LAPACK 3.12.1), SciPy 1.18.1 (its own OpenBLAS,
LAPACK 3.12.0's routines), nibabel 5.4.2, nilearn 0.14.1, pandas 3.0.6, Python 3.12; the second
kept environment has numpy 2.3.5 (OpenBLAS 0.3.30, LAPACK 3.12.0) and SciPy 1.15.3 (§17.1).
fMRIPrep 25.2.6's image has tedana 25.0.1, the reference of the final comparisons only (§1, §17.3).

---

## 1. Purpose and use

For a multi-echo BOLD series (the same volumes acquired at E echo times TE_1 < ... < TE_E), t2smap
fits, voxel by voxel, the monoexponential decay

    S(TE) = S0 · exp(-TE / T2*)

to the echoes' signal, giving a T2* map and an S0 map, and combines the echoes into one series,
the "optimally combined" series: at each volume a weighted average of the echoes' values, with
weights TE · exp(-TE / T2*) (Posse et al. 1999; `--combmode t2s`, the default) or each echo's
temporal signal-to-noise ratio times its echo time (PAID, Poser et al. 2006; `--combmode paid`).

An **adaptive mask** first counts, per voxel of the analysis mask, the echoes with good signal
(§6): a voxel's fit and combination use its first n good echoes, and a voxel with a single good
echo is fitted and combined with the first two. The fit is either **log-linear** (`--fittype
loglin`, the default: a least-squares line through log(|S| + 1) against TE, §7.2) or a **bounded
monoexponential curve fit** started from the log-linear one (`--fittype curvefit`, §7.3), one
fit per voxel over all volumes (`--fitmode all`) or one per voxel and volume (`--fitmode ts`,
4D maps). After the fit, floors and a cap make the maps usable (§8); a root-mean-square error of
the fit per voxel and per volume is reported (§9).

**fMRIPrep's use** (fMRIPrep 25.2.6, `init_bold_t2s_wf`; its nipype interface `T2SMap`): for every
multi-echo BOLD run, `t2smap -d <echo 1> <echo 2> ... -e <TE_1> ... <TE_E> --mask <mask>
--fittype curvefit` (`--me-t2s-fit-method`, `curvefit` by default, or `loglin`), the echo times
in milliseconds (tedana 26 warns that milliseconds are deprecated), the echoes each resampled to
the BOLD reference, the mask fMRIPrep's BOLD mask dilated by two voxels, run in its node
directory with one thread; fMRIPrep keeps `T2starmap.nii.gz` (the full T2* map) and
`desc-optcom_bold.nii.gz` (the combined series). Of the workspace's oracle runs, three reach it
(four calls): ds003592 sub-280 (3 echoes, TE 14, 29.96, 45.92 ms, 64 × 64 × 43 × 200), ds000210
sub-02 (two tasks, 3 echoes) and ds000258 sub-21262 (4 echoes, big-endian int16, LPS, oblique)
(§17.2).

## 2. Command line

### 2.1 Syntax and the parser

`t2smap [options]` (larmorx: `larmorx tedana t2smap [options]`; the usage names the program
`t2smap`). The parser is Python 3.12's `argparse` with the "argument defaults" help formatter:
options only (no positional arguments); long options may be abbreviated to any unambiguous prefix
(`--fitt` for `--fittype`; `--n` is ambiguous) and written `--option=value`; `-d`, `-e`, `-f`,
`-h`, `-v` are the short options (`-dFILE` attaches its value); an argument that starts with `-`
and is not a negative number (`-2:1`) is taken as an option (observed: `--exclude -2:1` is
"expected one argument"); `--` ends the options. Options may be repeated (the last value wins;
`-d`, `-e` and `--masktype` take one or more values each time). The main package's
`larmorx_core::python::argparse` reproduces this parser, its help at every width and every error
(§13.9).

### 2.2 Options

| Option | Values (type) | Default | Meaning |
|---|---|---|---|
| `-d FILE [FILE ...]` | one or more existing files | required | the echoes' series, ascending echo times; a single file is a z-concatenated series (the E echoes stacked along the third axis: deprecated, §4.1) |
| `-e TE [TE ...]` | one or more floats (Python's `float()`: `inf`, `nan`, `1e-2` accepted) | required | the echo times: seconds when every value is in (0, 1), milliseconds when every value is at least 1, otherwise an error (§3, step 7) |
| `--out-dir PATH` | text | `.` | the output directory (made absolute; created if missing, one level) |
| `--prefix PREFIX` | text | `""` | a prefix of every output file name; `_` is appended unless it is empty or already ends with `_` |
| `--convention {orig,bids}` | choice | `bids` | the output names (§11.1): `bids` = BIDS derivatives 1.5.0 |
| `--verbose` | flag | off | with `curvefit`: the fit's variance and covariance maps too |
| `--overwrite`, `-f` | flag | off | overwrite existing outputs; without it an existing output stops the run (§11.2) |
| `--mask FILE` | an existing file | none | the analysis mask (its voxels > 0); without it a mask is computed from the first echo (§4.3) |
| `--masktype {dropout,decay,none} [...]` | one or more choices | `dropout` | the adaptive mask's methods (§6) |
| `--dummy-scans INT` | integer | 0 | volumes removed from the start of every echo (also from the combined series) |
| `--exclude EXCLUDE` | text: indices and ranges, `0,5:10,15` | none | volumes left out of the adaptive mask and the fit, kept in the RMSE and the combination (§5) |
| `--fittype {loglin,curvefit}` | choice | `loglin` | the fit (§7) |
| `--fitmode {all,ts}` | choice | `all` | one fit per voxel over all volumes, or per voxel and volume (§7.4) |
| `--combmode {t2s,paid}` | choice | `t2s` | the combination's weights (§10) |
| `--n-independent-echos INT` | integer | none | the number of independent echoes (EPTI acquisitions): only adds two warnings to the adaptive mask (§6.5) |
| `--n-threads INT` | integer | 1 | threads (tedana: the BLAS thread limit and the curve fit's worker processes; -1 none, ≤ 0 all CPUs for the curve fit). Results must not depend on it in larmorx (§16) |
| `--debug` | flag | off | the log at DEBUG level (the help's promise of a TSV log file is not kept: no file is written; rule C, §15.3) |
| `--quiet` | flag (hidden: not in the help) | off | the log at WARNING level (wins over `--debug`) |
| `-v`, `--version` | | | prints `t2smap v26.0.3` on standard output, exit 0 (observed) |
| `-h`, `--help` | | | the help on standard output, exit 0 |

### 2.3 The usage and help text (observed)

At `COLUMNS=80` (argparse wraps to the terminal width minus 2; the width is `COLUMNS`, else the
terminal's, else 80), `t2smap -h` prints, on standard output, exit status 0:

```
usage: t2smap [-h] -d FILE [FILE ...] -e TE [TE ...] [--out-dir PATH]
              [--prefix PREFIX] [--convention {orig,bids}] [--verbose]
              [--overwrite] [--mask FILE]
              [--masktype {dropout,decay,none} [{dropout,decay,none} ...]]
              [--dummy-scans INT] [--exclude EXCLUDE]
              [--fittype {loglin,curvefit}] [--fitmode {all,ts}]
              [--combmode {t2s,paid}] [--n-independent-echos INT]
              [--n-threads INT] [--debug] [-v]

options:
  -h, --help            show this help message and exit
  -v, --version         show program's version number and exit

Required Arguments:
  -d FILE [FILE ...]    Multi-echo dataset for analysis. A set of echo-
                        specific files in ascending order. The TEs of the data
                        should match the TEs listed in the -e argument.
                        (default: None)
  -e TE [TE ...]        Ascending echo times in seconds (per BIDS convention).
                        E.g., 0.015 0.039 0.063. Millisecond values (e.g.,
                        15.0 39.0 63.0) are still accepted but deprecated.
                        (default: None)

Output Control:
  --out-dir PATH        Output directory. (default: .)
  --prefix PREFIX       Prefix for filenames generated. (default: )
  --convention {orig,bids}
                        Filenaming convention. "bids" will use the latest BIDS
                        derivatives version. (default: bids)
  --verbose             Generate intermediate and additional files. (default:
                        False)
  --overwrite, -f       Force overwriting of files. (default: False)

Temporal and Spatial Masking:
  --mask FILE           Binary mask of voxels to include in TE Dependent
                        ANAlysis. Must be in the same space as `data`. If an
                        explicit mask is not provided, then Nilearn's
                        compute_epi_mask function will be used to derive a
                        mask from the first echo's data. Providing a mask is
                        recommended. (default: None)
  --masktype {dropout,decay,none} [{dropout,decay,none} ...]
                        Method(s) by which to define the adaptive mask. The
                        adaptive mask starts with the mask from '--mask', when
                        provided. It identifies voxels that have good data in
                        all vs a subset of echoes. "dropout" removes voxels
                        with much lower voxels than other voxels within each
                        echo. "decay" removes voxels where the raw signal does
                        not decay across echoes. Users can specify one, both,
                        or neither of the models. (default: ['dropout'])
  --dummy-scans INT     Number of dummy scans to remove from the beginning of
                        the data. (default: 0)
  --exclude EXCLUDE     Volume indices to exclude from adaptive mask
                        generation and T2* and S0 estimation, but which will
                        be retained in the optimally combined data. Can be
                        individual indices (e.g., '0,5,10'), ranges (e.g.,
                        '5:10'), or a mix of the two (e.g., '0,5:10,15').
                        Indices are 0-based. As in Python lists, ranges are
                        start-inclusive and end-exclusive (for example, '0:5'
                        includes the first [0] through fifth [4] timepoints).
                        Default is to not exclude any volumes. (default: None)

Decay Model Fitting and Optimal Combination:
  --fittype {loglin,curvefit}
                        Desired T2*/S0 fitting method. "loglin" means that a
                        linear model is fit to the log of the data. "curvefit"
                        means that a more computationally demanding
                        monoexponential model is fit to the raw data.
                        (default: loglin)
  --fitmode {all,ts}    Monoexponential model fitting scheme. "all" means that
                        the model is fit, per voxel, across all timepoints.
                        "ts" means that the model is fit, per voxel and per
                        timepoint. (default: all)
  --combmode {t2s,paid}
                        Combination scheme for TEs: "t2s" (Posse 1999), "paid"
                        (Poser) (default: t2s)

Component Selection:
  --n-independent-echos INT
                        Number of independent echoes to use in goodness of fit
                        metrics (fstat). Primarily used for EPTI acquisitions,
                        which have dependency across echoes. If not provided,
                        number of echoes will be used. (default: None)

Performance Control:
  --n-threads INT       Number of threads to use. Used by threadpoolctl to set
                        the parameter outside of the workflow function. Higher
                        numbers of threads tend to slow down performance on
                        typical datasets. (default: 1)
  --debug               Logs in the terminal will have increased verbosity,
                        and will also be written into a TSV file in the output
                        directory. (default: False)
```

The help paragraphs, in order, are the argparse groups "Required Arguments", "Output Control",
"Temporal and Spatial Masking", "Decay Model Fitting and Optimal Combination", "Component
Selection", "Performance Control"; each option's help is the text above unwrapped (the "default"
suffix is argparse's, from the defaults of §2.2), re-wrapped by argparse at other widths. The
replica's parser check compares the help at 19 widths (observed).

### 2.4 Parser errors (standard error, exit status 2)

argparse prints the usage (the first block above, on standard error), then `t2smap: error:
<message>`, and exits 2 (observed). The messages t2smap meets:

| Case | Message |
|---|---|
| `-d` or `-e` missing | `the following arguments are required: -d, -e` (the missing ones, in that order) |
| a `-d` or `--mask` path that is not a regular file (missing, or a directory) | `The file <path> does not exist!` (the path as given) |
| a choice outside its list | `argument --fittype: invalid choice: 'exp' (choose from loglin, curvefit)` (Python 3.12.16's form, without quotes around the choices) |
| a non-integer for an INT option | `argument --dummy-scans: invalid int value: '1.5'` |
| a non-float echo time | `argument -e: invalid float value: 'x'` |
| an option without its value | `argument --exclude: expected one argument` (for `-d`/`-e`/`--masktype`: `expected at least one argument`) |
| an unknown option or a stray value | `unrecognized arguments: <them, space-separated>` |
| an ambiguous abbreviation | `ambiguous option: --n could match --n-independent-echos, --n-threads` |

Each `-d` path is checked when parsed, in order; the first failure stops the parse.

### 2.5 Exit statuses

0 (success, `-h`, `-v`); 2 (a parser error); 1 (any other failure: Python's traceback on standard
error, §12.4). A run that fails after writing some outputs leaves them (§11.2).

## 3. The run, in order

Every observable step, in tedana's order (the messages' texts and prefixes are in §12.2):

1. The output directory is made absolute (`os.path.abspath` of `--out-dir` against the working
   directory: `.` and `..` resolved textually, symbolic links kept) and created if it does not
   exist (one level: a missing parent is `FileNotFoundError: [Errno 2] No such file or
   directory: '<abs>'`, exit 1, before any message).
2. `--exclude` is parsed (§5.2); its errors stop the run here, before any message (the output
   directory exists by then) (observed).
3. The log starts (§12.1): INFO `Using output directory: <abs>`.
4. `<out>/t2smap_call.sh` is written (§11.6).
5. With `--fitmode ts` and a non-empty `--exclude`: `ValueError: Excluding volumes is not
   supported for fitmode='ts'. Please set fitmode='all' or remove the exclude argument.` (observed).
6. (The host's and the libraries' descriptions are gathered for `dataset_description.json`.)
7. The echo times: if every TE is in (0, 1): INFO `TE values appear to be in seconds. Converting
   to milliseconds for internal use.` and each TE is multiplied by 1000 (float64); else if every
   TE is ≥ 1: the WARNING of milliseconds (§12.2) and the TEs are used as given; else (a TE ≤ 0,
   NaN, a mix): `ValueError: TE values must be positive and either all in seconds (values < 1,
   preferred per BIDS convention) or all in milliseconds (values >= 1, deprecated).` (observed).
   `inf` passes as milliseconds (TED-10). From here on every TE is in milliseconds.
8. The reference image is read (§4.2).
9. The outputs are set up: if `<out>/figures` does not exist, INFO `Generating figures
   directory: <out>/figures` and it is created (empty; it stays empty); with `--overwrite`, an
   existing confounds table (`<prefix><name>.tsv` of §11.1) is removed.
10. The mask (§4.3): with `--mask`, INFO `Using user-defined mask`; without it, the WARNING of
    §12.2 and the computed mask.
11. INFO `Loading input data: <the -d paths as a Python list>` (§12.2), then the data are read
    (§4.4; one file: the z-concatenation WARNING).
12. Dummy scans (§5.1); DEBUG `Resulting data shape: (S, E, T)` (S the mask's voxels, T the
    volumes left).
13. Excluded volumes (§5.2).
14. The adaptive mask (§6), its messages; DEBUG `Retaining <n>/<S> samples for denoising`; the
    adaptive mask image is **written** (its existence check can stop the run here, before the
    fit) (observed).
15. INFO `Computing T2* map`; the fit (§7). The echo times' count against the data's echoes is
    checked here, after the adaptive mask (observed): `ValueError: Second dimension of data (3)
    does not match number of echoes provided (tes; 2)`.
16. With `curvefit`: the fit failures image; with `--verbose`, the variance and covariance images.
17. Floors, limited maps, cap (§8); the full S0 image.
18. INFO `Calculating model fit quality metrics`; the RMSE (§9); the RMSE image, the confounds
    table, the limited S0 image, the limited T2* image (seconds).
19. INFO `Computing optimal combination`; the combination (§10) and its messages; the full T2*
    image (seconds), the combined series.
20. `dataset_description.json`, the registry (§11.5).
21. INFO `Workflow completed`; the two newsletter INFO lines (§12.2); exit 0.

## 4. Reading the inputs

### 4.1 Echo files and the z-concatenated form

Each `-d` file is an image nibabel reads. larmorx reads NIfTI-1 and NIfTI-2 (single `.nii` or
`.nii.gz` files and `.hdr`/`.img` pairs, either byte order), ANALYZE 7.5 and MGH/MGZ, as nibabel
does; other formats nibabel reads (AFNI BRIK/HEAD, MINC, PAR/REC, compressed `.bz2`/`.zst`) are
refused with a clear message (decision D5, the replica's behaviour: a documented gap). tedana
converts every image that is not a NIfTI-1 single-file image (pairs, ANALYZE, MGH) to an
in-memory NIfTI-1 image with the same data, affine (§4.5) and voxel sizes; a NIfTI-2 image is
kept as it is (in Python a NIfTI-2 image is a kind of NIfTI-1 image), which matters for the
outputs' format without a mask (§11.3).

With E echo times and E files, file e holds echo e. With **one file** (whatever E), the file is a
z-concatenated series: its third axis holds the E echoes' slices one after the other, echo e's
being slices `(e − 1)·nz .. e·nz − 1` with `nz = ⌊Z / E⌋` (Z the file's third dimension), and the
WARNING `DEPRECATION WARNING: We are planning to deprecate supplying a single z-concatenated data
file at the end of 2026. If you are using this option, please comment on
https://github.com/ME-ICA/tedana/issues/1313.` is printed when the data are read (step 11)
(observed). A single file with a single echo time is TED-12 (§15).

### 4.2 The reference image

The reference is the first echo's image (for a z-concatenated file: its first echo's slab, a new
NIfTI-1 image of the slab's float64 values with the file's affine and voxel sizes). It gives the
computed mask (§4.3), the outputs' class without a mask (§11.3) and the 4D outputs' voxel sizes
and repetition time (§11.3).

### 4.3 The mask

- **User mask** (`--mask`): the image (converted to NIfTI-1 as in §4.1; a non-NIfTI-1 image with
  more than three dimensions is first squeezed of its trailing axes of length 1, and refused if it
  still has more than three: `ValueError: <file> has more than 3 dimensions`), its values read as
  float64 with the header's scaling (§4.4), the mask is **value > 0** (NaN is not). A NIfTI-1
  single-file mask of shape (X, Y, Z, 1) is **not** squeezed (observed through the replica's
  suite); see §4.4 for what it does to the data's reading. The mask's own affine and header are
  kept: the outputs carry the mask's affine (§11.3). tedana never compares the mask's affine
  with the data's (TED-04).
- **No mask**: nilearn 0.14.1's `compute_epi_mask` of the reference image with its defaults
  (`lower_cutoff` 0.2, `upper_cutoff` 0.85, `connected`, `opening` 2): the mean over time of the
  reference's data as nibabel holds them (stored values for unscaled images, float64 for scaled
  ones and for a z-concatenated reference), smoothed by a Gaussian of full width at half maximum
  1 voxel along each axis (σ = 1/√(8 ln 2) voxels, SciPy's 1-D Gaussian filter per axis, mode
  `reflect`, truncated at 4σ: radius 2), NaNs put back and non-finite values set to 0; the
  values sorted; the threshold is the midpoint of the largest gap between consecutive sorted
  values whose lower index lies in [⌊0.2·N⌋, min(⌊0.85·N⌋, N − 1)) (the first largest gap); mask =
  mean ≥ threshold; then two binary erosions (SciPy's, the 6-neighbour cross), the largest
  connected component (6-connectivity; the first label in a tie), four dilations and two
  erosions; an image of 0/1 values with the reference's affine (nilearn's `MaskWarning: Computed
  an empty mask. Are you sure that input data are EPI images not detrended.` when the erosion
  leaves nothing: TED-03). The main package's `larmorx_interp::nilearn::compute_epi_mask`
  reproduces it bit for bit (§13.9).
- The mask's voxels are taken in C order of the (x, y, z) grid (x slowest, z fastest): every
  array of this spec indexed by "voxel" is in that order, S voxels.

### 4.4 The data array

Each echo's 4D data are read as float32 with the header's scaling as nibabel applies it for a
float32 request (`larmorx_io::nifti` `Scaling::F32` reproduces it): no scaling when the slope is 0
or not finite, or when (slope, intercept) is (1, 0); otherwise the slope and intercept (float32 in
the header) are applied in the type nibabel chooses for the stored type (float32 for the integer
types of real data: the stored integer converted to float32, times the slope, plus the intercept,
each rounded to float32; float64 data scaled in float64), and the result rounded to float32 (the
user mask: the same rule in float64, then `> 0`). Masked to the S voxels, the data array is
`x[v, e, t]`: S voxels × E echoes × T volumes, float32 (the int16 case of the probe prints other
thresholds than its float32 source: observed). A 3D echo file is `ValueError: Expected 4D
image` (observed; tedana prints two chained tracebacks, §12.4). An echo whose grid is not the
mask's shape, or a 4D mask of one volume (§4.3), makes tedana read every echo through nilearn's
masking instead, which applies the same scaling and **sets non-finite values to 0**, and refuses
a mask whose shape or affine differs from the image's (nilearn's `ValueError` texts, through
TED-04's check in the default). A z-concatenated file whose slab does not match the mask's shape
is `ValueError: Z-cat echo slice/mask shape mismatch`.

### 4.5 The affine as nibabel reads it

An image's affine is nibabel's "best" affine: the sform when `sform_code` > 0, else the qform when
`qform_code` > 0, else the voxel-size diagonal of an unaligned image (ANALYZE's convention). The
qform's matrix is computed from the quaternion in numpy's `longdouble` (x87 extended precision on
x86-64 Linux): every zero of the result is +0 (a float64 product would give -0) and oblique
entries can differ from float64 arithmetic in their last bit (observed, random testing: 4,000
headers; `larmorx_io::nifti::NiftiHeader::best_affine_nibabel` reproduces it).

## 5. Volumes: dummy scans and excluded volumes

### 5.1 `--dummy-scans N`

With N > 0: WARNING `Removing the first N volumes as dummy scans.` and the first N volumes of every
echo are removed: from here on there are T = T_file − N volumes, and every output (the RMSE, the
table, the combined series, `--fitmode ts`'s maps) covers only those. N = 0: nothing. N ≥ T_file
leaves no volume (TED-13; tedana then fails in the dropout method, or with no dropout method writes
maps of no volume, §15). A negative N removes nothing and shifts `--exclude`'s indices (TED-13).

### 5.2 `--exclude`

The text is split at commas; each item, stripped of blanks, is either an index `i` (Python's
`int()`: an optional sign, digits, underscores between digits) or a range `a:b` (both ends
present: `a..b-1`, empty when b ≤ a). Errors (`ValueError`, exit 1, before any log line,
observed): `Invalid volume indices string (<text>). Step sizes are not supported.` (more than one
colon), `... Open-ended ranges are not supported.` (an empty end), `... Negative indices are not
supported.` (a negative index or end), `invalid literal for int() with base 10: '<item>'` (not an
integer). The set of indices, sorted ascending, is the exclude list (an empty text: none).

At step 13, when the list is not empty: INFO `Excluding volumes: [<the indices, Python list>]`;
the indices below N (the dummy scans) are dropped and N subtracted from the rest; if none is left:
WARNING `All exclude indices overlap with dummy scans (N).` and nothing is excluded (observed). If
the largest remaining index is **greater than** T: `ValueError: The maximum exclude index (<i>)
is greater than the number of timepoints in the data (<T>).` (observed; an index equal to T passes
this check and fails later: TED-02). Otherwise INFO `Excluding <k> volumes from adaptive mask and
T2*/S0 estimation` (k the number left), and the adaptive mask and the fit use only the other
volumes; the RMSE (§9) and the combination (§10) use all T volumes. Excluding every volume is
TED-16.

## 6. The adaptive mask

Computed from the data array of the volumes used (all T, or those left by `--exclude`), with
threshold 1, from the methods of `--masktype`. Every count below is a number of echoes, 0 to E.

### 6.1 The base count (always)

An echo is good in a voxel when none of its used samples is NaN or ≤ 0 (−0 included; +∞ is good).
The base count is the number of **leading** good echoes: echo 1 good, then echo 2, ...; it stops at
the first bad echo.

### 6.2 `dropout`

1. The temporal mean of each voxel and echo, m[v, e] (float32: §13.2).
2. Among the voxels whose first-echo mean is not 0 (NaN is not 0, so it is kept), the 33rd
   percentile of m[·, 1] by numpy's `"higher"` method (§13.6): the value at index ⌈(n − 1) · 0.33⌉
   of those n means sorted ascending; if one of them is NaN the percentile is NaN (numpy's rule),
   so that no voxel equals it (step 3, TED-01). An empty set (no voxel, or every first-echo mean
   0) is numpy's `IndexError: index -1 is out of bounds for axis 0 with size 0` (TED-03's empty
   mask).
3. The exemplar voxels are those whose first-echo mean **equals** that value. Each exemplar's
   candidate thresholds are its E means divided by 3 (float32). With one exemplar they are the
   thresholds; with several, the candidates of the exemplar whose **candidates** (the thirds, not
   the means: the two can rank differently by rounding) have the largest float32 sum (numpy's sum
   along the echoes, the innermost axis, §13.2: left to right for fewer than eight echoes; the
   first such exemplar in voxel order). No exemplar (a NaN percentile: TED-01) is `ValueError: attempt to get
   argmax of an empty sequence` (observed). With one echo the thresholds are a single float32
   value, printed (step 4), and step 5 fails: `IndexError: invalid index to scalar variable.`
   (observed; TED-12).
4. INFO `Echo-wise intensity thresholds for adaptive mask: <thresholds>` (numpy's printing of a
   float32 array, §13.7; for E = 1 numpy's printing of one float32 scalar) (observed).
5. The dropout count of a voxel is the **largest** e with |m[v, e]| > threshold_e (strict), 0 if
   none; earlier echoes below their thresholds do not stop it (the minimum of §6.4 does).

`dropout` is counted twice in the minimum (harmless).

### 6.3 `decay`

With the same means m, the decay count is e − 1 for the first echo e ≥ 2 whose mean is not below
the previous echo's (m[v, e] − m[v, e − 1] ≥ 0, the difference in float32; a NaN difference is not
≥ 0), or E when the means decrease throughout.

### 6.4 The minimum, the removed voxels

The adaptive count is the minimum of the base count and the counts of the methods given (`none`
alone: the base count only, with WARNING `No methods provided for adaptive mask generation. Only
removing voxels with negative or NaN values`; `none` beside other methods is ignored). Voxels with a
count below 1 are set to 0, with WARNING `<n> voxels in user-defined mask do not have good signal.
Removing voxels from mask.` (the same words without a user mask) (observed). The voxels with a
count ≥ 1 are the fit's voxels; the others take no part in the fit, the RMSE or the combination
(their outputs are 0, or NaN in the RMSE map: §9).

### 6.5 `--n-independent-echos k`

Only warnings (EPTI acquisitions; the counts do not change): with N1 the voxels of count ≥ 1,
- t3 = ⌊2E / k⌋ (float64; k = 0 is Python's `ZeroDivisionError: division by zero`: TED-11); n3 the
  voxels with 1 ≤ count < t3; p3 = 100 · n3 / N1 (float64; N1 = 0 gives NaN with numpy's
  `RuntimeWarning: invalid value encountered in scalar divide`); if p3 > 0: WARNING `<n3> voxels
  (<p3 rounded to 1 decimal>%) have fewer than <t3 rounded> good voxels. These voxels will be used
  in all analyses, but might not include 3 independent echo measurements.`;
- if k > 3: td = ⌊(k − 1)E / k⌋, nd and pd likewise, and always WARNING `<nd> voxels (<pd>%) have
  fewer than <td> good voxels. The degrees of freedom for fits across echoes will remain <k> even
  if there might be fewer independent echo measurements.` (observed: `0 voxels (0.0%) have fewer
  than 2.0 good voxels. The degrees of freedom for fits across echoes will remain 4 even if there
  might be fewer independent echo measurements.`).
The rounded numbers print as numpy float64 scalars (`2.0`, `12.3`, `nan`: §13.7).

### 6.6 The image

The counts over the mask's S voxels are written at once as the adaptive mask image (int32: 0..E)
(observed), before the fit.

## 7. The fits

All in float64 unless said; TE in milliseconds; the data x[v, e, t] float32 of the fit's voxels
(count ≥ 1) and the used volumes (T_u of them).

### 7.1 Echo-count groups

The groups are the distinct counts present, with 2 added when 1 is present, keeping those ≥ 2, in
ascending order. Group 2 holds the voxels of count 1 **and** 2 and uses echoes 1–2; group n > 2
holds the voxels of count n and uses echoes 1..n. Every voxel's T2* and S0 are its group's fit; a
voxel of count 1 is fitted with two echoes ("full" maps) and set to 0 in the "limited" maps (§8).

### 7.2 The log-linear fit (`loglin`; also the curve fit's start)

For a group of n echoes and the N voxels of the group, in voxel order: each voxel's observations
are the n · T_u values y = log(|x| + 1) (float32 absolute value, float32 sum, float32 natural
logarithm: numpy's own float32 kernel, NP-03, §13.3), ordered echo by echo (echo 1's volumes in
order, then echo 2's, ...); the design matrix has the row (1, −TE_e) for each of those
observations (TE_e of the observation's echo). The fit is the linear least-squares solution
β = (β0, β1) of X β ≈ y for every voxel, β1 the decay rate (1/ms) and β0 log S0:

    T2* = 1 / β1,   S0 = exp(β0)        (float64 division; float64 exp: §13.3)

tedana solves all N voxels as one least-squares problem with N right-hand sides (numpy's `lstsq`,
`rcond=None`): an SVD-based minimum-norm solution, the singular values below eps · max(m, 2) · s_max
treated as zero (m = n · T_u rows), so equal echo times give the minimum-norm solution of the rank-1
problem; a voxel with β1 = 0 gives T2* = ±∞ (numpy's division: inf, with numpy's
`RuntimeWarning: divide by zero encountered in divide`, §12.3).
The solution of a voxel depends in its last bits on the other voxels of the batch and on the BLAS
threads (§13.4, black-box). One infinite sample makes y infinite and every voxel of its group NaN
(TED-09); no row (T_u = 0, TED-16) gives β = 0. A non-finite design (an infinite TE: TED-10) is
`numpy.linalg.LinAlgError: SVD did not converge in Linear Least Squares` (observed).

### 7.3 The monoexponential curve fit (`curvefit`)

The log-linear fit of §7.2 runs first (its maps are the starts). Then, group by group in ascending
order, every voxel of the group (voxel order) is fitted with SciPy 1.18.1's `curve_fit` of the
model f(TE; S0, T2*) = S0 · exp(−TE / T2*) to the voxel's n · T_u samples (float32 widened to
float64, the same echo-major order; xdata the matching echo times), with

- the start p0 = (S0, T2*) of the log-linear fit (for count-1 voxels: their group-2 values);
- the bounds S0 ∈ [min of the voxel's samples, ∞), T2* ∈ [0, ∞);
- therefore SciPy's **trust-region reflective** method (`least_squares` with `method="trf"`, the
  only one `curve_fit` uses with bounds; MINPACK is not reached): the cost ½ Σ r², r = f − y;
  `jac="2-point"` (forward differences, relative step √eps · max(1, |p_i|) with the sign of p_i,
  turned to a backward step when the forward one would leave the bounds); `x_scale` 1;
  `tr_solver="exact"` (a dense SVD of the scaled Jacobian); `ftol = xtol = gtol = 1e-8`; at most
  `max_nfev` = 200 evaluations (100 per parameter).

A voxel's fit **fails**, keeping its log-linear S0 and T2*, when SciPy raises: samples not finite
(numpy's `ValueError: array must not contain infs or NaNs`, checked first), bounds that are not strictly ordered (`Each lower bound must
be strictly less than each upper bound.`: an infinite smallest sample), a start outside the bounds
(a log-linear S0 below the voxel's smallest sample, a T2* < 0, or a NaN start: `Initial guess is
outside of provided bounds`; most failures on real data are this: TED-06), residuals not finite at
the start (`Residuals are not finite in the initial point.`), or no convergence within 200
evaluations (`RuntimeError: Optimal parameters not found: The maximum number of function
evaluations is exceeded.`); a start on a bound (T2* = 0) is first moved strictly inside. tedana
catches each of these, marks the voxel in the failures map and goes on. Otherwise S0 and T2* are
the fit's.

The **covariance** (for `--verbose`): from the final Jacobian J (m × 2) by SVD, singular values
below eps · max(m, 2) · s_max dropped, P = V diag(1/s²) Vᵀ (the pseudo-inverse of JᵀJ), times
Σr² / (m − 2) when m > 2. When P has a NaN entry, or m ≤ 2 (two samples: `--fitmode ts` with two
echoes, or one volume), every entry is +∞ and SciPy warns `OptimizeWarning: Covariance of the
parameters could not be estimated` (§12.3: printed once). The variance of S0 is P[0, 0],
of T2* P[1, 1], the covariance P[0, 1]; tedana writes P[0, 0] as the T2* variance and P[1, 1] as
the S0 variance (TED-07). Failed voxels have 0 in all three.

Per group, with `--debug`: DEBUG `With <n> echoes, monoexponential fit failed on <f>/<N>
(<100f/N with 2 decimals>%) voxel(s), used log linear estimate instead` when f > 0. tedana prints a
progress bar per group on standard error (`<n>-echo monoexponential`, tqdm's; none with
`TQDM_DISABLE=1`, the reference setup: §12.3).

The failures and variances of count-1 voxels (fitted in group 2) are **not** reported: the
failures map and the variance maps hold 0 there, while the full maps hold their group-2 fit
(TED-08).

### 7.4 `--fitmode ts`

The fit of §7.1–§7.3 runs for each volume t separately (T_u = 1: the voxel's n samples of that
volume), with the same adaptive mask and groups; every map, the failures and the variances become
S × T arrays (4D images). Not with `--exclude` (§3, step 5). For three echoes, the curve fit's
per-volume fits of two-echo voxels have m = 2: their covariance is +∞ and SciPy's warning is
printed (observed).

### 7.5 The full maps

The fit gives, per fit voxel (and per volume in `ts`), T2* (ms) and S0, the "full" maps; with
`curvefit` also the failures (true/false) and the three variance values. They are carried to §8.

## 8. Floors, limited maps and the cap

On the full maps of the fit voxels (in `ts`, each volume's column separately for the floor), in
this order:

1. T2* = +∞ → 500 (ms); T2* ≤ 0 → 1 (NaN stays NaN; −∞ → 1).
2. The underflow floor: for each voxel whose T2* ≠ 0, the E values exp(−TE_e / T2*) (float64, every
   echo e = 1..E, whatever the voxel's count) are computed; where one of them is exactly 0 (an
   underflow: −TE_e / T2* below about −745.13), T2* is set to min_e(−TE_e) / log(eps) = TE_max /
   36.04365338911715 (eps = 2⁻⁵², float64 log) (§13.3). With `--debug`, when such voxels exist:
   DEBUG `T2* values for <b>/<E · V> voxels (<100b/(E·V) with 2 decimals>%) have been identified as
   close to zero and have been adjusted` (V the voxels: TED-14, the denominator).
3. S0 = NaN → 0.
4. The **limited** maps are copies of the full ones with the count-1 voxels set to 0 (T2* and S0).
5. The cap: c = SciPy's `scoreatpercentile` of every value of the limited T2* (the fit voxels, or
   voxels × volumes in `ts`; zeros included) at 99.5 with `interpolation_method="lower"`: the
   value at index ⌊0.995 · (N − 1)⌋ of the values sorted ascending (NaN last) (§13.6). With
   `--debug`: DEBUG `Setting cap on T2* map at <10c with 5 decimals>` (observed: `609.26107`).
   Limited T2* values > 10c are set to c. **The full map is not capped** (TED-05); c ≤ 0 caps every
   positive limited value to c (TED-18).

The maps are then placed back on the mask's S voxels (0 outside the fit voxels). The written T2*
maps are in **seconds**: the ms value / 1000 (float64), then float32 (§11.3).

## 9. The RMSE map and the confounds table

Over **all T volumes** (after the dummy scans; excluded volumes included), the mask's S voxels,
with the **limited** maps (ms):

1. For each count n ≥ 2 present (n = 2..E; count-1 and count-0 voxels get none), each voxel v of
   count n and volume t (`all`: the voxel's T2* and S0; `ts`: those of volume t): the predictions
   p_e = S0 · exp(−TE_e / T2*) (float64), rounded to float32, for e = 1..n; the residuals
   x[v, e, t] − p_e (float32), squared (float32); their mean over the n echoes (float32 sum in echo
   order, then divided by n in float64 and rounded to float32: §13.2); its square root (float32).
   This is rmse[v, t]; it is NaN for the voxels of count 0 or 1. (A limited T2* of 0 in a count ≥ 2
   voxel, after TED-18's cap, divides by zero: numpy's warning, §12.3.)
2. The RMSE map: per voxel, numpy's `nansum` over the volumes (the NaN values replaced by +0 in
   place, then the float32 sum along the volumes, the innermost axis: pairwise, §13.2) divided by
   the number of non-NaN values (float64 division, rounded to float32); NaN when there is none.
3. Per volume t over the S voxels: `rmse_mean` = numpy's `nansum` over the voxels (NaN as +0; a
   plain float32 sum in voxel order, the voxels an outer axis) divided by the number of non-NaN
   values (float64, rounded to float32), NaN when none; `rmse_std` = numpy's
   `nanstd` (degrees of freedom 0; float32) of the non-NaN values; the percentiles 0, 2, 25, 50,
   75, 98, 100 of the non-NaN values by numpy's `nanpercentile` (linear method, §13.6; float64
   results), named `rmse_min`, `rmse_percentile02`, `rmse_percentile25`, `rmse_median`,
   `rmse_percentile75`, `rmse_percentile98`, `rmse_max`. A volume with no value has NaN everywhere
   (numpy's warnings `Degrees of freedom <= 0 for slice.` and `All-NaN slice encountered`, §12.3).
   When volume 0 has no value, all the percentile columns are float32 values, otherwise float64
   (numpy's rule for the first column decides the type: it shows in the table's digits, §11.4).
4. The confounds table has these nine columns in that order and one row per volume (§11.4).

## 10. The optimal combination

Over all T volumes, the mask's S voxels, the groups of §7.1 (group 2: counts 1 and 2), with the
**full** T2* map (ms; after §8's floors, uncapped: TED-05). For a voxel v of group n and volume t,
with the samples x_e = x[v, e, t] (float32, widened) and weights w_e (float64), e = 1..n:

    combined[v, t] = Σ_e w_e · x_e / Σ_e w_e      (float64; numpy's weighted average)

- `--combmode t2s`: w_e = TE_e · exp(−TE_e / T2*_v) (`all`: the voxel's T2*, the same weights for
  every volume; `ts`: T2*[v, t] of each volume, and where all n weights of a voxel and volume are 0
  they are set to 1). INFO `Optimally combining data with voxel-wise T2* estimates`, or
  `... voxel- and volume-wise T2* estimates` whenever the T2* map has a volume axis (`--fitmode ts`,
  even with one volume) (observed).
- `--combmode paid`: per voxel and echo, the temporal mean μ_e and standard deviation σ_e of
  x[v, e, ·] over all T volumes (float32 mean and float32 standard deviation, degrees of freedom 0,
  §13.2), snr_e = μ_e / σ_e (a float32 division) where σ_e ≠ 0, else 0; w_e = snr_e · TE_e (float64),
  the same for every volume; with WARNING `Argument 't2s' is not required if 'combmode' is 'paid'.
  't2s' array will not be used.` and INFO `Optimally combining data with parallel-acquired
  inhomogeneity desensitized (PAID) method` (observed; the T2* fit still runs, in both modes).
- The sums over echoes: in echo order for fewer than 8 echoes (numpy's rule for 8 or more, and
  the weights' sum in `ts` and for one volume: §13.2).
- A voxel whose weights sum to zero stops the whole run: `ZeroDivisionError: Weights sum to zero,
  can't be normalized` (PAID with constant good echoes, or any one-volume series: TED-15). NaN
  weights (a NaN T2*) give a NaN combination for that voxel.
- Voxels of count 0 are 0. The combined series is written as float32 (§11.3).

## 11. Outputs

### 11.1 Names, the convention and the prefix

Every file is `<out>/<prefix><name><extension>`: the prefix of §2.2 (`_` added), the name of the
convention, `.nii.gz` for images, `.tsv` for the table, `.json` for the JSON files (observed:
`sub-01_dataset_description.json` with `--prefix sub-01`).

| Description (the registry's key) | `bids` (default) | `orig` | When |
|---|---|---|---|
| adaptive mask img | `desc-adaptiveGoodSignal_mask` | `adaptive_mask` | always |
| fit failures img | `desc-decayFitFailures_mask` | `fit_failures` | `curvefit` |
| t2star variance img | `stat-variance_desc-t2star_statmap` | `t2s_var` | `curvefit` + `--verbose` |
| s0 variance img | `stat-variance_desc-s0_statmap` | `s0_var` | `curvefit` + `--verbose` |
| t2star-s0 covariance img | `stat-covariance_desc-t2star+s0_statmap` | `t2s_s0_covar` | `curvefit` + `--verbose` |
| s0 img | `S0map` | `s0vG` | always |
| rmse img | `desc-rmse_statmap` | `rmse` | always |
| confounds tsv | `desc-confounds_timeseries` | `confounds` | always |
| limited s0 img | `desc-limited_S0map` | `s0v` | always |
| limited t2star img | `desc-limited_T2starmap` | `t2sv` | always |
| t2star img | `T2starmap` | `t2svG` | always |
| combined img | `desc-optcom_bold` | `ts_OC` | always |
| data description json | `dataset_description` | `dataset_description` | always |
| registry json | `desc-tedana_registry` | `registry` | always |

Also `<out>/t2smap_call.sh` and the directory `<out>/figures/` (no prefix), §11.6.

### 11.2 The order of writing and existing files

The files are written in the order of §3: adaptive mask (step 14); failures, T2* variance, S0
variance, covariance (16); S0 (17); RMSE, confounds, limited S0, limited T2* (18); T2*, combined
(19); dataset description, registry (20). Before each, if the file exists and `--overwrite` is not
given: `RuntimeError: File <absolute name> already exists. In order to allow overwrite please use
the --overwrite option in the command line or the overwrite parameter in the Python API.` (exit 1;
the files written before stay) (observed: a second run stops at the adaptive mask). With
`--overwrite` each file is replaced (and the confounds table removed at step 9). A failure at any
step leaves the files written before it.

### 11.3 Images

- **Values**: each image holds its values on the mask's voxels and 0 elsewhere; the failures and
  variance images hold values on the fit voxels (count ≥ 1) only, 0 on the other mask voxels.
  float64 arrays are written as float32 (rounded to nearest), the adaptive mask as int32, the
  failures as uint8 (0/1). The maps are 3D in `--fitmode all` and 4D (T volumes) in `ts`; the
  combined series is 4D. The T2* images (full and limited) are in seconds (§8), the RMSE in the
  data's units.
- **Header** (observed): a new header as nibabel makes it for a new image from the mask image's
  affine (the user mask's own affine, or the reference's for the computed mask):
  `sform_code` 2 with the affine in `srow_*`; `qform_code` 0 with the quaternion, offsets and
  `pixdim[0]` (qfac) nibabel derives from the affine (its `set_qform`, through numpy's SVD and
  eigen-decomposition: `larmorx_io::nifti::header_for_image_nibabel` reproduces it); `pixdim[1..3]`
  the affine's voxel sizes; `xyzt_units` 0; `scl_slope` and `scl_inter` NaN; `descrip`, `aux_file`,
  `intent_*`, `cal_*`, `slice_*`, `toffset` empty or 0; `vox_offset` 352; the datatype of the
  values. 4D images: the voxel sizes are set to the reference image's four (x, y, z and the
  repetition time in `pixdim[4]`; for a converted reference they are float32 values) (observed:
  `(3.0, 3.0, 3.5, 2.0)`; `xyzt_units` stays 0). The mask's affine and the data's need not agree
  (TED-04).
- **Format**: NIfTI-1 single files (`n+1`), gzip-compressed (tedana: no file name, mtime 0,
  compression level 1); the compressed bytes are not part of the contract (the records compare
  headers and data). Without `--mask` and with a NIfTI-2 single-file first echo, every image is
  NIfTI-2 (`n+2`, 540-byte header) (observed by random testing).

### 11.4 The confounds table

Tab-separated, every line (the last one too) ended by `\n`, a header line with the nine column
names of §9, one line per volume, no index column, NaN written `n/a`; each value is the float64 number in Python's shortest
round-trip form (`repr`: `11.061917304992676`, `1e-05`, `0.0`), the float32 columns (`rmse_mean`,
`rmse_std`, and the percentile columns in §9's float32 case) widened to float64 first (observed:
`11.061917304992676\t5.293344020843506\t1.1068437099456787\t3.1794732332229616\t...`). Ties
between two shortest digit strings go to the even last digit, as numpy and Python print
(`larmorx_core::python::number::float_repr`).

### 11.5 JSON files

Both are Python's `json.dump(..., indent=4, sort_keys=True)`: keys sorted, four-space indents,
`", "`/`": "` separators as indent mode writes them (`,` at line ends), non-ASCII escaped
`\uXXXX`, floats in shortest form, **no final newline** (observed). `larmorx_core::python::json`
reproduces it.

- **`dataset_description.json`** (observed):
  `{"BIDSVersion": "1.5.0", "DatasetType": "derivative", "GeneratedBy": [{"CodeURL":
  "https://github.com/ME-ICA/tedana", "Command": <the command of §11.6>, "Description": "A pipeline
  estimating T2* from multi-echo fMRI data and combining data across echoes.", "Name": "t2smap",
  "Node": {"Machine", "Name", "Processor", "Release", "System", "Version": Python's
  `platform.uname()` of the host}, "Python": <Python's `sys.version`>, "Python_Libraries":
  {"bokeh", "mapca", "matplotlib", "nibabel", "nilearn", "numpy", "pandas", "robustica",
  "scikit-learn", "scipy", "threadpoolctl", "tqdm": their installed versions}, "Version":
  "26.0.3"}], "Name": "t2smap Outputs"}`. **larmorx's documented difference** (decision D6, both
  modes): `Python` names the program that made the files (the replica writes `larmorx-gpl 0.0.1
  (tedana 26.0.3's t2smap translated to Rust, with the results of Python 3.12.16 and the libraries
  listed)`; the original writes `larmorx <version> (a clean-room implementation of tedana 26.0.3's
  t2smap, with the results of Python 3.12.16 and the libraries listed)`), and `Python_Libraries`
  the versions of the reference environment whose results it reproduces (the newest: bokeh 3.10.0,
  mapca 0.0.8, matplotlib 3.11.2, nibabel 5.4.2, nilearn 0.14.1, numpy 2.5.4, pandas 3.0.6,
  robustica 0.1.4, scikit-learn 1.9.1, scipy 1.18.1, threadpoolctl 3.7.0, tqdm 4.70.1; the shared
  environment's numpy 2.3.5 and scipy 1.15.3 when that environment is reproduced, §14); every other
  key is tedana's, `Node` the host's `uname` as Python's `platform.uname()` reports it (observed on
  Ubuntu: `System` `Linux`, `Machine` and `Processor` `x86_64`).
- **The registry** (`desc-tedana_registry.json`): an object mapping each description of §11.1
  written before it (not itself) to the file's base name (prefix included), e.g. `{"adaptive mask
  img": "desc-adaptiveGoodSignal_mask.nii.gz", "combined img": "desc-optcom_bold.nii.gz", ...}`
  (observed).

### 11.6 `t2smap_call.sh` and `figures/`

`t2smap_call.sh` is the text `t2smap ` followed by the command's arguments joined by single spaces,
exactly as given (no quoting; relative paths stay relative), with **no newline**, written at step 4
(it is not subject to the existence check: always replaced). The same text is the JSON's `Command`.
For larmorx's programs the arguments are those after `t2smap` (`larmorx tedana t2smap -d a.nii.gz
...` writes `t2smap -d a.nii.gz ...`). `figures/` is created empty at step 9 when missing.

## 12. Messages

### 12.1 The log

tedana's log goes to standard error, one line per message: `<LEVEL padded to 8> <module>:<function>:<line>
<message>` (Python's `%(levelname)-8s %(module)s:%(funcName)s:%(lineno)d %(message)s`): `INFO    `,
`WARNING `, `DEBUG   `, `ERROR   ` (observed). The level is INFO; `--debug` DEBUG; `--quiet` WARNING
(also with `--debug`). Nothing goes to standard output except `-h` and `-v`.

### 12.2 The messages

Each with its exact prefix (observed unless marked), in the order of §3:

| Prefix | Message |
|---|---|
| `INFO     t2smap:t2smap_workflow:365` | `Using output directory: <absolute out-dir>` |
| `INFO     utils:check_te_values:792` | `TE values appear to be in seconds. Converting to milliseconds for internal use.` |
| `WARNING  utils:check_te_values:796` | `TE values appear to be in milliseconds. Per BIDS convention, echo times should be provided in seconds. Support for millisecond TE values is deprecated and will be removed in a future version. Please provide TE values in seconds.` |
| `INFO     io:__init__:164` | `Generating figures directory: <absolute out-dir>/figures` |
| `INFO     utils:load_mask:916` | `Using user-defined mask` |
| `WARNING  utils:load_mask:943` | `Computing EPI mask from first echo using nilearn's compute_epi_mask function. Most external pipelines include more reliable masking functions. It is strongly recommended to provide an external mask, and to visually confirm that mask accurately conforms to data boundaries.` |
| `INFO     t2smap:t2smap_workflow:414` | `Loading input data: ['<path 1>', '<path 2>', ...]` (Python's list of the `-d` strings as given: single quotes, or double quotes for a path holding `'`, backslashes doubled) |
| `WARNING  io:load_data_nilearn:1104` | the z-concatenation warning of §4.1 |
| `WARNING  t2smap:t2smap_workflow:418` | `Removing the first <N> volumes as dummy scans.` |
| `DEBUG    t2smap:t2smap_workflow:422` | `Resulting data shape: (<S>, <E>, <T>)` |
| `INFO     t2smap:t2smap_workflow:426` | `Excluding volumes: [<i>, <j>, ...]` |
| `WARNING  t2smap:t2smap_workflow:433` | `All exclude indices overlap with dummy scans (<N>).` |
| `INFO     t2smap:t2smap_workflow:441` | `Excluding <k> volumes from adaptive mask and T2*/S0 estimation` |
| `WARNING  utils:make_adaptive_mask:113` | `No methods provided for adaptive mask generation. Only removing voxels with negative or NaN values` |
| `INFO     utils:make_adaptive_mask:167` | `Echo-wise intensity thresholds for adaptive mask: [247.7863   142.79353   81.549416]` (§6.2, §13.7) |
| `WARNING  utils:make_adaptive_mask:195` | `<n> voxels in user-defined mask do not have good signal. Removing voxels from mask.` |
| `WARNING  utils:make_adaptive_mask:215` | the first `--n-independent-echos` warning (§6.5; prefix from the analysis, not observed) |
| `WARNING  utils:make_adaptive_mask:231` | the second `--n-independent-echos` warning (§6.5) |
| `DEBUG    t2smap:t2smap_workflow:455` | `Retaining <n>/<S> samples for denoising` |
| `INFO     t2smap:t2smap_workflow:458` | `Computing T2* map` |
| `DEBUG    decay:fit_monoexponential:261` | the failure count of §7.3 (prefix from the analysis) |
| `DEBUG    decay:_apply_t2s_floor:58` | the floor message of §8 (prefix from the analysis) |
| `DEBUG    decay:modify_t2s_s0_maps:624` | `Setting cap on T2* map at <10c, 5 decimals>` |
| `INFO     t2smap:t2smap_workflow:505` | `Calculating model fit quality metrics` |
| `INFO     t2smap:t2smap_workflow:521` | `Computing optimal combination` |
| `WARNING  combine:make_optcom:181` | `Argument 't2s' is not required if 'combmode' is 'paid'. 't2s' array will not be used.` |
| `INFO     combine:make_optcom:187` | `Optimally combining data with parallel-acquired inhomogeneity desensitized (PAID) method` |
| `INFO     combine:make_optcom:202` | `Optimally combining data with voxel-wise T2* estimates` (or `voxel- and volume-wise`) |
| `INFO     t2smap:t2smap_workflow:566` | `Workflow completed` |
| `INFO     utils:log_newsletter_info:812` | `Don't forget to subscribe to the tedana newsletter for updates! This is a very low volume email list.` |
| `INFO     utils:log_newsletter_info:816` | `https://groups.google.com/g/tedana-newsletter` |

Numbers in messages print as Python prints integers, numpy's shapes as Python tuples of ints,
percentages with two decimals (`%.2f`), the cap with five.

### 12.3 Python warnings and progress bars

Python's warnings go to standard error, each **once per place** (Python's default filter) as two
lines: `<file>:<line>: <Category>: <message>` and the source line of the place, indented two
spaces. The file is the installed library's (`.../site-packages/...`), so their location lines are
not part of larmorx's contract: an original prints each warning where tedana would, with its
category and message (§17.3 compares those). The warnings t2smap's path reaches (observed unless
marked):

| Where | Category: message |
|---|---|
| the temporal means of no volume (§6.2, TED-13, TED-16) | `RuntimeWarning: Mean of empty slice` (numpy 2.5.4; `Mean of empty slice.` in 2.3.5), then `RuntimeWarning: invalid value encountered in divide` |
| §6.5 with no fit voxel | `RuntimeWarning: invalid value encountered in scalar divide` (analysis) |
| §7.2, β1 = 0; §9 with a limited T2* of 0 | `RuntimeWarning: divide by zero encountered in divide` (analysis) |
| §7.3 covariance | `OptimizeWarning: Covariance of the parameters could not be estimated` (printed when the first fit that warns has finished: after its group's fits, observed with `--fitmode ts`) |
| §9, a volume with no RMSE | `RuntimeWarning: Degrees of freedom <= 0 for slice.`, `RuntimeWarning: All-NaN slice encountered` (analysis) |
| §10 PAID with infinite samples | `RuntimeWarning: invalid value encountered in reduce` / `... in subtract` (analysis) |

tqdm's progress bars (`<n>-echo monoexponential`, one per group and volume) print their own
timings and are not part of the contract; the reference runs set `TQDM_DISABLE=1` (§17.1), with
which tedana prints none. tqdm 4.70.1 takes the default of its `disable` parameter from the
environment variable `TQDM_DISABLE` (a name `TQDM_` followed by `disable` in any case; the last
such variable wins) as Python's truth of the text: **any non-empty value disables the bars**
(`1`, also `0`), an empty one does not (observed). An original prints no bar when the
environment disables them; otherwise its progress display is its own.

### 12.4 Errors (exit status 1)

Any exception ends the run with Python's traceback on standard error: `Traceback (most recent
call last):`, the stack (installed paths and source lines: not part of the contract), and the last
line `<ExceptionClass>: <message>` (`numpy.linalg.LinAlgError: ...` names its module); for an
exception raised while handling another, both tracebacks joined by `During handling of the above
exception, another exception occurred:` (observed: a 3D echo file). An original prints the first
line, a placeholder for the stack (the replica's form: `  <stack>` is the probe's normalisation,
not a requirement) and the last line exactly. The errors and their texts are those of §§2–10 and
§15.

## 13. Numeric conventions that decide the bits

### 13.1 Precision per quantity

| Quantity | Type |
|---|---|
| the data x, the temporal means, the thresholds, the log-linear observations y, the RMSE's residuals, squares, means and roots, PAID's means, deviations and SNR | float32 |
| the echo times (ms), the design, β, T2*, S0, the curve fit, the floors, the cap, the predictions before rounding, the weights, the combination | float64 |
| the counts | integers |
| the written images | float32 (int32 adaptive mask, uint8 failures) |

float32 operations round each result to float32 (no fused multiply-add, no extended precision);
float64 likewise. Every NaN produced from numbers is x86-64's default NaN (the sign bit set:
`0xFFF8...`/`0xFFC0...`) on every platform; NaN inputs propagate as they are.

### 13.2 numpy's sums and means

- A float32 sum along an array's **innermost** axis (in memory) is +0.0 (numpy's reduction
  identity) plus numpy's pairwise sum P of the n values: for n < 8 a left-to-right sum started from
  −0.0; for 8 ≤ n ≤ 128 eight partial sums r_k of the elements i ≡ k (mod 8) among the first
  8⌊n/8⌋ (each started from its first element, then added in order), combined as
  ((r_0 + r_1) + (r_2 + r_3)) + ((r_4 + r_5) + (r_6 + r_7)), then the remaining elements added left
  to right; for n > 128, P(first n2) + P(the rest) with n2 = ⌊n/2⌋ rounded down to a multiple of 8.
- A sum along an **outer** axis is a plain sum in index order, from +0.0.
- A mean is that sum divided by the count in **float64** and rounded to the array's type (numpy's
  count is an integer scalar of its own type: the division is not float32).
- Which axis is innermost follows the array's memory layout: the temporal means of §6.2 sum along
  the volumes (innermost) pairwise; with `--exclude`, the array of the used volumes has the volumes
  outermost in memory, so the temporal means are plain sums in volume order (observed by the
  replica's suite: the thresholds' last bits); the RMSE's mean over echoes is a plain sum in echo
  order; the RMSE map sums volumes pairwise; the table's per-volume sums over voxels are plain sums
  in voxel order; the combination's sums over echoes are plain (echo order, the echoes an outer axis)
  except the weights' sum in `--fitmode ts` (echoes innermost: pairwise, plain for < 8 echoes);
  with one volume every sum is pairwise.
- numpy 2.2.6 (fMRIPrep's image) sums arrays of more than 8,192 elements in blocks; 2.3 and later
  in one pass (a difference of the step to tedana 25.0.1 only).
- `larmorx_core::numpy` (`pairwise_sum`, `add_reduce_axis`, `mean_axis`, `std_axis`, `nansum_axis`,
  `nanstd_axis`, `average_axis`) reproduces each of these orders (§13.9).

### 13.3 Elementwise functions

| Function | tedana (`reproduce`) | larmorx's default |
|---|---|---|
| float32 `log` (§7.2) | numpy's own float32 kernel on x86-64 CPUs with AVX2/FMA3 or AVX-512 (not correctly rounded: NP-03; the C library's `logf` on other CPUs); `larmorx_core::math::logf_numpy` equals it on all 2³² inputs | correctly rounded `logf` (`larmorx_core::math`) |
| float64 `exp`, `log` (§7.2, §7.3's model, §8, §9, §10) | glibc 2.35's `exp`/`log` (numpy's path without AVX-512; AVX-512 CPUs use Intel's SVML kernels: NP-04, off in the reference); numpy's `log` of a negative number is +NaN | correctly rounded (`larmorx_core::math`) |
| the curve fit's powers (`a ** 0.5`, `Δ ** 2` on numpy scalars, `x ** 3`) | C's `pow` (glibc 2.35) where numpy calls it; arrays' `** 0.5` and `** 2` are `sqrt` and a product | correctly rounded `pow` |
| `sqrt`, `abs`, `+`, `−`, `×`, `/` | IEEE (correctly rounded) | the same |
| log(eps) in §8 | float64 log of 2⁻⁵² = −36.04365338911715 | the same value |

The maths notices of §15.2 name GLIBC-07 and NP-03 where these choices change a written value.

### 13.4 The least-squares solution (§7.2)

tedana's β of a group is numpy's `lstsq` (LAPACK `dgelsd` with OpenBLAS's kernels) of the m × 2
design and the N right-hand sides at once. Its result is the least-squares β up to rounding, and
its last bits depend on: the batch (about a fifth of the columns solved alone differ in their
last bits; a shift of the batch by 1 to 3 columns changes 2 of 5,003), the BLAS threads
(`--n-threads` 2, 4 or 14 change 4 to 14 of 5,003 voxels' fits), and the LAPACK release (3.12.0
→ 3.12.1 moves a voxel's slope now and then, which T2* = 1/β1 magnifies near β1 = 0: up to 93 %
relative in one noise voxel) (black-box, measured by the replica author). The reference is one BLAS
thread. `larmorx_transform::numpy_linalg::lstsq` reproduces numpy 2.5.4's (and 2.3.5's) result
bit for bit at any thread count (§13.9); a direct per-voxel solution of the 2 × 2 problem (for
instance the closed form of the centred design) gives the same β within a few units in the last
place, which §17.3 allows (measured: the model's per-voxel closed form wrote the same float32
T2* and S0 as tedana on every voxel of §18's cases).

### 13.5 The curve fit (§7.3)

SciPy's trust-region reflective `least_squares` (Branch, Coleman and Li 1999, "A subspace,
interior, and conjugate gradient method for large-scale bound-constrained minimization problems",
SIAM J. Sci. Comput. 21:1–23; SciPy's documentation of `least_squares`) with the options of §7.3.
Its iterates depend on every rounding: SciPy 1.15.3 → 1.18.1 (the SVD factors' memory order,
hence other BLAS kernels) changes the last bits of most fits (fMRIPrep's ds003592 call: 39,211 of
52,826 voxels, median relative 9e-10), the number of evaluations or the status of 186 to 2,168 fits
per call, and a few fits by much more (largest relative difference 0.70 on ds003592, 3.46 on
ds000258), never which fits fail (black-box, measured by the replica author). `larmorx_optim::
curve_fit2` reproduces SciPy 1.18.1's (and 1.15.3's) fit bit for bit (0.2 ms a fit; §13.9). A
faster bounded solver may replace it within §17.3's thresholds.

### 13.6 Percentiles

- numpy's `percentile(a, q, method="higher")` (§6.2): the sorted values' element at
  ⌈(n − 1) · q/100⌉, the product in float64 with q/100 = 0.33 (numpy 2.5.4); numpy 2.3.5 divides q
  by a float32 100 for float32 data (0.33 as float32, the product in float32): the shared
  environment's index can differ for some n.
- numpy's `nanpercentile(a, q, axis=0)` (linear, §9): over the non-NaN values sorted, virtual
  index g = (n − 1) · q/100 (float64), the result a[⌊g⌋] + (a[⌊g⌋+1] − a[⌊g⌋]) · (g − ⌊g⌋) with
  numpy's interpolation formula (the difference in float32, the rest in float64; numpy's `_lerp`,
  which uses the other end when the fraction is ≥ 0.5); all NaN: NaN and the warning.
- SciPy's `scoreatpercentile(a, 99.5, interpolation_method="lower")` (§8): the sorted values'
  element at ⌊0.995 · (N − 1)⌋ (the per in float64: 99.5/100).
- `larmorx_core::numpy::quantile` reproduces all three (§13.9).

### 13.7 Number printing

- The thresholds line (§6.2): numpy's `str` of a float32 array: `[` + the values in numpy's
  shortest unique digits for float32 (Dragon4), padded to a common width with the decimal points
  aligned, separated by blanks, wrapped at 75 characters with a leading blank on continuation
  lines (`[247.7863   142.79353   81.549416]`, `[247.7863  142.79353]`); for one echo numpy's
  `str` of a float32 scalar (`247.7863`; small or large ones in `1e-04` style) (observed).
  `larmorx_core::numpy::format` reproduces it.
- Python lists (§12.2) and the numbers of messages: Python's `repr` of `str`/`int`; numpy float64
  scalars as Python floats (`2.0`, `0.0`, `nan`, `inf`).
- The table and JSON (§11.4, §11.5): Python's shortest round-trip `repr` of float64.

### 13.8 NaN and infinity

NaN compares false (so a NaN mean is not 0, not above a threshold, not ≥ the previous echo; a NaN
T2* is not ≤ 0 or infinite), sorts last, and propagates through sums and products (a NaN sample in
the data reaches the means, the fit, the RMSE and the combination of its voxel; with a fast path
through nilearn's masking it is read as 0: §4.4). +∞ is > 0 (a good sample for §6.1).

### 13.9 What the main package already has (permissive; usable by the original)

Written by the replica's part from numpy's, SciPy's, nilearn's, nibabel's and CPython's sources
(BSD, MIT, PSF), Apache-2.0 with their notices, **not** from tedana: `larmorx_core::numpy`
(reductions, means, deviations, percentiles, the weighted average, numpy's float printing),
`larmorx_core::math` (`logf_numpy`, the glibc functions, the correctly rounded ones),
`larmorx_core::python` (argparse, Python's float and int printing, `json.dump`),
`larmorx_transform::numpy_linalg::lstsq` (numpy's `lstsq` for an m × 2 design, any number of
right-hand sides), `larmorx_optim::curve_fit2` (SciPy's bounded `curve_fit`),
`larmorx_interp::ndimage` and `larmorx_interp::nilearn::compute_epi_mask`, `larmorx_io::nifti`
(nibabel's reading with `Scaling::F32`, its new headers, its best affine), `larmorx_core::defects`
(TED-01 to TED-18, NP-03, GLIBC-07 and the notices' form). Each is verified bit for bit against its
library on tedana's own calls (the replica's records). An original that uses them where this spec
names the library gets tedana's bits there; one that replaces them stays within §17.3.

## 14. larmorx's interfaces

- Command line: `larmorx tedana t2smap <t2smap's options>` (and `lx`), every option of §2 with
  tedana's parser behaviour; `LARMORX_IMPLEMENTATION=auto|replica|original` (`auto` runs the
  original once it exists, the replica before), `LARMORX_UPSTREAM_DEFECTS=fix|reproduce`
  (default `fix`), `LARMORX_TEDANA_LIBRARIES=newest|shared` (whose numpy and SciPy results are
  reproduced where they differ: §13.4–§13.6; `newest` by default). Notices go to standard error as
  `larmorx: upstream defect <ID> (<tool>) corrected: <detail> [LARMORX_UPSTREAM_DEFECTS=reproduce
  gives <tool>'s result]` (`reproduced: ...` in `reproduce`), after tedana's messages, before the
  exit (the replica's are `larmorx-gpl: ...`; `larmorx_core::defects` formats them).
- Python: `lx.tedana.t2smap(echoes, echo_times, *, mask=None, masktype="dropout", fittype="loglin",
  fitmode="all", combmode="t2s", dummy_scans=0, exclude=None, n_independent_echos=None,
  convention="bids", prefix="", verbose=False, debug=False, quiet=False, out_dir=None,
  overwrite=False, libraries="newest", implementation="auto", upstream_defects="fix",
  n_threads=1)` returning `T2smapResult` (the images by tedana's registry keys, the table as
  float64 columns with NaN for `n/a`, the dataset description, the files, the log, the
  implementation) — today's wrapper (`python/larmorx/tedana/`), which gains the original's path
  (`implementation="original"` raises until it exists).
- Threads: `--n-threads` (and `n_threads`; 0 or less: all logical CPUs). The results never depend
  on it (§16).

## 15. Defects (D13): what the default does

Decided by the rules A to D (CLAUDE.md, D13), each with `reproduce` giving tedana's behaviour of
§§2–13. The default names each one with a notice (§14) exactly where its correction changed the
run: an output value, a message, the exit status. The texts in quotes are the replica's default
(observed by `blackbox.py` against `larmorx-gpl tedana t2smap`); the original prints the same,
with `larmorx:` for the replica's `larmorx-gpl:`. Error texts are the last line of the traceback
(§12.4), exit status 1.

### 15.1 tedana's defects

| ID | tedana (`reproduce`) | Rule | Default |
|---|---|---|---|
| TED-01 | a NaN first-echo temporal mean (a NaN sample, or no volume) makes §6.2's percentile NaN: `ValueError: attempt to get argmax of an empty sequence` | A | the percentile and the exemplars from the **finite** non-zero first-echo means only; notice `the dropout thresholds are taken from the voxels whose first-echo mean is finite (a NaN mean made tedana fail)`; none finite: `ValueError: no voxel of the mask has a finite non-zero first-echo mean, so the dropout thresholds cannot be set (tedana fails with: attempt to get argmax of an empty sequence)` |
| TED-02 | an `--exclude` index equal to T passes §5.2's check, then `IndexError: index <T> is out of bounds for axis 0 with size <T>` (observed) | B | `ValueError: The maximum exclude index (<i>) is not below the number of timepoints in the data (<T>).`; notice `--exclude index <i> of <T> volumes` |
| TED-03 | no `--mask`: an empty computed mask (nilearn's MaskWarning) fails in the adaptive mask with `IndexError: index -1 is out of bounds for axis 0 with size 0`; too few voxels for nilearn's threshold, with the argmax ValueError | B | after the MaskWarning, `ValueError: nilearn's compute_epi_mask found no voxels in the first echo; give a mask with --mask (tedana fails later with an unrelated error)`, or `ValueError: the first echo has too few voxels for nilearn's compute_epi_mask to find a threshold; give a mask with --mask (tedana fails later with an unrelated error)` |
| TED-04 | a user mask of the data's shape is applied by voxel index, its affine never compared (observed: a mask shifted by one voxel is used silently) | B | for a 3D mask of the reference's shape whose affine is not nilearn's "close" to the first echo's, nilearn's own error (`ValueError: Mask affine:` + the affine printed by numpy + ` is different from img affine:` + ...), observed; notice `the mask's affine differs from the data's` |
| TED-05 | only the limited T2* is capped (§8); the full map (fMRIPrep's T2* map, the combination's weights) keeps values of up to 239,794 s on fMRIPrep's calls; tedana 25.0.1 capped the full map too | A | the full map gets the limited map's cap (values > 10c set to c) **before** the combination; notice `<k> value(s) of the full T2* map above <10c, 3 decimals> ms (ten times the cap) set to the cap, <c, 3 decimals> ms; tedana 26.0.3 caps only the limited map` |
| TED-06 | a curve fit whose log-linear start lies outside its bounds is a silent failure keeping the start (1,919 of the 1,930 failures on fMRIPrep's four calls) | C | kept as tedana does it; no switch, no notice (refits from a clipped start beat the log-linear residuals in only 15 to 29 % of those voxels) |
| TED-07 | `--verbose` writes S0's variance as the T2* variance map and T2*'s as the S0 map | A | each map its own variance; notice `the T2* and S0 variance maps hold their own variances` where the swap changes a written value outside count-1 voxels |
| TED-08 | count-1 voxels are fitted (group 2) but their failures and variances are not written (0) | A | written; notices `<k> failed fit(s) of voxels with one good echo marked in the failures map` (when k > 0) and, with `--verbose`, `the variances of <k> voxel(s) with one good echo kept in the variance maps` |
| TED-09 | one infinite sample makes every voxel of its group NaN in the log-linear fit (and so in the curve fit's starts) | A | the voxels whose observations are infinite get NaN T2* and S0 alone, the rest of the group is solved without them; notice `<k> voxel(s) with an infinite sample fitted alone (NaN); tedana's <n>-echo fit makes all <N> voxels of that count NaN` (observed) |
| TED-10 | `-e ... inf` passes as milliseconds and fails in the least squares: `numpy.linalg.LinAlgError: SVD did not converge in Linear Least Squares` (observed) | B | at the TE check: `ValueError: TE values must be finite (tedana fails later in numpy's least squares)`; notice `an infinite echo time` (observed) |
| TED-11 | `--n-independent-echos 0`: `ZeroDivisionError: division by zero` (observed) | B | `ValueError: --n-independent-echos must be a positive number of echoes (tedana fails with ZeroDivisionError)`; notice `--n-independent-echos 0` (observed) |
| TED-12 | one echo (one TE) is accepted, then `IndexError: invalid index to scalar variable.` (observed, dropout) or a numpy `column_stack` ValueError (log-linear fit) | B | at the TE check: `ValueError: t2smap needs two or more echoes (one echo time given; tedana fails later with an unrelated numpy error)`; notice `a single echo time` (observed) |
| TED-13 | `--dummy-scans` ≥ T: numpy's "Mean of empty slice" warnings, then the argmax ValueError (observed); a negative count shifts `--exclude`'s indices up | B | `ValueError: --dummy-scans must be at least 0 and below the number of volumes (<T>); <N> given`; notice `--dummy-scans <N> of <T> volumes` (observed) |
| TED-14 | `--debug`'s floor message divides by E · V | A | the message's count and percentage of V voxels (a message only); notice `the floor's message counts <V> voxels; tedana prints <E·V>` |
| TED-15 | PAID with a voxel whose good echoes do not vary over time (every voxel of a one-volume series): `ZeroDivisionError: Weights sum to zero, can't be normalized` | A (B for one volume) | those voxels combined with equal weights (notice `<k> voxel(s) whose good echoes do not vary over time combined with equal weights; tedana fails with ZeroDivisionError`); one volume: `ValueError: --combmode paid needs more than one volume: its weights are each echo's temporal signal-to-noise ratio (tedana fails with ZeroDivisionError)` |
| TED-16 | `--exclude` removing every volume: the adaptive mask and the fit see no volume; tedana writes T2* = 500 ms and S0 = 1 in every voxel with exit 0 (the dropout method fails first with the argmax ValueError: observed) | B | after the exclusion line: `ValueError: --exclude leaves no volume for the adaptive mask and the T2*/S0 fit (all <T> volumes excluded)`; notice `--exclude removes all <T> volumes from the adaptive mask and the fit` (observed) |
| TED-17 | a single fit voxel: `IndexError: too many indices for array: array is 0-dimensional, but 1 were indexed` (both fit types and modes) | A | fitted as any other voxel; notice `a single voxel is left for the fit` |
| TED-18 | a cap c ≤ 0 (more than 99.5 % of the limited values 0): every positive limited T2* set to c (then the RMSE divides by 0) | A | no cap of 0 or less, on either map (TED-05's not applied then); notice `the limited T2* map's cap is <c, 3 decimals> ms (its 99.5th percentile), so tedana sets its <k> value(s) above it to it` |

### 15.2 The maths (NP-03, NP-04, GLIBC-07)

By default every elementary function is correctly rounded (§13.3); `reproduce` uses tedana's:
numpy's float32 log kernel (NP-03) and glibc 2.35's float64 `exp`, `log` and `pow` (GLIBC-07).
NP-04 (numpy's AVX-512 SVML kernels) is a register row only: the reference setup turns them off,
and larmorx never reproduces them.

The two implementations name NP-03 and GLIBC-07 by different rules (decided 2026-11-08, by the
precedent of the user's decision of 2026-11-04 on GLIBC-07 in the FSL clean-room originals and on
GLIBC-06, where an exact notice that made BET 1.9 times slower was declined):

- **The replica** names them exactly where the other choice changes a written value or a printed
  line, in both modes: it re-runs the stages from the mask to the combination with the other
  functions, writing nothing (up to four passes: §16). Observed (the probe's `loglin`):
  `larmorx-gpl: upstream defect NP-03 (numpy) corrected: the log-linear fit takes the correctly
  rounded float32 logarithm; numpy's own kernel (tedana's) would give 1164 other written value(s)
  (S0map.nii.gz 175, desc-rmse_statmap.nii.gz 224, ...) [LARMORX_UPSTREAM_DEFECTS=reproduce gives
  numpy's result]`.
- **The original** makes no extra pass. In its one pass it names a function only where that
  function's **own value is written** as an output value, and exactly there: where the other
  function's value of the same argument, rounded to the output's type, differs from the value
  written. Where a function's value only feeds later arithmetic (the log-linear observations, the
  floors' test, the RMSE's predictions, the combination's weights, sums) or the iterative curve
  fit, the original names nothing, as FSL's iterative programs and filters do. On t2smap's path
  this leaves:
  - **NP-03: never named** by the original (the float32 logarithm's values are only the log-linear
    fit's observations, §7.2);
  - **GLIBC-07: the log-linear S0 values exp(β0)** (§7.2) where they are written: the full and
    limited S0 maps of a `loglin` run, and of a `curvefit` run's voxels whose curve fit failed
    (they keep the log-linear S0, §7.3), per volume in `--fitmode ts` (count-1 voxels' limited
    S0 is 0, §8). Named when at least one such written value would differ, with the count per
    file in the order of writing: by default `larmorx: upstream defect GLIBC-07 (glibc 2.35)
    corrected: the log-linear S0 = exp(b0) is written with the correctly rounded exp; glibc
    2.35's (tedana's) would write <k> other value(s) (S0map.nii.gz <k1>, desc-limited_S0map.nii.gz
    <k2>) [LARMORX_UPSTREAM_DEFECTS=reproduce gives glibc 2.35's result]`; in `reproduce`
    `... reproduced: the log-linear S0 = exp(b0) is written with glibc 2.35's exp, as numpy
    computes it for tedana; the correctly rounded exp would write <k> other value(s) (...)`.
  The cost is one more exponential per written log-linear S0 value, compared after rounding to
  float32.

### 15.3 Kept as tedana's, not defects

- `--debug`'s help promises a TSV log file in the output directory; none is written (rule C,
  documented).
- The dropout method counted twice in the minimum; tedana's report lines (its "REPORT" log) go
  nowhere; the warnings and bars naming paths and timings: tedana's design, no ID.
- tedana's log-linear maps depend on its `--n-threads` (§13.4): larmorx gives the one-thread
  result at any thread count (not a switch: the reference is one thread).

## 16. Built for speed: what must stay, what may change

**Must stay** (it decides the results): the command line and its errors (§2); the order of §3
with its side effects (the directory, the call file, the adaptive mask written before the fit,
the existence checks, what a failure leaves); the readers' values and the masks (§4); the volumes
(§5); the adaptive mask, exactly (§6: its means in numpy's order, cheap and decisive: one voxel's
count changes its fit); the groups, the observations, the bounds, the start, the failure rules
and the covariance's definition (§7); the floors, the limited maps and the cap (§8); the RMSE's
and the table's definitions (§9); the combination's weights and its zero-sum rule (§10); every
output's name, type, header, layout and text format (§11); the messages (§12); the defects'
behaviour (§15).

**May change** (how it is computed), as long as the result is deterministic and independent of the
thread count, within §17.3:
- **The log-linear fit**: a per-voxel solution of the 2-parameter least squares (sums of y and
  TE · y per voxel against the design's fixed moments, or an orthogonal factorisation), in any
  voxel order or in parallel, instead of the batched SVD; the float32 logarithm must stay the
  function of §13.3 (it is elementwise and decides whole ulps of y).
- **The curve fit**: the dominant cost (tedana 29 to 46 s on fMRIPrep's calls at one thread,
  0.2 ms a fit with `larmorx_optim::curve_fit2`); voxels are independent: parallel over voxels in
  fixed chunks; the model's exponentials once per distinct echo time and voxel; another bounded
  solver for the same minimum (e.g. the same trust-region method with analytic derivatives) where
  §17.3's objective criterion holds; the covariance from the final Jacobian as §7.3 defines it.
- **Sums**: other orders where §17.3 allows (the RMSE, the table, the combination); the means of
  §6 must stay numpy's.
- **Streaming and memory**: tedana holds every echo's whole series, the masked array and its
  copies: 1,243 MiB at its peak on fMRIPrep's ds003592 call (64 × 64 × 43 × 200, three echoes;
  1,224 MiB with `loglin`); the replica 545 MiB (543). The original stays at or under tedana's
  peak plus 10 % and 8 MiB (CLAUDE.md, memory) and should do better: echoes read one at a time
  into the masked float32 array (S · E · T · 4 bytes: 127 MB there), the combination written from
  float64 per voxel without a full float64 copy of the series.
- **Passes**: one pass of the work, the maths notices included (§15.2's one-pass rule); the
  replica's extra passes for exact maths notices (31.9 s against 11 s on ds003592) are the
  replica's role.
- **Speed today** (`docs/benchmarks/tedana-t2smap.md`, a shared machine): tedana 26.0.3 at one
  thread 29.2 / 46.2 / 32.2 / 44.3 s on fMRIPrep's four curvefit calls, 6.1 s on ds003592's loglin;
  the replica 31.0 / 44.7 / 42.2 / 42.1 s at one thread, 8.9 / 13.3 / 10.8 / 10.6 s at 14. The
  original's aim: several times tedana's speed at one thread on the curve-fit calls, near-linear
  scaling with threads.

## 17. Validation plan

### 17.1 The reference setup

tedana 26.0.3 as a program (the launchers of the header, run through the oracle-output cache,
`oracle_cache.run`, whose key family `python` covers these variables: the original's suite is its
own), in the newest environment (numpy 2.5.4,
SciPy 1.18.1), with `OPENBLAS_CORETYPE=Haswell`, `NPY_DISABLE_CPU_FEATURES="X86_V4 AVX512F
AVX512CD AVX512_SKX AVX512_CLX AVX512_CNL AVX512_ICL AVX512_SPR"` (numpy's AVX-512 kernels off:
NP-04), `OPENBLAS_NUM_THREADS=1`, `COLUMNS=80`, `TQDM_DISABLE=1`; the shared environment (numpy
2.3.5, SciPy 1.15.3) as the second, with `LARMORX_TEDANA_LIBRARIES=shared`. Two comparisons:
(A) the original with `reproduce` against tedana, in both environments; (B) the original's default
against the replica's default (`larmorx-gpl tedana t2smap`), newest environment.

### 17.2 The cases

- `blackbox.py`'s 54 cases (§18): every option, error and message path on a 16 × 14 × 8 × 6
  three-echo phantom with dropout and rising voxels, its int16, z-concatenated, NaN, infinite and
  3D variants and a shifted mask.
- fMRIPrep's four multi-echo calls with `curvefit` (ds003592 sub-280; ds000210 sub-02 cuedSGT and
  rest; ds000258 sub-21262, four echoes, big-endian, oblique) and ds003592's with `loglin`: their
  inputs are kept in the oracle runs' working directories (`oracles/fmriprep-25.2.6/<run>/work/
  .../bold_t2smap_wf/t2smap_node`, the echoes `_echoidx_*/.../*_xformresampled.nii.gz` and
  `dilate_mask/dilated_mask.nii.gz`), with fMRIPrep's own `T2starmap` and `desc-optcom_bold`
  (tedana 25.0.1) reported against tedana 26.0.3's and the original's (TED-05 and the version
  step); ds003592's also with `--verbose` (the variance maps on real data: fMRIPrep never asks
  for them).
- The edge cases of every defect of §15 (both modes), two and many echoes (nine), one volume,
  `--fitmode ts` with both fits and combinations, every input data type (float32, float64, int16
  scaled, uint8, uint16), both byte orders, pairs and NIfTI-2, no mask (NIfTI-1 and -2), a 4D mask
  of one volume, oblique and qform-only headers (§4.5).
- Smart testing (the user, 2026-11-08): every path once, fMRIPrep's real calls, the defects'
  edges; the extensive panel and long random testing in the final validation phase.

### 17.3 Thresholds

(spec writer, 2026-11-08, relay 8b: measured with the model of §18; `check.py` implements this
table.) Each allowance's **use** is the largest |difference| / allowance of a run; the numbers in
brackets are the largest uses measured on fMRIPrep's five calls (four `curvefit`, ds003592's
`loglin`, ds003592's with `--verbose`) and `blackbox.py`'s 30 completed cases, first for tedana's
own spread (tedana 26.0.3 in the shared environment, with `--n-threads 4` and the BLAS threads
free, with numpy's AVX-512 kernels on, each against the reference setup), then for the model's
`fast` variant (§18: the kind of change §16 allows). T is the volumes, E the echoes, S0 and T2* the
written values (T2* in seconds; ×1000 for the predictions and weights, in ms).

| Quantity | Requirement [measured use: tedana's spread / fast variant] |
|---|---|
| exit status, the last error line | identical |
| log lines (§12.2) | identical text, except: paths (the run's own), the cap's DEBUG value and the curve fit's DEBUG failure counts where §7's values differ within this table (reported) |
| Python warnings (§12.3) | the same categories and messages at the same points of the log; their location lines may differ |
| progress bars | none when `TQDM_DISABLE` disables them (§12.3); otherwise not compared |
| `t2smap_call.sh`, file names, the registry, the table's header and row count, JSON keys and every JSON value except `Node` and D6's `Python`/`Python_Libraries` | identical [0 / 0] |
| image headers, types, dimensions, affines | identical [0 / 0] |
| the adaptive mask | identical (its means follow §13.2 exactly) [0 / 0] |
| `loglin` full and limited T2* and S0 | per voxel identical, or: in the rate r = 1/T2* (s⁻¹), \|r − r'\| ≤ 2⁻²²·\|r\| + 1e-9 s⁻¹, and \|S0 − S0'\| ≤ 2⁻²²·\|S0\| (two float32 units in the last place); a value floored (§8: 1 ms, 500 ms or the underflow floor) in one run only is accepted when one run's rate is within 1e-9 s⁻¹ of 0 (β1's sign) or of the floor's rate [identical on every voxel: tedana's spread, and the fast variant's per-voxel closed-form solution, whose float64 differences vanish in the float32 values written] |
| `curvefit` T2* and S0 | per voxel (and volume in `ts`): (a) \|r − r'\| ≤ 1e-6·\|r\| + 1e-9 s⁻¹ and \|S0 − S0'\| ≤ 1e-6·\|S0\|; or (b) the other run's fit is at least as good: F' ≤ F·(1 + 1e-6) + 1e-12·Σy², F = Σ (S0·exp(−TE/T2*) − y)² in float64 from the written full maps over the voxel's samples of the fit (echoes 1..n of its group, the used volumes); or (c) its log-linear start lies near the feasibility boundary (a start's rate within 1e-9 s⁻¹ of 0, or its S0 within 1e-6 relative of the voxel's smallest sample). Voxels passing by (b) or (c): at most **3 %** of the fit voxels [(a)'s use 0.997 / 0.995: the rest pass by (b); (b) or (c) for 0.28 % / 0.31 % of the fit voxels at most, all by (b), with F'/F − 1 between −1.9e-8 and 3.2e-8] |
| the cap (limited T2*) | the cap is an order statistic: \|c − c'\| ≤ max \|ℓ − ℓ'\| over the voxels capped in neither run (count ≥ 2) + tol·c (tol the T2* row's relative tolerance); a voxel capped in one run only is accepted when the other run's value is within 10·\|c − c'\| + 10·tol·c of ten caps, or passes by (b)/(c) [0.005 / 0.005] |
| the failures map | identical, except voxels passing by (b) or (c) (reported) [identical / identical] |
| variance and covariance maps | where (a) holds: relative 1e-4; +∞ identical; elsewhere reported [0.08 / 0.08: at most 8.1e-6 relative, on ds003592 with `--verbose`] |
| the RMSE map | per voxel \|Δ\| ≤ max_t rms_e(p_e − p'_e) + 2⁻²¹·max_e\|p_e\| + 2⁻²⁴·(T + E + 64)·RMSE, p and p' the predictions of §9 (float32) from the two runs' written limited maps; the last term is the error of a float32 sum of the volumes and the echoes in another order (the standard bound n·2⁻²⁴ of the terms' sum, with a slack for numpy's pairwise blocks); NaN where tedana has NaN [0.43 / 0.14] |
| the confounds table | per volume t: `rmse_mean` and `rmse_std` \|Δ\| ≤ A_t + 2⁻²⁴·(N_t + 32)·max(\|value\|, \|rmse_mean\|); the percentile columns \|Δ\| ≤ A_t + 1e-6·\|value\| (order statistics move by at most the largest change of the values); A_t the largest over the voxels of rmse[v, t]'s allowance rms_e(p_e − p'_e) + 2⁻²¹·max_e\|p_e\| + 2⁻²⁴·(E + 32)·rmse[v, t], N_t the non-NaN values; `n/a` identical; the float32/float64 form of §9 identical [0.07 / 0.02] |
| the combined series | per voxel and volume \|Δ\| ≤ Σ_e \|x_e\|·\|ŵ_e − ŵ'_e\| + 2⁻²⁰·(\|∂c/∂ln T2*\| + \|c\|) + 2⁻¹⁴⁹, ŵ the normalised weights of §10 from each run's written full T2* (ms), ∂c/∂ln T2* = Σ_e ŵ_e x_e (TE_e − Σ_k ŵ_k TE_k) / T2*: each run's own T2* lies within half a float32 unit of its written value, and eight units are allowed; PAID: identical weights expected, so the 2⁻²⁰ term [0.85 / 0.44: the first term is a first-order bound, sharp by design] |
| the notices (B) | the same TED IDs named in the same runs, their texts identical except numbers that count values of this table (reported); NP-03 never named by the original; GLIBC-07 by §15.2's one-pass rule; the replica's NP-03 and GLIBC-07 notices (the exact rule) are not compared |
| memory | peak RSS ≤ tedana's · 1.1 + 8 MiB on fMRIPrep's ds003592 call (`tests/parity/test_memory_peak.py`) |

Bit-identity per output is reported, not required. **tedana's own spread** in values written
(the record `tedana-t2smap-replica`, of 35.6 million written values of fMRIPrep's ds003592 call; a
synthetic curve fit; a synthetic log-linear fit): numpy's AVX-512 kernels on: 22,899 / 8 / 0;
`--n-threads 4`: 1 / 0 / 0; the shared environment: 28,808 / 5 / 0; tedana 25.0.1 in fMRIPrep's
image: 176,938 / 447 / 1,646. fMRIPrep's own files against tedana 26.0.3: 12,295 (ds000210
cuedSGT), 12,565 (rest), 35,226 (ds000258), 41,897 (ds003592) values; against larmorx's default
12,117 / 12,602 / 34,918 / 42,180 (the default caps the full map, as 25.0.1 did). Every row above
holds for that spread (relay 8b, `measure.py`: every run of tedana's spread and of the fast
variant passes), and every number is at least the measured largest: about ten times it where the
allowance is a tolerance (the curve fit's objective, 30 times; the (b)/(c) share, 10 times; the
variances, 12 times), and the derived bounds (the cap, the RMSE, the table, the combination) are
error bounds of the computation itself, whose uses stay below 1 by construction.

### 17.4 Random testing, golden cases, memory, speed

- A difftest descriptor for the original (its own, written from this spec: multi-echo series of
  two to six echoes, 1 to 12 volumes, the data types and file forms of §17.2, special samples,
  every option), with tedana as the oracle and §17.3 as the checks, one short trial (0.5 h) in the
  part (smart testing); the replica's descriptor is not to be read.
- Golden cases (rule 10): each fit type and mode, both combinations and conventions, the variance
  maps, no mask, the mask methods, dummy scans and an excluded volume, float32 and int16, a
  z-concatenated file, both modes, a failure; inputs generated with plain arithmetic (the decay as
  a product of per-millisecond factors, as `blackbox.py` makes it).
- Memory as in §17.3; speed against tedana and the replica on fMRIPrep's four calls at 1 and 14
  threads (`docs/benchmarks/`).

## 18. The model (`validation/investigations/t2smap-spec/`)

Written from this spec and black-box runs only (a clean-room implementer may read and run them;
the folder's README says how):

- `blackbox.py` (relay 8a): 54 cases of a `t2smap` program on inputs it generates (seeded numpy;
  the decay as a product of per-millisecond factors), the report of every case's exit status,
  standard output and error (paths and the stack normalised) and every file's header fields and
  value summary; the source of this spec's **(observed)** facts. Run against tedana 26.0.3 (the
  newest launcher) and against `larmorx-gpl tedana t2smap` in both modes (2026-11-08): the
  replica's `reproduce` report equals tedana's except the warnings' location lines (D6), larmorx's
  notices, the progress bars (the replica printed them whatever `TQDM_DISABLE` said; corrected in
  relay 8b: it now honours §12.3's rule, observed with `1`, `0` and an empty value) and gzip's
  header byte for the compression level.
- `model.py` (relay 8b): §§3–11 in numpy and SciPy as this spec states them, with the orders of
  §13.2's sums and §13.6's percentile rules written out rather than left to numpy: nibabel's
  float32 reading, the user mask or nilearn's `compute_epi_mask`, the dummy scans and
  `--exclude`, the adaptive mask, the groups, numpy's `lstsq` per group, SciPy's `curve_fit` per
  voxel with §7.3's options passed explicitly, the floors, the limited maps, the cap, the RMSE,
  the table (written with Python's `repr`), the combination, the images (nibabel's new image from
  the mask's affine), the JSON files. Its scope: NIfTI-1 single files, a mask of the data's grid
  or none; tedana's behaviour (`reproduce`); the files (the messages are `blackbox.py`'s). Its
  `fast` variant makes the changes §16 allows: the log-linear fit per voxel in closed form (a
  centred 2 × 2 least squares), the curve fit with an analytic Jacobian (another path of the same
  trust-region method), the RMSE's sums in float64 with the echoes reversed and a plain sum over
  the volumes, the table's mean pairwise, the combination from normalised weights summed in
  reverse.
  **Result** (2026-11-08, the reference setup, `oracles/python-newest`): the model's files are
  **bit-identical** to tedana 26.0.3's (every image's header and data, the table's text, the
  registry, the call file, the JSON files but for D6's fields) on all 30 of `blackbox.py`'s cases
  that tedana completes (both fits and modes, both combinations and conventions, the variance
  maps, no mask, every mask method, dummy scans, `--exclude` with and without dummy scans, int16
  with a scale factor, a z-concatenated file, a NaN and an infinite sample, two echoes, a shifted
  mask, abbreviated options) and on fMRIPrep's four curve-fit calls, ds003592's log-linear call and
  ds003592's with `--verbose` (52,826 curve fits and their variances on ds003592 alone). Two
  statements were made precise while it was written, from tedana's behaviour as the spec writer
  analysed it: §6.2's choice between several exemplars sums their thirds, not their means, and
  §9's per-voxel sums are numpy's `nansum` (NaN as +0 in place, then the pairwise sum); neither
  changes a case above (no tie between exemplars ranks differently there, and no voxel mixes NaN
  and numbers over eight or more volumes).
- `check.py` (relay 8b): §17.3 between two output directories (the command read again from
  `t2smap_call.sh`, the inputs read again with the model's readers): per output the values
  compared, the identical ones, those passing by each rule, the failures, the largest use, and
  with `--details` the first failures.
- `measure.py` (relay 8b): the runs behind §17.3: per case tedana in the reference setup, in the
  shared environment, with `--n-threads 4`, with numpy's AVX-512 kernels on, the model and its
  `fast` variant, each checked against the reference by `check.py`, and the table of the largest
  uses (§17.3's brackets). 2026-11-08: every run passes; the model bit-identical everywhere.

## 19. How this text was checked, and open points

(spec writer, 2026-11-08, relays 8a and 8b) Written from tedana 26.0.3's source and the replica's
findings, then checked against `blackbox.py`'s 54 runs of tedana's program (every **(observed)**
statement) and of the replica in both modes, and against the model of §18: §§3–11 as written give
tedana's files bit for bit on every case of §18, and §17.3's thresholds hold, measured, for
tedana's own spread and for the kind of implementation §16 allows.

Open points:
1. (Decided 2026-11-08, §15.2.) The original's maths notices: the one-pass rule (NP-03 never
   named; GLIBC-07 where a written log-linear S0 would differ).
2. (Done, relay 8b.) The thresholds of §17.3, measured.
3. fMRIPrep's image (tedana 25.0.1, numpy 2.2.6's blocked sums) is a reference of the final
   comparisons only; no "25" mode.
4. What the model does not cover, for the original's own suite (§17.2): inputs other than NIfTI-1
   single files and nilearn's masking path (§4.4); the defects' corrections (larmorx's default,
   §15); the messages (`blackbox.py`'s report covers them); eight or more echoes (numpy's pairwise
   sums along the echoes, §13.2); a 4D mask of one volume.
