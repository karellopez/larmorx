# SPDX-License-Identifier: Apache-2.0
"""Parity of tedana's ``t2smap`` clean-room original (crate larmorx-tedana, Apache-2.0, written
from ``specs/t2smap.md``) with tedana 26.0.3's program and with the replica's default.

Written by the clean-room implementer: it reads no tedana source and no replica code, and runs the
replica (``larmorx-gpl tedana t2smap``, from ``LARMORX_GPL_BIN`` or the ``PATH``) only as a black
box. Its own name and module, beside the replica's suite ``tedana-t2smap-replica`` (never read).
Before the original is wired into ``larmorx tedana t2smap`` it runs through its own binding,
``larmorx._core.tedana_t2smap_cleanroom``, in this process.

Every case runs each program in a folder of its own with ``--out-dir out`` and the spec's
reference setup (``blackbox.environment()``: ``COLUMNS=80``, ``TQDM_DISABLE=1``, one OpenBLAS
thread, numpy's AVX-512 kernels off, §17.1):

(A) tedana (through the oracle-output cache) against the original with
    ``LARMORX_UPSTREAM_DEFECTS=reproduce``: the exit status, standard output, the messages (every
    log line and error; Python's warnings by category and message, their location lines being
    outside the contract, §12.3), the files written, and the values by the spec's ``check.py``
    (§17.3's thresholds), or file by file (``outputs.py``) where ``check.py``'s model cannot read
    the inputs (pairs, ANALYZE, MGH, a 4D mask);
(B) the replica's default against the original's default: the same, and the defects' notices
    the same (NP-03 and GLIBC-07 aside: the original's maths notices follow §15.2's one-pass
    rule).

The cases (§17.2): the spec probe's 54 (``validation/investigations/t2smap-spec/blackbox.py``)
and the clean-room part's 82 added ones (``validation/investigations/t2smap-original/
compare.py``: the readers, every defect's edges in both modes, the curve fit's paths, the
warnings), the original's default at 1, 4 and 8 threads (every output identical), and
fMRIPrep's multi-echo calls (ds003592 with ``loglin``, ``curvefit`` and ``--verbose``; ds000210's
two tasks; ds000258), read in place from the oracle runs' working folders. Inputs are made once
with seeded numbers into ``<workspace>/oracles/inputs/tedana-t2smap-original/suite-<version>/``.
"""

from __future__ import annotations

import contextlib
import importlib.util
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from larmorx_validation.parity.harness import (
    EXPECTED,
    SKIPPED,
    Case,
    Check,
    CheckList,
    Outcome,
    Suite,
)

SUITE = "tedana-t2smap-original"
#: The derived inputs' version (a new one when the probe's or the harness's inputs change).
INPUTS_VERSION = "v1"
#: The original's threads in its default runs; its reproduce runs use one, as tedana does.
LX_THREADS = 4

HERE = Path(__file__).resolve()
REPO = next(p for p in HERE.parents if (p / "specs" / "t2smap.md").is_file())
WORKSPACE = next(p for p in REPO.parents if (p / "oracles").is_dir())
INVESTIGATIONS = REPO / "validation" / "investigations"
LAUNCHER = WORKSPACE / "oracles/inputs/tedana-t2smap-replica/launcher-newest/t2smap"

