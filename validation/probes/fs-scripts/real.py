# SPDX-License-Identifier: Apache-2.0
"""seg2cc's path for a segmentation not conformed to its smallest voxel, on recorded heads:
FreeSurfer 8.2.0's official script against the port.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica).

recon-all's heads are conformed (256^3 at 1 mm), so its recorded seg2cc steps take the first path
(``replay.sh``). Here each head's ``aseg.auto_noCCseg.mgz`` and ``norm.mgz`` (the recording's) are
cut to 256 x 256 x 255 (the last slice dropped, the voxel-to-world matrix kept), which makes
``mri_info --conformed-to-min`` answer ``no``: seg2cc then conforms them (``mri_convert
--conform_min``, ``mri_label2vol``), runs ``mri_cc -norm`` there, maps the result back and merges
its five labels with ``mri_binarize`` and ``mergeseg`` (``fscalc``, ``mri_mask``, ``mri_convert
--ctab``). The official script (the reference installation's ``bin`` first on ``PATH``, the C
locale, ``TZ=UTC``) through the oracle-output cache (never stored: the outputs hold the clock and
process numbers), the port in both modes; each in a subjects folder of its own; compared with the
example ``fs_scripts_probe``'s canonical text (status, both streams, every file the folder holds,
what varies from run to run masked).

    scripts/orchestration/boxed.sh 16 python -P validation/probes/fs-scripts/real.py [--runs a,b] [--json out.json]
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

import nibabel as nib

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "validation" / "src"))

from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.freesurfer_unit import FS_HOME, freesurfer_env  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402

RELEASE = REPO / "crates-freesurfer" / "target" / "release"
EXAMPLE = RELEASE / "examples" / "fs_scripts_probe"
PORT = RELEASE / "larmorx-freesurfer"
NEVER = "the scripts' outputs hold the run's clock, process number and temporary names"
RUNS = ("ds000005-sub-01-v8", "ds002422-sub-02-v8", "ds005454-sub-16-v8")


def cut(src: Path, dst: Path) -> None:
    """``src`` without its last slice (the third axis), as an MGZ with the same matrix."""
    img = nib.load(str(src))
    data = img.get_fdata(dtype="float32") if img.get_data_dtype().kind == "f" else img.dataobj[...]
    out = nib.MGHImage(data[:, :, :-1].astype(img.get_data_dtype()), img.affine, img.header)
    nib.save(out, str(dst))


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--runs", default=",".join(RUNS))
    ap.add_argument("--json", type=Path)
    a = ap.parse_args(argv)
    ws = workspace()
    fsbin = ws / FS_HOME / "bin"
    uname = subprocess.run(["uname", "-a"], capture_output=True, text=True).stdout.rstrip("\n")
    work = ws / "oracles" / "inputs" / "fs-scripts-real"
    rows, bad = [], 0
    for run in a.runs.split(","):
        mri = ws / "oracles" / "freesurfer-8.2.0" / "runs" / run / "subjects" / "sub-01"
        if not mri.exists():
            mri = next(
                p for p in sorted((mri.parent).iterdir()) if p.name != "fsaverage" and p.is_dir()
            )
        mri = mri / "mri"
        base = work / run
        if base.exists():
            shutil.rmtree(base)
        inputs = base / "inputs"
        inputs.mkdir(parents=True)
        for name in ("aseg.auto_noCCseg.mgz", "norm.mgz"):
            cut(mri / name, inputs / name)
        texts, secs = {}, {}
        for side in ("official", "reproduce", "fix"):
            d = base / side / "case"
            smri = d / "subjects" / "sub" / "mri"
            smri.mkdir(parents=True)
            for name in ("aseg.auto_noCCseg.mgz", "norm.mgz"):
                shutil.copy(inputs / name, smri / name)
            env = {
                "FREESURFER_HOME": str(ws / FS_HOME),
                "SUBJECTS_DIR": str(d / "subjects"),
                "HOME": str(d),
                "LC_ALL": "C",
                "LANG": "C",
                "TZ": "UTC",
                "USER": "probe",
                "FS_DISABLE_LANG": "1",
                "OMP_NUM_THREADS": "1",
                "PATH": f"{fsbin}:/usr/bin:/bin",
                "FS_LICENSE": freesurfer_env()["FS_LICENSE"],
            }
            t0 = time.monotonic()
            if side == "official":
                done = oracle_cache.run(
                    ["seg2cc", "--s", "sub"],
                    env=env,
                    cwd=d,
                    policy=oracle_cache.Policy(threads=1, family="freesurfer", never=NEVER),
                    timeout=3600,
                    runner="fs-scripts-real",
                )
                rc, out, err = done.returncode, done.stdout_bytes, done.stderr_bytes
            else:
                p = subprocess.run(
                    [str(PORT), "--upstream-defects", side, "freesurfer", "seg2cc", "--s", "sub"],
                    env=env, cwd=d, capture_output=True, timeout=3600, check=False,
                )  # fmt: skip
                rc, out, err = p.returncode, p.stdout, p.stderr
            secs[side] = round(time.monotonic() - t0, 1)
            (base / side / "out").write_bytes(out)
            (base / side / "err").write_bytes(err)
            status = 128 - rc if rc < 0 else rc
            texts[side] = subprocess.run(
                [str(EXAMPLE), "outcome", str(d), str(status), str(base / side / "out"),
                 str(base / side / "err"), uname],
                check=True, capture_output=True, text=True, errors="surrogateescape",
                env=dict(os.environ, FS_SCRIPTS_PROBE_BIN=str(fsbin)),
            ).stdout  # fmt: skip
        row = {"run": run, "seconds": secs}
        for side in ("reproduce", "fix"):
            same = texts[side] == texts["official"]
            row[side] = "identical" if same else "different"
            if not same:
                bad += 1
                import difflib

                row[side + "_diff"] = list(
                    difflib.unified_diff(
                        texts["official"].splitlines(), texts[side].splitlines(), lineterm=""
                    )
                )[:60]
        row["official_status"] = texts["official"].splitlines()[0]
        print(json.dumps({k: v for k, v in row.items() if not k.endswith("_diff")}), flush=True)
        for side in ("reproduce", "fix"):
            for line in row.get(side + "_diff", [])[:30]:
                print("   ", side, line)
        rows.append(row)
        shutil.rmtree(base)
    if a.json:
        a.json.write_text(json.dumps({"rows": rows}, indent=1) + "\n", encoding="utf-8")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
