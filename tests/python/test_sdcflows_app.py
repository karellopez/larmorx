# SPDX-License-Identifier: Apache-2.0
"""sdcflows' BIDS app (``larmorx sdcflows sdcflows``, :mod:`larmorx.sdcflows.app`): its options,
settings, dry run, exit statuses and corrected defects on small generated datasets (sdcflows'
own app is compared with it in validation/investigations/sdcflows-fieldmap-b/check_app.py)."""

from __future__ import annotations

import json
import tomllib
from pathlib import Path

import numpy as np
import pytest

import larmorx as lx
from larmorx import _core
from larmorx.cli import run
from larmorx.sdcflows import app


def _bids(root: Path, *, epi: bool = True, fieldmap: bool = False) -> Path:
    """One participant: a T1w, a BOLD run and (``epi``) an AP/PA EPI pair intended for it."""
    aff = np.diag([2.0, 2.0, 2.0, 1.0])
    data = np.zeros((6, 6, 6), np.int16)
    files = {
        "anat/sub-01_T1w.nii.gz": {},
        "func/sub-01_task-rest_bold.nii.gz": {
            "PhaseEncodingDirection": "j",
            "TotalReadoutTime": 0.05,
            "RepetitionTime": 2.0,
        },
    }
    intent = {"IntendedFor": "func/sub-01_task-rest_bold.nii.gz"}
    if epi:
        for d, pe in (("AP", "j-"), ("PA", "j")):
            files[f"fmap/sub-01_dir-{d}_epi.nii.gz"] = {
                "PhaseEncodingDirection": pe,
                "TotalReadoutTime": 0.05,
                **intent,
            }
    if fieldmap:
        files["fmap/sub-01_fieldmap.nii.gz"] = {"Units": "Hz", **intent}
        files["fmap/sub-01_magnitude.nii.gz"] = {}
    for name, meta in files.items():
        path = root / "sub-01" / name
        path.parent.mkdir(parents=True, exist_ok=True)
        lx.save((data, aff), str(path))
        path.with_name(path.name.replace(".nii.gz", ".json")).write_text(
            json.dumps(meta), encoding="utf-8"
        )
    (root / "dataset_description.json").write_text(
        '{"Name": "t", "BIDSVersion": "1.8.0"}', encoding="utf-8"
    )
    return root


def _run(args: list[str], capsys: pytest.CaptureFixture[str], **kw: object) -> tuple[int, str, str]:
    code = app.run_app(args, env={"OMP_NUM_THREADS": "1"}, **kw)  # type: ignore[arg-type]
    out, err = capsys.readouterr()
    return code, out, err


def test_parser_options_and_version(capsys: pytest.CaptureFixture[str], tmp_path: Path) -> None:
    p = app.build_parser()
    ns = p.parse_args(
        [
            str(tmp_path),
            "out",
            "participant",
            "--participant-label",
            "sub-02 01",
            "01",
            "-vv",
            "--mem-gb",
            "512M",
            "--nprocs",
            "4",
            "--no-fmapless",
            "-n",
        ]
    )
    assert ns.participant_label == ["01", "02"]
    assert ns.verbose_count == 2 and ns.memory_gb == pytest.approx(0.512)
    assert ns.nprocs == 4 and ns.fmapless is False and ns.dry_run is True
    assert ns.bids_dir == tmp_path.absolute()
    with pytest.raises(SystemExit) as e:
        p.parse_args(["--version"])
    assert e.value.code == 0 and capsys.readouterr().out == "SDCFlows 2.17.1\n"
    with pytest.raises(SystemExit) as e:
        p.parse_args([str(tmp_path), "out", "participant", "--nprocs", "abc"])
    assert e.value.code == 2
    assert "invalid _min_one value: 'abc'" in capsys.readouterr().err  # SDC-34


def test_mem_gb_units_sdc33(capsys: pytest.CaptureFixture[str], tmp_path: Path) -> None:
    args = [str(tmp_path), "out", "participant", "--mem-gb", "16g"]
    with pytest.raises(SystemExit) as e:
        app.build_parser().parse_args(args)
    assert e.value.code == 2 and "invalid _to_gb value: '16g'" in capsys.readouterr().err
    with pytest.raises(KeyError):
        app.build_parser(upstream_defects="reproduce").parse_args(args)


def test_dry_run_listing_and_settings(capsys: pytest.CaptureFixture[str], tmp_path: Path) -> None:
    root = _bids(tmp_path / "bids")
    out, work = tmp_path / "out", tmp_path / "work"
    code, text, err = _run([str(root), str(out), "participant", "-w", str(work), "-n"], capsys)
    assert code == 0, err
    lines = text.splitlines()
    assert lines[0] == f"Estimation for <{root}> complete. Found:"
    assert lines[1] == "\tsub-01"
    assert lines[2] == (
        "\t\tFieldmapEstimation(sources=<2 files>, method=<EstimatorType.PEPOLAR: 2>, "
        "bids_id='auto_00000')"
    )
    assert lines[3].split("\t")[3:] == ["j-", str(Path("fmap/sub-01_dir-AP_epi.nii.gz"))]
    assert (out / "logs").is_dir() and not list(out.glob(".bids_db*"))
    (settings,) = work.glob(".sdcflows.*.toml")
    cfg = tomllib.loads(settings.read_text(encoding="utf-8"))
    assert list(cfg) == ["environment", "execution", "workflow", "nipype"]
    assert cfg["execution"]["participant_label"] == ["01"]
    assert cfg["execution"]["dry_run"] is True and cfg["workflow"]["fmapless"] is True
    assert cfg["nipype"]["omp_nthreads"] == 1 and cfg["environment"]["version"] == "2.17.1"
    assert cfg["execution"]["layout"].startswith("BIDS Layout: ...")


