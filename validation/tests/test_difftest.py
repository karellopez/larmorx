# SPDX-License-Identifier: Apache-2.0
"""Differential random testing (``larmorx_validation.difftest``, docs/validation/difftest.md):
recipes are reproducible and plain arithmetic, every descriptor draws valid recipes, shrinking
keeps a difference and drops what does not show it, verdicts, permanent cases, the report.
None of these runs an original program."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

import numpy as np
import pytest

difftest = pytest.importorskip("larmorx_validation.difftest")

from larmorx_validation.difftest import (  # noqa: E402
    core,
    gen,
    inputs,
    limits,
    permanent,
    report,
    shrink,
    tools,
)
from larmorx_validation.parity.harness import CaseResult, Check  # noqa: E402


def _sha(path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


@pytest.mark.parametrize("name", tools.names())
def test_every_tool_draws_reproducible_recipes(name):
    t = tools.get(name)
    for n in range(4):
        a = core.make_recipe(t, "test", n)
        b = core.make_recipe(t, "test", n)
        assert a == b
        assert json.loads(json.dumps(a)) == a  # a recipe is plain JSON
        assert core.recipe_id(a) == core.recipe_id(b)
        words = core.arguments(a)
        for ref in [w for w in words if "{in:" in w]:
            for name_ in [s.split("}")[0] for s in ref.split("{in:")[1:]]:
                assert name_ in a["inputs"], (name, n, name_)
    assert core.make_recipe(t, "test", 0) != core.make_recipe(t, "test", 1)


@pytest.mark.parametrize(
    "name",
    ["ants-image-math", "afni-tshift", "nifti-io", "ants-apply-transforms", "ants-registration"],
)
def test_inputs_are_the_same_bytes_every_time(name, tmp_path):
    t = tools.get(name)
    recipe = core.make_recipe(t, "bytes", 3)
    for spec_name, spec in recipe["inputs"].items():
        if spec.get("kind", "nifti") == "itk-h5":
            continue  # written by larmorx itself; its bytes are the golden tests' concern
        a = inputs.materialize(spec, tmp_path / "a" / (spec_name + inputs.suffix_of(spec)))
        b = inputs.materialize(spec, tmp_path / "b" / (spec_name + inputs.suffix_of(spec)))
        assert _sha(a) == _sha(b)


def test_images_respect_the_size_limit():
    d = gen.Draw("limit")
    for _ in range(200):
        shape = gen.draw_shape(d, 4)
        assert np.prod(shape[:3]) <= gen.MAX_VOXELS and 4 <= shape[3] <= 8


def test_rational_rotations_are_rotations():
    for q in [*gen.AXIS_QUATERNIONS, (10, 1, -2, 3), (7, 0, 0, 1)]:
        r = gen.rotation(q)
        assert np.allclose(r @ r.T, np.eye(3), atol=1e-15)
        assert np.isclose(np.linalg.det(r), 1.0)


def test_header_geometry_reads_back(tmp_path):
    import nibabel as nib

    spec = {
        "shape": [5, 4, 3],
        "geometry": {
            **gen.plumb_geometry((2.0, 3.0, 4.0), (-1.0, 2.0, 3.0)),
            "quaternion": [10, 1, -2, 1],
        },
        "content": {"type": "ramp", "seed": 1, "direction": [1, 0, 0]},
        "values": {"scale": 1.0, "offset": 0.0},
        "dtype": "int16",
        "scaling": {"slope": 2.0, "inter": -5.0},
        "suffix": ".nii.gz",
    }
    path = gen.write_nifti(spec, tmp_path / "x.nii.gz")
    img = nib.load(path)
    assert img.shape == (5, 4, 3)
    assert np.allclose(img.affine, gen.affine_of(spec["geometry"]), atol=1e-5)
    assert (img.dataobj.slope, img.dataobj.inter) == (2.0, -5.0)
    raw = np.asanyarray(img.dataobj.get_unscaled())
    assert raw.dtype == np.int16


def _result(status: str, checks: list[Check], reason: str = "") -> CaseResult:
    return CaseResult("c", "difftest", "", status, checks, reason)


def test_verdicts():
    t = tools.get("ants-image-math")
    agree = _result(
        "pass",
        [
            Check("values (bit-identical)", True, value=0),
            Check("bit-identical", True, value=0, threshold="reported"),
        ],
    )
    assert core.judge(t, "port", agree).verdict == core.AGREE
    # A reported count of 1 differing value is not "identical" (1 == True in Python).
    one = _result("pass", [Check("bit-identical", True, value=1, threshold="reported")])
    assert core.judge(t, "port", one).verdict == core.DIFFER
    fail = _result("fail", [Check("header (every field)", False, detail="srow_x differs")])
    j = core.judge(t, "port", fail)
    assert j.verdict == core.DIFFER and j.signature == ("header (every field)",)
    notice = _result(
        "fail", [Check("D13 fix pass", False, detail="ANTS-38, ANTS-35: no values changed")]
    )
    j = core.judge(t, "port", notice)
    assert j.verdict == core.NOTICE and j.signature == ("notice: ANTS-38, ANTS-35",)
    # A correction that changed values without being named is a difference, not a notice.
    silent = _result(
        "fail",
        [
            Check(
                "D13 fix pass", False, detail="no defect named: 3 of 8 values, max |diff| 1 changed"
            )
        ],
    )
    assert core.judge(t, "port", silent).verdict == core.DIFFER
    # A defect named in one mode only (another correction's path): a notice, with its defect.
    one_mode = _result(
        "fail",
        [
            Check(
                "D13 notices in both modes",
                False,
                detail="fix: ['ITK-50']; reproduce: ['ITK-50', 'ANTS-90']",
            )
        ],
    )
    j = core.judge(t, "port", one_mode)
    assert j.verdict == core.NOTICE and j.signature == ("notice: ANTS-90",)
    both = _result("fail", [one_mode.checks[0], silent.checks[0]])
    assert core.judge(t, "port", both).verdict == core.DIFFER
    clean_room = tools.get("afni-tshift")
    reported = _result("pass", [Check("bit-identical", True, value=7, threshold="reported")])
    assert core.judge(clean_room, "original", reported).verdict == core.AGREE
    assert core.judge(clean_room, "replica", reported).verdict == core.DIFFER


def test_shrinking_keeps_the_difference_and_drops_the_rest():
    t = tools.get("ants-image-math")
    recipe = None
    for n in range(50):
        r = core.make_recipe(t, "shrink", n)
        if r["inputs"]["a"]["shape"][0] >= 8 and len(r["inputs"]) == 1:
            recipe = r
            break
    assert recipe is not None

    def run(r: dict) -> core.Judgement:
        # A fake difference: it shows while axis 0 has at least 3 voxels.
        if r["inputs"]["a"]["shape"][0] >= 3:
            return core.Judgement(core.DIFFER, ("values (bit-identical)",), "")
        return core.Judgement(core.AGREE)

    small, kept, last, runs = shrink.shrink(recipe, ("values (bit-identical)",), run, budget_s=60)
    assert 3 <= small["inputs"]["a"]["shape"][0] < 6
    assert (
        all(n <= 2 for n in small["inputs"]["a"]["shape"][1:3])
        or len(small["inputs"]["a"]["shape"]) < 3
    )
    assert small["inputs"]["a"]["geometry"]["quaternion"] == [1, 0, 0, 0]
    assert last is not None and runs >= len(kept)


def test_permanent_cases_round_trip(tmp_path, monkeypatch):
    t = tools.get("ants-threshold-image")
    recipe = core.make_recipe(t, "perm", 2)
    finding = tmp_path / "f.json"
    finding.write_text(
        json.dumps(
            {
                "tool": t.name,
                "implementation": "port",
                "seed": "perm",
                "n": 2,
                "signature": ["x"],
                "recipe": recipe,
                "shrunk": recipe,
            }
        )
    )
    monkeypatch.setattr(inputs, "CASES", tmp_path / "cases")
    path = permanent.promote(
        finding, "example", tier="standard", status="fixed", out_root=tmp_path / "cases"
    )
    assert path.parent.name == t.name and json.loads(path.read_text())["recipe"] == recipe
    cs = permanent.cases(t.name, "standard")
    assert [c.id for c in cs] == ["difftest/example"]
    assert permanent.cases(t.name, "smoke") == []
    payload = cs[0].payload
    assert payload.tier == "standard" and any(
        "dt:case/ants-threshold-image/example/" in a for a in payload.args
    )
    # The inputs are rebuilt from the permanent case's recipe alone.
    key = next(a for a in payload.args if "dt:case/" in a)[len("{gen:dt:") : -1]
    assert inputs.path(key).is_file()


def test_report(tmp_path):
    out = tmp_path / "night"
    (out / "findings" / "ants-n4").mkdir(parents=True)
    recipe = core.make_recipe(tools.get("ants-n4"), "r", 0)
    (out / "run.json").write_text(
        json.dumps(
            {"seed": "r", "budget_s": 60, "jobs": 1, "threads": 1, "started": "2026-10-25T01:00:00"}
        )
    )
    lines = [
        {
            "tool": "ants-n4",
            "seed": "r",
            "n": 0,
            "verdict": "agree",
            "options": ["-b"],
            "impls": {"port": {"verdict": "agree"}},
            "seconds": 1.0,
        },
        {
            "tool": "ants-n4",
            "seed": "r",
            "n": 1,
            "verdict": "differ",
            "options": ["-c"],
            "impls": {"port": {"verdict": "differ"}},
            "seconds": 2.0,
        },
        {
            "tool": "ants-n4",
            "seed": "r",
            "n": 2,
            "verdict": "notice",
            "options": [],
            "impls": {"port": {"verdict": "notice", "signature": ["notice: ANTS-38"]}},
        },
    ]
    (out / "results.jsonl").write_text("".join(json.dumps(x) + "\n" for x in lines))
    (out / "findings" / "ants-n4" / "r-00001-port.json").write_text(
        json.dumps(
            {
                "tool": "ants-n4",
                "implementation": "port",
                "seed": "r",
                "n": 1,
                "signature": ["values"],
                "recipe": recipe,
                "shrunk": recipe,
                "shrink_moves": ["a"],
            }
        )
    )
    path = report.write(out)
    text = path.read_text()
    assert "ants-n4" in text and "ANTS-38" in text and "difftest --replay" in text
    s = json.loads((out / "summary.json").read_text())
    assert s["tools"]["ants-n4"]["cases"] == 3 and s["tools"]["ants-n4"]["verdicts"]["differ"] == 1


def test_heavy_cases_are_reported_with_their_results(tmp_path):
    out = tmp_path / "night"
    out.mkdir()
    (out / "run.json").write_text(
        json.dumps({"seed": "r", "memory": {"box": "8 GiB", "case": "3 GiB"}, "started": "x"})
    )
    lines = [
        # The worker died, then each implementation ran apart: the isolated line is the case's.
        {"tool": "ants-n4", "seed": "r", "n": 0, "verdict": "memory", "detail": "worker"},
        {
            "tool": "ants-n4",
            "seed": "r",
            "n": 0,
            "verdict": "memory",
            "isolated": True,
            "options": ["-b"],
            "impls": {
                "port": {"verdict": "memory", "signature": ["larmorx exceeds the memory limit"]}
            },
        },
        {"tool": "ants-n4", "seed": "r", "n": 1, "verdict": "agree", "impls": {}},
    ]
    (out / "results.jsonl").write_text("".join(json.dumps(x) + "\n" for x in lines))
    (out / "heavy.jsonl").write_text(
        json.dumps({"tool": "ants-n4", "seed": "r", "n": 0, "options": ["-b"], "detail": "x"})
        + "\n"
    )
    report.write(out)
    s = json.loads((out / "summary.json").read_text())
    assert s["tools"]["ants-n4"]["cases"] == 2
    assert s["tools"]["ants-n4"]["verdicts"] == {"agree": 1, "memory": 1}
    assert s["heavy"][0]["result"] is None
    assert "waiting for `difftest --heavy`" in (out / "report.md").read_text()
    (out / "heavy-results.jsonl").write_text(
        json.dumps(
            {
                "tool": "ants-n4",
                "seed": "r",
                "n": 0,
                "verdict": "agree",
                "impls": {"port": {"verdict": "agree"}},
                "peak": "5 GiB",
            }
        )
        + "\n"
    )
    report.write(out)
    assert json.loads((out / "summary.json").read_text())["heavy"][0]["result"]["peak"] == "5 GiB"
    assert "now: agree" in (out / "report.md").read_text()


def test_the_heavy_queue_runs_the_quickest_first_and_only_what_is_named(tmp_path):
    from larmorx_validation.difftest import runner

    out = tmp_path / "night"
    out.mkdir()
    queue = [{"tool": "ants-n4", "seed": "r", "n": n} for n in (1, 2, 3, 4)]
    (out / "heavy.jsonl").write_text("".join(json.dumps(h) + "\n" for h in queue))
    # Case 3 is recorded already (beyond any machine); case 4 has no sizing law.
    (out / "heavy-results.jsonl").write_text(
        json.dumps({"tool": "ants-n4", "seed": "r", "n": 3, "verdict": "beyond any machine"}) + "\n"
    )
    (out / "heavy-prediction.json").write_text(
        json.dumps(
            [
                {"case": "ants-n4:r:1", "ants_gib": 1, "ants_hours": 5.0, "larmorx_hours": 1.0},
                {"case": "ants-n4:r:2", "ants_gib": 1, "ants_hours": 0.5, "larmorx_hours": 0.1},
                {"case": "ants-n4:r:4", "prediction": "no sizing law here"},
            ]
        )
    )
    assert [h["n"] for h in runner.heavy_queue(out)] == [2, 1, 4]
    assert [h["n"] for h in runner.heavy_queue(out, {"ants-n4:r:1", "ants-n4:r:3"})] == [1]
    # A case pending a larger machine waits, unless it is named.
    with (out / "heavy-results.jsonl").open("a") as f:
        f.write(
            json.dumps(
                {"tool": "ants-n4", "seed": "r", "n": 1, "verdict": "pending: larger machine"}
            )
            + "\n"
        )
    assert [h["n"] for h in runner.heavy_queue(out)] == [2, 4]
    assert [h["n"] for h in runner.heavy_queue(out, {"ants-n4:r:1"})] == [1]


def test_a_heavy_case_runs_the_recipe_of_its_night_after_the_generator_changed(tmp_path):
    """A seed and case number name a case only for the night's generator; the heavy queue runs
    later, after the generator may have changed (2026-10-25: every recipe of the first night's
    queue is drawn differently since), so it takes the recipe the night kept."""
    from larmorx_validation.difftest import runner

    out = tmp_path / "night"
    tool = tools.get("ants-n4")
    # The night's case 7 had the recipe today's generator draws for case 8.
    kept = json.loads(json.dumps(core.make_recipe(tool, "r", 8)))
    drawn = core.make_recipe(tool, "r", 7)
    assert core.recipe_id(kept) != core.recipe_id(drawn)
    h = {"tool": "ants-n4", "seed": "r", "n": 7, "recipe_id": core.recipe_id(kept)}
    # Nothing kept: the case is not run with another recipe.
    assert runner.night_recipe(out, h) is None
    # The night's finding of the case keeps it (its other findings are other cases).
    (out / "findings" / "ants-n4").mkdir(parents=True)
    other = core.make_recipe(tool, "r", 9)
    for name, recipe in (("r-00007-port.json", kept), ("r-00070-port.json", other)):
        (out / "findings" / "ants-n4" / name).write_text(json.dumps({"recipe": recipe}))
    assert core.recipe_id(runner.night_recipe(out, h)) == core.recipe_id(kept)
    # A queue line written since keeps its recipe itself; a draw that gives the id is used.
    assert runner.night_recipe(out, {**h, "recipe": other}) == other
    same = {"tool": "ants-n4", "seed": "r", "n": 7, "recipe_id": core.recipe_id(drawn)}
    assert core.recipe_id(runner.night_recipe(tmp_path / "none", same)) == core.recipe_id(drawn)


def test_a_tool_held_to_thresholds_gets_one_group_per_implementation(tmp_path):
    out = tmp_path / "night"
    (out / "findings" / "mri-hmc").mkdir(parents=True)
    (out / "findings" / "ants-n4").mkdir(parents=True)
    (out / "run.json").write_text(json.dumps({"seed": "r", "started": "x"}))
    recipe = {"groups": [], "inputs": {}}
    lines = []

    def finding(tool, impl, n, signature):
        lines.append({"tool": tool, "seed": "r", "n": n, "verdict": "differ", "impls": {}})
        f = {
            "tool": tool,
            "implementation": impl,
            "seed": "r",
            "n": n,
            "signature": signature,
            "recipe": recipe,
            "shrunk": recipe,
        }
        (out / "findings" / tool / f"r-{n}-{impl}.json").write_text(json.dumps(f))

    finding("mri-hmc", "original", 0, ["par: median difference"])
    finding("mri-hmc", "original", 1, ["par: median difference", "voxel values"])
    finding("ants-n4", "port", 2, ["corrected: bit-identical"])
    finding("ants-n4", "port", 3, ["stderr"])
    (out / "results.jsonl").write_text("".join(json.dumps(x) + "\n" for x in lines))
    report.write(out)
    s = json.loads((out / "summary.json").read_text())
    (hmc,) = s["tools"]["mri-hmc"]["differences"]
    assert hmc["cases"] == 2 and hmc["variants"] == 2
    assert hmc["by_check"] == {"par: median difference": 2, "voxel values": 1}
    assert hmc["signature"] == ["par: median difference", "voxel values"]
    # A tool held to bit-identity keeps a group per signature.
    assert len(s["tools"]["ants-n4"]["differences"]) == 2
    text = (out / "report.md").read_text()
    assert "### mri-hmc (original): beyond its record's thresholds" in text
    assert "par: median difference: 2; voxel values: 1" in text


def _memory_box_available() -> bool:
    """A systemd user scope with a memory limit, whose cgroup v2 leaves this user may make."""
    import shutil
    import subprocess

    if (
        shutil.which("systemd-run") is None
        or not Path("/sys/fs/cgroup/cgroup.controllers").exists()
    ):
        return False
    probe = (
        "c=/sys/fs/cgroup$(sed -n 's/^0:://p' /proc/self/cgroup); mkdir \"$c/probe\" && "
        'rmdir "$c/probe"'
    )
    p = subprocess.run(
        [
            "systemd-run",
            "--user",
            "--scope",
            "--quiet",
            "--collect",
            "-p",
            "MemoryMax=256M",
            "--",
            "/bin/sh",
            "-c",
            probe,
        ],
        capture_output=True,
        check=False,
    )
    return p.returncode == 0


@pytest.mark.skipif(not _memory_box_available(), reason="no systemd user scope with cgroup v2")
def test_a_case_over_the_memory_limit_is_recorded_and_the_night_goes_on(tmp_path):
    """A real night of the harness's self-test tool, in a box it makes for itself: a case whose
    original (a child process) or larmorx (the worker) fills 4 GiB under a limit of 0.75 GiB is
    stopped by the kernel, recorded with the verdict ``memory`` on the right side and queued as
    a heavy case, and every case of the night is recorded."""
    import os
    import subprocess
    import sys

    from larmorx_validation.difftest.tools import selftest

    out = tmp_path / "night"
    cases = 8
    who = {
        n: core.make_recipe(tools.get("selftest-memory"), "box", n)["extra"]["who"]
        for n in range(cases)
    }
    assert set(who.values()) == set(selftest.WHO), "the seed must draw every kind of case"
    env = {**os.environ, "PYTHONPATH": os.pathsep.join(p for p in sys.path if p)}
    env.pop("LARMORX_DIFFTEST_BOXED", None)
    env.pop("LARMORX_DIFFTEST_CGROUP", None)
    p = subprocess.run(
        [
            sys.executable,
            "-m",
            "larmorx_validation",
            "difftest",
            "--tools",
            "selftest-memory",
            "--max-cases",
            str(cases),
            "--jobs",
            "1",
            "--budget",
            "0.1",
            "--seed",
            "box",
            "--memory-gib",
            "0.75",
            "--box-gib",
            "2",
            "--out",
            str(out),
        ],
        capture_output=True,
        text=True,
        env=env,
        timeout=600,
        check=False,
    )
    assert p.returncode == 0, p.stderr[-2000:]
    assert "DIFFTEST FINISHED" in p.stdout
    _, lines = report.load(out)
    by_n = {ln["n"]: ln for ln in lines}
    assert sorted(by_n) == list(range(cases)), "every case is recorded; the night went on"
    for n, kind in who.items():
        e = by_n[n]["impls"]["port"]
        if kind == "nobody":
            assert e["verdict"] == "agree", (n, e)
        else:
            assert e["verdict"] == "memory", (n, e)
            assert e["signature"] == [f"{kind} exceeds the memory limit"], (n, e)
            assert "exceeds 0.75 GiB" in e["detail"], (n, e)
    heavy = {h["n"] for h in report.heavy(out)}
    assert heavy == {n for n, kind in who.items() if kind != "nobody"}
    info = json.loads((out / "run.json").read_text())
    # The box it made for itself, or the one it already ran in (the gates run in a box).
    enclosing = limits.MemoryBox.find()
    box = limits.gib(enclosing.limit) if enclosing is not None else "2 GiB"
    assert info["memory"]["case"] == "0.75 GiB" and info["memory"]["box"] == box
    assert "ended" in info


def test_most_cases_are_realistic_and_the_extreme_share_is_labelled():
    """User, 2026-10-25: "reasonable size tests". Most cases are realistic (millimetres or no
    units, voxels of 0.2 to 10 mm, at most 32 voxels per axis); a small share, labelled
    ``extreme`` in the case's features, may have odd units and voxel sizes."""
    n_cases, extreme = 300, 0
    for name in ("ants-n4", "ants-resample-image-by-spacing", "ants-smooth-image"):
        tool = tools.get(name)
        for n in range(n_cases):
            r = core.make_recipe(tool, "realistic", n)
            is_extreme = r["features"]["extreme"]
            extreme += is_extreme
            for spec in r["inputs"].values():
                if spec.get("kind", "nifti") != "nifti" or "geometry" not in spec:
                    continue
                assert max(spec["shape"][:3]) <= 32, (name, n)
                if not is_extreme:
                    g = spec["geometry"]
                    assert g["units"] in ("mm", "unknown"), (name, n, g["units"])
                    assert all(0.2 <= s <= 10.0 for s in g["spacing"]), (name, n, g["spacing"])
    share = extreme / (3 * n_cases)
    assert 0.05 < share < 0.15, share


