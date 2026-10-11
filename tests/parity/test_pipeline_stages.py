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

import pytest

pytest.importorskip("nibabel")
stages = pytest.importorskip("larmorx_validation.stages")
from larmorx_validation.stages import catalogue, runners  # noqa: E402
from larmorx_validation.stages.oracle import OracleRun  # noqa: E402


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


ORACLE = _oracle()
needs_oracle = pytest.mark.skipif(ORACLE is None, reason="no fMRIPrep oracle run")


@needs_oracle
def test_every_program_node_is_classified():
    run = OracleRun(ORACLE)
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
