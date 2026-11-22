# SPDX-License-Identifier: Apache-2.0
"""fs_temp_file's validation record: ``docs/validation/freesurfer-fs-temp-file.{json,md}``.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it reads the replica's probe results).

Reads the probe's summaries (``check.py --json`` and ``check.py --golden --json``), the memory
figures of ``tests/parity/test_memory_peak.py`` (its printed lines) and recon-all's recorded call
(the first ``fs-check-version`` line of every reference run's ``recon-all.out``: the name
``fs_temp_file`` printed), and writes the record with the commit it was made from (a clean one: a
record stamped ``-dirty`` is redone).

    python -P validation/probes/fs-temp-file/record.py --probe all.json --golden golden.json \\
        --memory memory.log
"""

from __future__ import annotations

import argparse
import datetime
import json
import platform
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "validation" / "src"))
NAME = "freesurfer-fs-temp-file"


def commit() -> str:
    def git(*a: str) -> str:
        return subprocess.run(
            ["git", "-C", str(REPO), *a], check=True, capture_output=True, text=True
        ).stdout.strip()

    dirty = git("status", "--porcelain", "--untracked-files=no")
    return git("rev-parse", "--short=12", "HEAD") + ("-dirty" if dirty else "")


def recorded_calls() -> dict:
    """The name recon-all's ``fs_temp_file`` gave in every reference run, its letters masked."""
    from larmorx_validation.oracle_cache.store import workspace

    runs = sorted((workspace() / "oracles" / "freesurfer-8.2.0" / "runs").glob("*/recon-all.out"))
    shapes = Counter()
    for f in runs:
        for line in f.read_text(encoding="utf-8", errors="replace").splitlines():
            if line.startswith("fs-check-version --s "):
                shapes[re.sub(r"tmp\.[A-Za-z0-9]{6}$", "tmp.XXXXXX", line.split()[-1])] += 1
                break
    return {"runs": len(runs), "names": dict(shapes)}


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--probe", type=Path, required=True)
    ap.add_argument("--golden", type=Path, required=True)
    ap.add_argument("--memory", type=Path)
    ap.add_argument("--out", type=Path, default=REPO / "docs" / "validation")
    a = ap.parse_args(argv)
    stamp = commit()
    probe = json.loads(a.probe.read_text(encoding="utf-8"))
    golden = json.loads(a.golden.read_text(encoding="utf-8"))
    memory = []
    if a.memory and a.memory.is_file():
        pat = re.compile(
            r"fs_temp_file ([\w-]+) \((\w+)\): larmorx ([\d.]+) MiB, FreeSurfer ([\d.]+)"
        )
        for m in pat.finditer(a.memory.read_text(encoding="utf-8")):
            memory.append({"case": m[1], "mode": m[2], "larmorx_mib": float(m[3]),
                           "freesurfer_mib": float(m[4])})  # fmt: skip
    calls = recorded_calls()
    payload = {
        "name": NAME,
        "tool": "fs_temp_file (FreeSurfer 8.2.0)",
        "status": "experimental",
        "larmorx_commit": stamp,
        "generated": datetime.date.today().isoformat(),
        "platform": f"{platform.system()} {platform.machine()}",
        "replica": "crates-freesurfer/larmorx-freesurfer-volumes (src/fs_temp_file/), "
        "the larmorx-freesurfer program",
        "reference": "FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/fs_temp_file) and "
        "recon-all's reference runs (oracles/freesurfer-8.2.0/runs/)",
        "probe": {
            k: probe[k]
            for k in (
                "cases",
                "identical",
                "different",
                "fix_differs",
                "fix_unexplained",
                "unix_only",
            )
        },
        "probe_sha256": {
            "unix": probe["OFFICIAL_SHA256"],
            "no_modes": probe["OFFICIAL_SHA256_NO_MODES"],
        },
        "golden": {
            "cases": golden["cases"],
            "identical": golden["identical"],
            "sha256": golden["OFFICIAL_SHA256"],
            "sha256_no_modes": golden["OFFICIAL_SHA256_NO_MODES"],
        },
        "recon_all_call": calls,
        "memory": memory,
    }
    a.out.mkdir(parents=True, exist_ok=True)
    (a.out / f"{NAME}.json").write_text(json.dumps(payload, indent=1) + "\n", encoding="utf-8")
    p, g = payload["probe"], payload["golden"]
    names = ", ".join(f"`{k}` in {n}" for k, n in sorted(calls["names"].items()))
    lines = [
        "# Validation: FreeSurfer's `fs_temp_file` (replica)",
        "",
        f"Status `experimental`. Generated {payload['generated']} on {payload['platform']} from "
        f"commit `{stamp}` with `validation/probes/fs-temp-file/record.py` (queue item 3.2-11).",
        "",
        f"- **Replica:** {payload['replica']} (`larmorx-freesurfer [--upstream-defects "
        "reproduce|fix] freesurfer fs_temp_file ...`).",
        f"- **Reference:** {payload['reference']}.",
        "",
        "## recon-all's call",
        "",
        "recon-all runs `fs_temp_file` with no option before its version check; the name it "
        f"printed, in the {calls['runs']} reference runs (the six letters as `XXXXXX`): {names}. "
        "The probe's case without a variable gives that line.",
        "",
        "## The whole program against the official one",
        "",
        "`validation/probes/fs-temp-file/check.py`: each of the five variables of the temporary "
        "folder alone and before the next; folders that do not exist, files, read-only folders, a "
        "link, a final `/`, a blank, a relative name; `TEMPDIR` unusable alone; no variable "
        "(`/tmp`); every suffix (none, empty, `XXXXXX`, a blank, a dash, given twice, a `/`, too "
        "long); `--base` absolute, relative, missing, empty, a file, read-only, twice; `--scratch` "
        "alone and with the others; every message of the parser; the help and version options. "
        "Exit status, both streams, the folder's files with their modes, a file made in `/tmp`, "
        f"case by case: {p['cases']} cases, `reproduce` {p['identical']} identical, "
        f"{p['different']} different; `fix` differs on {p['fix_differs']} (FS-1171, each with "
        f"its notice; {p['fix_unexplained']} without). Recorded: "
        "`crates-freesurfer/larmorx-freesurfer-volumes/tests/fs_temp_file_official.rs` "
        f"(`{payload['probe_sha256']['unix'][:8]}...` on Unix; without modes and without the "
        f"{p['unix_only']} cases only Unix gives the same way, `{payload['probe_sha256']['no_modes'][:8]}...`).",
        "",
        "## Golden cases",
        "",
        f"`tests/fs_temp_file_golden.rs`: {g['cases']} cases (the scripts' two calls, a base with "
        "a suffix, a variable whose folder does not exist, the help) on every platform, both "
        f"modes: `{g['sha256']}` (the official program's value, `check.py --golden`; without "
        f"modes `{g['sha256_no_modes']}`).",
        "",
        "## Memory",
        "",
        "`tests/parity/test_memory_peak.py::test_fs_temp_file_replica_needs_no_more_memory_than_"
        "freesurfer` (one thread; the rule: at most 10 % above FreeSurfer's peak plus 8 MiB; one "
        "file of the same name form and mode 0600 made):",
        "",
    ]
    for e in memory:
        lines.append(
            f"- {e['case']} ({e['mode']}): larmorx {e['larmorx_mib']} MiB, FreeSurfer "
            f"{e['freesurfer_mib']} MiB."
        )
    (a.out / f"{NAME}.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {a.out / NAME}.json and .md ({stamp})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
