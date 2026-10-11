# SPDX-License-Identifier: Apache-2.0
"""Parity of ``larmorx afni 3dTshift`` with AFNI 25.2.09's ``3dTshift``.

Both programs get the same arguments and files. AFNI runs as its own binary (built by
``scripts/build_afni_oracle.sh``; set ``LARMORX_AFNI_BIN`` to use another); larmorx runs
in-process through its Python console entry point, with the implementation forced
(``larmorx.cli.run(..., implementation="original")``). The outputs are read with nibabel, a
reader independent of both, and compared value by value and field by field.

``suite("replica")`` runs the same cases against the GPL-3.0-or-later replica
(``larmorx-gpl afni 3dTshift``, ``crates-gpl/``) instead, the way users reach it: through
larmorx's own dispatch (``implementation="replica"``), which runs the replica's program as a
separate process. This package is Apache-2.0 and never loads the replica. The program under
test is ``LARMORX_GPL_BIN``, else the ``crates-gpl`` build, else ``larmorx-gpl`` on the PATH.

Every case the Python API can express (a ``-tpattern`` name, fMRIPrep's slice timing, the
options in seconds) also runs through ``lx.afni.tshift(..., implementation=...)``, from the file
and, for unscaled uint8, int16 and float32 inputs, from memory; the results must be the
command line's output, bit for bit.

Cases cover (``specs/3dTshift.md`` §9):
- every file of ``larmorx-testdata`` ``synthetic/tshift/`` (every FFT length class and every
  data type AFNI reads);
- every interpolation method;
- ``-ignore``, ``-tzero``, ``-slice``, ``-rlt``, ``-rlt+``, ``-no_detrend``, ``-TR`` units and
  every ``-tpattern`` form;
- slice timing from NIfTI headers, and the "copy of input" cases;
- the errors both programs must raise;
- real BOLD runs, called as fMRIPrep calls 3dTshift (``SliceTiming`` from the BIDS sidecar).
"""

from __future__ import annotations

import contextlib
import functools
import io
import json
import os
import re
import shutil
import subprocess
import tempfile
import warnings
from dataclasses import dataclass
from pathlib import Path

import larmorx_testdata as td
import numpy as np

from larmorx_validation.parity.harness import (
    BOTH_ERROR,
    EXPECTED,
    PASS,
    Case,
    CaseResult,
    Check,
    CheckList,
    Outcome,
    Suite,
)
from larmorx_validation.report import table

#: specs/3dTshift.md §9 (PLAN.md §11.3, slice timing as deterministic arithmetic).
RTOL = 1e-5
INT_ATOL = 1
#: Geometry is written from the same float32 header values: compare affines to 1e-6 mm.
AFFINE_ATOL = 1e-6

ORACLE = "oracles/afni-25.2.09/bin/3dTshift"
S = "synthetic/tshift"
F32 = f"{S}/dtypes/float32.nii.gz"
I16 = f"{S}/dtypes/int16.nii.gz"
METHODS = ("-Fourier", "-linear", "-cubic", "-quintic", "-heptic", "-wsinc5", "-wsinc9")
PATTERNS = (
    "alt+z",
    "altplus",
    "alt+z2",
    "alt-z",
    "altminus",
    "alt-z2",
    "seq+z",
    "seqplus",
    "seq-z",
    "seqminus",
    "zero",
    "simult",
)


@functools.cache
def afni_binary() -> str | None:
    """The AFNI ``3dTshift`` oracle: ``LARMORX_AFNI_BIN``, else the workspace's
    ``oracles/afni-25.2.09`` build, else ``3dTshift`` on the PATH."""
    if env := os.environ.get("LARMORX_AFNI_BIN"):
        path = Path(env)
        return str(path / "3dTshift" if path.is_dir() else path)
    for parent in Path(__file__).resolve().parents:
        candidate = parent / ORACLE
        if candidate.is_file():
            return str(candidate)
    return shutil.which("3dTshift")


@functools.cache
def afni_version() -> str:
    binary = afni_binary()
    if binary is None:
        return "not found"
    out = subprocess.run([binary, "-help"], capture_output=True, text=True, encoding="utf-8")
    match = re.search(r"AFNI_[0-9.]+", out.stdout + out.stderr)
    return match.group(0) if match else "unknown version"


# ------------------------------------------------------------------------------------------------
# Scenarios


@dataclass(frozen=True)
class Scenario:
    """One 3dTshift invocation.

    ``image`` is a catalog path, or ``gen:<name>`` for an input generated from a catalog file
    (see :func:`_generated`). In ``args``, ``{tmp}`` is the case's temporary directory.
    ``sidecar`` names a BIDS JSON whose ``SliceTiming`` is passed as fMRIPrep passes it.
    """

    image: str
    args: tuple[str, ...] = ()
    tier: str = "smoke"
    expect: str = "pass"  # "pass", "error" (both reject) or "divergence"
    reason: str = ""
    sidecar: str | None = None
    suffix: str = ".nii"
    files: tuple[tuple[str, str], ...] = ()  # (name, text) written to {tmp} first


