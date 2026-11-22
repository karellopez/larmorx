# SPDX-License-Identifier: Apache-2.0
"""mri_add_xform_to_header's validation record from the replica's runs: ``docs/validation/freesurfer-mri-add-xform-to-header.{json,md}``.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it reads the replica's probe results).

Reads the replays of recon-all's recorded steps (``freesurfer-replay run ... --json``, one file per
run and mode: only their verdicts and times are kept, never their paths), the probe's summaries
(``check.py --json``), the golden values and the memory figures of
``tests/parity/test_memory_peak.py`` (its printed lines), and writes the record with the commit it
was made from (a clean one: a record stamped ``-dirty`` is redone).

    python -P validation/probes/fs-add-xform/record.py --replays <dir> --probe <a.json> ... \\
        --memory <memory.log> --golden <official sha> --golden-fix <sha>
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
NAME = "freesurfer-mri-add-xform-to-header"


def commit() -> str:
    def git(*a: str) -> str:
        return subprocess.run(
            ["git", "-C", str(REPO), *a], check=True, capture_output=True, text=True
        ).stdout.strip()

    dirty = git("status", "--porcelain", "--untracked-files=no")
    return git("rev-parse", "--short=12", "HEAD") + ("-dirty" if dirty else "")


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--replays", type=Path, required=True, help="<dir>/replay-<mode>/<run>.json")
    ap.add_argument("--modes", default="reproduce,fix")
    ap.add_argument("--probe", type=Path, nargs="+", required=True)
    ap.add_argument("--memory", type=Path)
    ap.add_argument("--golden", required=True)
    ap.add_argument("--golden-fix", required=True)
    ap.add_argument("--out", type=Path, default=REPO / "docs" / "validation")
    a = ap.parse_args(argv)
    stamp = commit()
    replays = {}
    for mode in a.modes.split(","):
        rows = []
        for f in sorted((a.replays / f"replay-{mode}").glob("*.json")):
            rows.extend(json.loads(f.read_text(encoding="utf-8")))
        replays[mode] = {
            "steps": len(rows),
            "runs": len({r["run"] for r in rows}),
            "verdicts": dict(Counter(r["verdict"] for r in rows)),
            "seconds_recorded": round(sum(r["seconds"]["recorded"] for r in rows), 1),
            "seconds_replayed": round(sum(r["seconds"]["replayed"] for r in rows), 1),
        }
    # a summary of check.py --extra (its file named so) is the extra cases', not a seed's
    probes = [
        json.loads(p.read_text(encoding="utf-8")) | {"extra": "extra" in p.name} for p in a.probe
    ]
    memory = []
    if a.memory and a.memory.is_file():
        pat = re.compile(
            r"mri_add_xform_to_header ([\w-]+) \((\w+)\): larmorx ([\d.]+) MiB, FreeSurfer ([\d.]+)"
        )
        for m in pat.finditer(a.memory.read_text(encoding="utf-8")):
            memory.append(
                {"case": m[1], "mode": m[2], "larmorx_mib": float(m[3]),
                 "freesurfer_mib": float(m[4])}
            )  # fmt: skip
    payload = {
        "name": NAME,
        "tool": "mri_add_xform_to_header (FreeSurfer 8.2.0)",
        "status": "experimental",
        "larmorx_commit": stamp,
        "generated": datetime.date.today().isoformat(),
        "platform": f"{platform.system()} {platform.machine()}",
        "replica": "crates-freesurfer/larmorx-freesurfer-volumes (src/mri_add_xform_to_header/), "
        "the larmorx-freesurfer program",
        "reference": "FreeSurfer 8.2.0's recon-all runs (oracles/freesurfer-8.2.0/runs/: every "
        "step recorded) and its official program ($FREESURFER_HOME/bin/mri_add_xform_to_header)",
        "replay": replays,
        "probe": [
            {
                "set": "extra" if p["extra"] else f"seed {p['seed']}",
                "cases": p["cases"],
                "identical": p["reproduce_equal"],
                "different": p["reproduce_differ"],
                "skipped": p["skipped"],
                "fix_different_with_notice": p["fix_differ"] - p["fix_differ_without_notice"],
                "fix_different_without_notice": p["fix_differ_without_notice"],
            }
            for p in probes
        ],
        "golden_sha256": {"official_and_reproduce": a.golden, "fix": a.golden_fix},
        "memory": memory,
    }
    a.out.mkdir(parents=True, exist_ok=True)
    (a.out / f"{NAME}.json").write_text(json.dumps(payload, indent=1) + "\n", encoding="utf-8")
    lines = [
        "# Validation: FreeSurfer's `mri_add_xform_to_header` (replica)",
        "",
        f"Status `experimental`. Generated {payload['generated']} on {payload['platform']} from "
        f"commit `{stamp}` with `validation/probes/fs-add-xform/record.py` (queue item 3.2-10).",
        "",
        f"- **Replica:** {payload['replica']} (`larmorx-freesurfer [--upstream-defects "
        "reproduce|fix] freesurfer mri_add_xform_to_header ...`).",
        f"- **Reference:** {payload['reference']}.",
        "",
        "## The recorded steps",
        "",
        "recon-all runs the program twice per head: `-c <subject>/mri/transforms/talairach.xfm "
        "orig.mgz orig.mgz` (before the transform exists) and the same for `nu.mgz`. Each recorded "
        "step replayed on the subject folder as it was before it "
        "(`python -m larmorx_validation freesurfer-replay run`), its files compared with the "
        "recorded ones (FreeSurfer's provenance aside: the subjects folder's path in the "
        "transform name):",
        "",
    ]
    for mode, r in replays.items():
        v = ", ".join(f"{n} {k}" for k, n in sorted(r["verdicts"].items()))
        lines.append(
            f"- `{mode}`: {r['steps']} steps on {r['runs']} runs: {v} ({r['seconds_replayed']} s "
            f"in all; {r['seconds_recorded']} s recorded)."
        )
    lines += [
        "",
        "## The whole program against the official one",
        "",
        "`validation/probes/fs-add-xform/check.py`: the command line (no argument, every option, "
        "the help options, unknown options, `-n`'s argument, missing names, extra arguments), "
        "each input (MGZ and MGH volumes naming a transform that loads, is missing, does not "
        "read or fails after its first transform; names FreeSurfer refuses: other extensions, "
        "none, a dot last, a dot in a folder's name; a damaged volume; COR folders with and "
        "without their transform, of slices thicker than their pixels; empty folders; missing "
        "names), each transform file (good, inverted, two concatenated, failing after the first, "
        "a wrong header, a word for a number, a singular inverse, none, empty, completed by "
        "`.xfm`, missing, a folder, empty name), each output (MGZ, MGH, NIfTI, compressed NIfTI, "
        "Analyze, an unknown format, a missing folder, folders: their `COR-.info`, one that "
        "cannot be written), the environment (`FS_SKIP_TAGS`, `DIAG_VERBOSE`), a transform name "
        "of 1,104 bytes; then seeded random volumes and options. Exit status, both streams and "
        "every file and folder after the run, case by case; counted apart: the cases whose "
        "result in FreeSurfer is undefined (`check.py`'s `SKIP`: a crash or memory contents):",
        "",
    ]
    for p in payload["probe"]:
        name = "the extra cases (Linux's names only)" if p["set"] == "extra" else p["set"]
        lines.append(
            f"- {name}: {p['cases']} cases; `reproduce` {p['identical']} identical, "
            f"{p['different']} different, {p['skipped']} counted; `fix` "
            f"{p['fix_different_with_notice']} different with a notice, "
            f"{p['fix_different_without_notice']} without."
        )
    lines += [
        "",
        "## Golden cases",
        "",
        "`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_add_xform_to_header_golden.rs`: "
        "5 cases on a 32^3 byte volume made with integer arithmetic, as MGZ and as a COR folder "
        "(recon-all's call, the transform read, an MGH output, a COR folder's info written "
        "elsewhere and in place), on every platform: `reproduce` "
        f"`{a.golden}` (the official program's value on the same inputs, `check.py --golden`), "
        f"`fix` `{a.golden_fix}` (FS-1158: the COR folder's transform name kept with `-c`).",
        "",
        "## Memory",
        "",
        "`tests/parity/test_memory_peak.py::test_mri_add_xform_to_header_replica_needs_no_more_"
        "memory_than_freesurfer` (ds000005 sub-01's `orig.mgz`, one thread; the rule: at most "
        "10 % above FreeSurfer's peak plus 8 MiB; the output identical byte for byte):",
        "",
    ]
    for e in memory:
        lines.append(
            f"- {e['case']} ({e['mode']}): {e['larmorx_mib']:.1f} MiB, FreeSurfer "
            f"{e['freesurfer_mib']:.1f} MiB."
        )
    (a.out / f"{NAME}.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"{NAME}: {stamp}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
