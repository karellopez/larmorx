# SPDX-License-Identifier: Apache-2.0
"""The FreeSurfer replay tool (``larmorx_validation.freesurfer``): FreeSurfer's files compared
with their provenance left out, a recording read and a step's subject directory rebuilt, the
cache's FreeSurfer family. Synthetic files only (nibabel writes them; the provenance records are
built as 8.2.0's outputs hold them); no FreeSurfer installation is needed."""

from __future__ import annotations

import gzip
import json
import sys
from pathlib import Path

import numpy as np
import pytest

nib = pytest.importorskip("nibabel")

from larmorx_validation.freesurfer import files as ff  # noqa: E402
from larmorx_validation.freesurfer import recording as fr  # noqa: E402
from larmorx_validation.freesurfer import replay as rp  # noqa: E402
from larmorx_validation.oracle_cache import compare, key  # noqa: E402

REC = "/data/runs/x-v8/subjects"
REP = "/elsewhere/replay/work/x-v8/0011-prog-t1/subjects"


def cmdline(program: str, args: str, when: str, user: str = "alice") -> bytes:
    return (
        f"ProgramName: {program}  ProgramArguments: {program} {args}  ProgramVersion: 8.2.0  "
        f"TimeStamp: {when}-GMT  BuildTime: Mar 14 2026 12:21:06  BuildStamp: fs-8.2.0  "
        f"User: {user}  Machine: host-a  Platform: Linux  PlatformVersion: 6.8.0  "
        "CompilerName: GCC  CompilerVersion: 8.4.0"
    ).encode()


def record(tag: int, text: bytes) -> bytes:
    """A provenance record as 8.2.0's volumes hold it: 4-byte number, 8-byte length, text."""
    return tag.to_bytes(4, "big") + len(text).to_bytes(8, "big") + text


def mgz(path: Path, data: np.ndarray, tail: bytes) -> Path:
    img = nib.MGHImage(data, np.eye(4))
    path.write_bytes(gzip.compress(img.to_bytes() + tail))
    return path


def ctx() -> ff.Context:
    return ff.Context(pairs=((REC, REP),), names=("alice", "host-a"))


def test_a_volume_differing_only_in_provenance_is_the_same_result(tmp_path):
    data = np.arange(4 * 5 * 6, dtype=np.float32).reshape(4, 5, 6)
    a = mgz(
        tmp_path / "a.mgz",
        data,
        record(31, f"{REC}/s/mri/transforms/talairach.xfm\0".encode())
        + record(3, cmdline("mri_normalize", f"{REC}/s/mri/nu.mgz T1.mgz", "2026/11/12-11:28:21")),
    )
    b = mgz(
        tmp_path / "b.mgz",
        data,
        record(31, f"{REP}/s/mri/transforms/talairach.xfm\0".encode())
        + record(3, cmdline("mri_normalize", f"{REP}/s/mri/nu.mgz T1.mgz", "2026/11/13-09:00:01")),
    )
    c = ff.compare(a, b, ctx())
    assert c.verdict == ff.PROVENANCE and c.kind == "mgh", c
    assert "command-line" in c.detail and "path" in c.detail
    # one voxel changed: the values are measured
    d = mgz(
        tmp_path / "d.mgz",
        np.where(data == 7, 7.5, data).astype(np.float32),
        record(31, f"{REP}/s/mri/transforms/talairach.xfm\0".encode())
        + record(3, cmdline("mri_normalize", f"{REP}/s/mri/nu.mgz T1.mgz", "2026/11/13-09:00:01")),
    )
    c = ff.compare(a, d, ctx())
    assert c.verdict == ff.DIFFERS and "voxels: 1 of 120 differ, by at most 0.5" in c.detail, c


def test_a_length_field_must_follow_the_masked_run(tmp_path):
    text_a, text_b = f"{REC}/x.xfm\0".encode(), f"{REP}/x.xfm\0".encode()
    good = ff.masked_binary(record(31, text_a), record(31, text_b), ctx())
    assert good[0], good
    wrong = (31).to_bytes(4, "big") + (len(text_b) + 1).to_bytes(8, "big") + text_b
    bad = ff.masked_binary(record(31, text_a), wrong, ctx())
    assert not bad[0], bad
    extra = ff.masked_binary(record(3, b"x" + text_a), record(3, b"y" + text_b), ctx())
    assert not extra[0], extra  # a byte outside the runs differs


