# SPDX-License-Identifier: Apache-2.0
"""Parity of ``larmorx ants antsRegistration`` with ANTs 2.6.5's ``antsRegistration`` (every
stage; the SyN cases are in :mod:`ants_registration_syn`, the BSplineSyN,
GaussianDisplacementField, BSplineDisplacementField and BSpline cases in
:mod:`ants_registration_bspline`, the Exponential, BSplineExponential, TimeVaryingVelocityField
and TimeVaryingBSplineVelocityField cases and the corrections' paths in
:mod:`ants_registration_velocity`).

ANTs runs as its own binary with one thread and a fixed sampling seed
(``ANTS_RANDOM_SEED``; without one ANTs seeds from the clock and is not repeatable); larmorx
runs in-process (its Python console entry point) with 4 threads, which must not matter, with
ANTs' defects reproduced (``LARMORX_UPSTREAM_DEFECTS=reproduce``, D13). Compared:

- the **exit status**;
- **standard output** (``-v 1``: the stage set-up, ITK's image dumps, every ``DIAGNOSTIC``
  line with the metric and convergence values to 12 digits), after masking what cannot be
  reproduced: object addresses, modification times and reference counts, timings, the build's
  source paths in exception prints, the signs of printed NaNs (unspecified by IEEE 754), and
  the two output directories;
- **standard error**, larmorx's notice lines removed;
- **every file** each program writes: transform files byte for byte (``.mat``, ``.txt``),
  HDF5 transform files dataset by dataset (``/OSName`` and ``/OSVersion`` aside), images
  (warped images, ``Warp``/``InverseWarp`` displacement fields, interval volumes) by their
  header bytes and every value.

ANTs computes ``exp``, ``log``, ``sin`` and ``cos`` with glibc, which does not always round
correctly; larmorx rounds correctly (CLAUDE.md rule 5). Where the first comparison is not
identical, ANTs runs again with CORE-MATH's functions preloaded
(``scripts/build_crmath_preload.sh``): larmorx must then be identical. Both results are
reported, the unmodified ANTs first. Cases marked for the **thread band** also run ANTs at
2, 4 and 8 threads and report how far its final parameters move from its one-thread result,
next to larmorx's distance from ANTs.

A **fix pass** runs larmorx in its default mode (defects corrected) and reports the defects
it names and what they changed.

Real cases repeat the ``antsRegistration`` command lines fMRIPrep 25.2.6 ran in the oracle runs
(``oracles/fmriprep-25.2.6``), on the inputs it ran them on; they are skipped where those runs
are missing. sMRIPrep's brain extraction and template registration run whole (Rigid, Affine,
SyN) at full size in the full tier, and on every third voxel of their images (``real-syn-small``)
in the standard tier (its first run) and the full tier (the others). The template registration
runs again with its SyN stage replaced by antsRegistrationSyN.sh's ``BSplineSyN[0.1,26,0,3]``,
as that script's ``-t b`` would run it: on every third voxel (``real-bsplinesyn-small``: the
first run in the standard tier, the others in the full tier) and, for the first run, full size
(``real-bsplinesyn``, full tier).
"""

from __future__ import annotations

import gzip
import os
import re
import subprocess
from dataclasses import dataclass
from pathlib import Path

import numpy as np

from larmorx_validation.parity import ants_inputs, ants_point_sets, defects
from larmorx_validation.parity.ants_programs import ants_bin, ants_version, crmath_preload
from larmorx_validation.parity.ants_registration_bspline import bspline_cases
from larmorx_validation.parity.ants_registration_bspline import (
    transform_file as bspline_transform_file,
)
from larmorx_validation.parity.ants_registration_syn import syn_cases
from larmorx_validation.parity.ants_registration_syn import transform_file as syn_transform_file
from larmorx_validation.parity.ants_registration_velocity import velocity_cases
from larmorx_validation.parity.harness import (
    BOTH_ERROR,
    EXPECTED,
    SKIPPED,
    Case,
    CaseResult,
    CheckList,
    Outcome,
    Suite,
    in_tier,
)

PROGRAM = "antsRegistration"
SEED = "42"
#: How long a case whose ANTs never finishes runs before it is stopped.
HANG_SECONDS = 30
#: How long any other ANTs run may take: sMRIPrep's full-size template registration takes ANTs
#: hours at one thread.
ANTS_TIMEOUT = 12 * 3600
#: larmorx's thread count (its result does not depend on it): 4 unless
#: ``LARMORX_PARITY_LX_THREADS`` says otherwise (a shared machine: parallel jobs times threads
#: stay within the machine's share).
LX_THREADS = int(os.environ.get("LARMORX_PARITY_LX_THREADS", "4"))
#: The real cases' small versions keep every SMALL-th voxel of their images.
SMALL = 3
#: ANTs' thread band: 2, 4, 8 threads unless ``LARMORX_PARITY_BAND_THREADS``
#: (e.g. ``2,3`` on a shared machine) says otherwise.
BAND_THREADS = tuple(
    int(v) for v in os.environ.get("LARMORX_PARITY_BAND_THREADS", "2,4,8").split(",") if v
)
ORACLE_RUNS = "oracles/fmriprep-25.2.6"
#: The threshold of a check against ANTs as built that only reports (ANTs with CORE-MATH decides).
GLIBC = "reported (glibc's maths; ANTs with CORE-MATH decides)"


@dataclass(frozen=True)
class RegCase:
    """One ``antsRegistration`` command line, compared with larmorx's.

    ``args`` use placeholders: ``{out}`` the program's output directory, ``{gen:name}`` a
    generated input, ``{in:path}`` a ``larmorx-testdata`` file, ``{oracle:run/path}`` a file of
    an fMRIPrep oracle run, ``{missing:name}`` a file that does not exist, ``{gen-points:name}``
    a point-set file (:mod:`ants_point_sets`). ``expect`` is ``"pass"``, ``"error"`` (both
    fail), ``"crash"`` (ANTs crashes or aborts: everything it printed before must match, and
    larmorx exits 1 with a message), ``"hang"`` (ANTs never finishes: larmorx refuses) or
    ``"divergence"`` (a documented difference, ``reason``). ``seed`` is ``ANTS_RANDOM_SEED``
    (``None``: unset). ``band`` runs ANTs' thread band.
    """

    id: str
    category: str
    description: str
    args: tuple[str, ...]
    tier: str = "standard"
    expect: str = "pass"
    reason: str = ""
    seed: str | None = SEED
    band: bool = False
    #: Defects whose notice the fix pass may name without a visible change (besides ANTS-62's
    #: and ITK-41's), as the other ANTs suites' ``silent_ok`` (a permanent case of random
    #: testing whose other stages name corrections this input does not exercise).
    silent_ok: tuple[str, ...] = ()


# ------------------------------------------------------------------------------------------------
# Generated inputs


def _head(shape: tuple[int, ...], seed: int, scale: float = 1.0) -> np.ndarray:
    """A head-like phantom: an ellipsoid head, a brighter brain, darker ventricles, a gradient
    along x and Gaussian noise (seeded)."""
    rng = np.random.default_rng(seed)
    idx = np.indices(shape, dtype=float)
    c = [(s - 1) / 2 for s in shape]
    radii = (15, 17, 13, 6)[: len(shape)]
    r2 = sum(((idx[k] - c[k]) / (radii[k] * scale)) ** 2 for k in range(len(shape)))
    vent = sum(
        ((idx[k] - c[k] - (2 if k == 0 else 0)) / ((4, 6, 3, 3)[k] * scale)) ** 2
        for k in range(len(shape))
    )
    img = 200 * (r2 < 1) + 300 * (r2 < 0.6) - 250 * (vent < 1) + 5 * idx[0] * (r2 < 1)
    return img + rng.normal(0, 10, shape)


def _moved(img: np.ndarray, degrees: float, shift: tuple[float, ...]) -> np.ndarray:
    """``img`` rotated about the first two axes' centre and shifted (linear interpolation)."""
    from scipy.ndimage import map_coordinates

    th = np.deg2rad(degrees)
    idx = np.indices(img.shape, dtype=float)
    c = [(s - 1) / 2 for s in img.shape]
    x, y = idx[0] - c[0], idx[1] - c[1]
    coords = [
        np.cos(th) * x - np.sin(th) * y + c[0] + shift[0],
        np.sin(th) * x + np.cos(th) * y + c[1] + shift[1],
    ]
    coords += [idx[k] + shift[k] for k in range(2, img.ndim)]
    return map_coordinates(img, coords, order=1) * 1.3 + 20


def _save(
    data: np.ndarray, affine: np.ndarray, path: Path, zooms: tuple[float, ...] | None = None
) -> None:
    import nibabel as nib

    img = nib.Nifti1Image(
        np.asarray(data, dtype=np.float32 if data.dtype != np.uint8 else np.uint8), affine
    )
    if zooms is not None:
        img.header.set_zooms(zooms)
    nib.save(img, path)


def _affine(
    spacing: float, origin: tuple[float, float, float], oblique: bool = False
) -> np.ndarray:
    a = np.diag([spacing, spacing, spacing, 1.0])
    if oblique:
        a[:3, :3] = ants_inputs.oblique_affine()[:3, :3]
    a[:3, 3] = origin
    return a


@ants_inputs.builder("reg-fixed")
def _(t):
    """A 40 x 44 x 36 head phantom, 2 mm voxels."""
    _save(_head((40, 44, 36), 0), _affine(2.0, (-40, -44, -36)), t)


@ants_inputs.builder("reg-moving")
def _(t):
    """The phantom rotated 6 degrees about z, shifted, scaled by 1.3 plus 20, its origin moved."""
    _save(_moved(_head((40, 44, 36), 0), 6, (1.5, -2.0, 1.0)), _affine(2.0, (-37, -45, -34)), t)


@ants_inputs.builder("reg-mask")
def _(t):
    """The phantom's head region (uint8)."""
    idx = np.indices((40, 44, 36), dtype=float)
    c = [19.5, 21.5, 17.5]
    r2 = ((idx[0] - c[0]) / 15) ** 2 + ((idx[1] - c[1]) / 17) ** 2 + ((idx[2] - c[2]) / 13) ** 2
    _save((r2 < 0.8).astype(np.uint8), _affine(2.0, (-40, -44, -36)), t)


@ants_inputs.builder("reg-fixed-oblique")
def _(t):
    """The fixed phantom on an oblique grid (1.5 x 1.2 x 2 mm, rotated)."""
    a = ants_inputs.oblique_affine()
    a[:3, 3] = (-30, -26, -36)
    _save(_head((40, 44, 36), 0), a, t)


@ants_inputs.builder("reg-moving-oblique")
def _(t):
    """The moving phantom on an oblique grid."""
    a = ants_inputs.oblique_affine()
    a[:3, 3] = (-28, -27, -34)
    _save(_moved(_head((40, 44, 36), 0), 6, (1.5, -2.0, 1.0)), a, t)


@ants_inputs.builder("reg-fixed-2d")
def _(t):
    """A 2D slice of the fixed phantom (2 mm pixels)."""
    a = np.diag([2.0, 2.0, 1.0, 1.0])
    a[:2, 3] = (-40, -44)
    _save(_head((40, 44, 36), 0)[:, :, 18], a, t)


@ants_inputs.builder("reg-moving-2d")
def _(t):
    """A 2D slice of the moving phantom, its origin moved."""
    a = np.diag([2.0, 2.0, 1.0, 1.0])
    a[:2, 3] = (-37, -45)
    _save(_moved(_head((40, 44, 36), 0), 6, (1.5, -2.0, 1.0))[:, :, 18], a, t)


@ants_inputs.builder("reg-fixed-4d")
def _(t):
    """Four volumes of the fixed phantom at 4 mm, each scaled, with noise (spacing 4, 4, 4, 2)."""
    base = _head((40, 44, 36), 0)[::2, ::2, ::2]
    rng = np.random.default_rng(1)
    data = np.stack([base * (1 + 0.1 * k) for k in range(4)], -1) + rng.normal(
        0, 3, (*base.shape, 4)
    )
    a = np.diag([4.0, 4.0, 4.0, 1.0])
    a[:3, 3] = (-40, -44, -36)
    _save(data, a, t, zooms=(4.0, 4.0, 4.0, 2.0))


@ants_inputs.builder("reg-moving-4d")
def _(t):
    """Four volumes of the moving phantom at 4 mm, its origin moved."""
    base = _moved(_head((40, 44, 36), 0), 6, (1.5, -2.0, 1.0))[::2, ::2, ::2]
    data = np.stack([base * (1 + 0.1 * k) for k in range(4)], -1)
    a = np.diag([4.0, 4.0, 4.0, 1.0])
    a[:3, 3] = (-37, -45, -34)
    _save(data, a, t, zooms=(4.0, 4.0, 4.0, 2.0))


