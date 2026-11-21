# SPDX-License-Identifier: Apache-2.0
"""fMRIPrep's own call of mri_convert, official program against the replica.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica).

fMRIPrep 25.2.6 calls ``mri_convert`` once per run, as its FreeSurfer licence check
(``niworkflows/utils/misc.py``'s ``check_valid_fs_license``: ``mri_convert <sentinel.nii.gz>
<tmp>/out.mgz``, the status and ``ERROR:`` in the output judged). This runs that call with
FreeSurfer 8.2.0's official program (by name, ``FS_LICENSE`` set) and with the port in both modes,
each in its own folder, and compares the exit status, standard output (after its first line, the
command as called), standard error and the written volume (``larmorx_validation.freesurfer``'s
comparison: FreeSurfer's provenance aside). Without a licence the official program stops (status
255, the licence message); the port needs none.

    python -P validation/probes/fs-convert/fmriprep_call.py --port <larmorx-freesurfer> [--json out.json]
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "validation" / "src"))

from larmorx_validation.freesurfer import files as fsfiles  # noqa: E402
from larmorx_validation.freesurfer_unit import FS_HOME, freesurfer_env  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--port", type=Path, required=True)
    ap.add_argument("--work", type=Path, required=True, help="an empty folder for the runs")
    ap.add_argument("--json", type=Path)
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    exe = ws / FS_HOME / "bin" / "mri_convert"
    sentinel = ws / "niworkflows" / "niworkflows" / "data" / "sentinel.nii.gz"
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
           "OMP_NUM_THREADS": "1", "USER": "probe", **freesurfer_env()}  # fmt: skip
    runs = {}
    for name, cmd in [
        ("official", ["mri_convert", str(sentinel), "out.mgz"]),
        ("reproduce", [str(a.port), "--upstream-defects", "reproduce", "freesurfer",
                       "mri_convert", str(sentinel), "out.mgz"]),
        ("fix", [str(a.port), "--upstream-defects", "fix", "freesurfer", "mri_convert",
                 str(sentinel), "out.mgz"]),
    ]:  # fmt: skip
        d = a.work / name
        d.mkdir(parents=True, exist_ok=False)
        p = subprocess.run(cmd, env=env, cwd=d, capture_output=True, text=True, timeout=300)
        runs[name] = (p.returncode, p.stdout.splitlines()[1:], p.stderr, d / "out.mgz")
    off = runs["official"]
    result = {"call": "mri_convert <niworkflows' sentinel.nii.gz> out.mgz", "modes": {}}
    for mode in ("reproduce", "fix"):
        r = runs[mode]
        c = fsfiles.compare(off[3], r[3], fsfiles.Context(any_directory=True), name="out.mgz")
        result["modes"][mode] = {
            "status_same": r[0] == off[0],
            "stdout_same": r[1] == off[1],
            "stderr_same": r[2] == off[2],
            "volume": c.verdict,
        }
    print(json.dumps(result, indent=1))
    if a.json:
        a.json.write_text(json.dumps(result, indent=1) + "\n", encoding="utf-8")
    ok = all(
        v["status_same"] and v["stdout_same"] and v["stderr_same"] and v["volume"] in
        ("identical", "provenance") for v in result["modes"].values()
    )  # fmt: skip
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
