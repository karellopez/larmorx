# SPDX-License-Identifier: Apache-2.0
"""Running random cases: one case in a worker, a night of them across tools.

A **case** (:func:`run_case`) is drawn from (tool, seed, case number), run through the tool's
parity runner for each implementation, judged (:func:`~larmorx_validation.difftest.core.judge`)
and, where it differs, re-run with CORE-MATH preloaded into the original (the ANTs programs
whose suite compares only with ANTs as built) and shrunk
(:mod:`~larmorx_validation.difftest.shrink`). It runs in a worker process of its own process
group, with one thread for the original and larmorx (``--threads``), so a hung program is
stopped by killing the group.

**The oracle cache** (``docs/validation/difftest.md``): random cases run the originals with the
cache off (``LARMORX_ORACLE_CACHE_MODE=off``: nothing is read or stored), so random inputs
never make it grow; a case that differs is run once more, shrunk, with the cache on, so its
reproducer's original runs are stored and replay exactly (the parity suites then find them).

**A night** (:func:`night`): tools take turns by their weight until the time budget is spent,
``--jobs`` cases at a time; each case's line goes to ``results.jsonl`` as it ends, each
difference to ``findings/<tool>/``, and ``report.md`` and ``summary.json`` are written at the
end (:mod:`~larmorx_validation.difftest.report`).
"""

from __future__ import annotations

import contextlib
import json
import multiprocessing
import os
import re
import signal
import tempfile
import time
import traceback
from multiprocessing.connection import Connection, wait
from pathlib import Path

from larmorx_validation.difftest import core, gen, inputs, tools
from larmorx_validation.difftest.core import DIFFER, EXPLAINED, NOTICE, SKIP, Judgement
from larmorx_validation.parity import harness

TIMEOUT = "timeout"
#: How many cases with the same difference (tool, implementation, failed checks) are shrunk in a
#: night; later ones are counted and kept unshrunk (shrinking is the night's costliest part).
SHRINKS_PER_SIGNATURE = 2


class CaseTimeout(BaseException):  # noqa: N818 - not an error of the case: the clock's
    """A case ran past its time (raised by an alarm; not an ``Exception``, so the parity
    harness does not turn it into a failed check)."""


def _alarm(signum, frame):
    raise CaseTimeout()


# ------------------------------------------------------------------------------------------------
# One case, in this process


def limit_threads(threads: int) -> None:
    """The suites' thread counts for larmorx and the originals, at ``threads`` (random cases
    compare at one thread; the thread bands stay the parity suites' job)."""
    from larmorx_validation.parity import (
        afni_replica,
        ants_ai,
        ants_atropos,
        ants_programs,
        ants_registration,
        ants_threaded,
    )

    ants_programs.THREADS = threads
    ants_threaded.LX_THREADS = threads
    ants_registration.LX_THREADS = threads
    ants_ai.LX_THREADS = threads
    ants_atropos.LX_THREADS = threads
    os.environ["ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS"] = str(threads)
    os.environ["OMP_NUM_THREADS"] = str(threads)
    os.environ["LARMORX_PARITY_BAND_THREADS"] = ""
    _ = afni_replica


UNDEFINED = "undefined"
#: Defects whose original result depends on memory it never wrote (or on the clock): the oracle
#: cache's list, and the class (a) reads beyond a buffer the register names. When larmorx names
#: one while reproducing the original, the values are not comparable (the parity suites'
#: ``undefined_values``): the verdict is ``undefined``.
UNDEFINED_DEFECTS: dict[str, str] = {
    "ANTS-07": "a smaller second image is read beyond its voxels (memory)",
    "ANTS-19": "FillHoles reads outside the image (memory)",
    "ANTS-55": "antsAI reads a translation grid past the end of its extents (memory)",
    "ANTS-63": "the VTK reader's labels are uninitialised memory",
    "ANTS-77": "a B-spline mesh read past the end of its list (garbage indices)",
    "ITK-25": "the manifold Parzen windows read past the end of their neighbours (memory)",
    "SCI-01": "map_coordinates reads outside the array for huge or NaN coordinates (memory)",
    "AFNI-04": "a shift beyond the series reads past it (memory)",
}

#: A refusal that names its defect (``... an error (ANTS-160)``): larmorx refuses a class (a)
#: case in both modes, so no notice names it while reproducing.
_REFUSAL = re.compile(r"\(([A-Z]+-\d+)\)")