@ants_inputs.builder("reg-constant")
def _(t):
    """A constant image on the fixed grid (every metric degenerate)."""
    _save(np.full((40, 44, 36), 100.0), _affine(2.0, (-40, -44, -36)), t)


@ants_inputs.builder("reg-far")
def _(t):
    """The moving phantom 500 mm away: no sample point maps inside it."""
    _save(_moved(_head((40, 44, 36), 0), 6, (1.5, -2.0, 1.0)), _affine(2.0, (460, -45, -34)), t)


# ------------------------------------------------------------------------------------------------
# Cases

F, M, K = "{gen:reg-fixed}", "{gen:reg-moving}", "{gen:reg-mask}"
F2, M2 = "{gen:reg-fixed-2d}", "{gen:reg-moving-2d}"
F4, M4 = "{gen:reg-fixed-4d}", "{gen:reg-moving-4d}"
FO, MO = "{gen:reg-fixed-oblique}", "{gen:reg-moving-oblique}"
OUT = "[{out}/r,{out}/w.nii.gz,{out}/iw.nii.gz]"
PRE = "{out}/r"


def mattes(
    f: str = F, m: str = M, sampling: str = "Regular", p: str = "0.25", bins: str = "32"
) -> str:
    return f"Mattes[{f},{m},1,{bins},{sampling},{p}]"


def stage(
    t: str, metric: str, c: str = "[10x5,1e-6,5]", f: str = "2x1", s: str = "1x0vox"
) -> tuple[str, ...]:
    return ("-t", t, "-m", metric, "-c", c, "-f", f, "-s", s)


def reg(
    case_id: str, category: str, description: str, *args: str, d: str = "3", out: str = OUT, **kw
) -> RegCase:
    return RegCase(case_id, category, description, ("-d", d, "-v", "1", "-o", out, *args), **kw)


def _syn_case(
    case_id: str,
    category: str,
    description: str,
    *args: str,
    d: str = "3",
    out: str = OUT,
    verbose: bool = True,
    **kw,
) -> RegCase:
    head = ("-d", d, *(("-v", "1") if verbose else ()), "-o", out)
    return RegCase(case_id, category, description, (*head, *args), **kw)


