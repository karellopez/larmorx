# SPDX-License-Identifier: Apache-2.0
"""The estimators of a BIDS dataset: larmorx's ports against pybids, sdcflows and fMRIPrep.

Compares, value for value (types, key order, messages):

- ``index``: :class:`larmorx.bids.BIDSLayout` against pybids 0.22.0's ``BIDSLayout`` on every
  dataset (the fMRIPrep oracle runs' BIDS inputs, sdcflows' test datasets ``dsA``/``dsB``/``dsC``,
  sdcflows' and niworkflows' generated skeletons, and edge-case trees made here), with
  fMRIPrep's, sdcflows' BIDS app's, sdcflows' tests' and pybids' default indexers: every file's
  entities and metadata, the layout's entities, the ``get_<entity>s`` values, a battery of
  queries (``estimators_dump.QUERIES``) and the indexing errors.
- ``wrangler``: :func:`larmorx.sdcflows.find_estimators` against sdcflows 2.17.1's
  (``upstream_defects="reproduce"``; every layout call sdcflows made replayed on larmorx's
  layout too) and against sdcflows 2.15.2's ``utils/wrangler.py`` (the default, ``"fix"``):
  the estimators, the registries, the debug log, the errors, over argument sets from sdcflows'
  tests; the direct ``FieldmapFile``/``FieldmapEstimation``/``bidict`` cases.
- ``fmriprep``: fMRIPrep 25.2.7's ``map_fieldmap_estimation`` and ``get_estimator`` (taken from
  its ``workflows/base.py``) against larmorprepx's, for every combination of ``--use-syn-sdc``,
  ``--force syn-sdc`` and ``--ignore fieldmaps``; ``collect_fieldmaps`` on sdcflows' own runs'
  derivatives; and the oracle runs' own ``fmriprep.log``: the estimators larmorx sets up for
  each run (its options from ``RUN_INFO.txt``) against the run's "Setting up fieldmap",
  "Several fieldmaps" and "estimated with the following" lines.
- ``apps``: the estimators of sdcflows 2.17.1's own BIDS-app runs (``--no-fmapless``) against
  the ``fmapid`` labels of their derivatives.
- ``roots``: datasets below folders whose names hold pybids' entity patterns (SDC-45, PIPE-37;
  :func:`section_roots`): both modes against pybids, sdcflows 2.17.1 and 2.15.2.
- ``image``: pybids 0.20.0, sdcflows 2.15.1 and fMRIPrep 25.2.6 in fMRIPrep's image
  (``relay11/estimators_dump.py`` run there) against larmorx with ``pybids_version="0.20.0"``,
  ``sdcflows_version="2.15"`` and ``upstream_defects="reproduce"``.

Usage: ``LARMORX_WORKSPACE=<workspace> python -P check_estimators.py [--sections ...]
[--scratch <folder>]`` (seconds to a few minutes; the image section runs Apptainer).
"""

from __future__ import annotations

import argparse
import ast
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import warnings
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE / "relay11"))
import estimators_dump as dump  # noqa: E402

W = Path(os.environ.get("LARMORX_WORKSPACE", HERE.parents[4]))
TAGS = W / "tags"
ORACLES = W / "oracles" / "fmriprep-25.2.6"
SDC_RUNS = W / "oracles" / "sdcflows-2.17.1"
IMAGE = W / "oracles" / "images" / "fmriprep-25.2.6.sif"

FAILS: list[str] = []
EXPECTED: list[str] = []
COUNTS = {"ok": 0}


def report(name: str, a: Any, b: Any) -> None:
    if a == b:
        COUNTS["ok"] += 1
        return
    FAILS.append(name)
    ja, jb = json.dumps(a, indent=0).splitlines(), json.dumps(b, indent=0).splitlines()
    import difflib

    diff = list(difflib.unified_diff(ja, jb, "original", "larmorx", n=2, lineterm=""))
    print(f"DIFF {name}:\n  " + "\n  ".join(diff[:40]))


# --- datasets -------------------------------------------------------------------------------


def oracle_datasets() -> list[tuple[str, Path, str, dict[str, Any]]]:
    """(name, BIDS root, participant, RUN_INFO options) of every fMRIPrep oracle run."""
    out = []
    for run in sorted(ORACLES.iterdir()):
        info = run / "RUN_INFO.txt"
        if not info.is_file():
            continue
        text = info.read_text(encoding="utf-8")
        if not re.search(r"^exit status: 0$", text, re.M):
            print(f"skipped (not finished): {run.name}")
            continue
        root = Path(re.search(r"^BIDS input: (\S+)", text, re.M).group(1))  # type: ignore[union-attr]
        sub = re.search(r"^participant: (\S+)", text, re.M).group(1)  # type: ignore[union-attr]
        args = re.search(r"^arguments: (.*)$", text, re.M).group(1)  # type: ignore[union-attr]
        opts = {
            "use_syn": "error" if "--use-syn-sdc" in args else False,
            "force_syn": "syn-sdc" in re.findall(r"--force ([\w-]+)", args),
            "ignore_fieldmaps": "fieldmaps" in re.findall(r"--ignore ([\w-]+)", args),
        }
        out.append((run.name, root, sub, opts))
    return out


def _wrangler_specs() -> dict[str, Any]:
    specs = {}
    for version in ("2.17.1", "2.15.2"):
        src = TAGS / f"sdcflows-{version}" / "sdcflows" / "utils" / "tests" / "test_wrangler.py"
        for node in ast.parse(src.read_text(encoding="utf-8")).body:
            if isinstance(node, ast.Assign) and isinstance(node.targets[0], ast.Name):
                name = node.targets[0].id
                if name in ("pepolar", "pepolar_b0ids", "phasediff"):
                    specs[name] = ast.literal_eval(node.value)
    return specs


def _nifti(path: Path, shape: tuple[int, ...], affine: Any = None) -> None:
    import nibabel as nb
    import numpy as np

    path.parent.mkdir(parents=True, exist_ok=True)
    nb.Nifti1Image(np.zeros(shape, np.int16), np.eye(4) if affine is None else affine).to_filename(
        path
    )


def _json(path: Path, data: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data), encoding="utf-8")


