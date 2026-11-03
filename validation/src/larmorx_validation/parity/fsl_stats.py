# SPDX-License-Identifier: Apache-2.0
"""Parity of fslstats' clean-room original (``larmorx fsl fslstats``, crate larmorx-fsl,
Apache-2.0, written from ``specs/fslstats.md``) with FSL 6.0.7.23's ``fslstats``.

The cases are the replica suite's (``fsl_stats_replica``): fMRIPrep's four goodvoxels calls on
ds005040's chain (the ribbon a stand-in: the segmentation's grey matter), BET's ``-l 0.001 -C``
on ds005040's T1w, and its synthetic cases (the command line and its messages, every option, the
options in order, ``-t``, ``-K``, ``-json``, the data types, the defects in both modes), run here
against the original; and the original's own (``orig/``). The replica suite's ``fsltest/`` cases
(FSL's own test script, images and expected outputs, which live in FSL's source tree) are left
out: the clean-room implementer does not open FSL's source. Every FSL run goes through the
oracle-output cache with the replica suite's keys, so its runs are served.

What is compared (``specs/fslstats.md`` §10), each run against FSL's:

- the exit status (FSL's crash against the original's status 1), standard error line by line
  (the run directory replaced, the original's ``larmorx`` lines left out);
- standard output word by word: identical text, except the numbers of the options §10 allows a
  bound (``-M``, ``-S``, ``-m``, ``-s`` without a mask, ``-D``, ``-e``, ``-E``, ``-c``, ``-C``):
  relative difference at most 1e-6 (absolute below 1); with ``-json`` the document's text with
  its numbers taken out identical, and its numbers by the same rule (their text nlohmann's form
  where they are equal);
- the D13 fix pass: a defect named exactly when the default's output differs from
  ``reproduce``.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np

from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_stats_replica as rep
from larmorx_validation.parity.harness import BOTH_ERROR, SKIPPED, Case, CheckList, Outcome, Suite

SUITE = "fsl-stats"
PROGRAM = "fslstats"
#: The original's thread count in the cases.
LX_THREADS = 4
#: §10: the bound of the options whose sums' order the original may choose.
RELATIVE = 1e-6
#: The options printing a number by sums, with the words they print (a histogram: its bins).
_WORDS = {
    "-r": 2,
    "-R": 2,
    "-e": 1,
    "-E": 1,
    "-v": 2,
    "-V": 2,
    "-m": 1,
    "-M": 1,
    "-s": 1,
    "-S": 1,
    "-w": 8,
    "-x": 3,
    "-X": 3,
    "-c": 3,
    "-C": 3,
    "-p": 1,
    "-P": 1,
    "-D": 1,
}
#: The options' arguments.
_ARGS = {"-l": 1, "-u": 1, "-k": 1, "-d": 1, "-p": 1, "-P": 1, "-h": 1, "-H": 3}
#: The options a bound applies to always, and those only without a mask.
_BOUND_ALWAYS = ("-D", "-e", "-E", "-c", "-C")
_BOUND_UNMASKED = ("-m", "-M", "-s", "-S")
#: The options that make the masks (when there are none).
_MAKE_MASKS = ("-l", "-u", "-k", "-P", "-e", "-E", "-w")


# ------------------------------------------------------------------------------------------------
# The replica suite's cases, without FSL's own tests


def replica_cases() -> dict[str, tuple[str, str, rep.Run]]:
    """The replica suite's cases but its ``fsltest/`` ones (FSL's own tests, in FSL's source
    tree: listing them would read its test script)."""
    saved = rep._fsl_test_cases
    rep._fsl_test_cases = dict
    try:
        return rep._cases()
    finally:
        rep._fsl_test_cases = saved


def own_inputs() -> Path:
    """The original's own synthetic inputs, made once (fixed bytes)."""
    ws = rep.workspace()
    base = (ws / "oracles" / "inputs" if ws else Path(tempfile.gettempdir())) / SUITE
    marker = base / "v1.done"
    if marker.exists():
        return base
    base.mkdir(parents=True, exist_ok=True)
    sh = (7, 6, 5)
    x, y, z = np.meshgrid(*(np.arange(n) for n in sh), indexing="ij")
    ramp = (x + 7 * y + 42 * z).astype(np.float32)
    inf = ramp.copy()
    inf[3, 2, 1] = np.inf
    rep._save(base / "rampinf.nii.gz", inf, dtype=np.float32)
    marker.write_text("1\n", encoding="utf-8")
    return base