#: The D13 notices larmorx's in-process runs printed in ``reproduce`` mode during a case.
_NOTICES: list[str] = []


def _watch_notices() -> None:
    """Wraps ``larmorx._core.cli_main`` (every suite's in-process larmorx run goes through it) so
    the defects larmorx names while reproducing the original are known to the verdict: a
    defect whose original result depends on memory it never wrote or on the clock
    (:data:`~larmorx_validation.oracle_cache.policy.VARYING_DEFECTS`) makes the values
    incomparable, as the parity suites' ``undefined_values`` says."""
    from larmorx import _core

    if getattr(_core.cli_main, "_difftest", False):
        return
    original = _core.cli_main

    def cli_main(*args, **kw):
        code, out, err = original(*args, **kw)
        if os.environ.get("LARMORX_UPSTREAM_DEFECTS") == "reproduce":
            from larmorx_validation.parity.defects import notices_in

            _NOTICES.extend(notices_in(err))
            # A class (a) case larmorx refuses in both modes names its defect in the error.
            if code != 0:
                _NOTICES.extend(_REFUSAL.findall(err if isinstance(err, str) else str(err)))
        return code, out, err

    cli_main._difftest = True
    _core.cli_main = cli_main


def execute(tool: core.Tool, recipe: dict, impl: str, *, timeout: float | None = None, **case_kw) -> tuple[Judgement, float, harness.CaseResult | None]:
    """Runs one implementation of ``recipe`` through its parity runner and judges it."""
    from larmorx_validation.oracle_cache.policy import VARYING_DEFECTS

    undefined_defects = {**VARYING_DEFECTS, **UNDEFINED_DEFECTS}
    _watch_notices()
    _NOTICES.clear()
    case = tool.case(recipe, impl, **case_kw)
    suite = harness.Suite(
        name=f"difftest-{tool.name}",
        title=tool.title,
        tool=tool.title,
        reference="",
        thresholds=[],
        cases=lambda tier: [],
        run_case=tool.run(impl),
    )
    start = time.monotonic()
    limit = int(max(1, round(timeout if timeout is not None else tool.timeout)))
    previous = signal.signal(signal.SIGALRM, _alarm)
    signal.alarm(limit)
    try:
        result = harness.run_one(suite, case)
    except CaseTimeout:
        return Judgement(TIMEOUT, ("timeout",), f"no result within {limit} s"), time.monotonic() - start, None
    except BaseException as e:  # noqa: BLE001 - a Rust panic is a BaseException (PyO3)
        if type(e).__name__ != "PanicException":
            raise
        return Judgement(DIFFER, ("larmorx panicked",), f"PanicException: {e}"[:600], "fail"), time.monotonic() - start, None
    finally:
        signal.alarm(0)
        signal.signal(signal.SIGALRM, previous)
        inputs.release(f"run/{gen.digest(recipe['inputs'])}")
    j = core.judge(tool, impl, result)
    # The defects larmorx named while reproducing, and those the descriptor knows the drawn
    # case meets (``extra["undefined"]``: e.g. a clock-seeded sampling, ANTS-82).
    named = {*_NOTICES, *recipe.get("extra", {}).get("undefined", ())}
    varying = sorted({n for n in named if n in undefined_defects})
    if j.verdict == DIFFER and varying:
        j = Judgement(UNDEFINED, tuple(f"undefined: {n}" for n in varying), f"{', '.join(varying)}: {undefined_defects[varying[0]]}; {j.detail}", j.status, j.checks)
    return j, time.monotonic() - start, result


@contextlib.contextmanager
def _correctly_rounded():
    from larmorx_validation.parity.ants_programs import crmath_preload

    lib = crmath_preload()
    saved = os.environ.get("LD_PRELOAD")
    if lib is not None:
        os.environ["LD_PRELOAD"] = str(lib)
    try:
        yield lib is not None
    finally:
        if saved is None:
            os.environ.pop("LD_PRELOAD", None)
        else:
            os.environ["LD_PRELOAD"] = saved


@contextlib.contextmanager
def cache_mode(mode: str):
    from larmorx_validation import oracle_cache

    with oracle_cache.using(mode):
        yield


