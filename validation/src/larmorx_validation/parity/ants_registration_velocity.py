# SPDX-License-Identifier: Apache-2.0
"""The velocity-field cases of the ``ants-registration`` parity suite (A5 part 3): every option
of the ``Exponential``, ``BSplineExponential``, ``TimeVaryingVelocityField`` and
``TimeVaryingBSplineVelocityField`` stages, their outputs (``Warp`` and ``InverseWarp`` images,
``VelocityField`` images, composites), initial transforms, image and point-set metrics, masks,
2D, 3D and 4D, float, and every message and failure; and the paths of the corrections decided
on 2026-10-21 (ANTS-67, ANTS-68, ANTS-70, ITK-24, ITK-29, ITK-35, ANTS-78), whose reproduce
runs are compared with ANTs and whose fix passes measure what the corrections change.

The phantoms are the suite's (``gen:reg-*``, ``gen:ps-*``). The time-varying B-spline stage's
single point-set path runs on point-set phantoms stored so that ITK reads them with the identity
direction (``gen:ps-*-lpsid``): with any other direction ANTs fits the points outside the
B-spline's parametric domain and throws (ITK-38), as the cases on the RAS-stored phantoms show.

What cannot be compared: the Exponential, BSplineExponential and TimeVaryingVelocityField
stages never set their metric's sampling seed (ANTS-82), so ANTs samples from the clock;
an affine stage after a velocity field samples its scales from the clock (ANTS-72). Both are
expected divergences.
"""

from __future__ import annotations

from larmorx_validation.parity.ants_registration_syn import (
    PSF,
    PSI,
    PSIM,
    PSK,
    PSKM,
    PSM,
    _stage,
)

F, M, K = "{gen:reg-fixed}", "{gen:reg-moving}", "{gen:reg-mask}"
F2, M2 = "{gen:reg-fixed-2d}", "{gen:reg-moving-2d}"
F4, M4 = "{gen:reg-fixed-4d}", "{gen:reg-moving-4d}"
FO, MO = "{gen:reg-fixed-oblique}", "{gen:reg-moving-oblique}"
KO = "{gen:reg-mask-oblique}"
LF, LM = "{gen:ps-labels-fixed-lpsid}", "{gen:ps-labels-moving-lpsid}"
LK, LKM = "{gen:ps-mask-lpsid}", "{gen:ps-mask-moving-lpsid}"
LI, LIM = "{gen:ps-intensity-fixed-lpsid}", "{gen:ps-intensity-moving-lpsid}"
OUT = "[{out}/r,{out}/w.nii.gz,{out}/iw.nii.gz]"
PRE = "{out}/r"
WARPED = "[{out}/r,{out}/w.nii.gz]"
EXP, BEXP, TVF, TVB, FIX = (
    "exponential",
    "bspline-exponential",
    "time-varying",
    "time-varying-bspline",
    "corrections",
)

MS = f"MeanSquares[{F},{M},1]"
CC = f"CC[{F},{M},1,2]"
ONE = {"c": "[3,1e-6,3]", "f": "1", "s": "0"}
TWO = {"c": "[3x2,1e-6,3]", "f": "2x1", "s": "1x0vox"}
AFFINE = _stage("Affine[0.1]", [MS], c="[6,1e-6,5]", f="1", s="0vox")
CLOCK = (
    "the stage never sets its metric's sampling seed (ANTS-82): ANTs samples from the clock, "
    "two runs differ; larmorx samples with the registration's seed"
)


def velocity_cases(make) -> list:
    """The cases; `make(id, category, description, *args, d=, out=, **kw)` builds one."""
    return _exponential(make) + _bspline_exponential(make) + _tvf(make) + _tvb(make) + _fixes(make)


