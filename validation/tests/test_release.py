# SPDX-License-Identifier: Apache-2.0
"""The release's helpers (scripts/release/): one version everywhere, the licence expressions,
PyPI's descriptions and the companions' source distributions."""

from __future__ import annotations

import importlib.util
import re
import shutil
import sys
import tomllib
from pathlib import Path
from types import ModuleType

import pytest

ROOT = Path(__file__).resolve().parents[2]
RELEASE = ROOT / "scripts" / "release"


def load(name: str) -> ModuleType:
    spec = importlib.util.spec_from_file_location(f"release_{name}", RELEASE / f"{name}.py")
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


set_version = load("set_version")
licences = load("licences")
pypi_readme = load("pypi_readme")
companion_sdist = load("companion_sdist")


def test_one_version_everywhere():
    """The three workspaces, their locks and the [all] pins hold one version."""
    assert set_version.check() == []
    v = set_version.version()
    assert set_version.check(f"v{v}") == []
    assert set_version.check("v0.0.0") != []


@pytest.mark.parametrize(
    ("pep440", "cargo"),
    [
        ("0.1.0", "0.1.0"),
        ("0.1.0rc1", "0.1.0-rc.1"),
        ("0.2.0a3", "0.2.0-alpha.3"),
        ("1.10.0b12", "1.10.0-beta.12"),
    ],
)
def test_version_forms(pep440, cargo):
    assert set_version.to_cargo(pep440) == cargo
    assert set_version.to_pep440(cargo) == pep440
    packaging = pytest.importorskip("packaging.version")
    assert str(packaging.Version(cargo)) == pep440  # maturin's conversion is PEP 440's normal form


@pytest.mark.parametrize(
    "bad", ["0.1", "0.1.0.dev1", "0.1.0.post1", "0.1.0rc", "v0.1.0", "0.1.0-rc.1"]
)
def test_versions_larmorx_does_not_release(bad):
    with pytest.raises(ValueError):
        set_version.to_cargo(bad)


def test_set_version_rewrites_every_place(tmp_path, monkeypatch):
    """On a copy of the manifests: setting a version and setting it back changes nothing else."""
    for ws in ("", "crates-gpl/", "crates-nc/"):
        for name in ("Cargo.toml", "Cargo.lock"):
            (tmp_path / ws).mkdir(parents=True, exist_ok=True)
            shutil.copy(ROOT / ws / name, tmp_path / ws / name)
    for manifest in [*ROOT.glob("crates*/*/Cargo.toml")]:
        rel = manifest.relative_to(ROOT)
        (tmp_path / rel).parent.mkdir(parents=True, exist_ok=True)
        shutil.copy(manifest, tmp_path / rel)
    shutil.copy(ROOT / "pyproject.toml", tmp_path / "pyproject.toml")
    monkeypatch.setattr(set_version, "ROOT", tmp_path)
    monkeypatch.setattr(
        set_version,
        "WORKSPACES",
        {
            "larmorx": tmp_path,
            "larmorx-gpl": tmp_path / "crates-gpl",
            "larmorx-nc": tmp_path / "crates-nc",
        },
    )
    before = {p: p.read_bytes() for p in tmp_path.rglob("*") if p.is_file()}
    current = set_version.version()
    changed = set_version.set_version("9.8.7b6")
    assert {p.name for p in changed} == {"Cargo.toml", "Cargo.lock", "pyproject.toml"}
    assert len(changed) == 7
    assert set_version.check("v9.8.7b6") == []
    assert '"larmorx-nc==9.8.7b6"' in (tmp_path / "pyproject.toml").read_text(encoding="utf-8")
    lock = (tmp_path / "crates-nc" / "Cargo.lock").read_text(encoding="utf-8")
    assert 'name = "larmorx-core"\nversion = "9.8.7-beta.6"' in lock
    set_version.set_version(current)
    assert {p: p.read_bytes() for p in before} == before


def test_licence_expressions_cover_the_contents():
    """Each pyproject's licence expression names every licence of what the package holds."""
    if shutil.which("cargo") is None:
        pytest.skip("cargo is needed (cargo metadata)")
    assert licences.main(["--check"]) == 0


def test_licence_expressions_are_spdx():
    lic = pytest.importorskip("packaging.licenses")
    for package in licences.PACKAGES:
        expr = licences.declared(package)
        assert lic.canonicalize_license_expression(expr) == expr
        assert expr.split(" AND ")[0] == package.own