def _scenarios() -> list[tuple[str, str, str, Scenario]]:
    out: list[tuple[str, str, str, Scenario]] = []

    def add(cid: str, category: str, desc: str, s: Scenario) -> None:
        out.append((cid, category, desc, s))

    synthetic = sorted(e.path for e in td.select(tags={"tshift"}) if e.path.startswith(f"{S}/"))
    for path in synthetic:
        name = path.removeprefix(f"{S}/").removesuffix(".nii.gz")
        add(
            f"synthetic/{name}",
            "synthetic",
            f"default Fourier, -tpattern alt+z: {td.catalog()[path].description}",
            Scenario(path, ("-tpattern", "alt+z")),
        )
    for path in synthetic:
        if "/lengths/" in path:
            continue
        name = path.removeprefix(f"{S}/").removesuffix(".nii.gz")
        for method in METHODS[1:]:
            add(
                f"methods/{name}{method}",
                "methods",
                f"{method}, -tpattern alt-z, on {name}",
                Scenario(path, ("-tpattern", "alt-z", method)),
            )

    # Options, on float32 and int16 data.
    options = [
        ("ignore", ("-ignore", "3"), "-ignore 3: three leading points kept out of fit and shift"),
        ("tzero-0", ("-tzero", "0"), "-tzero 0: every slice moved to the start of the TR"),
        ("tzero-0.5", ("-tzero", "0.5"), "-tzero 0.5"),
        ("tzero-beyond-tr", ("-tzero", "5"), "-tzero 5: beyond the TR (allowed)"),
        ("slice-2", ("-slice", "2"), "-slice 2: align to slice 2's time"),
        ("slice-wins", ("-tzero", "0.5", "-slice", "1"), "-tzero and -slice: -slice wins"),
        ("rlt", ("-rlt",), "-rlt: no trend added back"),
        ("rlt+", ("-rlt+",), "-rlt+: only the intercept added back"),
        ("rlt-last-wins", ("-rlt", "-rlt+"), "-rlt then -rlt+: the last wins"),
        ("no-detrend", ("-heptic", "-no_detrend"), "-heptic -no_detrend: mean removed only"),
        (
            "no-detrend-fourier",
            ("-quintic", "-no_detrend", "-Fourier"),
            "-no_detrend then -Fourier (allowed, with a warning)",
        ),
        ("TR-seconds", ("-TR", "2.5s"), "-TR 2.5s"),
        ("TR-plain", ("-TR", "1.7"), "-TR 1.7 (no unit)"),
        ("TR-ms", ("-TR", "2000ms"), "-TR 2000ms: every time in milliseconds"),
        ("TR-msec", ("-TR", "2000msec", "-tzero", "500"), "-TR 2000msec -tzero 500"),
        ("verbose", ("-verbose",), "-verbose"),
        ("abbreviations", ("-cub", "-verb"), "-cub and -verb: AFNI's abbreviations"),
        ("wsinc-case", ("-WSINC5",), "-WSINC5: case-insensitive"),
        ("ignore-heptic", ("-ignore", "5", "-heptic"), "-ignore 5 with -heptic"),
        ("rlt-wsinc9", ("-rlt", "-wsinc9"), "-rlt with -wsinc9"),
        ("rlt+-linear", ("-rlt+", "-linear"), "-rlt+ with -linear"),
    ]
    for image, tag in [(F32, "float32"), (I16, "int16")]:
        for cid, args, desc in options:
            add(
                f"options/{tag}-{cid}",
                "options",
                f"{desc} ({tag})",
                Scenario(image, ("-tpattern", "alt+z", *args)),
            )

    # Every -tpattern form.
    for pattern in PATTERNS:
        add(
            f"tpattern/{pattern}",
            "tpattern",
            f"-tpattern {pattern}",
            Scenario(F32, ("-tpattern", pattern)),
        )
    files = {
        "file-tabs": ("0.0\t1.0\t0.5\t1.5\n", "one line, tab-separated (as fMRIPrep writes it)"),
        "file-lines": ("0\n1.5\n0.5\n1\n", "one value per line"),
        "file-matrix": ("0 9\n1.5 9\n0.5 9\n1 9\n", "two columns: read column by column"),
        "file-comments": ("# slice times\n0, 1.5, 0.25, 1.75 # end\n", "comments and commas"),
        "file-extra": ("0 1 0.5 1.5 0.25 1.25\n", "more values than slices"),
        "file-repeat": ("2@0 2@1\n", "N@value repetition"),
        "file-tr": ("0 2 1 0.5\n", "a value equal to the TR"),
    }
    for cid, (text, desc) in files.items():
        add(
            f"tpattern/{cid}",
            "tpattern",
            f"-tpattern @file: {desc}",
            Scenario(F32, ("-tpattern", "@{tmp}/st.1D"), files=(("st.1D", text),)),
        )
    add(
        "tpattern/inline-1D",
        "tpattern",
        "-tpattern '@1D: 0 1 0.5 1.5'",
        Scenario(F32, ("-tpattern", "@1D: 0 1 0.5 1.5")),
    )

    # Slice timing from the NIfTI header (inputs generated from the float32 file).
    header = [
        ("code1-seq-inc", "slice_code 1 (sequential increasing)"),
        ("code2-seq-dec", "slice_code 2: AFNI gives every slice time 0"),
        ("code3-alt-inc", "slice_code 3 (alternating increasing)"),
        ("code4-alt-dec", "slice_code 4 (alternating decreasing)"),
        ("code5-alt-inc2", "slice_code 5 (alternating increasing, from slice 1)"),
        ("code6-alt-dec2", "slice_code 6 (alternating decreasing, from nz-2)"),
        ("code3-partial", "slice_code 3 over slices 1..2 only"),
        ("code3-ms", "slice_code 3 with the TR and duration in milliseconds"),
        ("code3-beyond-tr", "times beyond the TR: a copy of the input, timing kept"),
        ("code3-no-slice-dim", "no slice axis in dim_info: no timing, a copy"),
        ("code3-zero-duration", "slice_duration 0: no timing, a copy"),
    ]
    for name, desc in header:
        add(
            f"header-timing/{name}",
            "header-timing",
            f"no -tpattern, {desc}",
            Scenario(f"gen:{name}", ("-cubic",)),
        )
    add(
        "header-timing/tpattern-overrides",
        "header-timing",
        "-tpattern overrides the header's timing",
        Scenario("gen:code3-alt-inc", ("-tpattern", "seq-z")),
    )

    # Copies of the input.
    for path, desc in [
        (I16, "int16"),
        (f"{S}/dtypes/int16-slope.nii.gz", "int16 with a brick factor"),
        (f"{S}/dtypes/int16-slope-inter.nii.gz", "int16 scaled to float32"),
        (f"{S}/dtypes/float64.nii.gz", "float64 converted to float32"),
        (f"{S}/dtypes/float32-nonfinite.nii.gz", "non-finite floats zeroed"),
    ]:
        name = path.split("/")[-1].removesuffix(".nii.gz")
        add(
            f"copy/no-timing-{name}",
            "copy",
            f"no slice timing anywhere: the output is a copy of the input ({desc})",
            Scenario(path, ()),
        )
    add(
        "copy/3d-input",
        "copy",
        "a 3D image: one value per voxel, written back as 3D",
        Scenario("synthetic/resampling/images/axial-1mm.nii.gz", ("-tpattern", "alt+z")),
    )

    # Errors both must raise.
    errors = [
        ("unknown-option", ("-bogus",), "an unknown option"),
        ("no-detrend-fourier", ("-no_detrend",), "-no_detrend while the method is Fourier"),
        ("rlt-no-detrend", ("-linear", "-rlt", "-no_detrend"), "-rlt with -no_detrend"),
        ("unknown-pattern", ("-tpattern", "ALT+Z"), "pattern names are case-sensitive"),
        ("slice-too-large", ("-tpattern", "alt+z", "-slice", "4"), "-slice beyond the last"),
        ("ignore-too-large", ("-tpattern", "alt+z", "-ignore", "96"), "-ignore > nt - 5"),
        ("negative-tzero", ("-tpattern", "alt+z", "-tzero", "-1"), "-tzero < 0"),
        ("negative-ignore", ("-tpattern", "alt+z", "-ignore", "-1"), "-ignore < 0"),
        ("zero-TR", ("-tpattern", "alt+z", "-TR", "0"), "-TR 0"),
        ("bad-TR", ("-tpattern", "alt+z", "-TR", "abc"), "-TR abc"),
        ("missing-file", ("-tpattern", "@{tmp}/none.1D"), "a missing -tpattern file"),
    ]
    for cid, args, desc in errors:
        add(f"errors/{cid}", "errors", desc, Scenario(F32, args, expect="error"))
    for cid, text, desc in [
        ("file-too-short", "0 1 0.5\n", "a -tpattern file with fewer values than slices"),
        ("file-beyond-tr", "0 1 0.5 2.5\n", "a -tpattern value beyond the TR"),
        ("file-negative", "0 1 -0.5 1.5\n", "a negative -tpattern value"),
        ("file-text", "0 1 abc 1.5\n", "a -tpattern file with text"),
    ]:
        add(
            f"errors/{cid}",
            "errors",
            desc,
            Scenario(F32, ("-tpattern", "@{tmp}/st.1D"), expect="error", files=(("st.1D", text),)),
        )
    add(
        "errors/missing-dataset",
        "errors",
        "an input file that does not exist",
        Scenario("missing", ("-tpattern", "alt+z"), expect="error"),
    )
    add(
        "errors/existing-output",
        "errors",
        "an output that exists already (AFNI exits 0 without writing; larmorx exits 1)",
        Scenario(F32, ("-tpattern", "alt+z"), expect="error", files=(("out.nii", "taken"),)),
    )

    # Deliberate differences.
    add(
        "divergence/afni-format-output",
        "divergence",
        "-prefix without .nii: AFNI writes its own BRIK/HEAD format",
        Scenario(
            F32,
            ("-tpattern", "alt+z"),
            expect="divergence",
            suffix="",
            reason="larmorx writes NIfTI only; it rejects a -prefix that does not end in "
            ".nii or .nii.gz, where AFNI writes its own format.",
        ),
    )
    add(
        "divergence/voxshift",
        "divergence",
        "-voxshift: per-voxel shifts from a dataset",
        Scenario(
            F32,
            ("-voxshift", "gen:voxshift"),
            expect="divergence",
            reason="-voxshift is not supported by larmorx (fMRIPrep does not use it).",
        ),
    )

    # Real BOLD runs, as fMRIPrep calls 3dTshift.
    o = "openneuro"
    real = [
        ("ds001600-acq-v4", f"{o}/ds001600/sub-1/func/sub-1_task-rest_acq-v4_bold", None, "smoke"),
        (
            "ds001600-acq-v1",
            f"{o}/ds001600/sub-1/func/sub-1_task-rest_acq-v1_bold",
            None,
            "standard",
        ),
        (
            "ds001600-acq-v2",
            f"{o}/ds001600/sub-1/func/sub-1_task-rest_acq-v2_bold",
            None,
            "standard",
        ),
        (
            "ds001600-acq-PA",
            f"{o}/ds001600/sub-1/func/sub-1_task-rest_acq-PA_bold",
            None,
            "standard",
        ),
        *(
            (
                f"ds000210-echo-{e}",
                f"{o}/ds000210/sub-02/func/sub-02_task-cuedSGT_run-01_echo-{e}_bold",
                f"{o}/ds000210/task-cuedSGT_echo-{e}_bold.json",
                "standard",
            )
            for e in (1, 2, 3)
        ),
        ("ds006736", f"{o}/ds006736/sub-004/func/sub-004_task-freeRecall_bold", None, "standard"),
        (
            "ds006010-uint16",
            f"{o}/ds006010/sub-206/func/sub-206_task-category_run-01_bold",
            None,
            "standard",
        ),
        (
            "ds003345-ms-header",
            f"{o}/ds003345/sub-22973/func/sub-22973_task-PenaltyKik_run-02_bold",
            None,
            "standard",
        ),
        (
            "ds005454-mb4-96-slices",
            f"{o}/ds005454/sub-16/func/sub-16_task-rest_bold",
            None,
            "standard",
        ),
    ]
    for cid, stem, sidecar, tier in real:
        add(
            f"real/{cid}",
            "real",
            f"fMRIPrep's call (-ignore 0 -tzero -TR -tpattern @file) on {stem.split('/')[-1]}",
            Scenario(
                f"{stem}.nii.gz",
                ("-ignore", "0"),
                tier=tier,
                sidecar=sidecar or f"{stem}.json",
                suffix=".nii.gz",
            ),
        )
    d3345 = f"{o}/ds003345/sub-22973/func/sub-22973_task-PenaltyKik_run-02_bold"
    for method in METHODS[1:]:
        add(
            f"real/ds003345{method}",
            "real",
            f"fMRIPrep's call with {method}",
            Scenario(
                f"{d3345}.nii.gz",
                ("-ignore", "0", method),
                tier="standard",
                sidecar=f"{d3345}.json",
                suffix=".nii.gz",
            ),
        )
    add(
        "real/ds003345-ignore-4",
        "real",
        "fMRIPrep's call with -ignore 4 (non-steady-state volumes)",
        Scenario(
            f"{d3345}.nii.gz",
            ("-ignore", "4"),
            tier="standard",
            sidecar=f"{d3345}.json",
            suffix=".nii.gz",
        ),
    )
    d210 = f"{o}/ds000210/sub-02/func/sub-02_task-cuedSGT_run-01_echo-1_bold.nii.gz"
    add(
        "real/ds000210-header-timing",
        "real",
        "no -tpattern: the slice timing in the header (slice_code 3, ALT_INC)",
        Scenario(d210, (), tier="standard", suffix=".nii.gz"),
    )
    add(
        "real/ds001600-header-zero-duration",
        "real",
        "no -tpattern: slice_code 5 with slice_duration 0, so a copy of the input",
        Scenario(f"{o}/ds001600/sub-1/func/sub-1_task-rest_acq-v4_bold.nii.gz", ()),
    )
    return out