#: Cases whose (A) comparison is expected to differ, with the reason.
EXPECTED_A = {
    "x-underflow-debug-curvefit": "SciPy's own floating-point warnings inside its trust-region "
    "and finite-difference steps (overflow in a scalar multiply and a dot product, invalid "
    "subtractions) are not reproduced: the original's solver is its own (§12.3 lists the curve "
    "fit's OptimizeWarning only); one voxel whose SciPy fit overflows converges in the "
    "original (its values pass §17.3 (b)/(c))",
}
#: Cases whose (B) comparison is expected to differ, with the reason (black-box facts).
EXPECTED_B = {
    "x-underflow-debug": "the replica does not show numpy's cast overflow again for the limited "
    "S0 map after the confounds table, where tedana's warnings registry has been reset "
    "(observed; the original shows it, as tedana)",
    "x-inf-both-paid": "the replica does not show numpy's 'invalid value encountered in "
    "reduce' of the adaptive mask's means of a series holding both infinities (tedana does: "
    "observed; the original does too)",
    "three-d": "the replica prints only the last of tedana's two chained tracebacks for a 3D "
    "echo file (the original prints both, as tedana)",
    "x-mask-grid": "the replica prints only nilearn's error where tedana chains it after its "
    "own 'Image/mask shape mismatch' (the original prints both, as tedana)",
    "x-underflow-debug-curvefit": "a chaotic regime (noise voxels with T2* ~ 0.2 ms and S0 ~ "
    "1e28 from echo times 0.05 ms apart): one voxel's fits end at different local optima; "
    "tedana's own result differs from the replica's default far more (33 % of the fit voxels "
    "by §17.3 (b)); the replica keeps SciPy's failure count (48), the original 47",
}
#: The replica's documented gap: it refuses ANALYZE and MGH images, which tedana reads.
REPLICA_FORMATS_GAP = re.compile(r"^x-(analyze|mgh)")
#: Cases outside ``check.py``'s model, judged file by file: its model reproduces tedana's
#: behaviour, which TED-01's NaN sample makes fail (since spec writer 2, 2026-11-11, the model
#: reads every image form of §4.1 and nilearn's masking path, so the readers' cases are judged by
#: §17.3's thresholds like every other).
FILE_BY_FILE = re.compile(r"^nan-sample$")
#: The relative difference allowed file by file where ``check.py``'s model cannot follow a run
#: (a defect the default corrects stops tedana, and the model with it): a few float32 units.
FILE_TOLERANCE = 1e-5

SMOKE = (
    "loglin",
    "curvefit",
    "exclude-step",
    "x-analyze-spm",
    "x-mgh-oblique-i16",
    "x-inf-sample-paid",
)
FMRIPREP = (
    ("ds003592-loglin", "ds003592", ["--fittype", "loglin"]),
    ("ds003592-curvefit", "ds003592", ["--fittype", "curvefit"]),
    ("ds003592-curvefit-verbose", "ds003592", ["--fittype", "curvefit", "--verbose"]),
    ("ds000210-cuedSGT-curvefit", "ds000210-cuedSGT", ["--fittype", "curvefit"]),
    ("ds000210-rest-curvefit", "ds000210-rest", ["--fittype", "curvefit"]),
    ("ds000258-curvefit", "ds000258", ["--fittype", "curvefit"]),
)
THREADS = (
    ("threads-loglin", []),
    ("threads-curvefit-ts", ["--fittype", "curvefit", "--fitmode", "ts", "--verbose"]),
)


def _module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def tools():
    """The spec's probe and the part's harness, loaded from their folders."""
    if "t2smap_original_compare" not in sys.modules:
        sys.path.insert(0, str(INVESTIGATIONS / "t2smap-spec"))
        _module("t2smap_original_compare", INVESTIGATIONS / "t2smap-original" / "compare.py")
        _module("t2smap_original_outputs", INVESTIGATIONS / "t2smap-original" / "outputs.py")
        _module(
            "t2smap_original_fmriprep", INVESTIGATIONS / "t2smap-original" / "fmriprep_calls.py"
        )
    return (
        sys.modules["blackbox"],
        sys.modules["t2smap_original_compare"],
        sys.modules["t2smap_original_outputs"],
        sys.modules["t2smap_original_fmriprep"],
    )


def replica_program() -> list[str] | None:
    found = os.environ.get("LARMORX_GPL_BIN") or shutil.which("larmorx-gpl")
    return [found, "tedana", "t2smap"] if found else None


def original_available() -> bool:
    try:
        from larmorx import _core
    except ImportError:
        return False
    return hasattr(_core, "tedana_t2smap_cleanroom")


def inputs_dir() -> Path:
    """The probe's inputs and the harness's, made once (atomically: the workers share them)."""
    root = WORKSPACE / "oracles" / "inputs" / SUITE
    d = root / f"suite-{INPUTS_VERSION}"
    if (d / "suite.done").exists():
        return d
    blackbox, compare, _, _ = tools()
    root.mkdir(parents=True, exist_ok=True)
    tmp = Path(tempfile.mkdtemp(dir=root, prefix="making-"))
    work = tmp / "inputs"
    blackbox.make_inputs(work)
    compare.extra_inputs(work)
    compare.format_inputs(work)
    compare.more_inputs(work)
    (work / "suite.done").write_text(f"{INPUTS_VERSION}\n", encoding="utf-8")
    with contextlib.suppress(OSError):  # another worker made them first
        work.rename(d)
    shutil.rmtree(tmp, ignore_errors=True)
    return d


