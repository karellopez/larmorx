# SPDX-License-Identifier: Apache-2.0
"""``larmorx nipype run`` against nipype's own ``nipypecli run``, case by case.

Each case runs ``nipypecli run <args>`` (nipype 1.12.0 with niworkflows 1.15.4 and fMRIPrep
25.2.7, this environment's) and larmorx's :func:`larmorx.nipype.run` with the same arguments
(``prog="nipypecli run"``, so the messages name the same command), each in a fresh folder, and
compares the exit status, the standard output (the folder's path and nipype's log timestamps
written as placeholders), the standard error (nipype's tracebacks as their first and last lines,
without Python's "Did you mean" suggestion; larmorx's notices of defects counted apart) and every
file written, byte for byte (gzipped files by their content). larmorx runs with
``upstream_defects="reproduce"`` (nipype's behaviour, its command line's defects included) and
again with ``"fix"``, which must give the same result but where a case names the defect it
corrects (PIPE-81, PIPE-82): there nipype's command line refuses the input, and larmorx's result is
compared with nipype's interface run through its Python API on the same inputs.

Cases: the click layer (no module, ``--list``, ``--list=x``, ``-h``, ``--``, an empty or unknown
interface, a module larmorx does not serve), every interface's help (all 61 of the nine modules,
those larmorx does not run and those nipype's command line cannot build included, at 80 and 132
columns), argparse's errors, traits' errors (a missing file, an enumeration, a range, a list's
element), ``xor``, ``requires`` and ``deprecated``, the runs of every interface larmorx runs on
generated inputs, with options, and (``--tier full``) fMRIPrep's real calls: the confounds nodes
of three oracle runs (fMRIPrep 25.2.6's working folders: ``ds000005`` single-band, ``ds005454``
multiband with 280 volumes, ``ds003592`` multi-echo) with their own inputs.

Run it in the newest environment with the worktree's extension::

    PYTHONPATH=<workspace>/tags/fmriprep-25.2.7 NIPYPE_NO_ET=1 OPENBLAS_CORETYPE=Haswell \\
        OPENBLAS_NUM_THREADS=1 OPENBLAS_L2_SIZE=512 LARMORX_WORKTREE=<worktree> \\
        <workspace>/oracles/python-sdcflows-2.17/bin/python -P check_nipype_cli.py \\
        [--tier quick|full] [--cases PATTERN]

(``LARMORX_WORKSPACE``: the workspace, for the oracle runs.) Exit status 1 when a case differs.
"""

from __future__ import annotations

import argparse
import contextlib
import fnmatch
import gzip
import os
import re
import shutil
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parent))
from check_nipype_cli_cases import WORKSPACE, WORKTREE, Case, cases, make_inputs  # noqa: E402

NIPYPECLI = Path(sys.executable).with_name("nipypecli")
LARMORX = (
    "import sys; from larmorx.nipype import run; "
    "sys.exit(run(sys.argv[2:], prog='nipypecli run', upstream_defects=sys.argv[1], n_threads=1))"
)


@dataclass
class Result:
    code: int
    out: str
    err: str
    notices: list[str]
    files: dict[str, bytes]


def env(columns: int) -> dict[str, str]:
    e = dict(os.environ)
    e["COLUMNS"] = str(columns)
    e["NIPYPE_NO_ET"] = "1"
    e["PYTHONPATH"] = os.pathsep.join(
        [str(WORKTREE / "python"), str(WORKSPACE / "tags" / "fmriprep-25.2.7")]
    )
    for k in ("LARMORX_UPSTREAM_DEFECTS", "OMP_NUM_THREADS"):
        e.pop(k, None)
    return e


TIME = re.compile(r"^\d{6}-\d\d:\d\d:\d\d,\d+ ", re.M)


def norm_out(text: str, cwd: Path) -> str:
    return TIME.sub("<time> ", text.replace(str(cwd), "<cwd>"))


