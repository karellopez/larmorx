# SPDX-License-Identifier: Apache-2.0
"""mri_cc's white-matter path (``-a 0``) on recorded heads: FreeSurfer 8.2.0's official program against the port.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica).

recon-all runs ``mri_cc`` only through ``seg2cc`` (the aseg path, replayed by
``validation/investigations/freesurfer-mri-cc-replay/``). The white-matter path reads
``<subject>/mri/wm`` without a suffix and ``mri/transforms/talairach.xfm``; recon-all writes
``wm.mgz``, so here each recorded head's ``wm.mgz`` is written as a COR folder ``wm`` by the
official ``mri_convert -ot cor`` (``oracles/inputs/fs-cc-real/<run>/wm``, through the
oracle-output cache) and the run's ``talairach.xfm`` copied beside it. The official program by name
through the cache at one thread and the port (``larmorx-freesurfer --upstream-defects
reproduce|fix freesurfer mri_cc``, built here), each with a subjects folder of its own; compared:
exit status, messages and every file written (``cc_org.mgz``; FreeSurfer's provenance aside).

    python -P validation/probes/fs-cc/real.py [--runs a,b] [--json out.json]
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import time
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "validation" / "src"))

from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.freesurfer import files as fsfiles  # noqa: E402
from larmorx_validation.freesurfer_unit import FS_HOME, freesurfer_env  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402

HEADS = (
    "ds000005-sub-01",
    "ds000228-sub-pixar008",
    "ds002422-sub-02",
    "ds004173-sub-603309",
    "ds005003-sub-RS001",
    "ds005454-sub-16",
)
RUNS = tuple(f"{h}-{s}" for h in HEADS for s in ("classic", "v8"))
#: The calls: the path's defaults, and other divisions and thickness (only the messages change).
CALLS = {
    "wm": ["-a", "0"],
    "wm-d7-t3": ["-a", "0", "-d", "7", "-t", "3"],
}
POLICY = oracle_cache.Policy(threads=1, family="freesurfer", formats="freesurfer")
TIMING = re.compile(r"^(corpus callosum segmentation took) .* minutes$", re.M)
PEAK = re.compile(r"^#VMPC# mri_cc VmPeak .*$", re.M)
MANIFEST = REPO / "crates-freesurfer" / "Cargo.toml"


def status(code: int) -> int:
    """An exit status as a shell reports it: a process ended by signal ``n`` as ``128 + n``."""
    return 128 - code if code < 0 else code


def mask(text: str, port: bool, folder: Path) -> str:
    """The messages without the time, the peak memory, the case's folder and the port's notices."""
    text = TIMING.sub(r"\1 <time> minutes", text)
    text = PEAK.sub("#VMPC# mri_cc VmPeak <peak>", text)
    text = text.replace(str(folder), "<folder>")
    if port:
        text = "".join(x for x in text.splitlines(True) if not x.startswith("larmorx: "))
    return text


def notices(text: str) -> list[str]:
    return sorted(
        {m.group(1) for m in re.finditer(r"^larmorx: upstream defect (\S+) ", text, re.M)}
    )


def written(folder: Path, before: set[str]) -> dict[str, Path]:
    return {
        str(p.relative_to(folder)): p
        for p in sorted(folder.rglob("*"))
        if p.is_file() and str(p.relative_to(folder)) not in before
    }


def subject_mri(ws: Path, run: str) -> Path:
    subjects = ws / "oracles" / "freesurfer-8.2.0" / "runs" / run / "subjects"
    subject = next(p for p in sorted(subjects.iterdir()) if p.name != "fsaverage")
    return subject / "mri"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--runs", default=",".join(RUNS))
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--json", type=Path)
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    env_build = {k: v for k, v in os.environ.items() if k != "FSLDIR"}
    subprocess.run(
        ["cargo", "build", "--release", "-p", "larmorx-freesurfer-cli", "-j", str(a.jobs),
         "--manifest-path", str(MANIFEST)],
        check=True,
        env=env_build,
    )  # fmt: skip
    port = REPO / "crates-freesurfer" / "target" / "release" / "larmorx-freesurfer"
    exe = ws / FS_HOME / "bin" / "mri_cc"
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
           "OMP_NUM_THREADS": "1", "USER": "probe", "FS_DISABLE_LANG": "1"}  # fmt: skip
    env.update(freesurfer_env())
    ctx = fsfiles.Context(any_directory=True)
    rows, verdicts = [], Counter()
    seconds = Counter()
    for run in a.runs.split(","):
        mri = subject_mri(ws, run)
        base = ws / "oracles" / "inputs" / "fs-cc-real" / run
        cor = base / "wm"
        if cor.exists():
            shutil.rmtree(cor)
        cor.mkdir(parents=True)
        made = oracle_cache.run(
            ["mri_convert", str(mri / "wm.mgz"), "wm", "-ot", "cor"],
            env=env,
            cwd=base,
            outputs=[cor],
            policy=POLICY,
            timeout=900,
            runner="fs-cc-real",
        )
        if made.returncode != 0:
            raise SystemExit(f"{run}: mri_convert -ot cor failed: {made.stderr[-500:]}")
        for kind, opts in CALLS.items():
            sides = {}
            for side in ("official", "reproduce", "fix"):
                d = base / kind / side
                if d.exists():
                    shutil.rmtree(d)
                smri = d / "s" / "mri"
                shutil.copytree(cor, smri / "wm")
                (smri / "transforms").mkdir()
                shutil.copy(mri / "transforms" / "talairach.xfm", smri / "transforms")
                given = sorted(p for p in d.rglob("*") if p.is_file())
                before = {str(p.relative_to(d)) for p in given}
                args = ["-sdir", ".", *opts, "s"]
                t0 = time.monotonic()
                if side == "official":
                    p = oracle_cache.run(
                        ["mri_cc", *args],
                        env=env,
                        cwd=d,
                        outputs=[d],
                        policy=POLICY,
                        timeout=900,
                        runner="fs-cc-real",
                        inputs=given,
                    )
                else:
                    p = subprocess.run(
                        [str(port), "--upstream-defects", side, "freesurfer", "mri_cc", *args],
                        env=env,
                        cwd=d,
                        capture_output=True,
                        text=True,
                        errors="surrogateescape",
                        timeout=900,
                    )
                seconds[side] += time.monotonic() - t0
                sides[side] = (status(p.returncode), p.stdout, p.stderr, d, before)
            oc, oo, oe, od, ob = sides["official"]
            row = {"run": run, "call": kind, "official_exit": oc}
            for side in ("reproduce", "fix"):
                pc, po, pe, pd, pb = sides[side]
                diffs = []
                if pc != oc:
                    diffs.append(f"exit {pc} != {oc}")
                if mask(po, True, pd) != mask(oo, False, od):
                    diffs.append("stdout")
                if mask(pe, True, pd) != mask(oe, False, od):
                    diffs.append("stderr")
                fo, fp = written(od, ob), written(pd, pb)
                if sorted(fo) != sorted(fp):
                    diffs.append(f"files {sorted(fo)} != {sorted(fp)}")
                for name in sorted(set(fo) & set(fp)):
                    c = fsfiles.compare(fo[name], fp[name], ctx, name=Path(name).name)
                    if not c.same:
                        diffs.append(f"{name}: {c.verdict} {c.detail[:200]}")
                ids = notices(pe)
                row[side] = {"exit": pc, "differ": diffs, "notices": ids}
                if side == "reproduce":
                    verdicts["identical" if not diffs else "different"] += 1
                else:
                    verdicts["fix-identical" if not diffs else
                             ("fix-explained" if ids else "fix-unexplained")] += 1  # fmt: skip
            rows.append(row)
            print(f"{run} {kind}: exit {oc}, reproduce "
                  f"{'ok' if not row['reproduce']['differ'] else row['reproduce']['differ']}, fix "
                  f"{'ok' if not row['fix']['differ'] else (row['fix']['differ'], row['fix']['notices'])}",
                  flush=True)  # fmt: skip
    summary = {
        "runs": a.runs.split(","),
        "calls": len(rows),
        "verdicts": dict(verdicts),
        "seconds": {k: round(v, 1) for k, v in seconds.items()},
        "rows": rows,
    }
    print(json.dumps({k: v for k, v in summary.items() if k != "rows"}, indent=1))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    return 1 if verdicts.get("different", 0) + verdicts.get("fix-unexplained", 0) else 0


if __name__ == "__main__":
    raise SystemExit(main())