def all_cases() -> list[RegCase]:
    T, ME, S, INI, OUTS, E = "transforms", "metrics", "sampling", "initial", "outputs", "errors"
    cases = [
        reg(
            "smoke/rigid",
            "smoke",
            "Rigid, Mattes, regular sampling, 2 levels, warped images",
            *stage("Rigid[0.1]", mattes()),
            tier="smoke",
            band=True,
        ),
        reg(
            "smoke/affine-float",
            "smoke",
            "Affine, MeanSquares, random sampling, --float 1",
            "--float",
            "1",
            *stage("Affine[0.1]", f"MeanSquares[{F},{M},1,0,Random,0.3]"),
            tier="smoke",
            band=True,
        ),
        reg(
            "smoke/composite",
            "smoke",
            "-a 1: the composite and its inverse (HDF5)",
            "-a",
            "1",
            *stage("Rigid[0.1]", mattes(), c="[5,1e-6,5]", f="1", s="0"),
            out=PRE,
            tier="smoke",
        ),
    ]
    # Every linear transform in 2D, 3D and 4D, double and float, collapsed and not.
    for t in ("Translation", "Rigid", "Similarity", "Affine", "CompositeAffine"):
        cases.append(reg(f"{T}/{t}-3d", T, f"{t} in 3D", *stage(f"{t}[0.1]", mattes())))
        cases.append(
            reg(
                f"{T}/{t}-3d-z0",
                T,
                f"{t} in 3D, -z 0, -r [fixed,moving,1]",
                "-z",
                "0",
                "-r",
                f"[{F},{M},1]",
                *stage(f"{t}[0.1]", mattes(), c="[10,1e-6,5]", f="1", s="0"),
                out=PRE,
            )
        )
        cases.append(
            reg(
                f"{T}/{t}-3d-float",
                T,
                f"{t} in 3D, --float 1",
                "--float",
                "1",
                *stage(
                    f"{t}[0.1]",
                    f"MeanSquares[{F},{M},1,0,Regular,0.25]",
                    c="[10,1e-6,5]",
                    f="1",
                    s="0",
                ),
                out=PRE,
            )
        )
        cases.append(
            reg(f"{T}/{t}-2d", T, f"{t} in 2D", *stage(f"{t}[0.1]", mattes(F2, M2)), d="2", out=PRE)
        )
        cases.append(
            reg(
                f"{T}/{t}-2d-z0",
                T,
                f"{t} in 2D, -z 0, -r [fixed,moving,1]",
                "-z",
                "0",
                "-r",
                f"[{F2},{M2},1]",
                *stage(
                    f"{t}[0.1]",
                    f"MeanSquares[{F2},{M2},1,0,Regular,0.5]",
                    c="[10,1e-6,5]",
                    f="1",
                    s="0",
                ),
                d="2",
                out=PRE,
            )
        )
        cases.append(
            reg(
                f"{T}/{t}-2d-float",
                T,
                f"{t} in 2D, --float 1",
                "--float",
                "1",
                *stage(f"{t}[0.1]", mattes(F2, M2, "Random", "0.5"), c="[10,1e-6,5]", f="1", s="1"),
                d="2",
                out=PRE,
            )
        )
        cases.append(
            reg(
                f"{T}/{t}-4d",
                T,
                f"{t} in 4D (an affine transform in 4D)",
                *stage(f"{t}[0.1]", mattes(F4, M4), c="[6x4,1e-6,5]"),
                d="4",
                out=PRE,
            )
        )
    cases.append(
        reg(
            f"{T}/two-stages",
            T,
            "Rigid then Affine",
            *stage("Rigid[0.1]", mattes()),
            *stage("Affine[0.1]", mattes()),
        )
    )
    cases.append(
        reg(
            f"{T}/two-stages-z0",
            T,
            "Rigid then Affine, not collapsed",
            "-z",
            "0",
            *stage("Rigid[0.1]", mattes(), c="[5,1e-6,5]", f="1", s="0"),
            *stage("Affine[0.1]", mattes(), c="[5,1e-6,5]", f="1", s="0"),
            out=PRE,
        )
    )
    cases.append(
        reg(
            f"{T}/oblique", T, "Affine between oblique grids", *stage("Affine[0.1]", mattes(FO, MO))
        )
    )
    # Metrics.
    for name, metric in (
        ("mattes-mi-alias", f"MI[{F},{M},1,32,Regular,0.25]"),
        ("mi2", f"MI2[{F},{M},1,32,Regular,0.25]"),
        ("mi2-dense", f"MI2[{F},{M},1,20]"),
        ("meansquares", f"MeanSquares[{F},{M},1,0,Regular,0.25]"),
        ("gc", f"GC[{F},{M},1,0,Regular,0.25]"),
        ("gc-dense", f"GC[{F},{M},1,0]"),
        ("cc-sampled", f"CC[{F},{M},1,2,Regular,0.25]"),
        ("cc-dense", f"CC[{F},{M},1,3]"),
        ("mattes-gradient-filter", f"Mattes[{F},{M},1,32,Regular,0.25,1]"),
        ("cc-gradient-filter", f"CC[{F},{M},1,2,None,1,1]"),
    ):
        cases.append(
            reg(
                f"{ME}/{name}",
                ME,
                f"Rigid with {metric.split('[')[0]} ({name})",
                *stage("Rigid[0.1]", metric),
                out=PRE,
                band=True,
            )
        )
    # The gradient filter on a grid whose spacings are not powers of two: ITK stores each
    # derivative through a float (findings/ants-ai.md, "The moving image's gradient").
    for name, metric in (
        ("mattes-gradient-filter-oblique", f"Mattes[{FO},{MO},1,32,Regular,0.25,1]"),
        ("cc-gradient-filter-oblique", f"CC[{FO},{MO},1,2,None,1,1]"),
    ):
        cases.append(
            reg(
                f"{ME}/{name}",
                ME,
                f"Rigid with {metric.split('[')[0]} and the gradient filter, oblique grids",
                *stage("Rigid[0.1]", metric),
                out=PRE,
            )
        )
    for name, metric, d in (
        ("mi2-2d", f"MI2[{F2},{M2},1,24,Regular,0.5]", "2"),
        ("cc-2d", f"CC[{F2},{M2},1,4,None]", "2"),
        ("gc-2d", f"GC[{F2},{M2},1,0,Regular,0.5]", "2"),
        ("mi2-4d-gradient-filter", f"MI2[{F4},{M4},1,20,Regular,0.5,1]", "4"),
    ):
        cases.append(
            reg(
                f"{ME}/{name}",
                ME,
                f"{metric.split('[')[0]} in {d}D",
                *stage("Affine[0.1]", metric),
                d=d,
                out=PRE,
            )
        )
    for name, metric in (
        ("mi2-float", f"MI2[{F},{M},1,32,Random,0.3]"),
        ("cc-float", f"CC[{F},{M},1,2,Random,0.3]"),
        ("gc-float", f"GC[{F},{M},1,0,Random,0.3]"),
    ):
        cases.append(
            reg(
                f"{ME}/{name}",
                ME,
                f"{metric.split('[')[0]}, --float 1",
                "--float",
                "1",
                *stage("Rigid[0.1]", metric),
                out=PRE,
            )
        )
    cases.append(
        reg(
            f"{ME}/multi",
            ME,
            "two metrics in one stage (weights 1 and 0.5)",
            "-t",
            "Affine[0.1]",
            "-m",
            mattes(),
            "-m",
            f"MeanSquares[{F},{M},0.5,0,Regular,0.25]",
            "-c",
            "[10x5,1e-6,5]",
            "-f",
            "2x1",
            "-s",
            "1x0vox",
            out=PRE,
        )
    )
    cases.append(
        reg(
            f"{ME}/demons",
            ME,
            "Demons in a linear stage: ITK throws and ANTs drops the stage (ANTS-49)",
            *stage("Rigid[0.1]", f"Demons[{F},{M},1,0,Regular,0.25]"),
            out=PRE,
        )
    )
    cases.append(
        reg(
            f"{ME}/no-overlap",
            ME,
            "no sample maps inside the moving image (Mattes throws, ANTS-49)",
            *stage("Rigid[0.1]", mattes(F, "{gen:reg-far}")),
            out=PRE,
        )
    )
    # Sampling, smoothing, preprocessing, masks.
    cases += [
        reg(
            f"{S}/random",
            S,
            "Random 0.4",
            *stage("Rigid[0.1]", mattes(sampling="Random", p="0.4")),
            out=PRE,
        ),
        reg(
            f"{S}/regular-1",
            S,
            "Regular 1 (every voxel, perturbed)",
            *stage("Rigid[0.1]", mattes(p="1")),
            out=PRE,
        ),
        reg(
            f"{S}/sigmas-mm",
            S,
            "smoothing sigmas in mm",
            *stage("Rigid[0.1]", mattes(), s="2x1mm"),
            out=PRE,
        ),
        reg(
            f"{S}/shrink-4",
            S,
            "three levels, shrink 4x2x1",
            *stage("Rigid[0.1]", mattes(), c="[8x6x4,1e-6,5]", f="4x2x1", s="2x1x0vox"),
            out=PRE,
        ),
        reg(
            f"{S}/winsorize-histogram",
            S,
            "-w [0.005,0.995] -u 1",
            "-w",
            "[0.005,0.995]",
            "-u",
            "1",
            *stage("Rigid[0.1]", mattes()),
            out=PRE,
        ),
        reg(
            f"{S}/masks",
            S,
            "-x [mask,mask]",
            "-x",
            f"[{K},{K}]",
            *stage("Rigid[0.1]", mattes()),
            out=PRE,
        ),
        reg(
            f"{S}/fixed-mask-only",
            S,
            "-x mask (fixed only), dense",
            "-x",
            K,
            *stage("Rigid[0.1]", f"Mattes[{F},{M},1,32]"),
            out=PRE,
        ),
        reg(
            f"{S}/moving-mask-null",
            S,
            "-x [NULL,mask]",
            "-x",
            f"[NULL,{K}]",
            *stage("Rigid[0.1]", mattes()),
            out=PRE,
        ),
        reg(
            f"{S}/restrict-deformation",
            S,
            "-g 1x1x0x1x1x1: a rotation frozen",
            *stage("Rigid[0.1]", mattes()),
            "-g",
            "1x1x0x1x1x1",
            out=PRE,
        ),
        reg(
            f"{S}/zero-iterations",
            S,
            "a first level of 0 iterations runs none",
            *stage("Rigid[0.1]", mattes(), c="[0x5,1e-6,5]"),
            out=PRE,
        ),
        reg(
            f"{S}/no-seed",
            S,
            "no seed: ANTs seeds its sampling from the clock",
            *stage("Rigid[0.1]", mattes()),
            out=PRE,
            seed=None,
            expect="divergence",
            reason="without a seed ANTs seeds its sampling from the clock and its result changes from run to "
            "run; larmorx uses a fixed seed",
        ),
    ]
    # Initial transforms and state.
    cases += [
        reg(
            f"{INI}/geometry",
            INI,
            "-r [fixed,moving,0]",
            "-r",
            f"[{F},{M},0]",
            *stage("Rigid[0.1]", mattes()),
        ),
        reg(
            f"{INI}/moments",
            INI,
            "-r [fixed,moving,1]",
            "-r",
            f"[{F},{M},1]",
            *stage("Rigid[0.1]", mattes()),
        ),
        reg(
            f"{INI}/origins",
            INI,
            "-r [fixed,moving,2]",
            "-r",
            f"[{F},{M},2]",
            *stage("Rigid[0.1]", mattes()),
        ),
        reg(
            f"{INI}/fixed-initial",
            INI,
            "-q [fixed,moving,1]",
            "-q",
            f"[{F},{M},1]",
            *stage("Rigid[0.1]", mattes(), c="[5,1e-6,5]", f="1", s="0"),
            out=PRE,
        ),
        reg(
            f"{INI}/file",
            INI,
            "-r a transform file",
            "-r",
            "{gen-transform:initial.mat}",
            *stage("Affine[0.1]", mattes(), c="[5,1e-6,5]", f="1", s="0"),
            out=PRE,
        ),
        reg(
            f"{INI}/file-inverted",
            INI,
            "-r [file,1]",
            "-r",
            "[{gen-transform:initial.mat},1]",
            *stage("Affine[0.1]", mattes(), c="[5,1e-6,5]", f="1", s="0"),
            out=PRE,
        ),
        reg(
            f"{INI}/per-stage",
            INI,
            "--initialize-transforms-per-stage 1: each stage starts from the previous",
            "-z",
            "0",
            "-a",
            "1",
            "-i",
            "1",
            *stage("Translation[0.5]", mattes(), c="[6,1e-6,5]", f="1", s="0"),
            *stage("Rigid[0.1]", mattes(), c="[6,1e-6,5]", f="1", s="0"),
            *stage("Affine[0.1]", mattes(), c="[6,1e-6,5]", f="1", s="0"),
            out=PRE,
        ),
        reg(
            f"{INI}/per-stage-from-r",
            INI,
            "-i 1 with -r [fixed,moving,1]",
            "-z",
            "0",
            "-a",
            "1",
            "-i",
            "1",
            "-r",
            f"[{F},{M},1]",
            *stage("Rigid[0.1]", mattes(), c="[6,1e-6,5]", f="1", s="0"),
            out=PRE,
        ),
        reg(
            f"{INI}/restore-state",
            INI,
            "--restore-state from a transform file",
            "-k",
            "{gen-transform:initial.mat}",
            *stage("Affine[0.1]", mattes(), c="[5,1e-6,5]", f="1", s="0"),
            out=PRE,
        ),
        reg(
            f"{INI}/save-state",
            INI,
            "--save-state after linear stages: ANTs has no state and exits 1 (ANTS-50)",
            "-a",
            "1",
            "--save-state",
            "{out}/state.h5",
            *stage("Rigid[0.1]", mattes(), c="[5,1e-6,5]", f="1", s="0"),
            out=PRE,
        ),
        reg(
            f"{INI}/names-restored",
            INI,
            "-z 0 with a restored state: a file named after the wrong stage, then a read "
            "past the end of the names (ANTS-51)",
            "-z",
            "0",
            "-k",
            "{gen-transform:initial.mat}",
            *stage("Affine[0.1]", mattes(), c="[5,1e-6,5]", f="1", s="0"),
            out=PRE,
            expect="divergence",
            reason="ANTs reads past the end of its list of file names and crashes (ANTS-51); "
            "larmorx writes the same files up to there and stops with an error",
        ),
    ]
    cases.extend(_dimension_cases())
    # Outputs.
    for interp in (
        "NearestNeighbor",
        "BSpline[3]",
        "LanczosWindowedSinc",
        "MultiLabel",
        "Gaussian",
    ):
        cases.append(
            reg(
                f"{OUTS}/interpolation-{interp.split('[')[0]}",
                OUTS,
                f"warped images with -n {interp}",
                "-n",
                interp,
                *stage("Rigid[0.1]", mattes(), c="[5,1e-6,5]", f="1", s="0"),
            )
        )
    cases += [
        reg(
            f"{OUTS}/print-similarity",
            OUTS,
            "--print-similarity-measure-interval 2 (full-scale CC)",
            "--print-similarity-measure-interval",
            "2",
            *stage("Rigid[0.1]", mattes(), c="[6x4,1e-6,5]"),
            out=PRE,
        ),
        reg(
            f"{OUTS}/write-interval-volumes",
            OUTS,
            "--write-interval-volumes 3",
            "--write-interval-volumes",
            "3",
            *stage("Rigid[0.1]", mattes(), c="[6x4,1e-6,5]"),
            out=PRE,
        ),
    ]
    # Errors.
    cases += [
        RegCase(f"{E}/help", E, "--help: the menu, exit 0", ("--help",)),
        RegCase(f"{E}/no-arguments", E, "no arguments: the short menu", ()),
        reg(
            f"{E}/bad-number",
            E,
            "-c [1x,1e-6,5]: a number that does not convert",
            *stage("Rigid[0.1]", mattes(), c="[1x,1e-6,5]"),
            expect="error",
        ),
        reg(
            f"{E}/levels-mismatch",
            E,
            "two shrink factors for one level",
            *stage("Rigid[0.1]", mattes(), c="[5]", f="2x1", s="0"),
            expect="error",
        ),
        reg(
            f"{E}/missing-image",
            E,
            "a metric image that does not exist: ANTs uses the null image and crashes (ANTS-02)",
            *stage("Rigid[0.1]", mattes(m="{missing:none.nii.gz}")),
            expect="error",
        ),
        reg(
            f"{E}/unknown-transform",
            E,
            "an unknown transform name",
            *stage("Foo[0.1]", mattes()),
            expect="error",
        ),
        reg(
            f"{E}/save-state-without-composite",
            E,
            "--save-state without -a 1",
            "--save-state",
            "{out}/s.h5",
            *stage("Rigid[0.1]", mattes()),
            expect="error",
        ),
        reg(
            f"{E}/per-stage-with-collapse",
            E,
            "-i 1 -a 1 with collapsing",
            "-a",
            "1",
            "-i",
            "1",
            *stage("Rigid[0.1]", mattes()),
            expect="error",
        ),
        reg(
            f"{E}/restore-with-r",
            E,
            "--restore-state with -r",
            "-k",
            "{gen-transform:initial.mat}",
            "-r",
            "{gen-transform:initial.mat}",
            *stage("Rigid[0.1]", mattes()),
            expect="error",
        ),
    ]
    cases += _message_cases()
    cases += _point_set_cases()
    cases += syn_cases(_syn_case)
    cases += bspline_cases(_syn_case)
    cases += velocity_cases(_syn_case)
    cases += _panel_cases()
    # Real cases: fMRIPrep's command lines in the oracle runs.
    for run, node, band in (
        (
            "ds000005-sub-01-noreconall",
            "sub_01_wf/bold_task_mixedgamblestask_run_01_wf/bold_fit_wf/enhance_and_skullstrip_bold_wf/norm",
            True,
        ),
        (
            "ds005040-sub-002-noreconall",
            "sub_002_wf/bold_task_clip_run_1_wf/bold_fit_wf/enhance_and_skullstrip_bold_wf/norm",
            False,
        ),
        (
            "ds005040-sub-002-noreconall",
            "sub_002_wf/bold_task_clip_run_1_wf/bold_fit_wf/fmapreg_wf/coregister",
            True,
        ),
        (
            "ds005454-sub-16-noreconall",
            "sub_16_wf/bold_task_rest_wf/bold_fit_wf/fmapreg_wf/coregister",
            False,
        ),
    ):
        short = node.split("/")[-1] if node.endswith("coregister") else "boldref-affine"
        cases.append(
            RegCase(
                f"real/{run.split('-noreconall')[0]}/{short}",
                "real",
                f"fMRIPrep 25.2.6's {node.split('/')[-2]}/{node.split('/')[-1]} of {run}",
                (f"{{fmriprep:{run}/{node}}}",),
                band=band,
            )
        )
    # The linear stages of sMRIPrep's brain extraction (OASIS30ANTs, MI, Rigid then Affine)
    # and template registration (MNI152NLin2009cAsym, Mattes, Rigid then Affine) as fMRIPrep
    # ran them, without their SyN stage (A5).
    for run, subject in (
        ("ds000005-sub-01-noreconall", "sub_01_wf"),
        ("ds001600-sub-1-noreconall", "sub_1_wf"),
        ("ds005040-sub-002-noreconall", "sub_002_wf"),
        ("ds005454-sub-16-noreconall", "sub_16_wf"),
        ("ds005752-sub-ON01016-noreconall", "sub_ON01016_ses_01_wf"),
    ):
        for short, node in (
            ("brain-extraction", "anat_fit_wf/brain_extraction_wf/norm"),
            (
                "template",
                "anat_fit_wf/register_template_wf/_template_MNI152NLin2009cAsym/registration",
            ),
        ):
            first = run.startswith("ds000005") and short == "template"
            cases.append(
                RegCase(
                    f"real-linear/{run.split('-noreconall')[0]}/{short}",
                    "real-linear",
                    f"the linear stages of fMRIPrep 25.2.6's {node.split('/')[-1]} of {run} "
                    "(its SyN stage left out)",
                    (f"{{fmriprep-linear:{run}/{subject}/{node}}}",),
                    tier="standard" if first else "full",
                )
            )
            # The whole command, its SyN stage included (A5): on every third voxel of its
            # images (the standard tier's first run, the full tier's others), and as it ran.
            name = run.split("-noreconall")[0]
            cases.append(
                RegCase(
                    f"real-syn-small/{name}/{short}",
                    "real-syn-small",
                    f"fMRIPrep 25.2.6's {node.split('/')[-1]} of {run}, whole (Rigid, Affine, "
                    f"SyN), on one voxel in {SMALL} along each axis of its images",
                    (f"{{fmriprep-small:{run}/{subject}/{node}}}",),
                    tier="standard" if run.startswith("ds000005") else "full",
                    band=run.startswith("ds000005"),
                )
            )
            cases.append(
                RegCase(
                    f"real-syn/{name}/{short}",
                    "real-syn",
                    f"fMRIPrep 25.2.6's {node.split('/')[-1]} of {run}, whole (Rigid, Affine, "
                    "SyN), full size",
                    (f"{{fmriprep-whole:{run}/{subject}/{node}}}",),
                    tier="full",
                )
            )
            if short == "template":
                # antsRegistrationSyN.sh -t b: its SyN stage as BSplineSyN[0.1,26,0,3].
                cases.append(
                    RegCase(
                        f"real-bsplinesyn-small/{name}/{short}",
                        "real-bsplinesyn-small",
                        f"fMRIPrep 25.2.6's {node.split('/')[-1]} of {run} with "
                        "BSplineSyN[0.1,26,0,3] (antsRegistrationSyN.sh's) for its SyN stage, on "
                        f"one voxel in {SMALL} along each axis of its images",
                        (f"{{fmriprep-small-bsplinesyn:{run}/{subject}/{node}}}",),
                        tier="standard" if run.startswith("ds000005") else "full",
                        band=run.startswith("ds000005"),
                    )
                )
                # Full size: the first run only (hours per run in ANTs at one thread).
                if run.startswith("ds000005"):
                    cases.append(
                        RegCase(
                            f"real-bsplinesyn/{name}/{short}",
                            "real-bsplinesyn",
                            f"fMRIPrep 25.2.6's {node.split('/')[-1]} of {run} with "
                            "BSplineSyN[0.1,26,0,3] for its SyN stage, full size",
                            (f"{{fmriprep-whole-bsplinesyn:{run}/{subject}/{node}}}",),
                            tier="full",
                        )
                    )
                # The velocity-field stages in place of its SyN stage (A5 part 3), with its
                # levels, iterations and metric: on every third voxel (the standard tier's
                # first run, the full tier's others), and the exponential ones full size for the
                # first run (full tier).
                for kind, stage_text in VELOCITY_STAGES.items():
                    first_run = run.startswith("ds000005")
                    cases.append(
                        RegCase(
                            f"real-{kind}-small/{name}/{short}",
                            f"real-{kind}-small",
                            f"fMRIPrep 25.2.6's {node.split('/')[-1]} of {run} with "
                            f"{stage_text} for its SyN stage, on one voxel in {SMALL} along "
                            "each axis of its images",
                            (f"{{fmriprep-small-{kind}:{run}/{subject}/{node}}}",),
                            tier="standard" if first_run else "full",
                            band=first_run and kind in ("exponential", "time-varying"),
                        )
                    )
                    if first_run and kind in ("exponential", "bspline-exponential"):
                        cases.append(
                            RegCase(
                                f"real-{kind}/{name}/{short}",
                                f"real-{kind}",
                                f"fMRIPrep 25.2.6's {node.split('/')[-1]} of {run} with "
                                f"{stage_text} for its SyN stage, full size",
                                (f"{{fmriprep-whole-{kind}:{run}/{subject}/{node}}}",),
                                tier="full",
                            )
                        )
    return cases