def judge_once(tool: core.Tool, recipe: dict, impl: str, explore_mode: str = "off") -> Judgement:
    """The verdict of ``recipe`` (CORE-MATH's explanation included), as shrinking needs it."""
    with cache_mode(explore_mode):
        j, _, _ = execute(tool, recipe, impl)
        if j.verdict == DIFFER and tool.crash_kw and _original_crashed(j):
            # The original crashed or aborted: compared the suite's way for such a run.
            j2, _, _ = execute(tool, recipe, impl, **tool.crash_kw)
            if j2.verdict != DIFFER or set(j2.signature) != set(j.signature):
                j = j2
        if j.verdict == DIFFER and tool.crmath_retry:
            with _correctly_rounded() as ok:
                if ok:
                    j2, _, _ = execute(tool, recipe, impl)
                    if j2.verdict == core.AGREE:
                        return Judgement(EXPLAINED, j.signature, "identical with CORE-MATH preloaded into the original: " + j.detail, j.status, j.checks)
    return j


_CRASHED = re.compile(r"ANTs (?:exit )?-\d+\b|ANTs killed by signal|ANTs \(?signal|by signal \d+|ANTs crash")


def _original_crashed(j: Judgement) -> bool:
    return bool(_CRASHED.search(j.detail)) or any(
        _CRASHED.search(str(c.get("detail", ""))) for c in j.checks if not c.get("passed")
    )


def run_case(
    tool_name: str,
    seed: str,
    n: int,
    *,
    shrink: bool = True,
    shrink_budget: float = 600.0,
    recipe: dict | None = None,
    store_findings: bool = True,
    shrunk_enough: list | None = None,
) -> dict:
    """Case ``n`` of ``tool_name`` for ``seed``: every implementation, judged; differences
    shrunk. Returns the case's line for ``results.jsonl`` (with the findings' details)."""
    from larmorx_validation.difftest import shrink as shrinking

    tool = tools.get(tool_name)
    start = time.monotonic()
    line: dict = {"tool": tool_name, "seed": seed, "n": n}
    try:
        recipe = recipe or core.make_recipe(tool, seed, n)
    except Exception:
        line.update(verdict="draw-error", detail=traceback.format_exc(limit=4)[-800:])
        return line
    line["recipe_id"] = core.recipe_id(recipe)
    line["options"] = recipe["options"]
    line["features"] = recipe.get("features", {})
    line["impls"] = {}
    line["findings"] = []
    for impl in tool.implementations:
        j = judge_once(tool, recipe, impl)
        entry: dict = {"verdict": j.verdict, "signature": list(j.signature), "detail": j.detail[:600], "status": j.status}
        again = [impl, *sorted(j.signature)] in (shrunk_enough or [])
        if j.verdict == DIFFER and again:
            # This difference was shrunk often enough tonight: counted, its recipe kept, not shrunk.
            entry["not_shrunk"] = "shrunk often enough tonight"
            line["findings"].append(
                {
                    "tool": tool_name,
                    "implementation": impl,
                    "seed": seed,
                    "n": n,
                    "signature": list(j.signature),
                    "detail": j.detail,
                    "checks": j.checks,
                    "recipe": recipe,
                    "shrunk": recipe,
                    "shrink_moves": [],
                    "shrink_runs": 0,
                    "not_shrunk": True,
                }
            )
        elif j.verdict == DIFFER and shrink:
            def attempt(r: dict, impl: str = impl) -> Judgement:
                return judge_once(tool, r, impl)

            small, kept, last, runs = shrinking.shrink(recipe, j.signature, attempt, budget_s=shrink_budget, extra=tool.shrink)
            finding = {
                "tool": tool_name,
                "implementation": impl,
                "seed": seed,
                "n": n,
                "signature": list(j.signature),
                "detail": j.detail,
                "checks": j.checks,
                "recipe": recipe,
                "shrunk": small,
                "shrink_moves": kept,
                "shrink_runs": runs,
                "shrunk_detail": (last.detail if last else j.detail),
                "shrunk_checks": (last.checks if last else j.checks),
            }
            if store_findings:
                # The shrunk reproducer's original runs go into the oracle cache, so it replays.
                with cache_mode("use"):
                    stored, _, _ = execute(tool, small, impl)
                finding["stored_verdict"] = stored.verdict
            line["findings"].append(finding)
            entry["shrunk_moves"] = len(kept)
        line["impls"][impl] = entry
    line["seconds"] = round(time.monotonic() - start, 2)
    verdicts = [e["verdict"] for e in line["impls"].values()]
    line["verdict"] = next((v for v in (DIFFER, TIMEOUT, UNDEFINED, NOTICE, EXPLAINED, SKIP) if v in verdicts), core.AGREE)
    return line


# ------------------------------------------------------------------------------------------------
# Crashes: a case that kills its process (larmorx aborting) runs in a child process of its own