def _prepare(run: rep.Run, tmp: Path) -> list[str]:
    """The run's arguments in ``tmp``: ``@orig:<name>`` one of :func:`own_inputs`, the rest as
    the replica suite prepares them (never its ``%`` names, FSL's test files)."""
    out: list[str] = []
    for a in run.args:
        if a.startswith("%"):
            raise ValueError(f"{a}: FSL's own test files are not for the clean room")
        if a.startswith("@orig:"):
            src = own_inputs() / f"{a[len('@orig:') :]}.nii.gz"
            shutil.copy(src, tmp / src.name)
            out.append(src.name)
        else:
            out.extend(rep._prepare(rep._r(a), tmp))
    return out


# ------------------------------------------------------------------------------------------------
# The original


def original_command() -> list[str]:
    """The original's command line: ``LARMORX_STATS_BIN`` (a ``larmorx`` binary), else
    ``LARMORX_MATHS_BIN``, else this Python's ``larmorx`` package."""
    exe = os.environ.get("LARMORX_STATS_BIN") or os.environ.get("LARMORX_MATHS_BIN")
    return [exe] if exe else [sys.executable, "-m", "larmorx"]


def run_original(
    args: list[str],
    *,
    cwd: Path,
    settings: dict[str, str | None],
    mode: str,
    threads: int | None = None,
) -> rep.Completed:
    env = rep._environment(settings)
    env[defects.ENV] = mode
    env["OMP_NUM_THREADS"] = str(threads or LX_THREADS)
    env["LARMORX_IMPLEMENTATION"] = "original"
    for k, v in os.environ.items():
        if k in ("PYTHONPATH", "VIRTUAL_ENV"):
            env[k] = v
    p = subprocess.run(
        [*original_command(), "fsl", PROGRAM, *args],
        cwd=cwd,
        env=env,
        capture_output=True,
        text=True,
        errors="replace",
        check=False,
        timeout=3600,
    )
    return rep.Completed(p.returncode, p.stdout, p.stderr)


# ------------------------------------------------------------------------------------------------
# Comparison


def _own(text: str, d: Path) -> str:
    keep = [
        ln
        for ln in text.split("\n")
        if not ln.startswith(("larmorx: ", "larmorx fsl ")) and not ln.endswith("(larmorx)")
    ]
    return "\n".join(keep).replace(str(d), "<dir>")


def _number(word: str) -> float | None:
    try:
        v = float(word)
    except ValueError:
        return None
    return v if np.isfinite(v) else None


def _close(a: float, b: float) -> bool:
    d = abs(a - b)
    m = max(abs(a), abs(b))
    return d <= RELATIVE * (m if m >= 1 else 1.0)


def _bounded_words(args: list[str], n_words: int) -> list[bool] | None:
    """For one pass's printed line of ``n_words`` words: which words §10's bound applies to,
    walking the options; ``None`` when the words cannot be attributed (a histogram that printed
    nothing, a message): then every word must be identical."""
    opts = list(args)
    masked = False
    while opts and opts[0] in ("-t", "-json", "-K"):
        if opts[0] == "-K":
            masked = True  # a label is a mask: the unmasked bound never applies
            opts = opts[2:]
        else:
            opts = opts[1:]
    opts = opts[1:]  # the input
    out: list[bool] = []
    k = 0
    while k < len(opts):
        o = opts[k]
        n_args = _ARGS.get(o, 0)
        if o in ("-h", "-H"):
            try:
                w = int(float(opts[k + 1]))
            except (IndexError, ValueError):
                return None
        else:
            w = _WORDS.get(o, 0)
        bound = o in _BOUND_ALWAYS or (o in _BOUND_UNMASKED and not masked)
        out += [bound] * w
        if o in _MAKE_MASKS:
            masked = True
        k += 1 + n_args
    return out if len(out) == n_words else None


def _compare_text(checks: CheckList, x: str, y: str, args: list[str]) -> None:
    """Standard output (§10): lines and words; numbers identical, or within the bound for the
    options that allow one (no mask when the bound needs none, ``-K`` is a mask)."""
    xs, ys = x.split("\n"), y.split("\n")
    bad, worst = "", 0.0
    if len(xs) != len(ys):
        bad = f"{len(xs)} vs {len(ys)} lines: FSL {x[-160:]!r} / original {y[-160:]!r}"
    for k, (p, q) in enumerate(zip(xs, ys, strict=False)):
        if bad:
            break
        pw, qw = p.split(" "), q.split(" ")
        if len(pw) != len(qw):
            bad = f"line {k + 1}: FSL {p[:160]!r} / original {q[:160]!r}"
            break
        words = [w for w in pw if w]
        bounds = _bounded_words(args, len(words))
        j = 0
        for u, v in zip(pw, qw, strict=True):
            if u == v:
                j += bool(u)
                continue
            a, b = _number(u), _number(v)
            ok = bounds is not None and bounds[j] and a is not None and b is not None
            if ok and _close(a, b):
                worst = max(worst, abs(a - b) / max(1.0, abs(a)))
                j += 1
                continue
            bad = f"line {k + 1}, word {j + 1}: FSL {u!r} / original {v!r}"
            break
    checks(
        "standard output",
        not bad,
        metric="largest relative difference of the bounded numbers",
        value=float(f"{worst:.3g}"),
        threshold=RELATIVE,
        detail=bad,
    )


