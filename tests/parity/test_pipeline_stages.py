# SPDX-License-Identifier: Apache-2.0
"""Stage by stage against a real fMRIPrep run (validation/, ``pipeline-stages``).

Needs an fMRIPrep oracle run (``scripts/run_fmriprep_oracle.sh``): ``LARMORX_FMRIPREP_ORACLE``,
else the workspace's ``oracles/fmriprep-25.2.6/ds000005-sub-01-noreconall``. Skipped without it,
so never in CI. Only the quick stages run here (the brain-extraction morphology, the
thresholding and the truncation), without the container and the ANTs references; the full
comparison is ``python -m larmorx_validation pipeline-stages``.
"""

import os
from pathlib import Path

import numpy as np
import pytest

pytest.importorskip("nibabel")
stages = pytest.importorskip("larmorx_validation.stages")
from larmorx_validation.stages import catalogue, runners  # noqa: E402
from larmorx_validation.stages.oracle import Node, OracleRun  # noqa: E402


def _oracle() -> Path | None:
    if env := os.environ.get("LARMORX_FMRIPREP_ORACLE"):
        return Path(env)
    for parent in Path(__file__).resolve().parents:
        candidate = parent / "oracles/fmriprep-25.2.6/ds000005-sub-01-noreconall"
        if (candidate / "work").is_dir():
            return candidate
    return None


def test_output_names():
    assert runners.output_names(["ImageMath", "3", "o.nii.gz", "ME", "i.nii.gz", "2"]) == [
        "o.nii.gz"
    ]
    assert runners.output_names(["ThresholdImage", "3", "i.nii.gz", "o.nii.gz", "0.5", "1"]) == [
        "o.nii.gz"
    ]
    argv = ["N4BiasFieldCorrection", "-d", "3", "--output", "[", "c.nii.gz,", "b.nii.gz", "]", "-r"]
    assert runners.output_names(argv) == ["c.nii.gz", "b.nii.gz"]
    argv = ["antsApplyTransforms", "-i", "a", "-o", "/work/x/out.nii.gz", "-r", "b"]
    assert runners.output_names(argv) == ["/work/x/out.nii.gz"]


def test_normalise():
    path = (
        "fmriprep_25_2_wf/sub_01_wf/bold_task_x_run_01_wf/bold_std_wf/"
        "_in_tuple_MNI152NLin2009cAsym.res2/resample"
    )
    norm, keys = catalogue.normalise(path)
    assert norm == "bold_wf/bold_std_wf/resample"
    assert keys == {"subject": "01", "run": "task_x_run_01", "space": "MNI152NLin2009cAsym.res2"}
    norm, keys = catalogue.normalise(
        "fmriprep_25_2_wf/sub_01_wf/anat_fit_wf/anat_template_wf/denoise/mapflow/_denoise0"
    )
    assert norm == "anat_fit_wf/anat_template_wf/denoise"
    assert keys["item"] == "_denoise0"


def _node(path: str, command: str | None = None, **kw) -> Node:
    return Node(
        path=path,
        name=path.rsplit("/", 1)[-1],
        interface=kw.pop("interface", ""),
        inputs=kw.pop("inputs", {}),
        outputs={},
        runtime={},
        environment={},
        command=command,
        **kw,
    )


def test_slice_timing_stage():
    """fMRIPrep 25.2.6 names the node slice_timing_correction (fmriprep/workflows/bold/stc.py);
    the catalogue's first pattern did not match it, so 3dTshift was never compared."""
    wf = "fmriprep_25_2_wf/sub_002_wf/bold_task_clip_run_1_wf/bold_native_wf/bold_stc_wf"
    node = _node(f"{wf}/slice_timing_correction", "3dTshift -ignore 0 -prefix o.nii.gz i.nii.gz")
    stage = catalogue.stage_of(node)
    assert stage is not None and stage.id == "F02" and stage.kind == "tshift"
    # the field-map stages
    n4 = "fmriprep_25_2_wf/sub_16_wf/fmap_preproc_wf/wf_gre16/magnitude_wf/brainextraction_wf/n4"
    assert catalogue.stage_of(_node(n4, "N4BiasFieldCorrection -d 3")).id == "S01"
    unwarp = "fmriprep_25_2_wf/sub_1_wf/bold_task_x_wf/bold_fit_wf/unwarp_boldref"
    assert catalogue.stage_of(_node(unwarp, interface="resampling")).id == "S02"
    topup = _node("fmriprep_25_2_wf/sub_1_wf/fmap_preproc_wf/wf_auto_00000/topup", "topup --a")
    assert catalogue.stage_of(topup) is None
    assert catalogue.missing_of(topup).tool == "topup"