_TIERS = ("smoke", "standard", "full")


def cases(tier: str) -> list[Case]:
    rank = _TIERS.index(tier)
    return [
        Case(cid, category, desc, scenario)
        for cid, category, desc, scenario in _scenarios()
        if _TIERS.index(scenario.tier) <= rank
    ]


# ------------------------------------------------------------------------------------------------
# Generated inputs


@functools.cache
def _generated_dir() -> Path:
    return Path(tempfile.mkdtemp(prefix="lx-tshift-gen-"))


def _generated(name: str) -> Path:
    """A copy of the float32 file (nz = 4, TR = 2 s) with slice timing in its header, or
    (``voxshift``) a 3D map of 0.25-TR shifts on its grid."""
    import nibabel as nib

    path = _generated_dir() / f"{name}.nii"
    if path.exists():
        return path
    src = nib.load(td.get(F32))
    data = np.asanyarray(src.dataobj)
    if name == "voxshift":
        nib.Nifti1Image(np.full(data.shape[:3], 0.25, np.float32), src.affine).to_filename(path)
        return path
    img = nib.Nifti1Image(data, src.affine, src.header)
    h = img.header
    code = int(name[4])
    h["slice_code"] = code
    h["slice_start"], h["slice_end"] = 0, 3
    h["slice_duration"] = 0.5
    h.set_dim_info(slice=2)
    if name.endswith("partial"):
        h["slice_start"], h["slice_end"] = 1, 2
    elif name.endswith("ms"):
        h.set_xyzt_units("mm", "msec")
        h["pixdim"][4] = 2000.0
        h["slice_duration"] = 500.0
    elif name.endswith("beyond-tr"):
        h["slice_duration"] = 0.7
    elif name.endswith("no-slice-dim"):
        h["dim_info"] = 0
    elif name.endswith("zero-duration"):
        h["slice_duration"] = 0.0
    img.to_filename(path)
    return path


