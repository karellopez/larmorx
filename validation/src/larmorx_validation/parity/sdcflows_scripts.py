# SPDX-License-Identifier: Apache-2.0
"""The parity suites of the field maps' part B: the bit-comparison scripts of
``validation/investigations/sdcflows-fieldmap-b`` (each one validated in both kept Python
environments when it was written) run as cases, so that their results become records and the
parity tiers run them.

Three suites (decision D12 of the part, ``docs/parts/``):

- ``ndimage-skimage``: the library ports under the field maps: SciPy's ndimage filters,
  morphology, labels, sums, ``watershed_ift`` and ``center_of_mass`` (``lx.ndimage``), SciPy's
  ``inv`` (``lx.transforms``), scikit-image's ``ball``, ``threshold_otsu``, ``random_walker`` and
  ``match_histograms`` (``lx.skimage``) and numpy's statistics and float32 ``exp``
  (``larmorx_core::numpy``), against the environment's SciPy, numpy and scikit-image;
- ``niworkflows-helpers``: niworkflows' interfaces (``IntraModalMerge``, ``MergeSeries``,
  ``RobustAverage``, ``RegridToZooms``, ``IntensityClip``) and nipype's command lines
  (``lx.niworkflows``), against niworkflows 1.15.4 and nipype in-process and fMRIPrep 25.2.6's
  own files;
- ``sdcflows-estimation``: sdcflows' masks, workflows (direct field maps, PEPOLAR with TOPUP,
  3dQwarp's Python steps, fieldmap-less SyN), the estimators' discovery, the derivatives, the
  correction (``unwarp_workflow``), sdcflows' BIDS app and fMRIPrep's own field-map steps
  (``lx.sdcflows``, ``lx.bids``, larmorprepx), against sdcflows 2.17.1 and fMRIPrep 25.2.6's
  own graphs and files (the programs underneath through larmorx's ports and the oracle cache).

**Each case** runs one script, or some of its sections, as a separate process of the running
environment's interpreter (``python -P <script> <arguments>``: the scripts parse their own
arguments, put stand-in programs on ``PATH``, set environment variables and logging levels, and
``check_regrid.py`` starts itself again with numpy's AVX-512 kernels off), with this larmorx
and this validation package on ``PYTHONPATH``, ``LARMORX_WORKSPACE``, numpy's OpenBLAS on its
Haswell kernels at one thread (``OPENBLAS_CORETYPE=Haswell``, ``OPENBLAS_NUM_THREADS=1``, as the
scripts were validated), the FSL replicas' program (``LARMORX_NC_BIN``) where a script runs
one, and a scratch folder of its own. A heavy case runs in a memory box: the one the
parity run itself runs in (every heavy job runs in one: ``scripts/orchestration/boxed.sh``),
else a box of its own of the size its README row names. Every summary line the
script prints is one check: ``<name>: <N> cases, identical <K>`` (``K == N``), SciPy's ``inv``
families (``<N> cases, <K> identical, rcond <a>/<b> identical, ..., <D> differ``: ``D == 0``,
``a == b``), the estimators' sections (``== <name>: <K> identical, <M> differ``: ``M == 0``),
``<name>: same``; ``INFO`` and ``NOTICES`` lines are reported values, ``EXPECTED`` lines
expected differences (the script names the registered defect); the script's final line
(``ALL OK``) and its exit status are two more checks, and a run that compared nothing fails.

**The originals' own step** (``python_versions``' ``STEP_SUITES``) for the library scripts: a
script that prints ``STEP <sha256> <name>`` lines (the digest of the originals' outputs behind
the comparison ``<name>``: SciPy's, numpy's or scikit-image's side, ``steplines.py`` beside the
scripts) gets one more check per line, whether those outputs are the same under both releases.
A run in ``newest`` first runs the step's cases in the shared environment, twice; the workflow
scripts carry no step (sdcflows 2.17.1, niworkflows 1.15.4 and nipype are the same in both
environments, and the libraries under them have their own), so the step's passes skip them.
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

import numpy as np

from larmorx_validation.parity import python_versions as pv
from larmorx_validation.parity.harness import (
    SKIPPED,
    Case,
    CaseResult,
    Check,
    CheckList,
    Outcome,
    Suite,
)

#: Where the scripts are (the repository's ``validation/investigations/sdcflows-fieldmap-b``).
SCRIPTS = Path(__file__).resolve().parents[3] / "investigations" / "sdcflows-fieldmap-b"
#: The memory box (``scripts/orchestration/boxed.sh``).
BOXED = Path(__file__).resolve().parents[4] / "scripts" / "orchestration" / "boxed.sh"
#: The environment variable that asks a library script for its ``STEP`` lines.
STEP_LINES = "LARMORX_STEP_LINES"
#: The originals' changes from the shared releases to the newest that a step line may show,
#: each with its reason (``python_versions.CHANGES``'s and this part's; a change without one
#: fails its check). ``validation/investigations/sdcflows-fieldmap-b/step_diff.py`` shows each
#: change's first case in both environments.
CHANGES: dict[str, str] = {
    **pv.CHANGES,
    "SCIPY-1.18-INV": "SciPy 1.18 rewrote scipy.linalg.inv: it detects the input's structure "
    "(diagonal, triangular, symmetric, general) and inverts each with its own LAPACK routines "
    "(trtri, potrf/potri, sytrf/sytri; getrf/getri otherwise), estimates the condition number "
    "(gecon, trcon, pocon, sycon) behind a new LinAlgWarning, and words its errors anew "
    "('A singular matrix detected: slice(s) [0] are singular.', 'Expected square matrix, got "
    "a1.shape=...'); 1.15.3 ran getrf/getri on every input. Signed zeros, last bits, warnings "
    "and messages differ; larmorx follows each release (scipy_inv's scipy_version; "
    "docs/findings/scipy-ndimage-filters.md, relays 18 and 19)",
    "NUMPY-2.5-PERCENTILE-RANGE": "numpy 2.5 checks percentile's q against [0, 100] before "
    "dividing it by 100; numpy 2.3 checked the quotient (for a Python q in the array's type), "
    "so a q just outside the range (100.000001 with a float32 array, 100.00000000000001, "
    "-1e-320) passed as 100 or 0 in 2.3 and raises ValueError in 2.5; larmorx follows each "
    "release (numpy_version; docs/api/ndimage-filters.md)",
    "SCIPY-1.16-ROW-ORDER": "SciPy 1.16 onwards sorts every row of a sparse matrix built "
    "through COO (1.15 stored the Laplacian's diagonal after each row's other entries), so "
    "scikit-image's random_walker adds the conjugate gradients' products in another order: "
    "the probabilities' last bits differ between the releases; larmorx follows each "
    "(scipy_version; docs/api/skimage.md, 'SciPy's row order')",
    "NUMPY-2.4-SCALAR": "numpy 2.4 onwards (2.5.4 here) raises TypeError when an array of one "
    "element with ndim > 0 is converted to a scalar ('only 0-dimensional arrays can be converted "
    "to Python scalars'; to an index, 'only integer scalar arrays can be converted to a scalar "
    "index'): numpy 1.25's deprecation expired (numpy/_core/src/multiarray/common.c, "
    "check_is_convertible_to_scalar; numpy 2.4.0's release notes); numpy 2.3.5 warned "
    "(DeprecationWarning) and took the element. So a one-element array as the filters' cval is "
    "an error in the newest releases; larmorx converts cval as SciPy's C does, through the "
    "running numpy's float() (_ndimage_filters._c_double), and follows each release",
    "NUMPY-2.5-WRITEBACK-TEXT": "numpy 2.5 words the warning it gives when a WRITEBACKIFCOPY "
    "copy is freed unresolved 'WRITEBACKIFCOPY detected in clearing of array.' "
    "(numpy/_core/src/multiarray/arrayobject.c, _clear_array_attributes); numpy 2.3.5's "
    "array_dealloc said 'WRITEBACKIFCOPY detected in array_dealloc.'; the values written back "
    "are the same. larmorx's reproduce gives the running numpy's text (SCI-23, SCI-26; "
    "docs/api/ndimage-filters.md)",
    "SCIPY-1.16-SCI-17": "SciPy 1.16.1 onwards (gh-23293) runs the n-dimensional filter for a 1-D "
    "input shorter than half the footprint, where 1.15.3's running rank returned zeros before "
    "writing (SCI-17, fixed upstream): those inputs are compared in the newest releases and "
    "counted apart ('1-D rank, SciPy 1.15 step') in the shared ones, so the comparison's cases "
    "differ; larmorx follows 1.18 in both (docs/api/ndimage-filters.md)",
    "SCIPY-1.18-SCI-24": "SciPy 1.15.3's running rank returns with a Python error set for an "
    "unknown type number (numpy's longlong) and a cval its type cannot take, which CPython turns "
    "into a SystemError (SCI-24, fixed in 1.18.1, whose TypeError larmorx gives in both): those "
    "calls are counted under SCI-24's comparison in the shared releases and under the filter's "
    "own (or SCI-22's) in the newest, so these comparisons' cases differ",
}
#: Which comparisons each change reaches: per case (``<suite>/<case>``) a regular expression of
#: the comparison's name -> the key of :data:`CHANGES` (filled as the step's changes are
#: measured; the next relay measures the rest: NOTES.md).
#: check_ndimage's changes (relay 24: ``step_diff.py``, each comparison's first changed case
#: in both environments read; the same in its quick and full runs).
_NDIMAGE_REASONS: dict[str, str] = {
    r"^1-D rank, short input$": "SCIPY-1.16-SCI-17",
    r"^arguments: SCI-22 reproduced \(SciPy's error\)$": "SCIPY-1.18-SCI-24",
    r"^arguments: SCI-2[13] reproduced ": "NUMPY-2.5-WRITEBACK-TEXT",
    r"^arguments: SCI-25 family: reproduce = SciPy$": "NUMPY-2.5-WRITEBACK-TEXT",
    r"^arguments: binary_(closing|dilation|erosion|fill_holes|opening|propagation) "
    r"\(reproduce = SciPy\)$": "NUMPY-2.5-WRITEBACK-TEXT",
    r"^arguments: (grey_(closing|dilation size|erosion structure|opening)|maximum_filter "
    r"footprint|(minimum|maximum)_filter1d|(median|rank|percentile)_filter|minimum_filter "
    r"size|minimum_filter1d 1-D|(median|rank|percentile)_filter 1-D)$": "NUMPY-2.4-SCALAR",
    r"^arguments: (median|rank|percentile)_filter 1-D$": "SCIPY-1.18-SCI-24",
}
REASONS: dict[str, dict[str, str]] = {
    "ndimage-skimage/ndimage-quick": _NDIMAGE_REASONS,
    "ndimage-skimage/ndimage": _NDIMAGE_REASONS,
    "ndimage-skimage/scipy-inv-quick": {".*": "SCIPY-1.18-INV"},
    "ndimage-skimage/scipy-inv": {".*": "SCIPY-1.18-INV"},
    "ndimage-skimage/numpy": {r"^percentile \(Python q\)$": "NUMPY-2.5-PERCENTILE-RANGE"},
    "ndimage-skimage/numpy-expf": {r"^percentile \(Python q\)$": "NUMPY-2.5-PERCENTILE-RANGE"},
    "ndimage-skimage/skimage-random-walker": {r"^random_walker$": "SCIPY-1.16-ROW-ORDER"},
}


@dataclass(frozen=True)
class Script:
    """One case: a script, its arguments and how it runs."""

    script: str
    args: tuple[str, ...]
    tier: str
    description: str
    #: The memory box in GiB (0: none; the README's size for the script's run).
    gib: int = 0
    #: Whether the script runs the FSL replicas' program (``LARMORX_NC_BIN``).
    nc: bool = False
    #: Whether the script takes ``--scratch <folder>``.
    scratch: bool = False
    #: Whether the script prints the originals' step (``STEP`` lines).
    step: bool = False
    #: The time limit of the run, in seconds.
    timeout: int = 3600
    #: Arguments computed when the case runs (``"oracle-images"``: the oracle runs' images).
    extra: str = ""


def _s(script, args, tier, description, **kw) -> Script:
    return Script(script, tuple(args), tier, description, **kw)


#: suite -> case id -> what the case runs.
SUITES: dict[str, dict[str, Script]] = {
    "ndimage-skimage": {
        "ndimage-quick": _s(
            "check_ndimage.py",
            ["--quick"],
            "smoke",
            "SciPy's ndimage filters, morphology, labels and sums, a quick draw of every section",
            step=True,
        ),
        "ndimage": _s(
            "check_ndimage.py",
            [],
            "standard",
            "SciPy's ndimage: every filter, binary and grey morphology, labels, sums, their "
            "arguments as SciPy's and SCI-09 to SCI-30 both ways",
            gib=8,
            step=True,
            timeout=5400,
        ),
        "numpy": _s(
            "check_numpy.py",
            [],
            "smoke",
            "numpy's percentiles, medians, clips and float32 exp on seeded arrays",
            step=True,
        ),
        "numpy-expf": _s(
            "check_numpy.py",
            ["--expf"],
            "full",
            "numpy's float32 exp on every float32 value (with numpy's AVX-512 kernels and "
            "without), its working files in the case's folder",
            gib=8,
            step=True,
            timeout=7200,
        ),
        "skimage": _s(
            "check_skimage.py",
            ["--sections", "ball,dot,hist,otsu"],
            "smoke",
            "scikit-image's ball and threshold_otsu, numpy's dot, norm and histogram",
            step=True,
        ),
        "skimage-random-walker": _s(
            "check_skimage.py",
            ["--sections", "rw"],
            "standard",
            "scikit-image's random_walker (both SciPy orders, copy=False, isolated labels)",
            gib=12,
            step=True,
        ),
        "scipy-inv-quick": _s(
            "check_scipy_inv.py",
            ["--n", "2000"],
            "smoke",
            "SciPy's inv on 2,000 seeded matrices per family and the edge cases",
            step=True,
        ),
        "scipy-inv": _s(
            "check_scipy_inv.py",
            ["--n", "20000"],
            "standard",
            "SciPy's inv on 20,000 seeded matrices per family, the edge cases and the oracle "
            "runs' image affines",
            gib=8,
            step=True,
            extra="oracle-images",
        ),
        "gaussian-filters": _s(
            "check_regrid.py",
            ["--sections", "filters"],
            "standard",
            "SciPy's correlate1d and Gaussian filters (derivatives, complex input, long-double "
            "sigma)",
            gib=8,
            step=True,
        ),
        "watershed-com": _s(
            "check_epi_mask.py",
            ["--sections", "watershed,com"],
            "standard",
            "SciPy's watershed_ift (SCI-W1 to SCI-W4 both ways) and center_of_mass",
            gib=16,
            scratch=True,
            step=True,
        ),
        "match-histograms": _s(
            "check_match_histogram.py",
            ["--sections", "arrays"],
            "smoke",
            "scikit-image's match_histograms on arrays (channels, types, errors)",
            scratch=True,
            step=True,
        ),
    },
    "niworkflows-helpers": {
        "merge": _s(
            "check_merge.py",
            ["--sections", "synth,errors,ras,gms"],
            "smoke",
            "IntraModalMerge: seeded series, errors, RAS reorientation, grand mean scaling",
        ),
        "merge-hmc": _s(
            "check_merge.py",
            ["--sections", "hmc,fmriprep"],
            "standard",
            "IntraModalMerge's head-motion correction (FSL's mcflirt replica) and fMRIPrep's "
            "magnitude merges",
            gib=12,
            nc=True,
        ),
        "nipype-command-lines": _s(
            "check_nipype.py",
            [],
            "smoke",
            "nipype's command lines of every program on these paths against every oracle "
            "run's command.txt",
        ),
        "regrid": _s(
            "check_regrid.py",
            ["--sections", "regrid,runs"],
            "standard",
            "RegridToZooms (SciPy's inv, gaussian_filter, nibabel) on seeded images and "
            "fMRIPrep's calls",
            gib=8,
            step=True,
        ),
        "intensity-clip": _s(
            "check_skimage.py",
            ["--sections", "clip"],
            "standard",
            "IntensityClip on fMRIPrep's calls",
            gib=12,
        ),
    },
    "sdcflows-estimation": {
        "masker": _s(
            "check_skimage.py",
            ["--sections", "masker"],
            "standard",
            "sdcflows' brain_masker on fMRIPrep's calls",
            gib=12,
        ),
        "masker-steps": _s(
            "check_masker_steps.py",
            [],
            "standard",
            "sdcflows' brain_masker step by step on fMRIPrep's calls",
            gib=12,
        ),
        "epi-mask": _s(
            "check_epi_mask.py",
            ["--sections", "synthetic,real"],
            "smoke",
            "sdcflows' epi_mask on synthetic images and four oracle runs' BOLD references",
            gib=16,
            scratch=True,
            step=True,
        ),
        "match-histogram": _s(
            "check_match_histogram.py",
            ["--sections", "synthetic,real"],
            "smoke",
            "sdcflows' match_histogram on synthetic images and the SyN runs' anatomy",
            gib=8,
            scratch=True,
            step=True,
        ),
        "workflows": _s(
            "check_workflows.py",
            [],
            "standard",
            "the brain-extraction, magnitude and field-map workflows (direct field maps)",
            gib=16,
            nc=True,
            timeout=5400,
        ),
        "pepolar-units": _s(
            "check_pepolar.py",
            ["--sections", "units,ravg"],
            "standard",
            "PEPOLAR's interfaces (readout times, SelectPEVolumes, SortPEBlips, UniformGrid, "
            "TOPUP's inputs, RobustAverage)",
            gib=24,
            nc=True,
        ),
        "pepolar-workflow": _s(
            "check_pepolar.py",
            ["--sections", "wf,image"],
            "full",
            "topup_workflow, both wirings (sdcflows 2.15 and 2.17.1), and fMRIPrep's own runs",
            gib=24,
            nc=True,
            timeout=7200,
        ),
        "contract-audit": _s(
            "relay22/audit_contract.py",
            [],
            "smoke",
            "the part's contract: every registered _core function has its stub, every public "
            "function of the part's families its API row, every source file its PROVENANCE line",
        ),
        "qwarp-steps": _s(
            "check_qwarp_steps.py",
            [],
            "smoke",
            "3dQwarp's Python steps (Flatten, _sorted_pe, qwarp2ants) and the workflow's refusal",
            scratch=True,
        ),
        "syn-preprocessing": _s(
            "check_syn_preprocessing.py",
            ["--sections", "nodes,library"],
            "standard",
            "init_syn_preprocessing_wf's nodes on fMRIPrep's --use-syn-sdc runs and its "
            "library steps",
            gib=16,
            timeout=5400,
        ),
        "syn-preprocessing-workflow": _s(
            "check_syn_preprocessing.py",
            ["--sections", "workflow"],
            "full",
            "sdcflows' own SyN preprocessing graph run by nipype",
            gib=16,
            timeout=7200,
        ),
        "syn-sdc": _s(
            "check_syn_sdc.py",
            ["--sections", "nodes"],
            "standard",
            "init_syn_sdc_wf's nodes on fMRIPrep's --use-syn-sdc estimators",
            gib=16,
        ),
        "syn-sdc-workflow": _s(
            "check_syn_sdc.py",
            ["--sections", "syn,workflow"],
            "full",
            "the SyN registration against ANTs 2.6.5 and sdcflows' own graph run by nipype",
            gib=16,
            timeout=7200,
        ),
        "estimators": _s(
            "check_estimators.py",
            ["--sections", "index,wrangler,units,fmriprep,runs,apps"],
            "standard",
            "lx.bids' index, sdcflows' wrangler and estimators, fMRIPrep's estimator "
            "selection on every oracle run",
            gib=8,
            scratch=True,
            timeout=5400,
        ),
        "estimators-image": _s(
            "check_estimators.py",
            ["--sections", "image"],
            "full",
            "the estimators against fMRIPrep's own image (Apptainer)",
            gib=8,
            scratch=True,
            timeout=5400,
        ),
        "derivatives": _s(
            "check_derivatives.py",
            ["--sections", "nodes", "workflow", "synthetic"],
            "standard",
            "DerivativesDataSink, build_path, the derivatives and fmap_preproc workflows",
            gib=16,
            scratch=True,
        ),
        "derivatives-preproc": _s(
            "check_derivatives.py",
            ["--sections", "preproc", "--datasets", "ds005752-sub-ON01016-direct"],
            "full",
            "init_fmap_preproc_wf run whole against sdcflows' own run",
            gib=16,
            nc=True,
            scratch=True,
            timeout=5400,
        ),
        "fmap-stage": _s(
            "check_fmap_stage.py",
            ["--sections", "graph", "nodes"],
            "standard",
            "larmorprepx's field-map estimation stage against fMRIPrep 25.2.6's graph and nodes",
            gib=12,
            scratch=True,
        ),
        "unwarp": _s(
            "check_unwarp.py",
            [],
            "standard",
            "unwarp_workflow (threads, cases, errors)",
            gib=10,
            scratch=True,
        ),
        "app": _s(
            "check_app.py",
            ["--sections", "parser", "toml", "settings"],
            "smoke",
            "sdcflows' BIDS app: its parser, TOML and settings",
            scratch=True,
        ),
        "app-dry-runs": _s(
            "check_app.py",
            ["--sections", "dryrun", "exits"],
            "standard",
            "sdcflows' BIDS app: dry runs and exit statuses",
            gib=16,
            nc=True,
            scratch=True,
        ),
        "app-run": _s(
            "check_app.py",
            ["--sections", "run"],
            "full",
            "sdcflows' BIDS app run whole on a dataset, against sdcflows' own",
            gib=16,
            nc=True,
            scratch=True,
            timeout=5400,
        ),
        "bold-fmap": _s(
            "check_bold_fmap.py",
            [],
            "standard",
            "fMRIPrep's BOLD field-map nodes (coregistration, skull strip, coeff2epi)",
            gib=12,
            nc=True,
        ),
        "bold-steps": _s(
            "check_bold_steps.py",
            [],
            "standard",
            "fMRIPrep's DistortionParameters, ConcatenateXFMs, reconstruct_fieldmap, unwarp",
            gib=8,
        ),
        "bold-stage": _s(
            "check_bold_stage.py",
            [],
            "standard",
            "larmorprepx's BOLD fit field-map stages against fMRIPrep's files (PIPE-20)",
            gib=12,
            nc=True,
            timeout=5400,
        ),
    },
}

#: What each suite says of itself: title, tool, reference.
ABOUT = {
    "ndimage-skimage": (
        "SciPy's ndimage, SciPy's inv, scikit-image and numpy on the field maps' paths",
        "`lx.ndimage` (`crates/larmorx-ndimage`), `lx.skimage`, `larmorx.transforms._linalg."
        "scipy_inv`, `larmorx_core::numpy`",
        "SciPy, numpy (on OpenBLAS's Haswell kernels, one thread) and scikit-image 0.26.0 "
        "in-process, in the environment's releases",
    ),
    "niworkflows-helpers": (
        "niworkflows' helpers and nipype's command lines on the field maps' paths",
        "`lx.niworkflows`, `larmorx._nipype`",
        "niworkflows 1.15.4 and nipype in-process (sdcflows 2.17.1's calls), and fMRIPrep "
        "25.2.6's own files and command lines of the oracle runs",
    ),
    "sdcflows-estimation": (
        "sdcflows' field-map estimation and correction, its BIDS app and fMRIPrep's own "
        "field-map steps",
        "`lx.sdcflows` (`crates/larmorx-sdcflows`), `lx.bids`, larmorprepx's field-map stages",
        "sdcflows 2.17.1 (and 2.15's PEPOLAR wiring) in-process with nipype, fMRIPrep 25.2.6's "
        "own graphs, files and command lines of the oracle runs, and sdcflows 2.17.1's own "
        "runs; the programs underneath through larmorx's ports and the oracle cache",
    ),
}

_TALLY = re.compile(
    r"^(?P<name>.+?): (?P<n>\d+) (?P<unit>[a-z]+), identical (?P<k>\d+)(?P<rest>.*)$"
)
_INV = re.compile(r"^(?P<name>.+?): (?P<n>\d+) cases, (?P<k>\d+) identical(?P<rest>.*)$")
_SECTION = re.compile(r"^== (?P<name>.+?): (?P<k>\d+) identical, (?P<m>\d+) differ(?P<rest>.*)$")
_SAME = re.compile(r"^(?P<name>[^:]+): (?P<what>same|refused\b.*)$")
_GAPS = re.compile(r"^(?P<name>[a-z]+): (?P<rest>.*\b(?P<g>\d+) (?:gaps|not named))$")
_FIRST = re.compile(r"^\s+first difference: (?P<what>.*)$")
_STEP = re.compile(r"^STEP (?P<hex>[0-9a-f]{64}) (?P<name>.+)$")
_FINAL_BAD = re.compile(r"^(FAIL \d+|FAILED \d+|DIFFERENCES: \d+)")
_TIMES = re.compile(r"\b\d+(?:\.\d+)? ?(?:s|ms|min)\b")


def _workspace() -> Path | None:
    return pv.workspace()


def _oracle_images(ws: Path) -> list[str]:
    """The images of every fMRIPrep oracle run's outputs (their affines are SciPy's ``inv``
    inputs in nilearn's ``resample_img``)."""
    root = ws / "oracles" / "fmriprep-25.2.6"
    return sorted(str(p) for p in root.glob("*/out/**/*.nii.gz"))


