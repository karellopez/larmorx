# SPDX-License-Identifier: Apache-2.0
"""Parity of ``lx.sdcflows`` (the port of sdcflows' field-map estimation, field maps part A)
with sdcflows itself, run in-process, and with fMRIPrep 25.2.6's own files.

The cases are the bit-comparison scripts of ``validation/investigations/sdcflows-fieldmap``
(each one validated in both kept environments when it was written), run by this suite so that
their results become a record and the parity tiers run them: one case per script, one check per
line it reports (``OK <name>: ...`` or ``FAIL <name>: ...``).

- ``weights``: ``bspline_grid``, ``grid_bspline_weights``, ``collocation_matrix`` against
  sdcflows (orientations, dense axes with explicit zeros, float64, single slices and rows, a
  rotated and a larger target reproduced, SDC-01 and SDC-02 corrected against an independent
  numpy construction, SDC-06, four oracle runs' field maps);
- ``ridge``: numpy's float32 dot products and norms, SciPy's ``cg``, scikit-learn's ``Ridge``
  and its messages, the B-spline fits of five synthetic and fMRIPrep's seven field maps at 1
  and 14 threads, against scikit-learn and fMRIPrep's own coefficient files;
- ``approx-core``: ``BSplineApprox``'s core on 16 synthetic cases and fMRIPrep's seven calls;
- ``bspline-approx``: ``bspline_approx`` complete (files, headers, log records, failures),
  ``resample_to_zooms``, ``rescale_affine``, and fMRIPrep's seven ``bs_filter`` calls with
  their own ``*_coeff000``/``*_extra`` files;
- ``coeffs``: ``reorient_pedir``, ``topup_coeff_reorient`` (SDC-07 both ways, ds001600's and
  ds005040's ``fix_coeff`` and their files), ``transform_coefficients``;
- ``b0field``: ``deoblique_and_zooms``, ``B0FieldTransform.fit`` reproduced (synthetic,
  ds005454's and ds005040's coefficients on their BOLD references, ds002603's oblique ones
  with its registration) and corrected (SDC-01, SDC-02, SDC-10 against sdcflows' fit with the
  corrections made in its pieces);
- ``apply``: ``B0FieldTransform.apply``, ``apply_coeffs_field``, the displacement-field
  conversions (every option and failure; ds001600, ds005040 and ds005454's series);
- ``phase``: the phase-difference preparation against sdcflows' interfaces and fMRIPrep's 21
  ``phmap2rads``, ``calc_phdiff`` and ``compfmap`` files of seven estimators, and the whole
  ``phdiff_workflow`` with ``lx.fsl.prelude``;
- ``nibabel-writer``: ``lx.io.save_nibabel`` against nibabel 5.4.2's writer.

The suite runs in the interpreter's environment (``python_versions``: ``newest`` by default)
with numpy's OpenBLAS on the Haswell kernels (skipped otherwise: ``OPENBLAS_CORETYPE=Haswell``,
as ``scripts/orchestration/env.sh`` sets it). **The originals' own step** (``python_versions``'
``STEP_SUITES``, as ``resample-series``): a run in ``newest`` first runs the same cases in the
shared environment, twice; every script passes what sdcflows, scikit-learn, SciPy, numpy or
nibabel gave on its side of a comparison through ``stepdigest.oracle``, and each reported line
carries one more check: whether those outputs are the same under both releases (the digests of
the content gathered since the previous line: arrays, decompressed files, headers, messages).
"""

from __future__ import annotations

import importlib.util
import io
import os
import re
import sys
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