def test_surfaces_stamp_and_volume_path_are_provenance(tmp_path):
    coords = np.random.default_rng(1).normal(size=(10, 3)).astype(np.float32)
    faces = np.array([[0, 1, 2], [2, 3, 4], [5, 6, 7]], dtype=np.int32)
    info = {
        "head": np.array([2, 0, 20], dtype=np.int32),
        "valid": "1  # volume info valid",
        "filename": f"{REC}/s/mri/wm.mgz",
        "volume": np.array([256, 256, 256]),
        "voxelsize": np.array([1.0, 1.0, 1.0]),
        "xras": np.array([-1.0, 0.0, 0.0]),
        "yras": np.array([0.0, 0.0, -1.0]),
        "zras": np.array([0.0, 1.0, 0.0]),
        "cras": np.array([0.0, 0.0, 0.0]),
    }
    a, b, m = tmp_path / "lh.white", tmp_path / "b" / "lh.white", tmp_path / "m" / "lh.white"
    b.parent.mkdir()
    m.parent.mkdir()
    nib.freesurfer.write_geometry(str(a), coords, faces, "created by alice on Thu Nov 12 12:55:29 2026", info)
    info_b = dict(info, filename=f"{REP}/s/mri/wm.mgz")
    nib.freesurfer.write_geometry(str(b), coords, faces, "created by alice on Fri Nov 13 09:00:00 2026", info_b)
    c = ff.compare(a, b, ctx())
    assert c.verdict == ff.PROVENANCE and c.kind == "surface", c
    moved = coords.copy()
    moved[3] += np.float32(0.25)
    nib.freesurfer.write_geometry(str(m), moved, faces, "created by alice on Fri Nov 13 09:00:00 2026", info_b)
    c = ff.compare(a, m, ctx())
    assert c.verdict == ff.DIFFERS and "1 of 10 vertices moved" in c.detail and "same faces" in c.detail, c


def test_curvatures_and_annotations_are_measured(tmp_path):
    v = np.linspace(-1, 1, 50).astype(np.float32)
    a, b = tmp_path / "lh.curv", tmp_path / "b.curv"
    nib.freesurfer.write_morph_data(str(a), v)
    w = v.copy()
    w[[3, 9]] += 1e-3
    nib.freesurfer.write_morph_data(str(b), w)
    c = ff.compare(a, b, ctx(), name="lh.curv")
    assert c.kind == "curv" and c.verdict == ff.DIFFERS and "values: 2 of 50 differ" in c.detail, c
    labels = np.array([0, 1, 1, 2, 0], dtype=np.int32)
    ctab = np.array([[10, 20, 30, 0, 0], [40, 50, 60, 0, 0], [70, 80, 90, 0, 0]], dtype=np.int32)
    names = [b"unknown", b"a", b"b"]
    x, y = tmp_path / "lh.x.annot", tmp_path / "lh.y.annot"
    nib.freesurfer.write_annot(str(x), labels, ctab, names, fill_ctab=True)
    nib.freesurfer.write_annot(str(y), np.array([0, 1, 2, 2, 0], dtype=np.int32), ctab, names, fill_ctab=True)
    c = ff.compare(x, y, ctx())
    assert c.kind == "annot" and "vertex labels: 1 of 5 differ" in c.detail, c


