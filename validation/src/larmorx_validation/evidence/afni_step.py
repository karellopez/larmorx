# SPDX-License-Identifier: Apache-2.0
"""AFNI 26.3.05 beside 25.2.09 in the AFNI tools' evidence (the version update of 2026-10-29).

The evidence's cache entries were computed with AFNI 25.2.09. Each AFNI run of an example is run
once more with AFNI 26.3.05 (the replicas' reference since 2026-10-29), on the same arguments,
its outputs named after :func:`new_label` beside the old ones (:func:`larmorx_validation.figures.cache.add_runs`:
the old runs stay). The examples then show AFNI 26.3.05's output as a panel of its own, the
replica against it, and AFNI 26.3.05 against 25.2.09 (the version step, a reference row: AFNI
against AFNI), so a reader sees which version each panel ran and what changed.
"""

from __future__ import annotations

import json
from collections.abc import Sequence
from pathlib import Path

from larmorx_validation.figures import cache as fcache
from larmorx_validation.parity import afni_versions

NEW = afni_versions.NEWEST
OLD = afni_versions.PREVIOUS


def new_label(label: str) -> str:
    """The label (and output stem) of AFNI 26.3.05's run beside the AFNI 25.2.09 run ``label``:
    its first ``afni`` becomes ``afni26`` (``afni`` → ``afni26``, ``bold-afni`` →
    ``bold-afni26``, ``afni-1`` → ``afni26-1``), so a tag before it stays a prefix."""
    return label.replace("afni", "afni26", 1)


def _renamed(name: str, label: str) -> str:
    return new_label(label) + name[len(label) :]


#: The label of a panel or diff of AFNI 26.3.05 (``AFNI 26.3.05``) and of 25.2.09.
NEW_LABEL = f"AFNI {NEW}"
OLD_LABEL = f"AFNI {OLD}"
WHY = (
    "AFNI 26.3.05, the GPL replicas' reference since 2026-10-29 (the version update), on the same "
    "arguments as the AFNI 25.2.09 run beside it"
)


def _outputs(e: fcache.Entry, label: str, argv: Sequence[str]) -> list[str]:
    """The arguments of an AFNI run that name its outputs: files of the entry's directory whose
    name starts with the run's label (``afni.nii.gz``, ``afni-masked.nii.gz``)."""
    d = str(e.dir) + "/"
    return [
        a
        for a in argv
        if a.startswith(d)
        and (Path(a).name.startswith(label + ".") or Path(a).name.startswith(label + "-"))
    ]


def ensure(e: fcache.Entry, labels: Sequence[str], compute: bool = True) -> dict[str, dict]:
    """AFNI 26.3.05 on each AFNI run ``labels`` of the complete entry ``e`` (added once, as
    :func:`new_label`, its outputs renamed alike); for each label: ``{"outputs": {stem: (old path, new path)}, "same":
    {stem: None or how they differ}, "identical": bool}``."""
    info = e.info()
    runs = {r["label"]: r for r in info["runs"]}
    binary_dir = afni_versions.bin_dir(NEW)
    todo = []
    for label in labels:
        r = runs[label]
        if new_label(label) in runs:
            continue
        if binary_dir is None:
            raise FileNotFoundError(f"the AFNI {NEW} build (oracles/afni-{NEW})")
        if not compute:
            raise FileNotFoundError(
                f"{e.tool}/{e.case}: AFNI {NEW}'s run of {label} (--no-compute)"
            )
        outs = set(_outputs(e, label, r["argv"]))
        argv = [str(binary_dir / Path(r["argv"][0]).name)]
        for a in r["argv"][1:]:
            argv.append(str(Path(a).with_name(_renamed(Path(a).name, label))) if a in outs else a)
        todo.append((new_label(label), argv, dict(r["env"])))
    if todo:
        fcache.add_runs(e, todo, WHY)
        info = e.info()
        versions = info.setdefault("versions", {})
        versions["afni_26"] = afni_versions.version_of(str(binary_dir / "x"))
        e.info_path.write_text(json.dumps(info, indent=1) + "\n", encoding="utf-8")
    runs = {r["label"]: r for r in e.info()["runs"]}
    out = {}
    for label in labels:
        pairs = {}
        for a in _outputs(e, label, runs[label]["argv"]):
            new = Path(a).with_name(_renamed(Path(a).name, label))
            pairs[Path(a).name] = (Path(a), new)
        same = {k: afni_versions.compare_output(o, n) for k, (o, n) in pairs.items()}
        out[label] = {
            "outputs": pairs,
            "same": same,
            "identical": all(v is None for v in same.values()),
        }
    return out


def fact(step: dict[str, dict], what: str) -> dict:
    """The example's fact on the version step: what AFNI 26.3.05 gives on its calls."""
    differ = {k: v for s in step.values() for k, v in s["same"].items() if v is not None}
    if not differ:
        text = (
            f"{NEW_LABEL} (the GPL replica's reference since 2026-10-29) on the same arguments "
            f"as {OLD_LABEL}: the same bytes in every output of {what}. What changed between "
            "the two releases in these programs (header slice code 2, AFNI-03; `-no_detrend`, "
            "AFNI-05; both corrected by LARMORX before AFNI fixed them) does not meet this call."
        )
    else:
        parts = "; ".join(f"`{k}`: {v}" for k, v in sorted(differ.items()))
        text = f"{NEW_LABEL} against {OLD_LABEL} on the same arguments ({what}): {parts}."
    return {"label": "AFNI versions", "text": text}