def _image_path(image: str) -> Path:
    if image.startswith("gen:"):
        return _generated(image.removeprefix("gen:"))
    if image == "missing":
        return _generated_dir() / "does-not-exist.nii"
    return td.get(image)


# ------------------------------------------------------------------------------------------------
# Running


def _fmriprep_args(sidecar: str, tmp: Path) -> list[str]:
    """``-tzero``, ``-TR`` and ``-tpattern`` as fMRIPrep (nipype's TShift) builds them."""
    meta = json.loads(td.get(sidecar).read_text(encoding="utf-8"))
    times = list(meta["SliceTiming"])
    if str(meta.get("SliceEncodingDirection", "")).endswith("-"):
        times = times[::-1]
    t0 = round(min(times) + 0.5 * (max(times) - min(times)), 3)
    pattern = tmp / "slice_timing.1D"
    pattern.write_text("\t".join(str(float(t)) for t in times) + "\n", encoding="utf-8")
    return ["-tzero", str(t0), "-TR", f"{meta['RepetitionTime']}s", "-tpattern", f"@{pattern}"]


def _args(s: Scenario, tmp: Path) -> list[str]:
    args = [
        str(_generated(a.removeprefix("gen:")))
        if a.startswith("gen:")
        else a.replace("{tmp}", str(tmp))
        for a in s.args
    ]
    if s.sidecar:
        args += _fmriprep_args(s.sidecar, tmp)
    return args


def _run_afni(args: list[str]) -> tuple[int, str]:
    env = {**os.environ, "AFNI_DONT_LOGFILE": "YES", "AFNI_NIFTI_TYPE_WARN": "NO"}
    p = subprocess.run(
        [afni_binary(), *args], capture_output=True, text=True, encoding="utf-8", env=env
    )
    return p.returncode, (p.stdout + p.stderr).strip()


