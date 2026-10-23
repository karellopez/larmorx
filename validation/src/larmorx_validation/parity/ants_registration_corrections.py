# SPDX-License-Identifier: Apache-2.0
"""What the corrections decided on 2026-10-21 change in ``antsRegistration`` (A5 part 3): each
case runs larmorx in its default mode (the corrections) and in ``reproduce`` mode (ANTs' result,
bit for bit: the ``ants-registration`` suite compares that mode with ANTs) and measures the
difference on phantoms whose answer is known, or, for ITK-35 (stages after a B-spline
transform, which ANTs cannot run), against the closest runs ANTs can do.

- **Point-set metrics** (ANTS-67, ANTS-68, ANTS-70; ITK-24, ITK-29): a rigid registration of the
  point-set phantoms, whose moving blocks are the fixed ones shifted by (1, −1, 1) voxels, i.e.
  by (−2, 2, 2.5) mm in ITK's LPS axes: the translation error of each mode.
- **ANTS-78** (B-spline field meshes at the base level): larmorx's corrected stage with twice
  the mesh must be ANTs' stage (reproduce) byte for byte; and how far the corrected fields move
  from ANTs' on the registration phantoms (within their mask), with the fit in each mode.
- **ITK-35** (the B-spline transform's Jacobian with respect to position): the stage after the
  B-spline transform, corrected, against ANTs' stage on the moving image resampled through the
  B-spline transform (a linear composite) and with the B-spline transform sampled as a
  displacement field (on the moving grid and on one of half its spacing): how far the
  resulting transforms map the fixed image's points apart (mm, within the mask) and the mean
  squared difference each leaves, computed exactly through the B-spline transform and the stage's
  result. ``reproduce`` must fail as ANTs does. The references with a displacement field are not
  repeatable: a linear stage after a displacement field estimates its scales and learning rate
  from 1000 voxels that ITK draws with a generator seeded from the clock (ANTS-72; with
  ``time`` and ``clock`` fixed, the runs are identical), so each runs ``FIELD_RUNS`` times and
  the record gives the median and the range of each number, and how far ANTs' own runs map the
  points apart.

Every number is reported; a case fails only where a correction is not named, a mode does not
run as it should, or an equivalence that must be exact is not.
"""

from __future__ import annotations

import os
import re
import subprocess
import tempfile
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np

from larmorx_validation.parity import ants_inputs, defects
from larmorx_validation.parity.ants_programs import ants_bin, ants_version
from larmorx_validation.parity.harness import SKIPPED, Case, CheckList, Outcome, Suite

#: The point-set phantoms' shift in LPS mm (ants_point_sets: the moving blocks moved by
#: (1, −1, 1) voxels of 2 × 2 × 2.5 mm, x and y reversed in LPS).
TRUE_SHIFT = np.array([-2.0, 2.0, 2.5])
SEED = "42"
REPORTED = "reported"
#: How many times each reference with a displacement field runs (ANTS-72: they vary).
FIELD_RUNS = 5


@dataclass(frozen=True)
class Scenario:
    kind: str  # "point-set", "mesh", "chain"
    args: tuple[str, ...] = ()
    #: defects the default mode must name
    named: tuple[str, ...] = ()
    #: mesh: the stage with twice the mesh (the corrected equivalent of ANTs' stage)
    doubled: tuple[str, ...] = ()
    #: chain: the stage after the B-spline transform, and where the B-spline transform comes
    #: from ("initial": a generated file given with -r; "stage": the run's first stage)
    second: tuple[str, ...] = ()
    first: tuple[str, ...] = ()
    second_file: str = ""
    reproduce_fails: bool = True
    extra: dict = field(default_factory=dict)


PSF, PSM = "{gen:ps-labels-fixed}", "{gen:ps-labels-moving}"
PSK, PSKM = "{gen:ps-mask}", "{gen:ps-mask-moving}"
PSI, PSIM = "{gen:ps-intensity-fixed}", "{gen:ps-intensity-moving}"
F, M, K = "{gen:reg-fixed}", "{gen:reg-moving}", "{gen:reg-mask}"
MS = f"MeanSquares[{F},{M},1]"


def _rigid(metric: str, its: str = "[40,1e-8,10]") -> tuple[str, ...]:
    return ("-x", PSK, "-t", "Rigid[0.1]", "-m", metric, "-c", its, "-f", "1", "-s", "0")