def norm_err(text: str, cwd: Path) -> tuple[str, list[str]]:
    """A traceback as its first and last lines; larmorx's notices apart."""
    text = text.replace(str(cwd), "<cwd>")
    lines = text.splitlines()
    notices = [ln for ln in lines if "UpstreamDefectWarning" in ln]
    keep = []
    skip_next = False
    for ln in lines:
        if skip_next:
            skip_next = False
            continue
        if "UpstreamDefectWarning" in ln:
            skip_next = True  # the warning's source line
            continue
        keep.append(ln)
    if "Traceback (most recent call last):" in keep:
        last = [ln for ln in keep if ln and not ln.startswith((" ", "\t"))][-1]
        last = re.sub(r"\. Did you mean: .*\?$", "", last)
        keep = ["Traceback (most recent call last):", last]
    return "\n".join(keep), notices


def files(cwd: Path, skip: set[str]) -> dict[str, bytes]:
    out = {}
    for p in sorted(cwd.rglob("*")):
        rel = p.relative_to(cwd).as_posix()
        if p.is_file() and rel not in skip:
            data = p.read_bytes()
            out[rel] = gzip.decompress(data) if rel.endswith(".gz") else data
    return out


def run_program(cmd: list[str], case: Case, cwd: Path) -> Result:
    for c in case.copy:
        shutil.copy(INPUTS / c, cwd / c)
    p = subprocess.run(cmd, cwd=cwd, env=env(case.columns), capture_output=True, text=True)
    err, notices = norm_err(p.stderr, cwd)
    return Result(p.returncode, norm_out(p.stdout, cwd), err, notices, files(cwd, set(case.copy)))


def run_api(case: Case, cwd: Path) -> Result:
    """nipype's interface through its Python API in ``cwd``: the inputs and outputs printed as
    ``run_instance`` prints them, the files."""
    import importlib
    import warnings

    module, iface, inputs = case.api  # type: ignore[misc]
    for c in case.copy:
        shutil.copy(INPUTS / c, cwd / c)
    with contextlib.chdir(cwd), warnings.catch_warnings():
        warnings.simplefilter("ignore")
        node = getattr(importlib.import_module(module), iface)()
        for k, v in inputs.items():
            setattr(node.inputs, k, v)
        text = f"{node.inputs}\n"
        res = node.run()
        text += f"{res.outputs}\n"
    return Result(0, norm_out(text, cwd), "", [], files(cwd, set(case.copy)))


def tsv_by_name(data: bytes) -> list[tuple[str, tuple[str, ...]]] | None:
    """A table's columns by name (PIPE-43: the order niworkflows gives some expanded columns is a
    Python set's, which changes between processes)."""
    try:
        rows = [ln.split("\t") for ln in data.decode().splitlines()]
    except UnicodeDecodeError:
        return None
    return sorted(zip(rows[0], zip(*rows[1:], strict=True), strict=True)) if rows else []


def compare(a: Result, b: Result, what: str, by_name: bool = False) -> list[str]:
    bad = []
    if a.code != b.code:
        bad.append(f"{what}: exit {a.code} vs {b.code}")
    if a.out != b.out:
        bad.append(f"{what}: stdout differs\n--- nipype\n{a.out}\n--- larmorx\n{b.out}")
    if a.err != b.err:
        bad.append(f"{what}: stderr differs\n--- nipype\n{a.err}\n--- larmorx\n{b.err}")
    if a.files.keys() != b.files.keys():
        bad.append(f"{what}: files {sorted(a.files)} vs {sorted(b.files)}")
        return bad
    for k in a.files:
        if a.files[k] == b.files[k]:
            continue
        if by_name and k.endswith(".tsv") and tsv_by_name(a.files[k]) == tsv_by_name(b.files[k]):
            BY_NAME.append(f"{what}: {k}")
            continue
        bad.append(f"{what}: file {k} differs")
    return bad


#: files equal by column name only (PIPE-43), as ``what: file``
BY_NAME: list[str] = []
CLI_DEFECTS = ("PIPE-81", "PIPE-82")


def _no_log(text: str) -> str:
    return "\n".join(
        ln for ln in text.splitlines() if "nipype.interface" not in ln and not ln.startswith("\t ")
    ).strip()


def _entries(notices: list[str]) -> set[str]:
    return {m.group(0) for n in notices for m in [re.search(r"[A-Z]+-\d+", n)] if m}