def _run_larmorx(args: list[str], implementation: str = "original") -> tuple[int, str]:
    """``larmorx afni 3dTshift`` in process, with the implementation forced: the original, or
    the replica's program, which larmorx runs as a separate process."""
    from larmorx.cli import run

    err = io.StringIO()
    with contextlib.redirect_stderr(err), contextlib.redirect_stdout(io.StringIO()):
        code = run(["larmorx", "afni", "3dTshift", *args], implementation=implementation)
    return code, err.getvalue().strip()


@contextlib.contextmanager
def _replica_program(implementation: str):
    """For the replica, larmorx's dispatch finds the program under test through
    ``LARMORX_GPL_BIN`` (:func:`replica_binary`)."""
    if implementation != "replica":
        yield
        return
    previous = os.environ.get("LARMORX_GPL_BIN")
    os.environ["LARMORX_GPL_BIN"] = str(replica_binary())
    try:
        yield
    finally:
        if previous is None:
            os.environ.pop("LARMORX_GPL_BIN", None)
        else:
            os.environ["LARMORX_GPL_BIN"] = previous


REPLICA = "crates-gpl/target/{profile}/larmorx-gpl{exe}"


@functools.cache
def replica_binary() -> str | None:
    """The ``larmorx-gpl`` binary (the GPL-3.0-or-later replica): ``LARMORX_GPL_BIN``, else
    the release (then debug) build of ``crates-gpl`` in this repository, else ``larmorx-gpl``
    on the PATH.

    The validation package is Apache-2.0: it only runs the replica as a separate process.
    """
    if env := os.environ.get("LARMORX_GPL_BIN"):
        return env
    exe = ".exe" if os.name == "nt" else ""
    for profile in ("release", "debug"):
        for parent in Path(__file__).resolve().parents:
            candidate = parent / REPLICA.format(profile=profile, exe=exe)
            if candidate.is_file():
                return str(candidate)
    return shutil.which("larmorx-gpl")


@functools.cache
def replica_version() -> str:
    binary = replica_binary()
    if binary is None:
        return "not found"
    out = subprocess.run([binary, "--version"], capture_output=True, text=True, encoding="utf-8")
    return out.stdout.strip() or "unknown version"


def _last_error(log: str, tmp: Path) -> str:
    """The last error line of a log, without colour codes or temporary paths."""
    lines = [ln for ln in log.splitlines() if "ERROR" in ln]
    line = lines[-1] if lines else (log.splitlines()[-1] if log else "")
    line = line.replace(str(tmp), "<tmp>").replace(str(_generated_dir()), "<generated>")
    return re.sub(r"\x1b\[[0-9;]*m", "", line)


def _ulps(a: np.ndarray, b: np.ndarray) -> int:
    if a.dtype != np.float32:
        return 0
    ia = a.view(np.int32).astype(np.int64)
    ib = b.view(np.int32).astype(np.int64)
    ia = np.where(ia < 0, np.iinfo(np.int32).min - ia, ia)
    ib = np.where(ib < 0, np.iinfo(np.int32).min - ib, ib)
    return int(np.abs(ia - ib).max()) if a.size else 0


HEADER_FIELDS = (
    "dim",
    "scl_slope",
    "scl_inter",
    "toffset",
    "xyzt_units",
    "dim_info",
    "slice_code",
    "slice_start",
    "slice_end",
    "slice_duration",
    "qform_code",
    "sform_code",
)


def _compare(checks: CheckList, a_path: Path, l_path: Path) -> None:
    import nibabel as nib

    a_img, l_img = nib.load(a_path), nib.load(l_path)
    a = np.asanyarray(a_img.dataobj.get_unscaled())
    b = np.asanyarray(l_img.dataobj.get_unscaled())
    checks(
        "data type",
        a.dtype == b.dtype,
        metric="dtype",
        value=str(b.dtype),
        threshold=str(a.dtype),
        detail=f"AFNI {a.dtype}, larmorx {b.dtype}",
    )
    checks("shape", a.shape == b.shape, detail=f"AFNI {a.shape}, larmorx {b.shape}")
    ha, hb = a_img.header, l_img.header
    differing = [
        f"{k}: AFNI {np.asarray(ha[k]).tolist()}, larmorx {np.asarray(hb[k]).tolist()}"
        for k in HEADER_FIELDS
        if not np.array_equal(np.asarray(ha[k]), np.asarray(hb[k]), equal_nan=True)
    ]
    checks(
        "header fields (scaling, timing, slice fields, units, xform codes)",
        not differing,
        detail="; ".join(differing),
    )
    tr_a, tr_b = float(ha["pixdim"][4]), float(hb["pixdim"][4])
    checks("TR (pixdim[4])", tr_a == tr_b, detail=f"AFNI {tr_a}, larmorx {tr_b}")
    for name, fa, fb in [
        ("sform", ha.get_sform(), hb.get_sform()),
        ("qform", ha.get_qform(), hb.get_qform()),
    ]:
        diff = float(np.abs(fa - fb).max())
        checks(
            f"geometry ({name})",
            diff <= AFFINE_ATOL,
            metric="max |diff| (mm)",
            value=f"{diff:.1e}",
            threshold=f"{AFFINE_ATOL:g}",
        )
    voxel_sizes = float(np.abs(ha["pixdim"][1:4] - hb["pixdim"][1:4]).max())
    checks(
        "voxel sizes (pixdim[1:4])",
        voxel_sizes <= 1e-5 * float(np.abs(ha["pixdim"][1:4]).max()),
        metric="max |diff|",
        value=f"{voxel_sizes:.1e}",
        threshold="1e-5 relative",
    )
    if a.shape != b.shape:
        return
    differing_voxels = int(np.count_nonzero(a != b))
    if a.dtype.kind in "iu":
        d = int(np.abs(a.astype(np.int64) - b.astype(np.int64)).max()) if a.size else 0
        checks(
            "values (integer output)",
            d <= INT_ATOL,
            metric="max |diff|",
            value=d,
            threshold=INT_ATOL,
            detail=f"{differing_voxels} of {a.size} values differ "
            f"({differing_voxels / max(a.size, 1):.2e})",
        )
    else:
        scale = float(np.abs(a).max()) or 1.0
        rel = float(np.abs(a.astype(np.float64) - b).max()) / scale
        checks(
            "values",
            rel <= RTOL,
            metric="max |diff| / max |AFNI|",
            value=f"{rel:.1e}",
            threshold=f"{RTOL:g}",
            detail=f"{differing_voxels} of {a.size} values differ; at most {_ulps(a, b)} ulp",
        )
    checks.add(
        Check(
            "bit-identical",
            True,
            metric="differing values",
            value=differing_voxels,
            threshold="reported",
            detail="identical" if differing_voxels == 0 else f"{differing_voxels} values differ",
        )
    )