def test_session_label_sdc30(capsys: pytest.CaptureFixture[str], tmp_path: Path) -> None:
    root = _bids(tmp_path / "bids")
    args = [str(root), str(tmp_path / "out"), "participant", "-w", str(tmp_path / "w"), "-n"]
    args += ["--session-label", "zz"]
    code, text, _ = _run(args, capsys)
    assert code == 0 and "No estimators found" in text  # no session zz
    code, text, _ = _run(args, capsys, upstream_defects="reproduce")
    assert code == 0 and "PEPOLAR" in text  # sdcflows ignores it


def test_exit_statuses(capsys: pytest.CaptureFixture[str], tmp_path: Path) -> None:
    bare = _bids(tmp_path / "bare", epi=False)
    base = [str(bare), str(tmp_path / "out"), "participant", "-w", str(tmp_path / "w")]
    # no estimator: os.EX_USAGE and sdcflows' message on standard error
    code, out, err = _run([*base, "--no-fmapless"], capsys)
    assert code == 64
    assert err == "Workflow did not generate any jobs. Please check your inputs are valid.\n"
    assert "Running SDCFlows 2.17.1:" in out and "cli IMPORTANT:" in out
    # SDC-29: fieldmap-less estimators stop the run before any work
    code, _, err = _run(base, capsys)
    assert code == 1 and "SDC-29" in err and "--no-fmapless" in err
    assert not list((tmp_path / "out").rglob("*.nii.gz"))
    code, _, err = _run(base, capsys, upstream_defects="reproduce")
    assert code == 1 and err.startswith("Traceback (most recent call last):\nValueError: ")
    # group: sdcflows' ValueError (its traceback's first and last lines)
    code, _, err = _run([str(bare), str(tmp_path / "o2"), "group"], capsys)
    assert code == 1
    assert err == (
        "Traceback (most recent call last):\nValueError: Analysis level can only be 'participant'\n"
    )
    # argparse's errors
    code, _, err = _run([str(bare), str(bare), "participant"], capsys)
    assert code == 2 and "The selected output folder is the same as the input BIDS folder" in err
    code, _, err = _run([*base, "--participant-label", "07", "05"], capsys)
    assert code == 2 and err.rstrip().endswith("BIDS directory: 05, 07.")
    code, _, err = _run([*base, "--bids-filter-file", str(tmp_path / "none.json")], capsys)
    assert code == 2 and "Path does not exist" in err  # SDC-33
    code, _, err = _run(
        [*base, "--bids-filter-file", str(tmp_path / "none.json")],
        capsys,
        upstream_defects="reproduce",
    )
    assert code == 1 and err.splitlines()[-1].startswith("FileNotFoundError: [Errno 2]")
    plugin = tmp_path / "plugin.yml"
    plugin.write_text("plugin: Linear\n", encoding="utf-8")
    code, _, err = _run([*base, "-n", "--use-plugin", str(plugin)], capsys)
    assert code == 0, err  # SDC-35: accepted
    code, _, err = _run(
        [*base, "-n", "--use-plugin", str(plugin)], capsys, upstream_defects="reproduce"
    )
    assert code == 1 and err.endswith(
        "TypeError: load() missing 1 required positional argument: 'Loader'\n"
    )


def test_bids_validation_error(capsys: pytest.CaptureFixture[str], tmp_path: Path) -> None:
    root = _bids(tmp_path / "bids")
    (root / "dataset_description.json").unlink()
    code, _, err = _run([str(root), str(tmp_path / "out"), "participant"], capsys)
    assert code == 1
    assert err.startswith(
        "Traceback (most recent call last):\nbids.exceptions.BIDSValidationError: "
    )


def test_sdcflows_workflow_built(tmp_path: Path) -> None:
    root = _bids(tmp_path / "bids", fieldmap=True)
    lay = lx.bids.BIDSLayout(str(root), validate=False)
    wf = lx.sdcflows.sdcflows_workflow(lay, output_dir=tmp_path / "out", fmapless=False)
    assert wf.subjects == ("01",)
    assert sorted(e.method.name for e in wf.all_estimators) == ["MAPPED", "PEPOLAR"]
    assert [w.name for w in wf.workflows] == [f"wf_{e.sanitized_id}" for e in wf.all_estimators]
    assert lx.niworkflows.collect_participants(lay) == ["01"]
    with pytest.warns(lx.niworkflows.BIDSWarning):
        assert lx.niworkflows.collect_participants(lay, ["sub-01", "02"]) == ["01"]
    with pytest.raises(lx.niworkflows.BIDSError, match="Could not find participants"):
        lx.niworkflows.collect_participants(lay, ["03"])
    with pytest.raises(lx.sdcflows.FieldmaplessError):
        lx.sdcflows.sdcflows_workflow(
            lx.bids.BIDSLayout(str(_bids(tmp_path / "b2", epi=False)), validate=False),
            output_dir=tmp_path / "o2",
        )


def test_toml_dumps() -> None:
    text = app.toml_dumps({"a": {"s": 'q"', "l": ["x", 1, True], "f": 1e-05, "n": None, "t": {}}})
    assert text == '[a]\ns = "q\\""\nl = [ "x", 1, true,]\nf = 1e-5\n\n[a.t]\n'


def test_command_line_routes(capsys: pytest.CaptureFixture[str]) -> None:
    assert run(["larmorx", "sdcflows", "sdcflows", "--version"]) == 0
    assert capsys.readouterr().out == "SDCFlows 2.17.1\n"
    # the Rust command line alone (the standalone binary) says it needs this package
    code, out, err = _core.cli_main(["larmorx", "sdcflows", "sdcflows", "--version"], [], None)
    assert code == 127 and out == "" and "is a Python program" in err
