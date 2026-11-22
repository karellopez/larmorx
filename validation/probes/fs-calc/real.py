# SPDX-License-Identifier: Apache-2.0
"""mris_calc's real calls on recorded heads: FreeSurfer 8.2.0's official program against the port.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica).

``mris_calc`` is no step of its own in the reference runs: ``mri_nu_correct.mni`` (the rescale of
its N3 output by the ratio of two means, ``-o nu0.mgz nu0.mgz mul <ratio>``), recon-all's vertex
volumes (``-o lh.area.mid lh.area add lh.area.pial``, then ``-o lh.area.mid lh.area.mid div 2``)
and ``label-cortex`` (``-o tmp.mgh tmp.mgh and lh.nz.mask.mgz``) run it on their way (their
recorded steps check it once those scripts are ported). This runs those calls on the recorded
heads' files (``oracles/freesurfer-8.2.0/runs/<run>/subjects/<subject>``; three heads, both
streams): the rescale with each run's own recorded ratio on a copy of its ``orig.mgz`` (N3's
output is not kept), the vertex volumes of both hemispheres (their result also against the run's
own ``surf/?h.area.mid``), the mask product on a copy of a surface overlay of the run
(``surf/?h.w-g.pct.mgh``). The official program by name through the oracle-output cache at one
thread and the port (``larmorx-freesurfer --upstream-defects reproduce|fix freesurfer
mris_calc``, built here), each in a folder of its own; compared: exit status, messages and every
file written (FreeSurfer's provenance aside).

    python -P validation/probes/fs-calc/real.py [--runs a,b] [--json out.json]
"""

from __future__ import annotations

import argparse
import importlib.util
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
from larmorx_validation.freesurfer_unit import freesurfer_env  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402


def _load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


compare = _load(
    "freesurfer_mris_calc_compare",
    REPO / "validation" / "investigations" / "freesurfer-mris-calc-probe" / "compare.py",
)

HEADS = ("ds000005-sub-01", "ds000228-sub-pixar008", "ds004173-sub-603309")
#: mri_nu_correct.mni's rescale in the run's log.
RATIO = re.compile(r"^mris_calc -o \S*nu0\.mgz \S*nu0\.mgz mul (\S+)$", re.M)