def test_text_files_mask_their_provenance_lines(tmp_path):
    lta = (
        "# transform file transforms/talairach.lta\n# created by alice on {when}\n\n"
        "type      = 0 # LINEAR_VOX_TO_VOX\n1 4 4\n1.0 0.0 0.0 {t}\n"
        "src volume info\nfilename = {d}/s/mri/nu.mgz\n"
    )
    a, b, c_ = tmp_path / "a.lta", tmp_path / "b.lta", tmp_path / "c.lta"
    a.write_text(lta.format(when="Thu Nov 12 12:32:35 2026", t="-6.29", d=REC))
    b.write_text(lta.format(when="Fri Nov 13 01:02:03 2026", t="-6.29", d=REP))
    c_.write_text(lta.format(when="Fri Nov 13 01:02:03 2026", t="-6.30", d=REP))
    assert ff.compare(a, b, ctx()).verdict == ff.PROVENANCE
    c = ff.compare(a, c_, ctx())
    assert c.verdict == ff.DIFFERS and "numbers by at most 0.01" in c.detail, c
    stats = (
        "# CreationTime {t}\n# cmdline mris_anatomical_stats -th3 {a}\n# hostname host-a\n"
        "# user     alice\n# SUBJECTS_DIR {d}\n# AnnotationFileTimeStamp {t}\n"
        "# Measure Cortex, NumVert, Number of Vertices, 118277, unitless\n"
    )
    s1, s2 = tmp_path / "lh.aparc.stats", tmp_path / "x" / "lh.aparc.stats"
    s2.parent.mkdir()
    s1.write_text(stats.format(t="2026/11/12-12:37:24-GMT", a="x", d=REC))
    s2.write_text(stats.format(t="2026/11/13 01:00:00", a="y", d=REP))
    assert ff.compare(s1, s2, ctx()).verdict == ff.PROVENANCE


def test_logs_mask_times_and_report_what_is_left(tmp_path):
    a, b, c_ = tmp_path / "x.log", tmp_path / "y.log", tmp_path / "z.log"
    a.write_text("mri_fill done\nmri_fill took 0.85 minutes\n@#@FSTIME 2026:11:12:12:17:49 a N 1 e 0.8\n")
    b.write_text("mri_fill done\nmri_fill took 0.91 minutes\n@#@FSTIME 2026:11:13:01:00:00 a N 1 e 0.9\n")
    c_.write_text("mri_fill failed\nmri_fill took 0.91 minutes\n")
    assert ff.compare(a, b, ctx()).verdict == ff.PROVENANCE
    c = ff.compare(a, c_, ctx())
    assert c.verdict == ff.LOG and not c.same, c


def test_the_cache_compares_freesurfer_files_provenance_aside(tmp_path):
    data = np.ones((3, 3, 3), dtype=np.uint8)
    a = mgz(tmp_path / "a.mgz", data, record(3, cmdline("p", "/x/y/in.mgz", "2026/11/12-11:28:21")))
    b = mgz(tmp_path / "b.mgz", data, record(3, cmdline("p", "/z/in.mgz", "2026/11/13-01:00:00")))
    assert compare.same_file(a, b, formats="freesurfer")[0]
    assert not compare.same_file(a, b)[0]
    with pytest.raises(ValueError):
        compare.same_file(a, b, formats="nonsense")


def test_the_freesurfer_family_changes_no_other_key():
    assert key.env_matters("FS_LICENSE", "freesurfer")
    assert key.env_matters("SUBJECTS_DIR", "freesurfer")
    assert key.env_matters("PATH", "freesurfer")
    assert key.env_matters("OMP_NUM_THREADS", "freesurfer")
    # a run naming no family keeps its key: no FreeSurfer variable joins it
    for name in ("FS_LICENSE", "SUBJECTS_DIR", "FREESURFER_HOME", "PATH"):
        assert not key.env_matters(name, None), name
    assert not key.env_matters("FS_LICENSE", "itk")


def test_parse_env_takes_variables_and_the_last_value():
    text = "date line\nFOO=1\nBASH_FUNC_x%%=() {  a;\n out=$x;\n}\nFOO=2\nSUBJECTS_DIR=/d/s\n"
    assert fr.parse_env(text) == {"FOO": "2", "SUBJECTS_DIR": "/d/s"}