#: Where the scripts are (the repository's ``validation/investigations/sdcflows-fieldmap``).
SCRIPTS = Path(__file__).resolve().parents[3] / "investigations" / "sdcflows-fieldmap"
#: case id -> (script, tier, description)
CASES = {
    "weights": (
        "check_weights.py",
        "smoke",
        "the control grid and the B-spline weights (SDC-01, SDC-02, SDC-06 both ways)",
    ),
    "coeffs": (
        "check_coeffs.py",
        "smoke",
        "TOPUPCoeffReorient (SDC-07), TransformCoefficients, reorient_pedir; fMRIPrep's fix_coeff",
    ),
    "phase": (
        "check_phase.py",
        "smoke",
        "the phase-difference preparation; fMRIPrep's phmap2rads, calc_phdiff, compfmap files",
    ),
    "nibabel-writer": ("check_nibabel_write.py", "smoke", "lx.io.save_nibabel against nibabel"),
    "b0field": (
        "check_b0field.py",
        "smoke",
        "deoblique_and_zooms, B0FieldTransform.fit reproduced and corrected (SDC-01, -02, -10)",
    ),
    "apply": (
        "check_apply.py",
        "standard",
        "B0FieldTransform.apply, ApplyCoeffsField, the displacement fields (SDC-05, SDC-08)",
    ),
    "approx-core": (
        "check_approx.py",
        "standard",
        "BSplineApprox's core: 16 synthetic cases and fMRIPrep's seven calls",
    ),
    "bspline-approx": (
        "check_bspline_approx.py",
        "standard",
        "bspline_approx complete and fMRIPrep's seven bs_filter calls with their files",
    ),
    "ridge": (
        "check_ridge.py",
        "standard",
        "numpy's dot, SciPy's cg, scikit-learn's Ridge; fMRIPrep's seven fits and coefficients",
    ),
}
_LINE = re.compile(r"^(OK|FAIL|EXPECTED)\s+(.*?)(?::\s(.*))?$")
#: The originals' changes from the shared releases to the newest on this suite's paths
#: (``NOTES``: none in sdcflows 2.15.1 -> 2.17.1 on part A's paths; the libraries' code read
#: side by side): a change the step finds has no known reason and fails its check.
CHANGES: dict[str, str] = {}


def _workspace() -> Path | None:
    for parent in Path(__file__).resolve().parents:
        if (parent / "oracles").is_dir():
            return parent
    return None


def _load(script: str):
    name = f"_sdcflows_check_{Path(script).stem}"
    if name in sys.modules:
        return sys.modules[name]
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / script)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    if str(SCRIPTS) not in sys.path:
        sys.path.insert(0, str(SCRIPTS))
    spec.loader.exec_module(module)
    return module


def cases(tier: str) -> list[Case]:
    from larmorx_validation.parity.harness import in_tier

    return [
        Case(cid, "sdcflows", desc, payload=script)
        for cid, (script, case_tier, desc) in CASES.items()
        if in_tier(case_tier, tier)
    ]


def run_case(case: Case, checks: CheckList) -> None:
    from larmorx_validation.parity.numpy_linalg import KERNELS, openblas_core

    core = openblas_core()
    if core not in KERNELS:
        raise Outcome(
            SKIPPED,
            f"numpy's OpenBLAS runs the {core} kernels; sdcflows' fit is compared on Haswell's "
            "(OPENBLAS_CORETYPE=Haswell)",
        )
    ws = _workspace()
    if ws is not None:
        os.environ.setdefault("LARMORX_WORKSPACE", str(ws))
    module = _load(case.payload)
    import stepdigest  # the scripts' own folder, on sys.path once a script is loaded

    stepdigest.reset()
    out = _Lines(stepdigest)
    old = sys.stdout
    sys.stdout = out
    try:
        status = module.main()
    finally:
        sys.stdout = old
    lines = out.getvalue().splitlines()
    n = 0
    with pv.case(case.id):
        for kind, name, detail, digest in out.reported:
            if kind != "EXPECTED":
                checks(name, kind == "OK", detail=detail[:400])
                n += 1
            if digest is not None:
                _step(checks, name, digest)
    skipped = [line for line in lines if line.startswith("skipped")]
    checks(
        "the script's exit status",
        status in (0, None),
        detail=f"{n} comparisons" + (f"; {len(skipped)} skipped: {skipped[:3]}" if skipped else ""),
    )
    if n == 0:
        raise Outcome(SKIPPED, "no comparison ran (the oracle runs or packages are missing)")