def make_trees(scratch: Path) -> list[tuple[str, Path, list[str]]]:
    """sdcflows' test skeletons and edge-case trees: (name, root, subjects)."""
    from niworkflows.utils.testing import generate_bids_skeleton

    trees = []
    T1w = {"suffix": "T1w"}
    bold = {
        "task": "rest",
        "suffix": "bold",
        "metadata": {"RepetitionTime": 0.8, "TotalReadoutTime": 0.5, "PhaseEncodingDirection": "j"},
    }
    me = [{"EchoTime": 0.01 * i, **bold["metadata"]} for i in range(1, 4)]
    sbref = {**bold, "suffix": "sbref"}
    epi = {"suffix": "epi", "ce": "gad", "metadata": {"TotalReadoutTime": 0.05}}
    skeletons = dict(_wrangler_specs())
    skeletons.update(
        {
            "fmapless1": {"01": {"anat": [T1w], "func": [bold]}},
            "fmapless_runs": {"01": {"anat": [T1w], "func": [{"run": i, **bold} for i in (1, 2)]}},
            "fmapless_me": {
                "01": {
                    "anat": [T1w],
                    "func": [{"echo": i + 1, **bold, "metadata": me[i]} for i in range(3)],
                }
            },
            "fmapless_sbref": {"01": {"anat": [T1w], "func": [bold, sbref]}},
            "fmapless_acq": {
                "01": {"anat": [T1w], "func": [{"acq": "A", **bold}, {"acq": "B", **sbref}]}
            },
            "fmapless_me_sbref": {
                "01": {
                    "anat": [T1w],
                    "func": [{"echo": i + 1, **bold, "metadata": me[i]} for i in range(3)]
                    + [{"echo": i + 1, **sbref, "metadata": me[i]} for i in range(3)],
                }
            },
            "sessionless_t1w": {"01": [{"anat": [T1w]}, {"session": "01", "func": [bold]}]},
            "filters": {
                "01": {
                    "anat": [{"suffix": "T1w"}],
                    "fmap": [
                        {
                            **epi,
                            "acq": acq,
                            "dir": d,
                            "metadata": {**epi["metadata"], "PhaseEncodingDirection": pe},
                        }
                        for acq in ("A", "B")
                        for d, pe in (("AP", "j"), ("PA", "j-"))
                    ],
                    "func": [bold],
                }
            },
        }
    )
    for name, spec in skeletons.items():
        root = scratch / "trees" / name
        if root.exists():
            shutil.rmtree(root)
        generate_bids_skeleton(str(root), spec)
        trees.append((name, root, ["01"]))
    trees.append(("sdc21", _tree_sdc21(scratch / "trees" / "sdc21"), ["01", "010"]))
    trees.append(("edges", _tree_edges(scratch / "trees" / "edges"), ["01"]))
    for name in ("conflict", "badjson"):
        trees.append((name, _tree_error(scratch / "trees" / name, name), ["01"]))
    return trees


def _tree_sdc21(root: Path) -> Path:
    """Two subjects 01 and 010 whose EPI pairs list the same identifier, and an upper-case
    and a part-imag variant: SDC-21."""
    if root.exists():
        shutil.rmtree(root)
    _json(root / "dataset_description.json", {"Name": "sdc21", "BIDSVersion": "1.8.0"})
    for sub in ("01", "010"):
        base = root / f"sub-{sub}"
        _nifti(base / "anat" / f"sub-{sub}_T1w.nii.gz", (4, 4, 4))
        for d, pe in (("AP", "j-"), ("PA", "j")):
            f = base / "fmap" / f"sub-{sub}_dir-{d}_epi.nii.gz"
            _nifti(f, (8, 8, 4))
            _json(f.with_name(f.name.replace(".nii.gz", ".json")), {
                "PhaseEncodingDirection": pe, "TotalReadoutTime": 0.05, "B0FieldIdentifier": ["pe", f"x{sub}"],
            })  # fmt: skip
        b = base / "func" / f"sub-{sub}_task-rest_bold.nii.gz"
        _nifti(b, (8, 8, 4, 2))
        _json(b.with_name(b.name.replace(".nii.gz", ".json")), {
            "PhaseEncodingDirection": "j", "TotalReadoutTime": 0.05, "RepetitionTime": 2.0, "B0FieldSource": "pe",
        })  # fmt: skip
    for name, meta in (
        ("sub-01_acq-up_dir-LR_epi", {"PhaseEncodingDirection": "i", "B0FieldIdentifier": ["PE"]}),
        ("sub-01_acq-up_dir-RL_epi", {"PhaseEncodingDirection": "i-", "B0FieldIdentifier": ["PE"]}),
        (
            "sub-01_acq-im_dir-AP_part-imag_epi",
            {"PhaseEncodingDirection": "j-", "B0FieldIdentifier": ["pe"]},
        ),
    ):
        f = root / "sub-01" / "fmap" / f"{name}.nii.gz"
        _nifti(f, (8, 8, 4))
        _json(f.with_name(name + ".json"), {"TotalReadoutTime": 0.05, **meta})
    return root