def check(case: Case, tmp: Path) -> tuple[bool, str]:
    d = Path(tempfile.mkdtemp(dir=tmp))
    for sub in "nrf":
        (d / sub).mkdir()
    nip = run_program([str(NIPYPECLI), "run", *case.args], case, d / "n")
    lx = [sys.executable, "-P", "-c", LARMORX]
    rep = run_program([*lx, "reproduce", *case.args], case, d / "r")
    fix = run_program([*lx, "fix", *case.args], case, d / "f")
    problems: list[str] = []
    if case.expect == "larmorx":
        ok = rep.code == 1 and fix.code == 1 and rep.err.startswith("error: ")
        return ok, f"nipype exit {nip.code}, larmorx exit {rep.code}: {rep.err.strip()}"
    problems += compare(nip, rep, "reproduce", case.by_name)
    entries = _entries(fix.notices)
    others = sorted(entries - set(CLI_DEFECTS))
    note = f"corrected by default: {', '.join(others)}" if others else ""
    if case.expect == "same":
        if entries & set(CLI_DEFECTS):
            problems.append(f"fix: unexpected notices {fix.notices}")
        if not others:
            problems += compare(nip, fix, "fix", case.by_name)
        elif fix.code != nip.code:
            problems.append(f"fix: exit {fix.code} vs {nip.code}")
    else:
        entry = case.expect.split(":", 1)[1]
        if entry not in entries:
            problems.append(f"fix: no {entry} notice ({fix.notices})")
        if nip.code == 0 and case.api is not None:
            problems.append("nipype's command line ran it")
        if case.api is not None:
            (d / "a").mkdir()
            api = run_api(case, d / "a")
            if fix.code != 0:
                problems.append(f"fix: exit {fix.code}: {fix.err}")
            ours = _no_log(fix.out.split("setting function inputs\n", 1)[-1])
            theirs = _no_log(api.out)
            if others:  # the inputs as printed; the results are the interfaces' corrections
                ours, theirs = ours.split("\n\n\n")[0], theirs.split("\n\n\n")[0]
            if ours != theirs:
                problems.append(f"fix vs API: printed\n--- API\n{api.out}\n--- larmorx\n{fix.out}")
            if not others:
                if api.files.keys() != fix.files.keys():
                    problems.append(f"fix vs API: files {sorted(api.files)} vs {sorted(fix.files)}")
                else:
                    problems += [
                        f"fix vs API: file {k} differs"
                        for k in api.files
                        if api.files[k] != fix.files[k]
                    ]
    return not problems, "\n".join(problems) or note


INPUTS = Path()


def main() -> int:
    global INPUTS
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--tier", choices=("quick", "full"), default="full")
    ap.add_argument("--cases", default="*", help="a glob on the cases' names")
    ap.add_argument("--keep", type=Path, help="a folder to keep the runs in")
    ap.add_argument("--jobs", type=int, default=8, help="cases run at once")
    a = ap.parse_args()
    with tempfile.TemporaryDirectory() as t:
        tmp = Path(a.keep or t).resolve()
        tmp.mkdir(parents=True, exist_ok=True)
        INPUTS = tmp / "inputs"
        INPUTS.mkdir(exist_ok=True)
        make_inputs(INPUTS)
        todo = [c for c in cases(a.tier, INPUTS) if fnmatch.fnmatch(c.name, a.cases)]
        failed = 0
        counts: dict[str, int] = {}
        corrected: dict[str, int] = {}
        with ThreadPoolExecutor(a.jobs) as pool:
            results = pool.map(lambda c: (c, *check(c, tmp)), todo)
            for c, ok, detail in results:
                kind = c.name.split("/", 1)[0]
                counts[kind] = counts.get(kind, 0) + 1
                if not ok:
                    failed += 1
                    print(f"FAIL {c.name}\n{detail}\n", flush=True)
                elif c.expect == "larmorx":
                    print(f"DOC  {c.name}: {detail}", flush=True)
                elif detail:
                    for e in detail.split(": ", 1)[1].split(", "):
                        corrected[e] = corrected.get(e, 0) + 1
        print("cases:", ", ".join(f"{k} {v}" for k, v in counts.items()))
        print(
            "fix mode, the interfaces' own defects corrected (reproduce identical):",
            ", ".join(f"{k} in {v}" for k, v in sorted(corrected.items())) or "none",
        )
        print(f"tables equal by column name only (PIPE-43): {len(BY_NAME)}")
        print(f"CHECK {'PASS' if not failed else 'FAIL'}: {len(todo) - failed} of {len(todo)} cases")
        return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