def _nc_binary() -> Path | None:
    from larmorx_validation.parity.fsl_mcflirt_replica import replica_binary

    return replica_binary()


def _paths() -> list[str]:
    """This larmorx's and this validation package's folders (the scripts import both)."""
    import larmorx
    import larmorx_validation

    out = [str(Path(larmorx.__file__).resolve().parents[1])]
    out.append(str(Path(larmorx_validation.__file__).resolve().parents[1]))
    return out


def _environment(spec: Script, ws: Path, folder: Path) -> dict[str, str]:
    env = {k: v for k, v in os.environ.items() if k not in ("LARMORX_ORACLE_SHIM", STEP_LINES)}
    old = [p for p in env.get("PYTHONPATH", "").split(os.pathsep) if p]
    env["PYTHONPATH"] = os.pathsep.join(dict.fromkeys([*_paths(), *old]))
    env["LARMORX_WORKSPACE"] = str(ws)
    env["OPENBLAS_CORETYPE"] = "Haswell"
    env["OPENBLAS_NUM_THREADS"] = "1"
    env["OMP_NUM_THREADS"] = "1"
    env["TMPDIR"] = str(folder / "tmp")
    env["PYTHONUNBUFFERED"] = "1"
    if spec.nc:
        nc = _nc_binary()
        if nc is None:
            raise Outcome(SKIPPED, "the FSL replicas' program (larmorx-nc) is not built")
        env["LARMORX_NC_BIN"] = str(nc)
    if spec.step and (os.environ.get(pv.STEP_RECORD) or os.environ.get(pv.STEP_FROM)):
        env[STEP_LINES] = "1"
    return env


