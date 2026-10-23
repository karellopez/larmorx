# SPDX-License-Identifier: Apache-2.0
"""Parity of ``larmorx ants antsAI`` with ANTs 2.6.5's ``antsAI``.

ANTs runs as its own binary with one thread; larmorx runs in-process (its Python console entry
point) with 4 threads, which must not matter, with ANTs' defects reproduced
(``LARMORX_UPSTREAM_DEFECTS=reproduce``, D13). ``antsAI`` seeds its sampling with 1234 unless
told otherwise, so most cases are repeatable without a seed. Compared:

- the **exit status**;
- **standard output** (``-v 1``: the metric, the number of start points, every printed metric
  value, the best one), after masking object addresses and the output directories;
- **standard error**, larmorx's notice lines removed;
- **every file** each program writes: transform files byte for byte (``.mat``, ``.txt``), HDF5
  transform files dataset by dataset (``/OSName`` and ``/OSVersion`` aside).

Where the first comparison is not identical, ANTs runs again with CORE-MATH's correctly rounded
functions preloaded (``scripts/build_crmath_preload.sh``): larmorx must then be identical.

Cases where ANTs aborts (an exception reaches ``std::terminate``) or crashes compare everything
ANTs prints before it stops; larmorx then adds one line and exits 1. Cases where ANTs never
finishes run it for a few seconds only. Cases where ANTs is not repeatable (``Random``
sampling and seed 0 draw from the clock) run ANTs twice and report how far apart its own runs
are, next to larmorx's distance from them. The **thread band** runs ANTs at 2, 4 and 8
threads. A **fix pass** runs larmorx in its default mode and reports the defects it names.

The real cases repeat fMRIPrep 25.2.6's own ``antsAI`` command lines from the oracle runs
(``oracles/fmriprep-25.2.6``: sMRIPrep's brain extraction and the BOLD reference's
skull-stripping), with fMRIPrep's seed, and also report the distance to the transform
fMRIPrep wrote (with 8 threads).
"""

from __future__ import annotations

import os
import re
import subprocess
import tempfile
from dataclasses import dataclass, replace
from pathlib import Path

import numpy as np

from larmorx_validation.parity import ants_ai_blobs, ants_inputs, defects
from larmorx_validation.parity import ants_registration as reg
from larmorx_validation.parity.ants_programs import ants_bin, ants_version, crmath_preload
from larmorx_validation.parity.harness import (
    BOTH_ERROR,
    EXPECTED,
    SKIPPED,
    Case,
    CaseResult,
    CheckList,
    Outcome,
    Suite,
)

PROGRAM = "antsAI"
LX_THREADS = 4
#: ANTs' thread band: 2, 4 and 8 threads unless ``LARMORX_PARITY_BAND_THREADS`` (e.g.
#: ``2,4,6`` when the machine is shared) says otherwise.
BAND_THREADS = tuple(
    int(v) for v in os.environ.get("LARMORX_PARITY_BAND_THREADS", "2,4,8").split(",") if v
)
_TIERS = ("smoke", "standard", "full")
GLIBC = reg.GLIBC
#: How long a case whose ANTs never finishes lets it run.
HANG_SECONDS = 20


@dataclass(frozen=True)
class AiCase:
    """One command line of ``program``, compared with larmorx's.

    ``args`` use placeholders: ``{out}`` the program's output directory, ``{gen:name}`` a
    generated input, ``{missing:name}`` a file that does not exist, ``{tf:path}`` a
    TemplateFlow file of ``larmorx-testdata``, ``{oracle:run/path}`` a file of an fMRIPrep
    oracle run, ``{fmriprep:run/node}`` (alone) the command line of an fMRIPrep node.
    ``expect``:

    - ``"pass"``: identical (with CORE-MATH where glibc's maths differ);
    - ``"error"``: both exit with the same status and messages;
    - ``"silent"``: both exit with the same status; ANTs says nothing (without ``-v``) and
      larmorx explains (ANTS-45);
    - ``"abort"``: ANTs aborts or crashes (a signal); larmorx prints what ANTs printed, adds
      one line and exits 1;
    - ``"hang"``: ANTs never finishes; larmorx refuses;
    - ``"random"``: ANTs draws from the clock, so its runs differ;
    - ``"divergence"``: a documented difference (``reason``).

    ``seed`` is ``ANTS_RANDOM_SEED`` (``None``: unset). ``band`` runs ANTs' thread band;
    ``fmriprep`` names the transform fMRIPrep wrote for a real case; ``truth`` names the known
    transform of a blob case (``phantom:<move>``, ``t1w:shift``, ``t1w:rotated``), against which
    ANTs' and larmorx's transforms are scored.
    """

    id: str
    category: str
    description: str
    args: tuple[str, ...]
    tier: str = "standard"
    expect: str = "pass"
    reason: str = ""
    seed: str | None = None
    band: bool = False
    program: str = PROGRAM
    fmriprep: str = ""
    truth: str = ""
    #: Defects the fix pass may name without a change in the written transform (the blob
    #: pairs change, the fitted transform does not).
    silent_ok: tuple[str, ...] = ()


# ------------------------------------------------------------------------------------------------
# Generated inputs (besides antsRegistration's phantoms)


@ants_inputs.builder("reg-zero")
def _(t):
    """An image of zeros on the fixed phantom's grid (its total mass is 0)."""
    reg._save(np.zeros((40, 44, 36)), reg._affine(2.0, (-40, -44, -36)), t)


@ants_inputs.builder("reg-zero-2d")
def _(t):
    """A 2D image of zeros on the 2D fixed phantom's grid."""
    a = np.diag([2.0, 2.0, 1.0, 1.0])
    a[:2, 3] = (-40, -44)
    reg._save(np.zeros((40, 44)), a, t)


@ants_inputs.builder("reg-mask-empty")
def _(t):
    """A mask of zeros on the fixed phantom's grid (uint8)."""
    reg._save(np.zeros((40, 44, 36), dtype=np.uint8), reg._affine(2.0, (-40, -44, -36)), t)


# ------------------------------------------------------------------------------------------------
# Cases

F, M, K = reg.F, reg.M, reg.K
F2, M2 = reg.F2, reg.M2
F4, M4 = reg.F4, reg.M4
FO, MO = reg.FO, reg.MO
Z, E = "{gen:reg-zero}", "{gen:reg-mask-empty}"
S = ("-s", "[90,0.5]")
C = ("-c", "[3,1e-6,5]")
O_MAT = ("-o", "{out}/t.mat")
V = ("-v", "1")


def mattes(fixed: str = F, moving: str = M, rest: str = "32,Regular,0.25") -> str:
    return f"Mattes[{fixed},{moving},{rest}]"


def ai(cid: str, category: str, description: str, *args: str, **kw) -> AiCase:
    return AiCase(cid, category, description, tuple(args), **kw)


