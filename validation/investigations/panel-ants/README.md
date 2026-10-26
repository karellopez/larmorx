# The ANTs tools on the data panel (re-validation, part a, 2026-10-26)

The tools behind the panel records of the ANTs and ITK ports (`docs/validation/ants-*-panel.md`),
their inventory of fMRIPrep's nodes, the memory and time measurements on the panel's largest
inputs, and the before/after photographs of the documentation site's pages.

| Script | What it shows | How to run it |
|---|---|---|
| `inventory.py` | Every ANTs/ITK program node of the finished fMRIPrep 25.2.6 oracle runs, by program and run (counts) and by normalised node path; `--json` writes every node with its command | `PYTHONPATH=<checkout>/python:<checkout>/validation/src python -P inventory.py <workspace>/oracles/fmriprep-25.2.6 [--json out.json]` |
| `memory_real.py` | Peak memory (`/usr/bin/time -v`) and time of one ANTs call and larmorx's (both D13 modes) on real files, one thread, outputs compared byte for byte | `python -P memory_real.py <work> [--crmath] <program> -- <args with {out}>` |
| `memory_largest.py` | `memory_real.py` on each tool's largest call on the data panel (the 7 T MP2RAGE, the 600- and 588-volume series); a table `memory.md` | `python -P memory_largest.py <out> [case ...]` (in a memory box, on a quiet machine) |
| `global_pool_probe.py` | Whether each registered program (74 code paths: the ANTs programs, the AFNI originals and GPL replicas, the head-motion correction, the FSL replicas) runs its parallel work only in its own pool of the call's thread count: every thread of its processes, at one thread (`docs/findings/ants-cli.md`, "larmorx's own threads"); exits 1 when one starts a thread outside its pool | `python -P global_pool_probe.py <dir> [--gpl <larmorx-gpl>] [--nc <larmorx-nc>]` (Linux) |
| `ai_variability.py` | antsAI's own spread on fMRIPrep's calls where larmorx (= ANTs 2.6.5 at one thread) and fMRIPrep's file are far apart: the ANTs oracle at 1 and three times at 8 threads, fMRIPrep's image at 1 and twice at the node's threads | `python -P ai_variability.py <out> <run>/<node> ...` |
| `time_registration.py` | Wall time and peak memory of fMRIPrep's registrations (whole and `/small` panel cases), ANTs at one thread and larmorx, on a quiet machine: the basis of the registration panel's estimate | `python -P time_registration.py <out> <lx threads> <case id> ...` |
| `site_shots.py` | The ANTs pages of the documentation site photographed whole (1440 and 390 px wide, light and dark) and the site's number and viewer checks, for the before/after comparison | `python -P site_shots.py <site checkout> <base url> <out>` with the site served by `tools/build.py --serve` |

The parity side is in the suites themselves: the panel tier (`--tier panel`) of each ANTs suite
runs fMRIPrep's calls on every finished oracle run (`larmorx_validation.parity.fmriprep_nodes`),
the template registration's inputs made again (`parity.niworkflows_inputs`) and the panel's
images (`parity.panel_images`); the evidence side is `larmorx_validation.evidence.ants_panel`
and `panel_examples`.

## The inventory (2026-10-26, fourteen finished runs)

| Program | Nodes | Distinct calls | Runs |
|---|---:|---:|---:|
| antsApplyTransforms | 421 | 421 | 14 |
| ImageMath | 378 | 365 (the template's Laplacian is one call, made in every run) | 14 |
| N4BiasFieldCorrection | 64 | 64 | 14 |
| antsRegistration | 57 | 57 | 14 |
| MultiplyImages | 42 | 42 | 14 |
| antsAI | 36 | 36 | 14 |
| DenoiseImage | 17 | 17 | 14 |
| Atropos | 14 | 14 | 14 |
| ThresholdImage | 14 | 14 | 14 |
| antsAffineInitializer | 0 (14 calls inside the template registration's node) | 14 | 14 |

No run calls SmoothImage, ResampleImage or ResampleImageBySpacing: their panel cases are the
panel's images. Every node's inputs are still in its run's working directory except the template
registration's (`fixed_masked.nii.gz`, `moving_masked.nii.gz`, the cost-function masks of the
stroke run, `transform.mat`), which niworkflows writes for itself and nipype does not keep; they
are made again exactly from the node's recorded inputs, the initial transform by the ANTs
oracle's antsAffineInitializer at one thread (fMRIPrep's ran at the node's six or eight threads).