def _tree_edges(root: Path) -> Path:
    """Inheritance, URIs, links, ignored folders, real headers for the readout time."""
    if root.exists():
        shutil.rmtree(root)
    _json(root / "dataset_description.json", {"Name": "edges", "BIDSVersion": "1.8.0"})
    _json(
        root / "task-rest_bold.json",
        {"PhaseEncodingDirection": "j-", "RepetitionTime": 2, "Zeta": 1},
    )
    s = root / "sub-01"
    for ses in ("pre", "post"):
        _nifti(s / f"ses-{ses}" / "anat" / f"sub-01_ses-{ses}_T1w.nii.gz", (4, 4, 4))
        _nifti(s / f"ses-{ses}" / "anat" / f"sub-01_ses-{ses}_T2w.nii.gz", (4, 4, 4))
        func = s / f"ses-{ses}" / "func"
        for run in ("1", "01", "2"):
            if run == "01" and ses == "pre":
                continue
            b = func / f"sub-01_ses-{ses}_task-rest_run-{run}_bold.nii.gz"
            _nifti(b, (10, 12, 4, 3))
            _json(
                b.with_name(b.name.replace(".nii.gz", ".json")),
                {"EffectiveEchoSpacing": 0.0005, "Zeta": 2.5, "Alpha": [1, 2]},
            )
        _json(
            func / f"sub-01_ses-{ses}_task-rest_bold.json",
            {"Alpha": 7, "mid": "m", "nul": None, "B0FieldSource": f"pd{ses}"},
        )
        sb = func / f"sub-01_ses-{ses}_task-rest_run-2_sbref.nii.gz"
        _nifti(sb, (10, 12, 4))
        _json(
            sb.with_name(sb.name.replace(".nii.gz", ".json")),
            {"PhaseEncodingDirection": "j", "EstimatedTotalReadoutTime": 0.03},
        )
        fm = s / f"ses-{ses}" / "fmap"
        if ses == "pre":
            for ph, te in (("phase1", 0.004), ("phase2", 0.006)):
                _nifti(fm / f"sub-01_ses-pre_{ph}.nii.gz", (8, 8, 4))
                _json(
                    fm / f"sub-01_ses-pre_{ph}.json",
                    {
                        "EchoTime": te,
                        "IntendedFor": [
                            "bids::sub-01/ses-pre/func/sub-01_ses-pre_task-rest_run-1_bold.nii.gz",
                            "bids:other:sub-01/x.nii.gz",
                        ],
                    },
                )
            for mag in ("magnitude1", "magnitude2"):
                _nifti(fm / f"sub-01_ses-pre_{mag}.nii.gz", (8, 8, 4))
            _nifti(fm / "sub-01_ses-pre_acq-dir_fieldmap.nii.gz", (8, 8, 4))
            _json(
                fm / "sub-01_ses-pre_acq-dir_fieldmap.json",
                {"Units": "Hz", "B0FieldIdentifier": "pdpre"},
            )
            _nifti(fm / "sub-01_ses-pre_acq-dir_magnitude.nii.gz", (8, 8, 4))
            _json(fm / "sub-01_ses-pre_acq-dir_magnitude.json", {"B0FieldIdentifier": "pdpre"})
        else:
            for d, pe in (("AP", "j-"), ("PA", "j")):
                f = fm / f"sub-01_ses-post_dir-{d}_epi.nii.gz"
                _nifti(f, (10, 12, 4))
                _json(
                    f.with_name(f.name.replace(".nii.gz", ".json")),
                    {
                        "PhaseEncodingDirection": pe,
                        "EffectiveEchoSpacing": 0.0006,
                        "IntendedFor": "ses-post/func/sub-01_ses-post_task-rest_run-01_bold.nii.gz",
                    },
                )
            f = fm / "sub-01_ses-post_acq-single_epi.nii.gz"
            _nifti(f, (10, 12, 4))
            _json(
                f.with_name("sub-01_ses-post_acq-single_epi.json"),
                {
                    "PhaseEncodingDirection": "j-",
                    "TotalReadoutTime": 0.04,
                    "IntendedFor": [
                        "ses-post/func/sub-01_ses-post_task-rest_run-2_sbref.nii.gz",
                        "ses-post/func/sub-01_ses-post_task-rest_run-2_bold.nii.gz",
                        "ses-post/func/missing_bold.nii.gz",
                    ],
                },
            )
    (s / "ses-post" / "dwi").mkdir(parents=True)
    _nifti(s / "ses-post" / "dwi" / "sub-01_ses-post_dwi.nii.gz", (4, 4, 4, 2))
    _json(
        s / "ses-post" / "dwi" / "sub-01_ses-post_dwi.json",
        {"PhaseEncodingDirection": "j", "TotalReadoutTime": 0.05},
    )
    for hidden in (
        root / ".git" / "x.json",
        s / ".hidden.nii.gz",
        root / "code" / "a.py",
        root / "sourcedata" / "sub-01_T1w.nii.gz",
        root / "codebook.tsv",
    ):
        hidden.parent.mkdir(parents=True, exist_ok=True)
        hidden.write_text("", encoding="utf-8")
    (root / "derivatives" / "pipe").mkdir(parents=True)
    _json(
        root / "derivatives" / "pipe" / "dataset_description.json",
        {
            "Name": "p",
            "BIDSVersion": "1.8.0",
            "DatasetType": "derivative",
            "GeneratedBy": [{"Name": "pipe"}],
        },
    )
    _nifti(
        root
        / "derivatives"
        / "pipe"
        / "sub-01"
        / "fmap"
        / "sub-01_fmapid-auto00000_desc-preproc_fieldmap.nii.gz",
        (4, 4, 4),
    )
    _json(root / "derivatives" / "top.json", {"a": 1})
    os.symlink(
        s / "ses-pre" / "anat" / "sub-01_ses-pre_T1w.nii.gz",
        s / "ses-pre" / "anat" / "sub-01_ses-pre_acq-link_T1w.nii.gz",
    )
    os.symlink(
        root / "nothing.nii.gz",
        s / "ses-post" / "fmap" / "sub-01_ses-post_acq-broken_dir-AP_epi.nii.gz",
    )
    os.symlink(s / "ses-pre" / "anat", s / "ses-pre" / "anatlink")
    return root


def _tree_error(root: Path, kind: str) -> Path:
    if root.exists():
        shutil.rmtree(root)
    _json(root / "dataset_description.json", {"Name": kind, "BIDSVersion": "1.8.0"})
    b = root / "sub-01" / "func" / "sub-01_task-rest_run-1_bold.nii.gz"
    _nifti(b, (4, 4, 4, 2))
    if kind == "conflict":
        _json(
            b.with_name("sub-01_task-rest_run-1_bold.json"),
            {"run": 2, "PhaseEncodingDirection": "j"},
        )
    else:
        b.with_name("sub-01_task-rest_run-1_bold.json").write_text("{bad json", encoding="utf-8")
    return root


# --- layouts --------------------------------------------------------------------------------


def indexers(subject: str) -> dict[str, dict[str, Any]]:
    """The ignore patterns of fMRIPrep, sdcflows' app, sdcflows' tests; pybids' default."""
    return {
        "fmriprep": {"ignore": [
            "code", "stimuli", "sourcedata", "models", re.compile(r"^\."),
            re.compile(r"sub-[a-zA-Z0-9]+(/ses-[a-zA-Z0-9]+)?/(beh|dwi|eeg|ieeg|meg|perf)"),
            re.compile(r"sub-(?!(" + subject + r")(\b|_))"),
        ]},
        "sdcflows_app": {"ignore": [
            re.compile(r"^(?!/sub-(" + subject + "))"),
            re.compile(r"^/sub-[a-zA-Z0-9]+(/ses-[a-zA-Z0-9]+)?/(beh|pet|perf|meg|eeg|ieeg|micr|nirs)"),
        ]},
        "sdcflows_tests": {"ignore": (
            "code", "stimuli", "sourcedata", "models", "derivatives", re.compile(r"^\."),
            re.compile(r"sub-[a-zA-Z0-9]+(/ses-[a-zA-Z0-9]+)?/(beh|eeg|ieeg|meg|micr|perf)"),
        )},
        "default": {},
    }  # fmt: skip


def make_layout(side: str, root: Path, which: str, subject: str, **kw: Any) -> Any:
    if side == "pybids":
        from bids.layout import BIDSLayout, BIDSLayoutIndexer
    else:
        from larmorx.bids import BIDSLayout, BIDSLayoutIndexer
    spec = indexers(subject)[which]
    if which == "default":
        return BIDSLayout(str(root), validate=False, **kw)
    return BIDSLayout(str(root), indexer=BIDSLayoutIndexer(validate=False, **spec), **kw)


def both(root: Path, which: str, subject: str, ser: dump.Ser, **kw: Any) -> tuple[Any, Any]:
    out = []
    for side in ("pybids", "larmorx"):
        try:
            with warnings.catch_warnings(record=True) as caught:
                warnings.simplefilter("always")
                lay = make_layout(side, root, which, subject, **kw)
            out.append(
                (lay, [ser.s(str(w.message)) for w in caught if "Skipping" in str(w.message)])
            )
        except Exception as err:
            out.append((None, ser.error(err)))
    return out[0], out[1]


# --- sections -------------------------------------------------------------------------------


def all_datasets(scratch: Path) -> list[tuple[str, Path, list[str]]]:
    seen: dict[Path, tuple[str, Path, list[str]]] = {}
    for _name, root, sub, _ in oracle_datasets():
        seen.setdefault(root, (root.name, root, [sub]))
    data = TAGS / "sdcflows-2.17.1" / "sdcflows" / "tests" / "data"
    for ds in ("dsA", "dsB", "dsC"):
        seen.setdefault(data / ds, (ds, data / ds, ["01"]))
    for t in make_trees(scratch):
        seen.setdefault(t[1], t)
    return list(seen.values())