#: The method options, as 3dTshift matches them (4 characters; wsinc 7, any case).
_METHOD_NAMES = {
    "-Fou": "Fourier",
    "-fou": "Fourier",
    "-lin": "linear",
    "-Lin": "linear",
    "-cub": "cubic",
    "-Cub": "cubic",
    "-qui": "quintic",
    "-Qui": "quintic",
    "-hep": "heptic",
    "-Hep": "heptic",
}


def _method_of(args: list[str]) -> str:
    """The interpolation method a command line selects (the last method option wins)."""
    method = "Fourier"
    for a in args:
        if a[:4] in _METHOD_NAMES:
            method = _METHOD_NAMES[a[:4]]
        elif a[:7].lower() in ("-wsinc5", "-wsinc9"):
            method = a[1:7].lower()
    return method


def _produced(code: int, target: Path, occupied: bool) -> bool:
    """Whether a run wrote its output (NIfTI, or AFNI's own format for a bare prefix)."""
    if code != 0:
        return False
    if occupied:  # the output existed before: written only if it was replaced
        return target.read_bytes() != b"taken"
    return target.exists() or Path(f"{target}+orig.HEAD").exists()


# ------------------------------------------------------------------------------------------------
# The Python API

#: The checks that compare the Python API with the command line.
PY_FILE = "Python API: the command line's output"
PY_MEMORY = "Python API, image in memory: the same values"


def _python_method(arg: str) -> str | None:
    """``lx.afni.tshift``'s method for a method option (as 3dTshift matches them), or ``None``."""
    if arg[:4] in _METHOD_NAMES:
        return _METHOD_NAMES[arg[:4]].lower()
    if arg[:7].lower() in ("-wsinc5", "-wsinc9"):
        return arg[1:7].lower()
    return None


def _python_call(s: Scenario, args: list[str]) -> dict | None:
    """``lx.afni.tshift``'s keyword arguments for a command line, or ``None`` when the Python
    API has no equivalent: the header's slice timing (no ``-tpattern``), AFNI 1D files and
    strings other than fMRIPrep's, ``-TR`` in milliseconds, options it does not have."""
    kw: dict = {}
    if s.sidecar:  # fMRIPrep's call: the BIDS SliceTiming, as _fmriprep_args writes it
        meta = json.loads(td.get(s.sidecar).read_text(encoding="utf-8"))
        times = [float(t) for t in meta["SliceTiming"]]
        if str(meta.get("SliceEncodingDirection", "")).endswith("-"):
            times = times[::-1]
        kw["slice_times"] = times
    i = 0
    while i < len(args):
        a = args[i]
        value = args[i + 1] if i + 1 < len(args) else ""
        step = 2
        if a == "-tpattern":
            if not value.startswith("@"):
                kw["slice_times"] = value
            elif not s.sidecar:
                return None
        elif a == "-TR":
            m = re.fullmatch(r"([0-9.]+)s?", value)
            if m is None:
                return None
            kw["tr"] = float(m.group(1))
        elif a in ("-tzero", "-slice", "-ignore"):
            kw[a[1:]] = float(value) if a == "-tzero" else int(value)
        else:
            step = 1
            if a in ("-rlt", "-rlt+"):
                kw["restore"] = "none" if a == "-rlt" else "intercept"
            elif a == "-no_detrend":
                kw["detrend"] = False
            elif (method := _python_method(a)) is not None:
                kw["method"] = method
            elif not a.startswith("-verb"):
                return None
        i += step
    return kw if "slice_times" in kw else None


def _differing(a: np.ndarray, b: np.ndarray) -> int:
    """How many values differ (all of them if the types or shapes do)."""
    if a.dtype != b.dtype or a.shape != b.shape:
        return max(a.size, b.size)
    return int(np.count_nonzero(a != b))


def _check_python(
    checks: CheckList, s: Scenario, args: list[str], image: Path, cli_out: Path, implementation: str
) -> None:
    """``lx.afni.tshift(..., implementation=...)`` gives the command line's output, from the
    file and from memory."""
    import larmorx as lx

    kw = _python_call(s, args)
    if kw is None:
        return
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        from_file = lx.afni.tshift(str(image), implementation=implementation, **kw)
    expected = lx.load(cli_out)
    header = lx.io.read_header(cli_out)
    differing = _differing(from_file.data, expected.data)
    same_header = (
        from_file.header.replace(vox_offset=header.vox_offset).to_bytes() == header.to_bytes()
    )
    ran = from_file.implementation.kind if from_file.implementation else "unknown"
    checks(
        PY_FILE,
        differing == 0 and same_header and ran == implementation,
        metric="differing values",
        value=differing,
        threshold=0,
        detail=f"ran the {ran}; header {'identical' if same_header else 'differs'}",
    )
    # In memory: only data 3dTshift reads as they are (uint8, int16, float32, unscaled).
    loaded = lx.load(image)
    unscaled = lx.io.read_header(image).slope_inter in (None, (1.0, 0.0))
    if loaded.data.dtype in (np.uint8, np.int16, np.float32) and unscaled:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            in_memory = lx.afni.tshift(loaded, implementation=implementation, **kw)
        d = _differing(in_memory.data, from_file.data)
        checks(PY_MEMORY, d == 0, metric="differing values", value=d, threshold=0)


