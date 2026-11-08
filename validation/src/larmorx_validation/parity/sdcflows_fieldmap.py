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
    if isinstance(case.payload, tuple) and case.payload[0] == "random":
        with pv.case(case.id):
            _random_case(case.payload[1], checks)
        return
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


# ------------------------------------------------------------------------------------------------
# Random testing's cases (difftest/tools/sdcflows_fieldmap.py) and their permanent cases


def _random_case(spec: dict, checks: CheckList) -> None:
    """One drawn call: sdcflows against larmorx's ``reproduce`` through the investigation's
    comparators (one line each, every output bit for bit), the originals' step, and the D13 fix
    pass (larmorx's default against ``reproduce``)."""
    import tempfile

    from larmorx_validation.difftest import inputs

    recipe = spec["recipe"]
    e = recipe["extra"]
    call = e["call"]
    files = {name: str(inputs.path(k)) for name, k in spec["files"].items()}
    _load("check_weights.py")
    import stepdigest

    with tempfile.TemporaryDirectory(prefix="lx-sdcflows-dt_") as t:
        tmp = Path(t)
        stepdigest.reset()
        out = _Lines(stepdigest)
        old = sys.stdout
        sys.stdout = out
        try:
            fix_inputs = _compare(call, e, files, tmp)
        finally:
            sys.stdout = old
        for kind, name, detail, digest in out.reported:
            ok = kind == "OK"
            checks(
                f"{name}: every output bit-identical",
                ok,
                value=0 if ok else 1,
                detail=detail[:400],
            )
            if digest is not None:
                _step(checks, name, digest)
        if not out.reported:
            raise Outcome(SKIPPED, f"the comparator of {call} reported nothing")
        if fix_inputs is not None:
            _fix_pass(call, e, fix_inputs, checks)


def _rigid(x: dict | None):
    if x is None:
        return None
    from larmorx_validation.difftest import gen

    m = np.eye(4)
    m[:3, :3] = gen.rotation(x["quaternion"])
    m[:3, 3] = x["translation"]
    return m


def _coefficient_files(fmap: str, c: dict, tmp: Path) -> list[str]:
    """The coefficients of the recipe on the field map's control grids (larmorx's grids,
    sdcflows' bit for bit), seeded values within the amplitude, written as float32 files."""
    import larmorx as lx

    out = []
    for i, sp in enumerate(c["spacings"]):
        g = lx.sdcflows.bspline_grid(fmap, sp)
        rng = np.random.default_rng([int(c["seed"]), i])
        data = (rng.uniform(-1.0, 1.0, tuple(g.shape)) * c["amplitude"]).astype(np.float32)
        path = tmp / f"lvl{i}_coeff000.nii.gz"
        lx.save((np.asfortranarray(data), np.asarray(g.affine)), path)
        out.append(str(path))
    return out


def _ro_time(value):
    return 1 if value == "int 1" else value


def _compare(call: str, e: dict, files: dict[str, str], tmp: Path) -> dict | None:
    """Runs the comparator of ``call``; returns what the fix pass needs (``None``: no switch)."""
    if call == "bspline_approx":
        m = _load("check_bspline_approx.py")
        params = dict(e["params"])
        params["bs_spacing"] = [tuple(s) for s in params["bs_spacing"]]
        if isinstance(params["zooms_min"], list):
            params["zooms_min"] = tuple(params["zooms_min"])
        mask = files.get("mask")
        m.case("bspline_approx", files["fmap"], mask, mine_kw={"upstream_defects": "reproduce"},
               **params)  # fmt: skip
        return {"args": (files["fmap"], mask), "kw": params}
    if call in ("fit", "apply"):
        coeffs = _coefficient_files(files["fmap"], e["coefficients"], tmp)
        xfm = _rigid(e.get("xfm"))
        if call == "fit":
            m = _load("check_b0field.py")
            m.fit_case("fit", coeffs, files["moving"], xfm, e["approx"])
            return {"coeffs": coeffs, "target": files["moving"], "xfm": xfm, "approx": e["approx"]}
        m = _load("check_apply.py")
        kw = dict(e["kw"])
        kw.update(xfm_data2fmap=xfm, approx=e["approx"])
        ro = _ro_time(e["ro_time"])
        m.apply_case("apply", coeffs, files["moving"], e["pe_dir"], ro, **kw)
        return {"coeffs": coeffs, "moving": files["moving"], "pe": e["pe_dir"], "ro": ro, "kw": kw}
    if call == "topup":
        m = _load("check_coeffs.py")
        coeff = m.topup_coeff(tmp / "coef_fieldcoef.nii.gz", files["ref"],
                              tuple(e["factors"]), e["wrong_shape"])  # fmt: skip
        m.topup_case("topup", [coeff], files["ref"], e["pe_dir"])
        return {"coeffs": [str(coeff)], "ref": files["ref"], "pe": e["pe_dir"]}
    if call == "displacements":
        m = _load("check_apply.py")
        epi = files.get("epi", files["fmap"])
        m.disp_case("displacements", files["fmap"], epi, e["ro_time"], e["pe_dir"], e["itk"])
        return None
    # phase
    import larmorx as lx

    m = _load("check_phase.py")
    m.rads_case("PhaseMap2rads", files["phase1"], upstream_defects="reproduce")
    if not e.get("subtract"):
        return {"phase1": files["phase1"]}
    try:
        rads = [lx.sdcflows.au2rads(files[k], out_dir=tmp) for k in ("phase1", "phase2")]
    except ValueError:  # a constant phase map (SDC-04 corrected): compared above
        return {"phase1": files["phase1"]}
    metas = [{} if te is None else {"EchoTime": te} for te in e["echo_times"]]
    phases = [str(r.out_file) for r in rads]
    m.sub_case("SubtractPhases", phases, metas, upstream_defects="reproduce")
    return {"phase1": files["phase1"], "phases": phases, "metas": metas}