def section_index(datasets: list[tuple[str, Path, list[str]]]) -> None:
    from bids.layout import Query as PQ
    from bids.layout import parse_file_entities as pparse

    import larmorx.bids as lb

    for name, root, subjects in datasets:
        ser = dump.Ser({str(root): "<root>"})
        for which in ("fmriprep", "sdcflows_app", "sdcflows_tests", "default"):
            for sub in subjects:
                kws = [{}] + (
                    [{"derivatives": True}] if name == "edges" and which == "default" else []
                )
                for kw in kws:
                    (p, pw), (lay, lw) = both(root, which, sub, ser, **kw)
                    tag = f"index {name} {which} sub-{sub} {kw}"
                    if p is None or lay is None:
                        report(
                            tag + " error",
                            pw if p is None else "built",
                            lw if lay is None else "built",
                        )
                        continue
                    report(tag + " warnings", pw, lw)
                    a = dump.dump_index(p, ser, PQ, pparse)
                    b = dump.dump_index(lay, ser, lb.Query, lb.parse_file_entities)
                    if kw:  # pybids orders layouts and configurations by a set of objects
                        for d in (a, b):
                            d["entities"] = sorted(d["entities"])
                            for f in d["files"].values():
                                if f["parse"][0] == "dict":
                                    f["parse"][2].sort()
                    report(tag, a, b)


def _replay(lay: Any, calls: list[Any], root: Path, ser: dump.Ser) -> list[Any]:
    import larmorx.bids as lb

    def dec(v: Any) -> Any:
        if isinstance(v, str) and v.startswith("@"):
            return getattr(lb.Query, v[1:])
        if isinstance(v, str):
            return v.replace("<root>", str(root))
        if isinstance(v, list):
            return [dec(x) for x in v]
        if isinstance(v, dict) and "@file" in v:
            return lay.get_file(v["@file"].replace("<root>", str(root)))
        if isinstance(v, dict) and "@path" in v:
            return Path(v["@path"].replace("<root>", str(root)))
        if isinstance(v, dict) and "@padded" in v:
            return lb.PaddedInt(v["@padded"])
        return v

    out = []
    for name, args, kwargs, _result in calls:
        try:
            res = ser(
                getattr(lay, name)(
                    *[dec(a) for a in args], **{k: dec(v) for k, v in kwargs.items()}
                )
            )
        except Exception as err:
            res = ser.error(err)
        out.append([name, args, kwargs, res])
    return out


def wrangler_cases(lay: Any, subject: str) -> list[dict[str, Any]]:
    cases: list[dict[str, Any]] = [
        {}, {"fmapless": False}, {"force_fmapless": True}, {"fmapless": ["bold"], "force_fmapless": True},
        {"anat_suffix": "T2w", "force_fmapless": True}, {"anat_suffix": ["T1w", "T2w"], "force_fmapless": True},
        {"bids_filters": {"datatype": "fmap"}, "force_fmapless": True},
        {"bids_filters": {"datatype": "fmap", "run": "9999"}, "force_fmapless": True},
        {"bids_filters": {"ceagent": "gad"}, "force_fmapless": True},
        {"bids_filters": {"suffix": "bold"}},
    ]  # fmt: skip
    for acq in lay.get_acquisitions(subject=subject)[:2]:
        cases.append({"bids_filters": {"acquisition": acq}, "force_fmapless": True})
    for ses in lay.get_sessions(subject=subject)[:3]:
        cases.append({"sessions": [ses], "force_fmapless": True})
        cases.append({"bids_filters": {"session": ses, "datatype": "fmap"}})
        cases.append({"bids_filters": {"session": ses}, "sessions": [ses]})
    return [{"subject": subject, **c} for c in cases]


def _load_wrangler_2152() -> Any:
    import importlib.util

    import sdcflows.utils

    path = TAGS / "sdcflows-2.15.2" / "sdcflows" / "utils" / "wrangler.py"
    spec = importlib.util.spec_from_file_location("sdcflows.utils.wrangler_2152", path)
    mod = importlib.util.module_from_spec(spec)  # type: ignore[arg-type]
    mod.__package__ = "sdcflows.utils"
    spec.loader.exec_module(mod)  # type: ignore[union-attr]
    assert sdcflows.utils
    return mod


def section_wrangler(datasets: list[tuple[str, Path, list[str]]]) -> None:
    import sdcflows.utils.wrangler as w2171
    from bids.layout import Query as PQ
    from sdcflows import fieldmaps as sfm
    from sdcflows.utils.wrangler import find_estimators as find_2171

    import larmorx.sdcflows as ls

    w2152 = _load_wrangler_2152()
    find_2152 = w2152.find_estimators
    for mod in (w2171, w2152):
        mod.reduce = dump.natural_reduce  # SDC-19: sdcflows' set order fixed to pybids' natural one
    for name, root, subjects in datasets:
        ser = dump.Ser({str(root): "<root>"})
        for which in ("fmriprep", "sdcflows_tests"):
            p = l_ = None
            for sub in subjects:
                (p, _), (l_, _) = both(root, which, sub, ser)
                if p is None or l_ is None:
                    continue
                cases = wrangler_cases(p, sub)

                def pstate(ser: dump.Ser = ser) -> dict[str, Any]:
                    return dump.registry_state(sfm._estimators, sfm._intents, ser)

                def lstate(ser: dump.Ser = ser) -> dict[str, Any]:
                    return dump.registry_state(ls.REGISTRY.estimators, ls.REGISTRY.intents, ser)

                for label, find, mode in (
                    ("2.17.1", find_2171, "reproduce"),
                    ("2.15.2", find_2152, "fix"),
                ):
                    a = dump.dump_wrangler(
                        find, sfm.clear_registry, pstate, p, ser, cases, record=True, query_cls=PQ
                    )

                    warned: list[list[str]] = []

                    def lfind(mode: str = mode, warned: list[list[str]] = warned, **kw: Any) -> Any:
                        with warnings.catch_warnings(record=True) as caught:
                            warnings.simplefilter("always")
                            try:
                                return ls.find_estimators(upstream_defects=mode, **kw)
                            finally:
                                warned.append([getattr(w.message, "entry", "") for w in caught])

                    b = dump.dump_wrangler(lfind, ls.clear_registry, lstate, l_, ser, cases)
                    for x, y, wn in zip(a, b, warned, strict=True):
                        tag = f"wrangler {name} {which} {label} {json.dumps(x['case'])[:120]}"
                        calls = x.pop("calls")
                        if mode == "fix" and "SDC-21" in wn:
                            EXPECTED.append(tag + " (SDC-21 corrected)")
                            continue
                        report(tag, x, y)
                        if mode == "reproduce":
                            report(tag + " calls", calls, _replay(l_, calls, root, ser))
                # fMRIPrep's process: one registry for every call (the auto_ numbering goes on)
                sfm.clear_registry()
                ls.clear_registry()
                seq = cases[:4]
                a = dump.dump_wrangler(
                    find_2171, sfm.clear_registry, pstate, p, ser, seq, keep_registry=True
                )
                b = dump.dump_wrangler(
                    lambda **kw: ls.find_estimators(upstream_defects="reproduce", **kw),
                    ls.clear_registry, lstate, l_, ser, seq, keep_registry=True,
                )  # fmt: skip
                report(f"wrangler {name} {which} sub-{sub} one registry", a, b)