def _command(spec: Script, ws: Path, folder: Path) -> list[str]:
    cmd = [sys.executable, "-P", str(SCRIPTS / spec.script), *spec.args]
    if spec.scratch:
        cmd += ["--scratch", str(folder / "scratch")]
    if spec.extra == "oracle-images":
        images = _oracle_images(ws)
        if images:
            cmd += ["--images", *images]
    if spec.gib and _box() is None and BOXED.is_file() and shutil.which("systemd-run"):
        cmd = ["bash", str(BOXED), str(spec.gib), *cmd]
    return cmd


def _box() -> int | None:
    """The memory limit (bytes) of the box this process runs in, if any: a control group of
    its own or an enclosing one with a finite ``memory.max``. Inside a box the cases run in it
    (a new scope would leave it: systemd puts a user scope beside the box, not in it)."""
    from larmorx_validation.difftest import limits

    group = limits.own_cgroup()
    while group is not None and group != limits.CGROUP_ROOT and group != group.parent:
        try:
            value = (group / "memory.max").read_text(encoding="utf-8").strip()
        except OSError:
            value = "max"
        if value != "max":
            return int(value)
        group = group.parent
    return None


def _reported(text: str) -> str:
    """A reported line without its times (they differ from run to run) and without the
    machine's paths (records are published)."""
    from larmorx_validation.report import redact

    return _TIMES.sub("<time>", redact(text.strip()))[:300]


