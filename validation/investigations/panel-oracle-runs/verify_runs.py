# SPDX-License-Identifier: Apache-2.0
"""Check the data panel's fMRIPrep oracle runs after they end, and print each one's RUN_INFO.txt
as docs/validation/oracle-runs.md records it (machine paths as <workspace> and ~).

For every run: the exit status 0 in RUN_INFO.txt, "fMRIPrep finished successfully!" in its log,
no crash file, the HTML report, the preprocessed T1w with its brain mask, segmentation and both
transforms to and from MNI152NLin2009cAsym, the derived lesion mask's record for ds004884; and,
unless the run is anatomical only, for every BOLD run of its BIDS view the preprocessed series in
T1w and MNI space, the BOLD reference, the brain mask and the confounds.

    python -P verify_runs.py <workspace> [run name ...]   (default: the nine panel runs)
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

RUNS = {
    "ds000228-sub-pixar008-noreconall": ("ds000228", "pixar008", False),
    "ds005003-sub-RS001-noreconall": ("ds005003", "RS001", False),
    "ds004173-sub-603309-noreconall": ("ds004173", "603309", True),
    "ds002422-sub-02-noreconall": ("ds002422", "02", False),
    "ds000256-sub-CTS203-noreconall": ("ds000256", "CTS203", False),
    "ds004884-sub-M2079-noreconall": ("ds004884", "M2079", False),
    "ds003592-sub-280-noreconall": ("ds003592", "280", False),
    "ds002603-sub-01-noreconall": ("ds002603", "01", False),
    "ds004958-sub-fsm64ps-noreconall": ("ds004958", "fsm64ps", False),
}
ENTITY = re.compile(r"_(echo-\d+|part-[a-z]+)(?=_|$)")


def bold_stems(view: Path, label: str) -> list[str]:
    """The BOLD series fMRIPrep derives from the view: one per run (echoes and parts combined)."""
    stems = set()
    for f in (view / f"sub-{label}").rglob("*_bold.nii*"):
        stem = f.name.split("_bold.nii")[0]
        stems.add(ENTITY.sub("", stem))
    return sorted(stems)


def check(ws: Path, name: str) -> tuple[list[str], str]:
    ds, label, anat_only = RUNS[name]
    run = ws / "oracles" / "fmriprep-25.2.6" / name
    problems: list[str] = []
    info_path = run / "RUN_INFO.txt"
    if not info_path.is_file():
        return [f"{name}: no RUN_INFO.txt (not run)"], ""
    info = info_path.read_text(encoding="utf-8")
    if "exit status: 0" not in info:
        problems.append(f"{name}: exit status not 0 ({re.findall(r'exit status: .*', info)})")
    log = run / "fmriprep.log"
    if not log.is_file() or "fMRIPrep finished successfully!" not in log.read_text(
        encoding="utf-8", errors="replace"
    ):
        problems.append(f"{name}: the log does not say 'fMRIPrep finished successfully!'")
    out = run / "out"
    sub = out / f"sub-{label}"
    crashes = sorted(p.name for p in sub.rglob("crash-*")) if sub.is_dir() else []
    if crashes:
        problems.append(f"{name}: crash files {crashes[:3]}")
    if not (out / f"sub-{label}.html").is_file():
        problems.append(f"{name}: no HTML report")
    anat = list(sub.rglob("anat/*desc-preproc_T1w.nii.gz")) if sub.is_dir() else []
    if not anat:
        problems.append(f"{name}: no preprocessed T1w")
    for pattern in (
        "anat/*desc-brain_mask.nii.gz",
        "anat/*_dseg.nii.gz",
        "anat/*from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5",
        "anat/*from-MNI152NLin2009cAsym_to-T1w_mode-image_xfm.h5",
    ):
        if sub.is_dir() and not list(sub.rglob(pattern)):
            problems.append(f"{name}: no {pattern}")
    n_bold = 0
    if not anat_only:
        view = ws / "oracles" / "bids-views" / ds
        echoes = {
            ENTITY.sub("", f.name.split("_bold.nii")[0])
            for f in (view / f"sub-{label}").rglob("*_echo-*_bold.nii*")
        }
        for stem in bold_stems(view, label):
            base = Path(stem).name
            suffixes = [
                "_space-T1w_desc-preproc_bold.nii.gz",
                "_space-MNI152NLin2009cAsym_res-2_desc-preproc_bold.nii.gz",
                "_desc-confounds_timeseries.tsv",
                "_desc-brain_mask.nii.gz",
                "_desc-coreg_boldref.nii.gz",
            ]
            if stem in echoes:  # multi-echo: the T2* map in every output space
                suffixes += ["_space-T1w_T2starmap.nii.gz", "_space-boldref_T2starmap.nii.gz"]
            for suffix in suffixes:
                if not list(sub.rglob(f"func/{base}{suffix}")):
                    problems.append(f"{name}: no func/{base}{suffix}")
            n_bold += 1
    if ds == "ds004884":
        if "derived input:" not in info:
            problems.append(f"{name}: RUN_INFO.txt does not record the derived lesion mask")
        # sMRIPrep handed the mask to the template registration (its lesion_mask input)
        reports = (run / "work").glob(
            "fmriprep_*/sub_*/anat_fit_wf/register_template_wf/*/registration/_report/report.rst"
        )
        if not any("label-lesion_roi" in r.read_text(encoding="utf-8") for r in reports):
            problems.append(f"{name}: the template registration did not read the lesion mask")
    if "BIDS view:" not in info:
        problems.append(f"{name}: RUN_INFO.txt does not record its BIDS view")
    home = str(Path.home())
    text = info.replace(str(ws), "<workspace>").replace(home, "~")
    summary = f"{name}: {'ok' if not problems else 'PROBLEMS'}; {n_bold} BOLD series checked" + (
        " (anatomical only)" if anat_only else ""
    )
    print(summary)
    return problems, text


def main() -> int:
    ws = Path(sys.argv[1]).resolve()
    names = sys.argv[2:] or list(RUNS)
    all_problems, sections = [], []
    for name in names:
        problems, text = check(ws, name)
        all_problems += problems
        if text:
            sections.append(f"## {name}\n\n```text\n{text.rstrip()}\n```\n")
    for p in all_problems:
        print("PROBLEM", p)
    print(f"\n{len(names) - len({p.split(':')[0] for p in all_problems})} of {len(names)} runs ok")
    sys.stdout.write("\n" + "\n".join(sections))
    return 1 if all_problems else 0


if __name__ == "__main__":
    sys.exit(main())