def section_units(scratch: Path) -> None:
    """FieldmapFile, FieldmapEstimation, bidict and the identifiers called directly."""
    from sdcflows import fieldmaps as sfm
    from sdcflows.utils import bimap

    import larmorx.sdcflows as ls

    data = TAGS / "sdcflows-2.17.1" / "sdcflows" / "tests" / "data" / "dsA" / "sub-01"
    edges = scratch / "trees" / "edges" / "sub-01"
    ser = dump.Ser({str(data.parent): "<dsA>", str(edges.parent): "<edges>"})
    files = sorted(data.rglob("*.nii*")) + sorted(edges.rglob("*.nii*"))
    files += [
        data / "fmap" / "nothing.nii.gz",
        data / "fmap" / "sub-01_T1w.txt",
        data / "anat" / "T1w.nii.gz",
    ]
    metas = [None, {}, {"TotalReadoutTime": None}, {"PhaseEncodingDirection": "i", "EstimatedTotalReadoutTime": 0.05},
             {"PhaseEncodingDirection": "j", "EffectiveEchoSpacing": 0.0005}]  # fmt: skip
    for f in files:
        for md in metas:
            for find_meta in (True, False):
                kw = {"find_meta": find_meta} | ({} if md is None else {"metadata": dict(md)})
                a = dump._call(
                    ser,
                    sfm.FieldmapFile,
                    f,
                    **dict(kw, metadata=dict(md)) if md is not None else kw,
                )
                b = dump._call(
                    ser, ls.FieldmapFile, f, **dict(kw, metadata=dict(md)) if md is not None else kw
                )
                report(f"units FieldmapFile {ser.s(str(f))} {md} {find_meta}", a, b)
    groups = [
        ["fmap/sub-01_phasediff.nii.gz"], ["fmap/sub-01_phase1.nii.gz"], ["fmap/sub-01_phase2.nii.gz"],
        ["fmap/sub-01_fieldmap.nii.gz"], ["fmap/sub-01_dir-LR_epi.nii.gz", "fmap/sub-01_dir-RL_epi.nii.gz"],
        ["fmap/sub-01_dir-LR_epi.nii.gz", "fmap/sub-01_dir-LR_epi.nii.gz"], ["fmap/sub-01_phasediff.nii.gz", "fmap/sub-01_fieldmap.nii.gz"],
        ["anat/sub-01_T1w.nii.gz"], ["anat/sub-01_T1w.nii.gz", "fmap/sub-01_dir-LR_epi.nii.gz"],
        ["fmap/sub-01_magnitude1.nii.gz"], ["fmap/sub-01_phasediff.nii.gz", "fmap/sub-01_dir-LR_epi.nii.gz", "fmap/sub-01_dir-RL_epi.nii.gz"],
    ]  # fmt: skip
    sfm.clear_registry()
    ls.clear_registry()
    for g in groups:
        for bids_id in (None, "my-id.1"):
            a = dump._call(ser, sfm.FieldmapEstimation, [data / x for x in g], bids_id=bids_id)
            b = dump._call(ser, ls.FieldmapEstimation, [data / x for x in g], bids_id=bids_id)
            report(f"units FieldmapEstimation {g} {bids_id}", a, b)
            report(f"units registry after {g} {bids_id}", dump.registry_state(sfm._estimators, sfm._intents, ser),
                   dump.registry_state(ls.REGISTRY.estimators, ls.REGISTRY.intents, ser))  # fmt: skip
    for fname in (
        "func/sub-01_task-rest_bold.nii.gz",
        "nothing",
        str(data / "fmap" / "sub-01_phasediff.nii.gz"),
    ):
        for by in ("IntendedFor", "sources", "bogus"):
            a = dump._call(ser, sfm.get_identifier, fname, by)
            b = dump._call(ser, ls.get_identifier, fname, by, upstream_defects="reproduce")
            report(f"units get_identifier {fname} {by}", a, b)
    ops = [("set", "a", 1), ("set", "b", 2), ("set", "a", 3), ("set", "c", 1), ("set", "d", "d"), ("set", "e", []),
           ("set", "1", "x"), ("add", "v1"), ("set", "auto_00001", "w"), ("add", "v2"), ("add", "v1"), ("set", "auto_00007", "z"),
           ("add", "v3"), ("add", "v4"), ("del", "a"), ("del", "x"), ("del", "nope"), ("get", "b"), ("get", 2), ("get", "zz"),
           ("contains", "w"), ("clear",)]  # fmt: skip
    res = {}
    for side, cls in (("orig", bimap.bidict), ("port", ls.bidict)):
        d = cls({"k": "v"})
        out = []
        for op in ops:
            try:
                if op[0] == "set":
                    d[op[1]] = op[2]
                    r: Any = None
                elif op[0] == "add":
                    r = d.add(op[1])
                elif op[0] == "del":
                    del d[op[1]]
                    r = None
                elif op[0] == "get":
                    r = d[op[1]]
                elif op[0] == "contains":
                    r = op[1] in d
                else:
                    d.clear()
                    r = None
                out.append([ser(r), ser(dict(d))])
            except Exception as err:
                out.append(ser.error(err))
        res[side] = out
    report("units bidict", res["orig"], res["port"])
    for cls in (bimap.bidict, ls.bidict):
        try:
            cls({"a": 1, "b": 1})
        except TypeError as err:
            res[cls.__module__] = str(err)
    # SDC-21 corrected: exact subjects, parts and identifiers (both subjects indexed)
    root = scratch / "trees" / "sdc21"
    lay = make_layout("larmorx", root, "sdcflows_tests", "01")
    got = {}
    for mode in ("fix", "reproduce"):
        ls.clear_registry()
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            ests = ls.find_estimators(
                layout=lay, subject="01", fmapless=False, upstream_defects=mode
            )
        got[mode] = sorted(
            (e.bids_id, sorted(Path(p).relative_to(root).as_posix() for p in e.paths()))
            for e in ests
        )
    report("units SDC-21 corrected", [
        ("PE", ["sub-01/fmap/sub-01_acq-up_dir-LR_epi.nii.gz", "sub-01/fmap/sub-01_acq-up_dir-RL_epi.nii.gz"]),
        ("pe", ["sub-01/fmap/sub-01_dir-AP_epi.nii.gz", "sub-01/fmap/sub-01_dir-PA_epi.nii.gz"]),
    ], got["fix"])  # fmt: skip
    report(
        "units SDC-21 reproduced takes sub-010's files",
        True,
        any("sub-010" in p for _, ps in got["reproduce"] for p in ps),
    )
    reg_a, reg_b = bimap.EstimatorRegistry(), ls.EstimatorRegistry()
    for r in (reg_a, reg_b):
        r.add(("file3.txt", "file4.txt"))
        r.add(("file1.txt", "file2.txt"))
        r.add(("file3.txt", "file2.txt"))
    report("units EstimatorRegistry", [reg_a.sources, reg_a.get_key("file3.txt"), reg_a.get_key("x")],
           [reg_b.sources, reg_b.get_key("file3.txt"), reg_b.get_key("x")])  # fmt: skip


