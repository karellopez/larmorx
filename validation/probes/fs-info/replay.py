# SPDX-License-Identifier: Apache-2.0
"""mri_info's recorded calls on the six heads, both streams: the port against the recording and the
official program.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica).

``mri_info`` is no step of its own in the recordings (it writes no file): recon-all runs it on
``mri/rawavg.mgz`` and on ``mri/orig.mgz`` (its output kept in ``scripts/recon-all.log``, also read
for the field of view), with ``--nframes`` on each input (``mri/orig/001.mgz``), and ``seg2cc``
runs ``mri_info --o <tmp> --conformed-to-min mri/aseg.auto_noCCseg.mgz`` (its answer kept as
``IsConformed <answer>`` in ``scripts/seg2cc.log``). For each recorded run
(``oracles/freesurfer-8.2.0/runs/*``) the port's program (``--port``, run as recon-all runs it,
with each mode) gives each call's output, which is compared with the logged text (the two logged
calls: the log holds the port's whole output where recon-all printed it; seg2cc's: its answer) and
with the official program's output now (through the oracle-output cache, one thread): exit status,
standard output and error, and ``--o``'s file. The files read are the run's final ones (none of
them changes after the call).

    python -P validation/probes/fs-info/replay.py --port <larmorx-freesurfer> [--json out.json]
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "validation" / "src"))

from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.freesurfer_unit import FS_HOME, freesurfer_env  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402


def env_of(exe: Path) -> dict[str, str]:
    return {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
            "OMP_NUM_THREADS": "1", **freesurfer_env()}  # fmt: skip


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--port", type=Path, required=True)
    ap.add_argument("--json", type=Path)
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    exe = ws / FS_HOME / "bin" / "mri_info"
    runs = sorted(p for p in (ws / "oracles" / "freesurfer-8.2.0" / "runs").iterdir() if p.is_dir())
    rows = []
    for run in runs:
        subj = next(p for p in (run / "subjects").iterdir() if p.name != "fsaverage")
        mri = subj / "mri"
        log = (subj / "scripts" / "recon-all.log").read_text(encoding="utf-8", errors="replace")
        s2c = subj / "scripts" / "seg2cc.log"
        calls = [
            ("rawavg", [str(mri / "rawavg.mgz")], "rawavg.mgz"),
            ("orig", [str(mri / "orig.mgz")], "orig.mgz"),
            ("nframes", ["--nframes", str(mri / "orig" / "001.mgz")], None),
        ]
        if (mri / "aseg.auto_noCCseg.mgz").exists():
            calls.append(("seg2cc", ["--o", "@TMP", "--conformed-to-min",
                                     str(mri / "aseg.auto_noCCseg.mgz")], None))  # fmt: skip
        for name, args, logged in calls:
            row = {"run": run.name, "call": name}
            with tempfile.TemporaryDirectory() as td:
                outs = {}
                for side in ("official", "reproduce", "fix"):
                    d = Path(td) / side
                    d.mkdir()
                    arg = [str(d / "isconformed") if x == "@TMP" else x for x in args]
                    if side == "official":
                        done = oracle_cache.run(
                            [exe.name, *arg], env=env_of(exe), cwd=mri, outputs=[d],
                            policy=oracle_cache.Policy(threads=1, family="freesurfer",
                                                       formats="freesurfer"),
                            timeout=300, runner="fs-info-replay",
                        )  # fmt: skip
                        rc, so, se = done.returncode, done.stdout, done.stderr
                    else:
                        p = subprocess.run(
                            [a.port, "--upstream-defects", side, "freesurfer", "mri_info", *arg],
                            env=env_of(exe), cwd=mri, capture_output=True, text=True,
                        )  # fmt: skip
                        rc, so = p.returncode, p.stdout
                        se = "".join(
                            x for x in p.stderr.splitlines(True) if not x.startswith("larmorx: ")
                        )
                    f = d / "isconformed"
                    text = f.read_text() if f.exists() else None
                    outs[side] = (rc, so, se.replace(str(d), "<TMP>"), text)
            row["reproduce_identical"] = outs["reproduce"] == outs["official"]
            row["fix_identical"] = outs["fix"] == outs["official"]
            if logged:
                marker = f"{logged} ========================================\n"
                at = log.find(marker)
                body = outs["reproduce"][1]
                row["log_identical"] = at >= 0 and log[at + len(marker) :].startswith(body)
            if name == "seg2cc" and s2c.exists():
                ans = (outs["reproduce"][3] or "").splitlines()[:1]
                row["log_identical"] = f"IsConformed {ans[0] if ans else ''}" in s2c.read_text()
            rows.append(row)
            print(json.dumps(row), flush=True)
    bad = [r for r in rows if not all(v for k, v in r.items() if k.endswith("identical"))]
    summary = {"calls": len(rows), "runs": len(runs), "not_identical": bad}
    print(json.dumps({k: v for k, v in summary.items()}, indent=1))
    if a.json:
        a.json.write_text(json.dumps({"rows": rows, **summary}, indent=1) + "\n", encoding="utf-8")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