def _search_cases() -> list[AiCase]:
    d3 = ("-d", "3")
    d2 = ("-d", "2")
    return [
        ai(
            "initial/rigid",
            "initial",
            "Rigid, no search: the centres of mass",
            *d3,
            "-m",
            mattes(rest="32"),
            "-t",
            "Rigid[0.1]",
            *O_MAT,
            *V,
            tier="smoke",
        ),
        ai(
            "initial/rigid-principal-axes",
            "initial",
            "Rigid from the principal axes (Wahba)",
            *d3,
            "-m",
            mattes(rest="32"),
            "-t",
            "Rigid[0.1]",
            "-p",
            "1",
            *O_MAT,
            *V,
        ),
        ai(
            "initial/affine-principal-axes",
            "initial",
            "Affine from the principal axes",
            *d3,
            "-m",
            mattes(rest="32"),
            "-t",
            "Affine[0.1]",
            "-p",
            "1",
            *O_MAT,
            *V,
        ),
        ai(
            "initial/similarity-principal-axes",
            "initial",
            "Similarity from the principal axes (Similarity3D's matrix: cbrt)",
            *d3,
            "-m",
            mattes(rest="32"),
            "-t",
            "Similarity[0.1]",
            "-p",
            "1",
            *O_MAT,
            *V,
        ),
        ai(
            "initial/geometric-centers",
            "initial",
            "AlignGeometricCenters",
            *d3,
            "-m",
            mattes(rest="32"),
            "-t",
            "AlignGeometricCenters",
            *O_MAT,
            *V,
        ),
        ai(
            "initial/centers-of-mass",
            "initial",
            "AlignCentersOfMass",
            *d3,
            "-m",
            mattes(rest="32"),
            "-t",
            "AlignCentersOfMass",
            *O_MAT,
            *V,
        ),
        ai(
            "initial/2d-rigid-principal-axes",
            "initial",
            "2D Rigid from the principal axes",
            *d2,
            "-m",
            mattes(F2, M2, "32"),
            "-t",
            "Rigid[0.1]",
            "-p",
            "1",
            *O_MAT,
            *V,
        ),
        ai(
            "initial/2d-centers-of-mass-txt",
            "initial",
            "2D AlignCentersOfMass, a text file",
            *d2,
            "-m",
            mattes(F2, M2, "32"),
            "-t",
            "AlignCentersOfMass",
            "-o",
            "{out}/t.txt",
            *V,
        ),
        ai(
            "initial/h5",
            "initial",
            "Affine from the principal axes into an HDF5 file",
            *d3,
            "-m",
            mattes(rest="32"),
            "-t",
            "Affine[0.1]",
            "-p",
            "1",
            "-o",
            "{out}/t.h5",
            *V,
        ),
        ai(
            "search/rigid-grid",
            "search",
            "Rigid search with a translation grid",
            *d3,
            "-m",
            mattes(),
            "-t",
            "Rigid[0.1]",
            *S,
            *C,
            "-g",
            "[10,10x0x10]",
            *O_MAT,
            *V,
            tier="smoke",
            band=True,
        ),
        ai(
            "search/similarity",
            "search",
            "Similarity search",
            *d3,
            "-m",
            mattes(),
            "-t",
            "Similarity[0.1]",
            *S,
            *C,
            *O_MAT,
            *V,
        ),
        ai(
            "search/affine-mi",
            "search",
            "Affine search, joint-histogram MI",
            *d3,
            "-m",
            f"MI[{F},{M},32,Regular,0.25]",
            "-t",
            "Affine[0.1]",
            *S,
            *C,
            *O_MAT,
            *V,
            band=True,
        ),
        ai(
            "search/rigid-gc",
            "search",
            "Rigid search from the principal axes, GC",
            *d3,
            "-m",
            f"GC[{F},{M},32,Regular,0.25]",
            "-t",
            "Rigid[0.2]",
            "-p",
            "1",
            *S,
            *C,
            *O_MAT,
            *V,
            band=True,
        ),
        ai(
            "search/affine-oblique",
            "search",
            "Affine search on oblique grids (1.5 x 1.2 x 2 mm)",
            *d3,
            "-m",
            mattes(FO, MO, "32,Regular,0.3"),
            "-t",
            "Affine[0.1]",
            "-p",
            "1",
            *S,
            *C,
            *O_MAT,
            *V,
        ),
        ai(
            "search/masks",
            "search",
            "Affine search with fixed and moving masks",
            *d3,
            "-m",
            mattes(),
            "-x",
            f"[{K},{K}]",
            "-t",
            "Affine[0.1]",
            *S,
            *C,
            *O_MAT,
            *V,
        ),
        ai(
            "search/fixed-mask",
            "search",
            "Rigid search with a fixed mask",
            *d3,
            "-m",
            mattes(),
            "-x",
            K,
            "-t",
            "Rigid[0.1]",
            *S,
            "-c",
            "[2,1e-6,5]",
            *O_MAT,
            *V,
        ),
        ai(
            "search/missing-mask",
            "search",
            "A mask that cannot be read is no mask",
            *d3,
            "-m",
            mattes(),
            "-x",
            "{missing:mask.nii.gz}",
            "-t",
            "Rigid[0.1]",
            *S,
            "-c",
            "[2,1e-6,5]",
            *O_MAT,
            *V,
        ),
        ai(
            "search/dense",
            "search",
            "Rigid search over every voxel (no sampling)",
            *d3,
            "-m",
            mattes(rest="32,None"),
            "-t",
            "Rigid[0.1]",
            "-s",
            "[180,0.5]",
            "-c",
            "[2,1e-6,5]",
            *O_MAT,
            *V,
        ),
        ai(
            "search/gc-dense",
            "search",
            "GC over every voxel",
            *d3,
            "-m",
            f"GC[{F},{M},1,None]",
            "-t",
            "Rigid[0.1]",
            "-s",
            "[180,0.5]",
            "-c",
            "[2,1e-6,5]",
            *O_MAT,
            *V,
        ),
        ai(
            "search/2d-affine-grid",
            "search",
            "2D Affine search with a translation grid",
            *d2,
            "-m",
            mattes(F2, M2, "32,Regular,0.5"),
            "-t",
            "Affine[0.1]",
            "-s",
            "[30,0.5]",
            "-c",
            "[5,1e-6,5]",
            "-g",
            "[5,5x5]",
            *O_MAT,
            *V,
            band=True,
        ),
        ai(
            "search/2d-rigid-grid",
            "search",
            "2D Rigid with a translation grid (ANTS-52: the grid is ignored)",
            *d2,
            "-m",
            mattes(F2, M2, "32,Regular,0.5"),
            "-t",
            "Rigid[0.1]",
            "-s",
            "[90,0.5]",
            "-c",
            "[2,1e-6,5]",
            "-g",
            "[5,5x5]",
            *O_MAT,
            *V,
        ),
        ai(
            "search/2d-rigid",
            "search",
            "2D Rigid from the principal axes",
            *d2,
            "-m",
            mattes(F2, M2, "32"),
            "-t",
            "Rigid[0.1]",
            "-p",
            "1",
            "-s",
            "[30,0.5]",
            "-c",
            "[5,1e-6,5]",
            *O_MAT,
            *V,
        ),
        ai(
            "search/2d-similarity-gc",
            "search",
            "2D Similarity, GC over every pixel",
            *d2,
            "-m",
            f"GC[{F2},{M2},32]",
            "-t",
            "Similarity[0.1]",
            "-s",
            "[30,0.5]",
            "-c",
            "[5,1e-6,5]",
            *O_MAT,
            *V,
        ),
        ai(
            "search/2d-mi-dense",
            "search",
            "2D Affine, MI over every pixel",
            *d2,
            "-m",
            f"MI[{F2},{M2},16]",
            "-t",
            "Affine[0.1]",
            "-s",
            "[45,1]",
            "-c",
            "[3,1e-6,5]",
            *O_MAT,
            *V,
        ),
        ai(
            "search/similarity-grid",
            "search",
            "3D Similarity with a translation grid (ANTS-52: the grid is ignored)",
            *d3,
            "-m",
            mattes(),
            "-t",
            "Similarity[0.1]",
            *S,
            "-c",
            "[2,1e-6,5]",
            "-g",
            "[10,10x0x0]",
            *O_MAT,
            *V,
        ),
        ai(
            "search/grid-two-extents",
            "search",
            "A grid of two extents in 3D (ANTS-55: the third is read past the end)",
            *d3,
            "-m",
            mattes(),
            "-t",
            "Affine[0.1]",
            *S,
            "-c",
            "[2,1e-6,5]",
            "-g",
            "[10,10x10]",
            *O_MAT,
            *V,
        ),
        ai(
            "search/grid-one-extent",
            "search",
            "A grid of one extent in 3D (ANTS-55)",
            *d3,
            "-m",
            mattes(),
            "-t",
            "Affine[0.1]",
            *S,
            "-c",
            "[2,1e-6,5]",
            "-g",
            "[10,10]",
            *O_MAT,
            *V,
        ),
        ai(
            "search/arc-zero",
            "search",
            "An arc fraction of 0: one angle per axis",
            *d3,
            "-m",
            mattes(),
            "-t",
            "Rigid[0.1]",
            "-s",
            "[90,0]",
            "-c",
            "[2,1e-6,5]",
            *O_MAT,
            *V,
        ),
        ai(
            "search/arc-wide",
            "search",
            "An arc fraction of 3 (beyond a full turn)",
            *d2,
            "-m",
            mattes(F2, M2, "32,Regular,0.5"),
            "-t",
            "Rigid[0.1]",
            "-s",
            "[90,3]",
            "-c",
            "[2,1e-6,5]",
            *O_MAT,
            *V,
        ),
        ai(
            "search/seed",
            "search",
            "--random-seed 7",
            *d3,
            "-m",
            mattes(),
            "-t",
            "Rigid[0.1]",
            *S,
            "-c",
            "[2,1e-6,5]",
            "--random-seed",
            "7",
            *O_MAT,
            *V,
        ),
        ai(
            "search/env-seed",
            "search",
            "ANTS_RANDOM_SEED=99",
            *d3,
            "-m",
            mattes(),
            "-t",
            "Rigid[0.1]",
            *S,
            "-c",
            "[2,1e-6,5]",
            *O_MAT,
            *V,
            seed="99",
        ),
        ai(
            "search/env-seed-prefix",
            "search",
            "ANTS_RANDOM_SEED=12abc (std::stoi reads 12)",
            *d3,
            "-m",
            mattes(),
            "-t",
            "Rigid[0.1]",
            *S,
            "-c",
            "[2,1e-6,5]",
            *O_MAT,
            *V,
            seed="12abc",
        ),
        ai(
            "search/percentage-above-one",
            "search",
            "Regular sampling of 5 (every voxel)",
            *d3,
            "-m",
            mattes(rest="32,Regular,5"),
            "-t",
            "Rigid[0.1]",
            *S,
            "-c",
            "[2,1e-6,5]",
            *O_MAT,
            *V,
        ),
        ai(
            "search/quiet",
            "search",
            "Without -v: nothing printed",
            *d3,
            "-m",
            mattes(),
            "-t",
            "Rigid[0.1]",
            *S,
            "-c",
            "[2,1e-6,5]",
            *O_MAT,
        ),
        ai(
            "search/txt",
            "search",
            "A text transform file",
            *d3,
            "-m",
            mattes(),
            "-t",
            "Rigid[0.1]",
            *S,
            "-c",
            "[2,1e-6,5]",
            "-o",
            "{out}/t.txt",
            *V,
        ),
        ai(
            "search/h5",
            "search",
            "An HDF5 transform file (no compression after a search)",
            *d3,
            "-m",
            mattes(),
            "-t",
            "Rigid[0.1]",
            *S,
            "-c",
            "[2,1e-6,5]",
            "-o",
            "{out}/t.h5",
            *V,
        ),
        ai(
            "search/plain-values",
            "search",
            "-s 90 and -c 2 without brackets",
            *d3,
            "-m",
            mattes(),
            "-t",
            "Rigid[0.1]",
            "-s",
            "90",
            "-c",
            "2",
            *O_MAT,
            *V,
        ),
        ai(
            "search/two-outputs",
            "search",
            "Two -o: the last is written",
            *d3,
            "-m",
            mattes(rest="32"),
            "-t",
            "Rigid[0.1]",
            "-o",
            "{out}/a.mat",
            "-o",
            "{out}/b.mat",
            *V,
        ),
        ai(
            "search/verbose-last",
            "search",
            "-v 1 -v 0: the last one counts",
            *d3,
            "-m",
            mattes(rest="32"),
            "-t",
            "Rigid[0.1]",
            "-v",
            "1",
            "-v",
            "0",
            *O_MAT,
        ),
        ai(
            "search/unknown-metric-initial",
            "search",
            "An unknown metric with 0 iterations: the metric is not used",
            *d3,
            "-m",
            f"Foo[{F},{M}]",
            "-t",
            "Rigid[0.1]",
            *O_MAT,
            *V,
        ),
        ai(
            "search/empty-start-list-no-output",
            "search",
            "A negative arc fraction without -o: no start point, nothing written",
            *d3,
            "-m",
            mattes(),
            "-t",
            "Rigid[0.1]",
            "-s",
            "[90,-0.5]",
            "-c",
            "[2,1e-6,5]",
            *V,
        ),
        ai(
            "search/align-with-iterations",
            "search",
            "AlignGeometricCenters with iterations (ANTS-53: an empty .mat)",
            *d3,
            "-m",
            mattes(rest="32"),
            "-t",
            "AlignGeometricCenters",
            "-c",
            "3",
            *O_MAT,
            *V,
        ),
        ai(
            "search/align-with-iterations-quiet",
            "search",
            "AlignGeometricCenters with iterations, without -v (ANTS-53)",
            *d3,
            "-m",
            mattes(rest="32"),
            "-t",
            "AlignGeometricCenters",
            "-c",
            "3",
            *O_MAT,
        ),
        ai(
            "search/align-with-iterations-2d-gc",
            "search",
            "2D AlignCentersOfMass with iterations, GC (ANTS-53)",
            *d2,
            "-m",
            f"GC[{F2},{M2}]",
            "-t",
            "AlignCentersOfMass",
            "-c",
            "3",
            "-s",
            "[30,0.5]",
            *O_MAT,
            *V,
        ),
        ai(
            "search/align-with-iterations-no-start",
            "search",
            "AlignGeometricCenters with iterations and a negative arc (ANTS-53)",
            *d3,
            "-m",
            mattes(rest="32"),
            "-t",
            "AlignGeometricCenters",
            "-c",
            "3",
            "-s",
            "[20,-1]",
            *O_MAT,
            *V,
        ),
        ai(
            "search/gc-empty-mask-dense",
            "search",
            "GC over every voxel with an empty fixed mask: no valid point, the maximum value",
            *d3,
            "-m",
            f"GC[{F},{M},32,None]",
            "-x",
            E,
            "-t",
            "Rigid[0.1]",
            *S,
            "-c",
            "[2,1e-6,5]",
            *O_MAT,
            *V,
        ),
        ai(
            "search/mi-empty-mask-dense",
            "search",
            "MI over every voxel with an empty fixed mask (ITK-19: every start point keeps the "
            "first value)",
            *d3,
            "-m",
            f"MI[{F},{M},32,None]",
            "-x",
            E,
            "-t",
            "Rigid[0.1]",
            *S,
            "-c",
            "[2,1e-6,5]",
            *O_MAT,
            *V,
        ),
    ]