def _bold_data(lay: Any, subject: str) -> list[list[str]]:
    files = lay.get(
        subject=subject,
        suffix="bold",
        extension=[".nii", ".nii.gz"],
        part=["mag", None],
        datatype="func",
    )
    series: dict[str, list[str]] = {}
    for f in files:
        key = re.sub(r"_echo-\d+", "", f.path)
        series.setdefault(key, []).append(f.path)
    return list(series.values())


COMBOS = [
    {"ignore_fieldmaps": i, "use_syn": u, "force_syn": f, "filters": flt}
    for i in (False, True) for u in (False, "error", "warn") for f in (False, True) for flt in (None, {"datatype": "fmap"})
]  # fmt: skip


def section_fmriprep(datasets: list[tuple[str, Path, list[str]]]) -> None:
    import sdcflows.utils.wrangler as w2171
    from sdcflows import fieldmaps as sfm

    import larmorx.sdcflows as ls
    from larmorx.pipelines.larmorprepx import estimators as le

    w2171.reduce = dump.natural_reduce  # SDC-19: sdcflows' set order fixed to pybids' natural one
    logger, cap = dump.capture_logger("check.fmriprep")
    fns = dump.fmriprep_functions(
        str(TAGS / "fmriprep-25.2.7" / "fmriprep" / "workflows" / "base.py"), logger
    )
    llogger, lcap = dump.capture_logger("check.larmorprepx")
    for name, root, subjects in datasets:
        ser = dump.Ser({str(root): "<root>"})
        for sub in subjects:
            (p, _), (_, _) = both(root, "fmriprep", sub, ser)
            if p is None:
                continue
            lay = le.fmriprep_layout(root, [sub])
            bold = _bold_data(lay, sub)
            report(f"fmriprep {name} bold runs", _bold_data(p, sub), bold)
            for combo in COMBOS:
                sfm.clear_registry()
                ls.clear_registry()
                cap.lines.clear()
                lcap.lines.clear()
                a = dump._call(ser, fns["map_fieldmap_estimation"], p, sub, None, bold, **combo)
                with warnings.catch_warnings():
                    warnings.simplefilter("ignore")
                    b = dump._call(ser, le.map_fieldmap_estimation, lay, sub, None, bold, logger=llogger,
                                   upstream_defects="reproduce", **combo)  # fmt: skip
                report(f"fmriprep {name} sub-{sub} {combo}", [a, cap.lines], [b, lcap.lines])
                for f in [s[0] for s in bold]:
                    report(f"fmriprep {name} get_estimator {ser.s(f)}", dump._call(ser, fns["get_estimator"], p, f),
                           dump._call(ser, le.get_estimator, lay, f))  # fmt: skip
    # collect_fieldmaps on sdcflows' own derivatives and the edges tree's
    import fmriprep.utils.bids as fbids

    orig_src = (TAGS / "fmriprep-25.2.7" / "fmriprep" / "utils" / "bids.py").read_text(
        encoding="utf-8"
    )
    import inspect

    same = (
        inspect.getsource(fbids.collect_fieldmaps) in orig_src
        and inspect.getsource(fbids._get_layout) in orig_src
    )
    report("fmriprep collect_fieldmaps source = 25.2.7's", True, same)
    spec = json.loads(
        (TAGS / "fmriprep-25.2.7" / "fmriprep" / "data" / "fmap_spec.json").read_text(
            encoding="utf-8"
        )
    )
    report("fmriprep fmap_spec.json", spec["queries"], le.FMAP_SPEC)
    for run in sorted(SDC_RUNS.iterdir()):
        deriv = run / "out" / "sdcflows"
        if not deriv.is_dir():
            continue
        ser = dump.Ser({str(deriv): "<deriv>"})
        sub = run.name.split("-sub-")[1].split("-")[0]
        a = dump._call(ser, fbids.collect_fieldmaps, deriv, {"subject": sub})
        b = dump._call(ser, le.collect_fieldmaps, deriv, {"subject": sub})
        report(f"fmriprep collect_fieldmaps {run.name}", a, b)


def section_runs() -> None:
    """The oracle runs' own fmriprep.log lines against larmorx's estimators."""
    import larmorx.sdcflows as ls
    from larmorx.pipelines.larmorprepx import estimators as le

    for run, root, sub, opts in oracle_datasets():
        log = (ORACLES / run / "fmriprep.log").read_text(encoding="utf-8", errors="replace")
        want_setup = re.findall(r'Setting up fieldmap "[^"]+" \([^)]+\) with <[^>]*>', log)
        want_several = [
            x.replace("/data/", f"{root}/")
            for x in re.findall(
                r"Several fieldmaps <[^>]+> are 'IntendedFor' <[^>]+>, using \S+", log
            )
        ]
        want_est = re.findall(
            r"B0 field inhomogeneity map will be estimated with the following .*", log
        )
        ls.clear_registry()
        lay = le.fmriprep_layout(root, [sub], pybids_version="0.20.0")
        logger, cap = dump.capture_logger("check.runs")
        # fMRIPrep 25.2 processes each session on its own (sub_<label>_ses_<session>_wf)
        subwf = sorted((ORACLES / run / "work" / "fmriprep_25_2_wf").glob(f"sub_{sub}*_wf"))
        ses = re.search(r"_ses_(.+)_wf$", subwf[0].name) if subwf else None
        session = ses.group(1) if ses else None
        bold = [b for b in _bold_data(lay, sub) if session is None or f"_ses-{session}_" in b[0]]
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            ests, _emap = le.map_fieldmap_estimation(
                lay, sub, session, bold, opts["ignore_fieldmaps"], opts["use_syn"], opts["force_syn"],
                None, logger=logger, sdcflows_version="2.15", upstream_defects="reproduce",
            )  # fmt: skip
            todo, _ = le.split_precomputed(ests, {}, logger=logger)
        got_setup = [le.setting_up_message(e) for e in todo]
        got_several = [x.split(": ", 1)[1] for x in cap.lines if "Several fieldmaps" in x]
        got_est = [x.split(": ", 1)[1] for x in cap.lines if "estimated with the following" in x]
        for e in todo:
            le.check_pepolar_scheme(e)
        report(f"runs {run} setting up", want_setup, got_setup)
        report(f"runs {run} several", want_several, got_several)
        report(f"runs {run} estimated", want_est, got_est)
        # their fmap_preproc_wf folders
        wf = sorted(
            p.name
            for p in (ORACLES / run / "work").glob(
                f"fmriprep_25_2_wf/sub_{sub}*_wf/fmap_preproc_wf/wf_*"
            )
        )
        report(f"runs {run} fmap_preproc_wf", wf, sorted(f"wf_{e.sanitized_id}" for e in todo))