def _exponential(make) -> list:
    e = "Exponential[0.5,3,0.5]"

    def exp(name: str, description: str, *args: str, **kw):
        return make(f"{EXP}/{name}", EXP, description, *args, **kw)

    return [
        exp("meansquares", "Exponential (update variance 3, velocity variance 0.5), MeanSquares, 2 levels, warped images",
            *_stage(e, [MS], **TWO), out=OUT, band=True, tier="smoke"),
        exp("no-collapse", "-z 0 after an affine stage: 0Affine.mat, 1Warp, 1InverseWarp and 1VelocityField",
            "-z", "0", *AFFINE, *_stage(e, [MS], **TWO), out=OUT),
        exp("short-name", "-t Exp[...] with -z 0: no InverseWarp file (ANTS-81)",
            "-z", "0", *_stage("Exp[0.5,3,0.5]", [MS], **TWO), out=OUT),
        exp("collapse", "an affine stage, then the velocity field: collapsed", *AFFINE, *_stage(e, [MS], **TWO), out=OUT),
        exp("composite", "-a 1", "-a", "1", *AFFINE, *_stage(e, [MS], **TWO), out=OUT),
        exp("integration-steps", "4 integration steps, no velocity smoothing", *_stage("Exponential[0.5,2,0,4]", [MS], **TWO), out=OUT),
        exp("sigmas-in-mm", "smoothing sigmas given in mm (as ANTs takes voxels, ANTS-80)",
            *_stage(e, [MS], c="[3x2,1e-6,3]", f="2x1", s="2x0mm"), out=PRE),
        exp("cc", "CC (radius 2)", *_stage(e, [CC], **TWO), out=OUT),
        exp("mattes", "Mattes (24 bins)", *_stage(e, [f"Mattes[{F},{M},1,24]"], **TWO), out=OUT),
        exp("mi2", "MI2", *_stage(e, [f"MI2[{F},{M},1,20]"], **TWO), out=OUT),
        exp("gc", "GC (ITK-30)", *_stage(e, [f"GC[{F},{M},1]"], **TWO), out=OUT),
        exp("demons", "Demons", *_stage(e, [f"Demons[{F},{M},1]"], **TWO), out=OUT),
        exp("multi", "CC (0.7) and Mattes (0.3)", *_stage(e, [f"CC[{F},{M},0.7,2]", f"Mattes[{F},{M},0.3,20]"], **TWO), out=OUT),
        exp("masks", "-x [mask,mask]", "-x", f"[{K},{K}]", *_stage(e, [MS], **TWO), out=OUT),
        exp("float", "--float 1", "--float", "1", *_stage(e, [MS], **TWO), out=OUT),
        exp("2d", "2D", *_stage(e, [f"MeanSquares[{F2},{M2},1]"], **TWO), d="2", out=OUT),
        exp("4d", "4D, MeanSquares", "-z", "0", *_stage("Exponential[0.5,2,0.5]", [f"MeanSquares[{F4},{M4},1]"], **ONE),
            d="4", out=PRE, tier="full"),
        exp("oblique", "oblique grids", *_stage(e, [f"MeanSquares[{FO},{MO},1]"], **ONE), out=OUT),
        exp("restrict", "-g 1x1x0", "-g", "1x1x0", *_stage(e, [MS], **TWO), out=OUT),
        exp("initial-field", "-r a displacement field", "-r", "{gen-syn:warp-3d.nii.gz}", *_stage(e, [MS], **ONE), out=OUT),
        exp("per-stage", "-i 1 -a 1 -z 0 after an affine stage",
            "-a", "1", "-z", "0", "-i", "1", *AFFINE, *_stage(e, [MS], **ONE), out=PRE),
        exp("then-affine", "an affine stage after the velocity field",
            *_stage(e, [MS], **ONE), *AFFINE, out=OUT, expect="divergence",
            reason="the affine stage after a displacement field samples its scales' 1000 voxels from the clock (ANTS-72): two ANTs runs differ"),
        exp("regular-sampling", "MeanSquares, regular sampling 0.5", *_stage(e, [f"MeanSquares[{F},{M},1,1,Regular,0.5]"], **ONE),
            out=PRE, expect="divergence", reason=CLOCK),
        exp("icp-second", "MeanSquares and ICP", "-x", PSK,
            *_stage(e, [f"MeanSquares[{PSI},{PSIM},1]", f"ICP[{PSF},{PSM},0.5]"], **ONE), out=PRE),
        exp("print-similarity", "--print-similarity-measure-interval: the observer's copy of the field fails (ANTS-75)",
            "--print-similarity-measure-interval", "1", *_stage(e, [MS], **ONE), out=PRE, expect="error"),
        exp("interval-volumes", "--write-interval-volumes: the observer's copy of the field fails (ANTS-75)",
            "--write-interval-volumes", "1", *_stage(e, [MS], **ONE), out=PRE, expect="error"),
        exp("save-state", "--save-state: no state (ANTS-50)", "-j", "{out}/state.h5", *_stage(e, [MS], **ONE),
            out=PRE, expect="error"),
        exp("zero-iterations", "no iteration and one level: the transform never integrates, its inverse has no field (ITK-41)",
            *_stage(e, [MS], c="[0,1e-6,3]", f="1", s="0"), out=OUT, expect="error"),
        exp("zero-iterations-prefix", "no iteration, no warped images: written as ANTs writes it (ITK-41)",
            *_stage(e, [MS], c="[0,1e-6,3]", f="1", s="0"), out=PRE),
        exp("icp-first", "ICP first: ANTs allocates the velocity field on a null image",
            "-x", PSK, *_stage(e, [f"ICP[{PSF},{PSM},1]"], **ONE), out=PRE, expect="crash"),
        exp("missing-parameter", "Exponential[0.5,3]: the velocity variance is required",
            *_stage("Exponential[0.5,3]", [MS], **ONE), out=PRE, expect="error"),
    ]  # fmt: skip