def _message_cases() -> list[AiCase]:
    d3 = ("-d", "3")
    t = ("-t", "Rigid[0.1]")
    return [
        ai(
            "messages/no-arguments",
            "messages",
            "No arguments: the short menu, exit 1",
            tier="smoke",
            expect="error",
        ),
        ai("messages/h", "messages", "-h: the short menu", "-h"),
        ai("messages/help", "messages", "--help: the long menu", "--help"),
        ai(
            "messages/help-0",
            "messages",
            "--help 0: nothing to do (ANTS-45)",
            "--help",
            "0",
            expect="silent",
        ),
        ai("messages/h-0", "messages", "-h 0 (ANTS-45)", "-h", "0", expect="silent"),
        ai("messages/no-dimension", "messages", "No -d", "-m", mattes(), *t, *V, expect="error"),
        ai(
            "messages/dimension-4",
            "messages",
            "-d 4",
            "-d",
            "4",
            "-m",
            mattes(),
            *t,
            *V,
            expect="error",
        ),
        ai(
            "messages/dimension-1",
            "messages",
            "-d 1",
            "-d",
            "1",
            "-m",
            mattes(),
            *t,
            *V,
            expect="error",
        ),
        ai("messages/no-metric", "messages", "No -m", *d3, *t, *V, expect="error"),
        ai("messages/no-metric-quiet", "messages", "No -m, without -v", *d3, *t, expect="silent"),
        ai(
            "messages/one-image",
            "messages",
            "-m with one image",
            *d3,
            "-m",
            f"Mattes[{F}]",
            *t,
            *V,
            expect="error",
        ),
        ai(
            "messages/unknown-sampling",
            "messages",
            "An unknown sampling",
            *d3,
            "-m",
            mattes(rest="32,Sparse,0.2"),
            *t,
            *V,
            expect="error",
        ),
        ai(
            "messages/unknown-transform",
            "messages",
            "An unknown transform",
            *d3,
            "-m",
            mattes(),
            "-t",
            "Bspline[0.1]",
            *V,
            expect="error",
        ),
        ai(
            "messages/unknown-transform-quiet",
            "messages",
            "An unknown transform, without -v (ANTS-45)",
            *d3,
            "-m",
            mattes(),
            "-t",
            "Bspline[0.1]",
            expect="silent",
        ),
        ai("messages/no-transform", "messages", "No -t", *d3, "-m", mattes(), *V, expect="error"),
        ai(
            "messages/unknown-metric",
            "messages",
            "An unknown metric with iterations",
            *d3,
            "-m",
            f"Foo[{F},{M},32,Regular,0.2]",
            *t,
            "-c",
            "2",
            *O_MAT,
            *V,
            expect="error",
        ),
        ai(
            "messages/unknown-metric-align",
            "messages",
            "An unknown metric, AlignCentersOfMass with iterations",
            *d3,
            "-m",
            f"Foo[{F},{M}]",
            "-t",
            "AlignCentersOfMass",
            "-c",
            "3",
            *O_MAT,
            *V,
            expect="error",
        ),
        ai(
            "messages/two-blobs",
            "messages",
            "-b 2: too few blobs",
            *d3,
            "-m",
            mattes(),
            *t,
            "-b",
            "2",
            *V,
            expect="error",
        ),
        ai(
            "messages/two-blobs-bracket",
            "messages",
            "-b [2,5]",
            *d3,
            "-m",
            mattes(),
            *t,
            "-b",
            "[2,5]",
            *V,
            expect="error",
        ),
        ai(
            "messages/unknown-flag",
            "messages",
            "An unknown flag",
            *d3,
            "-m",
            mattes(),
            *t,
            "--bogus",
            "1",
            *V,
            expect="error",
        ),
        ai(
            "messages/missing-fixed",
            "messages",
            "A fixed image that does not exist",
            *d3,
            "-m",
            mattes("{missing:fixed.nii.gz}", M),
            *t,
            *O_MAT,
            *V,
            expect="abort",
        ),
        ai(
            "messages/missing-moving-geometry",
            "messages",
            "AlignGeometricCenters with a missing moving image",
            *d3,
            "-m",
            mattes(F, "{missing:moving.nii.gz}"),
            "-t",
            "AlignGeometricCenters",
            *O_MAT,
            *V,
            expect="abort",
        ),
        ai(
            "messages/missing-fixed-mass",
            "messages",
            "AlignCentersOfMass with a missing fixed image",
            *d3,
            "-m",
            mattes("{missing:fixed.nii.gz}", M),
            "-t",
            "AlignCentersOfMass",
            *O_MAT,
            *V,
            expect="abort",
        ),
        ai(
            "messages/bad-iterations",
            "messages",
            "-c 1o: the parser's conversion fails",
            *d3,
            "-m",
            mattes(),
            *t,
            "-c",
            "1o",
            *V,
            expect="abort",
        ),
        ai(
            "messages/bad-bins",
            "messages",
            "32 bins written 3x2",
            *d3,
            "-m",
            mattes(rest="3x2"),
            *t,
            *V,
            expect="abort",
        ),
        ai(
            "messages/unbalanced-bracket",
            "messages",
            "An unbalanced bracket",
            *d3,
            "-m",
            f"Mattes[{F},{M}",
            *t,
            *V,
            expect="abort",
        ),
        ai(
            "messages/bad-seed",
            "messages",
            "--random-seed 1x",
            *d3,
            "-m",
            mattes(),
            *t,
            *S,
            "-c",
            "[2,1e-6,5]",
            "--random-seed",
            "1x",
            *O_MAT,
            *V,
            expect="abort",
        ),
        ai(
            "messages/bad-env-seed",
            "messages",
            "ANTS_RANDOM_SEED=abc (std::stoi throws)",
            *d3,
            "-m",
            mattes(),
            *t,
            *S,
            "-c",
            "[2,1e-6,5]",
            *O_MAT,
            *V,
            seed="abc",
            expect="abort",
        ),
        ai(
            "messages/env-seed-range",
            "messages",
            "ANTS_RANDOM_SEED out of int's range",
            *d3,
            "-m",
            mattes(),
            *t,
            *S,
            "-c",
            "[2,1e-6,5]",
            *O_MAT,
            *V,
            seed="99999999999",
            expect="abort",
        ),
        ai(
            "messages/zero-moving-mass",
            "messages",
            "AlignCentersOfMass, a moving image of zeros",
            *d3,
            "-m",
            mattes(F, Z),
            "-t",
            "AlignCentersOfMass",
            *O_MAT,
            *V,
            expect="abort",
        ),
        ai(
            "messages/zero-moving-geometry",
            "messages",
            "AlignGeometricCenters, a moving image of zeros: no moments needed",
            *d3,
            "-m",
            mattes(F, Z),
            "-t",
            "AlignGeometricCenters",
            *O_MAT,
            *V,
        ),
        ai(
            "messages/zero-principal-axes",
            "messages",
            "Principal axes of an image of zeros",
            *d3,
            "-m",
            mattes(F, Z),
            *t,
            "-p",
            "1",
            *O_MAT,
            *V,
            expect="abort",
        ),
        ai(
            "messages/zero-fixed-search",
            "messages",
            "A search from a fixed image of zeros",
            *d3,
            "-m",
            mattes(Z, M),
            *t,
            *S,
            "-c",
            "[2,1e-6,5]",
            *O_MAT,
            *V,
            expect="abort",
        ),
        ai(
            "messages/unknown-extension",
            "messages",
            "An output name no writer takes",
            *d3,
            "-m",
            mattes(),
            *t,
            *S,
            "-c",
            "[2,1e-6,5]",
            "-o",
            "{out}/t.xyz",
            *V,
            expect="abort",
        ),
        ai(
            "messages/no-directory",
            "messages",
            "An output in a directory that does not exist",
            *d3,
            "-m",
            mattes(rest="32"),
            *t,
            "-o",
            "{out}/none/t.mat",
            *V,
            expect="abort",
        ),
        ai(
            "messages/align-iterations-unknown-extension",
            "messages",
            "ANTS-53 with an output name no writer takes",
            *d3,
            "-m",
            mattes(rest="32"),
            "-t",
            "AlignGeometricCenters",
            "-c",
            "3",
            "-o",
            "{out}/t.xyz",
            *V,
            expect="abort",
        ),
        ai(
            "messages/align-iterations-txt",
            "messages",
            "ANTS-53 into a text file: ITK's writer reads an empty list",
            *d3,
            "-m",
            mattes(rest="32"),
            "-t",
            "AlignGeometricCenters",
            "-c",
            "3",
            "-o",
            "{out}/t.txt",
            *V,
            expect="abort",
        ),
        ai(
            "messages/align-iterations-h5",
            "messages",
            "ANTS-53 into an HDF5 file",
            *d3,
            "-m",
            mattes(rest="32"),
            "-t",
            "AlignGeometricCenters",
            "-c",
            "3",
            "-o",
            "{out}/t.h5",
            *V,
            expect="abort",
        ),
        ai(
            "messages/empty-start-list",
            "messages",
            "A negative arc fraction: no start point, the best of an empty list",
            *d3,
            "-m",
            mattes(),
            *t,
            "-s",
            "[90,-0.5]",
            "-c",
            "[2,1e-6,5]",
            *O_MAT,
            *V,
            expect="abort",
        ),
        ai(
            "messages/empty-grid",
            "messages",
            "A negative grid extent: no start point",
            "-d",
            "2",
            "-m",
            mattes(F2, M2, "32,Regular,0.5"),
            "-t",
            "Affine[0.1]",
            *S,
            "-c",
            "[2,1e-6,5]",
            "-g",
            "[5,-5x5]",
            *O_MAT,
            *V,
            expect="abort",
        ),
        ai(
            "messages/empty-sample-mattes",
            "messages",
            "Regular sampling inside an empty mask: no sample point",
            *d3,
            "-m",
            mattes(),
            "-x",
            E,
            *t,
            *S,
            "-c",
            "[2,1e-6,5]",
            *O_MAT,
            *V,
            expect="abort",
        ),
        ai(
            "messages/empty-sample-mi",
            "messages",
            "The same with MI",
            *d3,
            "-m",
            f"MI[{F},{M},32,Regular,0.25]",
            "-x",
            E,
            *t,
            *S,
            "-c",
            "[2,1e-6,5]",
            *O_MAT,
            *V,
            expect="abort",
        ),
        ai(
            "messages/empty-sample-gc",
            "messages",
            "The same with GC",
            *d3,
            "-m",
            f"GC[{F},{M},32,Regular,0.25]",
            "-x",
            E,
            *t,
            *S,
            "-c",
            "[2,1e-6,5]",
            *O_MAT,
            *V,
            expect="abort",
        ),
        ai(
            "messages/empty-sample-align",
            "messages",
            "No sample point, AlignGeometricCenters with iterations",
            *d3,
            "-m",
            mattes(),
            "-x",
            E,
            "-t",
            "AlignGeometricCenters",
            "-c",
            "2",
            *O_MAT,
            *V,
            expect="abort",
        ),
        ai(
            "messages/random-zero",
            "messages",
            "Random sampling of 0: no sample point",
            *d3,
            "-m",
            mattes(rest="32,Random,0"),
            *t,
            *S,
            "-c",
            "[2,1e-6,5]",
            *O_MAT,
            *V,
            expect="abort",
        ),
        ai(
            "messages/mattes-empty-mask-dense",
            "messages",
            "Mattes over every voxel with an empty fixed mask (ITK-20: the second start point "
            "crashes)",
            *d3,
            "-m",
            mattes(rest="32,None"),
            "-x",
            E,
            *t,
            *S,
            "-c",
            "[2,1e-6,5]",
            *O_MAT,
            *V,
            expect="abort",
        ),
        ai(
            "messages/mattes-empty-moving-mask",
            "messages",
            "Mattes with an empty moving mask (ITK-20)",
            *d3,
            "-m",
            mattes(),
            "-x",
            f"[{K},{E}]",
            *t,
            *S,
            "-c",
            "[2,1e-6,5]",
            *O_MAT,
            *V,
            expect="abort",
        ),
        ai(
            "messages/mattes-one-start-point",
            "messages",
            "Mattes with an empty mask and one start point: the best value of an empty list",
            *d3,
            "-m",
            mattes(rest="32,None"),
            "-x",
            E,
            *t,
            "-s",
            "[400,0.5]",
            "-c",
            "[2,1e-6,5]",
            *O_MAT,
            *V,
            expect="abort",
        ),
        ai(
            "messages/percentage-zero",
            "messages",
            "Regular sampling of 0 (ANTS-06), one sample point: one intensity (ITK-20)",
            *d3,
            "-m",
            mattes(rest="32,Regular,0"),
            *t,
            *S,
            "-c",
            "[2,1e-6,5]",
            *O_MAT,
            *V,
            expect="abort",
        ),
        ai(
            "messages/search-factor-negative",
            "messages",
            "A negative search factor: ANTs' loop never ends",
            *d3,
            "-m",
            mattes(),
            *t,
            "-s",
            "[-90,0.5]",
            "-c",
            "[2,1e-6,5]",
            *O_MAT,
            *V,
            expect="hang",
        ),
        ai(
            "messages/translation-step-zero",
            "messages",
            "A translation step of 0: ANTs' loop never ends",
            *d3,
            "-m",
            mattes(),
            "-t",
            "Affine[0.1]",
            *S,
            "-c",
            "[2,1e-6,5]",
            "-g",
            "[0,10x10x10]",
            *O_MAT,
            *V,
            expect="hang",
        ),
    ]


