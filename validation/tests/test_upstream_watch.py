# SPDX-License-Identifier: Apache-2.0
"""The upstream watch (``scripts/orchestration/upstream_watch.py``) on a fake workspace: the
current versions read from ``UPSTREAM.md``, the newest releases from fake lookups (no network),
the named files compared between two fake release trees, the report and its JSON."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import sys
from pathlib import Path

import pytest

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "orchestration" / "upstream_watch.py"


@pytest.fixture(scope="module")
def watch():
    spec = importlib.util.spec_from_file_location("upstream_watch", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    sys.modules["upstream_watch"] = module  # dataclasses look their module up
    spec.loader.exec_module(module)
    return module


UPSTREAM_MD = """# Upstream versions

## Ported sources

| Project | Licence | Port from (tag → commit) | Version in fMRIPrep's image | Notes |
|---|---|---|---|---|
| ANTs | Apache-2.0 | `v2.6.5` → `fdce4d2f84b6` | 2.6.2 | newest |
| AFNI | public domain | `AFNI_1.0.00` → `b1e12b26dae2` | AFNI_1.0.00 | x |
| CORE-MATH | MIT | `master` → `040ee482a8ca` (no releases) | – | x |
| SciPy | BSD-3-Clause | `v1.15.2` → `0f1fd4a7268b` | 1.15.2 | x |

## Behaviour references

| Project | Licence | Version | Used for |
|---|---|---|---|
| numpy | BSD-3-Clause | 2.3.5 (development venv; sdist SHA-256 `784db1dcdab5`) | x |

## FSL