#: The two whole-head registrations of sMRIPrep (hours at one thread at full size).
HEAVY_NODES = (
    "anat_fit_wf/brain_extraction_wf/norm",
    "anat_fit_wf/register_template_wf/registration",
)


def _panel_cases() -> list[RegCase]:
    """The data panel's tier: every antsRegistration node of the fourteen fMRIPrep oracle runs,
    its ``command.txt`` replayed exactly (the template registration's inputs that nipype did not
    keep made again exactly, its initial transform by the ANTs oracle at one thread:
    :mod:`~larmorx_validation.parity.niworkflows_inputs`); the two whole-head registrations
    again on one voxel in :data:`SMALL` along each axis of their images (``.../small``). ANTs'
    thread band on the first call of each kind (the small versions for the whole-head ones)."""
    from larmorx_validation.parity import fmriprep_nodes

    out = []
    banded: set[str] = set()
    for c in fmriprep_nodes.calls(PROGRAM):
        spec = f"{c.run}/{c.node}"
        heavy = c.norm in HEAVY_NODES
        # fMRIPrep's seed (its --random-seed 0), as the node's environment records it.
        seed = c.environ("ANTS_RANDOM_SEED") or SEED
        out.append(
            RegCase(
                fmriprep_nodes.PREFIX + c.id,
                fmriprep_nodes.CATEGORY,
                fmriprep_nodes.describe(c) + (", full size" if heavy else ""),
                (f"{{panel-node:{spec}}}",),
                tier="panel",
                band=not heavy and c.norm not in banded,
                seed=seed,
            )
        )
        if heavy:
            out.append(
                RegCase(
                    fmriprep_nodes.PREFIX + c.id + "/small",
                    "panel-fmriprep-small",
                    fmriprep_nodes.describe(c)
                    + f", on one voxel in {SMALL} along each axis of its images",
                    (f"{{panel-node-small:{spec}}}",),
                    tier="panel",
                    band=c.norm not in banded,
                    seed=seed,
                )
            )
        banded.add(c.norm)
    return out


def _panel_args(spec: str, out: Path, tmp: Path, small: int = 0) -> list[str]:
    """An oracle run's node (``run/node``, the node below ``work/``) as it ran: its arguments
    (:mod:`~larmorx_validation.parity.fmriprep_nodes`), the files nipype did not keep made
    again, with ``small`` every image replaced by its every ``small``-th voxel, the output
    prefix moved to ``out``."""
    from larmorx_validation.parity import fmriprep_nodes, niworkflows_inputs

    run, node = spec.split("/", 1)
    calls = [c for c in fmriprep_nodes.calls(PROGRAM) if c.run == run and c.node == node]
    if not calls:
        raise Outcome(SKIPPED, f"the fMRIPrep oracle node {spec} is not here")
    c = calls[0]
    args = niworkflows_inputs.mapped_args(c) if c.norm == niworkflows_inputs.NODE else list(c.args)
    args = " ".join(args).replace("[ ", "[").replace(" ]", "]").replace(", ", ",").split()
    if small:
        args = _small(args, tmp, small)
    i = args.index("--output")
    value = args[i + 1]
    if value.startswith("["):
        parts = value[1:-1].split(",")
        args[i + 1] = (
            "["
            + ",".join([f"{out}/transform"] + [f"{out}/{Path(p).name}" for p in parts[1:]])
            + "]"
        )
    else:
        args[i + 1] = f"{out}/transform"
    return [*args, "-v", "1"]


def _message_cases() -> list[RegCase]:
    """The command line's other checks and messages: help, version, missing and inconsistent
    options, conversions, unknown names, the point-set metrics and MINC larmorx refuses."""
    E = "errors"
    PFX = "{out}/o"
    mat = f"Mattes[{F},{M},1,32]"
    base = ("-d", "3", "-o", PFX, "-t", "Rigid[0.1]", "-m", mat, "-c", "10", "-f", "1", "-s", "0")
    tail = ("-c", "10", "-f", "1", "-s", "0")

    def err(name: str, description: str, *args: str, **kw) -> RegCase:
        kw.setdefault("expect", "error")
        return RegCase(f"{E}/{name}", E, description, args, **kw)

    def ok(name: str, description: str, *args: str, **kw) -> RegCase:
        return RegCase(f"{E}/{name}", E, description, args, **kw)

    return [
        ok("short-help", "-h: the short menu, exit 0", "-h"),
        ok(
            "version",
            "--version",
            "--version",
            expect="divergence",
            reason="larmorx prints its own version and the ANTs version it ports",
        ),
        err("version-0", "--version 0: no version, then no dimension", "--version", "0", expect="divergence", reason="ANTs exits 1 without a word (the reason is printed only with --verbose, which this command line does not set); larmorx prints the reason (ANTS-45)"),
        err("no-dimension", "no -d, verbose: the dimension from the first fixed image is unknown", "-o", PFX, "-v"),
        err("no-dimension-quiet", "no -d, quiet", "-o", PFX, expect="divergence", reason="ANTs exits 1 without a word (the reason is printed only with --verbose, which this command line does not set); larmorx prints the reason (ANTS-45)"),
        err("bad-dimension", "-d 5", "-d", "5", "-v"),
        err("invalid-flag", "an option ANTs does not know", "-d", "3", "--nope", "1", "-y"),
        err(
            "unbalanced-bracket",
            "-t Rigid[0.1 (no closing bracket): the rest of the line joins the transform",
            "-d", "3", "-t", "Rigid[0.1", "-v",
            expect="divergence", reason="ANTs exits 1 without a word (the reason is printed only with --verbose, which this command line does not set); larmorx prints the reason (ANTS-45)",
        ),
        err("misplaced-bracket", "-t Rig]id[0.1]", "-d", "3", "-t", "Rig]id[0.1]", "-v"),
        err("no-transform", "no -t", "-d", "3", "-v", "-o", PFX),
        err("no-metric", "no -m", "-d", "3", "-v", "-o", PFX, "-t", "Rigid[0.1]"),
        err("no-convergence", "no -c", "-d", "3", "-v", "-o", PFX, "-t", "Rigid[0.1]", "-m", mat),
        err("no-shrink-factors", "no -f", "-d", "3", "-v", "-o", PFX, "-t", "Rigid[0.1]", "-m", mat, "-c", "10"),
        err(
            "no-smoothing-sigmas",
            "no -s",
            "-d", "3", "-v", "-o", PFX, "-t", "Rigid[0.1]", "-m", mat, "-c", "10", "-f", "1",
        ),
        err("no-output", "no -o", "-d", "3", "-v", "-t", "Rigid[0.1]", "-m", mat, *tail),
        err("per-stage-without-composite", "-i 1 without -a 1", *base, "-v", "-i", "1"),
        err("convergence-count", "two -c for one stage", *base, "-v", "-c", "10"),
        ok("window-1", "-c [10,1e-6,1]: a convergence window of 1", *base[:-6], "-v", "-c", "[10,1e-6,1]", "-f", "1", "-s", "0"),
        err("bad-iterations", "-c [1ox,1e-6,5]", *base[:-6], "-v", "-c", "[1ox,1e-6,5]", "-f", "1", "-s", "0"),
        err("unknown-metric", "an unknown metric", *base[:6], "-v", "-m", f"Foo[{F},{M},1,32]", *base[8:]),
        err("bad-sampling", "an unknown sampling strategy", *base[:6], "-v", "-m", f"Mattes[{F},{M},1,32,Sparse,0.2]", *base[8:]),
        err("sigmas-mismatch", "three sigmas for two levels", *base[:-6], "-v", "-c", "10x5", "-f", "2x1", "-s", "1x0x0"),
        err("stage-without-metric", "a second -t without its -m", *base, "-v", "-t", "Affine[0.1]", *tail),
        ok("sampling-above-1", "a sampling percentage of 1.5", *base[:6], "-v", "-m", f"Mattes[{F},{M},1,32,Regular,1.5]", *base[8:]),
        err("missing-initial", "-r a transform file that does not exist", *base, "-v", "-r", "{missing:none.mat}"),
        err(
            "minc",
            "--minc 1: MINC transform files",
            *base,
            "-v",
            "--minc",
            "1",
            expect="divergence",
            reason="the oracle has no MINC transform IO (it prints ITK's error and exits 0); "
            "larmorx refuses MINC transform files with a message",
        ),
    ]  # fmt: skip


# ------------------------------------------------------------------------------------------------
# Point-set metrics (A4 part 4)

PSF, PSM = "{gen:ps-labels-fixed}", "{gen:ps-labels-moving}"
PSK, PSKM = "{gen:ps-mask}", "{gen:ps-mask-moving}"
PSI, PSIM = "{gen:ps-intensity-fixed}", "{gen:ps-intensity-moving}"


def _ps(
    case_id: str,
    description: str,
    *metrics: str,
    t: str = "Rigid[0.1]",
    c: str = "5",
    f: str = "1",
    s: str = "0",
    mask: str | None = PSK,
    d: str = "3",
    extra: tuple[str, ...] = (),
    out: str = PRE,
    **kw,
) -> RegCase:
    args: list[str] = ["-t", t]
    for m in metrics:
        args += ["-m", m]
    args += ["-c", c, "-f", f, "-s", s]
    if mask is not None:
        args += ["-x", mask]
    return reg(
        f"point-sets/{case_id}", "point-sets", description, *args, *extra, d=d, out=out, **kw
    )