class _Parsed:
    """The checks of a script's output, in order (names made unique)."""

    def __init__(self) -> None:
        self.items: list[dict] = []
        self.steps: list[tuple[str, str]] = []
        self._names: dict[str, int] = {}

    def add(self, name: str, **kw) -> None:
        name = name.strip()
        k = self._names.get(name, 0)
        self._names[name] = k + 1
        if k:
            name = f"{name} #{k + 1}"
        self.items.append({"name": name, **kw})

    @property
    def comparisons(self) -> int:
        return sum(1 for i in self.items if i.get("compared"))


def parse(stdout: str) -> _Parsed:
    """The checks of a script's output (see the module's description)."""
    out = _Parsed()
    for raw in stdout.splitlines():
        line = raw.rstrip()
        s = line.strip()
        if not s:
            continue
        if m := _STEP.match(s):
            out.steps.append((m.group("name").strip(), m.group("hex")))
            continue
        if s.startswith("INFO "):
            body = s[5:]
            name, _, value = body.partition(": ")
            out.add(
                f"INFO {name[:120]}",
                passed=True,
                threshold="reported",
                value=_reported(value or body),
            )
            continue
        if s.startswith("NOTICES "):
            name, _, value = s.partition(": ")
            out.add(name, passed=True, threshold="reported", value=_reported(value))
            continue
        if s.startswith("EXPECTED "):
            body = s[9:]
            name, _, value = body.partition(": ")
            out.add(
                f"EXPECTED {name[:120]}",
                passed=True,
                threshold="reported (an expected difference: a registered defect corrected)",
                value=_reported(value or body),
            )
            continue
        if s.startswith(("TIME ", "time ")):
            continue
        if m := _FIRST.match(line):
            if out.items:
                out.items[-1]["detail"] = (out.items[-1].get("detail", "") + " " + m["what"])[:400]
            continue
        if s.startswith("DIFF "):
            name, _, what = s[5:].partition(": ")
            out.add(f"DIFF {name}", passed=False, detail=what[:400], compared=True)
            continue
        if m := _INV.match(s):
            n, k, rest = int(m["n"]), int(m["k"]), m["rest"]
            differ = re.search(r"\b(\d+) differ\b", rest)
            rc = re.search(r"rcond (\d+)/(\d+) identical", rest)
            ok = (differ is None or differ.group(1) == "0") and (
                rc is None or rc.group(1) == rc.group(2)
            )
            if differ is None:
                ok = ok and k == n
            out.add(
                m["name"], passed=ok, value=f"{k}/{n}", detail=rest.strip(", ")[:400], compared=True
            )
            continue
        if m := _TALLY.match(s):
            n, k = int(m["n"]), int(m["k"])
            out.add(
                m["name"],
                passed=k == n,
                value=f"{k}/{n}",
                metric=f"identical {m['unit']}",
                detail=m["rest"].strip()[:400],
                compared=True,
            )
            continue
        if m := _SECTION.match(s):
            k, d = int(m["k"]), int(m["m"])
            out.add(f"== {m['name']}", passed=d == 0, value=f"{k}/{k + d}", compared=True)
            continue
        if m := _GAPS.match(s):
            out.add(f"contract: {m['name']}", passed=m["g"] == "0", value=m["rest"][:200],
                    compared=True)  # fmt: skip
            continue
        if m := _SAME.match(s):
            out.add(m["name"], passed=True, value=m["what"][:200], compared=True)
            continue
    return out