# --------------------------------------------------------------------------------------------
# running the programs


def run_tedana(args: list[str], case: Path, env: dict, never: str = "") -> tuple[int, str, str]:
    """tedana through the oracle-output cache. A replayed run gets back the files it wrote, not
    its empty directories: those the spec says it makes are made again (§3: the output
    directory once step 1 has passed; ``figures/`` where its log line says it was made)."""
    from larmorx_validation import oracle_cache

    case.mkdir(parents=True, exist_ok=True)
    res = oracle_cache.run(
        [str(LAUNCHER), *args],
        env=env,
        cwd=case,
        outputs=[case / "out"],
        policy=oracle_cache.Policy(threads=1, family="python", never=never),
        timeout=3600,
        runner=SUITE,
    )
    code = res.returncode
    out = res.stdout_bytes.decode("utf-8", "replace")
    err = res.stderr_bytes.decode("utf-8", "replace")
    if res.cached and code != 2 and "--out-dir" in args:
        target = case / args[args.index("--out-dir") + 1]
        last = err.rstrip().splitlines()[-1:] or [""]
        if not last[0].startswith(
            f"FileNotFoundError: [Errno 2] No such file or directory: '{target}"
        ):
            target.mkdir(exist_ok=True)
            # step 9 made figures/: its log line says so, or (with --quiet) a file written after it
            if "Generating figures directory" in err or any(target.glob("*.nii*")):
                (target / "figures").mkdir(exist_ok=True)
    return code, out, err


def run_original(args: list[str], case: Path, env: dict) -> tuple[int, str, str]:
    from larmorx import _core

    case.mkdir(parents=True, exist_ok=True)
    code, out, err = _core.tedana_t2smap_cleanroom(args, list(env.items()), str(case))
    return int(code), out, err


def run_replica(args: list[str], case: Path, env: dict) -> tuple[int, str, str]:
    case.mkdir(parents=True, exist_ok=True)
    proc = subprocess.run(
        [*replica_program(), *args],
        cwd=case,
        env=env,
        capture_output=True,
        text=True,
        timeout=3600,
        check=False,
    )
    return proc.returncode, proc.stdout, proc.stderr


# --------------------------------------------------------------------------------------------
# comparing

WARNING = re.compile(r"^\S+:\d*\S*: (\w+): (.*)$")
NOTICE = re.compile(r"^larmorx(-gpl)?: upstream defect ")
CAP = re.compile(r"^(DEBUG +decay:modify_t2s_s0_maps:\d+ Setting cap on T2\* map at ).*$")


def messages(err: str, work: Path, inputs: Path) -> list[str]:
    """The messages as the contract has them (§12): Python's warnings by category and message
    (their location and source lines left out), the notices left out (compared on their own),
    the cap's digits left out (judged by the cap rule)."""
    blackbox, _, _, _ = tools()
    lines = blackbox.normalise(err.replace(str(inputs), "<I>"), work)
    out: list[str] = []
    skip = False
    for line in lines:
        if skip and line.startswith("  ") and not line.startswith("  <stack>"):
            skip = False
            continue
        skip = False
        if NOTICE.match(line):
            continue
        m = WARNING.match(line)
        if m and m.group(1).endswith("Warning") and not line.startswith(("INFO", "WARNING")):
            out.append(f"{m.group(1)}: {m.group(2)}")
            skip = True
            continue
        out.append(CAP.sub(r"\1<cap>", line))
    return out


def notices(err: str) -> list[str]:
    """The defects named (sorted; NP-03 and GLIBC-07 aside), in the original's words."""
    found = []
    for line in err.splitlines():
        if NOTICE.match(line) and not re.search(r"defect (NP-03|GLIBC-07) ", line):
            line = re.sub(r"^larmorx-gpl:", "larmorx:", line)
            # the decimals (TED-05's cap and ten caps, TED-18's) are each run's own order
            # statistic, judged by §17.3's cap row with the maps; the counts are compared
            found.append(re.sub(r"\d+\.\d+", "<x>", line))
    return sorted(found)