def _two(t: str, its: str = "[20x10,1e-6,5]") -> tuple[str, ...]:
    return ("-x", K, "-t", t, "-m", MS, "-c", its, "-f", "2x1", "-s", "1x0vox")


def _linear(t: str) -> tuple[str, ...]:
    return ("-x", K, "-t", t, "-m", MS, "-c", "[100,1e-8,10]", "-f", "1", "-s", "0vox")


BSP = ("-x", K, "-t", "BSpline[0.2,2x2x2]", "-m", MS, "-c", "[3,1e-6,3]", "-f", "1", "-s", "0")
BSP2 = ("-x", K, "-t", "BSpline[0.2,1x1x1]", "-m", MS, "-c", "[20,1e-6,5]", "-f", "1", "-s", "0")
IGDM = f"IGDM[{PSI},{PSIM},1,{PSK},{PSKM}"

SCENARIOS: dict[str, tuple[str, str, Scenario]] = {
    # id: (category, description, scenario)
    "igdm/one-neighbour": ("igdm", "IGDM, given sigmas (50, 5), one neighbour, central differences: the true derivative alone (ANTS-68)",
                           Scenario("point-set", _rigid(f"{IGDM},0x0x0,50,5,1,0]"), ("ANTS-68",))),
    "igdm/one-neighbour-gradient-sigma": ("igdm", "as above with gradientSigma 1 mm: Gaussian gradients (ANTS-67, ANTS-68)",
                                          Scenario("point-set", _rigid(f"{IGDM},0x0x0,50,5,1,1]"), ("ANTS-67", "ANTS-68"))),
    "igdm/five-neighbours": ("igdm", "kNeighborhood 5, central differences (ANTS-67, ANTS-68)",
                             Scenario("point-set", _rigid(f"{IGDM},0x0x0,50,5,5,0]"), ("ANTS-67", "ANTS-68"))),
    "igdm/five-neighbours-gradient-sigma": ("igdm", "kNeighborhood 5, gradientSigma 1.5 mm (ANTS-67, ANTS-68)",
                                            Scenario("point-set", _rigid(f"{IGDM},0x0x0,50,5,5,1.5]"), ("ANTS-67", "ANTS-68"))),
    "igdm/estimated-sigmas": ("igdm", "estimated sigmas: the distance sigma's floor applies (ANTS-70, with ANTS-65, ANTS-67, ANTS-68)",
                              Scenario("point-set", _rigid(f"{IGDM}]"), ("ANTS-70",))),
    "igdm/same-sets-half-voxel": ("igdm", "an image with itself, moved by half a voxel: every nearest distance 1 mm, ANTs' sigma 0 and NaN parameters (ANTS-70)",
                                  Scenario("point-set", ("-r", "{gen-transform:half-voxel-x.txt}", *_rigid(f"IGDM[{PSI},{PSI},1,{PSK},{PSK}]", "[10,1e-8,10]")),
                                           ("ANTS-70",), extra={"truth": [0.0, 0.0, 0.0]})),
    "jhct/isotropic-sigma-2": ("jhct", "JHCT, isotropic, sigma 2, alpha 1.1: the total number of points alone (ITK-24)",
                               Scenario("point-set", _rigid(f"JHCT[{PSF},{PSM},1,1,0,2,50,1.1,0]"), ("ITK-24",))),
    "jhct/isotropic-sigma-1": ("jhct", "JHCT, isotropic, sigma 1: ITK-24 and ITK-23",
                               Scenario("point-set", _rigid(f"JHCT[{PSF},{PSM},1,1,0,1,50,1.1,0]"), ("ITK-24",))),
    "jhct/anisotropic-sigma-1": ("jhct", "JHCT, anisotropic, sigma 1, alpha 1.1: the Parzen kernel sigma (ITK-29, with ITK-24 and ITK-26)",
                                 Scenario("point-set", _rigid(f"JHCT[{PSF},{PSM},1,1,0,1,50,1.1,1]"), ("ITK-24", "ITK-29"))),
    "jhct/anisotropic-sigma-2": ("jhct", "JHCT, anisotropic, sigma 2, alpha 1.5 (ITK-29, ITK-24, ITK-26)",
                                 Scenario("point-set", _rigid(f"JHCT[{PSF},{PSM},1,1,0,2,50,1.5,1]"), ("ITK-24", "ITK-29"))),
    "mesh/bspline-field-3": ("mesh", "BSplineDisplacementField 3x3x3 over two levels: ANTs' stage is the corrected one with 6x6x6; the coarse corrected level needs ITK-42",
                             Scenario("mesh", _two("BSplineDisplacementField[0.3,3x3x3,0,3]"), ("ANTS-78",),
                                      doubled=_two("BSplineDisplacementField[0.3,6x6x6,0,3]"))),
    "mesh/bspline-field-5-total-4": ("mesh", "BSplineDisplacementField 5x5x5 with a total mesh of 4",
                                     Scenario("mesh", _two("BSplineDisplacementField[0.25,5x5x5,4x4x4,3]"), ("ANTS-78",),
                                              doubled=_two("BSplineDisplacementField[0.25,10x10x10,8x8x8,3]"))),
    "mesh/bspline-field-knots": ("mesh", "BSplineDisplacementField with a knot spacing of 12 mm",
                                 Scenario("mesh", _two("BSplineDisplacementField[0.3,12,0,3]"), ("ANTS-78",))),
    "mesh/bspline-exponential-4": ("mesh", "BSplineExponential 4x4x4 (the sigmas in mm: ANTS-80 aside)",
                                   Scenario("mesh", ("-x", K, "-t", "BSplineExponential[0.3,4x4x4,0,0,3]", "-m", MS, "-c", "[20x10,1e-6,5]", "-f", "2x1", "-s", "1x0mm"), ("ANTS-78",),
                                            doubled=("-x", K, "-t", "BSplineExponential[0.3,8x8x8,0,0,3]", "-m", MS, "-c", "[20x10,1e-6,5]", "-f", "2x1", "-s", "1x0mm"))),
    "chain/bspline-then-affine": ("bspline-chain", "a BSpline stage (mesh 2x2x2) then Affine (ANTs drops the affine stage, ANTS-49)",
                                  Scenario("chain", first=BSP, second=_linear("Affine[0.1]"), second_file="1GenericAffine.mat", named=("ITK-35",))),
    "chain/bspline-then-rigid": ("bspline-chain", "a BSpline stage then Rigid",
                                 Scenario("chain", first=BSP, second=_linear("Rigid[0.1]"), second_file="1GenericAffine.mat", named=("ITK-35",))),
    "chain/initial-bspline-then-rigid": ("bspline-chain", "-r a B-spline transform (smooth coefficients up to 2 mm), then Rigid",
                                         Scenario("chain", first=(), second=_linear("Rigid[0.1]"), second_file="1GenericAffine.mat", named=("ITK-35",))),
    "chain/initial-bspline-then-affine": ("bspline-chain", "-r a B-spline transform, then Affine",
                                          Scenario("chain", first=(), second=_linear("Affine[0.1]"), second_file="1GenericAffine.mat", named=("ITK-35",))),
    "chain/bspline-then-bspline": ("bspline-chain", "two BSpline stages (ANTs fails at the second)",
                                   Scenario("chain", first=BSP, second=BSP2, second_file="1BSpline.txt", named=("ITK-35",))),
}  # fmt: skip