def section_apps() -> None:
    import larmorx.sdcflows as ls
    from larmorx.bids import BIDSLayout, BIDSLayoutIndexer

    for run in sorted(SDC_RUNS.iterdir()):
        info = (run / "RUN_INFO.txt").read_text(encoding="utf-8")
        root = Path(re.search(r"^BIDS input: (\S+)", info, re.M).group(1))  # type: ignore[union-attr]
        sub = re.search(r"^participant: (\S+)", info, re.M).group(1)  # type: ignore[union-attr]
        lay = BIDSLayout(
            str(root), indexer=BIDSLayoutIndexer(validate=False, **indexers(sub)["sdcflows_app"])
        )
        ls.clear_registry()
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            ests = ls.find_estimators(
                layout=lay, subject=sub, fmapless=False, upstream_defects="reproduce"
            )
        got = sorted(re.sub(r"[^a-zA-Z0-9]", "", e.bids_id) for e in ests)
        want = sorted(
            {
                m.group(1)
                for m in (
                    re.search(r"_fmapid-([a-zA-Z0-9]+)_", p.name)
                    for p in (run / "out").rglob("*_fieldmap.nii.gz")
                )
                if m
            }
        )
        report(f"apps {run.name} fmapids", want, got)


def section_image(datasets: list[tuple[str, Path, list[str]]], scratch: Path) -> None:
    """pybids 0.20.0, sdcflows 2.15.1, fMRIPrep 25.2.6 inside fMRIPrep's image."""
    import larmorx.sdcflows as ls
    from larmorx.bids import Query as LQ
    from larmorx.bids import parse_file_entities
    from larmorx.pipelines.larmorprepx import estimators as le

    items = []
    for name, root, subjects in datasets:
        if name in ("conflict", "badjson"):
            continue
        sub = subjects[0]
        lay = le.fmriprep_layout(root, [sub], pybids_version="0.20.0")
        items.append({
            "name": name, "root": str(root), "subject": sub,
            "wrangler": [{k: v for k, v in c.items() if k != "subject"} for c in wrangler_cases(lay, sub)[:8]],
            "fmriprep": [dict(c, session_id=None, bold_data=_bold_data(lay, sub)) for c in COMBOS[:8]],
        })  # fmt: skip
    cases = scratch / "image_cases.json"
    cases.write_text(json.dumps(items), encoding="utf-8")
    out = scratch / "image_dump.json"
    binds = sorted(
        {str(Path(i["root"]).parent) for i in items} | {str(HERE / "relay11"), str(scratch)}
    )
    cmd = ["apptainer", "exec", "--cleanenv"]
    for b in binds:
        cmd += ["-B", f"{b}:{b}"]
    cmd += [
        str(IMAGE),
        "python",
        "-I",
        str(HERE / "relay11" / "estimators_dump.py"),
        str(cases),
        str(out),
    ]
    proc = subprocess.run(cmd, capture_output=True, text=True, timeout=1800, check=False)
    if proc.returncode:
        FAILS.append("image dump")
        print(proc.stdout[-2000:], proc.stderr[-4000:])
        return
    got = json.loads(out.read_text(encoding="utf-8"))
    for item in items:
        name, root, sub = item["name"], Path(item["root"]), item["subject"]
        ser = dump.Ser({str(root): "<root>"})
        lay = le.fmriprep_layout(root, [sub], pybids_version="0.20.0")
        want = got[name]
        if "error" in want:
            report(f"image {name} layout", want["error"], "built")
            continue
        report(
            f"image {name} index", want["index"], dump.dump_index(lay, ser, LQ, parse_file_entities)
        )

        def state(ser: dump.Ser = ser) -> dict[str, Any]:
            return dump.registry_state(ls.REGISTRY.estimators, ls.REGISTRY.intents, ser)

        def lfind(**kw: Any) -> Any:
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                return ls.find_estimators(
                    upstream_defects="reproduce", sdcflows_version="2.15", **kw
                )

        mine = dump.dump_wrangler(
            lfind,
            ls.clear_registry,
            state,
            lay,
            ser,
            [{"subject": sub, **c} for c in item["wrangler"]],
        )
        for x, y in zip(want["wrangler"], mine, strict=True):
            report(f"image {name} wrangler {json.dumps(x['case'])[:100]}", x, y)
        logger, cap = dump.capture_logger("check.image")
        for combo, w in zip(item["fmriprep"], want["fmriprep"], strict=True):
            ls.clear_registry()
            cap.lines.clear()
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                try:
                    ests, emap = le.map_fieldmap_estimation(lay, sub, logger=logger, sdcflows_version="2.15",
                                                            upstream_defects="reproduce", **combo)  # fmt: skip
                    res: Any = [ser(ests), [[ser.s(k), v] for k, v in emap.items()]]
                except Exception as err:
                    res = ser.error(err)
            report(f"image {name} fmriprep {combo['ignore_fieldmaps'], combo['use_syn'], combo['force_syn'], combo['filters']}",
                   [w["result"], w["log"], w["registry"]], [res, [ser.s(x) for x in cap.lines], state()])  # fmt: skip


#: The folders the roots section puts each dataset below: none of pybids' patterns, the
#: derivatives' unanchored ``den-`` and ``res-`` (SDC-45), ``space-`` (anchored: a control) and
#: two of the raw configuration's anchored ones, ``func`` and ``sub-`` (PIPE-37).
ROOT_PARENTS = ("plain", "golden-x", "andres-lab", "workspace-2024", "func", "sub-pilot")


def _strip_parse(d: dict[str, Any]) -> dict[str, Any]:
    """An index dump without the module's ``parse_file_entities`` of each absolute path (pybids'
    API, which parses the path it is given in both modes)."""
    out = dict(d)
    out["files"] = {
        k: {x: y for x, y in v.items() if not x.startswith("parse")} for k, v in d["files"].items()
    }
    return out


def _fmriprep_get_sbrefs() -> Any:
    """fMRIPrep 25.2.7's ``extract_entities`` and ``get_sbrefs`` from its source (PIPE-36)."""
    from bids.utils import listify

    ns: dict[str, Any] = {"listify": listify}
    for rel, name in (
        ("utils/bids.py", "extract_entities"),
        ("workflows/bold/fit.py", "get_sbrefs"),
    ):
        src = TAGS / "fmriprep-25.2.7" / "fmriprep" / rel
        tree = ast.parse(src.read_text(encoding="utf-8"))
        fn = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == name)
        for arg in fn.args.args:  # the annotations name fMRIPrep's imports
            arg.annotation = None
        fn.returns = None
        exec(compile(ast.Module(body=[fn], type_ignores=[]), str(src), "exec"), ns)
    return ns["get_sbrefs"]