def test_prepare_inputs(tmp_path):
    """3dTshift's input, linked into the node directory and removed by nipype after the run,
    is given by its original path; slice_timing.1D is written as nipype's TShift writes it."""
    (tmp_path / "work").mkdir()
    run = OracleRun(tmp_path)
    path = "fmriprep_25_2_wf/sub_1_wf/bold_task_x_wf/bold_native_wf/bold_stc_wf/slice_timing_correction"
    linked = f"/work/{path}/sub-1_task-x_bold.nii.gz"
    argv = [
        "3dTshift",
        "-ignore",
        "0",
        "-prefix",
        "o.nii.gz",
        "-tpattern",
        "@slice_timing.1D",
        "-TR",
        "2s",
        "-tzero",
        "0.5",
        linked,
    ]
    node = _node(
        path,
        " ".join(argv),
        inputs={"slice_timing": "[0.0, 0.5, 0.25, 1, 0.1]", "slice_encoding_direction": "k-"},
        hashed={"in_file": "/data/sub-1/func/sub-1_task-x_bold.nii.gz", "tr": "2s"},
    )
    out, files = runners.prepare_inputs(run, node, node.argv)
    assert out[-1] == "/data/sub-1/func/sub-1_task-x_bold.nii.gz"
    assert out[:-1] == argv[:-1]
    # reversed for a negative slice-encoding direction, joined by tabs, no newline
    assert files == {"slice_timing.1D": "0.1\t1.0\t0.25\t0.5\t0.0"}


def test_bids_run():
    """The run key of a derivative is fMRIPrep's workflow name: sessions included."""
    from larmorx_validation.stages.nodes import _bids_run

    assert _bids_run("sub-01_task-x_run-01_desc-confounds_timeseries.tsv") == "task_x_run_01"
    name = "sub-ON01016_ses-01_task-rest_dir-forward_desc-confounds_timeseries.tsv"
    assert _bids_run(name) == "ses_01_task_rest_dir_forward"


def test_tshift_thresholds():
    """afni-tshift.md's thresholds: integers within one stored unit, with the same scale."""
    from larmorx_validation.stages.compare import compare_arrays
    from larmorx_validation.stages.nodes import tshift_within

    a = np.zeros((2, 2, 2, 2), np.float64)
    b = a.copy()
    b[0, 0, 0, 0] = 47.7
    d = compare_arrays(a, b)
    stored = {"dtype": ["int16", "int16"], "scl_slope": [47.7, 47.7], "differing": 1}
    assert tshift_within(d, {**stored, "max_abs": 1, "values": 16})[0]
    assert not tshift_within(d, {**stored, "max_abs": 2, "values": 16})[0]
    assert not tshift_within(d, {**stored, "scl_slope": [47.7, 1.0], "max_abs": 1, "values": 16})[0]
    assert not tshift_within(d)[0]  # float: 47.7 / 47.7 relative


ORACLE = _oracle()
needs_oracle = pytest.mark.skipif(ORACLE is None, reason="no fMRIPrep oracle run")


def _oracle_runs() -> list[Path]:
    """Every finished fMRIPrep oracle run of the workspace (``oracles/fmriprep-*/<run>``)."""
    for parent in Path(__file__).resolve().parents:
        root = parent / "oracles"
        if root.is_dir():
            return sorted(
                p
                for p in root.glob("fmriprep-*/*")
                if (p / "work").is_dir() and "finished:" in _info(p)
            )
    return []


def _info(run: Path) -> str:
    f = run / "RUN_INFO.txt"
    return f.read_text(encoding="utf-8") if f.is_file() else ""


@pytest.mark.parametrize("root", _oracle_runs() or [None], ids=lambda p: p.name if p else "none")
def test_every_program_node_is_classified(root):
    if root is None:
        pytest.skip("no fMRIPrep oracle run")
    run = OracleRun(root)
    unclassified = [
        n.path
        for n in run.all_nodes
        if n.program and catalogue.stage_of(n) is None and catalogue.missing_of(n) is None
    ]
    assert not unclassified


@needs_oracle
def test_quick_stages_bit_identical(tmp_path):
    run = OracleRun(ORACLE)
    s = runners.Settings(
        run=run,
        compare=tmp_path,
        container=False,
        ants_oracle=False,
        select=("A03", "A07", "A08"),
    )
    payload = stages.run_stages(s)
    assert payload["results"]
    for r in payload["results"]:
        assert r["status"] == "identical", (r["node"], r.get("note"))