def _point_set_cases() -> list[RegCase]:
    icp = f"ICP[{PSF},{PSM},1]"
    txt = "{gen-points:fixed.txt}"
    mtxt = "{gen-points:moving.txt}"
    igdm = f"IGDM[{PSI},{PSIM},1,{PSK},{PSKM}]"
    head_mattes = f"Mattes[{PSI},{PSIM},1,32]"
    crash = "crash"
    F2D, M2D, K2D = "{gen:ps-labels-fixed-2d}", "{gen:ps-labels-moving-2d}", "{gen:ps-mask-2d}"
    return [
        _ps("icp", "ICP between two label images, the fixed mask as the virtual domain", icp, tier="smoke", band=True),
        _ps("icp-affine-two-levels", "ICP, Affine, two levels (shrink 2x1, smoothing ignored)", icp, t="Affine[0.1]", c="5x5", f="2x1", s="1x0"),
        _ps("icp-float", "ICP with --float 1", icp, extra=("--float", "1")),
        _ps("icp-text", "ICP between text point sets (a final newline: ANTs reads one more record, ANTS-62)", f"ICP[{txt},{mtxt},1]"),
        _ps("icp-text-no-final-newline", "A text point set without a final newline", f"ICP[{{gen-points:fixed-no-final-newline.txt}},{mtxt},1]"),
        _ps("icp-vtk", "ICP between ASCII VTK point sets", "ICP[{gen-points:fixed.vtk},{gen-points:moving.vtk},1]"),
        _ps("icp-vtk-binary", "A binary VTK point set: its labels read past one value (ANTS-63)", "ICP[{gen-points:fixed-binary.vtk},{gen-points:moving.vtk},1]"),
        _ps("icp-vtk-no-scalars", "A VTK point set without labels: label 0 for every point", "ICP[{gen-points:fixed-no-scalars.vtk},{gen-points:moving.vtk},1]"),
        _ps("icp-boundary", "ICP on the labels' contours (boundaryPointsOnly)", f"ICP[{PSF},{PSM},1,1,1]"),
        _ps("icp-not-bijective", "A label only the fixed set has: ITK throws, the stage is dropped (ITK-22)", f"ICP[{PSF},{{gen:ps-labels-moving-partial}},1]"),
        _ps("icp-empty", "An empty fixed point set: the scales estimator throws", f"ICP[{{gen:ps-labels-zero}},{PSM},1]"),
        _ps(
            "icp-sampled",
            "ICP on half of each point set (a sampling percentage of 0.5)",
            f"ICP[{PSF},{PSM},1,0.5]",
            expect="divergence",
            reason="ANTs draws the points of a sampling percentage below 1 with a generator seeded from the clock (ANTS-64): two runs differ; larmorx seeds it from the registration's seed",
        ),
        _ps("icp-2d", "ICP in 2D", f"ICP[{F2D},{M2D},1]", mask=K2D, d="2"),
        _ps("icp-4d", "ICP in 4D, Affine", "ICP[{gen:ps-labels-fixed-4d},{gen:ps-labels-moving-4d},1]", t="Affine[0.1]", c="3", mask="{gen:ps-mask-4d}", d="4"),
        _ps("icp-fixed-initial", "ICP with an initial fixed transform (-q)", icp, extra=("-q", "{gen-transform:initial.mat}")),
        _ps("icp-moving-initial", "ICP with an initial moving transform (-r)", icp, extra=("-r", "{gen-transform:initial.mat}")),
        _ps("icp-images-initial", "ICP with -r [fixed,moving,1] from intensity images", icp, extra=("-r", f"[{PSI},{PSIM},1]")),
        _ps("icp-same-sets", "ICP of a point set with itself", f"ICP[{PSF},{PSF},1]"),
        _ps("pse", "PSE (sigma 1, 50 neighbours)", f"PSE[{PSF},{PSM},1,1,0,1,50]", band=True),
        _ps("pse-boundary", "PSE on the contours, 20 neighbours", f"PSE[{PSF},{PSM},1,1,1,1,20]"),
        _ps("pse-many-neighbours", "PSE asking for more neighbours than a label has (ITK's warning at every call)", f"PSE[{PSF},{PSM},1,1,0,1,500]", tier="full"),
        _ps("pse-sigma-zero", "PSE with a point-set sigma of 0: ITK throws", f"PSE[{PSF},{PSM},1,1,0,0,50]"),
        _ps("pse-text-2d", "PSE between 2D text point sets", "PSE[{gen-points:fixed-2d.txt},{gen-points:moving-2d.txt},1,1,0,1,10]", mask=K2D, d="2"),
        _ps("jhct", "JHCT with anisotropic covariances (sigma 1, alpha 1.1)", f"JHCT[{PSF},{PSM},1,1,0,1,50,1.1,1]", band=True),
        _ps("jhct-isotropic", "JHCT with isotropic covariances (sigma 1: ITK's univariate prefactor, ITK-23)", f"JHCT[{PSF},{PSM},1,1,0,1,50,1.1,0]"),
        _ps("jhct-alpha-1", "JHCT with alpha 1 (the logarithm)", f"JHCT[{PSF},{PSM},1,1,0,1,50,1,0]"),
        _ps("jhct-alpha-2", "JHCT with alpha 2", f"JHCT[{PSF},{PSM},1,1,0,1,50,2,0]"),
        _ps("jhct-sigma-2", "JHCT with a point-set sigma of 2", f"JHCT[{PSF},{PSM},1,1,0,2,50,1.1,0]"),
        _ps("jhct-anisotropic-sigma-2", "JHCT, anisotropic, sigma 2, alpha 1.5", f"JHCT[{PSF},{PSM},1,1,0,2,50,1.5,1]"),
        _ps("jhct-text", "JHCT between text point sets (fewer points than neighbours: ITK's warnings)", f"JHCT[{txt},{mtxt},1,1,0,1,50,1.1,1]"),
        _ps("jhct-2d", "JHCT in 2D", f"JHCT[{F2D},{M2D},1,1,0,1,20,1.1,1]", mask=K2D, d="2"),
        _ps("igdm", "IGDM between masked intensity images (sigmas estimated)", igdm, band=True),
        _ps("igdm-sigmas", "IGDM with given sigmas", f"IGDM[{PSI},{PSIM},1,{PSK},{PSKM},0x0x0,50,5,1,2]"),
        _ps("igdm-fixed-initial", "IGDM with an initial fixed transform", igdm, extra=("-q", "{gen-transform:initial.mat}")),
        _ps(
            "igdm-neighbourhood",
            "IGDM with a neighbourhood of radius 1",
            f"IGDM[{PSI},{PSIM},1,{PSK},{PSKM},1x1x1]",
            expect="divergence",
            reason="ANTs' line search computes IGDM's centre index from a negative number and reads memory before the intensity arrays (ANTS-66, undefined); larmorx takes the two intensities as equal",
        ),
        _ps(
            "igdm-2d",
            "IGDM in 2D, radius 1",
            "IGDM[{gen:ps-intensity-fixed-2d},{gen:ps-intensity-moving-2d},1,{gen:ps-mask-2d},{gen:ps-mask-moving-2d},1x1]",
            mask=K2D,
            d="2",
            expect="divergence",
            reason="ANTs reads memory before IGDM's intensity arrays for neighbourhoods of more than one voxel (ANTS-66)",
        ),
        _ps("igdm-2d-radius-0", "IGDM in 2D", "IGDM[{gen:ps-intensity-fixed-2d},{gen:ps-intensity-moving-2d},1,{gen:ps-mask-2d},{gen:ps-mask-moving-2d}]", mask=K2D, d="2"),
        _ps("mattes-then-icp", "Mattes and ICP in one stage (the image metric's virtual domain)", head_mattes, icp),
        _ps("icp-then-mattes", "ICP and Mattes in one stage, weights 0.5 and 2", f"ICP[{PSF},{PSM},0.5]", f"Mattes[{PSI},{PSIM},2,32]"),
        _ps("icp-then-mattes-regular", "ICP first: the stage takes ICP's sampling (none)", icp, f"Mattes[{PSI},{PSIM},1,32,Regular,0.5]"),
        _ps("igdm-then-mattes", "IGDM and Mattes in one stage", igdm, head_mattes),
        _ps("icp-then-igdm", "ICP and IGDM in one stage: IGDM gets no input, ITK throws (ANTS-61)", icp, igdm),
        _ps("mattes-then-igdm", "Mattes and IGDM in one stage: IGDM gets no input (ANTS-61)", head_mattes, igdm),
        reg(
            "point-sets/icp-then-pse-stages",
            "point-sets",
            "Two stages: ICP (Rigid), then PSE (Affine)",
            "-t", "Rigid[0.1]", "-m", icp, "-c", "3", "-f", "1", "-s", "0",
            "-t", "Affine[0.1]", "-m", f"PSE[{PSF},{PSM},1,1,0,1,20]", "-c", "3", "-f", "1", "-s", "0",
            "-x", PSK,
            out=PRE,
        ),
        _ps("mattes-and-icp-warped", "Mattes first, then ICP: the warped images of the image metric", head_mattes, icp, out=f"[{PRE},{{out}}/w.nii.gz,{{out}}/iw.nii.gz]"),
        # Failures.
        _ps("igdm-few-parameters", "IGDM with three parameters", f"IGDM[{PSI},{PSIM},1]", expect="error"),
        _ps("igdm-radius-dimension", "IGDM with a 2D radius in 3D", f"IGDM[{PSI},{PSIM},1,{PSK},{PSKM},1x1]", expect="error"),
        _ps("icp-percentage-above-1", "A sampling percentage of 1.5", f"ICP[{PSF},{PSM},1,1.5]", expect="error"),
        _ps("icp-no-mask", "Point-set metrics without a fixed mask: no virtual domain", icp, mask=None, expect=crash),
        _ps("icp-missing", "A point set that does not exist", "ICP[{missing:none.nii.gz},{gen:ps-labels-moving},1]", expect=crash),
        _ps("icp-short-name", "A point set named with fewer than 3 characters", f"ICP[ab,{PSM},1]", expect=crash),
        _ps("icp-unreadable", "A point set no reader takes (ITK's image reader throws)", "ICP[{gen-points:points.csv},{gen-points:moving.txt},1]", expect=crash),
        _ps("icp-vtk-no-points", "A VTK file without POINTS: libstdc++'s out_of_range", "ICP[{gen-points:no-points.vtk},{gen-points:moving.vtk},1]", expect=crash),
        _ps("igdm-missing-mask", "An IGDM mask that does not exist", f"IGDM[{PSI},{PSIM},1,{{missing:none.nii.gz}},{PSKM}]", expect=crash),
        _ps("icp-warped", "Warped images with a point-set metric first", icp, out=f"[{PRE},{{out}}/w.nii.gz]", expect=crash),
        _ps("icp-print-similarity", "--print-similarity-measure-interval with a point-set metric first", icp, extra=("--print-similarity-measure-interval", "1"), expect=crash),
        _ps("icp-interval-volumes", "--write-interval-volumes with a point-set metric first", icp, extra=("--write-interval-volumes", "1"), expect=crash),
        _ps("mattes-regular-then-icp", "Regular sampling with a point-set metric in the stage", f"Mattes[{PSI},{PSIM},1,32,Regular,0.5]", icp, expect=crash),
        _ps("icp-text-garbage", "A text point set with a word among its numbers: ANTs' reader loops forever", "ICP[{gen-points:garbage.txt},{gen-points:moving.txt},1]", expect="hang", tier="full"),
    ]  # fmt: skip


def cases_for(tier: str) -> list[Case]:
    return [Case(c.id, c.category, c.description, c) for c in all_cases() if in_tier(c.tier, tier)]


# ------------------------------------------------------------------------------------------------
# Running


def _oracle_root() -> Path | None:
    for parent in Path(__file__).resolve().parents:
        candidate = parent / ORACLE_RUNS
        if candidate.is_dir():
            return candidate
    return None


#: nipype's per-stage options (``Registration._format_registration``).
_STAGE_OPTIONS = {
    "--transform",
    "--metric",
    "--convergence",
    "--smoothing-sigmas",
    "--shrink-factors",
    "--use-histogram-matching",
    "--restrict-deformation",
    "--masks",
}
_LINEAR = ("Translation[", "Rigid[", "Similarity[", "Affine[", "CompositeAffine[")


def _linear_stages(args: list[str]) -> list[str]:
    """``args`` without its deformable stages: each ``--transform`` that is not linear and the
    stage options after it, up to the next stage or the global options."""
    kept: list[str] = []
    skipping = False
    i = 0
    while i < len(args):
        word = args[i]
        if word == "--transform":
            skipping = not args[i + 1].startswith(_LINEAR)
        elif word.startswith("-") and word not in _STAGE_OPTIONS:
            skipping = False
        if not skipping:
            kept.append(word)
        i += 1
    return kept


def _masked(tmp: Path, name: str, image: Path, mask: Path) -> Path:
    """``image`` times ``mask`` (float32), as niworkflows' ``RobustMNINormalization`` masks its
    inputs with ``explicit_masking``."""
    import nibabel as nib

    path = tmp / name
    if not path.exists():
        img = nib.load(image)
        data = np.asarray(img.dataobj, dtype=np.float32) * (np.asarray(nib.load(mask).dataobj) > 0)
        nib.save(nib.Nifti1Image(data, img.affine, img.header), path)
    return path


def _laplacian(tmp: Path, image: Path) -> Path:
    """The template's Laplacian as sMRIPrep's brain extraction makes it (``lap_tmpl``:
    ``ImageMath 3 out Laplacian template 1.5 1``), by the ANTs oracle's ImageMath."""
    path = tmp / image.name.replace(".nii.gz", "_maths.nii.gz")
    if not path.exists():
        b = ants_bin()
        if b is None or not (b / "ImageMath").is_file():
            raise Outcome(SKIPPED, "the ANTs oracle's ImageMath was not found")
        from larmorx_validation import oracle_cache

        argv = [str(b / "ImageMath"), "3", str(path), "Laplacian", str(image), "1.5", "1"]
        p = oracle_cache.run(
            argv,
            env={**os.environ, "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1"},
            policy=oracle_cache.Policy(threads=1, family="itk"),
            runner="ants-registration-inputs",
        )
        if p.returncode != 0:
            raise subprocess.CalledProcessError(p.returncode, argv, p.stdout, p.stderr)
    return path


def _decimated(tmp: Path, image: Path, k: int) -> Path:
    """Every ``k``-th voxel of a 3D image along each axis, in the image's data type, its affine
    scaled so that the voxels kept stay where they were: the small versions of real images."""
    import hashlib

    import nibabel as nib

    d = tmp / "small"
    d.mkdir(exist_ok=True)
    path = d / f"{hashlib.sha1(str(image).encode()).hexdigest()[:10]}_{image.name}"
    if not path.exists():
        img = nib.load(image)
        dtype = img.get_data_dtype()
        data = np.asanyarray(img.dataobj)[::k, ::k, ::k]
        if np.issubdtype(dtype, np.integer) and not np.issubdtype(data.dtype, np.integer):
            dtype = np.dtype(np.float32)
        a = img.affine.copy()
        a[:3, :3] *= k
        nib.save(nib.Nifti1Image(np.asarray(data, dtype=dtype), a), path)
    return path