def cases(suite: str, tier: str) -> list[Case]:
    from larmorx_validation.parity.harness import in_tier

    return [
        Case(cid, spec.script.removesuffix(".py"), spec.description, payload=(suite, cid))
        for cid, spec in SUITES[suite].items()
        if in_tier(spec.tier, tier)
    ]


def run_case(case: Case, checks: CheckList) -> None:
    suite, cid = case.payload
    spec = SUITES[suite][cid]
    if os.environ.get(pv.STEP_RECORD) and not spec.step:
        raise Outcome(SKIPPED, "the originals' own step runs the library scripts only")
    ws = _workspace()
    if ws is None:
        raise Outcome(SKIPPED, "the workspace (its oracles/ folder) was not found")
    folder = Path(tempfile.mkdtemp(prefix=f"lxpb_{cid.replace('-', '_')}_"))
    (folder / "tmp").mkdir()
    keep = False
    try:
        env = _environment(spec, ws, folder)
        cmd = _command(spec, ws, folder)
        try:
            done = subprocess.run(
                cmd,
                env=env,
                cwd=str(folder),
                capture_output=True,
                text=True,
                encoding="utf-8",
                errors="replace",
                timeout=spec.timeout,
                check=False,
            )
        except subprocess.TimeoutExpired as exc:
            keep = True
            tail = (exc.stdout or "")[-600:] if isinstance(exc.stdout, str) else ""
            checks(
                "the script ends within its time limit", False, detail=f"{spec.timeout} s; {tail}"
            )
            return
        parsed = parse(done.stdout)
        with pv.case(f"{suite}/{cid}"):
            for item in parsed.items:
                checks(
                    item["name"],
                    item["passed"],
                    metric=item.get("metric", "exact"),
                    value=item.get("value"),
                    threshold=item.get("threshold"),
                    detail=item.get("detail", ""),
                )
            for name, hexdigest in parsed.steps:
                _step(checks, name, hexdigest, REASONS.get(f"{suite}/{cid}", {}))
        lines = [ln.strip() for ln in done.stdout.splitlines() if ln.strip()]
        final = next((ln for ln in reversed(lines) if ln == "ALL OK" or _FINAL_BAD.match(ln)), None)
        err_tail = " | ".join(done.stderr.strip().splitlines()[-6:])[:600]
        checks(
            "the script's summary: ALL OK",
            final == "ALL OK",
            detail=final or f"no summary line; standard error: {err_tail}",
        )
        checks(
            "the script's exit status",
            done.returncode == 0,
            value=done.returncode,
            detail=f"{parsed.comparisons} comparison lines"
            + ("" if done.returncode == 0 else f"; standard error: {err_tail}")
            + ("; 137: the memory box stopped it" if done.returncode == 137 else ""),
        )
        checks(
            "the script compared something",
            parsed.comparisons > 0,
            detail=f"{parsed.comparisons} comparison lines (a run that compares nothing is a "
            "misconfigured case: its oracle runs or arguments)",
        )
        keep = done.returncode != 0 or final != "ALL OK"
    finally:
        if keep:
            checks.add(
                Check(
                    "the case's folder (kept for a failure)",
                    True,
                    threshold="reported",
                    detail=str(folder),
                )
            )
        else:
            shutil.rmtree(folder, ignore_errors=True)