def cases_for(tier: str) -> list[Case]:
    return [Case(i, cat, desc, sc) for i, (cat, desc, sc) in SCENARIOS.items()]


def _resolve(args: tuple[str, ...], out: Path, tmp: Path) -> list[str]:
    from larmorx_validation.parity.ants_registration import _transform_file
    from larmorx_validation.parity.ants_registration_bspline import transform_file

    def sub(m: re.Match[str]) -> str:
        key = m.group(1)
        if key == "out":
            return str(out)
        if key.startswith("gen:"):
            return str(ants_inputs.path(key[4:]))
        if key.startswith("gen-transform:"):
            return str(_transform_file(tmp, key[14:]))
        if key.startswith("gen-bsp:"):
            return str(transform_file(tmp, key[8:]))
        raise KeyError(key)

    return [re.sub(r"\{([^{}]+)\}", sub, a) for a in args]


def _larmorx(args: list[str], mode: str) -> tuple[int, str, str]:
    from larmorx_validation.parity.ants_registration import _run_larmorx

    return _run_larmorx(args, SEED, mode)


def _ants(args: list[str]) -> tuple[int, str, str]:
    from larmorx_validation.parity.ants_registration import _run_ants

    code, out, err = _run_ants(args, SEED)
    return code, out, err


def _apply(args: list[str]) -> None:
    b = ants_bin()
    env = {**os.environ, "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1"}
    env.pop("LD_PRELOAD", None)
    subprocess.run(
        [str(b / "antsApplyTransforms"), "-d", "3", *args], env=env, check=True, capture_output=True
    )