#: The implementations a run can compare with AFNI (docs/licensing.md): the clean-room
#: original (larmorx-afni, in-process) or the GPL replica (the larmorx-gpl program, which
#: larmorx runs as a separate process).
IMPLEMENTATIONS = ("original", "replica")


def run_case(case: Case, checks: CheckList, implementation: str = "original") -> None:
    s: Scenario = case.payload
    if afni_binary() is None:
        raise Outcome("skipped", "the AFNI 3dTshift oracle was not found (LARMORX_AFNI_BIN)")
    if implementation == "replica" and replica_binary() is None:
        raise Outcome(
            "skipped", "the larmorx-gpl binary was not found (LARMORX_GPL_BIN, crates-gpl build)"
        )
    ours = functools.partial(_run_larmorx, implementation=implementation)
    with (
        _replica_program(implementation),
        tempfile.TemporaryDirectory(prefix="lx-tshift-") as tmp_name,
    ):
        tmp = Path(tmp_name)
        for name, text in s.files:
            (tmp / name).write_text(text, encoding="utf-8")
        occupied = any(name == "out.nii" for name, _ in s.files)
        image = _image_path(s.image)
        args = _args(s, tmp)
        outputs, logs, codes, produced = {}, {}, {}, {}
        for tool, runner in [("afni", _run_afni), ("larmorx", ours)]:
            target = tmp / ("out.nii" if occupied else f"{tool}{s.suffix}")
            codes[tool], logs[tool] = runner([*args, "-prefix", str(target), str(image)])
            outputs[tool] = target
            produced[tool] = _produced(codes[tool], target, occupied)
        if s.expect == "divergence":
            ok = produced["afni"] and not produced["larmorx"]
            checks(
                "AFNI succeeds and larmorx refuses",
                ok,
                detail=f"AFNI exit {codes['afni']}; larmorx: {_last_error(logs['larmorx'], tmp)}",
            )
            if ok:
                raise Outcome(EXPECTED, s.reason)
            return
        if not produced["afni"] and not produced["larmorx"]:
            raise Outcome(
                BOTH_ERROR,
                f"AFNI: {_last_error(logs['afni'], tmp)}; larmorx: {_last_error(logs['larmorx'], tmp)}",
            )
        checks(
            "both succeed",
            produced["afni"] and produced["larmorx"],
            detail=f"AFNI exit {codes['afni']}: {_last_error(logs['afni'], tmp)}; "
            f"larmorx exit {codes['larmorx']}: {_last_error(logs['larmorx'], tmp)}",
        )
        if s.expect == "error":
            checks("both reject the arguments", False, detail="expected an error from both")
        if produced["afni"] and produced["larmorx"]:
            copy = "just a copy of input" in logs["larmorx"]
            checks.add(
                Check(
                    "method",
                    True,
                    metric="interpolation",
                    value="copy" if copy else _method_of(args),
                    threshold="reported",
                )
            )
            _compare(checks, outputs["afni"], outputs["larmorx"])
            if s.expect == "pass" and not copy:
                _check_python(checks, s, args, image, outputs["larmorx"], implementation)


def _differing_fraction(r: CaseResult) -> float:
    for c in r.checks:
        if c.name in ("values", "values (integer output)"):
            match = re.match(r"(\d+) of (\d+) values differ", c.detail)
            if match:
                return int(match.group(1)) / max(int(match.group(2)), 1)
    return 0.0


