# SPDX-License-Identifier: Apache-2.0
"""mri_make_uchar's real calls on recorded heads: FreeSurfer 8.2.0's official program against the port.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica).

``mri_make_uchar`` is no step of its own in the reference runs: ``mri_nu_correct.mni`` runs it
last (``mri_make_uchar nu.mgz transforms/talairach.xfm nu.mgz``, on its rescaled N3 output, a
``float`` volume, which the runs do not keep: the step's recorded ``nu.mgz`` is its output). This
runs that call on the recorded heads (``oracles/freesurfer-8.2.0/runs/<run>/subjects/<subject>``;
three heads, both streams) with stand-ins: the call itself, in place, on the run's ``orig.mgz``
times the run's own recorded rescale ratio as ``float`` (``nu.mgz``, made here into
``oracles/inputs/fs-make-uchar-real/<run>/``), with the run's ``transforms/talairach.xfm``; the
run's ``orig.mgz`` (``uchar``); the run's ``T1.mgz`` with the histogram (``-h``) and ball (``-v``)
files and a 60 mm ball. The official program by name through the oracle-output cache at one
thread and the port (``larmorx-freesurfer --upstream-defects reproduce|fix freesurfer
mri_make_uchar``, built here), each in a folder of its own; compared: exit status, messages (the
timing line's figures aside) and every file written (FreeSurfer's provenance aside).

    python -P validation/probes/fs-make-uchar/real.py [--runs a,b] [--json out.json]
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

import nibabel as nib
import numpy as np

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
    "freesurfer_mri_make_uchar_compare",
    REPO / "validation" / "investigations" / "freesurfer-mri-make-uchar-probe" / "compare.py",
)

HEADS = ("ds000005-sub-01", "ds000228-sub-pixar008", "ds004173-sub-603309")
#: mri_nu_correct.mni's rescale in the run's log.
RATIO = re.compile(r"^mris_calc -o \S*nu0\.mgz \S*nu0\.mgz mul (\S+)$", re.M)
#: The run's own line of the call (its input is not kept: shown, not compared).
MAPPING = re.compile(r"^#mri_make_uchar# mapping .*$", re.M)

#: The calls: a name and the arguments (``{s}/x``: the subject's ``x``; ``@nu``: the stand-in
#: copied into the case's folder as ``nu.mgz``).
CALLS: list[tuple[str, list[str]]] = [
    ("nu in place", ["@nu", "{s}/mri/transforms/talairach.xfm", "nu.mgz"]),
    ("orig", ["{s}/mri/orig.mgz", "{s}/mri/transforms/talairach.xfm", "o.mgz"]),
    ("T1 files", ["-h", "h.txt", "-v", "rad.mgz", "-r", "60", "{s}/mri/T1.mgz",
                  "{s}/mri/transforms/talairach.xfm", "o.mgz"]),
]  # fmt: skip


def stand_in(subj: Path, ratio: str, path: Path) -> Path:
    """The run's ``orig.mgz`` times its rescale ratio, as ``float`` (the header kept)."""
    if not path.exists():
        path.parent.mkdir(parents=True, exist_ok=True)
        im = nib.load(str(subj / "mri" / "orig.mgz"))
        d = np.asarray(im.dataobj).astype(np.float32) * np.float32(float(ratio))
        hdr = im.header.copy()
        hdr.set_data_dtype(np.float32)
        nib.save(nib.MGHImage(d, im.affine, hdr), str(path))
    return path


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
    work = ws / "oracles" / "inputs" / "fs-make-uchar-real"
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
           "OMP_NUM_THREADS": "1", "HOME": str(work), "USER": "probe",
           "FS_DISABLE_LANG": "1"}  # fmt: skip
    env.update(freesurfer_env())
    ctx = fsfiles.Context(any_directory=True)
    verdicts: Counter = Counter()
    rows = []
    times: Counter = Counter()
    for run in runs:
        subj = next(p for p in (runs_root / run / "subjects").iterdir() if p.name != "fsaverage")
        log = (subj / "scripts" / "recon-all.log").read_text(encoding="utf-8", errors="replace")
        ratio = RATIO.search(log)[1]
        recorded = MAPPING.search(log)[0]
        nu = stand_in(subj, ratio, work / "standins" / run / "nu.mgz")
        for name, cmd in CALLS:
            sides = {}
            for side in ("official", "reproduce", "fix"):
                d = work / run / name.replace(" ", "-") / side
                if d.exists():
                    shutil.rmtree(d)
                d.mkdir(parents=True)
                args, copied = [], []
                for x in cmd:
                    if x == "@nu":
                        shutil.copyfile(nu, d / "nu.mgz")
                        copied.append(d / "nu.mgz")
                        x = "nu.mgz"
                    args.append(x.replace("{s}", str(subj)))
                t0 = time.monotonic()
                if side == "official":
                    done = oracle_cache.run(
                        ["mri_make_uchar", *args],
                        env=env,
                        cwd=d,
                        outputs=[d],
                        policy=compare.POLICY,
                        timeout=3600,
                        runner="fs-make-uchar-real",
                        inputs=copied,
                    )
                    rc, so, se = done.returncode, done.stdout, done.stderr
                    if not done.cached:
                        times[side] += time.monotonic() - t0
                else:
                    cmdl = [str(port), "--upstream-defects", side, "freesurfer", "mri_make_uchar"]
                    p = subprocess.run([*cmdl, *args], env=env, cwd=d, capture_output=True,
                                       text=True, timeout=3600)  # fmt: skip
                    rc, so, se = p.returncode, p.stdout, p.stderr
                    times[side] += time.monotonic() - t0
                sides[side] = (rc, so, se, d)
            oc, oo, oe, od = sides["official"]
            row = {"run": run, "call": name, "official_exit": oc,
                   "mapping": MAPPING.search(oo)[0] if MAPPING.search(oo) else None}  # fmt: skip
            if name == "nu in place":
                row["recorded_mapping"] = recorded
            for side in ("reproduce", "fix"):
                pc, po, pe, pd = sides[side]
                diffs = []
                if pc != compare.status(oc):
                    diffs.append(f"exit {pc} != {compare.status(oc)}")
                if compare.mask(po, True, pd) != compare.mask(oo, False, od):
                    diffs.append("stdout")
                if compare.mask(pe, True, pd) != compare.mask(oe, False, od):
                    diffs.append("stderr")
                fo, fp = compare.written(od), compare.written(pd)
                if sorted(fo) != sorted(fp):
                    diffs.append(f"files {sorted(fo)} != {sorted(fp)}")
                for f in sorted(set(fo) & set(fp)):
                    c = fsfiles.compare(fo[f], fp[f], ctx, name=Path(f).name)
                    if not c.same:
                        diffs.append(f"{f}: {c.verdict} {c.detail[:200]}")
                ids = compare.notices(pe)
                row[side] = {"exit": pc, "differ": diffs, "notices": ids}
                if side == "reproduce":
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
