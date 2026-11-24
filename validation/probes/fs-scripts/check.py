# SPDX-License-Identifier: Apache-2.0
"""seg2cc, mergeseg and fscalc against FreeSurfer 8.2.0's official scripts, case by case.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica's example and names its defect
handling).

Takes the cases of larmorx-freesurfer-volumes' example ``fs_scripts_probe``
(``crates-freesurfer/larmorx-freesurfer-volumes/tests/fs_scripts/cases.rs``: the scripts'
options, messages and failures, the names tcsh reads again, the environment's variables), writes
the example's fixture into a folder of its own for each case and side, runs the official script
(``$FREESURFER_HOME/bin`` of the reference installation first on ``PATH``, by name: ``/bin/tcsh``,
GNU coreutils and time, the C locale, ``TZ=UTC``; ``FREESURFER_HOME`` the fixture's stand-in)
through the oracle-output cache (never stored: the outputs hold the run's clock and process
number), runs the port (``larmorx-freesurfer --upstream-defects reproduce|fix freesurfer
<script>``) on the same case, and compares the canonical texts the example makes of each run
(status, both streams, every file the folder holds; what varies from run to run masked):
``reproduce`` must give FreeSurfer's on every case but those holding a form the port does not
reproduce (``UNMODELLED``: tcsh's reading of a name, ``--debug``'s trace, ``mri_volsynth``),
``fix`` may differ only with a notice. ``fs_time``'s lines (left out of the canonical text, which
every system shares) are compared apart: the same lines, their figures aside. Prints the hashes of
the recorded test (``tests/fs_scripts_official.rs``). ``--heavy`` adds the cases that conform a
volume to 256^3 (never in the tests).

    scripts/orchestration/boxed.sh 12 python -P validation/probes/fs-scripts/check.py [--heavy] [--json out.json]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "validation" / "src"))

from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.freesurfer_unit import FS_HOME, freesurfer_env  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402

MANIFEST = REPO / "crates-freesurfer" / "Cargo.toml"
RELEASE = REPO / "crates-freesurfer" / "target" / "release"
EXAMPLE = RELEASE / "examples" / "fs_scripts_probe"
PORT = RELEASE / "larmorx-freesurfer"
NEVER = "the scripts' outputs hold the run's clock, process number and temporary names"
TIMING = re.compile(rb"^(@#@FSTIME|@#@FSLOADPOST) +\S+ (\S+ N \d+).*$", re.M)
EXITED = re.compile(rb"^Command exited with non-zero status \d+$", re.M)


def status_of(rc: int | None) -> int:
    """An exit status as the shell reports it (a signal n as 128 + n; a time-out as 124)."""
    if rc is None:
        return 124
    return 128 - rc if rc < 0 else rc


def timing_lines(out: bytes) -> list[bytes]:
    """fs_time's report lines, their figures aside, and its exit lines."""
    rows = []
    for line in out.splitlines():
        m = TIMING.match(line)
        if m:
            rows.append(m.group(1) + b" " + m.group(2))
        elif EXITED.match(line):
            rows.append(line)
    return rows


def example(*args: str) -> str:
    # the official scripts' folder, masked as the port's $FREESURFER_HOME/bin
    env = dict(os.environ, FS_SCRIPTS_PROBE_BIN=str(workspace() / FS_HOME / "bin"))
    return subprocess.run(
        [str(EXAMPLE), *args], check=True, capture_output=True, text=True,
        errors="surrogateescape", env=env,
    ).stdout  # fmt: skip


def env_for(case: dict, d: Path, official: bool) -> dict[str, str]:
    fill = lambda s: s.replace("<D>", str(d))  # noqa: E731
    env = {
        "FREESURFER_HOME": fill("<D>/fsh"),
        "SUBJECTS_DIR": fill("<D>/subjects"),
        "HOME": str(d),
        "LC_ALL": "C",
        "LANG": "C",
        "TZ": "UTC",
        "USER": "probe",
        "FS_DISABLE_LANG": "1",
        "OMP_NUM_THREADS": "1",
        "PATH": "/usr/bin:/bin",
    }
    if official:
        ws = workspace()
        env["PATH"] = f"{ws / FS_HOME / 'bin'}:/usr/bin:/bin"
        env["FS_LICENSE"] = freesurfer_env()["FS_LICENSE"]
    for k, v in case["env"]:
        if v is None:
            env.pop(k, None)
        else:
            env[k] = fill(v)
    return env


