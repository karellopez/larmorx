# antsAI

`lx.ants.ai` (command line `larmorx ants antsAI`; Rust `larmorx_ants::ai` and the crate
`larmorx-registration`). A replica of ANTs 2.6.5's `antsAI` on ITK 5.4.5 (Apache-2.0): the
linear transform that best aligns two images, from an initial alignment (centres of mass,
principal axes, or the images' geometric centres or centres of mass alone) and a multi-start
search over rotations about the principal axes and translations on a grid, each start point
refined by a few conjugate-gradient iterations, with the `Mattes`, `GC` and `MI` metrics,
`None`/`Regular`/`Random` sampling and masks, in 2D and 3D, and the blob alignment (`-b`:
ANTs' multi-scale Laplacian-of-Gaussian blob detector, patch correlations, landmark fits;
[below](#blob-alignment)). fMRIPrep runs it in sMRIPrep's brain extraction and in the BOLD
reference's skull-stripping (`init_aff`), without blobs.

**Status: `validated`** against `antsAI` itself (the
[validation record](../validation/ants-ai.md), standard tier): every compared case identical to
ANTs 2.6.5 at one thread (exit status, standard output with every printed metric value,
standard error, the transform file byte for byte), most against the unmodified binary, the
rest once glibc's not-correctly-rounded `cbrt` is replaced by CORE-MATH's (CLAUDE.md rule 5).
fMRIPrep 25.2.6's own `init_aff` command lines, with fMRIPrep's seed, give fMRIPrep's
transforms within ANTs' 8-thread band: all ten `init_aff` nodes of the five oracle runs (full tier) within 1.5e-9 of fMRIPrep's files, nine of them within 1.3e-12.

| Tier | Cases | Identical to the unmodified ANTs | Identical with CORE-MATH | Rejected by both | Expected divergences | Failures |
|---|---|---|---|---|---|---|
| standard | 102 | 51 | 1 | 46 | 4 | 0 |
| full ([record](../validation/ants-ai-full.md)) | 110 | 59 | 1 | 46 | 4 | 0 |

"Rejected by both" includes the cases where ANTs aborts or crashes: everything ANTs printed
before it stopped is identical, and larmorx then exits 1 with one more line. The expected
divergences: two loops ANTs never finishes (larmorx refuses) and two clock-seeded runs
(ANTS-58). **ANTs' own thread band** (6 cases): up to 2e-13 in a transform parameter; larmorx
gives the one-thread result at any thread count.

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
| `-b n` / `-b [n,m]` | `blobs=n` / `blobs=(n, m)` | the initial transform from `n` blobs per image and up to `m` matched pairs (`m = n` by default); fewer than 3 blobs: ANTs' error; `-p` is then ignored ([blob alignment](#blob-alignment)) |
| `-s [factor,arc]` / `-s factor` | `search_factor` (degrees), `arc_fraction` | angles `−180°·arc` to `180°·arc` in steps of `factor`, about each axis |
| `-g [step,AxBxC]` | `translation_search_grid=(step, (A, B, C))` | translations of the start points, in mm (ignored by `Rigid`/`Similarity` in 2D and `Similarity` in 3D in ANTs: ANTS-52) |
| `-c [n,threshold,window]` / `-c n` | `iterations`, `convergence_threshold`, `convergence_window` | defaults 0, 1e-6, **5** (the help says 10: ANTS-59); 0 iterations: the initial alignment only |
| `-x mask` / `-x [fixed,moving]` | `fixed_mask`, `moving_mask` | a mask that cannot be read is no mask |
| `-o out.mat` | `AIResult.transform` | `.mat`, `.txt`, `.tfm` or an HDF5 name; Python reads the file back |
| `--random-seed N` / `ANTS_RANDOM_SEED` | `random_seed` | default 1234; 0: ANTs from the clock, larmorx 121212 |
| `-v 1` | `AIResult.log`, `.start_points`, `.values`, `.best_value` | Python always runs verbose; errors are explained with or without `-v` (ANTS-45) |
| `-h`, `--help` | – | the short and long menus, as ANTs prints them |
| `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`, `NSLOTS` | `n_threads` | the result does not depend on it |
| `LARMORX_UPSTREAM_DEFECTS` | `upstream_defects=` | ANTS-52, -53, -55, -110, -111, -112, -114, ITK-18, -19, -60 ([upstream defects](upstream-defects.md)) |

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
- **ANTS-112** (`-b`): ANTs' blob detector does not keep the `n` strongest maxima (its threshold
  is set from a list that is not full yet, and a heap that has just filled lets a weaker maximum
  in); corrected, the `n` strongest.
- **ANTS-111** (`-b`): once no positive patch correlation is left, ANTs pairs the first fixed and
  the first moving blob again at every step; corrected, the pairing stops.
- **ANTS-110** (`-b`): ANTs keeps the pairs whose displacements have about the same length as
  three others', not those whose distances to three others are preserved; corrected, the
  distances (as the code's comment and ImageMath's `MatchBlobs` say).
- **ANTS-114** (`-b`, `Rigid`/`Similarity`): without a landmark ANTs writes a transform of NaNs;
  corrected, an error.
- **ITK-60** (`-b`, 2D `Rigid`/`Similarity`): ITK's landmark fit takes −90° whenever the summed dot
  product is within 5e-5 of 0; corrected, `atan2` there too.

Never reproduced (class (a)): where ANTs crashes or never finishes, larmorx refuses with a
message (exit 1): `-b` with a blob list that is empty (ANTs dereferences a null transform,
ANTS-115), an empty start list with `-o` (a negative arc fraction or grid extent), a
search factor or translation step that does not advance (ANTs' loop never ends), the Mattes
metric after its joint histogram's spacing is refused (ITK-20), the alignments with iterations
into a text or HDF5 file. Where ANTs aborts on an exception (a missing or zero image, a
conversion the parser refuses, a sample set that is empty, an output name no writer takes),
larmorx prints what ANTs prints and exits 1.

## Blob alignment

`-b n` (or `blobs=n` in Python) starts the search from a transform fitted to matched blobs
instead of the centres of mass:

1. up to `n` blobs per image from ANTs' `MultiScaleLaplacianBlobDetectorImageFilter` (the
   maxima of the scale-normalised Laplacian of Gaussian over 53 scales, σ from 0.93 to 34 mm);
2. every pair of a fixed and a moving blob compared by the correlation of their spherical
   patches (radius 20 voxels, the moving patch turned onto the fixed one's gradient axes); a
   blob within 20 voxels of an image's edge cannot be compared;
3. the best-correlated pairs whose radii differ by less than 0.25, up to `m`;
4. the pairs whose distances to at least 3 others are preserved within 10 % (squared) become
   landmarks;
5. an affine (`Affine`) or rigid (otherwise) transform fitted to the landmarks.

`lx.ants.blob_alignment` returns every step:

```python
r = lx.ants.blob_alignment("t1w.nii.gz", "t1w_moved.nii.gz", blobs=30, transform="Rigid")
r.fixed_blobs[0]       # Blob(index=(44, 129, 84), sigma=1.52, radius=2.63, value=734.4)
r.correspondence       # (30, 30) float32 patch correlations
r.pairs, r.landmarks   # matched (fixed, moving) blobs; the pairs kept as landmarks
r.fixed_landmarks      # their centres in ITK's physical space (LPS mm)
r.transform            # AffineTransform_double_3_3, centre 0: antsAI's initial transform
```

**Threads.** ANTs' `antsAI -b` crashes at any thread count above one (ANTS-60: its detector
indexes per-thread heaps past their end; verified at 2, 4 and 6 threads); its one-thread result
is what larmorx computes, on any thread count.

**What to expect.** The blob alignment is a rough initialiser for images of one subject: on a
T1w against its own copy moved by a translation ANTs finds the translation exactly; rotated by
8 degrees, ANTs' distance test (ANTS-110) keeps no landmark and its rigid transform is NaN, while
larmorx's corrected test finds a rotation within a few degrees. Between subjects or to a template
the patch matching rarely pairs the same anatomy and neither keeps a landmark with 30 blobs.
Numbers: the [validation record](../validation/ants-ai.md) (category `blobs`) and the
[findings](../findings/ants-ai.md#blobs).