def _highlights(results: list[CaseResult], implementation: str = "original") -> list[str]:
    compared = [r for r in results if r.status == PASS]

    def identical(r: CaseResult) -> bool:
        return any(c.name == "bit-identical" and c.value == 0 for c in r.checks)

    def method(r: CaseResult) -> str:
        return next((str(c.value) for c in r.checks if c.name == "method"), "")

    def worst(rs: list[CaseResult], name: str) -> float | None:
        values = [float(c.value) for r in rs for c in r.checks if c.name == name]
        return max(values) if values else None

    fourier = [r for r in compared if method(r) == "Fourier"]
    other = [r for r in compared if method(r) != "Fourier"]
    lines = [
        f"**Bit-identical: {sum(map(identical, compared))} of {len(compared)} passing cases** "
        "produce exactly the bytes of AFNI's output data.",
    ]
    if implementation == "replica":
        lines += [
            f"- Fourier: {sum(map(identical, fourier))} of {len(fourier)} bit-identical.",
            f"- Lagrange and weighted-sinc methods and copies: {sum(map(identical, other))} of "
            f"{len(other)} bit-identical.",
        ]
        rest = [r.case for r in compared if not identical(r)]
        if rest:
            lines.append(
                "- Not bit-identical: "
                + ", ".join(f"`{c}`" for c in rest)
                + ". See `docs/findings/afni-tshift.md`."
            )
        lines.append(
            "- The clean-room original (`larmorx afni 3dTshift`, Apache-2.0) has its own record: "
            "[afni-tshift.md](afni-tshift.md)."
        )
    else:
        lines += [
            f"- Lagrange and weighted-sinc methods and copies: {sum(map(identical, other))} of "
            f"{len(other)} bit-identical.",
            f"- Fourier: {sum(map(identical, fourier))} of {len(fourier)} bit-identical. AFNI "
            "computes the FFT in float32 with its own kernels; larmorx computes it in double "
            "precision with its own FFT, so float32 outputs differ in the last bits and integer "
            "outputs occasionally round the other way (by 1).",
        ]
        rest = [r.case for r in other if not identical(r)]
        if rest:
            lines.append(
                "- Not bit-identical outside Fourier: "
                + ", ".join(f"`{c}`" for c in rest)
                + ". Known causes, in the last float32 bit only: `-no_detrend` (its rounding is "
                "not reproduced yet); quintic and heptic weights for some fractions (unresolved); "
                "weighted sinc, where AFNI calls glibc's float `sinf`/`cosf`, which are not "
                "correctly rounded, and larmorx uses correctly rounded functions "
                "(`docs/findings/platform-math.md`). See `docs/api/afni-tshift.md`."
            )
        lines.append(
            "- The bit-exact replica (`larmorx-gpl afni 3dTshift`, GPL-3.0-or-later) has its own "
            "record: [afni-tshift-replica.md](afni-tshift-replica.md)."
        )

    def python(name: str) -> tuple[int, int]:
        rs = [c for r in compared for c in r.checks if c.name == name]
        return sum(c.passed for c in rs), len(rs)

    (file_ok, file_n), (memory_ok, memory_n) = python(PY_FILE), python(PY_MEMORY)
    lines.append(
        f'- Python API (`lx.afni.tshift(..., implementation="{implementation}")`): run on the '
        f"{file_n} passing cases it can express; {file_ok} give the command line's output bit "
        f"for bit (data and header). From an image in memory: {memory_ok} of {memory_n} give "
        "the same values."
    )
    rows = []
    for category in sorted({r.category for r in compared}):
        rs = [r for r in compared if r.category == category]
        f, i = worst(rs, "values"), worst(rs, "values (integer output)")
        rows.append(
            (
                category,
                len(rs),
                sum(map(identical, rs)),
                "–" if f is None else f"{f:.1e}",
                "–" if i is None else f"{i:g}",
                f"{max(map(_differing_fraction, rs)):.1e}",
            )
        )
    lines += [
        "",
        table(
            (
                "Category",
                "Compared",
                "Bit-identical",
                "Worst float diff (× max AFNI)",
                "Worst integer diff",
                "Most values differing (fraction)",
            ),
            rows,
        ),
    ]
    return lines


def suite(implementation: str = "original") -> Suite:
    """The 3dTshift suite for the clean-room original (``larmorx afni 3dTshift``, in-process)
    or the GPL replica (``larmorx-gpl afni 3dTshift``, which larmorx runs as a separate
    process); both through larmorx's command line and its Python API."""
    if implementation not in IMPLEMENTATIONS:
        raise ValueError(f"implementation must be one of {IMPLEMENTATIONS}, not {implementation!r}")
    replica = implementation == "replica"
    tool = (
        f"`larmorx-gpl afni 3dTshift` ({replica_version()}; crate larmorx-gpl-afni, the "
        "GPL-3.0-or-later replica translated from AFNI 25.2.09's source), run by larmorx as a "
        "separate process: `larmorx afni 3dTshift` with `LARMORX_IMPLEMENTATION=replica` and "
        '`lx.afni.tshift(..., implementation="replica")`'
        if replica
        else "`larmorx afni 3dTshift` / `lx.afni.tshift` (crate larmorx-afni, clean-room from "
        '`specs/3dTshift.md`), with `implementation="original"`'
    )
    who = "larmorx-gpl" if replica else "larmorx"
    return Suite(
        name="afni-tshift-replica" if replica else "afni-tshift",
        title="3dTshift (replica)" if replica else "3dTshift",
        tool=tool,
        reference=f"3dTshift from AFNI ({afni_version()}), the binary built by `scripts/build_afni_oracle.sh`",
        thresholds=[
            ("Exit status", "both succeed, or both reject the arguments"),
            (
                "Output data type, shape, scaling (`scl_slope`), TR, `toffset`, units, slice fields, xform codes",
                "identical",
            ),
            ("Geometry: sform and qform (read by nibabel)", f"max |diff| ≤ {AFFINE_ATOL:g} mm"),
            (
                "float32 outputs",
                f"max |diff| ≤ {RTOL:g} × max |AFNI output| (specs/3dTshift.md §9); bit-identity reported",
            ),
            ("Integer outputs", f"max |diff| ≤ {INT_ATOL}, with the fraction of differing values"),
        ],
        cases=cases,
        run_case=functools.partial(run_case, implementation=implementation),
        packages=("nibabel",),
        highlights=functools.partial(_highlights, implementation=implementation),
        notes=(
            "Both programs read the same files with the same arguments; outputs are read with "
            "nibabel and compared as stored (before `scl_slope`). Real runs pass the BIDS "
            "`SliceTiming` as fMRIPrep does: a tab-separated `-tpattern` file of `str(float)` "
            "values (reversed when `SliceEncodingDirection` ends in `-`) and "
            "`-tzero round(min + 0.5 * (max - min), 3)`. Header-timing inputs are generated from "
            "the synthetic float32 file by setting `slice_code`, `slice_start`, `slice_end`, "
            f"`slice_duration`, `dim_info` and the time unit. {who} uses all logical CPUs "
            "(`OMP_NUM_THREADS` unset) on the command line and 1 thread through the Python API "
            "(its default); its results do not depend on the thread count. The Python API runs "
            "every passing case it can express (a `-tpattern` name or fMRIPrep's slice timing, "
            "options in seconds; not the header's timing, AFNI 1D files or copies of the input) "
            "and must give the command line's output: the same values and the same header bytes "
            "(`vox_offset` aside); for unscaled `uint8`, `int16` and `float32` inputs it also runs "
            "on the image loaded in memory."
        ),
    )
