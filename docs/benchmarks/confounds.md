# Benchmarks: fMRIPrep's confound estimation (`lx.confounds`, `lx.niworkflows`' confound steps, `bold_confounds`)

The port of nipype 1.12.0's confound algorithms, niworkflows 1.15.4's and fMRIPrep 25.2.7's confound
steps and fMRIPrep's confounds workflow (`larmorx.pipelines.larmorprepx.confounds.bold_confounds`)
against the originals themselves, in-process, in the newest Python environment
(`oracles/python-sdcflows-2.17` with fMRIPrep 25.2.7's tag: numpy 2.5.4, SciPy 1.18.1, pandas 3.0.6,
nitime 0.12.1, nibabel 5.4.2). Wall time of the whole call: reading, the work, writing every file. The
port is bit for bit (a replica); a clean room of the confounds, built for speed, comes later.

- **Generated:** 2026-11-12 on Linux x86_64 (the main machine: AMD Ryzen 9 7945HX, 16 cores / 32
  threads), larmorx at commit `b08c2a2a` (the nodes) and `117c91ab` (the workflow and the linear
  algebra; the commits between changed documents only, the extension built from `545f3ff5`'s source),
  inside a memory box (20 GiB), with
  [`bench.py`](../../validation/investigations/confounds/bench.py) (its runs:
  [confounds.json](confounds.json)). `OPENBLAS_CORETYPE=Haswell`, `OPENBLAS_NUM_THREADS=1` (numpy's and
  SciPy's BLAS at one thread, as fMRIPrep's nodes run them and as the parity checks compare: the bits
  depend on OpenBLAS's thread count) and `OPENBLAS_L2_SIZE=512` (numpy 2.5.4's depth blocks of 256 on
  this AMD processor: [confounds-blas.md](../findings/confounds-blas.md)). larmorx at 1 and 8 threads
  (`n_threads`; the originals have no thread option). Each time is the median of three runs after one
  untimed run of each implementation (of the calls above half a second, 36 of 64 have their three
  runs within 5 % of each other, the others within 21 %: the shared machine's load).
- **The machine was not quiet**: two other agents and FreeSurfer's reference runs (at low priority)
  shared it, a one-minute load average of 2.5 to 9 of 32 threads at the parts' starts; the quiet old
  laptop was not used (nothing on the development path runs there). The times compare the two
  implementations under the same load; their absolute values are a few percent high.
- **The same result:** every larmorx call ran with `upstream_defects="reproduce"` and the
  environment's releases, and every file it wrote (decompressed; a NIfTI file's four extension bytes
  aside) and every dictionary it returned was compared with the original's of the same call:
  **identical in every row**, `model_expansion` by column name (its columns' order is niworkflows'
  hash-seeded set order, PIPE-43, never reproduced: [confounds-model.md](../findings/confounds-model.md)).

## Every Python node of the confounds workflow

The 26 nodes of fMRIPrep's `init_bold_confs_wf` that run Python (the two `antsApplyTransforms` nodes
are programs: larmorx's ANTs port has its own benchmark, [ants-apply-transforms](ants-apply-transforms.md);
the reportlets are not ported yet), each with the inputs fMRIPrep gave it in the first confounds
workflow of three of fMRIPrep's reference runs: **ds000005** (64 × 64 × 34 × 240, 32,069 brain voxels, one dummy
scan), **ds004958** (104 × 104 × 72 × 600, multiband 8, the crown 30,453 voxels, three dummy scans) and
**ds005454** (160 × 160 × 96 × 280, multiband, 436,984 brain voxels, the crown 60,968). The original is
the interface fMRIPrep runs (niworkflows' `RobustACompCor` and `RobustTCompCor` for CompCor, nipype's
`ComputeDVARS`, fMRIPrep's `FSLMotionParams`, `GatherConfounds`, ..., `_binary_union` through nipype's
`Function`), run by `interface.run()`. "fMRIPrep recorded" is the node's duration in its
`_report/report.rst` from fMRIPrep's reference run itself (fMRIPrep 25.2.6 in its image on the old laptop, an
Intel i7-8750H, with 6 to 8 nodes at once, one of the runs beside a development agent): another
machine and other libraries, shown for scale only. In brackets the speed-up against the original
measured here.