def _random_cases() -> list[AiCase]:
    d3 = ("-d", "3")
    return [
        ai(
            "random/random-sampling",
            "random",
            "Random sampling: ANTs draws the voxels from the clock (ANTS-58)",
            *d3,
            "-m",
            mattes(rest="32,Random,0.25"),
            "-t",
            "Rigid[0.1]",
            *S,
            "-c",
            "[2,1e-6,5]",
            *O_MAT,
            *V,
            expect="random",
        ),
        ai(
            "random/seed-zero",
            "random",
            "--random-seed 0: ANTs seeds Regular sampling from the clock (ANTS-58)",
            *d3,
            "-m",
            mattes(),
            "-t",
            "Rigid[0.1]",
            *S,
            "-c",
            "[2,1e-6,5]",
            "--random-seed",
            "0",
            *O_MAT,
            *V,
            expect="random",
        ),
    ]


def _blob_cases() -> list[AiCase]:
    """``-b`` (the blob alignment): phantoms with known transforms, ANTs' failure paths,
    real T1w images."""
    d3, d2 = ("-d", "3"), ("-d", "2")
    f3, f2 = "{gen:ai-blobs-fixed-3d}", "{gen:ai-blobs-fixed-2d}"

    def mv(move: str) -> str:
        return "{gen:ai-blobs-moving-" + move + "}"

    def m(fixed: str, moving: str) -> str:
        return f"Mattes[{fixed},{moving},32,Regular,0.25]"

    out = []
    for move, dims, f in (
        ("shift", d3, f3),
        ("rigid", d3, f3),
        ("affine", d3, f3),
        ("2d-shift", d2, f2),
        ("2d-rigid", d2, f2),
    ):
        for transform in ("Rigid", "Affine", "Similarity"):
            if transform == "Similarity" and move not in ("shift", "2d-rigid"):
                continue
            what = {
                "shift": "an exact translation (ANTs' own test keeps the true pairs)",
                "rigid": "a rotation of 7 and 4 degrees and a shift",
                "affine": "a rotation of 5 degrees with scales 1.06 and 0.96",
                "2d-shift": "a 2D translation",
                "2d-rigid": "a 2D rotation of 9 degrees and a shift",
            }[move]
            expect = "abort" if (move == "affine" and transform == "Affine") else "pass"
            # An exact translation: the corrections (ANTS-111, ANTS-116) change the blob pairs,
            # but ANTs' pairs and larmorx's both give the exact translation, bit for bit.
            exact = ("ANTS-111", "ANTS-116") if move == "shift" and transform != "Affine" else ()
            out.append(
                ai(
                    f"blobs/{move}-{transform.lower()}",
                    "blobs",
                    f"{transform} from 30 blobs of a phantom and its copy moved by {what}",
                    *dims,
                    "-m",
                    m(f, mv(move)),
                    "-t",
                    f"{transform}[0.1]",
                    "-b",
                    "30",
                    *O_MAT,
                    *V,
                    expect=expect,
                    truth=f"phantom:{move}",
                    band=(move == "shift" and transform == "Rigid"),
                    silent_ok=exact,
                )
            )
    out += [
        ai(
            "blobs/shift-rigid-match-10",
            "blobs",
            "-b [30,10]: 30 blobs extracted, at most 10 pairs",
            *d3,
            "-m",
            m(f3, mv("shift")),
            "-t",
            "Rigid[0.1]",
            "-b",
            "[30,10]",
            *O_MAT,
            *V,
            truth="phantom:shift",
        ),
        ai(
            "blobs/shift-rigid-search",
            "blobs",
            "The blob alignment, then a search over rotations about the image axes",
            *d3,
            "-m",
            m(f3, mv("shift")),
            "-t",
            "Rigid[0.1]",
            "-b",
            "30",
            "-s",
            "[30,0.12]",
            "-c",
            "[3,1e-6,5]",
            *O_MAT,
            *V,
            truth="phantom:shift",
            silent_ok=("ANTS-111", "ANTS-116"),
        ),
        ai(
            "blobs/rigid-affine-search-h5",
            "blobs",
            "Affine from the blobs of the rotated phantom, a search, an HDF5 output",
            *d3,
            "-m",
            m(f3, mv("rigid")),
            "-t",
            "Affine[0.1]",
            "-b",
            "30",
            "-s",
            "[30,0.12]",
            "-c",
            "[2,1e-6,5]",
            "-o",
            "{out}/t.h5",
            *V,
            truth="phantom:rigid",
        ),
        ai(
            "blobs/identical-rigid",
            "blobs",
            "An image against itself: every displacement is 0, so ANTs' ratios are 0/0 (ANTS-110)",
            *d3,
            "-m",
            m(f3, f3),
            "-t",
            "Rigid[0.1]",
            "-b",
            "20",
            *O_MAT,
            *V,
        ),
        ai(
            "blobs/no-pairs-rigid",
            "blobs",
            "-b [5,0]: no pair to match; the rigid fit of no landmark (ANTS-114)",
            *d3,
            "-m",
            m(f3, mv("shift")),
            "-t",
            "Rigid[0.1]",
            "-b",
            "[5,0]",
            *O_MAT,
            *V,
        ),
        ai(
            "blobs/no-pairs-affine",
            "blobs",
            "-b [5,0] with Affine: too few landmarks, ITK's exception",
            *d3,
            "-m",
            m(f3, mv("shift")),
            "-t",
            "Affine[0.1]",
            "-b",
            "[5,0]",
            *O_MAT,
            *V,
            expect="abort",
        ),
        ai(
            "blobs/2d-no-pairs-rigid",
            "blobs",
            "2D, no pair: ITK's warning (fewer than 2 landmarks), then NaN (ANTS-114)",
            *d2,
            "-m",
            m(f2, mv("2d-shift")),
            "-t",
            "Rigid[0.1]",
            "-b",
            "[5,0]",
            *O_MAT,
            *V,
        ),
        ai(
            "blobs/constant",
            "blobs",
            "Two constant images: every voxel is a maximum of value 0",
            *d3,
            "-m",
            m("{gen:ai-blobs-constant}", "{gen:ai-blobs-constant}"),
            "-t",
            "Rigid[0.1]",
            "-b",
            "5",
            *O_MAT,
            *V,
        ),
        ai(
            "blobs/noise-rigid",
            "blobs",
            "Noise against itself, 48^3: patches barely fit",
            *d3,
            "-m",
            m("{gen:ai-blobs-noise}", "{gen:ai-blobs-noise}"),
            "-t",
            "Rigid[0.1]",
            "-b",
            "10",
            *O_MAT,
            *V,
        ),
        ai(
            "blobs/thin",
            "blobs",
            "An axis of 3 voxels: the Laplacian of Gaussian refuses it (ITK's exception)",
            *d3,
            "-m",
            m("{gen:ai-blobs-thin}", "{gen:ai-blobs-thin}"),
            "-t",
            "Rigid[0.1]",
            "-b",
            "5",
            *O_MAT,
            *V,
            expect="abort",
        ),
        ai(
            "blobs/missing-fixed",
            "blobs",
            "A fixed image that does not exist: the blob detector's input is not set",
            *d3,
            "-m",
            m("{missing:nothere.nii.gz}", mv("shift")),
            "-t",
            "Rigid[0.1]",
            "-b",
            "5",
            *O_MAT,
            *V,
            expect="abort",
        ),
        ai(
            "blobs/missing-moving",
            "blobs",
            "A moving image that does not exist (after the fixed image's blobs)",
            *d3,
            "-m",
            m(f3, "{missing:nothere.nii.gz}"),
            "-t",
            "Rigid[0.1]",
            "-b",
            "5",
            *O_MAT,
            *V,
            expect="abort",
        ),
        ai(
            "blobs/principal-axes-ignored",
            "blobs",
            "-p 1 with -b: the blobs decide the initial transform",
            *d3,
            "-m",
            m(f3, mv("shift")),
            "-t",
            "Rigid[0.1]",
            "-p",
            "1",
            "-b",
            "30",
            *O_MAT,
            *V,
            truth="phantom:shift",
            silent_ok=("ANTS-111", "ANTS-116"),
        ),
        ai(
            "real/ds000005-sub-01/blobs-t1w-shift",
            "real",
            "fMRIPrep's T1w of ds000005 against its copy with the origin moved (6, -4, 3) mm",
            *d3,
            "-m",
            m("{oracle:" + ants_ai_blobs.DS000005_T1W + "}", "{gen:ai-blobs-t1w-shift}"),
            "-t",
            "Rigid[0.1]",
            "-b",
            "30",
            *O_MAT,
            *V,
            truth="t1w:shift",
            band=True,
        ),
        ai(
            "real/ds000005-sub-01/blobs-t1w-rotated",
            "real",
            "fMRIPrep's T1w of ds000005 against its copy rotated by 8 degrees and shifted",
            *d3,
            "-m",
            m("{oracle:" + ants_ai_blobs.DS000005_T1W + "}", "{gen:ai-blobs-t1w-rotated}"),
            "-t",
            "Rigid[0.1]",
            "-b",
            "30",
            *O_MAT,
            *V,
            truth="t1w:rotated",
        ),
        ai(
            "real/ds000005-sub-01/blobs-t1w-rotated-affine",
            "real",
            "The same with Affine (ANTs: too few landmarks)",
            *d3,
            "-m",
            m("{oracle:" + ants_ai_blobs.DS000005_T1W + "}", "{gen:ai-blobs-t1w-rotated}"),
            "-t",
            "Affine[0.1]",
            "-b",
            "30",
            *O_MAT,
            *V,
            expect="abort",
            truth="t1w:rotated",
        ),
    ]
    tpl = "{tf:tpl-MNI152NLin2009cAsym/tpl-MNI152NLin2009cAsym_res-02_T1w.nii.gz}"
    oasis = "{tf:tpl-OASIS30ANTs/tpl-OASIS30ANTs_res-01_T1w.nii.gz}"
    runs = (
        ("ds000005-sub-01-noreconall", "sub-01/anat/sub-01_desc-preproc_T1w.nii.gz", "standard"),
        ("ds001600-sub-1-noreconall", "sub-1/anat/sub-1_desc-preproc_T1w.nii.gz", "full"),
        ("ds005040-sub-002-noreconall", "sub-002/anat/sub-002_desc-preproc_T1w.nii.gz", "full"),
        ("ds005454-sub-16-noreconall", "sub-16/anat/sub-16_desc-preproc_T1w.nii.gz", "full"),
        (
            "ds005752-sub-ON01016-noreconall",
            "sub-ON01016/ses-01/anat/sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_desc-preproc_T1w.nii.gz",
            "full",
        ),
    )
    for run, t1w, tier in runs:
        label = run.split("-noreconall")[0]
        out.append(
            ai(
                f"real/{label}/blobs-mni-rigid",
                "real",
                f"MNI152NLin2009cAsym (2 mm) against {label}'s T1w, Rigid from 30 blobs",
                *d3,
                "-m",
                m(tpl, "{oracle:" + run + "/out/" + t1w + "}"),
                "-t",
                "Rigid[0.1]",
                "-b",
                "30",
                *O_MAT,
                *V,
                tier=tier,
            )
        )
    out.append(
        ai(
            "real/ds005040-sub-002/blobs-oasis-affine",
            "real",
            "OASIS30ANTs (1 mm) against ds005040's T1w, Affine from 100 blobs",
            *d3,
            "-m",
            m(
                oasis,
                "{oracle:ds005040-sub-002-noreconall/out/sub-002/anat/sub-002_desc-preproc_T1w.nii.gz}",
            ),
            "-t",
            "Affine[0.1]",
            "-b",
            "100",
            *O_MAT,
            *V,
            tier="full",
            expect="abort",
        )
    )
    return out


