# SPDX-License-Identifier: Apache-2.0
"""mri_binarize's real calls on recorded heads: FreeSurfer 8.2.0's official program against the port.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica).

``mri_binarize`` is no step of its own in the reference runs: recon-all, ``mri_nu_correct.mni``,
``mri_mcadura_seg``, ``mri_vsinus_seg``, ``label-cortex`` and ``seg2cc`` run it on their way
(their recorded steps check it once those scripts are ported). This runs those calls on the
recorded heads' files (``oracles/freesurfer-8.2.0/runs/<run>/subjects/<subject>/mri``; three heads,
both streams, the v8 stream's own files where the call needs them), the official program by name
through the oracle-output cache at one thread and the port (``larmorx-freesurfer --upstream-defects
reproduce|fix freesurfer mri_binarize``, built here), each in a folder of its own, and compares
the exit status, the messages (the report's ``cwd`` line aside) and every file written
(FreeSurfer's provenance aside). An in-place call (recon-all's ``--i <f> --o <f>``) runs on a copy.
A call whose ``reproduce`` run notes a defect of the open probe's ``NOT_COMPARABLE`` is counted,
not compared (the topology fix of an already correct hippocampus: FS-1021).

    python -P validation/probes/fs-binarize/real.py [--runs a,b] [--json out.json]
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
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
from larmorx_validation.freesurfer_unit import freesurfer_env  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402


def _load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


compare = _load(
    "freesurfer_mri_binarize_compare",
    REPO / "validation" / "investigations" / "freesurfer-mri-binarize-probe" / "compare.py",
)

HEADS = ("ds000005-sub-01", "ds000228-sub-pixar008", "ds004173-sub-603309")

#: The calls (``{m}/x``: the head's ``mri/x``; ``@x``: a copy of it in the case's folder), with
#: the stream they need (``v8``: its own files) and the script that makes them.
CALLS: list[tuple[str, str, list[str]]] = [
    ("recon-all wm mask", "", ["--i", "{m}/aseg.presurf.mgz", "--match", "2", "41", "--o",
                               "wm.mask-from-aseg.presurf.mgz"]),
    ("recon-all wm255", "", ["--i", "{m}/wm.mgz", "--min", "255", "--max", "255", "--o",
                             "wm255.mgz", "--count", "wm255.txt"]),
    ("recon-all wm1", "", ["--i", "{m}/wm.mgz", "--min", "1", "--max", "1", "--o", "wm1.mgz",
                           "--count", "wm1.txt"]),
    ("recon-all bgc", "", ["--i", "@brainmask.mgz", "--o", "@brainmask.mgz", "--binval", "999",
                           "--min", "1"]),
    ("nu_correct ones", "", ["--i", "{m}/nu.mgz", "--min", "-1", "--o", "ones.mgz"]),
    ("nu_correct mask", "", ["--i", "{m}/brainmask.mgz", "--min", "1", "--o", "mask.mgz"]),
    ("mcadura ctx", "", ["--match", "3", "42", "--i", "{m}/aseg.mgz", "--o", "ctx.mgz"]),
    ("vsinus ctxmask", "", ["--i", "{m}/aseg.mgz", "--match", "3", "42", "--inv", "--o",
                            "ctxsegmask.mgz"]),
    ("vsinus crop", "", ["--crop-around-ras", "crop.mgz", "{m}/nu.mgz", "nolta", "cras",
                         "{m}/brainmask.mgz", "0", "96", "96", "96"]),
    ("seg2cc ccbin", "", ["--i", "{m}/aseg.mgz", "--match", "251", "--o", "ccbin.mgh"]),
    ("label-cortex lh ga", "v8", ["--match", "3201", "--i", "{m}/entowm.mgz", "--o", "ga.mgz"]),
    ("label-cortex rh ga", "v8", ["--match", "4201", "--i", "{m}/entowm.mgz", "--o", "ga.mgz"]),
    ("synthseg wm", "v8", ["--i", "{m}/synthseg.rca.mgz", "--all-wm", "--o", "wm.mgz"]),
    ("hippocampus surface", "", ["--i", "{m}/aseg.mgz", "--match", "17", "--surf", "lh.hippo",
                                 "--surf-smooth", "2"]),
    ("hippocampus topology", "", ["--i", "{m}/aseg.mgz", "--match", "17", "--fix-vol-topo",
                                  "--o", "hippo.mgz", "--surf", "lh.hippo"]),
]  # fmt: skip


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--runs", help="comma-separated run names (default: 3 heads x 2 streams)")
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--json", type=Path)
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    env_b = {k: v for k, v in os.environ.items() if k != "FSLDIR"}
    subprocess.run(
        ["cargo", "build", "--release", "-p", "larmorx-freesurfer-cli", "-j", str(a.jobs),
         "--manifest-path", str(REPO / "crates-freesurfer" / "Cargo.toml")],
        check=True,
        env=env_b,
    )  # fmt: skip
    port = REPO / "crates-freesurfer" / "target" / "release" / "larmorx-freesurfer"
    exe = compare.official()
    runs_root = ws / "oracles" / "freesurfer-8.2.0" / "runs"
    runs = a.runs.split(",") if a.runs else [f"{h}-{s}" for h in HEADS for s in ("classic", "v8")]
    work = ws / "oracles" / "inputs" / "fs-binarize-real"
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
           "OMP_NUM_THREADS": "1", "HOME": str(work), "USER": "probe"}  # fmt: skip
    env.update(freesurfer_env())
    ctx = fsfiles.Context(any_directory=True)
    verdicts: Counter = Counter()
    rows = []
    times = Counter()
    for run in runs:
        subj = next(p for p in (runs_root / run / "subjects").iterdir() if p.name != "fsaverage")
        m = subj / "mri"
        for name, stream, call in CALLS:
            if stream and not run.endswith(stream):
                continue
            sides = {}
            for side in ("official", "reproduce", "fix"):
                d = work / run / name.replace(" ", "-") / side
                if d.exists():
                    shutil.rmtree(d)
                d.mkdir(parents=True)
                args = []
                for x in call:
                    if x.startswith("@"):
                        if not (d / x[1:]).exists():
                            shutil.copyfile(m / x[1:], d / x[1:])
                        x = x[1:]
                    args.append(x.replace("{m}", str(m)))
                t0 = time.monotonic()
                if side == "official":
                    done = oracle_cache.run(
                        ["mri_binarize", *args],
                        env=env,
                        cwd=d,
                        outputs=[d],
                        policy=compare.POLICY,
                        timeout=3600,
                        runner="fs-binarize-real",
                    )
                    sides[side] = (done.returncode, done.stdout, done.stderr, d)
                    if not done.cached:
                        times[side] += time.monotonic() - t0
                else:
                    cmd = [str(port), "--upstream-defects", side, "freesurfer", "mri_binarize"]
                    p = subprocess.run(
                        [*cmd, *args], env=env, cwd=d, capture_output=True, text=True, timeout=3600
                    )
                    sides[side] = (p.returncode, p.stdout, p.stderr, d)
                    times[side] += time.monotonic() - t0
            oc, oo, oe, od = sides["official"]
            row = {"run": run, "call": name, "official_exit": oc}
            for side in ("reproduce", "fix"):
                pc, po, pe, pd = sides[side]
                diffs = []
                if pc != oc:
                    diffs.append(f"exit {pc} != {oc}")
                if compare.mask(po, True) != compare.mask(oo, False):
                    diffs.append("stdout")
                if compare.mask(pe, True) != compare.mask(oe, False):
                    diffs.append("stderr")
                fo, fp = compare.written(od), compare.written(pd)
                if sorted(fo) != sorted(fp):
                    diffs.append(f"files {sorted(fo)} != {sorted(fp)}")
                for f in sorted(set(fo) & set(fp)):
                    c = fsfiles.compare(fo[f], fp[f], ctx, name=f)
                    if not c.same:
                        diffs.append(f"{f}: {c.verdict} {c.detail[:200]}")
                ids = compare.notices(pe)
                row[side] = {"exit": pc, "differ": diffs, "notices": ids}
                if any(x in compare.NOT_COMPARABLE for x in compare.notices(sides["reproduce"][2])):
                    row[side]["skip"] = True
                    verdicts[f"{side}-skip"] += 1
                elif side == "reproduce":
                    verdicts["identical" if not diffs else "different"] += 1
                else:
                    verdicts[
                        "fix-identical"
                        if not diffs
                        else ("fix-explained" if ids else "fix-unexplained")
                    ] += 1
            rows.append(row)
            print(f"{run} {name}: exit {oc}, reproduce "
                  f"{'ok' if not row['reproduce']['differ'] else row['reproduce']['differ']}, fix "
                  f"{'ok' if not row['fix']['differ'] else row['fix']['notices']}",
                  flush=True)  # fmt: skip
    summary = {
        "runs": runs,
        "calls": len(rows),
        "verdicts": dict(verdicts),
        "seconds": {k: round(v, 1) for k, v in times.items()},
        "rows": rows,
    }
    print(json.dumps({k: v for k, v in summary.items() if k != "rows"}, indent=1))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    return 1 if verdicts.get("different", 0) + verdicts.get("fix-unexplained", 0) else 0


if __name__ == "__main__":
    raise SystemExit(main())