| Run | original (26 nodes) | larmorx (1) | larmorx (8) | fMRIPrep recorded |
|---|---|---|---|---|
| ds000005 | 7.48 s | 5.49 s (1.36×) | 4.62 s (1.62×) | 16.1 s |
| ds004958 | 77.9 s | 70.9 s (1.10×) | 50.8 s (1.53×) | 181.6 s |
| ds005454 | 96.3 s | 81.2 s (1.19×) | 62.6 s (1.54×) | 189.4 s |

**What is faster**: DVARS (1.0 to 2.1× at one thread, 2.3 to 3.9× at eight: the AR(1) loop and the sums
in threads), tCompCor (1.4 to 1.7×), the tissue signals (1.5 to 1.8×: read where the voxels are, no
float64 copy of the series), the aCompCor masks (1.1× at one thread, 3.6 to 4.8× at eight: the dilation
in threads), the motion parameters and the RMS deviation (1.4 to 2.6×), the masks' union, subtraction
and thresholds (2.8 to 24×), most metadata steps. **What is slower**: crownCompCor (0.72 to 0.78× at one
thread, 0.74 to 1.03× at eight) and aCompCor on ds005454 (0.86×), the small table steps (`concat`,
`rename_acompcor`, `model_expansion`, `spike_regressors`: 0.6 to 1.0×, `spike_regressors` 1.7× on the
smallest run; tens to a few hundred milliseconds), `dilated_mask` at one thread (0.64 to 0.81×; 0.91 to
1.34× at eight; hundredths of a second) and a few metadata steps of milliseconds. Why, and what
would make them faster with the same bits: "Where the time goes", below.

<details><summary>Every node of the three runs</summary>

#### ds000005