def _bspline_exponential(make) -> list:
    b = "BSplineExponential[0.5,4x4x4,0,0,3]"

    def bexp(name: str, description: str, *args: str, **kw):
        return make(f"{BEXP}/{name}", BEXP, description, *args, **kw)

    return [
        bexp("meansquares", "BSplineExponential (update mesh 4x4x4), MeanSquares, 2 levels, warped images",
             *_stage(b, [MS], **TWO), out=OUT, band=True),
        bexp("velocity-mesh", "a velocity-field mesh (2x2x2) and 5 integration steps",
             *_stage("BSplineExponential[0.5,4x4x4,2x2x2,5,3]", [MS], **TWO), out=OUT),
        bexp("knot-spacing", "a knot spacing (30 mm), -z 0", "-z", "0",
             *_stage("BSplineExponential[0.5,30,0,0,3]", [MS], **TWO), out=OUT),
        bexp("cc", "CC", *_stage(b, [CC], **TWO), out=OUT),
        bexp("float", "--float 1", "--float", "1", *_stage(b, [MS], **TWO), out=PRE),
        bexp("2d", "2D", *_stage("BSplineExponential[0.5,4x4,2x2,0,3]", [f"MeanSquares[{F2},{M2},1]"], **TWO), d="2", out=OUT),
        bexp("4d", "4D", *_stage("BSplineExponential[0.5,4x4x4x2,0,0,3]", [f"MeanSquares[{F4},{M4},1]"], **ONE),
             d="4", out=PRE, tier="full"),
        bexp("masks", "-x [mask,mask]", "-x", f"[{K},{K}]", *_stage(b, [MS], **ONE), out=PRE),
        bexp("order-2", "order 2", *_stage("BSplineExponential[0.5,4x4x4,0,0,2]", [MS], **ONE), out=PRE),
        bexp("mesh-dimension", "a 2D mesh in 3D: ERROR", *_stage("BSplineExponential[0.5,4x4,0,0,3]", [MS], **ONE),
             out=PRE, expect="error"),
        bexp("random-sampling", "Mattes, random sampling 0.5", *_stage(b, [f"Mattes[{F},{M},1,24,Random,0.5]"], **ONE),
             out=PRE, expect="divergence", reason=CLOCK),
    ]  # fmt: skip