def _params(path: Path) -> np.ndarray:
    import larmorx as lx

    return np.concatenate([np.asarray(t.parameters, float) for t in lx.transforms.read(path)])


def _named(err: str) -> list[str]:
    return defects.notices_in(err)


def _mask() -> np.ndarray:
    import nibabel as nib

    return np.asarray(nib.load(ants_inputs.path("reg-mask")).dataobj) > 0


def _displacement(path: Path, tmp: Path) -> np.ndarray:
    """A transform file's displacement on the fixed phantom's grid (antsApplyTransforms' field
    output), as (voxels, 3) in the image's C order."""
    import nibabel as nib

    out = tmp / (path.name + f".{abs(hash(str(path)))}.field.nii.gz")
    f = str(ants_inputs.path("reg-fixed"))
    _apply(["-i", f, "-r", f, "-t", str(path), "-o", f"[{out},1]"])
    return np.asarray(nib.load(out).dataobj, dtype=float).reshape(-1, 3)


def _distance(a: Path, b: Path, tmp: Path) -> tuple[float, float]:
    k = _mask().reshape(-1)
    d = np.linalg.norm(_displacement(a, tmp) - _displacement(b, tmp), axis=1)[k]
    return float(d.max()), float(d.mean())


def _true_mse(bspline: Path, second: Path, tmp: Path) -> float:
    """The mean squared difference, in the mask, between the fixed phantom and the moving one
    through the B-spline transform and the stage's result (antsApplyTransforms, linear)."""
    import nibabel as nib

    f, m = ants_inputs.path("reg-fixed"), ants_inputs.path("reg-moving")
    out = tmp / f"warped-{abs(hash(str(second)))}.nii.gz"
    _apply(["-i", str(m), "-r", str(f), "-t", str(bspline), "-t", str(second), "-o", str(out)])
    a = np.asarray(nib.load(f).dataobj, float)
    b = np.asarray(nib.load(out).dataobj, float)
    k = _mask()
    return float(np.mean((a[k] - b[k]) ** 2))


def _fmt(v: float) -> str:
    return f"{v:.4g}"


def _named_check(checks: CheckList, err: str, expected: tuple[str, ...]) -> None:
    named = _named(err)
    missing = [d for d in expected if d not in named]
    checks(
        "corrections named",
        not missing,
        metric="defects named in the default mode",
        value=", ".join(named) or "none",
        threshold="includes " + ", ".join(expected),
        detail="missing " + ", ".join(missing) if missing else "",
    )


def _point_set(sc: Scenario, tmp: Path, checks: CheckList) -> None:
    truth = np.array(sc.extra.get("truth", TRUE_SHIFT), dtype=float)
    res = {}
    for mode in (defects.REPRODUCE, defects.FIX):
        d = tmp / mode
        d.mkdir()
        code, out, err = _larmorx(
            _resolve((*sc.args, "-d", "3", "-v", "1", "-o", "{out}/r"), d, tmp), mode
        )
        if code != 0:
            raise AssertionError(f"{mode}: exit {code}: {err.strip()[-300:]}")
        files = sorted(d.glob("r*GenericAffine.mat"))
        p = _params(files[-1])
        res[mode] = (p, err, out)
        checks(
            f"{mode}: translation error (mm)",
            True,
            metric="|translation − the phantoms' shift|",
            value=_fmt(float(np.linalg.norm(p[-3:] - truth))),
            threshold=REPORTED,
            detail=f"translation {', '.join(_fmt(v) for v in p[-3:])} mm; the shift {', '.join(_fmt(v) for v in truth)} mm",
        )
        its = [
            ln for ln in out.splitlines() if "DIAGNOSTIC," in ln and not ln.startswith("DIAGNOSTIC")
        ]
        checks(
            f"{mode}: iterations",
            True,
            metric="DIAGNOSTIC lines",
            value=len(its),
            threshold=REPORTED,
        )
    pr, pf = res[defects.REPRODUCE][0], res[defects.FIX][0]
    checks(
        "parameters changed by the corrections",
        True,
        metric="max |default − reproduce|",
        value=_fmt(float(np.max(np.abs(pf - pr)))),
        threshold=REPORTED,
    )
    _named_check(checks, res[defects.FIX][1], sc.named)