def _larmorx_outputs(call: str, f: dict, mode: str) -> tuple[list[np.ndarray] | str, list[str]]:
    """larmorx's outputs of the call in ``mode`` (or its error's text) and the defects it names
    (its notices, and the IDs its error message names)."""
    import warnings

    import larmorx as lx
    from larmorx_validation.parity import defects

    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        try:
            if call == "bspline_approx":
                r = lx.sdcflows.bspline_approx(*f["args"], **f["kw"], upstream_defects=mode)
                arrays = [r.field.data, r.error.data, *(c.data for c in r.coefficients)]
                if r.extrapolated is not None:
                    arrays.append(r.extrapolated.data)
            elif call == "fit":
                t = lx.sdcflows.B0FieldTransform(coeffs=f["coeffs"])
                t.fit(f["target"], f["xfm"], f["approx"], upstream_defects=mode)
                arrays = [t.mapped.data]
            elif call == "apply":
                # The unwarped series and the field it used (``mapped``, which
                # ``ApplyCoeffsField`` writes as ``<in>_field``).
                t = lx.sdcflows.B0FieldTransform(coeffs=f["coeffs"])
                arrays = [t.apply(f["moving"], f["pe"], f["ro"], **f["kw"],
                                  upstream_defects=mode).data]  # fmt: skip
                assert t.mapped is not None
                arrays.append(t.mapped.data)
            elif call == "topup":
                r = lx.sdcflows.topup_coeff_reorient(f["coeffs"], f["ref"], f["pe"],
                                                     upstream_defects=mode)  # fmt: skip
                arrays = [c.data for c in r.coefficients]
            else:
                arrays = [lx.sdcflows.au2rads(f["phase1"], upstream_defects=mode).image.data]
                if "phases" in f:
                    arrays.append(
                        lx.sdcflows.subtract_phases(
                            f["phases"], f["metas"], upstream_defects=mode
                        ).image.data
                    )
        except Exception as exc:  # the error is the result
            text = f"{type(exc).__name__}: {exc}"
            named = re.findall(r"\b(SDC-\d\d)\b", text)
            return text, list(dict.fromkeys([*defects.warning_entries(w), *named]))
    return arrays, defects.warning_entries(w)


def _fix_pass(call: str, e: dict, f: dict, checks: CheckList) -> None:
    from larmorx_validation.parity import defects

    fixed, entries = _larmorx_outputs(call, f, "fix")
    reproduced, _ = _larmorx_outputs(call, f, "reproduce")
    other = ""
    change = defects.Change(0, 0, 0.0)
    if isinstance(fixed, str) or isinstance(reproduced, str):
        if fixed != reproduced:
            other = f"fix: {str(fixed)[:120]}; reproduce: {str(reproduced)[:120]}"
    elif len(fixed) != len(reproduced):
        other = f"{len(fixed)} outputs in fix, {len(reproduced)} in reproduce"
    else:
        for a, b in zip(fixed, reproduced, strict=True):
            change = change + defects.Change.of(a, b)
    # SCI-02 (lx.ndimage) is named when the spline prefilter pads an integer series with a cval
    # its type cannot hold, whether or not a sample point then reaches the padding: a condition
    # whose effect the comparison may not see.
    silent = ("SCI-02",) if call == "apply" else ()
    defects.add_fix_check(checks, entries, change, other_changes=other, silent_ok=silent)


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