#: The calls: a name, the commands run one after the other in one folder (``{s}/x``: the
#: subject's ``x``; ``@a:b``: a copy of the subject's ``a`` named ``b`` in the folder;
#: ``{ratio}``: the run's recorded ratio), and the recorded file the last output must equal.
CALLS: list[tuple[str, list[list[str]], tuple[str, str] | None]] = [
    ("nu rescale", [["-o", "nu0.mgz", "@mri/orig.mgz:nu0.mgz", "mul", "{ratio}"]], None),
    ("vertexvol lh", [["-o", "lh.area.mid", "{s}/surf/lh.area", "add", "{s}/surf/lh.area.pial"],
                      ["-o", "lh.area.mid", "lh.area.mid", "div", "2"]],
     ("lh.area.mid", "surf/lh.area.mid")),
    ("vertexvol rh", [["-o", "rh.area.mid", "{s}/surf/rh.area", "add", "{s}/surf/rh.area.pial"],
                      ["-o", "rh.area.mid", "rh.area.mid", "div", "2"]],
     ("rh.area.mid", "surf/rh.area.mid")),
    ("label-cortex and lh", [["-o", "tmp.mgh", "@surf/lh.w-g.pct.mgh:tmp.mgh", "and",
                              "{s}/surf/lh.w-g.pct.mgh"]], None),
    ("label-cortex and rh", [["-o", "tmp.mgh", "@surf/rh.w-g.pct.mgh:tmp.mgh", "and",
                              "{s}/surf/rh.w-g.pct.mgh"]], None),
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
    work = ws / "oracles" / "inputs" / "fs-calc-real"
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
           "OMP_NUM_THREADS": "1", "HOME": str(work), "USER": "probe",
           "FS_DISABLE_LANG": "1"}  # fmt: skip
    env.update(freesurfer_env())
    ctx = fsfiles.Context(any_directory=True)
    verdicts: Counter = Counter()
    rows = []
    times = Counter()
    recorded = Counter()
    for run in runs:
        subj = next(p for p in (runs_root / run / "subjects").iterdir() if p.name != "fsaverage")
        log = (subj / "scripts" / "recon-all.log").read_text(encoding="utf-8", errors="replace")
        ratio = RATIO.search(log)[1]
        for name, commands, want in CALLS:
            sides = {}
            for side in ("official", "reproduce", "fix"):
                d = work / run / name.replace(" ", "-") / side
                if d.exists():
                    shutil.rmtree(d)
                d.mkdir(parents=True)
                codes, outs, errs = [], "", ""
                for cmd in commands:
                    args, copied = [], []
                    for x in cmd:
                        if x.startswith("@"):
                            src, dst = x[1:].split(":")
                            shutil.copyfile(subj / src, d / dst)
                            copied.append(d / dst)
                            x = dst
                        elif not x.startswith("{") and (d / x).is_file():
                            copied.append(d / x)
                        args.append(x.replace("{s}", str(subj)).replace("{ratio}", ratio))
                    t0 = time.monotonic()
                    if side == "official":
                        done = oracle_cache.run(
                            ["mris_calc", *args],
                            env=env,
                            cwd=d,
                            outputs=[d],
                            policy=compare.POLICY,
                            timeout=3600,
                            runner="fs-calc-real",
                            inputs=copied,
                        )
                        rc, so, se = done.returncode, done.stdout, done.stderr
                        if not done.cached:
                            times[side] += time.monotonic() - t0
                    else:
                        cmdl = [str(port), "--upstream-defects", side, "freesurfer", "mris_calc"]
                        p = subprocess.run([*cmdl, *args], env=env, cwd=d, capture_output=True,
                                           text=True, timeout=3600)  # fmt: skip
                        rc, so, se = p.returncode, p.stdout, p.stderr
                        times[side] += time.monotonic() - t0
                    codes.append(rc)
                    outs += so
                    errs += se
                sides[side] = (codes, outs, errs, d)
            oc, oo, oe, od = sides["official"]
            row = {"run": run, "call": name, "official_exit": oc}
            for side in ("reproduce", "fix"):
                pc, po, pe, pd = sides[side]
                diffs = []
                if pc != oc:
                    diffs.append(f"exit {pc} != {oc}")
                if compare.mask(po, True, pd) != compare.mask(oo, False, od):
                    diffs.append("stdout")
                if compare.mask(pe, True, pd) != compare.mask(oe, False, od):
                    diffs.append("stderr")
                fo, fp = compare.written(od), compare.written(pd)
                if sorted(fo) != sorted(fp):
                    diffs.append(f"files {sorted(fo)} != {sorted(fp)}")
                for f in sorted(set(fo) & set(fp)):
                    c = fsfiles.compare(fo[f], fp[f], ctx, name=f)
                    if not c.same:
                        diffs.append(f"{f}: {c.verdict} {c.detail[:200]}")
                if want is not None:
                    c = fsfiles.compare(subj / want[1], pd / want[0], ctx, name=want[0])
                    row[f"{side}_recorded"] = c.verdict
                    recorded[f"{side}-{c.verdict}"] += 1
                    if not c.same:
                        diffs.append(f"recorded {want[1]}: {c.verdict} {c.detail[:200]}")
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
        "recorded": dict(recorded),
        "seconds": {k: round(v, 1) for k, v in times.items()},
        "rows": rows,
    }
    print(json.dumps({k: v for k, v in summary.items() if k != "rows"}, indent=1))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    return 1 if verdicts.get("different", 0) + verdicts.get("fix-unexplained", 0) else 0


if __name__ == "__main__":
    raise SystemExit(main())