def _small(args: list[str], tmp: Path, k: int) -> list[str]:
    """``args`` with every image they name replaced by its every ``k``-th voxel."""

    def replace(m: re.Match[str]) -> str:
        p = m.group(0)
        if p.endswith((".nii", ".nii.gz")) and Path(p).is_file():
            return str(_decimated(tmp, Path(p), k))
        return p

    return [re.sub(r"[^\[\],]+", replace, a) for a in args]


def _stand_ins(args: list[str], run_dir: Path, tmp: Path) -> list[str]:
    """``args`` with the intermediates nipype did not keep in the oracle's working directory
    replaced: the brain extraction's N4-corrected T1w by the run's ``desc-preproc_T1w`` (the
    same image after the final N4), the template registration's masked images by the template
    and the T1w times their brain masks, and its initial transform (which nipype did not keep
    either) by ``[fixed,moving,1]``."""
    import larmorx_testdata as td

    anat = sorted(
        p
        for p in (run_dir / "out").glob("sub-*/**/anat/*desc-preproc_T1w.nii.gz")
        if "space-" not in p.name
    )
    if not anat:
        raise Outcome(SKIPPED, f"{run_dir.name} has no desc-preproc_T1w")
    t1w = anat[0]
    brain = t1w.with_name(t1w.name.replace("desc-preproc_T1w", "desc-brain_mask"))
    tpl = "templateflow/tpl-MNI152NLin2009cAsym/tpl-MNI152NLin2009cAsym_res-01_"
    fixed = _masked(
        tmp,
        "fixed_masked.nii.gz",
        td.get(tpl + "T1w.nii.gz"),
        td.get(tpl + "desc-brain_mask.nii.gz"),
    )
    moving = _masked(tmp, f"{run_dir.name}_moving_masked.nii.gz", t1w, brain)

    def replace(path: str) -> str:
        if Path(path).exists() or not path.startswith(str(run_dir)):
            return path
        name = Path(path).name
        if name == "fixed_masked.nii.gz":
            return str(fixed)
        if name == "moving_masked.nii.gz":
            return str(moving)
        if name.endswith("_corrected.nii.gz"):
            return str(t1w)
        if Path(path).parent.name == "lap_tmpl":
            template = name.replace("_maths", "")
            return str(_laplacian(tmp, td.get(f"templateflow/{template.split('_')[0]}/{template}")))
        return path

    out = []
    i = 0
    while i < len(args):
        word = args[i]
        if word == "--initial-moving-transform" and "transform.mat" in args[i + 1]:
            path = args[i + 1][1:-1].split(",")[0]
            if not Path(path).exists():
                out += [word, f"[{fixed},{moving},1]"]
                i += 2
                continue
        out.append(re.sub(r"[^\[\],]+", lambda m: replace(m.group(0)), word))
        i += 1
    return out


def _fmriprep_args(
    spec: str,
    out: Path,
    linear_only: bool = False,
    tmp: Path | None = None,
    *,
    stand_ins: bool = False,
    small: int = 0,
) -> list[str]:
    """The ``antsRegistration`` arguments of an fMRIPrep node (``run/node``), its paths mapped to
    the oracle run and ``larmorx-testdata``, the output prefix moved to ``out``; with
    ``linear_only``, without its deformable stages; with ``linear_only`` or ``stand_ins``,
    stand-ins (made in ``tmp``) for the inputs nipype did not keep; with ``small``, every image
    replaced by its every ``small``-th voxel."""
    import larmorx_testdata as td

    root = _oracle_root()
    run, node = spec.split("/", 1)
    if root is None or not (root / run / "work").is_dir():
        raise Outcome(SKIPPED, f"the fMRIPrep oracle run {run} is not here")
    work = root / run / "work" / "fmriprep_25_2_wf" / node
    words = (work / "command.txt").read_text(encoding="utf-8").split()[1:]
    text = " ".join(words).replace("[ ", "[").replace(" ]", "]").replace(", ", ",")
    text = text.replace("/work/", f"{root / run / 'work'}/").replace(
        "/out/", f"{root / run / 'out'}/"
    )

    def template(m: re.Match[str]) -> str:
        return str(td.get(m.group(0)[1:]))

    text = re.sub(r"/templateflow/[^\s,\]]+", template, text)
    args = text.split()
    if linear_only:
        args = _linear_stages(args)
    if linear_only or stand_ins:
        args = _stand_ins(args, root / run, tmp or out)
    if small:
        args = _small(args, tmp or out, small)
    i = args.index("--output")
    value = args[i + 1]
    if value.startswith("["):
        parts = value[1:-1].split(",")
        args[i + 1] = (
            "["
            + ",".join([f"{out}/transform"] + [f"{out}/{Path(p).name}" for p in parts[1:]])
            + "]"
        )
    else:
        args[i + 1] = f"{out}/transform"
    return [*args, "-v", "1"]


#: The 2D and 4D transform files the cases start from: class, parameters, fixed parameters.
_TRANSFORMS_ND: dict[str, tuple[str, list[float], list[float]]] = {
    # Half a voxel of the point-set phantoms along x: every point's nearest neighbour 1 mm away.
    "half-voxel-x.txt": ("TranslationTransform_double_3_3", [1.0, 0.0, 0.0], []),
    "affine-2d.mat": ("AffineTransform_double_2_2", [1.01, 0.02, -0.03, 0.99, 1.5, -0.5], [2, -1]),
    "euler-2d.txt": ("Euler2DTransform_double_2_2", [0.05, 1.0, -2.0], [1.0, 0.5]),
    "rigid-2d.mat": ("Rigid2DTransform_double_2_2", [-0.04, 0.5, 1.0], [0.0, 1.0]),
    "similarity-2d.mat": ("Similarity2DTransform_double_2_2", [1.03, 0.06, -1.0, 0.5], [1.5, -0.5]),
    "translation-2d.txt": ("TranslationTransform_double_2_2", [0.75, -1.25], []),
    "centered-affine-2d.mat": (
        "ANTSCenteredAffine2DTransform_double_2_2",
        [0.05, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0],
        [],
    ),
    "affine-4d.mat": (
        "AffineTransform_double_4_4",
        # The matrix row by row, then the translation.
        [
            1.01,
            0.01,
            0.0,
            0.0,
            -0.02,
            0.99,
            0.01,
            0.0,
            0.0,
            0.01,
            1.0,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            1.0,
            -1.0,
            0.5,
            0.0,
        ],
        [1.0, 2.0, 0.0, 0.0],
    ),
}


def _transform_file(tmp: Path, name: str) -> Path:
    """A transform file the cases start from: ``initial.mat``, a rigid motion with a centre as
    an ``AffineTransform`` in MATLAB format; or one of :data:`_TRANSFORMS_ND`."""
    import larmorx as lx

    path = tmp / name
    if not path.exists():
        if name in _TRANSFORMS_ND:
            cls, params, fixed = _TRANSFORMS_ND[name]
            lx.transforms.write(
                path, lx.transforms.ItkTransform(cls, np.array(params), np.array(fixed, float))
            )
            return path
        th = np.deg2rad(4.0)
        matrix = np.array([[np.cos(th), -np.sin(th), 0], [np.sin(th), np.cos(th), 0], [0, 0, 1]])
        params = np.concatenate([matrix.ravel(), [2.0, -1.5, 1.0]])
        lx.transforms.write(
            path, lx.transforms.ItkTransform("AffineTransform_double_3_3", params, [1.0, -2.0, 0.5])
        )
    return path


def _dimension_cases() -> list[RegCase]:
    """2D and 4D: the warped images, interval volumes, composites and transform files."""
    D2, D4 = "dimensions-2d", "dimensions-4d"
    short = {"c": "[6x4,1e-6,5]"}
    one = {"c": "[4,1e-6,5]", "f": "1", "s": "0"}

    def case2(name: str, description: str, *args: str, t: str = "Rigid[0.1]", st=short, **kw):
        return reg(
            f"{D2}/{name}", D2, description, *args, *stage(t, mattes(F2, M2), **st), d="2", **kw
        )

    def case4(name: str, description: str, *args: str, t: str = "Affine[0.1]", st=short, **kw):
        return reg(
            f"{D4}/{name}", D4, description, *args, *stage(t, mattes(F4, M4), **st), d="4", **kw
        )

    def tf(name: str, inverse: bool = False) -> str:
        return f"[{{gen-transform:{name}}},1]" if inverse else f"{{gen-transform:{name}}}"

    return [
        case2("warped", "the warped and inverse-warped images (linear)"),
        case2("warped-nearest", "-n NearestNeighbor", "-n", "NearestNeighbor", t="Affine[0.1]"),
        case2("warped-float", "Similarity, --float 1", "--float", "1", t="Similarity[0.1]"),
        case2("fixed-initial", "-q [fixed,moving,1]", "-q", f"[{F2},{M2},1]", t="Affine[0.1]"),
        case2(
            "file-affine", "-r an AffineTransform file", "-r", tf("affine-2d.mat"), t="Affine[0.1]"
        ),
        case2(
            "file-affine-inverted", "-r [AffineTransform file,1]", "-r", tf("affine-2d.mat", True)
        ),
        case2(
            "file-euler",
            "-r a text Euler2DTransform file, -z 0",
            "-z",
            "0",
            "-r",
            tf("euler-2d.txt"),
            t="Affine[0.1]",
        ),
        case2(
            "file-euler-inverted",
            "-r [Euler2DTransform,1]: its own inverse class",
            "-r",
            tf("euler-2d.txt", True),
            t="Affine[0.1]",
        ),
        case2(
            "file-rigid2d",
            "-r a Rigid2DTransform file, -z 0 -a 1",
            "-z",
            "0",
            "-a",
            "1",
            "-r",
            tf("rigid-2d.mat"),
            t="Affine[0.1]",
            out=PRE,
        ),
        case2(
            "file-similarity-inverted",
            "-r [Similarity2DTransform,1], --float 1",
            "--float",
            "1",
            "-r",
            tf("similarity-2d.mat", True),
            t="Affine[0.1]",
        ),
        case2(
            "file-translation",
            "-r a TranslationTransform file",
            "-r",
            tf("translation-2d.txt"),
            t="Rigid[0.1]",
        ),
        case2(
            "file-unregistered",
            "-r an ANTSCenteredAffine2DTransform file: ITK's factory does not know the class",
            "-r",
            tf("centered-affine-2d.mat"),
            t="Affine[0.1]",
            expect="error",
        ),
        case2(
            "file-3d",
            "-r a 3D transform in a 2D registration",
            "-r",
            tf("initial.mat"),
            t="Affine[0.1]",
            expect="error",
        ),
        case2(
            "composite",
            "-a 1 with -r [fixed,moving,1]",
            "-a",
            "1",
            "-r",
            f"[{F2},{M2},1]",
            t="Affine[0.1]",
        ),
        case2(
            "interval-volumes",
            "--write-interval-volumes 2",
            "--write-interval-volumes",
            "2",
            st=one,
            out=PRE,
        ),
        *[
            case2(f"interpolation-{label}", f"-n {interp} in 2D", "-n", interp, *extra, st=one)
            for label, interp, extra in (
                ("BSpline", "BSpline[3]", ()),
                ("BSpline5-float", "BSpline[5]", ("--float", "1")),
                ("Gaussian", "Gaussian", ()),
                ("Gaussian-sigmas", "Gaussian[1.5x0.8,1.2]", ()),
                ("LanczosWindowedSinc", "LanczosWindowedSinc", ()),
                ("BlackmanWindowedSinc-float", "BlackmanWindowedSinc", ("--float", "1")),
                ("WelchWindowedSinc", "WelchWindowedSinc", ()),
                ("GenericLabel", "GenericLabel", ()),
                ("MultiLabel", "MultiLabel[1.2]", ()),
            )
        ],
        case4("warped", "the warped and inverse-warped images (linear)"),
        case4(
            "warped-nearest-float",
            "-n NearestNeighbor, --float 1",
            "-n",
            "NearestNeighbor",
            "--float",
            "1",
            t="Translation[0.1]",
        ),
        case4(
            "file-affine-inverted", "-r [AffineTransform file,1]", "-r", tf("affine-4d.mat", True)
        ),
        *[
            case4(f"interpolation-{label}", f"-n {interp} in 4D", "-n", interp, *extra, st=one)
            for label, interp, extra in (
                ("BSpline", "BSpline[2]", ()),
                ("Gaussian-float", "Gaussian[1x1x1x1,1.5]", ("--float", "1")),
                ("HammingWindowedSinc", "HammingWindowedSinc", ()),
                ("GenericLabel", "GenericLabel", ()),
                ("MultiLabel-float", "MultiLabel", ("--float", "1")),
            )
        ],
        case4(
            "composite-z0",
            "-a 1 -z 0 -r [fixed,moving,0]",
            "-a",
            "1",
            "-z",
            "0",
            "-r",
            f"[{F4},{M4},0]",
            t="Rigid[0.1]",
        ),
        case4(
            "interval-volumes",
            "--write-interval-volumes 1",
            "--write-interval-volumes",
            "1",
            st={"c": "[3,1e-6,5]", "f": "1", "s": "0"},
            out=PRE,
        ),
    ]