def _tvf(make) -> list:
    t = "TimeVaryingVelocityField[0.5,3,3,0,0.5,0]"

    def tvf(name: str, description: str, *args: str, **kw):
        return make(f"{TVF}/{name}", TVF, description, *args, **kw)

    return [
        tvf("meansquares", "TimeVaryingVelocityField (3 time indices), MeanSquares, 2 levels, -z 0: Warp, InverseWarp, VelocityField",
            "-z", "0", *_stage(t, [MS], **TWO), out=OUT, band=True),
        tvf("short-name", "tvf[...] with time variances (0.5, 0.25)", "-z", "0",
            *_stage("tvf[0.5,3,3,0.5,0.5,0.25]", [MS], **ONE), out=OUT),
        tvf("collapse", "an affine stage, then the velocity field: collapsed", *AFFINE, *_stage(t, [MS], **ONE), out=OUT),
        tvf("composite", "-a 1 after an affine stage", "-a", "1", *AFFINE, *_stage(t, [MS], **ONE), out=OUT),
        tvf("cc", "CC", *_stage(t, [CC], **ONE), out=OUT),
        tvf("mattes", "Mattes (24 bins)", *_stage(t, [f"Mattes[{F},{M},1,24]"], **ONE), out=PRE),
        tvf("multi", "CC (0.7) and Mattes (0.3)", *_stage("TimeVaryingVelocityField[0.5,3,3,0,0,0]", [f"CC[{F},{M},0.7,2]", f"Mattes[{F},{M},0.3,20]"], **ONE), out=OUT),
        tvf("masks", "-x [mask,mask]: the masks given to the metric unchanged (ITK-40)", "-x", f"[{K},{K}]",
            *_stage(t, [MS], **ONE), out=PRE),
        tvf("float", "--float 1", "--float", "1", *_stage(t, [MS], **TWO), out=OUT),
        tvf("2d", "2D, 4 time indices", "-z", "0", *_stage("TimeVaryingVelocityField[0.5,4,3,0,0.5,0]", [f"MeanSquares[{F2},{M2},1]"], **TWO), d="2", out=OUT),
        tvf("4d", "4D: the velocity field (5D) cannot be written as NIfTI", "-z", "0",
            *_stage("TimeVaryingVelocityField[0.5,2,2,0,0,0]", [f"MeanSquares[{F4},{M4},1]"], c="[2,1e-6,3]", f="1", s="0"),
            d="4", out=PRE, tier="full"),
        tvf("one-time-index", "one time index", *_stage("TimeVaryingVelocityField[0.5,1,3,0,0.5,0]", [MS], **ONE), out=OUT),
        tvf("restrict", "-g 1x0x1", "-g", "1x0x1", *_stage(t, [MS], **ONE), out=OUT),
        tvf("two-stages", "two TimeVaryingVelocityField stages: the number of time indices from the last -t (ANTS-79)", "-z", "0",
            *_stage("TimeVaryingVelocityField[0.5,2,3,0,0,0]", [MS], **ONE),
            *_stage("TimeVaryingVelocityField[0.5,3,3,0,0,0]", [MS], **ONE), out=PRE),
        tvf("then-syn", "a SyN stage after the velocity field", *_stage("TimeVaryingVelocityField[0.5,2,3,0,0,0]", [MS], **ONE),
            *_stage("SyN[0.25,3,0]", [MS], **ONE), out=OUT),
        tvf("per-stage", "-i 1 -a 1 -z 0 after an affine stage", "-a", "1", "-z", "0", "-i", "1",
            *AFFINE, *_stage(t, [MS], **ONE), out=PRE),
        tvf("fixed-initial", "-q an affine transform: ignored by the method (ITK-39)", "-q", "{gen-transform:initial.mat}",
            *_stage("TimeVaryingVelocityField[0.5,2,3,0,0,0]", [MS], **ONE), out=OUT),
        tvf("print-similarity", "--print-similarity-measure-interval: no full-scale similarity in this observer",
            "--print-similarity-measure-interval", "1", *_stage(t, [MS], **ONE), out=PRE),
        tvf("icp-second", "CC and ICP", "-x", PSK,
            *_stage("TimeVaryingVelocityField[0.5,2,3,0,0,0]", [f"CC[{PSI},{PSIM},1,2]", f"ICP[{PSF},{PSM},0.5]"], **ONE), out=PRE),
        tvf("regular-sampling", "MeanSquares, regular sampling 0.5", *_stage(t, [f"MeanSquares[{F},{M},1,1,Regular,0.5]"], **ONE),
            out=PRE, expect="divergence", reason=CLOCK),
        tvf("zero-iterations-prefix", "no iteration: the transform never integrates; no warped images (ITK-41)",
            *_stage(t, [MS], c="[0,1e-6,3]", f="1", s="0"), out=PRE),
        tvf("zero-iterations", "no iteration and warped images: ANTs maps points through a field without an interpolator input (ITK-41)",
            *_stage(t, [MS], c="[0,1e-6,3]", f="1", s="0"), out=OUT, expect="crash"),
        tvf("zero-time-indices", "0 time indices: ANTs divides by 0", *_stage("TimeVaryingVelocityField[0.5,0,3,0,0,0]", [MS], **ONE),
            out=PRE, expect="crash"),
        tvf("icp-first", "ICP first: ANTs allocates the velocity field on a null image",
            "-x", PSK, *_stage(t, [f"ICP[{PSF},{PSM},1]"], **ONE), out=PRE, expect="crash"),
        tvf("missing-parameter", "TimeVaryingVelocityField[0.5,2,3]: the other variances are required",
            *_stage("TimeVaryingVelocityField[0.5,2,3]", [MS], **ONE), out=PRE, expect="error"),
    ]  # fmt: skip