| Node | original | larmorx (1) | larmorx (8) | fMRIPrep recorded | result |
|---|---|---|---|---|---|
| `union_mask` | 0.021 s | 0.9 ms (23.09×) | 0.9 ms (24.66×) | 0.010 s | identical |
| `dilated_mask` | 4.9 ms | 6.0 ms (0.81×) | 3.6 ms (1.34×) | 0.017 s | identical |
| `subtract_mask` | 3.8 ms | 0.5 ms (7.44×) | 0.6 ms (6.86×) | 9.2 ms | identical |
| `dvars` | 2.49 s | 1.18 s (2.11×) | 0.639 s (3.90×) | 7.75 s | identical |
| `motion_params` | 0.041 s | 0.028 s (1.48×) | 0.026 s (1.60×) | 0.259 s | identical |
| `fdisp` | 3.1 ms | 1.3 ms (2.40×) | 1.2 ms (2.56×) | 0.011 s | identical |
| `rmsd` | 0.039 s | 0.021 s (1.87×) | 0.021 s (1.87×) | 0.130 s | identical |
| `acc_masks` | 0.651 s | 0.561 s (1.16×) | 0.148 s (4.39×) | 1.15 s | identical |
| `acc_msk_brain` | 0.031 s | 5.4 ms (5.70×) | 4.4 ms (7.07×) | 0.059 s | identical |
| `acc_msk_bin` | 0.024 s | 4.8 ms (5.08×) | 4.2 ms (5.80×) | 0.051 s | identical |
| `acompcor` | 0.917 s | 0.631 s (1.45×) | 0.621 s (1.48×) | 1.41 s | identical |
| `crowncompcor` | 1.22 s | 1.69 s (0.72×) | 1.70 s (0.72×) | 1.89 s | identical |
| `tcompcor` | 1.04 s | 0.721 s (1.44×) | 0.754 s (1.37×) | 1.62 s | identical |
| `rename_acompcor` | 0.031 s | 0.046 s (0.66×) | 0.046 s (0.67×) | 0.074 s | identical |
| `signals` | 0.730 s | 0.401 s (1.82×) | 0.435 s (1.68×) | 0.859 s | identical |
| `add_dvars_header` | 0.8 ms | 0.5 ms (1.70×) | 0.4 ms (1.76×) | 1.2 ms | identical |
| `add_std_dvars_header` | 0.8 ms | 0.4 ms (1.71×) | 0.4 ms (1.73×) | 1.4 ms | identical |
| `concat` | 0.063 s | 0.068 s (0.92×) | 0.068 s (0.92×) | 0.259 s | identical |
| `tcc_metadata_filter` | 1.4 ms | 0.5 ms (2.52×) | 0.5 ms (2.56×) | 5.4 ms | identical |
| `acc_metadata_filter` | 6.6 ms | 6.7 ms (0.97×) | 6.4 ms (1.02×) | 4.1 ms | identical |
| `tcc_metadata_fmt` | 5.8 ms | 0.2 ms (29.62×) | 0.2 ms (31.50×) | 5.5 ms | identical |
| `acc_metadata_fmt` | 2.6 ms | 2.1 ms (1.26×) | 1.9 ms (1.33×) | 6.8 ms | identical |
| `crowncc_metadata_fmt` | 2.7 ms | 2.4 ms (1.15×) | 2.3 ms (1.18×) | 3.6 ms | identical |
| `merge_confound_metadata2` | 1.6 ms | 0.0 ms (105.16×) | 0.0 ms (105.71×) | 2.4 ms | identical |
| `model_expansion` | 0.054 s | 0.060 s (0.90×) | 0.070 s (0.77×) | 0.265 s | identical by name (PIPE-43: the columns' order) |
| `spike_regressors` | 0.095 s | 0.056 s (1.68×) | 0.062 s (1.53×) | 0.207 s | identical |
| **all 26** | **7.48 s** | **5.49 s** (1.36×) | **4.62 s** (1.62×) | 16.1 s | |

#### ds004958

| Node | original | larmorx (1) | larmorx (8) | fMRIPrep recorded | result |
|---|---|---|---|---|---|
| `union_mask` | 0.023 s | 2.7 ms (8.38×) | 2.6 ms (8.56×) | 0.060 s | identical |
| `dilated_mask` | 0.020 s | 0.031 s (0.64×) | 0.017 s (1.15×) | 0.215 s | identical |
| `subtract_mask` | 0.010 s | 1.5 ms (6.58×) | 1.5 ms (6.68×) | 0.027 s | identical |
| `dvars` | 22.3 s | 22.6 s (0.99×) | 9.76 s (2.29×) | 77.1 s | identical |
| `motion_params` | 0.124 s | 0.074 s (1.67×) | 0.076 s (1.62×) | 0.385 s | identical |
| `fdisp` | 3.0 ms | 3.2 ms (0.92×) | 3.0 ms (0.99×) | 0.033 s | identical |
| `rmsd` | 0.127 s | 0.049 s (2.62×) | 0.050 s (2.54×) | 0.485 s | identical |
| `acc_masks` | 1.44 s | 1.32 s (1.09×) | 0.299 s (4.83×) | 4.20 s | identical |
| `acc_msk_brain` | 0.109 s | 0.025 s (4.32×) | 0.015 s (7.21×) | 0.248 s | identical |
| `acc_msk_bin` | 0.085 s | 0.021 s (3.99×) | 0.018 s (4.78×) | 0.196 s | identical |
| `acompcor` | 12.8 s | 8.91 s (1.44×) | 8.52 s (1.51×) | 25.1 s | identical |
| `crowncompcor` | 16.4 s | 21.0 s (0.78×) | 16.0 s (1.03×) | 23.9 s | identical |
| `tcompcor` | 14.2 s | 9.98 s (1.42×) | 9.33 s (1.52×) | 29.7 s | identical |
| `rename_acompcor` | 0.177 s | 0.255 s (0.69×) | 0.252 s (0.70×) | 0.485 s | identical |
| `signals` | 9.34 s | 5.74 s (1.63×) | 5.51 s (1.70×) | 17.6 s | identical |
| `add_dvars_header` | 1.1 ms | 0.8 ms (1.38×) | 0.8 ms (1.39×) | 4.9 ms | identical |
| `add_std_dvars_header` | 1.1 ms | 0.8 ms (1.32×) | 0.9 ms (1.14×) | 1.9 ms | identical |
| `concat` | 0.227 s | 0.327 s (0.70×) | 0.358 s (0.63×) | 0.737 s | identical |
| `tcc_metadata_filter` | 1.4 ms | 1.2 ms (1.19×) | 1.2 ms (1.20×) | 0.053 s | identical |
| `acc_metadata_filter` | 2.7 ms | 3.9 ms (0.70×) | 4.3 ms (0.64×) | 0.019 s | identical |
| `tcc_metadata_fmt` | 1.7 ms | 0.2 ms (9.41×) | 0.2 ms (9.36×) | 0.010 s | identical |
| `acc_metadata_fmt` | 5.5 ms | 5.7 ms (0.97×) | 5.3 ms (1.04×) | 0.013 s | identical |
| `crowncc_metadata_fmt` | 3.4 ms | 5.2 ms (0.65×) | 5.2 ms (0.65×) | 0.038 s | identical |
| `merge_confound_metadata2` | 3.1 ms | 0.0 ms (119.08×) | 0.0 ms (117.40×) | 4.5 ms | identical |
| `model_expansion` | 0.234 s | 0.275 s (0.85×) | 0.358 s (0.65×) | 0.601 s | identical by name (PIPE-43: the columns' order) |
| `spike_regressors` | 0.170 s | 0.266 s (0.64×) | 0.273 s (0.62×) | 0.457 s | identical |
| **all 26** | **77.9 s** | **70.9 s** (1.10×) | **50.8 s** (1.53×) | 181.6 s | |

#### ds005454

| Node | original | larmorx (1) | larmorx (8) | fMRIPrep recorded | result |
|---|---|---|---|---|---|
| `union_mask` | 0.040 s | 9.0 ms (4.48×) | 9.7 ms (4.15×) | 0.134 s | identical |
| `dilated_mask` | 0.061 s | 0.090 s (0.68×) | 0.067 s (0.91×) | 0.163 s | identical |
| `subtract_mask` | 0.021 s | 3.0 ms (6.98×) | 2.7 ms (7.81×) | 0.037 s | identical |
| `dvars` | 38.1 s | 28.9 s (1.32×) | 13.6 s (2.81×) | 88.0 s | identical |
| `motion_params` | 0.107 s | 0.075 s (1.42×) | 0.074 s (1.44×) | 0.302 s | identical |
| `fdisp` | 2.1 ms | 1.3 ms (1.71×) | 1.3 ms (1.71×) | 0.029 s | identical |
| `rmsd` | 0.059 s | 0.023 s (2.60×) | 0.022 s (2.62×) | 0.188 s | identical |
| `acc_masks` | 1.71 s | 1.56 s (1.10×) | 0.477 s (3.59×) | 3.40 s | identical |
| `acc_msk_brain` | 0.225 s | 0.069 s (3.26×) | 0.044 s (5.08×) | 0.394 s | identical |
| `acc_msk_bin` | 0.165 s | 0.059 s (2.82×) | 0.049 s (3.35×) | 0.478 s | identical |
| `acompcor` | 10.0 s | 11.7 s (0.86×) | 11.4 s (0.88×) | 23.9 s | identical |
| `crowncompcor` | 12.4 s | 17.3 s (0.72×) | 16.7 s (0.74×) | 20.7 s | identical |
| `tcompcor` | 21.1 s | 13.1 s (1.62×) | 12.2 s (1.73×) | 28.0 s | identical |
| `rename_acompcor` | 0.046 s | 0.059 s (0.78×) | 0.059 s (0.79×) | 0.160 s | identical |
| `signals` | 11.9 s | 8.06 s (1.47×) | 7.59 s (1.56×) | 22.8 s | identical |
| `add_dvars_header` | 0.8 ms | 0.5 ms (1.67×) | 0.5 ms (1.66×) | 1.5 ms | identical |
| `add_std_dvars_header` | 5.5 ms | 0.5 ms (11.72×) | 0.5 ms (11.56×) | 0.9 ms | identical |
| `concat` | 0.094 s | 0.114 s (0.82×) | 0.127 s (0.74×) | 0.218 s | identical |
| `tcc_metadata_filter` | 1.1 ms | 0.6 ms (1.77×) | 0.6 ms (1.78×) | 5.5 ms | identical |
| `acc_metadata_filter` | 1.8 ms | 1.9 ms (0.97×) | 1.9 ms (0.95×) | 0.010 s | identical |
| `tcc_metadata_fmt` | 1.3 ms | 0.2 ms (5.39×) | 0.2 ms (5.68×) | 3.8 ms | identical |
| `acc_metadata_fmt` | 3.1 ms | 3.4 ms (0.92×) | 3.4 ms (0.93×) | 7.4 ms | identical |
| `crowncc_metadata_fmt` | 2.1 ms | 2.5 ms (0.85×) | 2.5 ms (0.86×) | 5.9 ms | identical |
| `merge_confound_metadata2` | 1.8 ms | 0.0 ms (101.84×) | 0.0 ms (114.57×) | 2.8 ms | identical |
| `model_expansion` | 0.099 s | 0.100 s (0.98×) | 0.136 s (0.73×) | 0.247 s | identical by name (PIPE-43: the columns' order) |
| `spike_regressors` | 0.068 s | 0.098 s (0.70×) | 0.110 s (0.62×) | 0.165 s | identical |
| **all 26** | **96.3 s** | **81.2 s** (1.19×) | **62.6 s** (1.54×) | 189.4 s | |

</details>

## The whole workflow: `bold_confounds`

`bold_confounds` on each run's first confounds workflow, from its own inputs (as `check_workflow.py`'s
`real` section reads them), the default (`upstream_defects="fix"`) with the newest releases, every node
of fMRIPrep's workflow but the reportlets, `antsApplyTransforms` through larmorx's ANTs port; each
node's share timed inside the run. Against it, the original's 26 Python nodes measured here (the
table above: the two `antsApplyTransforms` runs come on top, not run here) and the sum of the
durations fMRIPrep's nipype recorded for the same nodes in the reference run (the old laptop, for scale).

| Run | the original's 26 Python nodes, here | larmorx `bold_confounds` (1) | larmorx `bold_confounds` (8) | fMRIPrep's nodes as recorded |
|---|---|---|---|---|
| ds000005 | 7.48 s | 5.86 s | 4.31 s | 18.3 s |
| ds004958 | 77.9 s | 66.3 s | 49.7 s | 191.2 s |
| ds005454 | 96.3 s | 96.5 s | 68.5 s | 212.0 s |

**At one thread** larmorx's whole workflow (its two ANTs resamplings included) takes about what the
original's Python nodes take alone (0.78 to 1.0 times); **at eight threads** 1.4 to 1.7 times less.
The original's whole workflow also runs its two `antsApplyTransforms` programs (fMRIPrep recorded 2.2,
9.6 and 22.7 s for them, one thread each on the old laptop); larmorx's ANTs port takes 0.7, 4.0 and
11.0 s at one thread, 0.1, 0.6 and 1.7 s at eight. ds005454, node by node:

| Node | original, here | larmorx (1) | larmorx (8) | fMRIPrep recorded |
|---|---|---|---|---|
| `dvars` | 38.1 s | 30.2 s | 14.1 s | 88.0 s |
| `tcompcor` | 21.1 s | 13.5 s | 13.0 s | 28.0 s |
| `acompcor` | 10.0 s | 11.7 s | 11.7 s | 23.9 s |
| `signals` | 11.9 s | 8.52 s | 8.15 s | 22.8 s |
| `crowncompcor` | 12.4 s | 19.1 s | 18.8 s | 20.7 s |
| `t1w_mask_tfm` | a program (not run here) | 6.35 s | 0.848 s | 13.1 s |
| `acc_msk_tfm` (three items) | a program (not run here) | 4.61 s | 0.804 s | 9.57 s |
| `acc_masks` | 1.71 s | 1.79 s | 0.566 s | 3.40 s |
| `acc_msk_bin` | 0.165 s | 0.076 s | 0.049 s | 0.478 s |
| `acc_msk_brain` | 0.225 s | 0.072 s | 0.041 s | 0.394 s |
| `motion_params` | 0.107 s | 0.107 s | 0.092 s | 0.302 s |
| the other 19 | 0.507 s | 0.634 s | 0.621 s | 1.38 s |

(The node times inside the workflow, in the default mode on larmorx's own intermediate files, are
within about 10 % of the same nodes' times in the first table, under `reproduce` on fMRIPrep's files;
the three CompCor nodes read the series again each, as fMRIPrep's nodes do.)

## The linear algebra on CompCor's largest matrices

numpy's `svd(full_matrices=False)` (CompCor's decomposition: `DGESDD` path 3t, the crowns of ds005454
and ds004958), `lstsq` (`cosine_filter`'s: the designs of ds004958, ds005454 and ds002603-like runs on
the crowns' voxels, and one of 26 regressors that takes `DLALSD`'s divide-and-conquer path) and SciPy's
`svd(lapack_driver='gesvd')` (nipype's `fallback_svd`) on random matrices of those shapes (the bits do
not depend on the values), numpy and SciPy at one BLAS thread, larmorx at 1 and 8 threads; numpy's
`svd` rows the median of three runs, the `lstsq` and `gesvd` rows from `check_gelsd.py --time` and
`check_gesvd.py --time` (one run per larmorx thread count; the original's time the median of its two
runs). Every result bit for bit identical.

| Call | original (one BLAS thread) | larmorx (1) | larmorx (8) | identical |
|---|---|---|---|---|
| numpy `svd` 280 × 60,968 | 3.46 s | 11.7 s (0.30×) | 10.5 s (0.33×) | yes |
| numpy `svd` 597 × 30,453 | 3.50 s | 11.0 s (0.32×) | 8.48 s (0.41×) | yes |
| numpy `lstsq` 600 × 7, 60,968 voxels | 0.573 s | 2.40 s (0.24×) | 1.98 s (0.29×) | yes |
| numpy `lstsq` 280 × 8, 60,968 voxels | 0.243 s | 1.02 s (0.24×) | 0.989 s (0.25×) | yes |
| numpy `lstsq` 600 × 25, 30,453 voxels | 0.563 s | 2.35 s (0.24×) | 2.30 s (0.24×) | yes |
| numpy `lstsq` 600 × 26, 60,968 voxels (divide and conquer) | 1.13 s | 5.15 s (0.22×) | 5.05 s (0.22×) | yes |
| SciPy `gesvd` 280 × 60,968 | 3.59 s | 11.3 s (0.32×) | 11.2 s (0.32×) | yes |
| SciPy `gesvd` 597 × 30,453 | 3.40 s | 10.3 s (0.33×) | 8.12 s (0.42×) | yes |

## Where the time goes, and what would make it faster with the same bits

- **CompCor's SVD and filter** (crownCompCor, and aCompCor on the largest run): larmorx's port of
  OpenBLAS and LAPACK is 3 to 4.5 times slower than numpy's library on these shapes and gains little
  from threads. Its time is in the level-1 and level-2 kernels of the Householder panels (`DGELQ2`,
  `DGEQR2`), the bidiagonal reduction (`DLABRD`) and `DORM2R`/`DLALS0` over the voxels' right-hand
  sides (`dgemv`, `dger`, `daxpy`), which use `f64::mul_add` without the FMA target feature (a C
  library `fma` call per element on the x86-64 baseline) and run on one thread; the blocked
  products (`dgemm`, `dtrmm`) already use the audited AVX2/FMA kernel and threads. On ds005454's crown
  the SVD alone takes about 11 s of larmorx's 17 s against numpy's 3.5 s. What keeps the bits and
  removes most of it: the same audited FMA dispatch for the level-1/2 kernels; `dgemv_t`, `dger` and
  `DORM2R`'s and `DLALS0`'s columns split over threads in groups of four (each column's operations
  fixed); finer `dgemm` tiles for narrow outputs; and `VT` not computed where CompCor keeps only `U`
  and `S` (path 3t's `DORGLQ` and its `DGEMM` do not feed them). [confounds-blas.md](../findings/confounds-blas.md)
  has the measurements of each kernel.
- **DVARS at one thread** (0.99× on ds004958's 600 volumes, 1.3× on ds005454): the AR(1) loop's FFTs
  of 2,048 and 1,024 points; the fused complex64 products go through the C library's `fmaf` on the
  baseline (the FMA dispatch again) and the FFT passes allocate their buffers per call
  ([confounds-dvars.md](../findings/confounds-dvars.md)). With threads DVARS is 2.3 to 3.9 times the
  original's speed.
- **The table steps** (`concat`, `rename_acompcor`, `model_expansion`, `spike_regressors`, 0.6 to 0.9×:
  tens to a few hundred milliseconds per call): larmorx's text layer (pandas' reading and writing
  without pandas: [confounds-pandas.md](../findings/confounds-pandas.md)) is Python over every cell:
  the tokens, pandas' integer attempt per column, both parses of every value (pandas' and the
  correctly rounded one, for the PD-01 notice), the writing cell by cell; pandas does these in C.
  Moving the per-cell work into Rust keeps the bits (they are pandas' and CPython's formatting rules,
  ported exactly).
- **Everything else** is faster than the original at one thread already, and the threaded steps
  (DVARS, the masks' dilation, the ANTs resamplings) gain the most from threads.

These are the port's speed items (a replica: bit for bit first); the clean room of the confounds, built
for speed, comes after FreeSurfer.

## Reproduce

```bash
# In the newest Python environment, the worktree's extension, fMRIPrep 25.2.7's tag, in a memory box.
export OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 OPENBLAS_L2_SIZE=512
export PYTHONPATH=<repo>/python:<workspace>/tags/fmriprep-25.2.7 LARMORX_WORKSPACE=<workspace>
for part in nodes workflow linalg; do
  scripts/orchestration/boxed.sh 20 <workspace>/oracles/python-sdcflows-2.17/bin/python -P \
    validation/investigations/confounds/bench.py $part --repeats 3 --threads 1,8 --out $part.json
done
```

The three JSON files are joined in [confounds.json](confounds.json) (`nodes`, `workflow`, `linalg`).