CRASH = "crash"


def isolated_judge(tool_name: str, recipe: dict, impl: str, timeout: float) -> Judgement:
    """The verdict of ``recipe`` judged in a child process; a child killed by a signal (an
    abort, a segmentation fault) is a difference with the signature ``crash``."""
    import subprocess
    import sys

    with tempfile.TemporaryDirectory(prefix="lx-dt-crash-") as t:
        path = Path(t) / "recipe.json"
        path.write_text(json.dumps({"tool": tool_name, "recipe": recipe}), encoding="utf-8")
        env = {**os.environ, "LARMORX_ORACLE_CACHE_MODE": os.environ.get("LARMORX_ORACLE_CACHE_MODE", "off")}
        try:
            p = subprocess.run(
                [sys.executable, "-m", "larmorx_validation", "difftest", "--judge-recipe", str(path), "--impl", impl],
                capture_output=True,
                text=True,
                errors="replace",
                env=env,
                timeout=timeout,
                check=False,
            )
        except subprocess.TimeoutExpired:
            return Judgement(TIMEOUT, ("timeout",), f"no result within {timeout:.0f} s")
    if p.returncode < 0 or not p.stdout.strip().startswith("{"):
        tail = " ".join(p.stderr.strip().splitlines()[-3:])[-400:]
        return Judgement(DIFFER, (CRASH,), f"the process ended by signal {-p.returncode}: {tail}" if p.returncode < 0 else f"exit {p.returncode}: {tail}", "crash")
    d = json.loads(p.stdout.strip().splitlines()[-1])
    return Judgement(d["verdict"], tuple(d["signature"]), d["detail"], d.get("status", ""), d.get("checks", []))


def crash_case(tool_name: str, seed: str, n: int, *, shrink_budget: float = 600.0, **_) -> dict:
    """A case whose run killed its worker: each implementation judged in a child process, the
    crash shrunk the same way (each candidate in a child of its own)."""
    from larmorx_validation.difftest import shrink as shrinking

    tool = tools.get(tool_name)
    recipe = core.make_recipe(tool, seed, n)
    line: dict = {"tool": tool_name, "seed": seed, "n": n, "recipe_id": core.recipe_id(recipe), "options": recipe["options"], "features": recipe.get("features", {}), "impls": {}, "findings": [], "isolated": True}
    start = time.monotonic()
    for impl in tool.implementations:
        j = isolated_judge(tool_name, recipe, impl, tool.timeout * 2)
        line["impls"][impl] = {"verdict": j.verdict, "signature": list(j.signature), "detail": j.detail[:600], "status": j.status}
        if j.verdict == DIFFER:
            def attempt(r: dict, impl: str = impl) -> Judgement:
                return isolated_judge(tool_name, r, impl, tool.timeout * 2)

            small, kept, last, runs = shrinking.shrink(recipe, j.signature, attempt, budget_s=shrink_budget, extra=tool.shrink)
            line["findings"].append(
                {
                    "tool": tool_name,
                    "implementation": impl,
                    "seed": seed,
                    "n": n,
                    "signature": list(j.signature),
                    "detail": j.detail,
                    "checks": j.checks,
                    "recipe": recipe,
                    "shrunk": small,
                    "shrink_moves": kept,
                    "shrink_runs": runs,
                    "shrunk_detail": last.detail if last else j.detail,
                    "shrunk_checks": last.checks if last else j.checks,
                }
            )
    line["seconds"] = round(time.monotonic() - start, 2)
    verdicts = [e["verdict"] for e in line["impls"].values()]
    line["verdict"] = CRASH if any(e["signature"] == [CRASH] for e in line["impls"].values()) else next((v for v in (DIFFER, TIMEOUT, UNDEFINED, NOTICE, EXPLAINED, SKIP) if v in verdicts), core.AGREE)
    return line


# ------------------------------------------------------------------------------------------------
# Workers


def _worker(conn: Connection, threads: int) -> None:
    os.setpgrp()
    os.environ["LARMORX_ORACLE_CACHE_MODE"] = "off"
    limit_threads(threads)
    while True:
        try:
            task = conn.recv()
        except EOFError:
            return
        if task is None:
            return
        try:
            kind = task.pop("kind", "case")
            line = crash_case(**task) if kind == CRASH else run_case(**task)
        except BaseException:  # noqa: BLE001 - report everything to the parent
            line = {**{k: task.get(k) for k in ("tool_name", "seed", "n")}, "verdict": "harness-error", "detail": traceback.format_exc(limit=6)[-1500:]}
        conn.send(line)