def _step(checks: CheckList, name: str, hexdigest: str, reasons: dict[str, str]) -> None:
    """The originals' own step for comparison ``name`` (``python_versions``): recorded in a
    ``shared`` step run, compared with that record in a ``newest`` run; a change needs a
    known reason (:data:`CHANGES`)."""
    value = np.frombuffer(bytes.fromhex(hexdigest), dtype=np.uint8)
    pv.record(name, value)
    olds = pv.previous(name)
    if not olds:
        return
    if len(set(olds)) > 1:
        checks.add(
            Check(
                f"{pv.STEP_CHECK}: {name}",
                True,
                metric="the originals' outputs",
                value=pv.VARIES,
                threshold="a change has a known reason",
                detail=f"{len(set(olds))} different outputs in {len(olds)} runs of the shared "
                "releases on the same inputs, so the step is not measured",
            )
        )
        return
    changed = olds[0] != pv.digest(value)
    why = (
        list(dict.fromkeys(v for k, v in reasons.items() if re.search(k, name))) if changed else []
    )
    checks.add(
        Check(
            f"{pv.STEP_CHECK}: {name}",
            (not changed) or bool(why),
            metric="the originals' outputs",
            value="changed" if changed else "identical",
            threshold="a change has a known reason",
            detail=(
                "identical"
                if not changed
                else "changed: " + ("; ".join(CHANGES[k] for k in why) or "no known reason")
            ),
        )
    )