def run_side(case: dict, root: Path, side: str, uname: str) -> tuple[str, bytes, list[str]]:
    """One side of a case in a fresh folder ``<root>/<side>/case``: its canonical text, standard
    output and the port's notices."""
    d = root / side / "case"
    if d.parent.exists():
        shutil.rmtree(d.parent)
    d.mkdir(parents=True)
    example("inputs", str(d))
    args = [a.replace("<D>", str(d)) for a in case["args"]]
    cwd = d / case["cwd"] if case["cwd"] else d
    env = env_for(case, d, side == "official")
    if side == "official":
        done = oracle_cache.run(
            [case["prog"], *args],
            env=env,
            cwd=cwd,
            policy=oracle_cache.Policy(threads=1, family="freesurfer", never=NEVER),
            timeout=600,
            runner="fs-scripts-probe",
        )
        rc, out, err = done.returncode, done.stdout_bytes, done.stderr_bytes
    else:
        p = subprocess.run(
            [str(PORT), "--upstream-defects", side, "freesurfer", case["prog"], *args],
            env=env,
            cwd=cwd,
            capture_output=True,
            timeout=600,
            check=False,
        )
        rc, out, err = p.returncode, p.stdout, p.stderr
    (root / side / "out").write_bytes(out)
    (root / side / "err").write_bytes(err)
    text = example(
        "outcome", str(d), str(status_of(rc)), str(root / side / "out"), str(root / side / "err"),
        uname,
    )  # fmt: skip
    ids = example("notices", str(root / side / "err")).split() if side != "official" else []
    shutil.rmtree(d.parent / "case")
    return text, out, ids


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--heavy", action="store_true", help="also the 256^3 cases")
    ap.add_argument("--only", help="the case numbers to run, comma-separated")
    ap.add_argument("--work", type=Path, default=None)
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--json", type=Path)
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    subprocess.run(
        ["cargo", "build", "--release", "-j", str(a.jobs), "--manifest-path", str(MANIFEST),
         "-p", "larmorx-freesurfer-cli", "-p", "larmorx-freesurfer-volumes",
         "--example", "fs_scripts_probe", "--bin", "larmorx-freesurfer"],
        check=True,
    )  # fmt: skip
    work = a.work or ws / "oracles" / "inputs" / "fs-scripts"
    work.mkdir(parents=True, exist_ok=True)
    uname = subprocess.run(["uname", "-a"], capture_output=True, text=True).stdout.rstrip("\n")
    cases = [json.loads(line) for line in example("cases").splitlines()]
    only = {int(x) for x in a.only.split(",")} if a.only else None
    verdicts, rows = Counter(), []
    official_texts, fix_texts = [], []
    quick_texts, quick_fix = [], []
    golden = [int(x) for x in example("golden").split()]
    golden_texts = {}
    for case in cases:
        k = case["k"]
        if (only is not None and k not in only) or (case["heavy"] and not a.heavy):
            continue
        root = work / f"c{k:03d}"
        o_text, o_out, _ = run_side(case, root, "official", uname)
        r_text, r_out, r_ids = run_side(case, root, "reproduce", uname)
        f_text, _, f_ids = run_side(case, root, "fix", uname)
        unmodelled = "UNMODELLED" in r_ids
        row = {"k": k, "prog": case["prog"], "args": case["args"], "env": case["env"]}
        if unmodelled:
            verdict = "unmodelled"
        elif r_text != o_text:
            verdict = "different"
        elif timing_lines(r_out) != timing_lines(o_out):
            verdict = "different-timing"
        else:
            verdict = "identical"
        if f_text == o_text:
            fv = "fix-identical"
        elif "UNMODELLED" in f_ids:
            fv = "fix-unmodelled"
        else:
            fv = "fix-explained" if [i for i in f_ids if i != "UNMODELLED"] else "fix-unexplained"
        verdicts[verdict] += 1
        verdicts[fv] += 1
        row |= {"verdict": verdict, "fix": fv, "reproduce_ids": r_ids, "fix_ids": f_ids}
        if verdict.startswith("different") or fv == "fix-unexplained":
            row["official"] = o_text
            row["reproduce"] = r_text
            row["fixed"] = f_text
            print(f"--- case {k} {case['prog']} {case['args']}: {verdict} {fv}", flush=True)
            if verdict.startswith("different"):
                import difflib

                for line in list(
                    difflib.unified_diff(o_text.splitlines(), r_text.splitlines(), lineterm="")
                )[:30]:
                    print("   ", line)
        if k in golden:
            golden_texts[k] = o_text
            row["golden"] = True
        if not case["heavy"]:
            official_texts.append("UNMODELLED" if unmodelled else o_text)
            fix_texts.append(f_text)
            if case["quick"]:
                quick_texts.append("UNMODELLED" if unmodelled else o_text)
                quick_fix.append(f_text)
        rows.append(row)
        shutil.rmtree(root)

    def digest(texts: list[str]) -> str:
        h = hashlib.sha256()
        for t in texts:
            h.update(t.encode("utf-8", "surrogateescape") + b"\n")
        return h.hexdigest()

    hg = hashlib.sha256()
    for k in golden:
        hg.update(golden_texts.get(k, "MISSING").encode("utf-8", "surrogateescape") + b"\n")
    summary = {
        "golden_sha256": hg.hexdigest(),
        "golden_cases": len(golden),
        "cases": len(rows),
        "verdicts": dict(verdicts),
        "official_sha256": digest(official_texts),
        "official_cases": len(official_texts),
        "fix_sha256": digest(fix_texts),
        "unmodelled": sum(1 for t in official_texts if t == "UNMODELLED"),
        "official_sha256_quick": digest(quick_texts),
        "quick_cases": len(quick_texts),
        "fix_sha256_quick": digest(quick_fix),
        "unmodelled_quick": sum(1 for t in quick_texts if t == "UNMODELLED"),
    }
    print(json.dumps(summary, indent=1))
    if a.json:
        a.json.write_text(json.dumps(summary | {"rows": rows}, indent=1) + "\n", encoding="utf-8")
    bad = sum(v for k, v in verdicts.items() if k.startswith("different")) + verdicts.get(
        "fix-unexplained", 0
    )
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
