# SPDX-License-Identifier: Apache-2.0
"""mri_mask on recorded heads beyond recon-all's steps: FreeSurfer 8.2.0's official program against the port.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica).

recon-all's own ``mri_mask`` steps are replayed by the replay tool (``record.py`` reads them). This
adds, on recorded heads (``oracles/freesurfer-8.2.0/runs/<run>/subjects/<subject>/mri``): in the
classic runs the path no recorded step takes, ``-xform`` with a GCA morph (the run's
``brainmask.mgz`` moved into the atlas by its ``transforms/talairach.m3z`` onto the run's
``norm.mgz`` moved there by the official ``mri_convert -at``,
``oracles/inputs/fs-mask-real/<run>/natlas.mgz``; with and without ``-T 5`` and ``-dilate 1``);
in the v8 runs the two calls of recon-all's scripts whose inputs the runs keep (``T1.mgz`` masked
by ``synthstrip.mgz``, the call that makes the v8 stream's ``brainmask.mgz``; ``-bb 3`` on
``orig.mgz`` into a NIfTI file, synthmorph's crop). The official program by name through the oracle-output cache at
one thread and the port (``larmorx-freesurfer --upstream-defects reproduce|fix freesurfer
mri_mask``, built here), each in a folder of its own; compared: exit status, messages and every
file written (FreeSurfer's provenance aside). The other way (the mask in the atlas, the input in
the head: ``Inverting transform to match MRI geometries``, the morph inverted) needs more than
4 GiB in both programs and is not run here.

    python -P validation/probes/fs-mask/real.py [--runs a,b] [--json out.json]
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
from larmorx_validation.freesurfer_unit import FS_HOME, freesurfer_env  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402


def _load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


compare = _load(
    "freesurfer_mri_mask_compare",
    REPO / "validation" / "investigations" / "freesurfer-mri-mask-probe" / "compare.py",
)

RUNS = (
    "ds000005-sub-01-classic",
    "ds000228-sub-pixar008-classic",
    "ds004173-sub-603309-classic",
    "ds000005-sub-01-v8",
    "ds000228-sub-pixar008-v8",
    "ds004173-sub-603309-v8",
)
V8_CALLS = {
    "synthstrip": ["{mri}/T1.mgz", "{mri}/synthstrip.mgz", "o.mgz"],
    "bb3": ["-bb", "3", "{mri}/orig.mgz", "{mri}/orig.mgz", "o.nii.gz"],
}
CALLS = {
    "morph": ["-xform", "{m3z}", "{natlas}", "{mri}/brainmask.mgz", "o.mgz"],
    "morph-T5-dilate": [
        "-xform",
        "{m3z}",
        "-T",
        "5",
        "-dilate",
        "1",
        "{natlas}",
        "{mri}/brainmask.mgz",
        "o.mgz",
    ],
}
MANIFEST = REPO / "crates-freesurfer" / "Cargo.toml"


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
    exe = ws / FS_HOME / "bin" / "mri_mask"
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
           "OMP_NUM_THREADS": "1", "USER": "probe", "FS_DISABLE_LANG": "1"}  # fmt: skip
    env.update(freesurfer_env())
    ctx = fsfiles.Context(any_directory=True)
    rows, verdicts = [], Counter()
    seconds = Counter()
    for run in a.runs.split(","):
        mri = subject_mri(ws, run)
        base = ws / "oracles" / "inputs" / "fs-mask-real" / run
        base.mkdir(parents=True, exist_ok=True)
        m3z = mri / "transforms" / "talairach.m3z"
        natlas = base / "natlas.mgz"
        calls = V8_CALLS if run.endswith("-v8") else CALLS
        made = (
            None
            if run.endswith("-v8")
            else oracle_cache.run(
                ["mri_convert", "-at", str(m3z), str(mri / "norm.mgz"), "natlas.mgz"],
                env=env,
                cwd=base,
                outputs=[natlas],
                policy=compare.POLICY,
                timeout=900,
                runner="fs-mask-real",
            )
        )
        if made is not None and made.returncode != 0:
            raise SystemExit(f"{run}: mri_convert -at failed: {made.stderr[-500:]}")
        for kind, words in calls.items():
            args = [w.format(m3z=m3z, natlas=natlas, mri=mri) for w in words]
            sides = {}
            for side in ("official", "reproduce", "fix"):
                d = base / kind / side
                if d.exists():
                    shutil.rmtree(d)
                d.mkdir(parents=True)
                t0 = time.monotonic()
                if side == "official":
                    p = oracle_cache.run(
                        ["mri_mask", *args],
                        env=env,
                        cwd=d,
                        outputs=[d],
                        policy=compare.POLICY,
                        timeout=900,
                        runner="fs-mask-real",
                    )
                else:
                    p = subprocess.run(
                        [str(port), "--upstream-defects", side, "freesurfer", "mri_mask", *args],
                        env=env,
                        cwd=d,
                        capture_output=True,
                        text=True,
                        errors="surrogateescape",
                        timeout=900,
                    )
                seconds[side] += time.monotonic() - t0
                sides[side] = (compare.status(p.returncode), p.stdout, p.stderr, d)
            oc, oo, oe, od = sides["official"]
            row = {"run": run, "call": kind, "official_exit": oc}
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
                for name in sorted(set(fo) & set(fp)):
                    c = fsfiles.compare(fo[name], fp[name], ctx, name=Path(name).name)
                    if not c.same:
                        diffs.append(f"{name}: {c.verdict} {c.detail[:200]}")
                ids = compare.notices(pe)
                row[side] = {"exit": pc, "differ": diffs, "notices": ids}
                if side == "reproduce":
                    verdicts["identical" if not diffs else "different"] += 1
                else:
                    verdicts["fix-identical" if not diffs else
                             ("fix-explained" if ids else "fix-unexplained")] += 1  # fmt: skip
            rows.append(row)
            print(f"{run} {kind}: exit {oc}, reproduce "
                  f"{'ok' if not row['reproduce']['differ'] else row['reproduce']['differ']}, fix "
                  f"{'ok' if not row['fix']['differ'] else row['fix']['notices']}",
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