def section_roots(scratch: Path) -> None:
    """SDC-45 and PIPE-37: datasets below folders whose names hold pybids' patterns
    (:data:`ROOT_PARENTS`): pybids' index against ``lx.bids``' reproduced, sdcflows 2.17.1's
    search against larmorx's reproduced (the same error under ``golden-x`` and ``andres-lab``);
    larmorx's corrected index and search below each folder against pybids' and sdcflows 2.15.2's
    on the dataset below no such folder; and fMRIPrep's ``get_sbrefs`` (PIPE-36, not ported) on
    each, reported."""
    import sdcflows.utils.wrangler as w2171
    from bids.layout import BIDSLayout as PLayout
    from bids.layout import Query as PQ
    from bids.layout import parse_file_entities as pparse
    from niworkflows.utils.testing import generate_bids_skeleton
    from sdcflows import fieldmaps as sfm

    import larmorx.bids as lb
    import larmorx.sdcflows as ls

    w2152 = _load_wrangler_2152()
    for mod in (w2171, w2152):
        mod.reduce = dump.natural_reduce  # SDC-19
    specs = _wrangler_specs()
    T1w = {"suffix": "T1w"}
    bold = {"task": "rest", "suffix": "bold",
            "metadata": {"RepetitionTime": 0.8, "TotalReadoutTime": 0.5, "PhaseEncodingDirection": "j"}}  # fmt: skip
    sources: list[tuple[str, Any, str]] = [
        ("phasediff", specs["phasediff"], "01"),
        ("pepolar", specs["pepolar"], "01"),
        ("fmapless_sbref", {"01": {"anat": [T1w], "func": [bold, {**bold, "suffix": "sbref"}]}}, "01"),
    ]  # fmt: skip
    syn = [(n, r, s) for n, r, s, o in oracle_datasets() if o["use_syn"]][:1]
    get_sbrefs = _fmriprep_get_sbrefs()
    for parent in ROOT_PARENTS:
        for name, spec, sub in sources + [(n, r, s) for n, r, s in syn]:
            root = scratch / "roots" / parent / name
            if root.exists() or root.is_symlink():
                shutil.rmtree(root) if not root.is_symlink() else root.unlink()
            root.parent.mkdir(parents=True, exist_ok=True)
            if isinstance(spec, Path):
                root.symlink_to(spec, target_is_directory=True)  # a real dataset, linked
            else:
                generate_bids_skeleton(str(root), spec)
            plain = scratch / "roots" / "plain" / name
            ser = dump.Ser({str(root): "<root>"})
            pser = dump.Ser({str(plain): "<root>"})
            which = "fmriprep" if isinstance(spec, Path) else "default"
            tag = f"roots {parent}/{name}"
            spec_kw = indexers(sub)[which]

            def layouts(
                side: str, mode: str | None = None, at: Path = root, spec_kw=spec_kw
            ) -> Any:
                kw: dict[str, Any] = {} if mode is None else {"upstream_defects": mode}
                if side == "pybids":
                    from bids.layout import BIDSLayoutIndexer as PI

                    return PLayout(str(at), indexer=PI(validate=False, **spec_kw))
                return lb.BIDSLayout(
                    str(at), indexer=lb.BIDSLayoutIndexer(validate=False, **spec_kw), **kw
                )

            with warnings.catch_warnings(record=True) as caught:
                warnings.simplefilter("always")
                p, lr, lf = layouts("pybids"), layouts("larmorx", "reproduce"), layouts("larmorx")
            pipe37 = sorted(
                {(w.message.entry, w.message.mode) for w in caught if hasattr(w.message, "entry")}
            )
            pidx = dump.dump_index(p, ser, PQ, pparse)
            report(
                f"{tag} index reproduce",
                pidx,
                dump.dump_index(lr, ser, lb.Query, lb.parse_file_entities),
            )
            plain_idx = dump.dump_index(layouts("pybids", at=plain), pser, PQ, pparse)
            report(f"{tag} index fix = pybids below plain/",
                   _strip_parse(plain_idx), _strip_parse(dump.dump_index(lf, ser, lb.Query, lb.parse_file_entities)))  # fmt: skip
            changed = _strip_parse(pidx) != _strip_parse(plain_idx)
            want = [("PIPE-37", "fix"), ("PIPE-37", "reproduce")] if changed else []
            report(f"{tag} PIPE-37 notices", want, pipe37)
            if changed:
                print(f"INFO {tag}: pybids' index differs from its index below plain/ (PIPE-37)")
            cases = [{"subject": sub, "force_fmapless": True}, {"subject": sub}]
            for label, find, mode, lay_o, lay_l, s_o in (
                ("2.17.1", w2171.find_estimators, "reproduce", p, lr, ser),
                ("2.15.2", w2152.find_estimators, "fix", layouts("pybids", at=plain), lf, pser),
            ):
                a = dump.dump_wrangler(
                    find, sfm.clear_registry,
                    lambda s_o=s_o: dump.registry_state(sfm._estimators, sfm._intents, s_o), lay_o, s_o, cases,
                )  # fmt: skip
                notes: list[list[str]] = []

                def lfind(mode: str = mode, notes: list[list[str]] = notes, **kw: Any) -> Any:
                    with warnings.catch_warnings(record=True) as cw:
                        warnings.simplefilter("always")
                        try:
                            return ls.find_estimators(upstream_defects=mode, **kw)
                        finally:
                            notes.append(
                                sorted({getattr(w.message, "entry", "") for w in cw} - {""})
                            )

                b = dump.dump_wrangler(
                    lfind, ls.clear_registry,
                    lambda ser=ser: dump.registry_state(ls.REGISTRY.estimators, ls.REGISTRY.intents, ser), lay_l, ser, cases,
                )  # fmt: skip
                for x, y, n in zip(a, b, notes, strict=True):
                    report(f"{tag} wrangler {label} {x['case']}", x, y)
                    if parent in ("golden-x", "andres-lab") and x["case"] == ser(cases[0]):
                        print(f"INFO {tag} {label}: larmorx's notices {n}; sdcflows: "
                              f"{json.dumps(x['result'])[:160]}")  # fmt: skip
            # fMRIPrep's get_sbrefs on each BOLD run (absolute paths), pybids' layout: PIPE-36
            runs = p.get(
                subject=sub, suffix="bold", extension=[".nii", ".nii.gz"], return_type="file"
            )
            for f in runs[:1]:
                try:
                    out = [ser.s(x) for x in get_sbrefs([f], {}, layout=p)]
                except Exception as err:
                    out = ser.error(err)
                print(f"INFO {tag} fMRIPrep get_sbrefs (PIPE-36): {json.dumps(out)[:200]}")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--sections", default="index,wrangler,units,fmriprep,runs,apps,roots,image")
    ap.add_argument("--scratch", default=None)
    args = ap.parse_args()
    scratch = Path(args.scratch or tempfile.mkdtemp(prefix="lx-estimators-"))
    scratch.mkdir(parents=True, exist_ok=True)
    sections = args.sections.split(",")
    datasets = all_datasets(scratch)
    for sec in sections:
        n0, f0 = COUNTS["ok"], len(FAILS)
        if sec == "index":
            section_index(datasets)
        elif sec == "wrangler":
            section_wrangler(datasets)
        elif sec == "units":
            section_units(scratch)
        elif sec == "fmriprep":
            section_fmriprep(datasets)
        elif sec == "runs":
            section_runs()
        elif sec == "apps":
            section_apps()
        elif sec == "roots":
            section_roots(scratch)
        elif sec == "image":
            section_image(datasets, scratch)
        print(f"== {sec}: {COUNTS['ok'] - n0} identical, {len(FAILS) - f0} differ", flush=True)
    if EXPECTED:
        print(f"expected differences ({len(EXPECTED)}): " + "; ".join(EXPECTED[:10]))
    print("ALL OK" if not FAILS else f"FAILED {len(FAILS)}: {FAILS[:20]}")
    return 1 if FAILS else 0


if __name__ == "__main__":
    raise SystemExit(main())
