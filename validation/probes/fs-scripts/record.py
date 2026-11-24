# SPDX-License-Identifier: Apache-2.0
"""The validation records of seg2cc, mergeseg and fscalc: ``docs/validation/freesurfer-seg2cc``,
``freesurfer-mergeseg`` and ``freesurfer-fscalc`` (``.json`` and ``.md``).

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it reads the replica's probe results).

Reads the probe's summary (``check.py --heavy --json``), the replays of recon-all's recorded seg2cc
steps (``replay.sh``'s JSON reports, one folder per mode), the second path on real heads
(``real.py --json``) and the memory figures of ``tests/parity/test_memory_peak.py`` (its printed
lines), and writes the records with the commit they were made from (a clean one: a record stamped
``-dirty`` is redone).

    python -P validation/probes/fs-scripts/record.py --probe all.json \\
        --replays <work>/replay-reproduce <work>/replay-fix --real real.json --memory memory.log
"""

from __future__ import annotations

import argparse
import datetime
import json
import platform
import re
import subprocess
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
ITEMS = {
    "seg2cc": "3.2-28",
    "mergeseg": "3.2-29",
    "fscalc": "3.5-29 (in the volumes crate: mergeseg runs it)",
}


def commit() -> str:
    def git(*a: str) -> str:
        return subprocess.run(
            ["git", "-C", str(REPO), *a], check=True, capture_output=True, text=True
        ).stdout.strip()

    dirty = git("status", "--porcelain", "--untracked-files=no")
    return git("rev-parse", "--short=12", "HEAD") + ("-dirty" if dirty else "")


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--probe", type=Path, required=True)
    ap.add_argument("--replays", type=Path, nargs="+", required=True)
    ap.add_argument("--real", type=Path, required=True)
    ap.add_argument("--memory", type=Path)
    ap.add_argument("--out", type=Path, default=REPO / "docs" / "validation")
    a = ap.parse_args(argv)
    stamp = commit()
    probe = json.loads(a.probe.read_text(encoding="utf-8"))
    real = json.loads(a.real.read_text(encoding="utf-8"))["rows"]
    replays: dict[str, Counter] = {}
    for folder in a.replays:
        mode = folder.name.removeprefix("replay-")
        c: Counter = Counter()
        for p in sorted(folder.glob("*.json")):
            for step in json.loads(p.read_text(encoding="utf-8")):
                c[str(step.get("verdict", "?")).lower()] += 1
        replays[mode] = c
    memory = []
    if a.memory and a.memory.is_file():
        pat = re.compile(
            r"(?:^|\.)(seg2cc|mergeseg) \((\w+)\): larmorx ([\d.]+) MiB, FreeSurfer ([\d.]+)"
        )
        for line in a.memory.read_text(encoding="utf-8").splitlines():
            m = pat.search(line)
            if m:
                memory.append({"program": m[1], "mode": m[2], "larmorx_mib": float(m[3]),
                               "freesurfer_mib": float(m[4])})  # fmt: skip
    a.out.mkdir(parents=True, exist_ok=True)
    for prog, item in ITEMS.items():
        rows = [r for r in probe["rows"] if r["prog"] == prog]
        v = Counter(r["verdict"] for r in rows)
        fv = Counter(r["fix"] for r in rows)
        payload = {
            "name": f"freesurfer-{prog}",
            "tool": f"{prog} (FreeSurfer 8.2.0)",
            "status": "experimental",
            "larmorx_commit": stamp,
            "generated": datetime.date.today().isoformat(),
            "platform": f"{platform.system()} {platform.machine()}",
            "replica": f"crates-freesurfer/larmorx-freesurfer-volumes (src/{prog}/, src/script.rs), "
            "the larmorx-freesurfer program",
            "reference": f"FreeSurfer 8.2.0's official script ($FREESURFER_HOME/bin/{prog}, "
            "/bin/tcsh 6.21.00, GNU coreutils 8.32, GNU time 1.9)",
            "queue_item": item,
            "cases": len(rows),
            "verdicts": dict(v),
            "fix": dict(fv),
            "official_sha256_all": probe["official_sha256"],
            "golden_sha256": probe["golden_sha256"],
            "memory": [e for e in memory if e["program"] == prog],
        }
        if prog == "seg2cc":
            payload["recorded_steps"] = {k: dict(c) for k, c in replays.items()}
            payload["second_path_real_heads"] = [
                {k: r[k] for k in ("run", "reproduce", "fix", "seconds", "official_status")}
                for r in real
            ]
        (a.out / f"freesurfer-{prog}.json").write_text(
            json.dumps(payload, indent=1) + "\n", encoding="utf-8"
        )
        lines = [
            f"# Validation: FreeSurfer's `{prog}` (replica)",
            "",
            f"Status `experimental`. Generated {payload['generated']} on {payload['platform']} "
            f"from commit `{stamp}` with `validation/probes/fs-scripts/record.py` (queue item "
            f"{item}).",
            "",
            f"- **Replica:** {payload['replica']} (`larmorx-freesurfer [--upstream-defects "
            f"reproduce|fix] freesurfer {prog} ...`).",
            f"- **Reference:** {payload['reference']}.",
            "",
        ]
        if prog == "seg2cc":
            lines += [
                "## recon-all's recorded steps",
                "",
                "The seg2cc step of every recorded run (the six heads, both streams; `replay.sh`: "
                "the subject rebuilt before the step, the step run, `aseg.auto.mgz`, "
                "`transforms/cc_up.lta` and `scripts/seg2cc.log` compared, FreeSurfer's "
                "provenance and the log's dates and times aside):",
                "",
            ]
            for mode, c in replays.items():
                lines.append(f"- `{mode}`: {dict(c)}.")
            lines += [
                "",
                "## The second path (a segmentation not conformed), on real heads",
                "",
                "`real.py`: three heads' `aseg.auto_noCCseg.mgz` and `norm.mgz` without their last "
                "slice (256 x 256 x 255), so that seg2cc conforms them, runs `mri_cc -norm`, maps "
                "the callosum back and merges its five labels (mri_binarize, mergeseg, fscalc); "
                "status, both streams and every file compared:",
                "",
            ]
            for r in real:
                lines.append(
                    f"- {r['run']}: `reproduce` {r['reproduce']}, the default {r['fix']} "
                    f"(official {r['seconds'].get('official')} s, port "
                    f"{r['seconds'].get('reproduce')} s)."
                )
            lines.append("")
        lines += [
            "## The script against the official one",
            "",
            f"`validation/probes/fs-scripts/check.py` ({prog}'s cases of "
            "`crates-freesurfer/larmorx-freesurfer-volumes/tests/fs_scripts/cases.rs`: every "
            "option, message and failure, the environment's variables, the names tcsh reads "
            "again; 32^3 volumes, NaNs and infinities in a float one): status, both streams and "
            f"every file written. {len(rows)} cases: `reproduce` {dict(v)}; the default "
            f"{dict(fv)}. Not reproduced (both modes give the names as given, with a notice): "
            "tcsh's reading of names (FS-1277), `--debug`'s trace"
            + (", fscalc's number as a first input (mri_volsynth)" if prog == "fscalc" else "")
            + ".",
            "",
            "Recorded: `tests/fs_scripts_official.rs` (every case of the three scripts but the "
            f"heavy ones `{probe['official_sha256'][:8]}...`, a debug build's quick set "
            f"`{probe['official_sha256_quick'][:8]}...`); golden `tests/scripts_golden.rs` "
            f"(`{probe['golden_sha256'][:8]}...` with `reproduce`).",
            "",
        ]
        mem = payload["memory"]
        if mem:
            lines += [
                "## Memory",
                "",
                "`tests/parity/test_memory_peak.py::test_scripts_replica_needs_no_more_memory_"
                "than_freesurfer` (at most 10 % above FreeSurfer's peak plus 8 MiB; the same "
                "outputs):",
                "",
            ]
            for e in mem:
                lines.append(
                    f"- {e['mode']}: larmorx {e['larmorx_mib']} MiB, FreeSurfer "
                    f"{e['freesurfer_mib']} MiB (its largest program)."
                )
            lines.append("")
        (a.out / f"freesurfer-{prog}.md").write_text("\n".join(lines), encoding="utf-8")
    print(f"wrote the three records ({stamp})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