#: fMRIPrep's antsAI nodes in the oracle runs: (run, node directory, tier).
REAL_NODES = (
    (
        "ds000005-sub-01-noreconall",
        "sub_01_wf/anat_fit_wf/brain_extraction_wf/init_aff",
        "standard",
    ),
    (
        "ds000005-sub-01-noreconall",
        "sub_01_wf/bold_task_mixedgamblestask_run_01_wf/bold_fit_wf/"
        "enhance_and_skullstrip_bold_wf/init_aff",
        "standard",
    ),
    (
        "ds000005-sub-01-noreconall",
        "sub_01_wf/bold_task_mixedgamblestask_run_02_wf/bold_fit_wf/"
        "enhance_and_skullstrip_bold_wf/init_aff",
        "full",
    ),
    (
        "ds000005-sub-01-noreconall",
        "sub_01_wf/bold_task_mixedgamblestask_run_03_wf/bold_fit_wf/"
        "enhance_and_skullstrip_bold_wf/init_aff",
        "full",
    ),
    ("ds001600-sub-1-noreconall", "sub_1_wf/anat_fit_wf/brain_extraction_wf/init_aff", "full"),
    ("ds005040-sub-002-noreconall", "sub_002_wf/anat_fit_wf/brain_extraction_wf/init_aff", "full"),
    (
        "ds005040-sub-002-noreconall",
        "sub_002_wf/bold_task_clip_run_1_wf/bold_fit_wf/enhance_and_skullstrip_bold_wf/init_aff",
        "full",
    ),
    ("ds005454-sub-16-noreconall", "sub_16_wf/anat_fit_wf/brain_extraction_wf/init_aff", "full"),
    (
        "ds005454-sub-16-noreconall",
        "sub_16_wf/bold_task_rest_wf/bold_fit_wf/enhance_and_skullstrip_bold_wf/init_aff",
        "full",
    ),
    (
        "ds005752-sub-ON01016-noreconall",
        "sub_ON01016_ses_01_wf/anat_fit_wf/brain_extraction_wf/init_aff",
        "full",
    ),
)