class _Lines(io.StringIO):
    """The script's output; at each reported line the digests of the originals' outputs
    gathered since the previous one (``stepdigest``) are taken for that line."""

    def __init__(self, stepdigest) -> None:
        super().__init__()
        self._digests = stepdigest
        self._partial = ""
        self._names: dict[str, int] = {}
        #: (kind, name, detail, digest or None) per reported line, in order.
        self.reported: list[tuple[str, str, str, str | None]] = []

    def write(self, s: str) -> int:
        n = super().write(s)
        self._partial += s
        while "\n" in self._partial:
            line, self._partial = self._partial.split("\n", 1)
            m = _LINE.match(line.strip())
            if m:
                name = m.group(2).strip()
                k = self._names.get(name, 0)
                self._names[name] = k + 1
                if k:  # a name reported twice: numbered, so each line keeps its own step
                    name = f"{name} #{k + 1}"
                self.reported.append((m.group(1), name, m.group(3) or "", self._digests.take()))
        return n


def _step(checks: CheckList, name: str, digest: str) -> None:
    """The originals' own step for line ``name`` (``python_versions``): recorded in a
    ``shared`` step run, compared with that record in a ``newest`` run."""
    value = np.frombuffer(bytes.fromhex(digest), dtype=np.uint8)
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
    checks.add(
        Check(
            f"{pv.STEP_CHECK}: {name}",
            not changed,
            metric="the originals' outputs",
            value="changed" if changed else "identical",
            threshold="a change has a known reason",
            detail="identical" if not changed else "changed: no known reason",
        )
    )


def _highlights(results: list[CaseResult]) -> list[str]:
    from larmorx_validation.parity.resample_series import step_summary

    lines = step_summary(results)
    stepped = sum(1 for r in results for c in r.checks if c.name.startswith(pv.STEP_CHECK))
    if stepped:
        lines.append(
            f"The step's comparisons: {stepped} reported lines whose originals' outputs were "
            f"compared under both releases ({pv.RELEASES[pv.SHARED]}; {pv.RELEASES[pv.NEWEST]}; "
            "sdcflows 2.17.1, scikit-learn 1.9.1 and nibabel 5.4.2 in both)."
        )
    return lines


def suite() -> Suite:
    import numpy

    try:
        import sdcflows

        sdc = sdcflows.__version__
    except ImportError:  # pragma: no cover - reported by the harness
        sdc = "missing"
    return Suite(
        name="sdcflows-fieldmap",
        title="sdcflows' field-map estimation (B-spline fit, field, phase-difference preparation)",
        tool="`lx.sdcflows` (`crates/larmorx-sdcflows`), `lx.io.save_nibabel`",
        reference=(
            f"sdcflows {sdc} with scikit-learn, SciPy and numpy {numpy.__version__} in-process "
            "(numpy's OpenBLAS on the Haswell kernels), nibabel 5.4.2, and fMRIPrep 25.2.6's own "
            "files of the oracle runs (ds001600, ds002422, ds002603, ds005040, ds005454)"
        ),
        thresholds=[
            ("Every array, file (decompressed), header, warning and message", "bit-identical"),
            (
                "upstream_defects='reproduce'",
                "sdcflows' result bit for bit, its errors and warnings",
            ),
            (
                "Corrected (SDC-01, -02, -10)",
                "bit-identical to sdcflows' own construction with the correction made in its "
                "pieces (measure_sdc01_02.py)",
            ),
        ],
        cases=cases,
        run_case=run_case,
        packages=("numpy", "scipy", "scikit-learn", "nibabel", "nitransforms", "sdcflows"),
        highlights=_highlights,
        notes=(
            "Each case is one of the investigation's check scripts, run in this process; one "
            "check per line it reports. The originals' own step (python_versions): every line "
            "whose script passed the originals' outputs to stepdigest.oracle carries one more "
            "check, whether those outputs are identical in the shared releases (two runs) and "
            "the newest."
        ),
    )