def _tvb(make) -> list:
    t = "TimeVaryingBSplineVelocityField[0.5,4x4x4x2,4,3]"
    one = {"c": "[2,1e-6,3]", "f": "1", "s": "0"}
    two = {"c": "[2x2,1e-6,3]", "f": "2x1", "s": "1x0vox"}

    def tvb(name: str, description: str, *args: str, **kw):
        return make(f"{TVB}/{name}", TVB, description, *args, **kw)

    return [
        tvb("meansquares", "TimeVaryingBSplineVelocityField (mesh 4x4x4x2, 4 time samples), MeanSquares, 2 levels, warped images",
            *_stage(t, [MS], **two), out=OUT, band=True),
        tvb("float", "--float 1", "--float", "1", *_stage(t, [MS], **two), out=PRE),
        tvb("2d", "2D", *_stage("TimeVaryingBSplineVelocityField[0.5,4x4x2,4,3]", [f"MeanSquares[{F2},{M2},1]"], **two), d="2", out=OUT),
        tvb("4d", "4D", *_stage("TimeVaryingBSplineVelocityField[0.5,3x3x3x2x2,2,3]", [f"MeanSquares[{F4},{M4},1]"], c="[1,1e-6,3]", f="1", s="0"),
            d="4", out=PRE, tier="full"),
        tvb("masks", "-x [mask,mask]", "-x", f"[{K},{K}]", *_stage(t, [MS], **one), out=PRE),
        tvb("fixed-mask-oblique", "a fixed mask on an oblique grid", "-x", KO, *_stage(t, [MS], **one), out=PRE),
        tvb("cc", "CC", *_stage(t, [CC], **one), out=PRE),
        tvb("multi", "CC (0.7) and Mattes (0.3)", *_stage(t, [f"CC[{F},{M},0.7,2]", f"Mattes[{F},{M},0.3,20]"], **one), out=PRE),
        tvb("order-2", "order 2", *_stage("TimeVaryingBSplineVelocityField[0.5,4x4x4x2,4,2]", [MS], **one), out=PRE),
        tvb("samples-2", "2 time samples, mesh 3x3x3x1", *_stage("TimeVaryingBSplineVelocityField[0.5,3x3x3x1,2,3]", [MS], **one), out=PRE),
        tvb("samples-1", "1 time sample", *_stage("TimeVaryingBSplineVelocityField[0.5,3x3x3x1,1,3]", [MS], **one), out=PRE),
        tvb("oblique", "oblique grids", *_stage(t, [f"MeanSquares[{FO},{MO},1]"], **one), out=PRE),
        tvb("restrict", "-g 1x1x0", "-g", "1x1x0", *_stage(t, [MS], **one), out=PRE),
        tvb("interval-volumes", "--write-interval-volumes: not written by this observer", "--write-interval-volumes", "1",
            *_stage("TimeVaryingBSplineVelocityField[0.5,3x3x3x1,2,3]", [MS], **one), out=PRE),
        tvb("per-stage", "-i 1 -a 1 -z 0 after an affine stage", "-a", "1", "-z", "0", "-i", "1",
            *AFFINE, *_stage("TimeVaryingBSplineVelocityField[0.5,3x3x3x1,2,3]", [MS], **one), out=PRE),
        tvb("zero-iterations", "no iteration on two levels", *_stage("TimeVaryingBSplineVelocityField[0.5,3x3x3x1,2,3]", [MS], c="[0x1,1e-6,3]", f="2x1", s="0x0"), out=OUT),
        tvb("icp", "ICP alone (identity-direction phantoms): the points fitted directly", "-x", LK,
            *_stage(t, [f"ICP[{LF},{LM},1]"], **two), out=PRE),
        tvb("pse", "PSE alone", "-x", LK, *_stage(t, [f"PSE[{LF},{LM},1,1,0,2,5]"], **one), out=PRE),
        tvb("jhct", "JHCT alone", "-x", LK, *_stage(t, [f"JHCT[{LF},{LM},1,1,0,2,5,1.1,0]"], **one), out=PRE),
        tvb("igdm", "IGDM alone", "-x", LK, *_stage(t, [f"IGDM[{LI},{LIM},1,{LK},{LKM}]"], **one), out=PRE),
        tvb("cc-and-icp", "CC and ICP: the image metric warped twice (ITK-36)", "-x", LK,
            *_stage(t, [f"CC[{LI},{LIM},1,2]", f"ICP[{LF},{LM},0.5]"], **one), out=PRE),
        tvb("meansquares-icp-cc", "MeanSquares, ICP and CC (ITK-36, ITK-37)", "-x", LK,
            *_stage(t, [f"MeanSquares[{LI},{LIM},1]", f"ICP[{LF},{LM},0.5]", f"CC[{LI},{LIM},1,2]"], **one), out=PRE),
        tvb("icp-ras", "ICP alone on RAS-stored phantoms: ANTs fits the points outside the parametric domain and throws (ITK-38)",
            "-x", PSK, *_stage(t, [f"ICP[{PSF},{PSM},1]"], **two), out=PRE, expect="error"),
        tvb("icp-then-cc", "ICP first, then CC: ANTs cannot convert the multi-metric queue",
            "-x", LK, *_stage(t, [f"ICP[{LF},{LM},0.5]", f"CC[{LI},{LIM},1,2]"], **one), out=PRE, expect="error"),
        tvb("mesh-dimension", "a mesh without its time size: the message, then failure",
            *_stage("TimeVaryingBSplineVelocityField[0.5,4x4x4,4,3]", [MS], **one), out=PRE, expect="error"),
        tvb("mesh-0", "a mesh size of 0: ANTs divides by zero and fails on a parametric domain of no span (ANTS-77)",
            *_stage("TimeVaryingBSplineVelocityField[0.5,3x0x3x1,2,3]", [MS], **one), out=PRE, expect="divergence",
            reason="ANTs computes control-point spacings of infinity, prints vnl's SVD warnings and fails with an exception on a parametric domain of no span (ANTS-77); larmorx stops with an error after the same messages"),
    ]  # fmt: skip