def test_licence_expression_parser():
    assert licences.identifiers("(MIT OR Apache-2.0) AND Unicode-3.0") == {
        "MIT",
        "Apache-2.0",
        "Unicode-3.0",
    }
    assert licences.additions("MIT OR Apache-2.0", {"Apache-2.0"}) == set()
    assert licences.additions("Zlib OR Apache-2.0 OR MIT", {"MIT"}) == set()
    assert licences.additions("Zlib", {"MIT"}) == {"Zlib"}
    assert licences.additions("MIT/Apache-2.0", set()) == {"Apache-2.0"}
    assert licences.additions("Apache-2.0 WITH LLVM-exception", {"Apache-2.0"}) == set()
    assert licences.additions("(MIT OR Apache-2.0) AND Unicode-3.0", {"MIT"}) == {"Unicode-3.0"}
    assert licences.identifiers("Apache-2.0 AND LicenseRef-fdlibm") == {"Apache-2.0", "SunPro"}


@pytest.mark.parametrize("path", ["README.md", "crates-nc/PYPI.md", "crates-gpl/PYPI.md"])
def test_pypi_descriptions_have_only_pinned_links(path):
    """Every link of a PyPI description, rewritten, is absolute and pinned to the release."""
    source = ROOT / path
    text = pypi_readme.rewrite(source.read_text(encoding="utf-8"), source, "v9.9.9")
    assert pypi_readme.relative_links(text) == []
    assert "/blob/main/" not in text and "/tree/main/" not in text
    assert re.search(r"github\.com/karellopez/larmorx/(blob|tree)/v9\.9\.9/", text)


def test_pypi_readme_link_forms():
    readme = ROOT / "README.md"
    repo = pypi_readme.REPO
    assert (
        pypi_readme.absolute("#development", readme, "v1")
        == f"{repo}/blob/v1/README.md#development"
    )
    assert (
        pypi_readme.absolute("docs/licensing.md", readme, "v1")
        == f"{repo}/blob/v1/docs/licensing.md"
    )
    assert (
        pypi_readme.absolute("docs/validation/", readme, "v1") == f"{repo}/tree/v1/docs/validation"
    )
    assert pypi_readme.absolute("NOTICE", readme, "abc") == f"{repo}/blob/abc/NOTICE"
    assert (
        pypi_readme.absolute(f"{repo}/blob/main/NOTICE", readme, "v1") == f"{repo}/blob/v1/NOTICE"
    )
    assert pypi_readme.absolute("https://doi.org/10.15497/RDA00068", readme, "v1").startswith(
        "https://doi"
    )
    image = pypi_readme.absolute("PYPI.md", ROOT / "crates-nc" / "PYPI.md", "v1", image=True)
    assert image == f"{pypi_readme.RAW}/v1/crates-nc/PYPI.md"
    with pytest.raises(pypi_readme.LinkError):
        pypi_readme.absolute("docs/no-such-file.md", readme, "v1")
    code = "Run `[x](docs/no-such-file.md)` and\n```\n[y](nowhere.md)\n```\n"
    assert pypi_readme.rewrite(code, readme, "v1") == code


@pytest.mark.parametrize("pkg", ["nc", "gpl"])
def test_companion_sdist_pyproject_is_rooted_at_the_repository(pkg):
    sub = f"crates-{pkg}"
    text = (ROOT / sub / "pyproject.toml").read_text(encoding="utf-8")
    staged = tomllib.loads(companion_sdist.staged_pyproject(text, sub))
    original = tomllib.loads(text)
    assert staged["project"]["readme"] == f"{sub}/{original['project']['readme']}"
    assert staged["project"]["license-files"] == [
        f"{sub}/{f}" for f in original["project"]["license-files"]
    ]
    maturin, before = staged["tool"]["maturin"], original["tool"]["maturin"]
    assert maturin["manifest-path"] == f"{sub}/{before['manifest-path']}"
    for key in ("data",):
        if key in before:
            assert maturin[key] == f"{sub}/{before[key]}"
    for new, old in zip(maturin.get("include", []), before.get("include", []), strict=True):
        assert new["path"] == f"{sub}/{old['path']}"
    for required in companion_sdist.REQUIRED[pkg]:
        rx = companion_sdist.glob_regex(f"{sub}/{required}")
        assert any(rx.match(rel) for rel in companion_sdist.tracked()), required


def test_glob_regex():
    rx = companion_sdist.glob_regex("crates-nc/larmorx-nc-*/src/**/*")
    assert rx.match("crates-nc/larmorx-nc-bet/src/lib.rs")
    assert rx.match("crates-nc/larmorx-nc-core/src/a/b/c.rs")
    assert not rx.match("crates-nc/larmorx-nc-core/tools/x/src/main.rs")
    assert not rx.match("crates-nc/larmorx-nc-bet/Cargo.toml")