def files(out: Path) -> list[str]:
    if not out.exists():
        return []
    return sorted(str(p.relative_to(out)) for p in out.rglob("*") if p.is_file())


def _within(diffs: list[str], tolerance: float) -> list[str]:
    """``outputs.compare``'s differences beyond ``tolerance`` (relative), headers and text kept."""
    out = []
    for d in diffs:
        m = re.search(r"values differ, largest relative ([0-9.e+-]+|nan|inf)$", d)
        if m and m.group(1) not in ("nan", "inf") and float(m.group(1)) <= tolerance:
            continue
        out.append(d)
    return out


def judge_values(ref: Path, other: Path, file_by_file: bool) -> tuple[bool, str]:
    """``check.py`` (§17.3) on two case folders' outputs, or file by file; where ``check.py``'s
    model cannot follow the run (it stops where tedana stops, and a defect the default corrects
    let the run go on), file by file within ``FILE_TOLERANCE``."""
    _, _, outputs, _ = tools()
    combined = any((ref / "out").glob("*optcom_bold.nii.gz")) or any(
        (ref / "out").glob("*ts_OC.nii.gz")
    )
    if file_by_file or not combined:
        diffs = outputs.compare(ref / "out", other / "out") if (ref / "out").exists() else []
        return not diffs, "; ".join(diffs)[:600] or "identical"
    proc = subprocess.run(
        [
            sys.executable,
            "-I",
            str(INVESTIGATIONS / "t2smap-spec" / "check.py"),
            str(ref / "out"),
            str(other / "out"),
            "--cwd",
            str(ref),
            "--details",
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    lines = proc.stdout.splitlines()
    failed = [x for x in lines if x.startswith("FAIL") or x.startswith("    [")]
    if failed:
        return False, "; ".join(failed)[:600]
    if proc.returncode != 0:
        if "model.py" in proc.stderr:
            diffs = _within(outputs.compare(ref / "out", other / "out"), FILE_TOLERANCE)
            last = proc.stderr.strip().splitlines()[-1][:120]
            note = f"outside check.py's model ({last}): file by file within {FILE_TOLERANCE:g}"
            return not diffs, note + ("; " + "; ".join(diffs)[:500] if diffs else "")
        return False, proc.stderr.strip()[-300:]
    share = [x for x in lines if "(b)/(c) share" in x]
    identical = sum(1 for x in lines if x.startswith("PASS") and " failed" in x)
    m = re.search(r"share ([0-9.]+%) of the fit voxels", share[0]) if share else None
    return True, f"{identical} rows pass" + (f"; (b)/(c) share {m.group(1)}" if m else "")


def compare_runs(
    checks: CheckList,
    label: str,
    ref: tuple[int, str, str],
    other: tuple[int, str, str],
    ref_case: Path,
    other_case: Path,
    inputs: Path,
    *,
    file_by_file: bool,
    with_notices: bool,
) -> None:
    checks(f"{label} exit status", ref[0] == other[0], value=other[0], threshold=ref[0])
    checks(
        f"{label} standard output",
        ref[1].replace(str(inputs), "<I>") == other[1].replace(str(inputs), "<I>"),
    )
    m_ref = messages(ref[2], ref_case.parent, inputs)
    m_other = messages(other[2], other_case.parent, inputs)
    m_ref = [x.replace(f"<W>/{ref_case.name}/", "<W>/<case>/") for x in m_ref]
    m_other = [x.replace(f"<W>/{other_case.name}/", "<W>/<case>/") for x in m_other]
    first = next((i for i, (a, b) in enumerate(zip(m_ref, m_other, strict=False)) if a != b), None)
    if first is None and len(m_ref) != len(m_other):
        first = min(len(m_ref), len(m_other))
    checks(
        f"{label} messages",
        first is None,
        detail=""
        if first is None
        else f"line {first}: {m_ref[first : first + 1]} / {m_other[first : first + 1]}",
    )
    if with_notices:
        n_ref, n_other = notices(ref[2]), notices(other[2])
        checks(
            f"{label} notices",
            n_ref == n_other,
            detail="" if n_ref == n_other else f"{n_ref[:3]} / {n_other[:3]}",
        )
    f_ref, f_other = files(ref_case / "out"), files(other_case / "out")
    checks(
        f"{label} files",
        f_ref == f_other,
        detail="" if f_ref == f_other else f"{sorted(set(f_ref) ^ set(f_other))[:6]}",
    )
    if "t2smap_call.sh" in f_ref and "t2smap_call.sh" in f_other:
        ok, detail = judge_values(ref_case, other_case, file_by_file)
        checks(f"{label} values (§17.3)", ok, detail=detail)


# --------------------------------------------------------------------------------------------
# known divergences (black-box facts and decisions, each with its reason)

#: The replica's default's known message gaps (relays 2a, 2c, 3a-ii: black-box facts, in the
#: replica-side relay's list): a chained traceback printed as its last part only, numpy's
#: RuntimeWarning lines it does not print, numpy's scientific thresholds padded with zeros.
_CHAIN = "During handling of the above exception, another exception occurred:"
_WARNING_LINE = re.compile(r"^RuntimeWarning: ")
_THRESHOLDS = re.compile(r"^(INFO +utils:make_adaptive_mask:\d+ Echo-wise intensity thresholds for adaptive mask: )\[(.*)\]$")  # fmt: skip
#: The lines TED-46's equal observations change (§7.2: tedana's slope there is rounding noise,
#: not 0): numpy's divide warning, the curve fit's failure counts and SciPy's warning.
_TED46_LINES = re.compile(
    r"^(RuntimeWarning: divide by zero encountered in divide|OptimizeWarning: |DEBUG +decay:"
    r"fit_monoexponential:\d+ With \d+ echoes, monoexponential fit failed)"
)
_TED46 = re.compile(r"upstream defect TED-46 ")
_TED31 = re.compile(r"upstream defect TED-31 ")
REASON_TED46_A = (
    "TED-46 (§7.2's observations without a slope): reproduce writes the exact zero slope where "
    "tedana's batched least squares leaves rounding noise (1 ms or 1e16 ms and more), until the "
    "exact port of numpy's lstsq serves reproduce"
)
REASON_TED46_B = (
    "TED-46: the replica's default does not take the slope 0 at observations without a slope "
    "yet (its record's list for the replica-side relay)"
)
REASON_TED31_B = (
    "TED-31: the replica's default does not refuse a run with no voxel left for the fit yet (it "
    "completes the log-linear fit, and stops with nilearn's empty-mask text in the curve fit)"
)
REASON_REPLICA_GAPS = (
    "the replica's known message gaps (one of two chained tracebacks, numpy's RuntimeWarning "
    "lines, the thresholds' zero-padded digits)"
)


def join_wrapped(lines: list[str]) -> list[str]:
    """numpy's printing of a long thresholds array goes on over several lines: one line again."""
    out: list[str] = []
    for line in lines:
        prev = out[-1] if out else ""
        if prev.startswith("INFO") and "intensity thresholds" in prev and "]" not in prev:
            out[-1] = f"{prev} {line.strip()}"
            continue
        out.append(line)
    return out


def by_value(line: str) -> str:
    """The dropout thresholds' line with each number as the float32 it names (larmorx-gpl
    0.1.0rc1 pads numpy's scientific form with zeros where numpy prints the element's own further
    digits: the same values)."""
    import numpy as np

    m = _THRESHOLDS.match(line)
    if not m:
        return line
    values = [repr(float(np.float32(w))) for w in m.group(2).split()]
    return f"{m.group(1)}{values}"


def without_replica_gaps(lines: list[str]) -> list[str]:
    """The messages with what the replica's known gaps change taken out."""
    lines = join_wrapped(lines)
    if _CHAIN in lines:
        first = lines.index("Traceback (most recent call last):")
        last = len(lines) - 1 - lines[::-1].index("Traceback (most recent call last):")
        lines = lines[:first] + lines[last:]
    return [by_value(x) for x in lines if not _WARNING_LINE.match(x)]


def _failed(part: CheckList) -> list[str]:
    """The failed checks' names without their label (``(A)``, ``(B) run 2``, ...)."""
    return [re.sub(r"^\([AB]\)( run \d+)? ", "", c.name) for c in part.checks if not c.passed]


def explain(
    label: str,
    part: CheckList,
    ref: tuple[int, str, str],
    other: tuple[int, str, str],
    ref_case: Path,
    other_case: Path,
    inputs: Path,
) -> str | None:
    """The reason of ``part``'s failures when every one is a known divergence, else ``None``:
    (A) tedana against the original's ``reproduce``, (B) the replica's default against the
    original's default (``other`` is always the original's run)."""
    failed = _failed(part)
    if not failed:
        return None
    m_ref = messages(ref[2], ref_case.parent, inputs)
    m_other = messages(other[2], other_case.parent, inputs)
    m_ref = [x.replace(f"<W>/{ref_case.name}/", "<W>/<case>/") for x in m_ref]
    m_other = [x.replace(f"<W>/{other_case.name}/", "<W>/<case>/") for x in m_other]
    if label.startswith("(B)") and _TED31.search(other[2]):
        return REASON_TED31_B
    if _TED46.search(other[2]):

        def strip46(lines: list[str]) -> list[str]:
            return [x for x in lines if not _TED46_LINES.match(x)]

        n_ref = [x for x in notices(ref[2]) if "TED-46" not in x]
        n_other = [x for x in notices(other[2]) if "TED-46" not in x]
        ok = {"values (§17.3)"}
        if strip46(m_ref) == strip46(m_other) or (
            label.startswith("(B)")
            and without_replica_gaps(strip46(m_ref)) == without_replica_gaps(strip46(m_other))
        ):
            ok.add("messages")
        if n_ref == n_other:
            ok.add("notices")
        if all(f in ok for f in failed):
            return REASON_TED46_A if label.startswith("(A)") else REASON_TED46_B
    if (
        label.startswith("(B)")
        and all(f == "messages" for f in failed)
        and without_replica_gaps(m_ref) == without_replica_gaps(m_other)
    ):
        return REASON_REPLICA_GAPS
    return None


# --------------------------------------------------------------------------------------------
# the cases


def cases(tier: str) -> list[Case]:
    from larmorx_validation.parity.harness import in_tier

    blackbox, compare, _, _ = tools()
    out: list[Case] = []
    for name, args, runs in [*blackbox.CASES, *compare.EXTRA]:
        case_tier = "smoke" if name in SMOKE else "standard"
        if in_tier(case_tier, tier):
            out.append(Case(name, "probe", " ".join(args) or "(no arguments)", (args, runs)))
    if in_tier("standard", tier):
        for name, extra in THREADS:
            out.append(Case(name, "threads", "the default at 1, 4 and 8 threads", extra))
    if in_tier("full", tier):
        for name, call, extra in FMRIPREP:
            out.append(
                Case(name, "fmriprep", f"fMRIPrep 25.2.6's {call} call {extra}", (call, extra))
            )
    return out


def _work() -> Path:
    base = Path(os.environ.get("TMPDIR", tempfile.gettempdir()))
    return Path(tempfile.mkdtemp(prefix="lx-t2smap-original-", dir=base))


def _settle(checks: CheckList, part: CheckList, reason: str | None) -> str | None:
    """``part``'s checks added to ``checks``; with a ``reason`` for an expected difference, its
    failures are added as passed, marked expected, and the reason returned when any failed."""
    failed = any(not c.passed for c in part.checks)
    for c in part.checks:
        if reason is not None and not c.passed:
            c = Check(c.name, True, c.metric, c.value, c.threshold, "expected: " + c.detail)
        checks.add(c)
    return reason if (reason is not None and failed) else None


def run_probe(case: Case, checks: CheckList) -> None:
    blackbox, compare, _, _ = tools()
    args, runs = case.payload
    inputs = inputs_dir()
    work = _work()
    try:
        full = [w.replace("<I>", str(inputs)) for w in args]
        if case.id not in compare.NO_OUT_DIR:
            full = [*full, "--out-dir", "out"]
        env = {**blackbox.environment(), **compare.CASE_ENV.get(case.id, {})}
        renv = {**env, "LARMORX_UPSTREAM_DEFECTS": "reproduce"}
        dirs = {k: work / k / case.id for k in ("tedana", "reproduce", "replica", "default")}
        res_t, res_r = [], []
        for run in range(runs):
            never = "" if run == 0 else "a second run depends on the first run's outputs"
            res_t.append(run_tedana(full, dirs["tedana"], env, never))
            res_r.append(run_original(full, dirs["reproduce"], renv))
        fbf = bool(FILE_BY_FILE.match(case.id))
        a = CheckList()
        for i in range(runs):
            compare_runs(
                a,
                f"(A) run {i + 1}",
                res_t[i],
                res_r[i],
                dirs["tedana"],
                dirs["reproduce"],
                inputs,
                file_by_file=fbf,
                with_notices=False,
            )
        reason_a = EXPECTED_A.get(case.id)
        for i in range(runs):
            reason_a = reason_a or explain(
                "(A)", a, res_t[i], res_r[i], dirs["tedana"], dirs["reproduce"], inputs
            )
        expected = [_settle(checks, a, reason_a)]
        replica = replica_program()
        if replica is None:
            checks("(B) the replica", True, detail="larmorx-gpl not found: (B) not run")
            _expected(checks, expected)
            return
        if REPLICA_FORMATS_GAP.match(case.id):
            code, _, err = run_replica(full, dirs["replica"], env)
            if "UnsupportedFormat" in err:
                checks("(B) the replica refuses ANALYZE and MGH (its documented gap)", code == 1)
                res_d = run_original(
                    full, dirs["default"], {**env, "LARMORX_UPSTREAM_DEFECTS": "fix"}
                )
                default_reads(checks, res_d, res_r[0])
                _expected(checks, expected)
                return
        res_p, res_d = [], []
        denv = {**env, "LARMORX_UPSTREAM_DEFECTS": "fix"}
        for _run in range(runs):
            res_p.append(run_replica(full, dirs["replica"], env))
            res_d.append(run_original(full, dirs["default"], denv))
        b = CheckList()
        for i in range(runs):
            compare_runs(
                b,
                f"(B) run {i + 1}",
                res_p[i],
                res_d[i],
                dirs["replica"],
                dirs["default"],
                inputs,
                file_by_file=fbf,
                with_notices=True,
            )
        reason_b = EXPECTED_B.get(case.id)
        for i in range(runs):
            reason_b = reason_b or explain(
                "(B)", b, res_p[i], res_d[i], dirs["replica"], dirs["default"], inputs
            )
        expected.append(_settle(checks, b, reason_b))
        _expected(checks, expected)
    finally:
        shutil.rmtree(work, ignore_errors=True)


def default_reads(
    checks: CheckList, default: tuple[int, str, str], reproduce: tuple[int, str, str]
):
    """Where the replica refuses ANALYZE and MGH: the original's default reads them, and where its
    exit status differs from ``reproduce``'s a corrected defect's notice says why."""
    explained = default[0] == reproduce[0] or bool(
        re.search(r"^larmorx: upstream defect \S+ \(\w+\) corrected: ", default[2], re.M)
    )
    checks(
        "(B) the original's default reads them",
        "UnsupportedFormat" not in default[2] and explained,
        detail=f"exit {default[0]} (reproduce {reproduce[0]})",
    )


def _expected(checks: CheckList, reasons: list[str | None]) -> None:
    """Ends the case as an expected difference when one was met and nothing else failed."""
    met = [r for r in reasons if r]
    if met and all(c.passed for c in checks.checks):
        raise Outcome(EXPECTED, "; ".join(met))


def run_threads(case: Case, checks: CheckList) -> None:
    blackbox, _, outputs, _ = tools()
    inputs = inputs_dir()
    work = _work()
    try:
        base = [*blackbox.E3, *blackbox.TES, *blackbox.MASK, *case.payload]
        base = [w.replace("<I>", str(inputs)) for w in base]
        env = blackbox.environment()
        dirs = []
        for n in (1, 4, 8):
            d = work / f"threads-{n}"
            code, _, err = run_original([*base, "--n-threads", str(n), "--out-dir", "out"], d, env)
            checks(f"{n} thread(s): exit 0", code == 0, detail=err[-300:] if code else "")
            dirs.append(d)
        for d, n in zip(dirs[1:], (4, 8), strict=True):
            diffs = [
                x
                for x in outputs.compare(dirs[0] / "out", d / "out")
                if not x.startswith("t2smap_call.sh")
            ]
            checks(f"1 = {n} threads: every output identical", not diffs, detail="; ".join(diffs))
    finally:
        shutil.rmtree(work, ignore_errors=True)


def run_fmriprep(case: Case, checks: CheckList) -> None:
    blackbox, _, _, fmriprep = tools()
    call, extra = case.payload
    run, task, tes = fmriprep.CALLS[call]
    if not (fmriprep.RUNS / run).is_dir():
        raise Outcome(SKIPPED, f"the oracle run {run} is not on this machine")
    echoes, mask = fmriprep.inputs(run, task)
    args = ["-d", *map(str, echoes), "-e", *tes, "--mask", str(mask), *extra, "--out-dir", "out"]
    env = blackbox.environment()
    inputs = echoes[0].parent
    work = _work()
    try:
        dirs = {k: work / k / case.id for k in ("tedana", "reproduce", "replica", "default")}
        res_t = run_tedana(args, dirs["tedana"], env)
        res_r = run_original(
            args, dirs["reproduce"], {**env, "LARMORX_UPSTREAM_DEFECTS": "reproduce"}
        )
        compare_runs(
            checks,
            "(A)",
            res_t,
            res_r,
            dirs["tedana"],
            dirs["reproduce"],
            inputs,
            file_by_file=False,
            with_notices=False,
        )
        if replica_program() is None:
            checks("(B) the replica", True, detail="larmorx-gpl not found: (B) not run")
            return
        res_p = run_replica(args, dirs["replica"], env)
        threads = ["--n-threads", str(LX_THREADS)]
        res_d = run_original(
            [*args[:-2], *threads, "--out-dir", "out"],
            dirs["default"],
            {**env, "LARMORX_UPSTREAM_DEFECTS": "fix"},
        )
        compare_runs(
            checks,
            "(B)",
            res_p,
            res_d,
            dirs["replica"],
            dirs["default"],
            inputs,
            file_by_file=False,
            with_notices=True,
        )
    finally:
        shutil.rmtree(work, ignore_errors=True)


def run_case(case: Case, checks: CheckList) -> None:
    if not original_available():
        raise Outcome(SKIPPED, "larmorx._core.tedana_t2smap_cleanroom is not built")
    if not LAUNCHER.exists():
        raise Outcome(SKIPPED, "tedana 26.0.3's launcher is not on this machine")
    {"probe": run_probe, "threads": run_threads, "fmriprep": run_fmriprep}[case.category](
        case, checks
    )


def highlights(results) -> list[str]:
    """fMRIPrep's calls: §17.3's (b)/(c) share of the curve fit in (A) and (B)."""
    out = []
    for r in results:
        if r.category != "fmriprep" or r.status not in ("pass", EXPECTED):
            continue
        parts = [
            f"{c.name.split(' ')[0]} {c.detail}"
            for c in r.checks
            if c.name.endswith("values (§17.3)")
        ]
        out.append(f"`{r.case}`: " + "; ".join(parts))
    return out


def suite() -> Suite:
    return Suite(
        name=SUITE,
        title="tedana t2smap: the clean-room original against tedana 26.0.3 and the replica",
        tool="tedana t2smap (clean-room original)",
        reference="tedana 26.0.3's t2smap (oracle cache); larmorx-gpl's replica by default",
        thresholds=[
            ("exit status, standard output, files", "identical"),
            (
                "messages",
                "identical but Python warnings' location lines (§12.3) and the cap's DEBUG digits",
            ),
            (
                "notices (B)",
                "the same defects named in the same words and counts (NP-03, GLIBC-07 aside; "
                "the cap's decimals by §17.3's cap row)",
            ),
            (
                "values",
                "specs/t2smap.md §17.3 by its check.py; file by file where its model "
                "cannot read the inputs",
            ),
            ("threads", "every output identical at 1, 4 and 8 threads"),
        ],
        cases=cases,
        run_case=run_case,
        notes="(A) is the original's reproduce against tedana, (B) its default against the "
        "replica's default (a black box). The fitted numbers of reproduce are the original's own "
        "solvers' within §17.3, not bit for bit (decision of 2026-11-11).",
        packages=("tedana", "numpy", "scipy", "nibabel", "nilearn"),
        highlights=highlights,
    )
