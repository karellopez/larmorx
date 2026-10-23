# SPDX-License-Identifier: Apache-2.0
"""Phantoms whose correct result is known independently, for the corrections decided on
2026-10-23 (rule A: ANTS-17, ANTS-22, ANTS-23, ANTS-32). Each ``expect_*`` function is an
``AntsCase.fix_expect``: it computes the methodically correct output another way (SciPy's
morphology, NumPy, or the ANTs oracle itself on an input where the defect does not arise) and
compares larmorx's default output with it, bit for bit.
"""

from __future__ import annotations

from pathlib import Path

import numpy as np

from larmorx_validation.parity import ants_inputs

#: ITK's `BinaryBallStructuringElement` of radius 1 in 3D: the 3 × 3 × 3 cube without its
#: corners (offsets with Σ o² ≤ r(r + 1) = 2).
BALL_1 = ((np.indices((3, 3, 3)) - 1) ** 2).sum(0) <= 2

GRID = np.diag([1.0, 1.0, 1.0, 1.0])


def _load(path: str | Path) -> np.ndarray:
    import nibabel as nib

    return np.asarray(nib.load(path).dataobj, dtype=np.float32)


def _compare(got: Path, expected: np.ndarray, what: str) -> tuple[bool, str, object]:
    a = _load(got)
    if a.shape != expected.shape:
        return False, f"{what}: shape {a.shape}, expected {expected.shape}", None
    exp = expected.astype(np.float32)
    differ = int(np.count_nonzero(a.view(np.uint32) != exp.view(np.uint32)))
    return differ == 0, f"{what}: {differ} of {a.size} values differ from it", differ


# ------------------------------------------------------------------------------------------------
# ANTS-17: ME on labels and a fractional foreground, MO with the foreground 0


@ants_inputs.builder("known-labels-cubes")
def _(t):
    """Two 9³ cubes of labels 1 and 2 and a 9³ cube of 0.3 apart in a 40 × 20 × 20 image of 0s,
    1 mm voxels: the erosion of each is known exactly."""
    a = np.zeros((40, 20, 20), np.float32)
    a[2:11, 5:14, 5:14] = 1.0
    a[15:24, 5:14, 5:14] = 2.0
    a[28:37, 5:14, 5:14] = 0.3
    ants_inputs.save(a, t, affine=GRID)


@ants_inputs.builder("known-mask-holes")
def _(t):
    """A 0/1 mask: a 24³ cube of 1s with a one-voxel hole and a 5³ hole, in a 32³ image of 0s
    (1 mm voxels): the opening of its zeros is known."""
    a = np.zeros((32, 32, 32), np.float32)
    a[4:28, 4:28, 4:28] = 1.0
    a[10, 10, 10] = 0.0
    a[16:21, 16:21, 16:21] = 0.0
    ants_inputs.save(a, t, affine=GRID)


def _erode(region: np.ndarray) -> np.ndarray:
    """ITK's binary erosion with the radius-1 ball: outside the image counts as foreground
    (``BoundaryToForeground``)."""
    from scipy import ndimage

    return ndimage.binary_erosion(region, structure=BALL_1, border_value=1)


def expect_erode(value: float):
    """``ME 1 value``'s correct result: 1 where the voxel is still ``value`` after the erosion."""

    def check(args: list[str], outputs: list[Path], _tmp: Path) -> tuple[bool, str, object]:
        data = _load(args[3])
        return _compare(outputs[0], _erode(data == np.float32(value)), "the eroded foreground")

    return check


def expect_open_zeros(args: list[str], outputs: list[Path], _tmp: Path) -> tuple[bool, str, object]:
    """``MO 1 0``'s correct result on a 0/1 mask: SciPy's opening of the zeros (erosion with
    the border as foreground, dilation), the zeros it removes 1."""
    from scipy import ndimage

    data = _load(args[3])
    zeros = data == 0
    opened = ndimage.binary_dilation(_erode(zeros), structure=BALL_1)
    expected = np.where(zeros & ~opened, 1.0, data)
    return _compare(outputs[0], expected, "the opening of the zeros")