def _resolve(c: RegCase, out: Path, tmp: Path) -> list[str]:
    if len(c.args) == 1 and c.args[0].startswith("{panel-node:"):
        return _panel_args(c.args[0][12:-1], out, tmp)
    if len(c.args) == 1 and c.args[0].startswith("{panel-node-small:"):
        return _panel_args(c.args[0][18:-1], out, tmp, small=SMALL)
    if len(c.args) == 1 and c.args[0].startswith("{fmriprep:"):
        return _fmriprep_args(c.args[0][10:-1], out)
    if len(c.args) == 1 and c.args[0].startswith("{fmriprep-linear:"):
        return _fmriprep_args(c.args[0][17:-1], out, linear_only=True, tmp=tmp)
    if len(c.args) == 1 and c.args[0].startswith("{fmriprep-whole:"):
        return _fmriprep_args(c.args[0][16:-1], out, tmp=tmp, stand_ins=True)
    if len(c.args) == 1 and c.args[0].startswith("{fmriprep-small:"):
        return _fmriprep_args(c.args[0][16:-1], out, tmp=tmp, stand_ins=True, small=SMALL)
    if len(c.args) == 1 and c.args[0].startswith("{fmriprep-whole-bsplinesyn:"):
        args = _fmriprep_args(c.args[0][27:-1], out, tmp=tmp, stand_ins=True)
        return _bsplinesyn(args)
    if len(c.args) == 1 and c.args[0].startswith("{fmriprep-small-bsplinesyn:"):
        args = _fmriprep_args(c.args[0][27:-1], out, tmp=tmp, stand_ins=True, small=SMALL)
        return _bsplinesyn(args)
    for kind, stage_text in VELOCITY_STAGES.items():
        for size, small in (("small", SMALL), ("whole", 0)):
            head = f"{{fmriprep-{size}-{kind}:"
            if len(c.args) == 1 and c.args[0].startswith(head):
                args = _fmriprep_args(
                    c.args[0][len(head) : -1], out, tmp=tmp, stand_ins=True, small=small
                )
                return _replace_syn(args, stage_text)

    def sub(m: re.Match[str]) -> str:
        key = m.group(1)
        if key == "out":
            return str(out)
        if key == "tmp":
            return str(tmp)
        if key.startswith("gen:"):
            return str(ants_inputs.path(key[4:]))
        if key.startswith("gen-transform:"):
            return str(_transform_file(tmp, key[14:]))
        if key.startswith("in:"):
            import larmorx_testdata as td

            return str(td.get(key[3:]))
        if key.startswith("missing:"):
            return str(tmp / key[8:])
        if key.startswith("gen-points:"):
            return str(ants_point_sets.write_points(key[11:], tmp))
        if key.startswith("gen-syn:"):
            return str(syn_transform_file(tmp, key[8:]))
        if key.startswith("gen-bsp:"):
            return str(bspline_transform_file(tmp, key[8:]))
        raise KeyError(f"unknown placeholder {{{key}}}")

    return [re.sub(r"\{([^{}]+)\}", sub, a) for a in c.args]


#: The velocity-field stages the real cases put in place of the template registration's SyN
#: stage: antsRegistrationSyN.sh has none, so they take its SyN stage's smoothing (variance 3)
#: and BSplineSyN's knot spacing (26 mm); the time-varying B-spline field a mesh of 4 x 4 x 4
#: over 2 time spans.
VELOCITY_STAGES = {
    "exponential": "Exponential[0.25,3,0.5]",
    "bspline-exponential": "BSplineExponential[0.25,26,0,0,3]",
    "time-varying": "TimeVaryingVelocityField[0.5,4,3,0,0,0]",
    "time-varying-bspline": "TimeVaryingBSplineVelocityField[0.5,4x4x4x2,4,3]",
}


def _replace_syn(args: list[str], stage_text: str) -> list[str]:
    """The arguments with their SyN stage's transform replaced by ``stage_text``."""
    out = list(args)
    for i, a in enumerate(out[:-1]):
        if a in ("--transform", "-t") and out[i + 1].startswith("SyN["):
            out[i + 1] = stage_text
    return out


def _bsplinesyn(args: list[str]) -> list[str]:
    """The arguments with their SyN stage replaced by antsRegistrationSyN.sh's
    ``BSplineSyN[0.1,26,0,3]`` (the script's ``-t b``)."""
    out = list(args)
    for i, a in enumerate(out[:-1]):
        if a in ("--transform", "-t") and out[i + 1].startswith("SyN["):
            out[i + 1] = "BSplineSyN[0.1,26,0,3]"
    return out


def _run_ants(
    args: list[str],
    seed: str | None,
    threads: int = 1,
    *,
    correctly_rounded: bool = False,
    timeout: float = ANTS_TIMEOUT,
    never: str = "",
):
    """antsRegistration, through the oracle cache: never kept without a seed (the clock), above
    one thread (``VARIES_ABOVE_ONE_THREAD``), or when ``never`` says why (:func:`case_never`)."""
    from larmorx_validation import oracle_cache

    b = ants_bin()
    if b is None or not (b / PROGRAM).is_file():
        raise Outcome(SKIPPED, "the ANTs oracle's antsRegistration was not found")
    env = {k: v for k, v in os.environ.items() if k not in ("ANTS_RANDOM_SEED", "LD_PRELOAD")}
    env["ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS"] = str(threads)
    if seed is not None:
        env["ANTS_RANDOM_SEED"] = seed
    if correctly_rounded:
        lib = crmath_preload()
        if lib is None:
            raise FileNotFoundError("libcrmath.so (scripts/build_crmath_preload.sh)")
        env["LD_PRELOAD"] = str(lib)
    policy = oracle_cache.Policy(
        threads=oracle_cache.itk_threads(env),
        never=never or oracle_cache.ants_seed_never(PROGRAM, list(args), env),
        family="itk",
    )
    p = oracle_cache.run(
        [str(b / PROGRAM), *args],
        env=env,
        policy=policy,
        timeout=timeout,
        runner="ants-registration",
    )
    if p.timed_out:
        return None, "", ""
    return p.returncode, p.stdout, p.stderr


def case_never(c) -> str:
    """Why ANTs' runs of a registration (or antsAI) case are never kept in the oracle cache:
    a hang (its outcome is the clock's), a case that measures ANTs' own spread, or the case's
    texts naming a defect whose result varies (``VARYING_DEFECTS``)."""
    from larmorx_validation.oracle_cache import never_for

    if c.expect == "hang":
        return "ANTs never finishes: the outcome is the timeout's"
    if c.expect == "random":
        return "ANTs seeds from the clock: the case measures its own spread"
    if why := getattr(c, "nondeterministic", ""):
        return why
    return never_for(c.description, c.reason)


def _run_larmorx(args: list[str], seed: str | None, mode: str = defects.REPRODUCE):
    from larmorx import _core, _replica

    saved = {
        k: os.environ.get(k) for k in ("ANTS_RANDOM_SEED", "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS")
    }
    os.environ["ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS"] = str(LX_THREADS)
    if seed is None:
        os.environ.pop("ANTS_RANDOM_SEED", None)
    else:
        os.environ["ANTS_RANDOM_SEED"] = seed
    try:
        with defects.mode(mode):
            code, out, err = _core.cli_main(
                ["larmorx", "ants", PROGRAM, *args], _replica.search_dirs(), None
            )
    finally:
        for k, v in saved.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v
    return code, out, err


_NOTICE = re.compile(r"^larmorx: upstream defect ")


def _normalise(text: str, out: Path) -> list[str]:
    lines = []
    for line in text.replace(str(out), "{out}").splitlines():
        line = re.sub(r"\(0x[0-9a-f]+\)", "(0x)", line)
        line = re.sub(r"(Pointer|ImportImageContainer) (0x)?[0-9a-f]+", r"\1 0x", line)
        line = re.sub(r"(Pointer|pointer): 0x[0-9a-f]+", r"\1: 0x", line)
        line = re.sub(
            r"(Modified Time|UpdateMTime|PipelineMTime|Reference Count): \d+", r"\1: N", line
        )
        line = re.sub(
            r"File: (?:<ITK source>|\S*?)/(Modules|Utilities|Examples|ImageRegistration|Temporary)/",
            r"File: \1/",
            line,
        )
        line = re.sub(
            r"WARNING: In \S*?/(Modules|Utilities|Examples|ImageRegistration|Temporary)/",
            r"WARNING: In \1/",
            line,
        )
        # An exception's what() inside another's description: the thrower's file and line.
        line = re.sub(r"^\S*?/(Modules|Utilities|Examples)/(\S+:\d+:)$", r"\1/\2", line)
        # vnl's warnings name their header by the build's path (`__FILE__`; vnl_svd.hxx).
        line = re.sub(r"^\S*?/(Modules/ThirdParty/VNL/\S+?\.(?:hxx|h|cxx)): ", r"\1: ", line)
        line = re.sub(r"(Elapsed time \(stage \d+\)|Total elapsed time): .*", r"\1: T", line)
        # The sign of a NaN result: IEEE 754 leaves it unspecified, the compiler's operand order
        # decides it (docs/findings/ants-registration.md).
        line = re.sub(r"-nan\b", "nan", line)
        if "DIAGNOSTIC," in line and not line.startswith("DIAGNOSTIC"):
            fields = line.split(", ")
            line = ", ".join(fields[:4] + fields[6:])
        if _NOTICE.match(line):
            continue
        lines.append(line)
    return lines


def _first_difference(a: list[str], b: list[str]) -> str:
    for i, (x, y) in enumerate(zip(a, b, strict=False)):
        if x != y:
            return f"line {i + 1}: ANTs {x[:120]!r}, larmorx {y[:120]!r}"
    if len(a) != len(b):
        return f"ANTs {len(a)} lines, larmorx {len(b)}"
    return ""


def _h5(path: Path) -> list:
    from larmorx import _core

    return [
        (n, k, list(s), np.asarray(v).tobytes() if k.startswith("float") else v)
        for n, k, s, v in _core.hdf5_datasets(str(path))
        if n not in ("ITKVersion", "HDFVersion", "OSName", "OSVersion")
    ]


def _same_file(a: Path, b: Path) -> tuple[bool, str]:
    """Whether two outputs are identical, and a description of the difference."""
    if a.suffix == ".h5":
        same = _h5(a) == _h5(b)
        return same, "" if same else "datasets differ"
    if a.name.endswith((".nii", ".nii.gz")):
        da, db = a.read_bytes(), b.read_bytes()
        if a.name.endswith(".gz"):
            da, db = gzip.decompress(da), gzip.decompress(db)
        if da == db:
            return True, ""
        import nibabel as nib

        va, vb = np.asanyarray(nib.load(a).dataobj), np.asanyarray(nib.load(b).dataobj)
        header = "header identical" if da[:352] == db[:352] else "headers differ"
        if va.shape != vb.shape:
            return False, f"{header}; shapes {va.shape} and {vb.shape}"
        n = int(np.count_nonzero(va != vb))
        diff = float(np.max(np.abs(va.astype(float) - vb.astype(float)))) if n else 0.0
        return False, f"{header}; {n} of {va.size} values differ (max {diff:.3g})"
    same = a.read_bytes() == b.read_bytes()
    return same, "" if same else "bytes differ"


def _outputs(d: Path) -> dict[str, Path]:
    return {p.name: p for p in sorted(d.iterdir()) if p.is_file()}


def _h5_parameters(path: Path) -> list[np.ndarray]:
    """The ``TransformParameters`` datasets of an ITK HDF5 transform file, in group order."""
    import h5py

    out = []
    with h5py.File(path, "r") as f:
        group = f.get("TransformGroup")
        if group is None:
            return out
        for name in sorted(group, key=lambda k: int(k) if k.isdigit() else -1):
            if "TransformParameters" in group[name]:
                out.append(np.asarray(group[name]["TransformParameters"], dtype=np.float64))
    return out