def _leaves(v, key=""):
    """The numbers of a JSON document with their option's key, in order."""
    if isinstance(v, dict):
        for k in sorted(v):
            yield from _leaves(v[k], k)
    elif isinstance(v, list):
        for x in v:
            yield from _leaves(x, key)
    else:
        yield key, v


def _compare_json(checks: CheckList, x: str, y: str, args: list[str]) -> None:
    """``-json``: the document's text with its numbers taken out identical; its numbers equal
    (their text too), or within the bound for the options that allow one."""
    import re

    num = re.compile(r"-?\d+(?:\.\d+)?(?:[eE][-+]?\d+)?")
    bad, worst = "", 0.0
    if num.sub("#", x) != num.sub("#", y):
        bad = "the documents' text differs beyond their numbers"
    else:
        try:
            jx = json.loads(x[x.index("{") :], parse_float=str, parse_int=str)
            jy = json.loads(y[y.index("{") :], parse_float=str, parse_int=str)
        except (ValueError, json.JSONDecodeError) as e:
            jx = jy = None
            bad = f"not JSON: {e}"
        if jx is not None:
            masked = "-K" in args or any(a in args for a in _MAKE_MASKS)
            for (kx, u), (_, v) in zip(_leaves(jx), _leaves(jy), strict=True):
                if u == v:
                    continue
                bound = kx in _BOUND_ALWAYS or (kx in _BOUND_UNMASKED and not masked)
                a, b = _number(str(u)), _number(str(v))
                if bound and a is not None and b is not None and _close(a, b):
                    worst = max(worst, abs(a - b) / max(1.0, abs(a)))
                    continue
                bad = f"{kx}: FSL {u} / original {v}"
                break
    checks(
        "standard output (JSON)",
        not bad,
        metric="largest relative difference of the bounded numbers",
        value=float(f"{worst:.3g}"),
        threshold=RELATIVE,
        detail=bad,
    )


def compare(
    checks: CheckList, oracle: rep.Completed, ours: rep.Completed, a: Path, b: Path, args: list[str]
) -> None:
    ok_exit = oracle.code == ours.code or (oracle.code < 0 and ours.code == 1)
    checks(
        "exit status",
        ok_exit,
        metric="exact",
        value=f"{ours.code} (FSL {oracle.code})",
        threshold="the same (FSL's crash: 1)",
    )
    so, su = _own(oracle.stdout, a), _own(ours.stdout, b)
    if "-json" in args[:4] and so.strip().endswith("}"):
        _compare_json(checks, so, su, args)
    else:
        _compare_text(checks, so, su, args)
    eo, eu = _own(oracle.stderr, a), _own(ours.stderr, b)
    checks("standard error", eo == eu, detail="" if eo == eu else _first_diff(eo, eu))


def _first_diff(x: str, y: str) -> str:
    xs, ys = x.split("\n"), y.split("\n")
    for k, (p, q) in enumerate(zip(xs, ys, strict=False)):
        if p != q:
            return f"line {k + 1}: FSL {p[:160]!r} / original {q[:160]!r}"
    return f"{len(xs)} vs {len(ys)} lines; FSL {x[-200:]!r} / original {y[-200:]!r}"


def fix_pass(checks: CheckList, prepare, args, settings, reproduced: rep.Completed) -> None:
    """D13: the default run against the ``reproduce`` run: a defect named exactly when the
    output (standard output, exit status) changes."""
    with tempfile.TemporaryDirectory(prefix="lx-stats-fix_") as name:
        tmp = Path(name)
        prepare(tmp)
        r = run_original(args, cwd=tmp, settings=settings, mode=defects.FIX)
    entries = defects.notices_in(r.stderr)
    others = []
    if r.code != reproduced.code:
        others.append(f"exit {reproduced.code} -> {r.code}")
    if r.stdout != reproduced.stdout:
        others.append(f"standard output {_first_diff(reproduced.stdout, r.stdout)[:160]}")
    defects.add_fix_check(
        checks, entries, defects.Change(0, 0, 0.0), other_changes="; ".join(others), silent_ok=()
    )