def _real_cases() -> list[AiCase]:
    out = []
    for run, node, tier in REAL_NODES:
        what = "brain extraction" if "brain_extraction" in node else "BOLD reference"
        bold = re.search(r"bold_task_(\w+?)_wf", node)
        label = what + (f" ({bold.group(1)})" if bold else "")
        cid = f"real/{run.split('-noreconall')[0]}/{'brain-extraction' if bold is None else 'bold-' + bold.group(1).replace('_', '-')}"
        out.append(
            AiCase(
                cid,
                "real",
                f"fMRIPrep's init_aff, {label}, with fMRIPrep's seed",
                (f"{{fmriprep:{run}/{node}}}",),
                tier=tier,
                band=tier == "standard",
                fmriprep=f"{run}/{node}",
            )
        )
    return out


def all_cases() -> list[AiCase]:
    return _search_cases() + _message_cases() + _random_cases() + _real_cases() + _blob_cases()


def cases_for(tier: str) -> list[Case]:
    rank = _TIERS.index(tier)
    return [
        Case(c.id, c.category, c.description, c)
        for c in all_cases()
        if _TIERS.index(c.tier) <= rank
    ]


# ------------------------------------------------------------------------------------------------
# Running


def _fmriprep_node(spec: str) -> tuple[Path, list[str], str | None]:
    """An fMRIPrep node's directory, its ``antsAI`` arguments (paths mapped to the oracle run and
    ``larmorx-testdata``, the output name kept) and its ``ANTS_RANDOM_SEED``."""
    import larmorx_testdata as td

    root = reg._oracle_root()
    run, node = spec.split("/", 1)
    if root is None or not (root / run / "work").is_dir():
        raise Outcome(SKIPPED, f"the fMRIPrep oracle run {run} is not here")
    work = root / run / "work" / "fmriprep_25_2_wf" / node
    words = (work / "command.txt").read_text(encoding="utf-8").split()[1:]
    text = " ".join(words)
    text = text.replace("/work/", f"{root / run / 'work'}/").replace(
        "/out/", f"{root / run / 'out'}/"
    )
    text = re.sub(r"/templateflow/[^\s,\]]+", lambda m: str(td.get(m.group(0)[1:])), text)
    seed = None
    report = work / "_report" / "report.rst"
    if report.is_file():
        m = re.search(r"\* ANTS_RANDOM_SEED : (\S+)", report.read_text(encoding="utf-8"))
        if m:
            seed = m.group(1)
    return work, text.split(), seed


def _resolve(c: AiCase, out: Path, tmp: Path) -> tuple[list[str], str | None]:
    """The case's arguments for an output directory ``out``, and its seed."""
    if len(c.args) == 1 and c.args[0].startswith("{fmriprep:"):
        _work, args, seed = _fmriprep_node(c.args[0][10:-1])
        i = args.index("-o")
        args[i + 1] = str(out / Path(args[i + 1]).name)
        return args, seed

    def sub(m: re.Match[str]) -> str:
        key = m.group(1)
        if key == "out":
            return str(out)
        if key.startswith("gen:"):
            return str(ants_inputs.path(key[4:]))
        if key.startswith("missing:"):
            return str(tmp / key[8:])
        if key.startswith("tf:"):
            import larmorx_testdata as td

            return str(td.get("templateflow/" + key[3:]))
        if key == "nw-fixed" or key.startswith("nw-moving:"):
            return str(_niworkflows_input(key, tmp))
        if key.startswith("oracle:"):
            root = reg._oracle_root()
            target = None if root is None else root / key[7:]
            if target is None or not target.is_file():
                raise Outcome(SKIPPED, f"the fMRIPrep oracle file {key[7:]} is not here")
            return str(target)
        raise KeyError(f"unknown placeholder {{{key}}}")

    return [re.sub(r"\{([^{}]+)\}", sub, a) for a in c.args], c.seed


def _niworkflows_input(key: str, tmp: Path) -> Path:
    """The images niworkflows' ``RobustMNINormalization`` gives ``antsAffineInitializer``
    (nipype did not keep them): ``nw-fixed`` the 1 mm MNI template times its brain mask,
    ``nw-moving:<run>`` the run's ``desc-preproc_T1w`` times its brain mask (as the
    antsRegistration suite's stand-ins)."""
    import larmorx_testdata as td

    tpl = "templateflow/tpl-MNI152NLin2009cAsym/tpl-MNI152NLin2009cAsym_res-01_"
    if key == "nw-fixed":
        return reg._masked(
            tmp, "fixed_masked.nii.gz", td.get(tpl + "T1w.nii.gz"),
            td.get(tpl + "desc-brain_mask.nii.gz"),
        )  # fmt: skip
    root = reg._oracle_root()
    run = key.split(":", 1)[1]
    run_dir = None if root is None else root / run
    if run_dir is None or not run_dir.is_dir():
        raise Outcome(SKIPPED, f"the fMRIPrep oracle run {run} is not here")
    anat = sorted(
        p
        for p in (run_dir / "out").glob("sub-*/**/anat/*desc-preproc_T1w.nii.gz")
        if "space-" not in p.name
    )
    if not anat:
        raise Outcome(SKIPPED, f"{run} has no desc-preproc_T1w")
    brain = anat[0].with_name(anat[0].name.replace("desc-preproc_T1w", "desc-brain_mask"))
    return reg._masked(tmp, "moving_masked.nii.gz", anat[0], brain)