def _parameters(d: Path) -> np.ndarray | None:
    """Every transform parameter written, in file order (for the thread band): the parameters
    of transform files, and the vectors of displacement-field images."""
    import larmorx as lx

    vals = []
    for p in _outputs(d).values():
        if p.suffix in (".mat", ".txt", ".h5"):
            try:
                transforms = lx.transforms.read(p)
            except ValueError:
                if p.suffix != ".h5":
                    raise
                # A transform larmorx's reader does not hold (a 4D displacement field,
                # `DisplacementFieldTransform_double_4_4`; random testing, 2026-10-25): its
                # parameters as the file stores them.
                vals.extend(_h5_parameters(p))
                continue
            for t in transforms:
                vals.append(np.asarray(t.parameters, dtype=np.float64))
        elif p.name.endswith("Warp.nii.gz"):
            import nibabel as nib

            vals.append(np.asarray(nib.load(p).dataobj, dtype=np.float64).ravel())
    return np.concatenate(vals) if vals else None


def _compare(
    checks: CheckList, label: str, ants: tuple, lx: tuple, a_dir: Path, l_dir: Path
) -> bool:
    """Checks of one comparison (``label``) added to ``checks``; whether everything is
    identical."""
    (a_code, a_out, a_err), (l_code, l_out, l_err) = ants, lx
    ok = True
    same = a_code == l_code
    checks(f"{label}: exit status", same, detail=f"ANTs {a_code}, larmorx {l_code}")
    ok &= same
    na, nl = _normalise(a_out, a_dir), _normalise(l_out, l_dir)
    same = na == nl
    checks(
        f"{label}: standard output",
        same,
        metric="lines",
        value=len(na),
        threshold="identical, masked",
        detail="identical" if same else _first_difference(na, nl),
    )
    ok &= same
    ea, el = _normalise(a_err, a_dir), _normalise(l_err, l_dir)
    same = ea == el
    checks(
        f"{label}: standard error", same, detail="identical" if same else _first_difference(ea, el)
    )
    ok &= same
    fa, fl = _outputs(a_dir), _outputs(l_dir)
    same = sorted(fa) == sorted(fl)
    checks(
        f"{label}: files written",
        same,
        detail=", ".join(sorted(fa)) if same else f"ANTs {sorted(fa)}, larmorx {sorted(fl)}",
    )
    ok &= same
    for name in sorted(set(fa) & set(fl)):
        s, why = _same_file(fa[name], fl[name])
        checks(f"{label}: {name}", s, detail="identical" if s else why)
        ok &= s
    return ok


def run_case(case: Case, checks: CheckList) -> None:
    c: RegCase = case.payload
    import tempfile

    with tempfile.TemporaryDirectory(prefix="lx-reg-") as t:
        tmp = Path(t)
        a_dir, l_dir = tmp / "ants", tmp / "larmorx"
        a_dir.mkdir()
        l_dir.mkdir()
        a_args, l_args = _resolve(c, a_dir, tmp), _resolve(c, l_dir, tmp)
        ants = _run_ants(
            a_args,
            c.seed,
            timeout=HANG_SECONDS if c.expect == "hang" else ANTS_TIMEOUT,
            never=case_never(c),
        )
        lx = _run_larmorx(l_args, c.seed)
        if c.expect == "hang":
            checks(
                "ANTs does not finish",
                ants[0] is None,
                detail=f"ANTs {'still running' if ants[0] is None else f'exit {ants[0]}'} after {HANG_SECONDS} s",
            )
            last = (lx[2].strip().splitlines() or [""])[-1]
            checks(
                "larmorx refuses (exit 1)",
                lx[0] == 1 and last.startswith("larmorx: "),
                detail=f"exit {lx[0]}: {last[:160]}",
            )
            raise Outcome(EXPECTED, "ANTs never finishes (class (a)); larmorx refuses")
        if c.expect == "crash":
            crashed = ants[0] is not None and ants[0] < 0
            checks(
                "ANTs crashes or aborts, larmorx exits 1",
                crashed and lx[0] == 1,
                detail=f"ANTs {ants[0]} (a signal), larmorx {lx[0]}",
            )
            oa, ol = _normalise(ants[1], a_dir), _normalise(lx[1], l_dir)
            checks(
                "standard output up to the crash",
                oa == ol,
                detail="identical" if oa == ol else _first_difference(oa, ol),
            )
            ea = _normalise(ants[2], a_dir)
            el = [x for x in _normalise(lx[2], l_dir) if not x.startswith("larmorx: ")]
            checks(
                "standard error up to the crash",
                ea == el,
                detail="identical" if ea == el else _first_difference(ea, el),
            )
            fa, fl = _outputs(a_dir), _outputs(l_dir)
            checks(
                "files written before the crash",
                sorted(fa) == sorted(fl),
                detail=f"ANTs {sorted(fa)}, larmorx {sorted(fl)}",
            )
            for name in sorted(set(fa) & set(fl)):
                same, why = _same_file(fa[name], fl[name])
                checks(f"{name} (before the crash)", same, detail="identical" if same else why)
            last = (lx[2].strip().splitlines() or [""])[-1]
            raise Outcome(BOTH_ERROR, last[:200])
        if c.expect == "divergence":
            checks(
                "larmorx: exit status and message",
                True,
                detail=f"ANTs exit {ants[0]}; larmorx exit {lx[0]}: {lx[2].strip().splitlines()[-1:]}",
            )
            raise Outcome(EXPECTED, c.reason)
        if c.expect == "error" or (ants[0] != 0 and lx[0] != 0 and ants[0] == lx[0]):
            same = ants[0] == lx[0] and ants[0] != 0
            checks(
                "both fail with the same status", same, detail=f"ANTs {ants[0]}, larmorx {lx[0]}"
            )
            ea, el = _normalise(ants[2] + ants[1], a_dir), _normalise(lx[2] + lx[1], l_dir)
            checks(
                "their messages",
                ea == el,
                detail="identical" if ea == el else _first_difference(ea, el),
            )
            raise Outcome(BOTH_ERROR, (ants[2].strip().splitlines() or [str(ants[0])])[-1][:200])
        plain = CheckList()
        identical = _compare(plain, "unmodified ANTs", ants, lx, a_dir, l_dir)
        if identical:
            checks.checks.extend(plain.checks)
        else:
            # ANTs with CORE-MATH's functions decides; the first comparison is reported.
            from dataclasses import replace

            checks.checks.extend(
                ch if ch.passed else replace(ch, passed=True, threshold=GLIBC)
                for ch in plain.checks
            )
            c_dir = tmp / "ants-cr"
            c_dir.mkdir()
            cr = _run_ants(
                _resolve(c, c_dir, tmp), c.seed, correctly_rounded=True, never=case_never(c)
            )
            _compare(checks, "ANTs with CORE-MATH", cr, lx, c_dir, l_dir)
        if c.band and ants[0] == 0:
            ref = _parameters(a_dir)
            band = 0.0
            for n in BAND_THREADS:
                b_dir = tmp / f"ants-{n}"
                b_dir.mkdir()
                _run_ants(_resolve(c, b_dir, tmp), c.seed, threads=n, never=case_never(c))
                other = _parameters(b_dir)
                if ref is not None and other is not None and other.shape == ref.shape:
                    band = max(band, float(np.max(np.abs(other - ref))))
            mine = _parameters(l_dir)
            dist = (
                float(np.max(np.abs(mine - ref))) if ref is not None and mine is not None else 0.0
            )
            checks(
                "thread band",
                True,
                metric="max |parameter difference|",
                value=f"{dist:.3g}",
                threshold=f"reported; ANTs at {', '.join(map(str, BAND_THREADS))} threads vs 1: {band:.3g}",
                detail=f"larmorx vs ANTs (1 thread) {dist:.3g}; ANTs' own band {band:.3g}",
            )
        # D13: larmorx's default on the same arguments.
        f_dir = tmp / "larmorx-fix"
        f_dir.mkdir()
        fx = _run_larmorx(_resolve(c, f_dir, tmp), c.seed, mode=defects.FIX)
        change = defects.Change(0, 0, 0.0)
        other = []
        if fx[0] != lx[0]:
            other.append(f"exit {lx[0]} -> {fx[0]}")
        fl, ff = _outputs(l_dir), _outputs(f_dir)
        if sorted(fl) != sorted(ff):
            other.append(f"files {sorted(set(fl) ^ set(ff))}")
        pa, pb = _parameters(l_dir), _parameters(f_dir)
        if pa is not None and pb is not None and pa.shape == pb.shape:
            change = defects.Change.of(pb, pa)
        # ANTS-62's extra record repeats the last point: in a moving point set it may change
        # no nearest neighbour. ITK-41's notice is about the next use of a transform that never
        # integrated, which a run without warped images or later stages does not make. ITK-96
        # is named in both modes, which write the same files (no inverse composite where a
        # displacement field has no inverse; reproduce does not make ITK's part-written file).
        defects.add_fix_check(
            checks,
            defects.notices_in(fx[2]),
            change,
            other_changes="; ".join(other),
            silent_ok=("ANTS-62", "ITK-41", "ITK-96", *c.silent_ok),
        )


def highlights(results: list[CaseResult]) -> list[str]:
    compared = [r for r in results if r.status == "pass"]
    plain = [r for r in compared if all(ch.threshold != GLIBC for ch in r.checks)]
    lines = [
        f"{len(compared)} cases compared: {len(plain)} identical to the unmodified ANTs (exit status, "
        f"standard output with every DIAGNOSTIC line, standard error, every file), "
        f"{len(compared) - len(plain)} identical once ANTs uses CORE-MATH's correctly rounded "
        "`exp`, `log`, `sin` and `cos` (glibc's are not always correctly rounded)."
    ]
    band = []
    for r in compared:
        for ch in r.checks:
            m = re.search(r"larmorx vs ANTs \(1 thread\) (\S+); ANTs' own band (\S+)", ch.detail)
            if ch.name == "thread band" and m:
                band.append((r.case, float(m.group(1)), float(m.group(2))))
    if band:
        threads = ", ".join(map(str, BAND_THREADS))
        lines.append("")
        lines.append(
            f"Thread band, {len(band)} cases: ANTs at {threads} threads differs from its own "
            f"one-thread result by up to {max(b for _, _, b in band):.3g} in a transform "
            "parameter ("
            + ", ".join(f"`{case}` {b:.3g}" for case, _, b in band)
            + "); larmorx gives the one-thread result at any thread count, within "
            f"{max(d for _, d, _ in band):.3g} of ANTs at one thread."
        )
    lines.append("")
    lines += defects.summary(results)
    from larmorx_validation.parity import fmriprep_nodes

    small = {
        r.case[len(fmriprep_nodes.PREFIX) : -len("/small")]: r.status
        for r in results
        if r.case.startswith(fmriprep_nodes.PREFIX) and r.case.endswith("/small")
    }
    why = (
        "full size not run in this record (hours of ANTs at one thread each, as built and with "
        "CORE-MATH: the next part, on a larger machine)"
    ) + (f"; compared on every {SMALL}rd voxel instead ({len(small)} small cases)" if small else "")
    return lines + fmriprep_nodes.record_lines(
        PROGRAM, results, why_not=dict.fromkeys(HEAVY_NODES, why)
    )


def suite() -> Suite:
    return Suite(
        name="ants-registration",
        title="antsRegistration (every stage: linear, SyN, B-spline, displacement-field and "
        "velocity-field)",
        tool="`larmorx ants antsRegistration` / `lx.ants.registration` (crates larmorx-registration, "
        "larmorx-ants)",
        reference=f"antsRegistration from {ants_version()}, the binary built by "
        "`scripts/build_ants_oracle.sh`, run with one thread and ANTS_RANDOM_SEED=42",
        thresholds=[
            ("Exit status", "identical"),
            ("Standard output", "identical after masking addresses, times and output paths"),
            ("Standard error", "identical (larmorx's D13 notices aside)"),
            (
                "Files",
                "the same files; transforms byte for byte (HDF5: every dataset), images bit for bit",
            ),
            (
                "glibc's maths",
                "where ANTs as built differs, ANTs with CORE-MATH preloaded must be identical",
            ),
            (
                "Thread band",
                f"reported: ANTs at {', '.join(map(str, BAND_THREADS))} threads against 1",
            ),
        ],
        cases=cases_for,
        run_case=run_case,
        packages=("nibabel", "scipy"),
        highlights=highlights,
        notes="Inputs named `gen:<name>` are built by this suite (seeded phantoms); the real cases "
        "re-run fMRIPrep 25.2.6's own command lines from the oracle runs.",
    )


__all__ = ["all_cases", "run_case", "suite"]