def _agree_on_errors(checks: CheckList, oracle: rep.Completed) -> None:
    if oracle.code != 0 and all(c.passed for c in checks.checks):
        text = " ".join(oracle.stderr.split())[:160]
        raise Outcome(
            BOTH_ERROR,
            f"fslstats exits {oracle.code}; the same from the original."
            + (f" {text}" if text else ""),
        )


# ------------------------------------------------------------------------------------------------
# Cases


def _run_case(run: rep.Run, checks: CheckList) -> None:
    settings = dict(run.settings)
    with (
        tempfile.TemporaryDirectory(prefix="lx-stats-") as a_,
        tempfile.TemporaryDirectory(prefix="lx-stats-") as b_,
    ):
        a, b = Path(a_), Path(b_)
        args = _prepare(run, a)
        _prepare(run, b)
        ours = run_original(args, cwd=b, settings=settings, mode=defects.REPRODUCE)
        oracle = rep.run_oracle(args, cwd=a, settings=settings)
        compare(checks, oracle, ours, a, b, args)
        if oracle.code < 0:
            # FSL crashed (the original refuses in reproduce): the default refuses or corrects,
            # naming the same defect
            ids = defects.notices_in(ours.stderr) or [
                w.strip("():,") for w in ours.stderr.split() if w.strip("():,").startswith("FSL-")
            ]
            with tempfile.TemporaryDirectory(prefix="lx-stats-fix-") as c_:
                c = Path(c_)
                _prepare(run, c)
                r = run_original(args, cwd=c, settings=settings, mode=defects.FIX)
            named = set(defects.notices_in(r.stderr)) | {
                w.strip("():,") for w in r.stderr.split() if w.strip("():,").startswith("FSL-")
            }
            checks(
                "D13: FSL crashes; the default refuses or corrects, naming the defect",
                bool(ids) and set(ids) <= named and r.code in (0, 1),
                detail=f"reproduce {ids}, default exit {r.code} {sorted(named)}",
            )
        else:
            fix_pass(checks, lambda t: _prepare(run, t), args, settings, ours)
    _agree_on_errors(checks, oracle)


def _goodvoxels_case(payload: tuple[str, str], checks: CheckList) -> None:
    run, node = payload
    base = rep._goodvoxels_chain(run)
    if base is None:
        raise Outcome(SKIPPED, f"the oracle run {run} is not available")
    template = next(t for n, _, t in rep.GOODVOXELS_STEPS if n == node)
    src = template[0][1:-1]
    image = base / src / f"{src}.nii.gz"
    args = [image.name, *template[1:]]
    _real_case(checks, [image], args)


def _bet_case(checks: CheckList) -> None:
    image = rep._bet_image()
    if image is None:
        raise Outcome(SKIPPED, f"the oracle run {rep.BET_RUN} is not available")
    _real_case(checks, [(image, "t1w.nii.gz")], ["t1w.nii.gz", "-l", "0.001", "-C"])


def _real_case(checks: CheckList, files, args: list[str]) -> None:
    def prepare(t: Path) -> None:
        for f in files:
            src, name = f if isinstance(f, tuple) else (f, f.name)
            shutil.copy(src, t / name)

    with (
        tempfile.TemporaryDirectory(prefix="lx-stats-real-") as a_,
        tempfile.TemporaryDirectory(prefix="lx-stats-real-") as b_,
    ):
        a, b = Path(a_), Path(b_)
        prepare(a)
        prepare(b)
        oracle = rep.run_oracle(args, cwd=a, settings={})
        ours = run_original(args, cwd=b, settings={}, mode=defects.REPRODUCE)
        compare(checks, oracle, ours, a, b, args)
        if ours.code == 0:
            fix_pass(checks, prepare, args, {}, ours)
    _agree_on_errors(checks, oracle)