# ------------------------------------------------------------------------------------------------
# ANTS-22: PadImage with a fractional pad


def expect_pad(pad: float):
    """``PadImage pad``'s correct result: ``trunc(pad)`` voxels of 0 on each side (cropped if
    negative), the voxels where the origin says (NumPy's ``pad`` and slicing)."""

    def check(args: list[str], outputs: list[Path], _tmp: Path) -> tuple[bool, str, object]:
        data = _load(args[3])
        k = int(pad)
        if k >= 0:
            expected = np.pad(data, k)
        else:
            sl = tuple(slice(-k, n + k) for n in data.shape)
            expected = data[sl]
        ok, detail, value = _compare(outputs[0], expected, f"{k} voxel(s) on each side")
        # The origin moves by exactly k voxels.
        import nibabel as nib

        a_in, a_out = nib.load(args[3]).affine, nib.load(outputs[0]).affine
        shift = a_in[:3, :3] @ np.full(3, -float(k)) + a_in[:3, 3]
        moved = float(np.abs(a_out[:3, 3] - shift).max())
        return ok and moved < 1e-4, f"{detail}; origin off by {moved:.2g} mm", value

    return check


# ------------------------------------------------------------------------------------------------
# ANTS-23: ExtractContours of non-integer values


@ants_inputs.builder("known-float-labels")
def _(t):
    """Regions of 1.2, 1.7 (touching), 0.6, 2, -1.5 and 0.6 + 2 touching in a 30 × 16 × 16 image
    of 0s (none on the image's border; 1 mm voxels)."""
    a = np.zeros((30, 16, 16), np.float32)
    a[2:8, 3:12, 3:12] = 1.2
    a[8:13, 3:12, 3:12] = 1.7
    a[15:20, 3:9, 3:12] = 0.6
    a[15:20, 9:13, 3:12] = 2.0
    a[22:28, 3:12, 3:12] = -1.5
    ants_inputs.save(a, t, affine=GRID)


def expect_contours(fully_connected: bool):
    """``ExtractContours``' correct result: every value its own label (0 the background), the
    voxels of a label with a neighbour of another label (26 or 6 neighbours) keep their value
    (NumPy, by shifting the image; the phantom keeps its labels off the border)."""

    def check(args: list[str], outputs: list[Path], _tmp: Path) -> tuple[bool, str, object]:
        data = _load(args[3])
        p = np.pad(data, 1)
        contour = np.zeros(data.shape, bool)
        n = data.shape
        for dx in (-1, 0, 1):
            for dy in (-1, 0, 1):
                for dz in (-1, 0, 1):
                    if (dx, dy, dz) == (0, 0, 0):
                        continue
                    if not fully_connected and abs(dx) + abs(dy) + abs(dz) != 1:
                        continue
                    nb = p[1 + dx : 1 + dx + n[0], 1 + dy : 1 + dy + n[1], 1 + dz : 1 + dz + n[2]]
                    contour |= nb != data
        expected = np.where(contour & (data != 0), data, 0.0)
        return _compare(outputs[0], expected, "the contours of the values")

    return check


# ------------------------------------------------------------------------------------------------
# ANTS-32: ResampleImageBySpacing's sigma and outside value


@ants_inputs.builder("known-head-2mm")
def _(t):
    """The head phantom's voxels on an axis-aligned grid of 2 mm voxels."""
    ants_inputs.save(ants_inputs.head(1), t, affine=np.diag([2.0, 2.0, 2.0, 1.0]))


@ants_inputs.builder("known-corner-marker")
def _(t):
    """The head phantom on a 1 mm axis-aligned grid with 7777 at index (1, 1, 1), the value ANTs
    gives voxels beyond the input (no other voxel holds it)."""
    a = ants_inputs.head(1).astype(np.float32)
    a[a == 7777.0] = 7776.0
    a[1, 1, 1] = 7777.0
    ants_inputs.save(a, t, affine=GRID)