def _fake_recording(root: Path) -> fr.Recording:
    """A two-step recording of subject ``s`` (a symbolic link, a dated log, recon-all's
    environment) as the recorder leaves one."""
    blobs = root / "record" / "blobs"
    store: dict[bytes, str] = {}

    def put(data: bytes) -> str:
        import hashlib

        sha = hashlib.sha256(data).hexdigest()
        p = blobs / sha[:2] / sha
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(data)
        store[data] = sha
        return sha

    sd = str(root / "subjects")
    env = (
        f"some date\nFREESURFER_HOME=/opt/fs\nSUBJECTS_DIR={sd}\nPATH=/opt/fs/bin:/usr/bin\n"
        "USER=alice\nHOST=host-a\nCLAUDE_TOKEN=secret\nOMP_NUM_THREADS=4\nLC_NUMERIC=es_ES.UTF-8\n"
        f"LARMORX_FS_RECORD={root}/record\nLARMORX_FS_SUBJDIR={sd}/s\n"
    ).encode()
    e, t1, link = put(env), put(b"T1 volume"), put(b"surface")
    steps = root / "record" / "steps"
    steps.mkdir(parents=True)
    base_env = {"SUBJECTS_DIR": sd, "OMP_NUM_THREADS": "4", "FREESURFER_HOME": "/opt/fs"}
    before = {"scripts/recon-all.env": e, "mri/T1.mgz": t1, "surf/lh.white.preaparc": link, "surf/lh.white": link}
    step = {
        "index": 2,
        "argv": ["mris_place_surface", "--threads", "4", "--i", f"{sd}/s/surf/lh.white", "--o", "lh.pial"],
        "cwd": f"{sd}/s/mri",
        "env": base_env,
        "start": 2e9,
        "seconds": 1.5,
        "exit": 0,
        "before": before,
        "between": {},
        "between_deleted": [],
        "outputs": {"surf/lh.pial": put(b"pial"), "scripts/p.Y2026.M11.D12.H12.M19.log": put(b"log took 3 s\n")},
        "deleted": [],
    }
    (steps / "0002-mris_place_surface.json").write_text(json.dumps(step))
    (root / "RUN_INFO.txt").write_text("command: recon-all -s s -all -no-v8\nsubject: s\nEXIT 0\n")
    fin = root / "subjects" / "s"
    (fin / "surf").mkdir(parents=True)
    (fin / "mri").mkdir()
    (fin / "surf" / "lh.white.preaparc").write_bytes(b"surface")
    (fin / "surf" / "lh.white").symlink_to("lh.white.preaparc")
    return fr.Recording(root)


@pytest.mark.skipif(sys.platform != "linux", reason="GNU find's birth times (Linux)")
def test_a_step_is_rebuilt_and_compared(tmp_path):
    rec = _fake_recording(tmp_path / "run")
    assert rec.stream == "classic" and rec.subject == "s"
    step = rec.select("mris_place_surface")
    assert rec.select("2") == step and step.after()["surf/lh.pial"]
    rb = rp.rebuild(rec, step, tmp_path / "replay", threads=1)
    assert (rb.subject_dir / "mri" / "T1.mgz").read_bytes() == b"T1 volume"
    assert (rb.subject_dir / "surf" / "lh.white").is_symlink()
    assert rb.argv[:3] == ["mris_place_surface", "--threads", "1"]
    assert rb.argv[4] == f"{rb.subjects_dir}/s/surf/lh.white"
    assert rb.cwd == rb.subject_dir / "mri"
    assert rb.env["SUBJECTS_DIR"] == str(rb.subjects_dir) and rb.env["OMP_NUM_THREADS"] == "1"
    assert rb.env["LC_NUMERIC"] == "es_ES.UTF-8" and "CLAUDE_TOKEN" not in rb.env
    assert not any(k.startswith("LARMORX_") for k in rb.env)  # never the recorder's
    recorded = rp.rebuild(rec, step, tmp_path / "replay2", threads=None)
    assert recorded.argv[2] == "4" and recorded.threads == 4
    # the program "runs": the same pial surface, its log at another time
    (rb.subject_dir / "surf" / "lh.pial").write_bytes(b"pial")
    (rb.subject_dir / "scripts" / "p.Y2027.M01.D01.H00.M00.log").write_bytes(b"log took 9 s\n")

    class Done:
        returncode, seconds, cached = 0, 0.1, False

    report = rp.compare_step(rb, Done())
    assert report.verdict == "reproduced", report.to_dict()
    assert {f.comparison.verdict for f in report.files} == {ff.IDENTICAL, ff.PROVENANCE}
    (rb.subject_dir / "surf" / "lh.pial").write_bytes(b"other")
    assert rp.compare_step(rb, Done()).verdict == "differs"
    # a directory that is not a replay's is never emptied
    (tmp_path / "keep").mkdir()
    (tmp_path / "keep" / "x").write_text("x")
    with pytest.raises(FileExistsError):
        rp.rebuild(rec, step, tmp_path / "keep")