def _fixes(make) -> list:
    """The paths of the corrections: reproduce compared with ANTs; the fix pass reports what
    the correction changes."""
    igdm = f"IGDM[{PSI},{PSIM},1,{PSK},{PSKM}"

    def fix(name: str, description: str, *args: str, **kw):
        return make(f"{FIX}/{name}", FIX, description, *args, **kw)

    return [
        fix("igdm-k-neighbourhood", "IGDM with kNeighborhood 5 and gradientSigma 1.5, given sigmas (ANTS-67, ANTS-68)",
            "-x", PSK, *_stage("Rigid[0.1]", [f"{igdm},0x0x0,50,5,5,1.5]"], c="[20,1e-8,10]", f="1", s="0"), out=PRE),
        fix("igdm-true-gradient", "IGDM, given sigmas, one neighbour, central differences: the derivative alone (ANTS-68)",
            "-x", PSK, *_stage("Rigid[0.1]", [f"{igdm},0x0x0,50,5,1,0]"], c="[20,1e-8,10]", f="1", s="0"), out=PRE),
        fix("igdm-same-sets", "IGDM of an image with itself moved by half a voxel: every nearest distance 1 mm, the distance sigma 0, NaN parameters, the stage dropped (ANTS-70)",
            "-x", PSK, "-r", "{gen-transform:half-voxel-x.txt}",
            *_stage("Rigid[0.1]", [f"IGDM[{PSI},{PSI},1,{PSK},{PSK}]"], c="[10,1e-8,10]", f="1", s="0"), out=PRE),
        fix("jhct-isotropic-sigma-2", "JHCT, isotropic, sigma 2: the total number of points alone (ITK-24)",
            "-x", PSK, *_stage("Rigid[0.1]", [f"JHCT[{PSF},{PSM},1,1,0,2,50,1.1,0]"], c="[20,1e-8,10]", f="1", s="0"), out=PRE),
        fix("jhct-anisotropic", "JHCT, anisotropic: the Parzen kernel sigma (ITK-29, ITK-24, ITK-26)",
            "-x", PSK, *_stage("Rigid[0.1]", [f"JHCT[{PSF},{PSM},1,1,0,1,50,1.1,1]"], c="[20,1e-8,10]", f="1", s="0"), out=PRE),
        fix("bspline-then-affine", "an affine stage after a BSpline stage, with a mask (ITK-35; ANTs drops it, ANTS-49)",
            "-x", K, *_stage("BSpline[0.2,2x2x2]", [MS], **ONE), *_stage("Affine[0.1]", [MS], c="[20,1e-8,10]", f="1", s="0vox"),
            out=WARPED),
        fix("bspline-then-bspline", "two BSpline stages (ITK-35; ANTs fails)",
            "-x", K, *_stage("BSpline[0.2,2x2x2]", [MS], **ONE), *_stage("BSpline[0.2,1x1x1]", [MS], c="[20,1e-6,5]", f="1", s="0"),
            out=PRE, expect="error"),
        fix("initial-bspline-then-rigid", "-r a B-spline transform, then Rigid, with a mask (ITK-35)",
            "-x", K, "-r", "{gen-bsp:bspline-3d.txt}", *_stage("Rigid[0.1]", [MS], c="[20,1e-8,10]", f="1", s="0vox"), out=WARPED),
        fix("bspline-field-coarse", "BSplineDisplacementField 3x3x3 over 2 levels (ANTS-78; corrected, the coarse level needs ITK-42)",
            *_stage("BSplineDisplacementField[0.3,3x3x3,0,3]", [MS], c="[5x3,1e-6,5]", f="2x1", s="1x0vox"), out=WARPED),
    ]  # fmt: skip