class Pool:
    """Worker processes, each in its own process group; a task past its deadline kills its
    group (the worker and any original program it started) and a new worker takes its place."""

    def __init__(self, jobs: int, threads: int):
        self.ctx = multiprocessing.get_context("spawn")
        self.threads = threads
        self.workers: list[dict] = [self._start() for _ in range(jobs)]

    def _start(self) -> dict:
        parent, child = self.ctx.Pipe()
        p = self.ctx.Process(target=_worker, args=(child, self.threads), daemon=True)
        p.start()
        child.close()
        return {"proc": p, "conn": parent, "task": None, "deadline": None, "started": None}

    def idle(self) -> list[dict]:
        return [w for w in self.workers if w["task"] is None]

    def submit(self, w: dict, task: dict, deadline_s: float) -> None:
        w["task"], w["started"] = task, time.monotonic()
        w["deadline"] = w["started"] + deadline_s
        w["conn"].send(dict(task))

    def busy(self) -> int:
        return sum(w["task"] is not None for w in self.workers)

    def collect(self, timeout: float = 5.0) -> list[tuple[dict, dict]]:
        """Finished (task, line) pairs; tasks past their deadline as timeouts."""
        out = []
        conns = [w["conn"] for w in self.workers if w["task"] is not None]
        ready = wait(conns, timeout=timeout) if conns else []
        for w in self.workers:
            if w["task"] is None:
                continue
            if w["conn"] in ready:
                task = w["task"]
                try:
                    line = w["conn"].recv()
                except (EOFError, OSError):
                    code = w["proc"].exitcode
                    line = {"verdict": CRASH, "detail": f"the worker died (exit code {code}): larmorx or the harness crashed"}
                    self._replace(w)
                out.append((task, line))
                w["task"] = None
            elif time.monotonic() > w["deadline"]:
                task, started = w["task"], w["started"]
                self._replace(w)
                out.append((task, {"verdict": TIMEOUT, "detail": f"killed after {time.monotonic() - started:.0f} s"}))
        return out

    def _replace(self, w: dict) -> None:
        with contextlib.suppress(ProcessLookupError, PermissionError):
            os.killpg(w["proc"].pid, signal.SIGKILL)
        w["proc"].join(timeout=10)
        fresh = self._start()
        w.update(fresh)

    def close(self) -> None:
        for w in self.workers:
            with contextlib.suppress(OSError, BrokenPipeError):
                w["conn"].send(None)
        for w in self.workers:
            w["proc"].join(timeout=10)
            if w["proc"].is_alive():
                with contextlib.suppress(ProcessLookupError, PermissionError):
                    os.killpg(w["proc"].pid, signal.SIGKILL)


# ------------------------------------------------------------------------------------------------
# A night


