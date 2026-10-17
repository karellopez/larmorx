# antsAI

`lx.ants.ai` (command line `larmorx ants antsAI`; Rust `larmorx_ants::ai` and the crate
`larmorx-registration`). A replica of ANTs 2.6.5's `antsAI` on ITK 5.4.5 (Apache-2.0): the
linear transform that best aligns two images, from an initial alignment (centres of mass,
principal axes, or the images' geometric centres or centres of mass alone) and a multi-start
search over rotations about the principal axes and translations on a grid, each start point
refined by a few conjugate-gradient iterations, with the `Mattes`, `GC` and `MI` metrics,
`None`/`Regular`/`Random` sampling and masks, in 2D and 3D. fMRIPrep runs it in sMRIPrep's
brain extraction and in the BOLD reference's skull-stripping (`init_aff`). Not implemented
yet: blob alignment (`-b n` with `n ≥ 3`), refused with a message.

**Status: `validated`** against `antsAI` itself (the
[validation record](../validation/ants-ai.md), standard tier): every compared case identical to
ANTs 2.6.5 at one thread (exit status, standard output with every printed metric value,
standard error, the transform file byte for byte), most against the unmodified binary, the
rest once glibc's not-correctly-rounded `cbrt` is replaced by CORE-MATH's (CLAUDE.md rule 5).
fMRIPrep 25.2.6's own `init_aff` command lines, with fMRIPrep's seed, give fMRIPrep's
transforms within ANTs' 8-thread band (about 1e-13). The counts are in the record.

**Results do not depend on threads.** ANTs' do, by about 1e-13 in a parameter (its metrics sum
per work unit); larmorx always gives ANTs' one-thread result. **`Random` sampling is never
repeatable in ANTs** (its voxels are drawn from the clock, whatever the seed; so are `Regular`
sampling's perturbations with `--random-seed 0`, ANTS-58); larmorx seeds them from the seed.

## Quick start

```python
import larmorx as lx

res = lx.ants.ai(
    "template.nii.gz", "t1w.nii.gz",
    transform="Affine", gradient_step=0.1,
    metric="Mattes", bins=32, sampling="Regular", sampling_percentage=0.25,
    search_factor=15, arc_fraction=0.1, translation_search_grid=(40, (0, 40, 40)),
    iterations=10, convergence_threshold=1e-6, convergence_window=10,
    fixed_mask="template_mask.nii.gz", random_seed=50495,
)
res.transform            # an ItkTransform (AffineTransform_double_3_3), fixed -> moving
res.start_points, res.best_value, res.values[:3]
```

```bash
larmorx ants antsAI -d 3 -m Mattes[template.nii.gz,t1w.nii.gz,32,Regular,0.25] \
    -x template_mask.nii.gz -t Affine[0.1] -s [15,0.1] -g [40,0x40x40] -c [10,1e-6,10] \
    -o initialization.mat -v 1
```

## Option mapping

| antsAI | Python | Notes |
|---|---|---|
| `-d 2/3` | `dimension` | default: the fixed image's |
| `-m Mattes[f,m,bins,sampling,p]` | `fixed`, `moving`, `metric="Mattes"`, `bins`, `sampling`, `sampling_percentage` | `GC[f,m,radius,...]` (the third parameter is read but unused), `MI[f,m,bins,...]` (joint-histogram MI, smoothing variance 1); the images are read from `-m` whatever the metric |
| `-t Rigid[step]` | `transform="Rigid"`, `gradient_step=step` | `Affine`, `Similarity`; `AlignGeometricCenters`, `AlignCentersOfMass` (no step) |
| `-p 0/1` | `align_principal_axes` | start from the principal axes' rotation (Wahba's problem) |
| `-b n` / `-b [n,m]` | – | blob alignment: not implemented yet (refused); fewer than 3 blobs: ANTs' error |
| `-s [factor,arc]` / `-s factor` | `search_factor` (degrees), `arc_fraction` | angles `−180°·arc` to `180°·arc` in steps of `factor`, about each axis |
| `-g [step,AxBxC]` | `translation_search_grid=(step, (A, B, C))` | translations of the start points, in mm (ignored by `Rigid`/`Similarity` in 2D and `Similarity` in 3D in ANTs: ANTS-52) |
| `-c [n,threshold,window]` / `-c n` | `iterations`, `convergence_threshold`, `convergence_window` | defaults 0, 1e-6, **5** (the help says 10: ANTS-59); 0 iterations: the initial alignment only |
| `-x mask` / `-x [fixed,moving]` | `fixed_mask`, `moving_mask` | a mask that cannot be read is no mask |
| `-o out.mat` | `AIResult.transform` | `.mat`, `.txt`, `.tfm` or an HDF5 name; Python reads the file back |
| `--random-seed N` / `ANTS_RANDOM_SEED` | `random_seed` | default 1234; 0: ANTs from the clock, larmorx 121212 |
| `-v 1` | `AIResult.log`, `.start_points`, `.values`, `.best_value` | Python always runs verbose; errors are explained with or without `-v` (ANTS-45) |
| `-h`, `--help` | – | the short and long menus, as ANTs prints them |
| `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`, `NSLOTS` | `n_threads` | the result does not depend on it |
| `LARMORX_UPSTREAM_DEFECTS` | `upstream_defects=` | ANTS-52, -53, -55, ITK-18, -19 ([upstream defects](upstream-defects.md)) |

## The output transform

The class `antsAI` writes: `Euler2DTransform` / `Euler3DTransform` for `Rigid` and the two
alignments (the alignments as a rigid transform without rotation, "to be consistent with
antsRegistration"), `AffineTransform`, `Similarity2DTransform` / `Similarity3DTransform`,
centred at the fixed image's centre of mass (geometric centre for `AlignGeometricCenters`).
It maps fixed-space points to the moving image, as `antsApplyTransforms -t` and
`antsRegistration -r` take it.

## Upstream defects (D13)

Corrected by default, reproduced with `upstream_defects="reproduce"`
(`LARMORX_UPSTREAM_DEFECTS=reproduce`):

- **ANTS-52**: in ANTs the translation grid does not move `Rigid` and `Similarity` start points
  in 2D, nor `Similarity` ones in 3D (every translation repeats the same start point);
  corrected, they are translated as the 3D rigid ones are (named when the best start point is
  a translated one).
- **ANTS-53**: `AlignGeometricCenters` / `AlignCentersOfMass` with iterations run a search that
  lists no start point and write an empty `.mat` (a `.txt` or `.h5` output crashes); corrected,
  the alignment is written as with 0 iterations.
- **ANTS-55**: a translation grid with fewer extents than dimensions is read past the end of its
  vector; corrected, an error; reproduced, the missing extents read as 0.
- **ITK-18** (the `MI` metric's swapped marginals) and **ITK-19** (a start point that throws keeps
  the previous start point's value): as in `antsRegistration`; corrected, start points that throw
  take no part in the search for the best.

Never reproduced (class (a)): where ANTs crashes or never finishes, larmorx refuses with a
message (exit 1): an empty start list with `-o` (a negative arc fraction or grid extent), a
search factor or translation step that does not advance (ANTs' loop never ends), the Mattes
metric after its joint histogram's spacing is refused (ITK-20), the alignments with iterations
into a text or HDF5 file. Where ANTs aborts on an exception (a missing or zero image, a
conversion the parser refuses, a sample set that is empty, an output name no writer takes),
larmorx prints what ANTs prints and exits 1.

## Not supported yet

- `-b n` (blob alignment: `MultiScaleLaplacianBlobDetectorImageFilter` and
  `LandmarkBasedTransformInitializer`): refused with a message.
