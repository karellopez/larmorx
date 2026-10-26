# SPDX-License-Identifier: Apache-2.0
"""Parity of ``larmorx ants Atropos`` / ``lx.ants.atropos`` with ANTs 2.6.5's ``Atropos``.

Every case runs the same command line through the ANTs oracle and through larmorx (in
process, with ANTs' defects reproduced, ``LARMORX_UPSTREAM_DEFECTS=reproduce``), and compares:

- the **exit status**, and what both print where it matters (error messages, ``-v`` output
  without the ITK base classes' lines, which hold addresses, times and thread settings);
- every **output** (the classified image, the posteriors, and the likelihood, distance and
  B-spline images where asked): header field by field, type, shape, affine, values.

Two references, as for N4 and DenoiseImage:

- **ANTs as built** (glibc's ``expf``/``logf``/``exp``/``log``/``pow``), one thread: the
  unmodified program, the headline. The classified images are compared voxel by voxel; the
  posteriors may differ in the last bits where glibc's functions are not correctly rounded
  for a value the run meets: their largest difference and NCC are reported.
- **ANTs with correctly rounded math** (CORE-MATH preloaded into the same binary), one
  thread: what larmorx computes with (CLAUDE.md rule 5). larmorx must be **bit-identical**.

Atropos' own variability: its **thread band** (ANTs at 2, 4 and 8 threads, or as
``LARMORX_PARITY_BAND_THREADS`` says, compared with one
thread) and its **seed band** (``--use-random-seed 1`` seeds ANTs from the clock: two ANTs
runs differ; larmorx never reads the clock and uses the seed 121212, so its distance from
ANTs is reported next to ANTs' distance from itself). larmorx's result does not depend on
its thread count (checked at 4 and 1 threads). A **fix pass** runs larmorx in its default
mode and reports what the corrections of ANTs' defects change (D13).

The real cases repeat sMRIPrep's Atropos call from the five fMRIPrep 25.2.6 oracle runs
(``oracles/fmriprep-25.2.6/<run>/work/.../brain_extraction_wf/atropos_wf/01_atropos``: K-means
with three classes, Gaussian likelihoods, MRF 0.1, three iterations, the fixed seed, on the
N4-corrected T1w and the OASIS30ANTs brain mask) and also report the distance to the files
fMRIPrep wrote (its container's ANTs build, 8 threads). antsCorticalThickness-style
segmentations use the OASIS30ANTs tissue priors (CSF, cortical GM, WM; and all six) brought
into each subject's space with the transform fMRIPrep's brain extraction computed (ANTs'
``antsApplyTransforms``, as fMRIPrep's ``map_brainmask`` does for the brain prior).
"""

from __future__ import annotations

import functools
import os
import re
import subprocess
import tempfile
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np

from larmorx_validation.parity import ants_inputs, defects
from larmorx_validation.parity.ants_programs import (
    ants_bin,
    ants_version,
    crmath_preload,
    header_differences,
    raw_header,
)
from larmorx_validation.parity.harness import (
    BOTH_ERROR,
    EXPECTED,
    PASS,
    SKIPPED,
    Case,
    CaseResult,
    Check,
    CheckList,
    Outcome,
    Suite,
    in_tier,
)
from larmorx_validation.report import table

PROGRAM = "Atropos"
LX_THREADS = 4
#: ANTs' thread band: 2, 4 and 8 threads unless ``LARMORX_PARITY_BAND_THREADS`` (e.g.
#: ``2,4,6`` on a shared machine) says otherwise.
BAND_THREADS = tuple(
    int(v) for v in os.environ.get("LARMORX_PARITY_BAND_THREADS", "2,4,8").split(",") if v
)
NCC_MIN = 0.995
AFFINE_ATOL = 1e-6
ORACLE_RUNS = "oracles/fmriprep-25.2.6"
TEMPLATEFLOW = "oracles/templateflow"
#: The standard outputs: the classified image and three posteriors.
OUT3 = ("seg.nii.gz", "post01.nii.gz", "post02.nii.gz", "post03.nii.gz")
O3 = ("-o", "[{out}/seg.nii.gz,{out}/post%02d.nii.gz]")


@dataclass(frozen=True)
class AtroposCase:
    """One ``Atropos`` command line, compared with larmorx's.

    ``args`` use placeholders: ``{out}`` the program's output directory; ``{gen:name}`` a
    generated input; ``{series:name}`` the ``%02d`` pattern of generated inputs
    ``name01``, ``name02``, ...; ``{missing:name}`` a file that does not exist;
    ``{oracle:run/path}`` a file of an fMRIPrep oracle run (``work/...``);
    ``{priors:run:n}`` the ``%02d`` pattern of the OASIS30ANTs tissue priors (3 or 6) in
    ``run``'s T1w space; ``{fmriprep:run/node}`` (alone) the command line of an fMRIPrep node,
    its outputs written to ``{out}``. ``outputs`` are the files compared (in ``{out}``).

    ``expect``:

    - ``"pass"``: both succeed; larmorx bit-identical to ANTs with correctly rounded math;
    - ``"error"``: both exit with the same status and messages;
    - ``"abort"``: ANTs aborts or crashes (a signal); larmorx prints what ANTs printed (an
      uncaught exception's text) and exits 1;
    - ``"divergence"``: a documented difference (``reason``);
    - ``"random"``: ANTs seeds from the clock: two ANTs runs, and larmorx's distance from them.

    ``stdout`` compares standard output (``-v``: without the ITK base classes' lines);
    ``band`` runs ANTs' thread band; ``fmriprep`` names the fMRIPrep node whose outputs are
    also compared. ``silent_ok`` and ``one_mode`` are as for the other ANTs suites;
    ``undefined`` says why ANTs' values depend on memory it never wrote (reported only).
    """

    id: str
    category: str
    description: str
    args: tuple[str, ...]
    tier: str = "smoke"
    expect: str = "pass"
    reason: str = ""
    outputs: tuple[str, ...] = OUT3
    stdout: bool = False
    band: bool = False
    fmriprep: str = ""
    #: ITK-50 changes k-means' starting centres; the final result may not show it.
    silent_ok: tuple[str, ...] = ("ITK-50",)
    one_mode: tuple[str, ...] = field(default=())
    #: Why ANTs' values depend on memory it never wrote: they are reported, not required to
    #: match (everything else is compared).
    undefined: str = ""
    #: Score the labels against the partial-volume phantom's truth (ANTS-106; reported).
    pv_truth: bool = False


# ------------------------------------------------------------------------------------------------
# Generated inputs: a three-tissue phantom and what Atropos' options need

SHAPE = (40, 44, 36)


def _affine() -> np.ndarray:
    a = np.diag([1.2, 1.0, 1.5, 1.0])
    a[:3, 3] = [-24.0, -22.0, -27.0]
    return a


def _save(data: np.ndarray, target: Path, affine: np.ndarray | None = None) -> None:
    ants_inputs.save(np.asarray(data), target, affine=_affine() if affine is None else affine)


@functools.cache
def _phantom() -> dict[str, np.ndarray]:
    """Three tissues in nested spheroids (60, 110, 160) over a background of 20, a smooth bias
    and Gaussian noise; a second contrast; the labels and the mask (seeded)."""
    rng = np.random.default_rng(1234)
    i, j, k = np.meshgrid(*[np.arange(s, dtype=np.float64) for s in SHAPE], indexing="ij")
    c = np.array(SHAPE) / 2.0
    r = np.sqrt(((i - c[0]) * 1.2) ** 2 + ((j - c[1]) * 1.0) ** 2 + ((k - c[2]) * 1.5) ** 2)
    labels = np.zeros(SHAPE, np.int16)
    labels[r < 24] = 1
    labels[r < 18] = 2
    labels[r < 11] = 3
    mask = (r < 25).astype(np.uint8)
    t1 = np.choose(labels, [20.0, 60.0, 110.0, 160.0])
    bias = 1.0 + 0.15 * (i / SHAPE[0]) - 0.1 * (k / SHAPE[2])
    t1 = (t1 * bias + rng.normal(0, 9.0, SHAPE)).astype(np.float32)
    t2 = np.choose(labels, [80.0, 120.0, 70.0, 30.0])
    t2 = (t2 + rng.normal(0, 7.0, SHAPE)).astype(np.float32)
    tensors = np.zeros((*SHAPE, 6), np.float32)
    evals = {0: (1.0, 1.0, 1.0), 1: (2.0, 1.0, 0.5), 2: (1.5, 1.5, 0.7), 3: (3.0, 0.6, 0.4)}
    for lab, (a, b, cc) in evals.items():
        tensors[labels == lab] = np.array([a, 0.0, 0.0, b, 0.0, cc], np.float32)
    tensors += rng.normal(0, 0.05, tensors.shape).astype(np.float32)
    return {"t1": t1, "t2": t2, "labels": labels, "mask": mask, "i": i, "tensors": tensors}


def _smooth(x: np.ndarray, passes: int = 4) -> np.ndarray:
    """Binomial ([1, 2, 1] / 4) smoothing along every axis, ``passes`` times, edges repeated."""
    y = x.astype(np.float64)
    for _ in range(passes):
        for axis in range(y.ndim):
            p = np.pad(y, [(1, 1) if a == axis else (0, 0) for a in range(y.ndim)], mode="edge")
            lo = np.take(p, range(0, y.shape[axis]), axis=axis)
            mid = np.take(p, range(1, y.shape[axis] + 1), axis=axis)
            hi = np.take(p, range(2, y.shape[axis] + 2), axis=axis)
            y = (lo + 2 * mid + hi) / 4
    return y


@functools.cache
def _priors() -> list[np.ndarray]:
    ph = _phantom()
    smooth = [_smooth((ph["labels"] == c).astype(np.float64)) for c in (1, 2, 3)]
    total = np.sum(smooth, axis=0) + 1e-6
    return [(p / total * ph["mask"]).astype(np.float32) for p in smooth]


PV_SHAPE = (40, 40, 40)


@functools.cache
def _pv_phantom() -> dict[str, np.ndarray]:
    """Nested spheres (radii 8.3, 13.7, 17.9 voxels: values 200, 130, 60, over 0) rendered on a
    grid five times finer and box-averaged, so every voxel's tissue fractions are known; noise
    of sd 6 (seeded). The truth: a tissue where one tissue's share (of the three) is at least
    0.8, else the partial-volume class of its two largest (4 = 1x2, 5 = 2x3)."""
    n, up = PV_SHAPE[0], 5
    c = (n - 1) / 2.0
    g = (np.indices((n * up,) * 3) + 0.5) / up - 0.5
    r = np.sqrt(((g - c) ** 2).sum(0))
    tissue = np.where(r < 8.3, 3, np.where(r < 13.7, 2, np.where(r < 17.9, 1, 0)))
    frac = np.stack([(tissue == k).reshape(n, up, n, up, n, up).mean((1, 3, 5)) for k in range(4)])
    img = np.tensordot(np.array([0.0, 60.0, 130.0, 200.0]), frac, 1)
    img = img + np.random.default_rng(7).normal(scale=6.0, size=img.shape)
    mask = (frac[0] < 0.5).astype(np.uint8)
    f = frac[1:] / np.maximum(frac[1:].sum(0), 1e-9)
    order = np.argsort(-f, 0)
    top, second = order[0] + 1, order[1] + 1
    lo, hi = np.minimum(top, second), np.maximum(top, second)
    truth = np.zeros(PV_SHAPE, np.int16)
    pure = f.max(0) >= 0.8
    truth[pure] = top[pure]
    truth[~pure & (lo == 1) & (hi == 2)] = 4
    truth[~pure & (lo == 2) & (hi == 3)] = 5
    truth[mask == 0] = 0
    return {"t1": img.astype(np.float32), "mask": mask, "truth": truth}


