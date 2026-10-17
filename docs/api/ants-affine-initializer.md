# antsAffineInitializer

`lx.ants.affine_initializer` (command line `larmorx ants antsAffineInitializer`; Rust
`larmorx_ants::affine_initializer` and the crate `larmorx-registration`). A replica of ANTs
2.6.5's `antsAffineInitializer` on ITK 5.4.5 (Apache-2.0): an affine transform that aligns two
images' centres of mass (and, optionally, their principal axes), then the best start point of a
multi-start search over rotations about the fixed image's principal axes, each refined by 20
conjugate-gradient iterations of Mattes mutual information on every 10th voxel; in 2D, 3D and
4D (in 4D the initial alignment only). niworkflows' `RobustMNINormalization` runs it when it is
given no initial transform (fMRIPrep's template registration).

**Status: `validated`** against `antsAffineInitializer` itself (the
[validation record](../validation/ants-affine-initializer.md), standard tier): every compared
case identical to ANTs 2.6.5 at one thread (exit status, both streams, the transform file byte
for byte), including the usage, every abort and its messages, 2D, 3D and 4D, masks, oblique
grids and real images (the MNI template and fMRIPrep's preprocessed T1w). The counts are in
the record.

**Results do not depend on threads.** larmorx gives ANTs' one-thread result at any thread
count.

## Quick start

```python
import larmorx as lx

t = lx.ants.affine_initializer(
    "tpl-MNI152NLin2009cAsym_res-01_T1w.nii.gz", "sub-01_T1w.nii.gz",
    search_factor=15, radian_fraction=0.1, principal_axes=False,
)
t                        # ItkTransform('AffineTransform_double_3_3', 12 parameters, 3 fixed)
```

```bash
larmorx ants antsAffineInitializer 3 fixed.nii.gz moving.nii.gz transform.mat 15 0.1 0 10
```

## Option mapping

| antsAffineInitializer (positional) | Python | Notes |
|---|---|---|
| 1 `ImageDimension` | `dimension` | default: the fixed image's; `std::stoi`; any number but 2, 3, 4 runs as 2D in ANTs (ANTS-56; corrected, an error) |
| 2, 3 `Image1`, `Image2` | `fixed`, `moving` | read as float images; a missing image, or one whose values sum to 0, aborts (` zero image1 error `) |
| 4 `TransformOutput` | the returned `ItkTransform` | written twice: the initial alignment, then the search's best (`.mat`, `.txt`, `.tfm`, HDF5) |
| 5 `SearchFactor` | `search_factor` | degrees (×0.0174532925), `atof`; default 10 |
| 6 `Radian-Fraction` | `radian_fraction` | `atof`, clamped to `[0.01, 1]`; the angles run over `[−π·f, π·f + step]`; default 0.25 (`π/4`) |
| 7 `UsePrincipalAxes` | `principal_axes` | `std::stoi`, non-zero is true; default false |
| 8 `UseLocalSearch` | `local_search_iterations` | **not read** by ANTs: every start point runs 20 iterations (ANTS-54, reproduced until decided) |
| 9 `Image1Mask` | `fixed_mask` | the metric's fixed mask; one that cannot be read is no mask |
| fewer than 2 arguments | – | the usage on standard error, exit 0; 2 or 3 arguments abort (ANTS-57) |
| `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`, `NSLOTS` | `n_threads` | the result does not depend on it |
| `LARMORX_UPSTREAM_DEFECTS` | `upstream_defects=` | ANTS-56, ITK-19 ([upstream defects](upstream-defects.md)) |

Standard error carries ANTs' ` bad det ...` lines when the principal axes' rotation has a
negative determinant (they are printed whatever `principal_axes` says) and ITK's warnings.

## Upstream defects (D13)

- **ANTS-56** (corrected by default): a dimension other than 2, 3 or 4 runs as 2D; larmorx
  stops with an error; `reproduce` runs it as 2D.
- **ITK-19**: as in [antsAI](ants-ai.md).
- **ANTS-54** and **ITK-21** (questionable, reproduced until decided): the local-search argument
  is ignored; the learning rate the line search leaves in one start point is where the next
  one starts (no scales estimator re-estimates it).

Never reproduced (class (a)): a search factor that does not advance the angle (0, negative,
text) never ends ANTs' loop, and one that is not a number lists no start point (ANTs then
crashes); larmorx refuses both with a message. Where ANTs aborts on an exception, larmorx prints
what ANTs prints and exits 1.