def _field(d: Path) -> np.ndarray:
    import nibabel as nib

    f = next(p for p in sorted(d.glob("r*Warp.nii.gz")) if "Inverse" not in p.name)
    return np.asarray(nib.load(f).dataobj, dtype=float).reshape(-1, 3)


def _warped_mse(d: Path) -> float:
    import nibabel as nib

    a = np.asarray(nib.load(ants_inputs.path("reg-fixed")).dataobj, float)
    b = np.asarray(nib.load(d / "w.nii.gz").dataobj, float)
    k = _mask()
    return float(np.mean((a[k] - b[k]) ** 2))


def _mesh(sc: Scenario, tmp: Path, checks: CheckList) -> None:
    head = ("-d", "3", "-v", "1", "-o", "[{out}/r,{out}/w.nii.gz]")
    runs = {}
    for label, mode, args in (
        ("reproduce", defects.REPRODUCE, sc.args),
        ("default", defects.FIX, sc.args),
        *((("default-doubled", defects.FIX, sc.doubled),) if sc.doubled else ()),
    ):
        d = tmp / label
        d.mkdir()
        code, _out, err = _larmorx(_resolve((*head, *args), d, tmp), mode)
        if code != 0:
            raise AssertionError(f"{label}: exit {code}: {err.strip()[-300:]}")
        runs[label] = (d, err)
    if sc.doubled:
        a = (runs["reproduce"][0] / "r0Warp.nii.gz").read_bytes()
        b = (runs["default-doubled"][0] / "r0Warp.nii.gz").read_bytes()
        checks(
            "ANTs' stage is the corrected one with twice the mesh",
            a == b,
            metric="reproduce (the given mesh) against default (twice it)",
            value="byte-identical" if a == b else "different",
            threshold="byte-identical",
        )
    k = _mask().reshape(-1)
    fr, fd = _field(runs["reproduce"][0]), _field(runs["default"][0])
    n = np.linalg.norm(fr - fd, axis=1)[k]
    for name, value in (
        ("field moved by the correction: largest (mm)", n.max()),
        ("field moved by the correction: mean (mm)", n.mean()),
        ("default field: largest vector (mm)", np.linalg.norm(fd, axis=1)[k].max()),
        ("reproduce field: largest vector (mm)", np.linalg.norm(fr, axis=1)[k].max()),
    ):
        checks(name, True, metric="in the mask", value=_fmt(float(value)), threshold=REPORTED)
    for label in ("reproduce", "default"):
        checks(
            f"{label}: mean squared difference in the mask",
            True,
            metric="(fixed − warped)²",
            value=_fmt(_warped_mse(runs[label][0])),
            threshold=REPORTED,
        )
    _named_check(checks, runs["default"][1], sc.named)