def _pv_save(data: np.ndarray, t: Path) -> None:
    ants_inputs.save(np.asarray(data), t, affine=np.eye(4))


@ants_inputs.builder("atropos-pv")
def _(t):
    """Atropos' partial-volume phantom: nested spheres (200, 130, 60 over 0) box-averaged from
    a five-times finer grid, so each voxel's tissue fractions are known; noise sd 6, 40^3."""
    _pv_save(_pv_phantom()["t1"], t)


@ants_inputs.builder("atropos-pv-mask")
def _(t):
    """The partial-volume phantom's mask: voxels less than half background (uint8)."""
    _pv_save(_pv_phantom()["mask"], t)


def _pv_scores(seg: Path) -> str:
    """Dice per class (1-3 tissues, 4 = 1x2, 5 = 2x3) and the share of voxels labelled as
    the truth says."""
    labels = _load(seg)[0]
    truth = _pv_phantom()["truth"]
    dice = []
    for k in (1, 2, 3, 4, 5):
        a, b = labels == k, truth == k
        dice.append(2 * float((a & b).sum()) / max(float(a.sum() + b.sum()), 1.0))
    inside = truth > 0
    accuracy = float((labels[inside] == truth[inside]).mean())
    return (
        f"Dice {' '.join(f'{d:.3f}' for d in dice)} (classes 1-5), "
        f"{accuracy:.3f} of the voxels as the truth"
    )


@ants_inputs.builder("atropos-t1")
def _(t):
    """Atropos' phantom: three tissues (60, 110, 160) in nested spheroids over a background of
    20, a smooth bias field and Gaussian noise (sd 9), 40 x 44 x 36 at 1.2 x 1 x 1.5 mm."""
    _save(_phantom()["t1"], t)


@ants_inputs.builder("atropos-t2")
def _(t):
    """A second contrast of the phantom (120, 70, 30 over 80; noise sd 7)."""
    _save(_phantom()["t2"], t)


@ants_inputs.builder("atropos-mask")
def _(t):
    """The phantom's mask (uint8, 1 inside)."""
    _save(_phantom()["mask"], t)


@ants_inputs.builder("atropos-mask255")
def _(t):
    """The phantom's mask with the value 255 (Atropos labels the mask's voxels by its label)."""
    _save((_phantom()["mask"] * 255).astype(np.uint8), t)


@ants_inputs.builder("atropos-mask-float")
def _(t):
    """The phantom's mask as float32 with 0.6 at the rim (Atropos reads masks as unsigned
    int: 0.6 is 0)."""
    m = _phantom()["mask"].astype(np.float32)
    rim = (m > 0) & (_smooth(m, 1) < 0.99)
    _save(np.where(rim, np.float32(0.6), m), t)


for _n in (1, 2, 3):

    @ants_inputs.builder(f"atropos-prior{_n:02d}")
    def _(t, n=_n):
        """Tissue n's prior probability: the smoothed indicator of the tissue, normalized over
        the three, zero outside the mask."""
        _save(_priors()[n - 1], t)


@ants_inputs.builder("atropos-priors-vector")
def _(t):
    """The three priors as one vector image (intent ``vector``, 5D: x, y, z, 1, 3)."""
    import nibabel as nib

    vec = np.stack(_priors(), axis=-1)[:, :, :, None, :]
    img = nib.Nifti1Image(vec, _affine())
    img.header.set_intent("vector")
    img.to_filename(t)


@ants_inputs.builder("atropos-prior-label")
def _(t):
    """The phantom's labels with every seventh voxel (by i + j + k) unlabelled (int16)."""
    ph = _phantom()
    idx = np.indices(SHAPE).sum(axis=0)
    _save(np.where(idx % 7 == 0, 0, ph["labels"]).astype(np.int16), t)


for _q in range(6):

    @ants_inputs.builder(f"atropos-tensor{_q}")
    def _(t, q=_q):
        """Component q of a diffusion-like tensor per tissue (xx, xy, xz, yy, yz, zz)."""
        _save(_phantom()["tensors"][..., q], t)


@ants_inputs.builder("atropos-mrf-coefficients")
def _(t):
    """An MRF coefficient image rising from 0.05 to 0.25 along the first axis."""
    ph = _phantom()
    _save((0.05 + 0.2 * (ph["i"] / SHAPE[0])).astype(np.float32), t)


@ants_inputs.builder("atropos-t1-outliers")
def _(t):
    """The phantom with six bright outliers (+1800 to +3800) in the brightest tissue."""
    ph = _phantom()
    data = ph["t1"].copy().ravel(order="F")
    idx = np.flatnonzero(ph["labels"].ravel(order="F") == 3)
    pick = np.random.default_rng(11).choice(idx, 6, replace=False)
    data[pick] += np.array([90, 110, 130, 150, 170, 190], np.float32) * 20
    _save(data.reshape(SHAPE, order="F"), t)


@ants_inputs.builder("atropos-t1-2d")
def _(t):
    """The phantom's middle slice (2D)."""
    _save(_phantom()["t1"][:, :, 18], t, affine=np.diag([1.2, 1.0, 1.0, 1.0]))


@ants_inputs.builder("atropos-mask-2d")
def _(t):
    """The mask's middle slice (2D)."""
    _save(_phantom()["mask"][:, :, 18], t, affine=np.diag([1.2, 1.0, 1.0, 1.0]))


@ants_inputs.builder("atropos-t1-4d")
def _(t):
    """Two volumes of 16 slices of the phantom (the second 1 % brighter plus 2), TR 1 s."""
    v = _phantom()["t1"][:, :, 10:26]
    _save(np.stack([v, v * 1.01 + 2], axis=-1).astype(np.float32), t)


@ants_inputs.builder("atropos-mask-4d")
def _(t):
    """The mask over the same 16 slices, twice (4D)."""
    m = _phantom()["mask"][:, :, 10:26]
    _save(np.stack([m, m], axis=-1).astype(np.uint8), t)


# ------------------------------------------------------------------------------------------------
# Real inputs: the fMRIPrep oracle runs


@functools.cache
def _oracle_root() -> Path | None:
    for parent in Path(__file__).resolve().parents:
        if (parent / ORACLE_RUNS).is_dir():
            return parent / ORACLE_RUNS
    return None


@functools.cache
def _templateflow() -> Path | None:
    for parent in Path(__file__).resolve().parents:
        if (parent / TEMPLATEFLOW / "tpl-OASIS30ANTs").is_dir():
            return parent / TEMPLATEFLOW
    return None


@functools.cache
def _priors_dir() -> Path:
    return Path(tempfile.mkdtemp(prefix="lx-atropos-priors-"))


#: The five runs and their subject's workflow directory.
RUNS = {
    "ds000005-sub-01-noreconall": "sub_01_wf",
    "ds001600-sub-1-noreconall": "sub_1_wf",
    "ds005040-sub-002-noreconall": "sub_002_wf",
    "ds005454-sub-16-noreconall": "sub_16_wf",
    "ds005752-sub-ON01016-noreconall": "sub_ON01016_ses_01_wf",
}
BRAIN_EXTRACTION = "anat_fit_wf/brain_extraction_wf"
ATROPOS_NODE = BRAIN_EXTRACTION + "/atropos_wf/01_atropos"


def _work(run: str) -> Path:
    """The subject's workflow directory of an oracle run (any finished run: the data panel's
    tier reads all fourteen)."""
    root = _oracle_root()
    if root is None or not (root / run / "work").is_dir():
        raise Outcome(SKIPPED, f"the fMRIPrep oracle run {run} is not here")
    wf = root / run / "work" / "fmriprep_25_2_wf"
    if run in RUNS:
        return wf / RUNS[run]
    found = sorted(p for p in wf.glob("sub_*_wf") if p.is_dir())
    if len(found) != 1:
        raise Outcome(SKIPPED, f"{run}: {len(found)} subject workflows in its working directory")
    return found[0]


def _node_args(run: str, node: str, out: Path) -> list[str]:
    """An fMRIPrep node's Atropos command line, paths mapped to the oracle run, the outputs
    written to ``out``."""
    work = _work(run)
    words = (work / node / "command.txt").read_text(encoding="utf-8").split()[1:]
    root = work.parents[1]
    args = [w.replace("/work/", f"{root}/") for w in words]
    i = args.index("--output")
    args[i + 1] = f"[{out / 'seg.nii.gz'},{out / 'post%02d.nii.gz'}]"
    return args


def _node_inputs(run: str) -> tuple[Path, Path]:
    """sMRIPrep's Atropos inputs: the N4-corrected T1w and the brain mask."""
    args = _node_args(run, ATROPOS_NODE, Path("/unused"))
    image = Path(args[args.index("--intensity-image") + 1])
    mask = Path(args[args.index("--mask-image") + 1])
    return image, mask


OASIS_CLASSES = {3: ("CSF", "CGM", "WM"), 6: ("CSF", "CGM", "WM", "SCGM", "BS", "CBM")}
#: SHA-256 of TemplateFlow's ``tpl-OASIS30ANTs_res-01_label-<label>_probseg.nii.gz``
#: (``oracles/templateflow`` holds some of them; the others are fetched and verified).
OASIS_SHA256 = {
    "CSF": "ebbe851b6d5898c5637194ddc386c92df0ddfe727a63c61a81abe919d8fdfc60",
    "CGM": "c4be7df569fa12d059033fc8775f78240785659591d3c00913619536a80193da",
    "WM": "3579a79d197cb3e8722638fa57228d3bd8fb6e6f88de4d3e76e306d6871ce0d6",
    "SCGM": "7755a667e4d658b49cccf9f325f939b5effa4bbc3a7801299e9484908659d573",
    "BS": "8e0e0b0f77056671e2a4b3136daccd04c0bb117c6bd7dffa5e349f2534e94dc3",
    "CBM": "235a34466ca143e37cc0c9c0f3a8802588923b1eeda450e4b4a5dbece3bbacd1",
}
TEMPLATEFLOW_URL = "https://templateflow.s3.amazonaws.com/tpl-OASIS30ANTs/"


def _sha256(path: Path) -> str:
    import hashlib

    return hashlib.sha256(path.read_bytes()).hexdigest()


@functools.cache
def _oasis_prior(label: str) -> Path:
    """TemplateFlow's OASIS30ANTs prior of ``label``: the workspace's copy
    (``oracles/templateflow``) when it is the file (not an empty placeholder), else fetched
    from TemplateFlow into a temporary directory; its SHA-256 checked either way."""
    name = f"tpl-OASIS30ANTs_res-01_label-{label}_probseg.nii.gz"
    tf = _templateflow()
    local = None if tf is None else tf / "tpl-OASIS30ANTs" / name
    if (
        local is not None
        and local.is_file()
        and local.stat().st_size > 0
        and _sha256(local) == OASIS_SHA256[label]
    ):
        return local
    import urllib.request

    target = _priors_dir() / name
    try:
        urllib.request.urlretrieve(TEMPLATEFLOW_URL + name, target)
    except OSError as e:
        raise Outcome(SKIPPED, f"{name} is neither in {TEMPLATEFLOW} nor fetchable ({e})") from e
    if _sha256(target) != OASIS_SHA256[label]:
        raise RuntimeError(f"{name} from TemplateFlow does not have the recorded SHA-256")
    return target