def _run_ants(
    program: str,
    args: list[str],
    seed: str | None,
    threads: int = 1,
    *,
    correctly_rounded: bool = False,
    timeout: float = 7200,
):
    b = ants_bin()
    if b is None or not (b / program).is_file():
        raise Outcome(SKIPPED, f"the ANTs oracle's {program} was not found")
    env = {k: v for k, v in os.environ.items() if k not in ("ANTS_RANDOM_SEED", "LD_PRELOAD")}
    env["ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS"] = str(threads)
    if seed is not None:
        env["ANTS_RANDOM_SEED"] = seed
    if correctly_rounded:
        lib = crmath_preload()
        if lib is None:
            raise FileNotFoundError("libcrmath.so (scripts/build_crmath_preload.sh)")
        env["LD_PRELOAD"] = str(lib)
    try:
        p = subprocess.run(
            [str(b / program), *args],
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=env,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired as e:

        def text(v) -> str:
            return v.decode("utf-8", "replace") if isinstance(v, bytes) else (v or "")

        return None, text(e.stdout), text(e.stderr)
    return p.returncode, p.stdout, p.stderr


def _run_larmorx(program: str, args: list[str], seed: str | None, mode: str = defects.REPRODUCE):
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
                ["larmorx", "ants", program, *args], _replica.search_dirs(), None
            )
    finally:
        for k, v in saved.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v
    return code, out, err


def _normalise(text: str, out: Path) -> list[str]:
    """``antsRegistration``'s masking, and the build's source paths in ITK's warnings and in
    the exceptions libstdc++ prints."""
    lines = reg._normalise(text, out)
    return [
        re.sub(r"(what\(\):  |WARNING: In )\S*?/(Modules|Utilities|Examples)/", r"\1\2/", line)
        for line in lines
    ]


def _compare(
    checks: CheckList, label: str, ants: tuple, lx: tuple, a_dir: Path, l_dir: Path
) -> bool:
    """``antsRegistration``'s comparison (exit status, both streams, every file) with this
    suite's masking; whether everything is identical."""
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
        detail="identical" if same else reg._first_difference(na, nl),
    )
    ok &= same
    ea, el = _normalise(a_err, a_dir), _normalise(l_err, l_dir)
    same = ea == el
    checks(
        f"{label}: standard error",
        same,
        detail="identical" if same else reg._first_difference(ea, el),
    )
    ok &= same
    fa, fl = reg._outputs(a_dir), reg._outputs(l_dir)
    same = sorted(fa) == sorted(fl)
    checks(
        f"{label}: files written",
        same,
        detail=", ".join(sorted(fa)) if same else f"ANTs {sorted(fa)}, larmorx {sorted(fl)}",
    )
    ok &= same
    for name in sorted(set(fa) & set(fl)):
        s, why = reg._same_file(fa[name], fl[name])
        checks(f"{label}: {name}", s, detail="identical" if s else why)
        ok &= s
    return ok


_LARMORX = re.compile(r"^larmorx: ")


def _without_larmorx(lines: list[str]) -> list[str]:
    return [line for line in lines if not _LARMORX.match(line)]


def _parameters(d: Path) -> np.ndarray | None:
    """Every transform parameter written, in file order; empty files (ANTS-53) hold none."""
    import larmorx as lx

    vals = []
    for p in reg._outputs(d).values():
        if p.suffix in (".mat", ".txt", ".h5") and p.stat().st_size > 0:
            for t in lx.transforms.read(p):
                vals.append(np.asarray(t.parameters, dtype=np.float64))
    return np.concatenate(vals) if vals else None


def _distance(a: np.ndarray | None, b: np.ndarray | None) -> float:
    if a is None or b is None or a.shape != b.shape:
        return float("nan")
    return float(np.max(np.abs(a - b)))


def _fmriprep_parameters(spec: str) -> np.ndarray | None:
    import larmorx as lx

    root = reg._oracle_root()
    if root is None:
        return None
    run, node = spec.split("/", 1)
    work = root / run / "work" / "fmriprep_25_2_wf" / node
    files = sorted(work.glob("*.mat"))
    if not files:
        return None
    return np.concatenate(
        [np.asarray(t.parameters, dtype=np.float64) for t in lx.transforms.read(files[0])]
    )


def run_case(case: Case, checks: CheckList) -> None:
    c: AiCase = case.payload
    program = c.program
    with tempfile.TemporaryDirectory(prefix="lx-ai-") as t:
        tmp = Path(t)
        a_dir, l_dir = tmp / "ants", tmp / "larmorx"
        a_dir.mkdir()
        l_dir.mkdir()
        (a_args, seed), (l_args, _) = _resolve(c, a_dir, tmp), _resolve(c, l_dir, tmp)
        timeout = HANG_SECONDS if c.expect == "hang" else 7200
        ants = _run_ants(program, a_args, seed, timeout=timeout)
        lx = _run_larmorx(program, l_args, seed)
        if c.expect == "divergence":
            checks(
                "larmorx: exit status and message",
                True,
                detail=f"ANTs exit {ants[0]}; larmorx exit {lx[0]}: "
                f"{lx[2].strip().splitlines()[-1:]}",
            )
            raise Outcome(EXPECTED, c.reason)
        if c.expect == "hang":
            checks(
                "ANTs does not finish",
                ants[0] is None,
                detail=f"ANTs {'still running' if ants[0] is None else f'exit {ants[0]}'} "
                f"after {HANG_SECONDS} s",
            )
            last = (lx[2].strip().splitlines() or [""])[-1]
            checks(
                "larmorx refuses (exit 1)",
                lx[0] == 1 and last.startswith("larmorx: "),
                detail=f"exit {lx[0]}: {last[:160]}",
            )
            raise Outcome(EXPECTED, "ANTs never finishes (class (a)); larmorx refuses")
        if c.expect == "abort":
            aborted = ants[0] is not None and ants[0] < 0
            checks(
                "ANTs aborts or crashes, larmorx exits 1",
                aborted and lx[0] == 1,
                detail=f"ANTs {ants[0]} (a signal), larmorx {lx[0]}",
            )
            oa, ol = _normalise(ants[1], a_dir), _normalise(lx[1], l_dir)
            checks(
                "standard output up to the abort",
                oa == ol,
                detail="identical" if oa == ol else reg._first_difference(oa, ol),
            )
            ea, el = _normalise(ants[2], a_dir), _without_larmorx(_normalise(lx[2], l_dir))
            checks(
                "standard error up to the abort",
                ea == el,
                detail="identical" if ea == el else reg._first_difference(ea, el),
            )
            fa, fl = reg._outputs(a_dir), reg._outputs(l_dir)
            for name in sorted(set(fa) & set(fl)):
                s, why = reg._same_file(fa[name], fl[name])
                checks(f"{name} (written before the abort)", s, detail="identical" if s else why)
            last = (lx[2].strip().splitlines() or [""])[-1]
            raise Outcome(BOTH_ERROR, last[:200])
        if c.expect == "silent":
            same = ants[0] == lx[0] and ants[0] != 0
            checks(
                "both fail with the same status", same, detail=f"ANTs {ants[0]}, larmorx {lx[0]}"
            )
            ea = _normalise(ants[2] + ants[1], a_dir)
            el = _normalise(lx[2] + lx[1], l_dir)
            checks(
                "ANTs is silent, larmorx explains (ANTS-45)",
                not ea and bool(el),
                detail=f"ANTs {len(ea)} lines; larmorx: {(el or [''])[0][:160]}",
            )
            raise Outcome(BOTH_ERROR, (el or [str(ants[0])])[0][:200])
        if c.expect == "error" or (
            ants[0] is not None and ants[0] != 0 and lx[0] != 0 and ants[0] == lx[0]
        ):
            same = ants[0] == lx[0] and ants[0] != 0
            checks(
                "both fail with the same status", same, detail=f"ANTs {ants[0]}, larmorx {lx[0]}"
            )
            ea, el = _normalise(ants[2] + ants[1], a_dir), _normalise(lx[2] + lx[1], l_dir)
            checks(
                "their messages",
                ea == el,
                detail="identical" if ea == el else reg._first_difference(ea, el),
            )
            raise Outcome(
                BOTH_ERROR,
                ((ants[2] + ants[1]).strip().splitlines() or [str(ants[0])])[-1][:200],
            )
        if c.expect == "random":
            r_dir = tmp / "ants-again"
            r_dir.mkdir()
            again = _run_ants(program, _resolve(c, r_dir, tmp)[0], seed)
            checks(
                "exit status",
                ants[0] == lx[0] == again[0],
                detail=f"ANTs {ants[0]} and {again[0]}, larmorx {lx[0]}",
            )
            oa, ol = _normalise(ants[1], a_dir), _normalise(lx[1], l_dir)
            checks(
                "standard output: the same lines up to the first metric value",
                len(oa) == len(ol) and oa[:2] == ol[:2],
                detail=f"ANTs {len(oa)} lines, larmorx {len(ol)}",
            )
            pa, pb, pl = _parameters(a_dir), _parameters(r_dir), _parameters(l_dir)
            own = _distance(pa, pb)
            mine = min(_distance(pa, pl), _distance(pb, pl))
            checks(
                "ANTs' own runs",
                True,
                metric="max |parameter difference|",
                value=f"{own:.3g}",
                threshold="reported (ANTs seeds from the clock)",
                detail=f"ANTs run to run {own:.3g}; larmorx to the nearer run {mine:.3g}",
            )
            raise Outcome(
                EXPECTED,
                "ANTs seeds from the clock (ANTS-58): two runs differ; larmorx is repeatable",
            )
        plain = CheckList()
        identical = _compare(plain, "unmodified ANTs", ants, lx, a_dir, l_dir)
        if identical:
            checks.checks.extend(plain.checks)
        else:
            checks.checks.extend(
                ch if ch.passed else replace(ch, passed=True, threshold=GLIBC)
                for ch in plain.checks
            )
            c_dir = tmp / "ants-cr"
            c_dir.mkdir()
            cr = _run_ants(program, _resolve(c, c_dir, tmp)[0], seed, correctly_rounded=True)
            _compare(checks, "ANTs with CORE-MATH", cr, lx, c_dir, l_dir)
        if c.fmriprep:
            theirs = _fmriprep_parameters(c.fmriprep)
            d = _distance(_parameters(l_dir), theirs)
            checks(
                "fMRIPrep's own transform",
                True,
                metric="max |parameter difference|",
                value=f"{d:.3g}",
                threshold="reported (fMRIPrep ran ANTs 2.6.2 with 8 threads)",
                detail=f"larmorx vs fMRIPrep's output {d:.3g}",
            )
        if c.band and ants[0] == 0:
            ref = _parameters(a_dir)
            band = 0.0
            crashed = []
            for n in BAND_THREADS:
                b_dir = tmp / f"ants-{n}"
                b_dir.mkdir()
                code = _run_ants(program, _resolve(c, b_dir, tmp)[0], seed, threads=n)[0]
                if code is not None and code < 0:
                    # ANTS-60: the blob detector indexes its per-thread heaps past their end.
                    crashed.append(f"{n} threads (signal {-code})")
                    continue
                other = _parameters(b_dir)
                if ref is not None and other is not None and other.shape == ref.shape:
                    band = max(band, float(np.max(np.abs(other - ref))))
            dist = _distance(_parameters(l_dir), ref)
            crash_text = f"; ANTs crashes at {', '.join(crashed)}" if crashed else ""
            checks(
                "thread band",
                True,
                metric="max |parameter difference|",
                value=f"{dist:.3g}",
                threshold=f"reported; ANTs at {', '.join(map(str, BAND_THREADS))} threads vs 1: "
                f"{band:.3g}{crash_text}",
                detail=f"larmorx vs ANTs (1 thread) {dist:.3g}; ANTs' own band {band:.3g}"
                f"{crash_text}",
            )
        # D13: larmorx's default on the same arguments.
        f_dir = tmp / "larmorx-fix"
        f_dir.mkdir()
        fx = _run_larmorx(program, _resolve(c, f_dir, tmp)[0], seed, mode=defects.FIX)
        change = defects.Change(0, 0, 0.0)
        other = []
        if fx[0] != lx[0]:
            other.append(f"exit {lx[0]} -> {fx[0]}")
        fl, ff = reg._outputs(l_dir), reg._outputs(f_dir)
        if sorted(fl) != sorted(ff):
            other.append(f"files {sorted(set(fl) ^ set(ff))}")
        pa, pb = _parameters(l_dir), _parameters(f_dir)
        if pa is not None and pb is not None and pa.shape == pb.shape:
            change = defects.Change.of(pb, pa)
        elif (pa is None) != (pb is None):
            other.append("a transform written by one only")
        # ITK-18 is named whenever the joint-histogram MI runs: where no point is valid (an
        # empty mask) its correction cannot change the result. ANTS-54 is named wherever
        # antsAffineInitializer's local-search argument is not 20, whether or not the search
        # converges before the iterations differ.
        defects.add_fix_check(
            checks,
            defects.notices_in(fx[2]),
            change,
            other_changes="; ".join(other),
            silent_ok=("ITK-18", "ANTS-54", *c.silent_ok),
        )
        if c.truth:
            _score(checks, c, a_args, a_dir, f_dir)