def test_the_sizing_laws_and_a_case_predicted_over_the_limit():
    """The case that took the machine down (ants-n4:20261025:24) as the sizing law gives it,
    and the pre-check of a case over the case's limit (checked as a guard case, not run)."""
    from larmorx_validation.difftest import runner, sizing

    p = sizing.predict_from(
        "ants-n4",
        ["-d", "4", "-i", "{in:a}", "-s", "3", "-b", "[20,3]", "-o", "{out}"],
        [14, 28, 23, 7],
        [4000.0, 1000.0, 1000.0, 3.0],
    )
    assert p["sizing"]["lattice_last"] == [20803, 10803, 8803, 11]
    assert p["ants_gib"] > 1e5 and p["larmorx_gib"] < p["ants_gib"]
    ribs = sizing.predict_from(
        "ants-resample-image-by-spacing",
        ["3", "{in:a}", "{out}", "0.5", "0.5", "0.5"],
        [32, 32, 32],
        [1000.0, 1000.0, 1000.0],
    )
    assert ribs["sizing"]["output"] == [64000, 64000, 64000]
    ball = sizing.predict_from(
        "afni-unifize",
        ["-EPI", "-prefix", "uni.nii", "{in:input}"],
        [8, 8, 8, 4],
        [0.003, 0.003, 0.003, 2.0],
    )
    assert round(ball["sizing"]["radius"]) == 6100 and ball["ants_gib"] > 8000
    # A realistic case of a sized tool fits; an extreme one may be predicted over the limit.
    tool = tools.get("ants-n4")
    over = None
    for n in range(400):
        r = core.make_recipe(tool, "guard", n)
        predicted = runner.predict_case("ants-n4", r)
        if not r["features"]["extreme"]:
            assert predicted is None, (n, predicted)
        elif predicted is not None:
            over = predicted
            break
    assert over is not None and max(over["ants_gib"], over["larmorx_gib"]) > over["limit_gib"]
    # An implementation without a guard is not run: queued, its prediction recorded.
    e = runner.guard_case(tools.get("afni-unifize"), {}, "original", {**over, "law": "x"})
    assert e["verdict"] == "memory" and "no guard" in e["detail"]