@functools.cache
def _oasis_priors(run: str, n: int) -> str:
    """The ``%02d`` pattern of the OASIS30ANTs tissue priors (``n`` = 3: CSF, cortical GM, WM;
    6: also deep GM, brainstem, cerebellum) in ``run``'s T1w space: fMRIPrep's
    ``map_brainmask`` command with each prior as input (ANTs' antsApplyTransforms, Gaussian
    interpolation, the template-to-T1w transforms of the brain extraction)."""
    work = _work(run)
    words = (work / BRAIN_EXTRACTION / "map_brainmask/command.txt").read_text().split()[1:]
    root = work.parents[1]
    words = [w.replace("/work/", f"{root}/") for w in words]
    out = _priors_dir() / f"{run}-{n}"
    out.mkdir(exist_ok=True)
    for c, label in enumerate(OASIS_CLASSES[n], 1):
        target = out / f"prior{c:02d}.nii.gz"
        if target.exists():
            continue
        args = list(words)
        args[args.index("--input") + 1] = str(_oasis_prior(label))
        args[args.index("--output") + 1] = str(target)
        env = {**os.environ, "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "8"}
        p = subprocess.run(
            [str(ants_bin() / "antsApplyTransforms"), *args],
            capture_output=True,
            text=True,
            env=env,
            timeout=1800,
        )
        if p.returncode != 0:
            raise RuntimeError(f"antsApplyTransforms failed for {label}: {p.stderr[-500:]}")
    return str(out / "prior%02d.nii.gz")


# ------------------------------------------------------------------------------------------------
# Cases

T1, T2, MASK = "{gen:atropos-t1}", "{gen:atropos-t2}", "{gen:atropos-mask}"
UNDEFINED_WEIGHTS = (
    "ANTS-107: from the second update on, Grubbs-Rosner's output has grown (ANTS-94) and ANTs "
    "weighs the samples beyond the masked voxels with an array it never sets, so its result "
    "depends on what the heap held (it changes with the length of the output paths); larmorx "
    "weighs them 0, what a fresh allocation holds"
)
PRIORS = "{series:atropos-prior}"
BASE = ("-d", "3", "-a", T1, "-x", MASK, "-c", "[3,0]", "-r", "0")
TENSORS = tuple(a for q in range(6) for a in ("-a", f"{{gen:atropos-tensor{q}}}"))


def _c(cid: str, category: str, description: str, *args: str, **kw) -> AtroposCase:
    return AtroposCase(cid, category, description, tuple(args), **kw)


def _phantom_cases() -> list[AtroposCase]:
    P, INIT, L, M, OUTL = "posteriors", "initialization", "likelihood", "mrf", "outliers"
    out_all = ("-o", "[{out}/seg.nii.gz,{out}/post%02d.nii.gz,{out}/lik%02d.nii.gz,"
               "{out}/dist%02d.nii.gz]")  # fmt: skip
    lik_dist = (*OUT3, "lik01.nii.gz", "lik02.nii.gz", "lik03.nii.gz", "dist01.nii.gz",
                "dist02.nii.gz", "dist03.nii.gz")  # fmt: skip
    prior = f"PriorProbabilityImages[3,{PRIORS},0.5]"
    label = "PriorLabelImage[3,{gen:atropos-prior-label},0.5]"
    return [
        _c(
            "kmeans-3",
            INIT,
            "K-means, 3 classes, MRF 0.1 (sMRIPrep's options on the phantom)",
            "-i",
            "KMeans[3]",
            *BASE,
            "-m",
            "[0.1,1x1x1]",
            *O3,
            band=True,
        ),
        _c(
            "kmeans-4",
            INIT,
            "K-means, 4 classes, MRF 0.3",
            "-i",
            "KMeans[4]",
            *BASE,
            "-m",
            "[0.3,1x1x1]",
            "-o",
            "[{out}/seg.nii.gz,{out}/post%02d.nii.gz]",
            outputs=(*OUT3, "post04.nii.gz"),
        ),
        _c(
            "kmeans-centres",
            INIT,
            "K-means from given centres (50, 100, 150)",
            "-i",
            "KMeans[3,50x100x150]",
            *BASE,
            *O3,
        ),
        _c("otsu", INIT, "Otsu thresholds, 3 classes", "-i", "Otsu[3]", *BASE, *O3),
        _c(
            "random",
            INIT,
            "random initial labels (fixed seed)",
            "-i",
            "Random[3]",
            *BASE,
            *O3,
            tier="standard",
        ),
        _c(
            "kmeans-multivariate",
            INIT,
            "K-means on two images (ANTS-90: only the first decides the initial labels)",
            "-i",
            "KMeans[3]",
            *BASE,
            "-a",
            T2,
            *O3,
        ),
        _c(
            "priors-socrates",
            P,
            "prior probability images (weight 0.5), Socrates",
            "-i",
            prior,
            *BASE,
            *O3,
        ),
        _c(
            "priors-plato",
            P,
            "prior probability images, Plato",
            "-i",
            prior,
            *BASE,
            "-p",
            "Plato",
            *O3,
            tier="standard",
        ),
        _c(
            "priors-aristotle",
            P,
            "prior probability images, Aristotle",
            "-i",
            prior,
            *BASE,
            "-p",
            "Aristotle",
            *O3,
        ),
        _c(
            "priors-sigmoid",
            P,
            "prior probability images, Sigmoid",
            "-i",
            prior,
            *BASE,
            "-p",
            "Sigmoid",
            *O3,
            tier="standard",
        ),
        _c(
            "priors-euclidean",
            P,
            "prior probability images (weight 0.25), Euclidean distance priors",
            "-i",
            f"PriorProbabilityImages[3,{PRIORS},0.25]",
            *BASE,
            "-e",
            "1",
            *O3,
        ),
        _c(
            "priors-threshold",
            P,
            "prior probability images with a probability threshold of 0.1",
            "-i",
            f"PriorProbabilityImages[3,{PRIORS},0.5,0.1]",
            *BASE,
            *O3,
            tier="standard",
        ),
        _c(
            "priors-vector",
            P,
            "the priors as one vector image (ANTS-100: every class reads "
            "the last component's normalized copy)",
            "-i",
            "PriorProbabilityImages[3,{gen:atropos-priors-vector},0.5]",
            *BASE,
            *O3,
        ),
        _c(
            "priors-minimize-memory",
            P,
            "prior probability images with "
            "--minimize-memory-usage and a mask (ANTS-105: ANTs crashes)",
            "-i",
            prior,
            *BASE,
            "-u",
            "1",
            *O3,
            expect="abort",
        ),
        _c(
            "priors-all-outputs",
            P,
            "prior probability images: posteriors, likelihoods and distance priors written",
            "-i",
            prior,
            *BASE,
            *out_all,
            outputs=lik_dist,
        ),
        _c(
            "priors-adaptive-smoothing",
            P,
            "prior probability images with adaptive smoothing "
            "(0.5) of the intensities: B-spline fits per class, 3 levels",
            "-i",
            prior,
            "-d",
            "3",
            "-a",
            f"[{T1},0.5]",
            "-x",
            MASK,
            "-c",
            "[2,0]",
            "-r",
            "0",
            "-b",
            "[3,1,3]",
            "-o",
            "[{out}/seg.nii.gz,{out}/post%02d.nii.gz,{out}/lik%02d.nii.gz,"
            "{out}/dist%02d.nii.gz,{out}/bsp%02d.nii.gz]",
            outputs=(*lik_dist, "bsp01.nii.gz", "bsp02.nii.gz", "bsp03.nii.gz"),
            tier="standard",
        ),
        _c(
            "label-fast-marching",
            P,
            "a prior label image (holes filled by fast marching)",
            "-i",
            label,
            *BASE,
            *O3,
        ),
        _c(
            "label-euclidean",
            P,
            "a prior label image, Euclidean distances",
            "-i",
            label,
            *BASE,
            "-e",
            "1",
            *O3,
            tier="standard",
        ),
        _c(
            "label-propagation",
            P,
            "a prior label image with label propagation for every class ([0.5,0.8])",
            "-i",
            label,
            *BASE,
            "-l",
            "[0.5,0.8]",
            *O3,
        ),
        _c(
            "label-propagation-per-label",
            P,
            "label propagation for labels 1 and 3",
            "-i",
            label,
            *BASE,
            "-l",
            "1[0.3,0.9]",
            "-l",
            "3[1,0.5]",
            *O3,
            tier="standard",
        ),
        _c(
            "histogram-parzen",
            L,
            "histogram Parzen windows (sigma 1, 32 bins)",
            "-i",
            "KMeans[3]",
            *BASE,
            "-k",
            "HistogramParzenWindows[1,32]",
            *O3,
        ),
        _c(
            "manifold-parzen",
            L,
            "manifold Parzen windows (k = 10; ANTS-95: the weights ignored)",
            "-i",
            "KMeans[3]",
            *BASE,
            "-k",
            "ManifoldParzenWindows[1,10]",
            "-c",
            "[2,0]",
            *O3,
        ),
        _c(
            "manifold-parzen-covariance",
            L,
            "manifold Parzen windows with local covariances (5 neighbours; ANTS-104)",
            "-i",
            "KMeans[3]",
            *BASE,
            "-k",
            "ManifoldParzenWindows[1,10,5,2]",
            "-c",
            "[1,0]",
            *O3,
            tier="standard",
        ),
        _c(
            "log-euclidean",
            L,
            "log-Euclidean Gaussians of six tensor components",
            "-i",
            "KMeans[3]",
            "-d",
            "3",
            *TENSORS,
            "-x",
            MASK,
            "-c",
            "[2,0]",
            "-r",
            "0",
            "-k",
            "LogEuclideanGaussian",
            *O3,
        ),
        _c(
            "joint-shape-orientation",
            L,
            "joint shape and orientation histograms of six tensor "
            "components (ANTS-103: an unset coordinate)",
            "-i",
            "KMeans[3]",
            "-d",
            "3",
            *TENSORS,
            "-x",
            MASK,
            "-c",
            "[2,0]",
            "-r",
            "0",
            "-k",
            "JointShapeAndOrientationProbability",
            *O3,
            tier="standard",
        ),
        _c(
            "mrf-synchronous",
            M,
            "synchronous updating (no ICM)",
            "-i",
            "KMeans[3]",
            *BASE,
            "-g",
            "0",
            *O3,
        ),
        _c(
            "mrf-radius",
            M,
            "an anisotropic MRF radius (2x1x2), smoothing 0.2",
            "-i",
            "KMeans[3]",
            *BASE,
            "-m",
            "[0.2,2x1x2]",
            *O3,
        ),
        _c(
            "mrf-coefficient-image",
            M,
            "an MRF coefficient image",
            "-i",
            "KMeans[3]",
            *BASE,
            "-m",
            "[{gen:atropos-mrf-coefficients},1x1x1]",
            *O3,
        ),
        _c(
            "mrf-none",
            M,
            "no MRF (smoothing 0)",
            "-i",
            "KMeans[3]",
            *BASE,
            "-m",
            "[0,1x1x1]",
            *O3,
            tier="standard",
        ),
        _c(
            "mrf-icm-codes",
            M,
            "two ICM iterations and the ICM code image",
            "-i",
            "KMeans[3]",
            *BASE,
            "-g",
            "1[1,2,{out}/icm.nii.gz]",
            *O3,
            outputs=(*OUT3, "icm.nii.gz"),
            tier="standard",
        ),
        _c(
            "no-proportions",
            P,
            "no mixture proportions (Socrates[0])",
            "-i",
            "KMeans[3]",
            *BASE,
            "-p",
            "Socrates[0]",
            *O3,
            tier="standard",
        ),
        _c(
            "annealing",
            P,
            "annealing from 2 at a rate of 0.5 down to 0.1",
            "-i",
            "KMeans[3]",
            *BASE,
            "-p",
            "Socrates[1,2,0.5,0.1]",
            *O3,
        ),
        _c(
            "box-plot",
            OUTL,
            "box-plot winsorizing (ANTS-94: the filter re-runs on its own output)",
            "-i",
            "KMeans[3]",
            *BASE,
            "-w",
            "BoxPlot[0.25,0.75,1.5]",
            *O3,
        ),
        _c(
            "box-plot-wide",
            OUTL,
            "box-plot winsorizing with a whisker of 100 (nothing beyond)",
            "-i",
            "KMeans[3]",
            "-d",
            "3",
            "-a",
            T1,
            "-x",
            MASK,
            "-c",
            "[1,0]",
            "-r",
            "0",
            "-w",
            "BoxPlot[0.25,0.75,100]",
            *O3,
            tier="standard",
        ),
        _c(
            "grubbs-rosner",
            OUTL,
            "Grubbs-Rosner winsorizing, three iterations (ANTS-94; ANTS-107: from the second "
            "iteration on, ANTs weighs the grown sample with uninitialized memory)",
            "-i",
            "KMeans[3]",
            "-d",
            "3",
            "-a",
            T1,
            "-x",
            MASK,
            "-c",
            "[3,-1]",
            "-r",
            "0",
            "-w",
            "GrubbsRosner[0.05,0.1]",
            *O3,
            undefined=UNDEFINED_WEIGHTS,
            one_mode=("ANTS-107",),
        ),
        _c(
            "grubbs-rosner-outliers",
            OUTL,
            "Grubbs-Rosner on an image with six far outliers "
            "(ITK-51: ITK's Student quantiles are NaN, no outlier is found)",
            "-i",
            "KMeans[3]",
            "-d",
            "3",
            "-a",
            "{gen:atropos-t1-outliers}",
            "-x",
            MASK,
            "-c",
            "[2,0]",
            "-r",
            "0",
            "-w",
            "GrubbsRosner[0.05,0.1]",
            *O3,
            tier="standard",
            undefined=UNDEFINED_WEIGHTS,
            one_mode=("ANTS-107",),
        ),
        _c(
            "grubbs-rosner-multivariate",
            OUTL,
            "Grubbs-Rosner on two images: the class covariance becomes singular in both",
            "-i",
            "KMeans[3]",
            "-d",
            "3",
            "-a",
            T1,
            "-a",
            T2,
            "-x",
            MASK,
            "-c",
            "[1,0]",
            "-r",
            "0",
            "-w",
            "GrubbsRosner[0.2,0.3]",
            *O3,
            expect="error",
            tier="standard",
        ),
        _c(
            "partial-volume-mrf-only",
            "partial volume",
            "a partial-volume label set used by the MRF only",
            "-i",
            "KMeans[3]",
            *BASE,
            "-s",
            "1x2",
            *O3,
        ),
        _c(
            "partial-volume-likelihoods",
            "partial volume",
            "partial-volume classes 1x2 and 2x3 with their likelihoods, no MRF",
            "-i",
            "KMeans[3]",
            *BASE,
            "-s",
            "1x2",
            "-s",
            "2x3",
            "--use-partial-volume-likelihoods",
            "1",
            "-p",
            "Socrates[0]",
            "-m",
            "[0,1x1x1]",
            *O3,
        ),
        _c(
            "partial-volume-with-mrf",
            "partial volume",
            "partial-volume classes with the MRF (ANTS-106: they can never win a voxel; both stop)",
            "-i",
            "KMeans[3]",
            *BASE,
            "-s",
            "1x2",
            "-s",
            "2x3",
            "--use-partial-volume-likelihoods",
            "1",
            *O3,
            expect="error",
            silent_ok=("ITK-50", "ANTS-96"),
            tier="standard",
        ),
        *(
            _c(
                f"partial-volume-phantom-mrf-{beta}",
                "partial volume",
                f"the partial-volume phantom, classes 1x2 and 2x3 with their likelihoods, MRF "
                f"{beta} (ANTS-106), scored against the known fractions",
                "-d",
                "3",
                "-a",
                "{gen:atropos-pv}",
                "-x",
                "{gen:atropos-pv-mask}",
                "-i",
                "KMeans[3]",
                "-s",
                "1x2",
                "-s",
                "2x3",
                "--use-partial-volume-likelihoods",
                "1",
                "-m",
                f"[{beta},1x1x1]",
                "-c",
                "[5,0]",
                "-r",
                "0",
                "-o",
                "[{out}/seg.nii.gz,{out}/post%02d.nii.gz]",
                expect=expect,
                silent_ok=("ITK-50", "ANTS-96"),
                tier="standard",
                pv_truth=True,
            )
            for beta, expect in (("0", "pass"), ("0.1", "pass"), ("0.3", "error"))
        ),
        _c(
            "2d",
            "dimensions",
            "a 2D slice",
            "-i",
            "KMeans[3]",
            "-d",
            "2",
            "-a",
            "{gen:atropos-t1-2d}",
            "-x",
            "{gen:atropos-mask-2d}",
            "-c",
            "[3,0]",
            "-r",
            "0",
            *O3,
        ),
        _c(
            "4d",
            "dimensions",
            "a 4D series (two volumes)",
            "-i",
            "KMeans[3]",
            "-d",
            "4",
            "-a",
            "{gen:atropos-t1-4d}",
            "-x",
            "{gen:atropos-mask-4d}",
            "-c",
            "[2,0]",
            "-r",
            "0",
            *O3,
            tier="standard",
        ),
        _c(
            "mask-255",
            "inputs",
            "a mask of 255s (one LabelMap label, ITK's error)",
            "-i",
            "KMeans[3]",
            "-d",
            "3",
            "-a",
            T1,
            "-x",
            "{gen:atropos-mask255}",
            "-c",
            "[3,0]",
            "-r",
            "0",
            *O3,
            expect="error",
        ),
        _c(
            "mask-float",
            "inputs",
            "a float32 mask with 0.6 at its rim (read as 0)",
            "-i",
            "KMeans[3]",
            "-d",
            "3",
            "-a",
            T1,
            "-x",
            "{gen:atropos-mask-float}",
            "-c",
            "[2,0]",
            "-r",
            "0",
            *O3,
            tier="standard",
        ),
        _c(
            "no-dimension",
            "inputs",
            "no -d: Atropos assumes 3D (ANTS-99)",
            "-i",
            "KMeans[3]",
            "-a",
            T1,
            "-x",
            MASK,
            "-c",
            "[2,0]",
            "-r",
            "0",
            *O3,
            tier="standard",
        ),
    ]


def _message_cases() -> list[AtroposCase]:
    E, V = "messages", "verbose"
    X = ("-x", MASK)
    A = ("-a", T1)
    run = ("-d", "3", "-i", "KMeans[3]", *A, *X)
    cases = [
        _c(
            "no-arguments",
            E,
            "no arguments: the short help",
            outputs=(),
            expect="error",
            stdout=True,
        ),
        _c("help", E, "--help", "--help", outputs=(), expect="error", stdout=True),
        _c("short-help", E, "-h", "-h", outputs=(), expect="error", stdout=True),
        _c("no-initialization", E, "no -i", "-d", "3", *A, *X, outputs=(), expect="error"),
        _c(
            "initialization-without-classes",
            E,
            "-i KMeans (no classes)",
            "-d",
            "3",
            "-i",
            "KMeans",
            *A,
            *X,
            "-v",
            "1",
            outputs=(),
            expect="error",
            stdout=True,
        ),
        _c(
            "unknown-initialization",
            E,
            "-i Foo[3]",
            "-d",
            "3",
            "-i",
            "Foo[3]",
            *A,
            *X,
            "-v",
            "1",
            outputs=(),
            expect="error",
            stdout=True,
        ),
        _c(
            "bad-number",
            E,
            "-i KMeans[3x] (the parser cannot convert)",
            "-d",
            "3",
            "-i",
            "KMeans[3x]",
            *A,
            *X,
            outputs=(),
            expect="abort",
        ),
        _c(
            "wrong-centres",
            E,
            "two centres for three classes",
            "-d",
            "3",
            "-i",
            "KMeans[3,10x20]",
            *A,
            *X,
            "-v",
            "1",
            outputs=(),
            expect="error",
            stdout=True,
        ),
        _c(
            "priors-without-weight",
            E,
            "PriorProbabilityImages without a weight",
            "-d",
            "3",
            "-i",
            f"PriorProbabilityImages[3,{PRIORS}]",
            *A,
            *X,
            "-v",
            "1",
            outputs=(),
            expect="error",
            stdout=True,
        ),
        _c(
            "priors-missing",
            E,
            "prior images that do not exist",
            "-d",
            "3",
            "-i",
            "PriorProbabilityImages[3,{missing:nope%02d.nii.gz},0.5]",
            *A,
            *X,
            outputs=(),
            expect="error",
        ),
        _c(
            "prior-label-missing",
            E,
            "a prior label image that does not exist",
            "-d",
            "3",
            "-i",
            "PriorLabelImage[3,{missing:nope.nii.gz},0.5]",
            *A,
            *X,
            outputs=(),
            expect="error",
        ),
        _c(
            "no-mask",
            E,
            "no -x",
            "-d",
            "3",
            "-i",
            "KMeans[3]",
            *A,
            "-v",
            "1",
            outputs=(),
            expect="error",
            stdout=True,
        ),
        _c(
            "mask-missing",
            E,
            "a mask that does not exist",
            "-d",
            "3",
            "-i",
            "KMeans[3]",
            *A,
            "-x",
            "{missing:nope.nii.gz}",
            outputs=(),
            expect="error",
        ),
        _c(
            "no-image",
            E,
            "no -a",
            "-d",
            "3",
            "-i",
            "KMeans[3]",
            *X,
            "-v",
            "1",
            outputs=(),
            expect="error",
            stdout=True,
        ),
        _c(
            "image-missing",
            E,
            "an image that does not exist",
            "-d",
            "3",
            "-i",
            "KMeans[3]",
            "-a",
            "{missing:nope.nii.gz}",
            *X,
            outputs=(),
            expect="error",
        ),
        _c(
            "dimension-5",
            E,
            "-d 5",
            "-d",
            "5",
            "-i",
            "KMeans[3]",
            *A,
            *X,
            outputs=(),
            expect="error",
            stdout=True,
        ),
        _c(
            "negative-temperature",
            E,
            "a negative annealing temperature",
            *run,
            "-p",
            "Socrates[1,-1]",
            "-v",
            "1",
            outputs=(),
            expect="error",
            stdout=True,
        ),
        _c(
            "rate-above-one",
            E,
            "an annealing rate of 2",
            *run,
            "-p",
            "Socrates[1,1,2]",
            "-v",
            "1",
            outputs=(),
            expect="error",
            stdout=True,
        ),
        _c(
            "wrong-levels",
            E,
            "-b with two levels in 3D",
            *run,
            "-b",
            "[2x2]",
            "-v",
            "1",
            outputs=(),
            expect="error",
            stdout=True,
        ),
        _c(
            "wrong-mesh",
            E,
            "-b with a 2D mesh in 3D",
            *run,
            "-b",
            "[2,1x1]",
            "-v",
            "1",
            outputs=(),
            expect="error",
            stdout=True,
        ),
        _c(
            "wrong-radius",
            E,
            "-m with a 2D radius in 3D",
            *run,
            "-m",
            "[0.1,1x1]",
            "-v",
            "1",
            outputs=(),
            expect="error",
            stdout=True,
        ),
        _c(
            "unknown-likelihood",
            E,
            "-k Foo",
            *run,
            "-k",
            "Foo",
            "-v",
            "1",
            outputs=(),
            expect="error",
            stdout=True,
        ),
        _c(
            "tensor-image-count",
            E,
            "a tensor model with one image",
            *run,
            "-k",
            "LogEuclideanGaussian",
            "-v",
            "1",
            outputs=(),
            expect="error",
            stdout=True,
        ),
        _c(
            "partial-volume-labels",
            E,
            "-s 1x2x3 (three labels)",
            *run,
            "-s",
            "1x2x3",
            "-v",
            "1",
            outputs=(),
            expect="error",
            stdout=True,
        ),
        _c(
            "partial-volume-label-range",
            E,
            "-s 1x4 with three classes",
            *run,
            "-c",
            "[1,0]",
            "-r",
            "0",
            "-s",
            "1x4",
            "-o",
            "{out}/seg.nii.gz",
            outputs=("seg.nii.gz",),
        ),
        _c(
            "unknown-outliers",
            E,
            "-w Foo",
            *run,
            "-w",
            "Foo",
            "-v",
            "1",
            outputs=(),
            expect="error",
            stdout=True,
        ),
        _c(
            "icm-brackets",
            E,
            "--icm [1,2], the documented form (ANTS-102: ANTs aborts)",
            *run,
            "-g",
            "[1,2]",
            outputs=(),
            expect="abort",
        ),
        _c(
            "label-propagation-without-parameters",
            E,
            "-l 2 (the parser cannot convert the missing parameters: ANTs aborts)",
            *run,
            "-l",
            "2",
            outputs=(),
            expect="abort",
        ),
        _c(
            "unknown-formulation",
            E,
            "-p Zeno (Socrates kept, ANTS-101)",
            *run,
            "-c",
            "[1,0]",
            "-r",
            "0",
            "-p",
            "Zeno",
            "-o",
            "{out}/seg.nii.gz",
            outputs=("seg.nii.gz",),
        ),
        _c(
            "output-extension",
            E,
            "a classified image with an unknown extension",
            *run,
            "-c",
            "[1,0]",
            "-r",
            "0",
            "-o",
            "{out}/seg.foo",
            outputs=(),
            expect="abort",
        ),
        _c(
            "posterior-without-percent",
            E,
            "a posterior name without %d (every class writes the same file)",
            *run,
            "-c",
            "[1,0]",
            "-r",
            "0",
            "-o",
            "[{out}/seg.nii.gz,{out}/p.nii.gz]",
            outputs=("seg.nii.gz", "p.nii.gz"),
        ),
        _c(
            "verbose",
            V,
            "-v 1: progress, the filter's settings and its models",
            *run,
            "-c",
            "[2,0]",
            "-r",
            "0",
            "-o",
            "[{out}/seg.nii.gz,{out}/post%02d.nii.gz]",
            "-v",
            "1",
            stdout=True,
        ),
        _c(
            "verbose-grubbs",
            V,
            "-v 1 with Grubbs-Rosner (its outlier listing; one iteration, before ANTS-107)",
            *run,
            "-c",
            "[1,0]",
            "-r",
            "0",
            "-w",
            "GrubbsRosner[0.05,0.1]",
            "-o",
            "[{out}/seg.nii.gz,{out}/post%02d.nii.gz]",
            "-v",
            "1",
            stdout=True,
            tier="standard",
        ),
        _c(
            "verbose-box-plot",
            V,
            "-v 1 with box-plot winsorizing",
            *run,
            "-c",
            "[2,0]",
            "-r",
            "0",
            "-w",
            "BoxPlot[0.25,0.75,1.5]",
            "-o",
            "[{out}/seg.nii.gz,{out}/post%02d.nii.gz]",
            "-v",
            "1",
            stdout=True,
            tier="standard",
        ),
        _c(
            "verbose-priors",
            V,
            "-v 1 with prior probability images and label propagation",
            "-d",
            "3",
            "-i",
            f"PriorProbabilityImages[3,{PRIORS},0.5]",
            *A,
            *X,
            "-c",
            "[2,0]",
            "-r",
            "0",
            "-l",
            "[0.5,0.8]",
            "-o",
            "[{out}/seg.nii.gz,{out}/post%02d.nii.gz]",
            "-v",
            "1",
            stdout=True,
            tier="standard",
        ),
        _c(
            "verbose-parzen",
            V,
            "-v 1 with histogram Parzen windows",
            *run,
            "-c",
            "[1,0]",
            "-r",
            "0",
            "-k",
            "HistogramParzenWindows[1,32]",
            "-o",
            "[{out}/seg.nii.gz,{out}/post%02d.nii.gz]",
            "-v",
            "1",
            stdout=True,
            tier="standard",
        ),
    ]
    return cases


def _random_cases() -> list[AtroposCase]:
    R = "random seeds"
    return [
        _c(
            "clock-seed",
            R,
            "--use-random-seed 1: ANTs seeds from the clock (two runs); "
            "larmorx uses the seed 121212",
            "-i",
            "KMeans[3]",
            "-d",
            "3",
            "-a",
            T1,
            "-x",
            MASK,
            "-c",
            "[3,0]",
            "-r",
            "1",
            "-m",
            "[0.1,1x1x1]",
            *O3,
            expect="random",
        ),
        _c(
            "clock-seed-random-labels",
            R,
            "Random[3] with the clock's seed",
            "-i",
            "Random[3]",
            "-d",
            "3",
            "-a",
            T1,
            "-x",
            MASK,
            "-c",
            "[3,0]",
            *O3,
            expect="random",
            tier="standard",
        ),
    ]


def _real_cases() -> list[AtroposCase]:
    out = []
    for n, run in enumerate(RUNS):
        short = run.split("-noreconall")[0]
        out.append(
            _c(
                f"real/{short}/smriprep",
                "real (sMRIPrep)",
                f"sMRIPrep's Atropos call in "
                f"{short} (fMRIPrep 25.2.6's command line, its N4-corrected T1w and mask)",
                f"{{fmriprep:{run}/{ATROPOS_NODE}}}",
                tier="standard",
                band=n == 0,
                fmriprep=f"{run}/{ATROPOS_NODE}",
            )
        )
    for run, n, tier in (
        ("ds000005-sub-01-noreconall", 3, "standard"),
        ("ds005040-sub-002-noreconall", 3, "standard"),
        ("ds001600-sub-1-noreconall", 3, "full"),
        ("ds005454-sub-16-noreconall", 3, "full"),
        ("ds005752-sub-ON01016-noreconall", 3, "full"),
        ("ds000005-sub-01-noreconall", 6, "full"),
    ):
        short = run.split("-noreconall")[0]
        outs = ("seg.nii.gz", *(f"post{c:02d}.nii.gz" for c in range(1, n + 1)))
        out.append(
            _c(
                f"real/{short}/priors-{n}",
                "real (priors)",
                f"antsCorticalThickness-style "
                f"segmentation in {short}: the OASIS30ANTs priors ({', '.join(OASIS_CLASSES[n])}) "
                "in T1w space, weight 0.25, MRF 0.1, five iterations, Socrates",
                "-d",
                "3",
                "-a",
                f"{{node-image:{run}}}",
                "-x",
                f"{{node-mask:{run}}}",
                "-i",
                f"PriorProbabilityImages[{n},{{priors:{run}:{n}}},0.25]",
                "-k",
                "Gaussian",
                "-m",
                "[0.1,1x1x1]",
                "-c",
                "[5,0]",
                "-g",
                "1",
                "-p",
                "Socrates[1]",
                "-r",
                "0",
                "-e",
                "0",
                *O3,
                outputs=outs,
                tier=tier,
            )
        )
    for run, tier in (
        ("ds000005-sub-01-noreconall", "standard"),
        ("ds005040-sub-002-noreconall", "full"),
    ):
        short = run.split("-noreconall")[0]
        out.append(
            _c(
                f"real/{short}/smriprep-partial-volume",
                "real (partial volume)",
                f"sMRIPrep's options on {short}'s N4-corrected T1w and brain mask with "
                "partial-volume classes 1x2 and 2x3 (ANTS-106: ANTs stops)",
                "-d",
                "3",
                "-a",
                f"{{node-image:{run}}}",
                "-x",
                f"{{node-mask:{run}}}",
                "-i",
                "KMeans[3]",
                "-k",
                "Gaussian",
                "-s",
                "1x2",
                "-s",
                "2x3",
                "--use-partial-volume-likelihoods",
                "1",
                "-m",
                "[0.1,1x1x1]",
                "-c",
                "[3,0]",
                "-r",
                "0",
                "-o",
                "[{out}/seg.nii.gz,{out}/post%02d.nii.gz]",
                expect="error",
                silent_ok=("ITK-50", "ANTS-96"),
                tier=tier,
            )
        )
    ds5 = "ds000005-sub-01-noreconall"
    out += [
        _c(
            "real/ds000005/otsu",
            "real (other)",
            "Otsu[3] on ds000005's N4-corrected T1w",
            "-d",
            "3",
            "-a",
            f"{{node-image:{ds5}}}",
            "-x",
            f"{{node-mask:{ds5}}}",
            "-i",
            "Otsu[3]",
            "-m",
            "[0.1,1x1x1]",
            "-c",
            "[3,0]",
            "-r",
            "0",
            *O3,
            tier="standard",
        ),
        _c(
            "real/ds000005/random",
            "real (other)",
            "Random[3] on ds000005's T1w (fixed seed)",
            "-d",
            "3",
            "-a",
            f"{{node-image:{ds5}}}",
            "-x",
            f"{{node-mask:{ds5}}}",
            "-i",
            "Random[3]",
            "-m",
            "[0.1,1x1x1]",
            "-c",
            "[5,0]",
            "-r",
            "0",
            *O3,
            tier="standard",
        ),
        _c(
            "real/ds000005/smriprep-clock-seed",
            "real (other)",
            "sMRIPrep's call with "
            "--use-random-seed 1: ANTs seeds from the clock (two ANTs runs, ANTs' seed band)",
            "-d",
            "3",
            "-a",
            f"{{node-image:{ds5}}}",
            "-x",
            f"{{node-mask:{ds5}}}",
            "-i",
            "KMeans[3]",
            "-k",
            "Gaussian",
            "-m",
            "[0.1,1x1x1]",
            "-c",
            "[3,0]",
            "-r",
            "1",
            *O3,
            expect="random",
            tier="standard",
        ),
    ]
    return out


STROKE_RUN = "ds004884-sub-M2079-noreconall"
CHILD_RUN = "ds000256-sub-CTS203-noreconall"


@ants_inputs.builder("atropos-panel-stroke-mask")
def _(t):
    """sMRIPrep's Atropos mask of ds004884 sub-M2079 (the brain extraction's) without the
    stroke's manual lesion mask (the oracle run's ``label-lesion_roi``, on the T1w's grid)."""
    import nibabel as nib

    from larmorx_validation.parity import fmriprep_nodes

    mask_path = _node_inputs(STROKE_RUN)[1]
    roi = sorted(
        Path(fmriprep_nodes.oracle_run(STROKE_RUN).mounts["/data"]).glob(
            "sub-*/ses-*/anat/*_label-lesion_roi.nii.gz"
        )
    )
    if not roi:
        raise Outcome(SKIPPED, f"{STROKE_RUN}'s lesion mask is not here")
    m = nib.load(mask_path)
    lesion = np.asanyarray(nib.load(roi[0]).dataobj) > 0
    data = (np.asanyarray(m.dataobj) > 0) & ~lesion
    out = nib.Nifti1Image(data.astype(np.uint8), m.affine, m.header)
    out.set_data_dtype(np.uint8)
    nib.save(out, t)


@ants_inputs.builder("atropos-panel-child-t2w")
def _(t):
    """ds000256 sub-CTS203's T2w on its N4-corrected T1w's grid (the brain extraction's): larmorx's
    antsApplyTransforms, linear, identity (reproduce mode), float32: Atropos' second channel."""
    import larmorx_testdata as td

    import larmorx as lx
    from larmorx_validation.parity import panel_images as P

    image = _node_inputs(CHILD_RUN)[0]
    out = lx.ants.apply_transforms(
        td.get(P.CHILD_T2W.path),
        reference=image,
        transforms=[],
        interpolation="linear",
        upstream_defects="reproduce",
    )
    lx.save(out, t)


@ants_inputs.builder("atropos-panel-flair-mask")
def _(t):
    """ds003592 sub-280's FLAIR head: the largest connected component of the voxels above 10 %
    of its 99th percentile, holes filled slice by slice (uint8 on the FLAIR's grid)."""
    import larmorx_testdata as td
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.parity import panel_images as P

    img = nib.load(td.get(P.FLAIR_OLDER.path))
    data = np.asanyarray(img.dataobj).astype(np.float64)
    above = data > 0.1 * np.percentile(data, 99)
    labels, n = ndimage.label(above)
    sizes = np.bincount(labels.ravel())
    sizes[0] = 0
    head = labels == int(np.argmax(sizes)) if n else above
    for k in range(head.shape[2]):
        head[..., k] = ndimage.binary_fill_holes(head[..., k])
    out = nib.Nifti1Image(head.astype(np.uint8), img.affine)
    out.header.set_qform(img.affine, code=int(img.header["qform_code"]))
    out.header.set_sform(img.affine, code=int(img.header["sform_code"]))
    nib.save(out, t)


def _panel_cases() -> list[AtroposCase]:
    """The data panel's tier: sMRIPrep's Atropos node of every finished fMRIPrep oracle run
    (fMRIPrep's own outputs compared too; ANTs' thread band on the first), and the same options
    on what fMRIPrep's calls do not isolate: a lesion left out of the mask, a second contrast,
    a FLAIR of 3 mm slices."""
    from larmorx_validation.parity import fmriprep_nodes
    from larmorx_validation.parity import panel_images as P

    out = []
    for n, call in enumerate(fmriprep_nodes.calls(PROGRAM)):
        node = call.node.split("/", 2)[2]  # below the subject's workflow directory
        out.append(
            _c(
                fmriprep_nodes.PREFIX + call.id,
                fmriprep_nodes.CATEGORY,
                fmriprep_nodes.describe(call),
                f"{{fmriprep:{call.run}/{node}}}",
                tier="panel",
                band=n == 0,
                fmriprep=f"{call.run}/{node}",
            )
        )
    smriprep = ("-i", "KMeans[3]", "-k", "Gaussian", "-m", "[0.1,1x1x1]", "-c", "[3,0]",
                "-r", "0", *O3)  # fmt: skip
    out += [
        _c(
            P.PREFIX + "stroke/outside-lesion",
            P.CATEGORY,
            P.describe(
                "sMRIPrep's options on the stroke's N4-corrected T1w (the oracle run's), its "
                "brain mask without the manual lesion mask",
                P.STROKE_T1W,
                P.STROKE_LESION,
            ),
            "-d",
            "3",
            "-a",
            f"{{node-image:{STROKE_RUN}}}",
            "-x",
            "{gen:atropos-panel-stroke-mask}",
            *smriprep,
            tier="panel",
        ),
        _c(
            P.PREFIX + "child/t1w-t2w",
            P.CATEGORY,
            P.describe(
                "sMRIPrep's options with two channels: a child's N4-corrected T1w (the oracle "
                "run's) and its T2w on the same grid, in its brain mask",
                P.CHILD_T1W,
                P.CHILD_T2W,
            ),
            "-d",
            "3",
            "-a",
            f"{{node-image:{CHILD_RUN}}}",
            "-a",
            "{gen:atropos-panel-child-t2w}",
            "-x",
            f"{{node-mask:{CHILD_RUN}}}",
            *smriprep,
            tier="panel",
        ),
        _c(
            P.PREFIX + "flair/kmeans-3",
            P.CATEGORY,
            P.describe(
                "sMRIPrep's options on a FLAIR of 0.42 x 0.42 x 3 mm voxels, in its head mask",
                P.FLAIR_OLDER,
            ),
            "-d",
            "3",
            "-a",
            P.FLAIR_OLDER.arg,
            "-x",
            "{gen:atropos-panel-flair-mask}",
            *smriprep,
            tier="panel",
        ),
    ]
    return out


def all_cases() -> list[AtroposCase]:
    return _phantom_cases() + _message_cases() + _random_cases() + _real_cases() + _panel_cases()


def cases_for(tier: str) -> list[Case]:
    return [Case(c.id, c.category, c.description, c) for c in all_cases() if in_tier(c.tier, tier)]


# ------------------------------------------------------------------------------------------------
# Running


def _series(name: str) -> str:
    """The ``%02d`` pattern of the generated inputs ``name01``, ``name02``, ``name03``."""
    first = ants_inputs.path(f"{name}01")
    for n in (2, 3):
        ants_inputs.path(f"{name}{n:02d}")
    return str(first).replace(f"{name}01.nii.gz", f"{name}%02d.nii.gz")


def _resolve(c: AtroposCase, out: Path, tmp: Path) -> list[str]:
    """The case's arguments with its outputs in ``out``."""
    if len(c.args) == 1 and c.args[0].startswith("{fmriprep:"):
        run, node = c.args[0][10:-1].split("/", 1)
        return _node_args(run, node, out)

    def sub(m: re.Match[str]) -> str:
        key = m.group(1)
        if key == "out":
            return str(out)
        if key.startswith("gen:"):
            return str(ants_inputs.path(key[4:]))
        if key.startswith("in:"):
            import larmorx_testdata as td

            return str(td.get(key[3:]))
        if key.startswith("series:"):
            return _series(key[7:])
        if key.startswith("missing:"):
            return str(tmp / key[8:])
        if key.startswith("node-image:"):
            return str(_node_inputs(key[11:])[0])
        if key.startswith("node-mask:"):
            return str(_node_inputs(key[10:])[1])
        if key.startswith("priors:"):
            run, n = key[7:].rsplit(":", 1)
            return _oasis_priors(run, int(n))
        raise KeyError(f"unknown placeholder {{{key}}}")

    return [re.sub(r"\{([^{}]+)\}", sub, a) for a in c.args]


def _run_ants(
    args: list[str], threads: int = 1, *, correctly_rounded: bool = False, never: str = ""
):
    """Atropos, through the oracle cache: never kept with the clock's seed
    (``--use-random-seed 1``), or when ``never`` says why (:func:`case_never`)."""
    from larmorx_validation import oracle_cache

    b = ants_bin()
    if b is None or not (b / PROGRAM).is_file():
        raise Outcome(SKIPPED, "the ANTs oracle's Atropos was not found (LARMORX_ANTS_BIN)")
    env = {k: v for k, v in os.environ.items() if k not in ("ANTS_RANDOM_SEED", "LD_PRELOAD")}
    env["ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS"] = str(threads)
    if correctly_rounded:
        lib = crmath_preload()
        if lib is None:
            raise FileNotFoundError("libcrmath.so (scripts/build_crmath_preload.sh)")
        env["LD_PRELOAD"] = str(lib)
    argv = [str(b / PROGRAM), *args]
    policy = oracle_cache.Policy(
        threads=oracle_cache.itk_threads(env),
        never=never or oracle_cache.ants_seed_never(PROGRAM, list(args), env),
        family="itk",
    )
    p = oracle_cache.run(argv, env=env, policy=policy, timeout=3600, runner="ants-atropos")
    if p.timed_out:
        raise subprocess.TimeoutExpired(argv, 3600)
    return p.returncode, p.stdout, p.stderr


def case_never(c: AtroposCase) -> str:
    """Why ANTs' runs of an Atropos case are never kept in the oracle cache: the clock's seed
    (the case measures ANTs' own spread), values that depend on memory ANTs never wrote
    (``undefined``), or the case's texts naming a defect whose result varies."""
    from larmorx_validation.oracle_cache import never_for

    if c.expect == "random":
        return "ANTs seeds from the clock: the case measures its own spread"
    if c.undefined:
        return f"ANTs' values depend on memory it never wrote ({c.undefined[:120]})"
    return never_for(c.description, c.reason)


def _run_larmorx(args: list[str], threads: int | None = None, mode: str = defects.REPRODUCE):
    from larmorx import _core

    threads = LX_THREADS if threads is None else threads
    saved = os.environ.get("ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS")
    os.environ["ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS"] = str(threads)
    try:
        with defects.mode(mode):
            code, out, err = _core.cli_main(["larmorx", "ants", PROGRAM, *args], [], "original")
    finally:
        if saved is None:
            os.environ.pop("ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS", None)
        else:
            os.environ["ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS"] = saved
    return code, out, err


@dataclass
class _Run:
    code: int
    out: str
    err: str
    outputs: list[Path]

    @property
    def ok(self) -> bool:
        return self.code == 0 and all(p.exists() for p in self.outputs)


def _execute(c: AtroposCase, tmp: Path, name: str, runner, **kw) -> _Run:
    if runner is _run_ants:
        kw.setdefault("never", case_never(c))
    out = tmp / name
    out.mkdir()
    args = _resolve(c, out, tmp)
    code, stdout, stderr = runner(args, **kw)
    return _Run(code, stdout, stderr, [out / o for o in c.outputs])


def _load(path: Path) -> tuple[np.ndarray, np.ndarray]:
    import nibabel as nib

    img = nib.load(path)
    return np.asanyarray(img.dataobj.get_unscaled()), img.affine


def _bits(x: np.ndarray) -> np.ndarray:
    x = np.ascontiguousarray(x)
    return x.view(f"u{x.dtype.itemsize}") if x.dtype.kind == "f" else x


def _differing(a: np.ndarray, b: np.ndarray) -> int:
    return int(np.count_nonzero(_bits(a) != _bits(b))) if a.shape == b.shape else a.size


def _ncc(a: np.ndarray, b: np.ndarray) -> float:
    x = a.astype(np.float64).ravel()
    y = b.astype(np.float64).ravel()
    x, y = x - x.mean(), y - y.mean()
    # numpy's own pairwise sums, not `@` (the BLAS's threads change the last bits, 2026-10-24)
    den = np.sqrt(np.sum(x * x) * np.sum(y * y))
    return float(np.sum(x * y) / den) if den > 0 else (1.0 if np.array_equal(x, y) else 0.0)


def _dice(a: np.ndarray, b: np.ndarray) -> dict[int, float]:
    """Dice overlap of every non-zero label of two label images."""
    out = {}
    for lab in sorted(set(np.unique(a)) | set(np.unique(b))):
        if lab == 0:
            continue
        x, y = a == lab, b == lab
        den = int(x.sum() + y.sum())
        out[int(lab)] = 2 * int((x & y).sum()) / den if den else 1.0
    return out


def distance(a_path: Path, b_path: Path) -> dict[str, float]:
    """How far output ``b`` is from ``a``: values differing; for labels the smallest Dice, for
    posteriors the largest absolute difference and 1 - NCC."""
    a, _ = _load(a_path)
    b, _ = _load(b_path)
    out: dict[str, float] = {"differing": _differing(a, b), "size": a.size}
    if a.shape != b.shape:
        return out | {"min_dice": 0.0, "max_abs": float("inf"), "one_minus_ncc": 1.0}
    if a.dtype.kind in "iu":
        d = _dice(a, b)
        out["min_dice"] = min(d.values()) if d else 1.0
    else:
        diff = np.abs(a.astype(np.float64) - b.astype(np.float64))
        out["max_abs"] = float(np.nanmax(diff)) if diff.size else 0.0
        out["one_minus_ncc"] = max(0.0, 1.0 - _ncc(a, b))
    return out


def _describe(m: dict[str, float]) -> str:
    if m["differing"] == 0:
        return "identical"
    text = f"{int(m['differing'])} of {int(m['size'])} differ"
    if "min_dice" in m:
        return text + f"; smallest Dice {m['min_dice']:.6f}"
    return text + f"; max |diff| {m['max_abs']:.3g}, 1 - NCC {m['one_minus_ncc']:.2g}"


#: What no port can print alike: addresses, the ITK base classes' lines of ``Print``,
#: elapsed times, the source tree's path in exceptions, larmorx's D13 notices and the text it
#: adds where ANTs aborts.
def printed(text: str, tmp: Path) -> str:
    text = text.replace(str(tmp), "<tmp>")
    text = re.sub(r"\b0x[0-9a-f]+\b", "0x0", text)
    text = re.sub(
        r"(?ms)^    RTTI typeinfo:   itk::ants::Atropos.*?^    DirectionTolerance: [^\n]*\n",
        "",
        text,
    )
    text = re.sub(r"(?m)^Elapsed time: .*$", "Elapsed time: T", text)
    text = re.sub(r"/[^\s\"]*/(ANTs-v2\.6\.5|ITK-v5\.4\.5)/", "", text)
    text = re.sub(r"(?m)^larmorx: .*\n?", "", text)
    return text.strip()


def _same_text(name: str, checks: CheckList, a: str, b: str) -> None:
    same = a == b
    detail = ""
    if not same:
        al, bl = a.splitlines(), b.splitlines()
        for i in range(max(len(al), len(bl))):
            x = al[i] if i < len(al) else "<none>"
            y = bl[i] if i < len(bl) else "<none>"
            if x != y:
                detail = f"line {i + 1}: ANTs {x[:160]!r}, larmorx {y[:160]!r}"
                break
    checks(name, same, detail=detail)


def run_case(case: Case, checks: CheckList) -> None:
    c: AtroposCase = case.payload
    if ants_bin() is None:
        raise Outcome(SKIPPED, "the ANTs 2.6.5 oracle was not found (LARMORX_ANTS_BIN)")
    with tempfile.TemporaryDirectory(prefix="lx-atropos-") as tmp_name:
        tmp = Path(tmp_name)
        if c.expect == "random":
            _random(c, tmp, checks)
            return
        ants = _execute(c, tmp, "ants", _run_ants)
        lx = _execute(c, tmp, "larmorx", _run_larmorx)
        summary = (
            f"ANTs exit {ants.code}: {(ants.err or ants.out).strip()[-160:]!r}; "
            f"larmorx exit {lx.code}: {(lx.err or lx.out).strip()[-160:]!r}"
        ).replace(str(tmp), "<tmp>")
        if c.expect == "abort":
            ok = ants.code < 0 and lx.code == 1
            checks("ANTs aborts (a signal); larmorx exits 1", ok, detail=summary)
            # What ANTs printed before it stopped, and the exception's text.
            _same_text("stdout", checks, printed(ants.out, tmp), printed(lx.out, tmp))
            _same_text("stderr", checks, printed(ants.err, tmp), printed(lx.err, tmp))
            if ok and _fix_pass(c, tmp, checks, lx):
                raise Outcome(EXPECTED, c.reason or "ANTs aborts; larmorx exits 1 with its text")
            return
        if c.stdout and (c.expect == "error" or not (ants.ok and lx.ok)):
            _same_text("stdout", checks, printed(ants.out, tmp), printed(lx.out, tmp))
        if c.expect == "error" or (not ants.ok and not lx.ok):
            checks("same exit status", ants.code == lx.code, detail=summary)
            if not c.stdout:
                _same_text("stdout", checks, printed(ants.out, tmp), printed(lx.out, tmp))
            _same_text("stderr", checks, printed(ants.err, tmp), printed(lx.err, tmp))
            if not ants.ok and not lx.ok:
                if ants.code == lx.code and _fix_pass(c, tmp, checks, lx):
                    raise Outcome(BOTH_ERROR, summary)
                return
            if c.expect == "error" and not c.outputs:
                return
        checks("both succeed", ants.ok and lx.ok, detail=summary)
        if not (ants.ok and lx.ok):
            return
        _fix_pass(c, tmp, checks, lx)
        lx1 = _execute(c, tmp, "larmorx-1", _run_larmorx, threads=1)
        same = lx1.ok and all(
            _load(p)[0].tobytes() == _load(q)[0].tobytes()
            for p, q in zip(lx.outputs, lx1.outputs, strict=True)
        )
        checks(
            f"larmorx: {LX_THREADS} threads and 1 thread give the same bits",
            same,
            metric="identical",
            value=str(same),
            threshold="True",
        )
        cr = None
        if crmath_preload() is not None:
            cr = _execute(c, tmp, "ants-cr", _run_ants, correctly_rounded=True)
            checks("ANTs with correctly rounded math succeeds", cr.ok)
            if not cr.ok:
                cr = None
        if c.stdout:
            # -v prints the models' means with 17 digits: glibc's maths shows there.
            reference = cr if cr is not None else ants
            _same_text(
                "stdout (as ANTs with correctly rounded math prints it)"
                if cr is not None
                else "stdout",
                checks,
                printed(reference.out, tmp),
                printed(lx.out, tmp),
            )
            same_built = printed(ants.out, tmp) == printed(lx.out, tmp)
            checks.add(
                Check(
                    "stdout as ANTs as built prints it",
                    True,
                    metric="identical",
                    value=str(same_built),
                    threshold="reported",
                    detail=""
                    if same_built
                    else "the models' means differ in their last digits (glibc's maths)",
                )
            )
        band: dict[int, _Run] = {}
        if c.band:
            for t in BAND_THREADS:
                r = _execute(c, tmp, f"ants-{t}", _run_ants, threads=t)
                if r.ok:
                    band[t] = r
        for i, name in enumerate(c.outputs):
            _compare_output(checks, name, ants.outputs[i], lx.outputs[i],
                            None if cr is None else cr.outputs[i],
                            {t: r.outputs[i] for t, r in band.items()}, c.undefined)  # fmt: skip
        if c.fmriprep:
            _compare_fmriprep(c, checks, lx)


def _compare_output(
    checks: CheckList,
    name: str,
    a_path: Path,
    l_path: Path,
    cr_path: Path | None,
    band: dict[int, Path],
    undefined: str = "",
) -> None:
    label = name.removesuffix(".nii.gz")
    differing = header_differences(raw_header(a_path), raw_header(l_path))
    checks(
        f"{label}: header (every field)",
        not differing,
        metric="fields differing",
        value=len(differing),
        threshold=0,
        detail="; ".join(differing),
    )
    a, a_aff = _load(a_path)
    b, b_aff = _load(l_path)
    checks(
        f"{label}: data type and shape",
        a.dtype == b.dtype and a.shape == b.shape,
        detail=f"ANTs {a.dtype} {a.shape}, larmorx {b.dtype} {b.shape}",
    )
    aff = float(np.max(np.abs(a_aff - b_aff)))
    checks(f"{label}: affine", aff <= AFFINE_ATOL, metric="max |diff| (mm)", value=f"{aff:.1e}",
           threshold=f"{AFFINE_ATOL:g}")  # fmt: skip
    if a.shape != b.shape or a.dtype != b.dtype:
        return
    if cr_path is not None:
        m = distance(cr_path, l_path)
        if undefined:
            checks.add(
                Check(
                    f"{label}: values against ANTs with correctly rounded math (not required)",
                    True,
                    metric="values differing",
                    value=int(m["differing"]),
                    threshold="reported",
                    detail=f"{_describe(m)}; {undefined}",
                )
            )
            return
        checks(
            f"{label}: bit-identical to ANTs with correctly rounded math",
            m["differing"] == 0,
            metric="values differing",
            value=int(m["differing"]),
            threshold=0,
            detail=_describe(m),
        )
    m = distance(a_path, l_path)
    checks.add(
        Check(
            f"{label}: bit-identical to ANTs as built",
            True,
            metric="values differing",
            value=int(m["differing"]),
            threshold="reported",
            detail=_describe(m),
        )
    )
    if m["differing"] and "one_minus_ncc" in m:
        checks(
            f"{label}: NCC with ANTs as built",
            1.0 - m["one_minus_ncc"] > NCC_MIN,
            metric="NCC",
            value=f"{1.0 - m['one_minus_ncc']:.10f}",
            threshold=f"> {NCC_MIN}",
        )
    if band:
        worst = max((distance(a_path, p)["differing"] for p in band.values()), default=0)
        checks.add(
            Check(
                f"{label}: ANTs' thread band",
                True,
                metric="values differing (ANTs at n threads vs 1)",
                value=int(worst),
                threshold="reported",
                detail=f"ANTs at {', '.join(map(str, band))} threads vs 1: up to {worst} "
                "values differ",
            )
        )


def _compare_fmriprep(c: AtroposCase, checks: CheckList, lx: _Run) -> None:
    """larmorx's outputs next to the files fMRIPrep wrote (its container's ANTs, 8 threads)."""
    run, node = c.fmriprep.split("/", 1)
    work = _work(run) / node
    seg = sorted(work.glob("*_labeled.nii.gz"))
    pairs = [(seg[0] if seg else None, lx.outputs[0])]
    pairs += [(work / f"POSTERIOR_{k:02d}.nii.gz", lx.outputs[k]) for k in (1, 2, 3)]
    for theirs, ours in pairs:
        if theirs is None or not theirs.exists():
            continue
        m = distance(theirs, ours)
        checks.add(
            Check(
                f"{ours.name.removesuffix('.nii.gz')}: vs fMRIPrep's file",
                True,
                metric="values differing",
                value=int(m["differing"]),
                threshold="reported",
                detail=_describe(m),
            )
        )


def _random(c: AtroposCase, tmp: Path, checks: CheckList) -> None:
    """ANTs seeds from the clock: run it twice; larmorx (seed 121212) once; report larmorx's
    distance from each ANTs run next to the ANTs runs' distance from each other."""
    a1 = _execute(c, tmp, "ants-1", _run_ants)
    a2 = _execute(c, tmp, "ants-2", _run_ants)
    lx = _execute(c, tmp, "larmorx", _run_larmorx)
    lx2 = _execute(c, tmp, "larmorx-again", _run_larmorx, threads=1)
    checks("all runs succeed", a1.ok and a2.ok and lx.ok and lx2.ok,
           detail=f"ANTs {a1.code}, {a2.code}; larmorx {lx.code}, {lx2.code}")  # fmt: skip
    if not (a1.ok and a2.ok and lx.ok and lx2.ok):
        return
    same = all(
        _load(p)[0].tobytes() == _load(q)[0].tobytes()
        for p, q in zip(lx.outputs, lx2.outputs, strict=True)
    )
    checks("larmorx repeats itself (no clock)", same)
    for i, name in enumerate(c.outputs):
        label = name.removesuffix(".nii.gz")
        own = distance(a1.outputs[i], a2.outputs[i])
        ours = [distance(a.outputs[i], lx.outputs[i]) for a in (a1, a2)]
        key = "min_dice" if "min_dice" in own else "max_abs"
        worst = min(o[key] for o in ours) if key == "min_dice" else max(o[key] for o in ours)
        checks.add(
            Check(
                f"{label}: larmorx vs ANTs' two runs",
                True,
                metric=key,
                value=f"{worst:.6g}",
                threshold="reported",
                detail=f"ANTs run 1 vs run 2: {_describe(own)}; larmorx vs ANTs: "
                + " / ".join(_describe(o) for o in ours),
            )
        )
    if not _fix_pass(c, tmp, checks, lx):
        return
    raise Outcome(EXPECTED, "ANTs draws its seed from the clock; larmorx uses 121212")


def _fix_pass(c: AtroposCase, tmp: Path, checks: CheckList, reproduced: _Run) -> bool:
    """D13: larmorx's default mode on the same arguments, compared with its ``reproduce`` run."""
    fixed = _execute(c, tmp, "larmorx-fix", _run_larmorx, mode=defects.FIX)
    other = []
    if fixed.code != reproduced.code:
        other.append(f"exit {reproduced.code} -> {fixed.code}")
    if fixed.ok != reproduced.ok:
        other.append("outputs written" if fixed.ok else "outputs not written")
    change = defects.Change(0, 0, 0.0)
    if fixed.ok and reproduced.ok:
        for f, r in zip(fixed.outputs, reproduced.outputs, strict=True):
            fields = [d.split(":")[0] for d in header_differences(raw_header(r), raw_header(f))]
            if fields:
                other.append(f"header {', '.join(fields)}")
            change = change + defects.Change.of(_load(f)[0], _load(r)[0])
    entries = defects.notices_in(fixed.err)
    check = defects.add_fix_check(
        checks, entries, change, other_changes="; ".join(other), silent_ok=c.silent_ok
    )
    if c.pv_truth:
        ants_seg = tmp / "ants" / "seg.nii.gz"
        ants_text = _pv_scores(ants_seg) if ants_seg.is_file() else "no labels (ANTs stopped)"
        mine = _pv_scores(fixed.outputs[0]) if fixed.ok else "no labels"
        checks(
            "partial-volume scores: ANTs",
            True,
            metric="against the phantom's known fractions",
            value=ants_text,
            threshold="reported",
            detail=f"ANTs 2.6.5 as built, one thread: {ants_text}",
        )
        checks(
            "partial-volume scores",
            True,
            metric="against the phantom's known fractions",
            value=mine,
            threshold="reported",
            detail=f"ANTs: {ants_text}; larmorx's default: {mine}",
        )
    if c.category == "real (partial volume)" and fixed.ok:
        labels = _load(fixed.outputs[0])[0]
        counts = [int((labels == k).sum()) for k in range(1, 6)]
        checks(
            "partial-volume voxels",
            True,
            metric="voxels per class (larmorx's default)",
            value=" ".join(map(str, counts)),
            threshold="reported",
            detail="classes 1-3 (CSF, GM, WM) and 4 = 1x2, 5 = 2x3: "
            + ", ".join(f"{k}: {n}" for k, n in zip(range(1, 6), counts, strict=True)),
        )
    if fixed.ok and reproduced.ok:
        named = [e for e in entries if e not in c.one_mode]
        before = [e for e in defects.notices_in(reproduced.err) if e not in c.one_mode]
        if named != before:
            checks(
                "D13 notices in both modes",
                False,
                detail=f"fix: {entries}; reproduce: {defects.notices_in(reproduced.err)}",
            )
            return False
    return check.passed


# ------------------------------------------------------------------------------------------------
# Report


def _values(r: CaseResult, suffix: str) -> list:
    return [ch.value for ch in r.checks if ch.name.endswith(suffix)]


def highlights(results: list[CaseResult]) -> list[str]:
    compared = [r for r in results if r.status == PASS and _values(r, "to ANTs as built")]
    exact = [r for r in compared if all(v == 0 for v in _values(r, "correctly rounded math"))]
    built = [r for r in compared if all(v == 0 for v in _values(r, "to ANTs as built"))]
    labels_built = [
        r
        for r in compared
        if all(
            ch.value == 0
            for ch in r.checks
            if ch.name.startswith("seg: bit-identical to ANTs as built")
        )
    ]
    lines = [
        f"**Bit-identical to the unmodified ANTs program (glibc's math, one thread): "
        f"{len(built)} of {len(compared)} compared cases; the classified image identical in "
        f"{len(labels_built)} of {len(compared)}.** The others' posteriors differ in the last "
        "bits where glibc's `expf`/`logf`/`exp`/`log`/`pow` are not correctly rounded for a "
        "value the run meets (their largest differences and NCC are in the table).",
        f"**The cause, confirmed: with those functions correctly rounded (CORE-MATH preloaded "
        f"into the same ANTs program), {len(exact)} of {len(compared)} are bit-identical.**",
    ]
    lines += defects.summary(results)
    rows = []
    for r in compared:
        for ch in r.checks:
            if ch.name.endswith("bit-identical to ANTs as built") and ch.value:
                rows.append((f"`{r.case}`", ch.name.split(":")[0], ch.detail))
    if rows:
        lines += ["", table(("Case", "Output", "Difference from ANTs as built"), rows)]
    rand = [r for r in results if r.category.startswith("random") or "clock" in r.case]
    for r in rand:
        for ch in r.checks:
            if ch.name.endswith("larmorx vs ANTs' two runs"):
                lines.append(f"- `{r.case}` {ch.name.split(':')[0]}: {ch.detail}")
    pv = []
    for r in results:
        for ch in r.checks:
            m = re.match(r"ANTs: (.*); larmorx's default: (.*)", ch.detail)
            if ch.name == "partial-volume scores" and m:
                pv.append((f"`{r.case}`", m.group(1), m.group(2)))
    if pv:
        lines += [
            "",
            "**ANTS-106 on the partial-volume phantom** (known fractions; Dice per class, 4 = 1x2 "
            "and 5 = 2x3, and the share of voxels labelled as the truth). larmorx's default "
            "corrects ANTS-106 and, as everywhere, ITK-50 and ANTS-96; without the MRF only "
            "those two apply:",
            "",
            table(("Case", "ANTs", "larmorx's default"), pv),
        ]
    real = []
    for r in results:
        for ch in r.checks:
            if ch.name == "partial-volume voxels":
                code = re.match(r"ANTs exit (-?\d+)", r.reason)
                ants_text = f"stops (exit {code.group(1)})" if code else r.status
                real.append((f"`{r.case}`", ants_text, ch.detail))
    if real:
        lines += [
            "",
            "**sMRIPrep's call with partial-volume classes on fMRIPrep's T1w** (ANTS-106): ANTs "
            "stops; larmorx's default labels",
            "",
            table(("Case", "ANTs", "larmorx's default: voxels per class"), real),
        ]
    from larmorx_validation.parity import fmriprep_nodes

    return lines + fmriprep_nodes.record_lines(PROGRAM, results)


def suite() -> Suite:
    return Suite(
        name="ants-atropos",
        title="Atropos",
        tool="`larmorx ants Atropos` / `lx.ants.atropos` (crates larmorx-ants, larmorx-image)",
        reference=f"Atropos from {ants_version()}, the binary built by "
        "`scripts/build_ants_oracle.sh`, run with one thread",
        thresholds=[
            ("Exit status", "both produce their outputs, or neither does (same status)"),
            (
                "Messages",
                "identical text where compared (stdout; `-v` without the ITK base classes' lines)",
            ),
            ("Header", "every field of the 348-byte header identical"),
            ("Data type, shape, affine", f"identical (affine within {AFFINE_ATOL:g} mm)"),
            ("larmorx's threads", f"{LX_THREADS} threads and 1 thread give the same bits"),
            (
                "Values",
                "bit-identical to ANTs with correctly rounded math (CORE-MATH preloaded); with "
                f"ANTs as built, bit-identity reported and NCC > {NCC_MIN} where posteriors "
                "differ",
            ),
            (
                "Reported",
                f"ANTs' thread band ({', '.join(map(str, BAND_THREADS))} threads vs 1), "
                "ANTs' seed band (two "
                "clock-seeded runs), the distance to fMRIPrep's own files",
            ),
        ],
        cases=cases_for,
        run_case=run_case,
        packages=("nibabel",),
        highlights=highlights,
        notes=(
            "ANTs runs as its own binary, larmorx in-process (`larmorx._core.cli_main`), with "
            "ANTs' defects reproduced (`LARMORX_UPSTREAM_DEFECTS=reproduce`). Outputs are read "
            "with nibabel (values as stored) and the headers from their raw bytes. Inputs named "
            "`gen:atropos-*` are built by this suite (seeded); the real cases read the fMRIPrep "
            "oracle runs and the OASIS30ANTs template in `oracles/`."
        ),
    )


__all__ = ["all_cases", "cases_for", "distance", "run_case", "suite"]