The oracle is the FSL 6.0.7.17
conda installation in `~/fsl`.
"""


def make_repo(root: Path) -> Path:
    repo = root / "repo"
    (repo / "crates-gpl" / "larmorx-gpl-afni").mkdir(parents=True)
    (repo / "UPSTREAM.md").write_text(UPSTREAM_MD, encoding="utf-8")
    (repo / "crates-gpl" / "larmorx-gpl-afni" / "PROVENANCE.md").write_text(
        "Translated from AFNI.\n\n| `src/a.c` | `src/b.c` | `src/c.c` | `thd_d.c` | `itkFoo.h/.hxx` |\n",
        encoding="utf-8",
    )
    nc = repo / "crates-nc" / "fsl-source"
    nc.mkdir(parents=True)
    rows = ["package\tversion\tfile\tsha256"]
    for name, text in (("x.cc", b"old x\n"), ("y.cc", b"same y\n")):
        rows.append(f"fsl-pkg\t1.0\t{name}\t{hashlib.sha256(text).hexdigest()}")
    (nc / "MANIFEST.tsv").write_text("\n".join(rows) + "\n", encoding="utf-8")
    return repo


def make_tags(ws: Path) -> None:
    old, new = ws / "tags" / "AFNI-1.0.00" / "src", ws / "tags" / "AFNI-1.1.00" / "src"
    for d in (old, new, old / "sub", new / "sub"):
        d.mkdir(parents=True, exist_ok=True)
    header_old = "/* released under the Gnu General Public License, Version 2 */\n"
    header_new = "/* released under the Creative Commons Attribution License (CC BY 4.0) */\n"
    (old / "a.c").write_text(header_old + "int a = 1;\n", encoding="utf-8")
    (new / "a.c").write_text(header_new + "int a = 1;\n", encoding="utf-8")  # licence only
    (old / "b.c").write_text("int b = 1;\n", encoding="utf-8")
    (new / "b.c").write_text("int b = 2;\n", encoding="utf-8")  # code
    (old / "c.c").write_text("int c;\n", encoding="utf-8")  # removed in new
    (old / "sub" / "thd_d.c").write_text("same\n", encoding="utf-8")
    (new / "sub" / "thd_d.c").write_text("same\n", encoding="utf-8")
    (old / "itkFoo.hxx").write_text("x\n", encoding="utf-8")
    (new / "itkFoo.hxx").write_text("y\n", encoding="utf-8")


def fake_lookup(watch, ws: Path):
    def remote(url, *args):
        if "afni" in url:
            return [
                ("1" * 40, "refs/tags/AFNI_1.0.00"),
                ("2" * 40, "refs/tags/AFNI_1.1.00"),
                ("3" * 40, "refs/tags/AFNI_1.1.00^{}"),
                ("4" * 40, "refs/tags/latest"),
            ]
        if "ANTs" in url:
            return [("1" * 40, "refs/tags/v2.6.5"), ("2" * 40, "refs/tags/v2.6.5rc1")]
        if "core-math" in url:
            return [("c7100a074285" + "0" * 28, "HEAD")]
        raise OSError("unreachable")

    def http(url):
        if "pypi.org/pypi/scipy" in url:
            return json.dumps({"info": {"version": "1.15.2"}, "releases": {"1.15.2": [{}]}})
        if "pypi.org/pypi/numpy" in url:
            rel = {"2.3.5": [{}], "2.5.4": [{}], "2.6.0rc1": [{}], "2.4.0": []}
            return json.dumps({"info": {"version": "2.5.4"}, "releases": rel})
        if url.endswith("manifest.json"):
            return json.dumps({"versions": {"6.0.7.17": [], "6.0.7.23": [], "latest": "6.0.7.23"}})
        if url.endswith("fsl-6.0.7.17_linux-64.yml"):
            return "dependencies:\n - fsl-pkg 1.0\n - fslpy 3.21.1\n"
        if url.endswith("fsl-6.0.7.23_linux-64.yml"):
            return "dependencies:\n - fsl-pkg 1.1\n - fslpy 3.21.1\n"
        if "AFNI.version" in url:
            return "AFNI_1.1.00\n"
        raise OSError(f"no fake for {url}")

    return watch.Lookup(offline=False, workspace=ws, http=http, remote=remote)


def test_current_versions(watch):
    cur = watch.current_versions(UPSTREAM_MD)
    assert cur["ANTs"] == "v2.6.5"
    assert cur["AFNI"] == "AFNI_1.0.00"
    assert cur["CORE-MATH"] == "040ee482a8ca"
    assert cur["SciPy"] == "v1.15.2"
    assert cur["numpy"] == "2.3.5"  # the version, not the sdist's hash
    assert cur["FSL"] == "6.0.7.17"


def test_watch_on_a_fake_workspace(watch, tmp_path):
    repo = make_repo(tmp_path)
    ws = tmp_path / "ws"
    make_tags(ws)
    fsl_root = tmp_path / "home"
    src = fsl_root / "fsl-6.0.7.23" / "src" / "fsl-pkg"
    src.mkdir(parents=True)
    (src / "x.cc").write_bytes(b"new x\n")
    (src / "y.cc").write_bytes(b"same y\n")
    results = {r.name: r for r in watch.run_watch(repo, fake_lookup(watch, ws), fsl_root)}

    ants = results["ANTs"]
    assert (ants.status, ants.newest) == ("newest", "2.6.5")  # the rc tag is no release

    afni = results["AFNI"]
    assert (afni.status, afni.newest, afni.newer) == ("newer", "1.1.00", ["1.1.00"])
    files = afni.files
    assert files["named"] == 5 and files["unchanged"] == 1
    changed = {c["file"]: c for c in files["changed"]}
    assert set(changed) == {"src/a.c", "src/b.c", "src/itkFoo.hxx"}
    assert changed["src/a.c"]["licence_only"] and not changed["src/b.c"]["licence_only"]
    assert changed["src/b.c"]["crates"] == ["larmorx-gpl-afni"]
    assert [r["file"] for r in files["removed"]] == ["src/c.c"]
    assert "AFNI.version says AFNI_1.1.00" in afni.source

    core = results["CORE-MATH"]
    assert core.status == "newer" and "040ee482a8ca" in core.fetch

    assert results["SciPy"].status == "newest"
    numpy = results["numpy"]
    assert (numpy.status, numpy.newest, numpy.newer) == ("newer", "2.5.4", ["2.5.4"])
    assert "numpy-2.3.5" in numpy.fetch  # no release trees: what to fetch

    fsl = results["FSL"]
    assert fsl.status == "newer" and fsl.packages == {"fsl-pkg": ["1.0", "1.1"]}
    assert [c["file"] for c in fsl.files["changed"]] == ["fsl-pkg/x.cc"]
    assert fsl.files["unchanged"] == 1

    for name in ("ITK", "FreeSurfer", "OpenBLAS"):  # not in this UPSTREAM.md
        assert results[name].error == "not named in UPSTREAM.md"

    text = watch.render(list(results.values()), "2026-10-29")
    assert (
        "| AFNI | AFNI_1.0.00 | 1.1.00 | **newer release** | 3 of 5 (1 licence text only)" in text
    )
    assert "removed: `src/c.c`" in text


def test_offline_reads_the_reference_clone(watch, tmp_path):
    ws = tmp_path / "ws"
    git = ws / "afni" / ".git"
    (git / "refs" / "tags").mkdir(parents=True)
    (git / "packed-refs").write_text(
        "# pack-refs\n" + "1" * 40 + " refs/tags/AFNI_1.0.00\n" + "^" + "2" * 40 + "\n",
        encoding="utf-8",
    )
    (git / "refs" / "tags" / "AFNI_1.2.00").write_text("3" * 40 + "\n", encoding="utf-8")
    lookup = watch.Lookup(offline=True, workspace=ws)
    afni = next(u for u in watch.UPSTREAMS if u.name == "AFNI")
    tags, newest, source = lookup.newest(afni)
    assert (newest, source) == ("AFNI_1.2.00", "the reference clone afni")
    assert tags == ["AFNI_1.2.00", "AFNI_1.0.00"]


def test_main_writes_markdown_and_json(watch, tmp_path, monkeypatch):
    repo = make_repo(tmp_path)
    out = tmp_path / "out"
    real = fake_lookup(watch, tmp_path / "ws")
    monkeypatch.setattr(watch, "Lookup", lambda offline, workspace: real)
    assert (
        watch.main(
            [
                "--repo",
                str(repo),
                "--workspace",
                str(tmp_path / "ws"),
                "--out",
                str(out),
                "--fsl-root",
                str(tmp_path / "none"),
                "--only",
                "AFNI",
                "FSL",
            ]
        )
        == 0
    )
    md = list(out.glob("upstream-watch-*.md"))
    js = list(out.glob("upstream-watch-*.json"))
    assert len(md) == 1 and len(js) == 1
    data = json.loads(js[0].read_text(encoding="utf-8"))
    names = [u["name"] for u in data["upstreams"]]
    assert names == ["AFNI", "FSL"]
    fsl = data["upstreams"][1]
    assert "install FSL 6.0.7.23" in fsl["fetch"]
