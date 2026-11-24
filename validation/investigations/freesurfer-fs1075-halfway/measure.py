# SPDX-License-Identifier: Apache-2.0
"""FS-1075 measured: the official mri_robust_register's half-way outputs after its Powell registration.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica's example and names the replica's
defect handling).

The Powell registration (every ``--cost`` but the robust ones) takes its half-way geometry from an
image it has freed by then (FS-1075). This runs the probe's half-way family
(``fs_robreg_probe halfway-cases``: 24 cases, every Powell cost, random half-way outputs) with the
official program ``--runs`` times each, never served from the oracle-output cache (the result may
hang on memory contents), every run in a folder of its own, and reports per case: the exit status
of each run, whether the runs agree (standard output and error with the time masked, every file
written), and whether the half-way transforms' target geometry (``dst volume info`` of
``--halfmovlta``) is the destination's (``dst volume info`` of ``--lta``) or another.

    scripts/orchestration/boxed.sh 16 python -P validation/investigations/freesurfer-fs1075-halfway/measure.py \\
        --runs 3 --json <out.json>

Relay 49 (FS-1079): ``--extra "--debug --halfmov hm.mgz"`` appends those arguments to every case
(its runs in folders of their own, ``fs1075-halfway-extra``); the official program's standard input
is ``/dev/null`` (the debug output's key presses fail at once).
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
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

_spec = importlib.util.spec_from_file_location(
    "fs_robreg_check", REPO / "validation" / "probes" / "fs-robust-register" / "check.py"
)
check = importlib.util.module_from_spec(_spec)
assert _spec.loader is not None
_spec.loader.exec_module(check)

NEVER = "FS-1075: the half-way geometry is read from a freed image (memory contents)"


def run_official(exe: Path, line: str, d: Path, preload: str, extra: list[str]) -> dict:
    t = line.split()
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
           "OMP_NUM_THREADS": "1", "FS_DISABLE_LANG": "1", "USER": "larmorx", "TZ": "UTC0",
           "FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING": "Thu Jan  1 00:00:00 1970",
           **freesurfer_env()}  # fmt: skip
    if preload:
        env["LD_PRELOAD"] = preload
    if any(x.startswith("DIAG_NO") or x.lower() == "diag" for x in env):
        raise SystemExit("DIAG_NO would print the licence file")
    if d.exists():
        shutil.rmtree(d)
    d.mkdir(parents=True)
    n = int(t[2])
    i = 3
    inputs = []
    for _ in range(n):
        name, h = t[i], t[i + 1]
        p = d / name
        p.write_bytes(bytes.fromhex(h))
        inputs.append(p)
        i += 2
    ndirs = int(t[i])
    i += 1 + ndirs
    nargs = int(t[i])
    args = [bytes.fromhex(h).decode() for h in t[i + 1 : i + 1 + nargs]] + extra
    done = oracle_cache.run(
        [exe.name, *args],
        env=env,
        cwd=d,
        outputs=[d],
        policy=oracle_cache.Policy(
            threads=1, family="freesurfer", formats="freesurfer", volatile=(check.TIME.pattern,)
        ).but_never(NEVER),
        timeout=300,
        runner="fs1075-halfway-measure",
        inputs=inputs,
    )
    rc = done.returncode
    status = f"EXIT {rc & 0xFF}" if rc >= 0 else f"CRASH {-rc}"
    dd = str(d)

    def mask(s: str) -> str:
        return check.TIME.sub("Registration took <T>", s.replace(dd, "<DIR>"))

    files = {}
    for p in sorted(d.rglob("*")):
        if p.is_file() and p not in inputs:
            files[p.relative_to(d).as_posix()] = p.read_bytes()
    return {
        "args": args,
        "status": status,
        "stdout": mask(done.stdout),
        "stderr": mask(done.stderr),
        "files": files,
    }


def dst_info(lta: bytes | None) -> str | None:
    """The ``dst volume info`` block of an LTA's text (``None``: no file)."""
    if lta is None:
        return None
    text = lta.decode("latin-1")
    k = text.find("dst volume info")
    if k < 0:
        return "<none>"
    end = text.find("subject", k)
    return text[k : end if end > 0 else None]


def strip_volatile_lta(b: bytes) -> bytes:
    """An LTA without its first comment lines (the file name, the creation line)."""
    return b"\n".join(x for x in b.split(b"\n") if not x.startswith(b"# "))


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--json", type=Path)
    ap.add_argument("--preload", type=Path, help="the official program's LD_PRELOAD library")
    ap.add_argument("--cases", type=str, default="", help="case numbers (comma-separated)")
    ap.add_argument("--extra", type=str, default="", help="arguments added to every case")
    a = ap.parse_args(argv)
    devnull = os.open(os.devnull, os.O_RDONLY)
    os.dup2(devnull, 0)
    os.close(devnull)
    extra = a.extra.split()
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    exe = ws / FS_HOME / "bin" / "mri_robust_register"
    text = subprocess.run(
        [check.EXAMPLE, "halfway-cases"], check=True, capture_output=True, text=True
    ).stdout
    cases = text.splitlines()
    pick = [int(x) for x in a.cases.split(",") if x] or list(range(len(cases)))
    root = ws / "oracles" / "inputs" / ("fs1075-halfway-extra" if extra else "fs1075-halfway")
    preload = str(a.preload.resolve()) if a.preload else ""
    out = []
    counts: Counter[str] = Counter()
    for k in pick:
        runs = [
            run_official(exe, cases[k], root / f"run-{r}" / f"case-{k}", preload, extra)
            for r in range(a.runs)
        ]
        statuses = [x["status"] for x in runs]
        same_text = all(
            x["stdout"] == runs[0]["stdout"] and x["stderr"] == runs[0]["stderr"] for x in runs
        )
        names = sorted(set().union(*(x["files"] for x in runs)))
        same_files = {}
        for n in names:
            vals = [x["files"].get(n) for x in runs]
            if n.endswith(".lta"):
                vals = [None if v is None else strip_volatile_lta(v) for v in vals]
            same_files[n] = all(v == vals[0] for v in vals)
        hm = [dst_info(x["files"].get("hm.lta")) for x in runs]
        dst = [dst_info(x["files"].get("out.lta")) for x in runs]
        geometry = [
            "none" if h is None else "dst" if h == o else "other"
            for h, o in zip(hm, dst, strict=True)
        ]
        row = {
            "case": k,
            "args": " ".join(runs[0]["args"]),
            "statuses": statuses,
            "same_text": same_text,
            "same_files": same_files,
            "halfway_geometry": geometry,
            "hm_dst_info": hm[0],
            "out_dst_info": dst[0],
            "stdout_tail": runs[0]["stdout"][-600:],
            "stderr_tail": runs[0]["stderr"][-600:],
        }
        out.append(row)
        verdict = (
            f"{'/'.join(sorted(set(statuses)))} "
            f"{'same' if same_text and all(same_files.values()) else 'VARY'} "
            f"{'/'.join(sorted(set(geometry)))}"
        )
        counts[verdict] += 1
        print(f"case {k}: {verdict} | {row['args'][-160:]}", flush=True)
    for v, n in sorted(counts.items()):
        print(f"{n:3d}  {v}")
    if a.json:
        a.json.write_text(json.dumps(out, indent=1), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
