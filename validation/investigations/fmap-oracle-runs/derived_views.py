# SPDX-License-Identifier: Apache-2.0
"""The BIDS inputs of the field maps' part B oracle runs (fMRIPrep 25.2.6) for the paths no earlier
run reaches: PEPOLAR (two opposite-direction EPIs, and one EPI with the BOLD itself) and a direct
field map in Hz.

Each view hard-links a dataset's cached files (the bytes the test-data catalog verified; nothing is
copied) and writes as new files only the JSON sidecars it changes, so the cache is never altered.
The changes link a field map to its BOLD run with BIDS' ``IntendedFor`` (paths relative to the
subject's directory), or remove a link; each changed sidecar's SHA-256 is written beside the view
(``<view>.DERIVED.txt``), with the original's.

- ``ds006736``: sub-004's AP/PA EPIs name six BOLD runs of which the catalog holds one
  (``task-freeRecall``): their ``IntendedFor`` keeps that one.
- ``ds004958-pepolar``: sub-fsm64ps's spin-echo AP/PA pair, unlinked in the dataset, linked to its
  resting-state BOLD (the gradient-echo phase difference stays unlinked).
- ``ds002603-epi``: sub-01's phase difference loses its ``IntendedFor``, so the two single-direction
  EPIs (each linked to its BOLD) become fMRIPrep's EPI-and-BOLD PEPOLAR estimators.
- ``ds005752-direct``: sub-ON01016's Hz field map (with its magnitude), unlinked in the dataset,
  linked to the session's two BOLD runs.

Usage: python -P derived_views.py <workspace>   (writes <workspace>/oracles/bids-views/<view>)
"""

import hashlib
import json
import os
import sys
from pathlib import Path

CACHE = Path.home() / ".cache" / "larmorx-testdata" / "data" / "openneuro"

#: view name -> (dataset, participant directory, {sidecar path in the subject: IntendedFor or None})
VIEWS = {
    "ds006736": (
        "ds006736",
        "sub-004",
        {
            "fmap/sub-004_dir-AP_epi.json": ["func/sub-004_task-freeRecall_bold.nii.gz"],
            "fmap/sub-004_dir-PA_epi.json": ["func/sub-004_task-freeRecall_bold.nii.gz"],
        },
    ),
    "ds004958-pepolar": (
        "ds004958",
        "sub-fsm64ps",
        {
            "fmap/sub-fsm64ps_acq-se_dir-AP_epi.json": ["func/sub-fsm64ps_task-rest_bold.nii.gz"],
            "fmap/sub-fsm64ps_acq-se_dir-PA_epi.json": ["func/sub-fsm64ps_task-rest_bold.nii.gz"],
        },
    ),
    "ds002603-epi": (
        "ds002603",
        "sub-01",
        {"fmap/sub-01_acq-Mm2_phasediff.json": None},
    ),
    "ds005752-direct": (
        "ds005752",
        "sub-ON01016",
        {
            "ses-01/fmap/sub-ON01016_ses-01_acq-bold_fieldmap.json": [
                "ses-01/func/sub-ON01016_ses-01_task-rest_dir-forward_bold.nii.gz",
                "ses-01/func/sub-ON01016_ses-01_task-rest_dir-reverse_bold.nii.gz",
            ],
        },
    ),
}


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def make(views_dir: Path, name: str) -> list[str]:
    ds, sub, changes = VIEWS[name]
    src = CACHE / ds
    out = views_dir / name
    if out.exists():
        raise SystemExit(f"{out} exists: remove it by hand first (never overwritten)")
    # only the participant's files and the dataset's top-level files
    for path in sorted(src.rglob("*")):
        rel = path.relative_to(src)
        if path.is_dir() or (rel.parts[0].startswith("sub-") and rel.parts[0] != sub):
            continue
        target = out / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        os.link(path, target)
    lines = []
    for rel, intended in changes.items():
        original = src / sub / rel
        target = out / sub / rel
        data = json.loads(original.read_text(encoding="utf-8"))
        if intended is None:
            data.pop("IntendedFor", None)
        else:
            data["IntendedFor"] = intended
        target.unlink()  # the hard link to the cache: a new file replaces it
        target.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
        lines.append(
            f"{sub}/{rel}: original {sha256(original)}, derived {sha256(target)}, "
            + ("IntendedFor removed" if intended is None else f"IntendedFor = {intended}")
        )
    return lines


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)
    views_dir = Path(sys.argv[1]) / "oracles" / "bids-views"
    for name in VIEWS:
        lines = make(views_dir, name)
        (views_dir / f"{name}.DERIVED.txt").write_text(
            f"BIDS view {name}: the cached files of {VIEWS[name][0]} ({VIEWS[name][1]}) hard-linked; "
            "derived sidecars (larmorx validation/investigations/fmap-oracle-runs/derived_views.py):\n"
            + "\n".join(lines)
            + "\n",
            encoding="utf-8",
        )
        print(name, len(lines), "sidecars derived")


if __name__ == "__main__":
    main()