def _highlights(results: list[CaseResult]) -> list[str]:
    lines = []
    compared = sum(
        1
        for r in results
        for c in r.checks
        if c.threshold is None and not c.name.startswith(("the script", pv.STEP_CHECK))
    )
    lines.append(f"{compared} comparison lines in {len(results)} script runs.")
    stepped = [c for r in results for c in r.checks if c.name.startswith(pv.STEP_CHECK)]
    if stepped:
        changed = sum(1 for c in stepped if c.value == "changed")
        varies = sum(1 for c in stepped if c.value == pv.VARIES)
        lines.append(
            f"The originals' own step ({pv.RELEASES[pv.SHARED]} → {pv.RELEASES[pv.NEWEST]}): "
            f"{len(stepped)} comparisons, {changed} changed with a known reason, {varies} "
            "varying in the shared releases, the rest identical."
        )
    return lines


def make_suite(name: str) -> Suite:
    import numpy

    title, tool, reference = ABOUT[name]
    return Suite(
        name=name,
        title=title,
        tool=tool,
        reference=f"{reference} (numpy {numpy.__version__})",
        thresholds=[
            (
                "Every array, file (decompressed), header, warning, message and command line",
                "bit-identical",
            ),
            (
                "upstream_defects='reproduce'",
                "the original's result bit for bit, its errors and warnings",
            ),
            (
                "A corrected defect (the default)",
                "the correction the register names, checked "
                "against an independent construction; an EXPECTED line names it",
            ),
        ],
        cases=lambda tier: cases(name, tier),
        run_case=run_case,
        packages=(
            "numpy",
            "scipy",
            "scikit-image",
            "nibabel",
            "nipype",
            "niworkflows",
            "sdcflows",
            "nilearn",
        ),
        highlights=_highlights,
        notes=(
            "Each case is one of the investigation's check scripts (some of its sections), run "
            "as a separate process of the environment's interpreter; one check per summary "
            "line it prints, its final line and its exit status. The library scripts carry the "
            "originals' own step (python_versions)."
        ),
    )