def _chain(sc: Scenario, tmp: Path, checks: CheckList) -> None:
    import nibabel as nib

    from larmorx_validation.parity.ants_registration_bspline import transform_file

    head = ("-d", "3", "-v", "1", "-o", "{out}/r")
    initial = not sc.first
    bsp_in = transform_file(tmp, "bspline-3d.txt") if initial else None
    pre = ("-r", str(bsp_in)) if initial else ()
    # larmorx: the chain, both modes.
    chain = {}
    for mode in (defects.REPRODUCE, defects.FIX):
        d = tmp / f"chain-{mode}"
        d.mkdir()
        code, _out, err = _larmorx(_resolve((*head, *pre, *sc.first, *sc.second), d, tmp), mode)
        chain[mode] = (code, d, err)
    code_r, d_r, _ = chain[defects.REPRODUCE]
    code_f, d_f, err_f = chain[defects.FIX]
    fails_r = code_r != 0 or not (d_r / f"r{sc.second_file}").exists()
    checks(
        "reproduce: the stage after the B-spline transform fails, as in ANTs",
        fails_r,
        metric="exit status and output",
        value=f"exit {code_r}, {sc.second_file} {'missing' if not (d_r / f'r{sc.second_file}').exists() else 'written'}",
        threshold="a failure (an exception; a linear stage dropped)",
    )
    if code_f != 0:
        raise AssertionError(f"default: exit {code_f}: {err_f.strip()[-300:]}")
    _named_check(checks, err_f, sc.named)
    bsp = bsp_in if initial else d_f / "r0BSpline.txt"
    second = d_f / f"r{sc.second_file}"
    # ANTs: its stage on the moving image resampled through the B-spline transform, and with
    # the B-spline transform as a displacement field (the moving grid, and half its spacing).
    m = ants_inputs.path("reg-moving")
    mov = tmp / "moving-through-bspline.nii.gz"
    _apply(["-i", str(m), "-r", str(m), "-t", str(bsp), "-o", str(mov)])
    img = nib.load(m)
    a = img.affine.copy()
    a[:3, :3] = a[:3, :3] / 2
    a[:3, 3] = img.affine[:3, 3] - 0.25 * (img.affine[:3, :3] @ np.ones(3))
    fine = tmp / "fine-reference.nii.gz"
    nib.Nifti1Image(np.zeros(tuple(2 * s for s in img.shape), np.float32), a).to_filename(fine)
    constructions = {}
    second_args = list(_resolve(sc.second, tmp, tmp))
    resampled = [x.replace(str(m), str(mov)) for x in second_args]
    d = tmp / "ants-resampled"
    d.mkdir()
    code, _o, err = _ants(["-d", "3", "-v", "1", "-o", str(d / "r"), *resampled])
    constructions["ANTs on the moving image resampled through the B-spline transform"] = (
        code,
        d / f"r{sc.second_file}".replace("r1", "r0", 1),
    )
    repeated = {}
    for tag, ref in (("moving grid", m), ("half its spacing", fine)):
        fld = tmp / f"bspline-field-{'fine' if ref == fine else 'grid'}.nii.gz"
        _apply(["-i", str(m), "-r", str(ref), "-t", str(bsp), "-o", f"[{fld},1]"])
        runs = []
        for r in range(FIELD_RUNS):
            d = tmp / f"ants-field-{'fine' if ref == fine else 'grid'}-{r}"
            d.mkdir()
            code, _o, err = _ants(
                ["-d", "3", "-v", "1", "-o", str(d / "r"), "-r", str(fld), *second_args]
            )
            runs.append((code, d / f"r{sc.second_file}"))
        repeated[f"ANTs with the B-spline transform as a displacement field ({tag})"] = runs
    checks(
        "default: mean squared difference left (exact composite)",
        True,
        metric="(fixed − moving through both)² in the mask",
        value=_fmt(_true_mse(bsp, second, tmp)),
        threshold=REPORTED,
    )
    for label, runs in repeated.items():
        _repeated_reference(checks, label, runs, second, bsp, tmp)
    for label, (code, path) in constructions.items():
        if code != 0 or not path.exists():
            checks(label, False, detail=f"ANTs exit {code}")
            continue
        mx, mean = _distance(second, path, tmp)
        metric = "where the two results map the fixed points apart, in the mask"
        checks(
            f"{label}: largest distance (mm)",
            True,
            metric=metric,
            value=_fmt(mx),
            threshold=REPORTED,
        )
        checks(
            f"{label}: mean distance (mm)",
            True,
            metric=metric,
            value=_fmt(mean),
            threshold=REPORTED,
        )
        checks(
            f"{label}: mean squared difference left",
            True,
            metric="(fixed − moving through both)² in the mask",
            value=_fmt(_true_mse(bsp, path, tmp)),
            threshold=REPORTED,
        )