def _score(checks: CheckList, c: AiCase, args: list[str], a_dir: Path, f_dir: Path) -> None:
    """How far ANTs' and larmorx's default transforms are from the known one (reported)."""
    fixed = Path(re.match(r"\w+\[([^,\]]+)", args[args.index("-m") + 1]).group(1))
    name = "t.h5" if "{out}/t.h5" in c.args else "t.mat"

    def text(err: tuple[float, float] | None) -> str:
        if err is None:
            return "no transform"
        return f"{err[0]:.3g} mm mean, {err[1]:.3g} mm largest"

    ants = ants_ai_blobs.transform_error(a_dir / name, c.truth, fixed)
    mine = ants_ai_blobs.transform_error(f_dir / name, c.truth, fixed)
    checks(
        "alignment error: ANTs",
        True,
        metric="distance to the true transform",
        value=text(ants),
        threshold="reported",
        detail=f"ANTs 2.6.5 as built, one thread: {text(ants)}",
    )
    checks(
        "alignment error",
        True,
        metric="distance to the true transform",
        value=text(mine),
        threshold="reported",
        detail=f"ANTs {text(ants)}; larmorx's default {text(mine)}",
    )


def highlights(results: list[CaseResult]) -> list[str]:
    compared = [r for r in results if r.status == "pass"]
    plain = [r for r in compared if all(ch.threshold != GLIBC for ch in r.checks)]
    lines = [
        f"{len(compared)} cases compared: {len(plain)} identical to the unmodified ANTs (exit "
        "status, standard output, standard error, every file), "
        f"{len(compared) - len(plain)} identical once ANTs uses CORE-MATH's correctly rounded "
        "functions (glibc's are not always correctly rounded)."
    ]
    band = []
    crashes = []
    real = []
    aligned = []
    for r in results:
        for ch in r.checks:
            m = re.match(r"ANTs (.*); larmorx's default (.*)", ch.detail)
            if ch.name == "alignment error" and m:
                aligned.append((f"`{r.case}`", m.group(1), m.group(2)))
    for r in compared:
        for ch in r.checks:
            m = re.search(
                r"larmorx vs ANTs \(1 thread\) ([^\s;]+); ANTs' own band ([^\s;]+)", ch.detail
            )
            if ch.name == "thread band" and m:
                if "ANTs crashes at" in ch.detail:
                    crashes.append(r.case)
                else:
                    band.append((r.case, float(m.group(1)), float(m.group(2))))
            m = re.search(r"larmorx vs fMRIPrep's output (\S+)", ch.detail)
            if ch.name == "fMRIPrep's own transform" and m:
                real.append((r.case, m.group(1)))
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
    if crashes:
        lines.append("")
        lines.append(
            f"Thread band, blob alignment ({len(crashes)} cases: "
            + ", ".join(f"`{case}`" for case in crashes)
            + f"): ANTs' `antsAI -b` crashes at {', '.join(map(str, BAND_THREADS))} threads "
            "(ANTS-60: its blob detector indexes per-thread heaps past their end); larmorx gives "
            "ANTs' one-thread result at any thread count."
        )
    if aligned:
        lines += [
            "",
            "**Blob alignment against the known transforms** (`-b`; where the moving image is "
            "the fixed one moved by a known transform: the distance from where each program's "
            "transform sends test points to where they truly are; larmorx's default corrects "
            "ANTS-110, ANTS-111, ANTS-112, ANTS-114, ANTS-116 and ITK-60, `reproduce` is ANTs' result):",
            "",
            "| Case | ANTs 2.6.5 (one thread) | larmorx's default |",
            "|---|---|---|",
            *(f"| {case} | {a} | {b} |" for case, a, b in aligned),
        ]
    lines += ["", *defects.summary(results)]
    if real:
        lines.append("")
        lines.append(
            "fMRIPrep's own transforms (ANTs 2.6.2, 8 threads, fMRIPrep's seed): larmorx is within "
            + ", ".join(f"{d} (`{case}`)" for case, d in real)
            + " of them."
        )
    return lines


def suite() -> Suite:
    return Suite(
        name="ants-ai",
        title="antsAI",
        tool="`larmorx ants antsAI` / `lx.ants.ai` (crates larmorx-ants, larmorx-registration)",
        reference=f"antsAI from {ants_version()}, the binary built by "
        "`scripts/build_ants_oracle.sh`, run with one thread",
        thresholds=[
            ("Exit status", "identical"),
            ("Standard output", "identical after masking addresses and output paths"),
            ("Standard error", "identical (larmorx's D13 notices aside)"),
            ("Files", "transforms byte for byte (HDF5: every dataset)"),
            (
                "glibc's maths",
                "where ANTs as built differs, ANTs with CORE-MATH preloaded must be identical",
            ),
            ("Aborts and crashes", "everything ANTs printed before it stopped; larmorx exits 1"),
            (
                "Thread band",
                f"reported: ANTs at {', '.join(map(str, BAND_THREADS))} threads against 1",
            ),
        ],
        cases=cases_for,
        run_case=run_case,
        packages=("nibabel", "scipy"),
        highlights=highlights,
        notes="Inputs named `gen:<name>` are built by the suites (seeded phantoms); the real cases "
        "re-run fMRIPrep 25.2.6's own command lines from the oracle runs.",
    )


__all__ = ["AiCase", "all_cases", "run_case", "suite"]