def _own_cases() -> dict[str, tuple[str, str, rep.Run]]:
    """Paths the replica suite's cases do not reach (black-box runs of the clean-room part)."""
    T = "standard"
    r = rep._r
    return {
        "orig/P-makes-masks": (
            T,
            "-P makes the masks from the thresholds and multiplies the image by them (+inf "
            "becomes NaN for the later -D)",
            r("@orig:rampinf", "-P", "50", "-D"),
        ),
        "orig/w-makes-masks": (T, "-w makes the masks likewise", r("@orig:rampinf", "-w", "-D")),
        "orig/e-makes-masks": (T, "-e makes the masks likewise", r("@orig:rampinf", "-e", "-D")),
        "orig/zeros-M": (T, "-M without nonzero values (0, no message)", r("@zeros3", "-M")),
        "orig/zeros-sS": (
            T,
            "-m, -s, -S, -V without nonzero values",
            r("@zeros3", "-m", "-s", "-S", "-V"),
        ),
        "orig/json-K-none": (
            T,
            "-json -K without a positive label: an empty list of indices",
            r("-json", "-K", "@labneg", "@a", "-m"),
        ),
        "orig/e-empty-mask": (
            T,
            "-l 100 -e -E: the histogram's message, then FSL-185 (FSL crashes)",
            r("@a", "-l", "100", "-e", "-E"),
        ),
        "orig/p-nan": (T, "-p nan -P nan: the smallest value", r("@a", "-p", "nan", "-P", "nan")),
        "orig/t-3d": (T, "-t on a 3D image: one pass", r("-t", "@a", "-m")),
    }


def cases(tier: str) -> list[Case]:
    from larmorx_validation.parity.harness import in_tier

    out = []
    for k, (run, node) in enumerate(rep.goodvoxels_cases()):
        t = "smoke" if k == 0 else "standard"
        if in_tier(t, tier):
            out.append(
                Case(
                    f"fmriprep-goodvoxels/{run}/{node}",
                    "fmriprep",
                    f"fMRIPrep's goodvoxels mask, node {node} (--project-goodvoxels; the "
                    "ribbon a stand-in: the segmentation's grey matter)",
                    ("goodvoxels", (run, node)),
                )
            )
    if in_tier("standard", tier) and rep._bet_image() is not None:
        out.append(
            Case(
                f"bet/{rep.BET_RUN}/T1w",
                "bet",
                "BET's call: fslstats <T1w> -l 0.001 -C (fMRIPrep's preprocessed T1w)",
                ("bet", None),
            )
        )
    for name, (t, desc, _run) in replica_cases().items():
        if in_tier(t, tier):
            out.append(Case(name, name.split("/")[0], desc, ("run", name)))
    for name, (t, desc, _run) in _own_cases().items():
        if in_tier(t, tier):
            out.append(Case(name, "orig", desc, ("own", name)))
    return out


def run_case(case: Case, checks: CheckList) -> None:
    kind, payload = case.payload
    if kind == "goodvoxels":
        _goodvoxels_case(payload, checks)
    elif kind == "bet":
        _bet_case(checks)
    elif kind == "own":
        _run_case(_own_cases()[payload][2], checks)
    else:
        _run_case(replica_cases()[payload][2], checks)


def suite() -> Suite:
    import larmorx

    return Suite(
        name=SUITE,
        title="fslstats (clean-room original)",
        tool=f"`larmorx fsl fslstats` (larmorx {larmorx.__version__}, crate larmorx-fsl: the "
        "clean-room original written from specs/fslstats.md, Apache-2.0)",
        reference="fslstats from FSL 6.0.7.23 (fsl-avwutils 2209.6), the conda build at "
        "`~/fsl-6.0.7.23`, as built: glibc 2.35's maths",
        thresholds=[
            ("Exit status", "the same (FSL's crash: the original exits 1)"),
            (
                "Standard error",
                "identical (the run directory replaced; the original's `larmorx` lines left out)",
            ),
            (
                "Standard output",
                "identical text; the numbers of -M, -S, -m, -s without a mask, -D, -e, -E, -c "
                "and -C within 1e-6 relative (absolute below 1); -json: the text without its "
                "numbers identical, the numbers by the same rule (§10)",
            ),
            ("D13 fix pass", "a defect named exactly when the default's output differs"),
        ],
        cases=cases,
        run_case=run_case,
        packages=("nibabel",),
        notes=(
            "The cases are the replica suite's (`fsl-stats-replica`) but its `fsltest/` ones "
            "(FSL's own tests, in FSL's source tree, which the clean-room implementer does not "
            "open); FSL's runs are served from the oracle cache under that suite's keys. The "
            "original is compared in `reproduce` mode (D13) and its default `fix` pass against "
            "the reproduced run. The goodvoxels calls run on FSL's own chain of fslmaths "
            "outputs (the ribbon a stand-in: the segmentation's grey matter, as the no-reconall "
            "oracle runs have no FreeSurfer ribbon)."
        ),
    )


__all__ = ["suite"]