def _repeated_reference(
    checks: CheckList,
    label: str,
    runs: list[tuple[int, Path]],
    second: Path,
    bsp: Path,
    tmp: Path,
) -> None:
    """A reference that varies from run to run (ANTS-72): the median and range over its runs of
    each number, and the largest distance between its own runs' transforms."""
    ok = [path for code, path in runs if code == 0 and path.exists()]
    if len(ok) < len(runs):
        checks(label, False, detail=f"ANTs failed in {len(runs) - len(ok)} of {len(runs)} runs")
        if not ok:
            return
    metric = "where the two results map the fixed points apart, in the mask"
    note = f"ANTs varies from run to run (ANTS-72): median and range over {len(ok)} runs"
    dist = [_distance(second, path, tmp) for path in ok]
    mse = [_true_mse(bsp, path, tmp) for path in ok]

    def spread(values: list[float]) -> tuple[str, str]:
        v = np.asarray(values)
        return _fmt(float(np.median(v))), f"{_fmt(float(v.min()))} to {_fmt(float(v.max()))}"

    for name, values in (
        ("largest distance (mm)", [d[0] for d in dist]),
        ("mean distance (mm)", [d[1] for d in dist]),
        ("mean squared difference left", mse),
    ):
        median, rng = spread(values)
        checks(
            f"{label}: {name}",
            True,
            metric=metric if "distance" in name else "(fixed − moving through both)² in the mask",
            value=median,
            threshold=REPORTED,
            detail=f"{note}: {median} (range {rng})",
        )
    own = 0.0
    for i in range(len(ok)):
        for j in range(i + 1, len(ok)):
            own = max(own, _distance(ok[i], ok[j], tmp)[0])
    checks(
        f"{label}: ANTs' own runs",
        True,
        metric="the largest distance between two of ANTs' runs, in the mask",
        value=_fmt(own),
        threshold=REPORTED,
        detail=f"{len(ok)} runs at one thread with ANTS_RANDOM_SEED={SEED} (ANTS-72)",
    )


def run_case(case: Case, checks: CheckList) -> None:
    sc: Scenario = case.payload
    if ants_bin() is None:
        raise Outcome(SKIPPED, "the ANTs oracle was not found")
    with tempfile.TemporaryDirectory(prefix="lx-reg-corr-") as t:
        tmp = Path(t)
        {"point-set": _point_set, "mesh": _mesh, "chain": _chain}[sc.kind](sc, tmp, checks)


def highlights(results: list) -> list[str]:
    """ITK-35's references, with the spread of those that vary from run to run (ANTS-72)."""
    rows = []
    for r in results:
        if not r.case.startswith("chain/"):
            continue
        found: dict[str, dict[str, str]] = {}
        for ch in r.checks:
            label, _, name = ch.name.partition(": ")
            if not label.startswith("ANTs "):
                continue
            ref = label.removeprefix("ANTs ")
            m = re.search(r"\(range ([^)]+)\)", ch.detail)
            value = f"{ch.value} ({m.group(1)})" if m else str(ch.value)
            found.setdefault(ref, {})[name] = value
        for ref, values in found.items():
            rows.append(
                (
                    f"`{r.case}`",
                    ref,
                    values.get("largest distance (mm)", ""),
                    values.get("mean distance (mm)", ""),
                    values.get("ANTs' own runs", "– (one construction, repeatable)"),
                )
            )
    if not rows:
        return []
    header = (
        "| Case | ANTs reference | largest distance (mm) | mean distance (mm) | "
        "ANTs' own runs: largest distance (mm) |"
    )
    lines = [
        "**ITK-35's references** (how far each maps the fixed points from larmorx's corrected "
        "stage, in the mask). A linear stage after a displacement field is not repeatable in "
        f"ANTs (ANTS-72): those references ran {FIELD_RUNS} times each, and show the median "
        "with the range in brackets, and the largest distance between two of ANTs' own runs.",
        "",
        header,
        "|---|---|---|---|---|",
        *(f"| {' | '.join(row)} |" for row in rows),
    ]
    return lines


def suite() -> Suite:
    return Suite(
        name="ants-registration-corrections",
        title="antsRegistration: what the corrections decided on 2026-10-21 change",
        tool="`larmorx ants antsRegistration` / `lx.ants.registration`, default mode against "
        "`reproduce` (ANTs' result, which the ants-registration record compares with ANTs)",
        reference=f"larmorx in `reproduce` mode; for ITK-35, antsRegistration and "
        f"antsApplyTransforms from {ants_version()} (one thread, ANTS_RANDOM_SEED=42)",
        thresholds=[
            ("Corrections named", "every correction the case exercises is named in a notice"),
            ("Effects", "reported: translation errors, field differences, fits, distances"),
            ("ANTS-78", "the corrected stage with twice the mesh is ANTs' stage byte for byte"),
            ("ITK-35", "`reproduce` fails where ANTs fails; the default runs"),
        ],
        cases=cases_for,
        run_case=run_case,
        packages=("nibabel",),
        notes="The phantoms are the ants-registration suite's (`gen:reg-*`, `gen:ps-*`).",
        highlights=highlights,
    )


__all__ = ["SCENARIOS", "run_case", "suite"]