def expect_sigma_in_voxels(
    args: list[str], outputs: list[Path], tmp: Path
) -> tuple[bool, str, object]:
    """The smoothing in voxels does not depend on the units: the 2 mm phantom resampled to
    4 mm gives the voxels that ANTs itself gives for the same voxels at 1 mm resampled to 2 mm,
    where its millimetres are voxels (the outputs compared bit for bit)."""
    import nibabel as nib

    from larmorx_validation.parity.ants_programs import run_ants

    img = nib.load(args[1])
    unit = tmp / "unit-spacing.nii.gz"
    ants_inputs.save(np.asarray(img.dataobj, np.float32), unit, affine=GRID)
    out = tmp / "unit-spacing-resampled.nii.gz"
    factor = [float(a) / 2.0 for a in args[3:6]]
    code, _out, err = run_ants(
        "ResampleImageBySpacing", [args[0], str(unit), str(out), *[str(f) for f in factor]]
    )
    if code != 0:
        return False, f"ANTs on the 1 mm copy failed: {err.strip()[-200:]}", None
    return _compare(outputs[0], _load(out), "ANTs' output at 1 mm (where mm are voxels)")


def expect_outside_zero(
    args: list[str], outputs: list[Path], tmp: Path
) -> tuple[bool, str, object]:
    """The voxels beyond the input are 0 and the others ANTs': ANTs' output (``ants/out``) with
    its outside value, the marker 7777 at index (1, 1, 1), replaced by 0."""
    ants_out = _load(tmp / "ants" / "out.nii.gz")
    expected = np.where(ants_out == 7777.0, 0.0, ants_out)
    outside = int((ants_out == 7777.0).sum())
    ok, detail, value = _compare(outputs[0], expected, f"ANTs' with its {outside} outside voxels 0")
    return ok and outside > 0, detail, value


# ------------------------------------------------------------------------------------------------
# ANTS-40: N4's zero and negative voxels


@ants_inputs.builder("known-n4-zero-background")
def _(t):
    """The head phantom's brain (a smooth bias times WM, GM, CSF, with noise) on an exact 0
    background and with 1 % of the brain's voxels 0 or -20 (dropout), 1.5 mm voxels: N4 without
    a mask fits the zeros and negative values like tissue."""
    rng = np.random.default_rng(40)
    a = ants_inputs.head(3).astype(np.float32)
    brain = ants_inputs.brain_mask().astype(bool)
    a = np.where(brain, a, 0.0)
    drop = brain & (rng.random(a.shape) < 0.01)
    a = np.where(drop, np.where(rng.random(a.shape) < 0.5, 0.0, -20.0), a).astype(np.float32)
    ants_inputs.save(a, t, affine=np.diag([1.5, 1.5, 1.5, 1.0]))


def expect_n4_masked(args: list[str], outputs: list[Path], tmp: Path) -> tuple[bool, str, object]:
    """N4's correct field with the non-positive voxels left out is ANTs' own field with a mask
    of the positive voxels (where the defect cannot arise): the ANTs oracle at one thread with
    correctly rounded maths (CORE-MATH), ``-x positive -r 0``, its bias field compared with
    larmorx's default bias field bit for bit."""
    import nibabel as nib

    from larmorx_validation.parity.ants_programs import run_ants

    i = args.index("-i")
    img = nib.load(args[i + 1])
    positive = tmp / "positive-mask.nii.gz"
    ants_inputs.save(
        (np.asarray(img.dataobj, np.float32) > 0).astype(np.uint8), positive, affine=img.affine
    )
    c, b = tmp / "ants-masked-c.nii.gz", tmp / "ants-masked-b.nii.gz"
    o = args.index("-o")
    argv = [*args[:o], "-x", str(positive), "-r", "0", "-o", f"[{c},{b}]", *args[o + 2 :]]
    code, _out, err = run_ants("N4BiasFieldCorrection", argv, 1, correctly_rounded=True)
    if code != 0:
        return False, f"ANTs with the positive voxels' mask failed: {err.strip()[-200:]}", None
    return _compare(outputs[1], _load(b), "ANTs' bias field with a mask of the positive voxels")


__all__ = [
    "expect_contours",
    "expect_erode",
    "expect_n4_masked",
    "expect_open_zeros",
    "expect_outside_zero",
    "expect_pad",
    "expect_sigma_in_voxels",
]