def night(
    out: Path,
    *,
    tool_names: list[str],
    seed: str,
    budget_s: float,
    jobs: int,
    threads: int = 1,
    shrink_budget: float = 600.0,
    max_cases: int | None = None,
    log=print,
) -> dict:
    """Runs random cases of ``tool_names`` for ``budget_s`` seconds, ``jobs`` at a time, into
    ``out`` (see the module's documentation). Returns the run's description."""
    from larmorx_validation.difftest import report

    out.mkdir(parents=True, exist_ok=True)
    selected = []
    skipped = {}
    for name in tool_names:
        t = tools.get(name)
        why = t.unavailable()
        if why:
            skipped[name] = why
        else:
            selected.append(t)
    info = {
        "command": "python -m larmorx_validation difftest",
        "seed": seed,
        "budget_s": budget_s,
        "jobs": jobs,
        "threads": threads,
        "shrink_budget_s": shrink_budget,
        "tools": [t.name for t in selected],
        "unavailable": skipped,
        "started": time.strftime("%Y-%m-%dT%H:%M:%S"),
        "environment": _environment(),
    }
    (out / "run.json").write_text(json.dumps(info, indent=1), encoding="utf-8")
    results = (out / "results.jsonl").open("a", encoding="utf-8")
    spent = {t.name: 0.0 for t in selected}
    counts = {t.name: 0 for t in selected}
    next_n = {t.name: _resume(out, t.name) for t in selected}
    pool = Pool(jobs, threads)
    crashes: list[dict] = []
    shrinks: dict[tuple, int] = {}
    started = time.monotonic()
    try:
        while True:
            elapsed = time.monotonic() - started
            open_tools = [t for t in selected if max_cases is None or counts[t.name] < max_cases]
            for w in pool.idle():
                if crashes:
                    task = crashes.pop(0)
                    ct = tools.get(task["tool_name"])
                    pool.submit(w, task, deadline_s=ct.timeout * 4 * len(ct.implementations) + shrink_budget * 2 + 300)
                    continue
                if elapsed >= budget_s or not open_tools:
                    break
                t = min(open_tools, key=lambda t: (spent[t.name] / t.weight, t.name))
                n = next_n[t.name]
                next_n[t.name] += 1
                counts[t.name] += 1
                spent[t.name] += t.timeout * 0.05  # a reservation until the case reports
                task = {
                    "tool_name": t.name,
                    "seed": seed,
                    "n": n,
                    "shrink_budget": shrink_budget,
                    "shrunk_enough": [list(k[1:]) for k, c in shrinks.items() if k[0] == t.name and c >= SHRINKS_PER_SIGNATURE],
                }
                pool.submit(w, task, deadline_s=t.timeout * len(t.implementations) * 3 + shrink_budget * 2 + 120)
                open_tools = [t for t in selected if max_cases is None or counts[t.name] < max_cases]
            if pool.busy() == 0 and not crashes and (elapsed >= budget_s or not open_tools):
                break
            for task, line in pool.collect():
                line = {"tool": task["tool_name"], "seed": task["seed"], "n": task["n"], **line}
                spent[line["tool"]] += float(line.get("seconds") or (time.monotonic() - started) * 0)
                if line.get("verdict") == TIMEOUT and "seconds" not in line:
                    spent[line["tool"]] += tools.get(line["tool"]).timeout
                if line.get("verdict") == CRASH and task.get("kind") != CRASH:
                    crashes.append({"kind": CRASH, "tool_name": task["tool_name"], "seed": task["seed"], "n": task["n"], "shrink_budget": shrink_budget})
                findings = line.pop("findings", [])
                for f in findings:
                    if not f.get("not_shrunk"):
                        k = (f["tool"], f["implementation"], *sorted(f["signature"]))
                        shrinks[k] = shrinks.get(k, 0) + 1
                    path = _write_finding(out, f)
                    line.setdefault("finding_files", []).append(str(path.relative_to(out)))
                results.write(json.dumps(line, sort_keys=True) + "\n")
                results.flush()
                if line.get("verdict") not in (core.AGREE,):
                    log(f"{line['tool']} #{line['n']}: {line.get('verdict')} {line.get('impls', '')}"[:300])
    finally:
        pool.close()
        results.close()
    info["ended"] = time.strftime("%Y-%m-%dT%H:%M:%S")
    info["elapsed_s"] = round(time.monotonic() - started, 1)
    (out / "run.json").write_text(json.dumps(info, indent=1), encoding="utf-8")
    report.write(out)
    return info


def _resume(out: Path, tool: str) -> int:
    """The next case number of ``tool`` in a night that already ran some (a run resumed)."""
    path = out / "results.jsonl"
    if not path.is_file():
        return 0
    ns = [json.loads(ln).get("n", -1) for ln in path.read_text(encoding="utf-8").splitlines() if f'"tool": "{tool}"' in ln]
    return max(ns, default=-1) + 1


def _write_finding(out: Path, f: dict) -> Path:
    d = out / "findings" / f["tool"]
    d.mkdir(parents=True, exist_ok=True)
    path = d / f"{f['seed']}-{f['n']:05d}-{f['implementation']}.json"
    path.write_text(json.dumps(f, indent=1, sort_keys=True, default=str), encoding="utf-8")
    return path


def _environment() -> dict:
    import platform
    import subprocess

    from larmorx_validation import environment

    repo = Path(__file__).resolve().parents[4]
    try:
        commit = subprocess.run(["git", "-C", str(repo), "describe", "--always", "--dirty", "--abbrev=12"], capture_output=True, text=True, check=False).stdout.strip()
    except OSError:
        commit = ""
    env = {"larmorx_commit": commit, "machine": platform.machine(), "cpus": os.cpu_count()}
    with contextlib.suppress(Exception):
        env["packages"] = environment.describe(("nibabel", "numpy"))
    return env


__all__ = ["CRASH", "TIMEOUT", "UNDEFINED", "CaseTimeout", "Pool", "crash_case", "execute", "isolated_judge", "judge_once", "limit_threads", "night", "run_case"]

