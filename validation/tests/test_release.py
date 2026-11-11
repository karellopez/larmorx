# SPDX-License-Identifier: Apache-2.0
"""The release's helpers (scripts/release/): one version everywhere, the licence expressions,
PyPI's descriptions, the companions' source distributions, the third-party crates' licence texts,
the check of Windows files' imported DLLs, and the release workflow's own: the release notes, the
standalone archives, the set of files with SHA256SUMS, PyPI's state, the tagged commit's CI, the
container images (their build context, the multi-architecture indexes, the Dockerfile), and the
workflow's environments, pins and jobs."""

from __future__ import annotations

import fnmatch
import hashlib
import importlib.util
import io
import json
import re
import shutil
import struct
import subprocess
import sys
import tarfile
import tomllib
import zipfile
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
third_party = load("third_party")
pe_imports = load("pe_imports")


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


def test_citation_version(tmp_path, monkeypatch):
    """CITATION.cff, once it exists, holds the release's version and date."""
    (tmp_path / "CITATION.cff").write_text(
        'cff-version: 1.2.0\nversion: "0.0.9"\ndate-released: "2026-01-01"\n', encoding="utf-8"
    )
    for ws in ("", "crates-gpl/", "crates-nc/"):
        (tmp_path / ws).mkdir(parents=True, exist_ok=True)
        for name in ("Cargo.toml", "Cargo.lock"):
            shutil.copy(ROOT / ws / name, tmp_path / ws / name)
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
    assert any("CITATION.cff" in p for p in set_version.check())
    set_version.set_version(set_version.version(), "2026-11-12")
    text = (tmp_path / "CITATION.cff").read_text(encoding="utf-8")
    assert f'version: "{set_version.version()}"' in text and 'date-released: "2026-11-12"' in text
    assert set_version.check() == []
    with pytest.raises(ValueError):
        set_version.set_version(set_version.version(), "2026-13-01")


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


@pytest.mark.parametrize("package", third_party.PACKAGES, ids=lambda p: p.name)
def test_third_party_crates_match_cargo_lock(package):
    """The crates of third-party-rust.txt's sections 1 to 3 are exactly the registry crates that
    Cargo.lock's graph reaches from the package's programs, each in one section only, with its
    version; every crate of 1 and 2 names its licence texts, and every text named is there."""
    text = package.output.read_text(encoding="utf-8")
    sections = third_party.listed(text)
    reachable = third_party.lock_reachable(package.lock, [c for c, _ in package.programs])
    assert sections["1"], "no crate compiled into the programs"
    assert sections["1"] | sections["2"] | sections["3"] == reachable
    assert not (sections["1"] & sections["2"] or sections["1"] & sections["3"])
    assert not sections["2"] & sections["3"]
    blocks = set(re.findall(r"(?m)^\[(T\d+)\] ", text))
    for line in text.split("\n4. ", 1)[0].splitlines():
        if third_party._ENTRY.match(line) and "[" in line:
            named = set(re.findall(r"\b(T\d+) ", line.split("[", 1)[1]))
            assert named <= blocks, line
    for name, version in sections["1"] | sections["2"]:
        assert re.search(rf"(?m)^- {re.escape(name)} {re.escape(version)}: .*\[T\d+ ", text), name


def test_third_party_texts_are_where_the_packages_ship_them():
    """Each package's licence files (so its wheel and sdist) hold its third-party-rust.txt, and
    bzip2-1.0.6's copyright notice is in the files of the packages that link libbz2-rs-sys."""
    for package in third_party.PACKAGES:
        pyproject = tomllib.loads((package.workspace / "pyproject.toml").read_text("utf-8"))
        rel = package.output.relative_to(package.workspace).as_posix()
        assert any(fnmatch.fnmatch(rel, g) for g in pyproject["project"]["license-files"]), rel
        text = package.output.read_text(encoding="utf-8")
        linked = third_party.listed(text)["1"]
        if any(name == "libbz2-rs-sys" for name, _ in linked):
            assert "Copyright (C) 1996-2021 Julian R Seward." in text
    gpl = companion_sdist.REQUIRED["gpl"]
    assert "LICENSES/third-party-rust.txt" in gpl or "LICENSES/*" in gpl


def test_third_party_files_are_up_to_date():
    """The files equal what the generator writes from the registry's sources (needs cargo)."""
    if shutil.which("cargo") is None:
        pytest.skip("cargo is needed (cargo tree, cargo metadata)")
    assert third_party.main(["--check"]) == 0


def _pe(dlls=(), delayed=(), machine=0x8664, plus=True, old_delay=False) -> bytes:
    """A minimal PE image: a DOS header, the COFF and optional headers and one section with an
    import directory naming ``dlls`` and a delay-load directory naming ``delayed``."""
    va, raw = 0x1000, 0x200
    image_base = 0x140000000 if plus else 0x400000
    imports_size = 20 * (len(dlls) + 1)
    delay_size = 32 * (len(delayed) + 1)
    names = b""
    name_rvas = []
    for dll in (*dlls, *delayed):
        name_rvas.append(va + imports_size + delay_size + len(names))
        names += dll.encode("ascii") + b"\0"
    body = b"".join(struct.pack("<IIIII", 1, 0, 0, r, 1) for r in name_rvas[: len(dlls)])
    body += b"\0" * 20
    for r in name_rvas[len(dlls) :]:
        name = r + image_base if old_delay else r
        body += struct.pack("<8I", 0 if old_delay else 1, name, 0, 1, 1, 0, 0, 0)
    body += b"\0" * 32 + names
    n_dirs = 16
    opt_size = (112 if plus else 96) + 8 * n_dirs
    dirs = [(0, 0)] * n_dirs
    if dlls:
        dirs[1] = (va, imports_size)
    if delayed:
        dirs[13] = (va + imports_size, delay_size)
    opt = bytearray(opt_size)
    struct.pack_into("<H", opt, 0, 0x20B if plus else 0x10B)
    if plus:
        struct.pack_into("<Q", opt, 24, image_base)
    else:
        struct.pack_into("<I", opt, 28, image_base)
    struct.pack_into("<I", opt, 108 if plus else 92, n_dirs)
    for k, (rva, size) in enumerate(dirs):
        struct.pack_into("<II", opt, (112 if plus else 96) + 8 * k, rva, size)
    coff = struct.pack("<HHIIIHH", machine, 1, 0, 0, 0, opt_size, 0x22)
    section = struct.pack("<8sIIIIIIHHI", b".idata", len(body), va, len(body), raw, 0, 0, 0, 0, 0)
    head = bytearray(0x40)
    head[:2] = b"MZ"
    struct.pack_into("<I", head, 0x3C, 0x40)
    image = bytes(head) + b"PE\0\0" + coff + bytes(opt) + section
    return image + b"\0" * (raw - len(image)) + body


def test_pe_imports_reads_the_import_tables():
    info = pe_imports.parse(_pe(["KERNEL32.dll", "python3.dll"], ["bcryptprimitives.dll"]))
    assert (info.machine, info.imports) == ("x86_64", ["KERNEL32.dll", "python3.dll"])
    assert info.delay_imports == ["bcryptprimitives.dll"] and info.forbidden == []
    info = pe_imports.parse(_pe(["kernel32.dll"], ["VCRUNTIME140_1.dll"], 0x14C, plus=False))
    assert (info.machine, info.forbidden) == ("i386", ["VCRUNTIME140_1.dll"])
    arm = pe_imports.parse(_pe(["KERNEL32.dll"], ["api-ms-win-crt-math-l1-1-0.dll"], 0xAA64))
    assert (arm.machine, arm.forbidden) == ("aarch64", ["api-ms-win-crt-math-l1-1-0.dll"])
    # The old delay-load form (addresses, not RVAs) of PE32 images.
    old = pe_imports.parse(_pe([], ["MSVCR90.dll"], 0x14C, plus=False, old_delay=True))
    assert (old.imports, old.forbidden) == ([], ["MSVCR90.dll"])
    for dll in (
        "MSVCP140.dll",
        "ucrtbase.dll",
        "vcruntime140d.dll",
        "api-ms-win-crt-heap-l1-1-0.dll",
    ):
        assert pe_imports.FORBIDDEN.match(dll), dll
    for dll in (
        "KERNEL32.dll",
        "ntdll.dll",
        "python3.dll",
        "msvcrt.dll",
        "api-ms-win-core-synch-l1-2-0.dll",
    ):
        assert not pe_imports.FORBIDDEN.match(dll), dll
    with pytest.raises(pe_imports.NotPE):
        pe_imports.parse(b"\x7fELF" + b"\0" * 200)
    with pytest.raises(pe_imports.NotPE):
        pe_imports.parse(_pe(["KERNEL32.dll"])[:300])


def test_pe_imports_command_line(tmp_path, capsys):
    good, bad = tmp_path / "good", tmp_path / "bad"
    good.mkdir()
    bad.mkdir()
    (good / "larmorx.exe").write_bytes(_pe(["KERNEL32.dll", "ntdll.dll"]))
    with zipfile.ZipFile(good / "larmorx-0-cp312-abi3-win_amd64.whl", "w") as z:
        z.writestr("larmorx/_core.pyd", _pe(["python3.dll", "KERNEL32.dll"]))
        z.writestr("larmorx/__init__.py", "")
    (good / "notes.txt").write_text("not a PE file", encoding="utf-8")
    assert pe_imports.main([str(good), "--machine", "x86_64"]) == 0
    out = capsys.readouterr().out
    assert "_core.pyd: x86_64: python3.dll, KERNEL32.dll" in out and "PE-IMPORTS ok: 2 PE" in out
    assert pe_imports.main([str(good), "--machine", "aarch64", "--quiet"]) == 1
    (bad / "larmorx-nc.exe").write_bytes(_pe(["KERNEL32.dll", "VCRUNTIME140.dll"]))
    assert pe_imports.main([str(bad)]) == 1
    assert "imports the C runtime: VCRUNTIME140.dll" in capsys.readouterr().err
    (bad / "broken.dll").write_bytes(b"MZ" + b"\0" * 10)
    assert pe_imports.main([str(bad / "broken.dll")]) == 2
    assert pe_imports.main([str(tmp_path / "nothing-here")]) == 1
    assert "no such file or directory" in capsys.readouterr().err
    empty = tmp_path / "empty"
    empty.mkdir()
    assert pe_imports.main([str(empty)]) == 1
    assert "no PE file found" in capsys.readouterr().err


# The release workflow's helpers (relay 3a): the notes, the archives, the set of files, PyPI's
# state, the tagged commit's CI, and the workflow itself.
changelog = load("changelog")
archives = load("archives")
assemble = load("assemble")
pypi_state = load("pypi_state")
ci_green = load("ci_green")

CHANGES = """\
# Changelog

## [Unreleased]

- next

## [0.2.0rc1] - 2026-12-01

The second, with [the licences](docs/licensing.md) and [this page](#changelog).

```
## [0.2.0rc1] in a fence is not a heading
```

## 0.2.0 (2026-12-24)

Final.

### Fixed
- a thing

## [0.1.0rc1] - 2026-11-12
- first

[0.2.0rc1]: https://github.com/karellopez/larmorx/releases/tag/v0.2.0rc1
[0.1.0rc1]: https://github.com/karellopez/larmorx/releases/tag/v0.1.0rc1
"""


def test_changelog_sections():
    second = changelog.section(CHANGES, "0.2.0rc1")
    assert second.startswith("The second") and second.endswith("```")
    assert "in a fence is not a heading" in second and "Final." not in second
    final = changelog.section(CHANGES, "0.2.0")
    assert final.startswith("Final.") and final.endswith("- a thing")
    assert changelog.section(CHANGES, "0.1.0rc1") == "- first"  # the link definitions left out


@pytest.mark.parametrize(
    ("text", "version", "message"),
    [
        (CHANGES, "0.3.0", "no section for 0.3.0: add a heading '## [0.3.0] - YYYY-MM-DD'"),
        (CHANGES, "0.1.0", "the versions it has: 0.2.0rc1, 0.2.0, 0.1.0rc1"),
        ("## [1.0.0]\ntext\n## 1.0.0 (again)\nmore\n", "1.0.0", "2 sections for 1.0.0"),
        ("## [1.0.0] - 2027-01-01\n\n## [0.9.0]\n- x\n", "1.0.0", "has no text"),
        ("# Changelog\n", "1.0.0", "the versions it has: none"),
    ],
)
def test_changelog_refuses_a_missing_section(text, version, message):
    with pytest.raises(changelog.ChangelogError, match=re.escape(message)):
        changelog.section(text, version)


def test_changelog_notes_have_absolute_links_and_the_footer():
    text = changelog.notes(CHANGES, "0.2.0rc1", "v0.2.0rc1", ROOT / "CHANGELOG.md")
    blob = "https://github.com/karellopez/larmorx/blob/v0.2.0rc1"
    assert f"[the licences]({blob}/docs/licensing.md)" in text
    assert f"[this page]({blob}/CHANGELOG.md#changelog)" in text
    assert 'pip install "larmorx[all]==0.2.0rc1"' in text and "SHA256SUMS" in text
    assert "`apptainer pull docker://ghcr.io/karellopez/larmorx:0.2.0rc1`" in text
    assert "`ghcr.io/karellopez/larmorx:0.2.0rc1-all`" in text
    assert pypi_readme.relative_links(text) == []


def test_changelog_command_line(tmp_path, capsys):
    (tmp_path / "CHANGELOG.md").write_text("## [1.0.0] - 2027-01-01\n- done\n", encoding="utf-8")
    out = tmp_path / "notes.md"
    assert (
        changelog.main(
            [
                "1.0.0",
                "--ref",
                "v1.0.0",
                "--file",
                str(tmp_path / "CHANGELOG.md"),
                "--out",
                str(out),
            ]
        )
        == 0
    )
    assert out.read_text(encoding="utf-8").startswith("- done\n\n---\n")
    assert (
        changelog.main(["1.0.1", "--ref", "v1.0.1", "--file", str(tmp_path / "CHANGELOG.md")]) == 1
    )
    assert changelog.main(["1.0.0", "--ref", "v1.0.0", "--file", str(tmp_path / "none.md")]) == 1
    err = capsys.readouterr().err
    assert "no section for 1.0.1" in err and "none.md does not exist" in err


TOOLS_TOML = """\
[[tool]]
id = "a"
title = "Tool A (one | two)"
validation = "[{nifti-io.cases} cases](record:nifti-io); data panel: [x](record:nifti-io-panel); [b](record:afni-tshift-replica)"

[[tool]]
id = "b"
title = "Tool B"
validation = "[c](record:oracles)"

[[record]]
id = "nifti-io"
status = "`validated` Data panel: [nifti-io-panel](record:nifti-io-panel), {nifti-io-panel.cases} cases."

[[record]]
id = "afni-tshift-replica"
status = "`experimental` (2 of 3)"

[[record]]
id = "oracles"
status = "reference"
"""

STATUS_TABLE = (
    "| Tool | Validation records and their status |\n"
    "|---|---|\n"
    "| Tool A (one \\| two) | [nifti-io](docs/validation/nifti-io.md): `validated`; "
    "[afni-tshift-replica](docs/validation/afni-tshift-replica.md): `experimental` |\n"
    "| Tool B | [oracles](docs/validation/oracles.md): `reference` |"
)


def test_changelog_statuses_from_tools_toml():
    """Each tool in its order with the records its text links (a data panel's record left out,
    as part of its tool's main record) and each record's status word."""
    assert changelog.statuses(TOOLS_TOML) == STATUS_TABLE
    for text, message in (
        (TOOLS_TOML.replace("(record:oracles)", "(record:c)"), "b links c, which has no record"),
        (TOOLS_TOML.replace('"reference"', '"to be decided"'), "the record oracles has no status"),
        (TOOLS_TOML.replace('"[c](record:oracles)"', '"no link"'), "b links no validation record"),
    ):
        with pytest.raises(changelog.ChangelogError, match=re.escape(message)):
            changelog.statuses(text)
    # the repository's own tools.toml gives a table: every tool, every linked record a status
    table = changelog.statuses(changelog.TOOLS.read_text(encoding="utf-8"))
    tools = tomllib.loads(changelog.TOOLS.read_text(encoding="utf-8"))["tool"]
    assert len(table.splitlines()) == 2 + len(tools)
    for record in re.findall(r"\]\(docs/validation/([^)]+)\.md\)", table):
        assert (ROOT / "docs" / "validation" / f"{record}.md").is_file(), record


def _with_statuses(table: str) -> str:
    return (
        "# Changelog\n\n## [1.1.0] - 2027-02-01\n\nNew.\n\n"
        f"{changelog.BEGIN}\n{table}\n{changelog.END}\n\nAfter.\n\n"
        "## [1.0.0] - 2027-01-01\n\nOld.\n\n"
        f"{changelog.BEGIN}\n| old |\n{changelog.END}\n"
    )


def test_changelog_statuses_checked_and_written(tmp_path, monkeypatch):
    """The notes refuse a statuses table that is not tools.toml's and leave the markers out;
    --write-statuses makes the table of one version's section anew, and only that one."""
    tools = tmp_path / "tools.toml"
    tools.write_text(TOOLS_TOML, encoding="utf-8")
    text = _with_statuses(STATUS_TABLE)
    notes = changelog.notes(text, "1.1.0", "v1.1.0", ROOT / "CHANGELOG.md", tools)
    assert (
        "statuses" not in notes and "| Tool B | [oracles](https://github.com/karellopez/" in notes
    )
    assert notes.startswith("New.\n\n| Tool | Validation records") and "\n\nAfter.\n\n---" in notes
    with pytest.raises(
        changelog.ChangelogError, match=re.escape("the statuses table of 1.0.0 is not")
    ):
        changelog.notes(text, "1.0.0", "v1.0.0", ROOT / "CHANGELOG.md", tools)
    stale = _with_statuses("| stale |")
    with pytest.raises(changelog.ChangelogError, match="--write-statuses"):
        changelog.checked(changelog.section(stale, "1.1.0"), "1.1.0", tools)
    written = changelog.write_statuses(stale, "1.1.0", tools)
    assert written == text and changelog.write_statuses(written, "1.1.0", tools) == written
    assert "| old |" in written  # the other section's table is left alone
    for broken, message in (
        (text.replace(changelog.END + "\n\nAfter", "After"), "needs one"),
        ("## [1.1.0] - 2027-02-01\n\nNo table.\n", "has no statuses table"),
    ):
        with pytest.raises(changelog.ChangelogError, match=message):
            changelog.write_statuses(broken, "1.1.0", tools)
    # the command line
    path = tmp_path / "CHANGELOG.md"
    path.write_text(stale, encoding="utf-8")
    monkeypatch.setattr(changelog, "TOOLS", tools)
    assert changelog.main(["1.1.0", "--write-statuses", "--file", str(path)]) == 0
    assert path.read_text(encoding="utf-8") == text
    assert changelog.main(["1.0.0", "--ref", "v1.0.0", "--file", str(path)]) == 1  # stale


def test_changelog_notes_are_unwrapped():
    """A release's text has one line per paragraph and list item (GitHub keeps line breaks in a
    release's text); lists, tables, headings, fences and hard breaks stay as they are."""
    text = (
        "A paragraph\nwritten over\n  three lines.\n\n"
        "- an item\n  continued\n  - a nested one\n    continued\n- the next\n\n"
        "### A heading\nA line.\n\n"
        "| a | b |\n|---|---|\n| 1 | 2 |\n\n"
        "```\ncode\nlines\n```\nafter the fence\njoined\n\n"
        "a hard  \nbreak and\\\nanother\n\n"
        "1. first\n   more\n2. second\n\n> quoted\n> again\n"
    )
    assert changelog.unwrapped(text) == (
        "A paragraph written over three lines.\n\n"
        "- an item continued\n  - a nested one continued\n- the next\n\n"
        "### A heading\nA line.\n\n"
        "| a | b |\n|---|---|\n| 1 | 2 |\n\n"
        "```\ncode\nlines\n```\nafter the fence joined\n\n"
        "a hard  \nbreak and\\\nanother\n\n"
        "1. first more\n2. second\n\n> quoted\n> again\n"
    )


def test_the_release_documents():
    """CHANGELOG.md has the current version's section, dated as CITATION.cff, with its statuses
    table; CITATION.cff names the author, the licence and the repository; README.md and the
    changelog name RELEASING.md, which exists."""
    version = set_version.version()
    text = (ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
    body = changelog.section(text, version)
    assert changelog.BEGIN in body and changelog.END in body
    heading = re.search(rf"(?m)^## \[{re.escape(version)}\] - (\d{{4}}-\d{{2}}-\d{{2}})$", text)
    assert heading is not None, "the heading '## [<version>] - YYYY-MM-DD'"
    assert f"[{version}]: https://github.com/karellopez/larmorx/releases/tag/v{version}" in text
    citation = (ROOT / "CITATION.cff").read_text(encoding="utf-8")
    assert re.search(r'(?m)^date-released: "(.*)"$', citation).group(1) == heading.group(1)
    assert set_version.citation_version() == version
    for line in (
        "cff-version: 1.2.0",
        "type: software",
        "  - given-names: Karel Mauricio",
        "    family-names: López Vilaret",
        '    orcid: "https://orcid.org/0009-0007-0504-5258"',
        "license: Apache-2.0",
        'repository-code: "https://github.com/karellopez/larmorx"',
        'repository-artifact: "https://pypi.org/project/larmorx/"',
    ):
        assert line in citation.splitlines(), line
    assert citation.count("given-names:") == 1  # one author (the user, 2026-11-10)
    assert (ROOT / "RELEASING.md").is_file()
    for name in ("README.md", "CHANGELOG.md"):
        assert "(RELEASING.md)" in (ROOT / name).read_text(encoding="utf-8"), name


def _program_wheels(folder: Path, version: str, exe: str = "") -> None:
    """Small stand-ins of the three program wheels, laid out as maturin makes them."""
    folder.mkdir(parents=True, exist_ok=True)
    for dist, programs, sub in (
        ("larmorx_cli", ("larmorx", "lx"), None),
        ("larmorx_nc", ("larmorx-nc",), "crates-nc"),
        ("larmorx_gpl", ("larmorx-gpl",), "crates-gpl"),
    ):
        with zipfile.ZipFile(folder / f"{dist}-{version}-py3-none-any.whl", "w") as z:
            for p in programs:
                info = zipfile.ZipInfo(f"{dist}-{version}.data/scripts/{p}{exe}")
                info.external_attr = 0o100755 << 16
                z.writestr(info, f"program {p}")
            z.writestr(f"{dist}-{version}.dist-info/METADATA", "Name: x\n")
            if sub is None:
                continue
            lic = f"{dist}-{version}.dist-info/licenses/{sub}"
            for name in ("LICENSE", "NOTICE", "LICENSES/third-party-rust.txt", "Cargo.toml"):
                z.writestr(f"{lic}/{name}", f"{sub} {name}")
            z.writestr(f"{lic}/{dist.replace('_', '-')}-cli/src/main.rs", "fn main() {}")
            z.writestr(f"{dist}-{version}.dist-info/sboms/{dist}.cyclonedx.json", "{}")
            if dist == "larmorx_nc":
                z.writestr(f"{lic}/fsl-source/MANIFEST.tsv", "file\n")
                data = f"{dist}-{version}.data/data/share/larmorx-nc/fsl-data"
                z.writestr(f"{data}/MANIFEST.tsv", "file\tbytes\tsha256\n")


def test_archives_unix(tmp_path):
    _program_wheels(tmp_path / "wheels", "1.2.3")
    made = archives.build("1.2.3", "x86_64-unknown-linux-gnu", tmp_path / "wheels")
    assert [a.filename for a in made] == [
        "larmorx-1.2.3-x86_64-unknown-linux-gnu.tar.gz",
        "larmorx-nc-1.2.3-x86_64-unknown-linux-gnu.tar.gz",
        "larmorx-gpl-1.2.3-x86_64-unknown-linux-gnu.tar.gz",
    ]
    main, nc, gpl = (a.entries for a in made)
    assert main["bin/lx"].link == "larmorx" and main["bin/larmorx"].executable
    assert main["LICENSE"].data == (ROOT / "LICENSE").read_bytes()
    assert "LICENSES/third-party-rust.txt" in main and "source/Cargo.toml" not in main
    assert nc["LICENSE"].data == b"crates-nc LICENSE" and "source/Cargo.toml" in nc
    assert "source/fsl-source/MANIFEST.tsv" in nc and "share/larmorx-nc/fsl-data/MANIFEST.tsv" in nc
    assert "source/larmorx-nc-cli/src/main.rs" in nc and "sbom/larmorx_nc.cyclonedx.json" in nc
    assert "bin/larmorx-gpl" in gpl and "source/larmorx-gpl-cli/src/main.rs" in gpl
    (tmp_path / "a").mkdir()
    (tmp_path / "b").mkdir()
    first = archives.write(made[0], tmp_path / "a", 1_700_000_000)
    again = archives.write(made[0], tmp_path / "b", 1_700_000_000)
    assert first.read_bytes() == again.read_bytes()  # the same files give the same bytes
    with tarfile.open(first) as t:
        members = {m.name: m for m in t.getmembers()}
    top = "larmorx-1.2.3-x86_64-unknown-linux-gnu"
    assert members[f"{top}/bin/lx"].issym() and members[f"{top}/bin/lx"].linkname == "larmorx"
    assert members[f"{top}/bin/larmorx"].mode == 0o755 and members[f"{top}/LICENSE"].mode == 0o644
    assert {m.uid for m in members.values()} == {0} and members[f"{top}/bin"].isdir()
    assert {m.mtime for m in members.values()} == {1_700_000_000}


def test_archives_windows(tmp_path):
    _program_wheels(tmp_path / "wheels", "1.2.3", exe=".exe")
    made = archives.build("1.2.3", "aarch64-pc-windows-msvc", tmp_path / "wheels")
    assert made[0].filename == "larmorx-1.2.3-aarch64-pc-windows-msvc.zip"
    path = archives.write(made[0], tmp_path, 0)
    with zipfile.ZipFile(path) as z:
        names = z.namelist()
        top = "larmorx-1.2.3-aarch64-pc-windows-msvc"
        assert z.read(f"{top}/bin/lx.cmd") == b'@"%~dp0larmorx.exe" %*\r\n'
        assert z.read(f"{top}/README.txt").count(b"\r\n") > 5
    assert f"{top}/bin/larmorx.exe" in names and f"{top}/bin/lx.exe" not in names
    assert "bin/larmorx-nc.exe" in made[1].entries


def test_archives_refuse_what_is_missing(tmp_path):
    _program_wheels(tmp_path / "wheels", "1.2.3")
    with pytest.raises(ValueError, match=re.escape("expected one larmorx_cli-1.2.4")):
        archives.build("1.2.4", "x86_64-unknown-linux-gnu", tmp_path / "wheels")
    with pytest.raises(ValueError, match="not a release target"):
        archives.build("1.2.3", "x86_64-unknown-linux-musl", tmp_path / "wheels")
    with pytest.raises(ValueError, match=r"no program larmorx\.exe"):
        archives.build("1.2.3", "x86_64-pc-windows-msvc", tmp_path / "wheels")


def _release_files(folder: Path, version: str) -> None:
    for i, name in enumerate(sorted(assemble.expected(version))):
        sub = folder / ("a" if i % 2 else "b")  # any layout
        sub.mkdir(parents=True, exist_ok=True)
        (sub / name).write_bytes(name.encode())


def test_assemble_the_whole_set(tmp_path, capsys):
    want = assemble.expected("0.1.0rc1")
    assert len(want) == 39
    assert sum(folder == "pypi" for folder, _ in want.values()) == 21
    assert "larmorx-0.1.0rc1-cp312-abi3-manylinux_2_28_x86_64.whl" in want
    assert "larmorx_nc-0.1.0rc1-py3-none-macosx_11_0_arm64.whl" in want
    assert "larmorx-gpl-0.1.0rc1-aarch64-pc-windows-msvc.zip" in want
    _release_files(tmp_path / "in", "0.1.0rc1")
    assert (
        assemble.main(
            ["--version", "0.1.0rc1", "--in", str(tmp_path / "in"), "--out", str(tmp_path / "out")]
        )
        == 0
    )
    assert "ASSEMBLE ok: 39 files" in capsys.readouterr().out
    sums = (tmp_path / "out" / "SHA256SUMS").read_text(encoding="ascii").splitlines()
    assert len(sums) == 39 and sums == sorted(sums, key=lambda line: line.split("  ")[1])
    digest, name = sums[0].split("  ")
    assert digest == hashlib.sha256(name.encode()).hexdigest()
    assert len(list((tmp_path / "out" / "pypi").iterdir())) == 21
    assert len(list((tmp_path / "out" / "archives").iterdir())) == 18


def test_assemble_refuses_an_incomplete_set(tmp_path, capsys):
    _release_files(tmp_path / "in", "0.1.0rc1")
    gone = next((tmp_path / "in").rglob("larmorx_gpl-0.1.0rc1-py3-none-win_arm64.whl"))
    gone.unlink()
    (tmp_path / "in" / "larmorx-0.1.0rc1-cp312-abi3-manylinux_2_34_x86_64.whl").write_bytes(b"x")
    (tmp_path / "in" / "c").mkdir()
    (tmp_path / "in" / "c" / "larmorx-0.1.0rc1.tar.gz").write_bytes(b"x")
    assert (
        assemble.main(
            ["--version", "0.1.0rc1", "--in", str(tmp_path / "in"), "--out", str(tmp_path / "out")]
        )
        == 1
    )
    err = capsys.readouterr().err
    assert "missing: larmorx_gpl-0.1.0rc1-py3-none-win_arm64.whl" in err
    assert "not a file of 0.1.0rc1's release: larmorx-0.1.0rc1-cp312-abi3-manylinux_2_34" in err
    assert "larmorx-0.1.0rc1.tar.gz found twice" in err
    assert not (tmp_path / "out").exists()


def test_pypi_state_names_and_comparison():
    assert pypi_state.project("larmorx_nc-0.1.0rc1-py3-none-win_amd64.whl") == "larmorx-nc"
    assert pypi_state.project("larmorx_gpl-0.1.0rc1.tar.gz") == "larmorx-gpl"
    assert pypi_state.project("larmorx-0.1.0rc1-cp312-abi3-win_arm64.whl") == "larmorx"
    want = {"a.whl": "1", "b.whl": "2", "c.tar.gz": "3"}
    assert pypi_state.compare(want, None) == (["a.whl", "b.whl", "c.tar.gz"], [])
    assert pypi_state.compare(want, {"a.whl": "1", "b.whl": "9"}) == (["c.tar.gz"], ["b.whl"])
    assert pypi_state.compare(want, {**want, "d.whl": "4"}) == ([], [])


def test_ci_green_ignores_what_ci_ignores():
    """ci_green.py's paths are ci.yml's `paths-ignore`, for the push to main and pull requests."""
    ci = (ROOT / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")
    found = re.findall(r"paths-ignore:\s*\[([^\]]*)\]", ci)
    assert found and all(
        [p.strip().strip("'\"") for p in f.split(",")] == list(ci_green.IGNORED) for f in found
    )
    assert ci_green.ignored("README.md") and ci_green.ignored("crates-nc/PYPI.md")
    assert ci_green.ignored("docs/validation/x.json") and not ci_green.ignored("scripts/x.py")
    assert not ci_green.ignored("mydocs/x.json") and not ci_green.ignored("CHANGELOG.mdx")


def test_ci_green_verdict(tmp_path, monkeypatch):
    git = shutil.which("git")
    if git is None:
        pytest.skip("git is not installed")
    monkeypatch.chdir(tmp_path)
    for var in ("GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE"):
        monkeypatch.delenv(var, raising=False)

    def commit(path: str, text: str) -> str:
        (tmp_path / path).parent.mkdir(parents=True, exist_ok=True)
        (tmp_path / path).write_text(text, encoding="utf-8")
        ci_green.git("add", path)
        ci_green.git("-c", "user.name=t", "-c", "user.email=t@example.org", "commit", "-qm", path)
        return ci_green.git("rev-parse", "HEAD").strip()

    ci_green.git("init", "-q")
    code = commit("src/a.py", "1")
    docs = commit("docs/a.md", "1")
    readme = commit("README.md", "1")
    more = commit("src/a.py", "2")
    ci_green.git("update-ref", "refs/remotes/origin/main", more)
    assert ci_green.verdict(readme, {code: 1}) == (
        True,
        f"CI passed for {code[:12]} (run 1); since then documentation only (2 files)",
    )
    assert ci_green.verdict(code, {code: 1})[0] and ci_green.verdict(docs, {code: 1})[0]
    ok, why = ci_green.verdict(more, {code: 1})
    assert not ok and "differs in code (src/a.py)" in why
    assert not ci_green.verdict(more, {})[0]
    ci_green.git("checkout", "-q", "-b", "side", readme)
    side = commit("src/b.py", "1")
    ok, why = ci_green.verdict(side, {side: 2})
    assert not ok and "is not on origin/main" in why


def test_release_workflow_names_and_pins():
    """The environments PyPI's trusted publishers name (the user's, 2026-11-10), every action
    pinned by its commit, the publish jobs' permissions, and only a v* tag publishes."""
    text = (ROOT / ".github" / "workflows" / "release.yml").read_text(encoding="utf-8")
    assert re.findall(r"^\s+name: (pypi[-a-z]*)$", text, re.M) == ["pypi-nc", "pypi-gpl", "pypi"]
    uses = re.findall(r"^\s+(?:- )?uses: (\S+)", text, re.M)
    assert uses and all(re.fullmatch(r"[\w.-]+/[\w.-]+@[0-9a-f]{40}", u) for u in uses), uses
    assert text.count("id-token: write # trusted publishing") == 3
    assert re.search(r"^permissions: \{\}$", text, re.M)
    assert 'tags: ["v*"]' in text and 'branches: ["release/dry-run"]' in text
    assert text.count("if: needs.check.outputs.publish == 'true'") == 6


# ---------------------------------------------------------------------------------------------
# The container images (scripts/release/container.py, docker/Dockerfile; relay 3b)

container = load("container")
CV = "0.2.0rc1"
LAYER = "application/vnd.oci.image.layer.v1.tar+gzip"


def _workflow_jobs(text: str) -> dict[str, str]:
    """The release workflow's jobs: name -> its block of text."""
    body = text.split("\njobs:\n", 1)[1]
    parts = re.split(r"^  ([a-z][a-z0-9-]*):\n", body, flags=re.M)
    return dict(zip(parts[1::2], parts[2::2], strict=True))


def _needs(block: str) -> set[str]:
    match = re.search(r"^    needs: \[([^\]]*)\]", block, re.M)
    assert match, block[:200]
    return {n.strip() for n in match.group(1).split(",")}


def test_container_stage_takes_one_linux_wheel_per_package(tmp_path):
    wheels, sdists = tmp_path / "wheels", tmp_path / "sdists"
    wheels.mkdir()
    sdists.mkdir()
    for arch in ("x86_64", "aarch64"):
        for dist, tags in (
            ("larmorx", "cp312-abi3"),
            ("larmorx_nc", "py3-none"),
            ("larmorx_gpl", "py3-none"),
        ):
            (wheels / f"{dist}-{CV}-{tags}-manylinux_2_28_{arch}.whl").write_bytes(b"w")
    # others of the release and of another version: left alone
    (wheels / f"larmorx-{CV}-cp312-abi3-macosx_11_0_arm64.whl").write_bytes(b"m")
    (wheels / "larmorx-0.1.0-cp312-abi3-manylinux_2_28_aarch64.whl").write_bytes(b"old")
    (wheels / f"larmorx_cli-{CV}-py3-none-manylinux_2_28_aarch64.whl").write_bytes(b"cli")
    for dist in ("larmorx_nc", "larmorx_gpl", "larmorx"):
        (sdists / f"{dist}-{CV}.tar.gz").write_bytes(b"s")
    out = tmp_path / "context"
    files = container.stage(CV, "arm64", [wheels], sdists, out)
    assert sorted(p.relative_to(out).as_posix() for p in files) == [
        "README.txt",
        f"source/larmorx_gpl-{CV}.tar.gz",
        f"source/larmorx_nc-{CV}.tar.gz",
        f"wheels/larmorx-{CV}-cp312-abi3-manylinux_2_28_aarch64.whl",
        f"wheels/larmorx_gpl-{CV}-py3-none-manylinux_2_28_aarch64.whl",
        f"wheels/larmorx_nc-{CV}-py3-none-manylinux_2_28_aarch64.whl",
    ]
    with pytest.raises(container.ContainerError, match="is not empty"):
        container.stage(CV, "arm64", [wheels], sdists, out)
    with pytest.raises(container.ContainerError, match="unknown architecture"):
        container.stage(CV, "riscv64", [wheels], sdists, tmp_path / "c1")
    (wheels / f"larmorx_nc-{CV}-py3-none-manylinux_2_34_aarch64.whl").write_bytes(b"2")
    with pytest.raises(container.ContainerError, match=r"larmorx_nc 0\.2\.0rc1: .* found 2"):
        container.stage(CV, "arm64", [wheels], sdists, tmp_path / "c2")
    with pytest.raises(
        container.ContainerError, match=re.escape("larmorx_nc-0.2.0rc1.tar.gz not found")
    ):
        container.stage(CV, "amd64", [wheels], tmp_path, tmp_path / "c3")


def _oci_archive(path: Path, arch: str, image: str, version: str = CV) -> dict:
    """A small OCI archive as `docker buildx --output type=oci` writes one: one image whose
    first layer is shared by every image (the base), the second its own."""
    blobs: dict[str, bytes] = {}

    def put(data: bytes) -> dict:
        digest = container.sha256(data)
        blobs[digest] = data
        return {"digest": digest, "size": len(data)}

    layers = [put(b"the base image's layer"), put(f"{image} on {arch}".encode())]
    labels = {"org.opencontainers.image.version": version, "org.opencontainers.image.title": image}
    config = {"architecture": arch, "os": "linux", "config": {"Labels": labels}}
    if arch == "arm64":
        config["variant"] = "v8"
    config_desc = put(json.dumps(config).encode())
    manifest = {
        "schemaVersion": 2,
        "mediaType": container.MANIFEST,
        "config": {"mediaType": "application/vnd.oci.image.config.v1+json", **config_desc},
        "layers": [{"mediaType": LAYER, **layer} for layer in layers],
    }
    manifest_desc = put(json.dumps(manifest).encode())
    index = {
        "schemaVersion": 2,
        "manifests": [{"mediaType": container.MANIFEST, **manifest_desc}],
    }
    with tarfile.open(path, "w") as tar:
        members = {"oci-layout": b'{"imageLayoutVersion":"1.0.0"}', "index.json": b""}
        members["index.json"] = json.dumps(index).encode()
        members.update({f"blobs/sha256/{d.split(':')[1]}": b for d, b in blobs.items()})
        for name, data in members.items():
            info = tarfile.TarInfo(name)
            info.size = len(data)
            tar.addfile(info, io.BytesIO(data))
    return {
        "archive": path.name,
        "manifest": manifest_desc["digest"],
        "config": config_desc["digest"],
        "bytes": path.stat().st_size,
    }


def _images_folder(folder: Path, arch: str, version: str = CV, freeze: str = "numpy==2.5.4\n"):
    folder.mkdir(parents=True)
    record = {"version": version, "arch": arch, "revision": "0" * 40, "created": "x", "images": {}}
    for image in container.IMAGES:
        archive = folder / f"{image}-{version}-{arch}.oci.tar"
        record["images"][image] = _oci_archive(archive, arch, image, version)
        (folder / f"pip-freeze-{image}.txt").write_text(freeze, encoding="utf-8")
    (folder / "images.json").write_text(json.dumps(record), encoding="utf-8")
    return record


def test_container_index_layout(tmp_path):
    amd = _images_folder(tmp_path / "amd64", "amd64")
    arm = _images_folder(tmp_path / "arm64", "arm64")
    out = tmp_path / "layout"
    digests = container.index_layout(
        CV, [tmp_path / "amd64", tmp_path / "arm64"], out, ["amd64", "arm64"]
    )
    assert list(digests) == [CV, f"{CV}-all"]
    assert json.loads((out / "digests.json").read_text(encoding="utf-8")) == digests
    assert json.loads((out / "oci-layout").read_bytes()) == {"imageLayoutVersion": "1.0.0"}
    index = json.loads((out / "index.json").read_bytes())
    refs = {m["annotations"]["org.opencontainers.image.ref.name"]: m for m in index["manifests"]}
    assert set(refs) == set(digests)
    blobs = out / "blobs" / "sha256"
    for path in blobs.iterdir():  # every blob is what its name says
        assert container.sha256(path.read_bytes()) == f"sha256:{path.name}"
    # 1 shared base layer + per image and architecture (own layer, config, manifest) + 2 indexes
    assert len(list(blobs.iterdir())) == 1 + 2 * 2 * 3 + 2
    for name, image in ((CV, "larmorx"), (f"{CV}-all", "larmorx-all")):
        desc = refs[name]
        assert desc["digest"] == digests[name] and desc["mediaType"] == container.INDEX
        body = (blobs / desc["digest"].split(":")[1]).read_bytes()
        assert len(body) == desc["size"]
        children = json.loads(body)["manifests"]
        assert [c["platform"] for c in children] == [
            {"architecture": "amd64", "os": "linux"},
            {"architecture": "arm64", "os": "linux", "variant": "v8"},
        ]
        assert [c["digest"] for c in children] == [
            amd["images"][image]["manifest"],
            arm["images"][image]["manifest"],
        ]
    # the same inputs give the same index (its digest is what the registry will show)
    again = container.index_layout(
        CV, [tmp_path / "arm64", tmp_path / "amd64"], tmp_path / "l2", ["amd64", "arm64"]
    )
    assert again == digests


def test_container_index_layout_refusals(tmp_path):
    _images_folder(tmp_path / "amd64", "amd64")
    _images_folder(tmp_path / "arm64", "arm64")
    both = [tmp_path / "amd64", tmp_path / "arm64"]
    with pytest.raises(container.ContainerError, match="architectures"):
        container.index_layout(CV, both[:1], tmp_path / "l1", ["amd64", "arm64"])
    with pytest.raises(container.ContainerError, match=re.escape("not 0.3.0")):
        container.index_layout("0.3.0", both, tmp_path / "l2", ["amd64", "arm64"])
    # an archive of the other architecture in a folder
    record = json.loads((tmp_path / "arm64" / "images.json").read_text(encoding="utf-8"))
    wrong = tmp_path / "wrong"
    shutil.copytree(tmp_path / "amd64", wrong)
    record_amd = json.loads((wrong / "images.json").read_text(encoding="utf-8"))
    record_amd["arch"] = "arm64"
    (wrong / "images.json").write_text(json.dumps(record_amd), encoding="utf-8")
    with pytest.raises(container.ContainerError, match="an image for amd64 in the arm64 folder"):
        container.index_layout(CV, [tmp_path / "amd64", wrong], tmp_path / "l3", ["amd64", "arm64"])
    # images.json naming another manifest
    record["images"]["larmorx"]["manifest"] = "sha256:" + "0" * 64
    (tmp_path / "arm64" / "images.json").write_text(json.dumps(record), encoding="utf-8")
    with pytest.raises(container.ContainerError, match=re.escape("not the one images.json names")):
        container.index_layout(CV, both, tmp_path / "l4", ["amd64", "arm64"])
    # a blob whose content is not its name
    broken = tmp_path / "broken.oci.tar"
    _oci_archive(broken, "amd64", "larmorx")
    with tarfile.open(broken) as tar:
        members = [(m, tar.extractfile(m).read()) for m in tar]
    with tarfile.open(broken, "w") as tar:
        for member, data in members:
            if member.name.startswith("blobs/") and data.startswith(b"the base"):
                data = b"the base image's layer, changed"
                member.size = len(data)
            tar.addfile(member, io.BytesIO(data))
    with pytest.raises(container.ContainerError, match="has another content"):
        container.read_archive(broken)


def test_container_image_manifests_skips_attestations_and_follows_indexes(tmp_path):
    record = _oci_archive(tmp_path / "a.oci.tar", "amd64", "larmorx")
    index, blobs = container.read_archive(tmp_path / "a.oci.tar")
    attestation = json.dumps({"schemaVersion": 2, "layers": []}).encode()
    blobs[container.sha256(attestation)] = attestation
    nested = {
        "schemaVersion": 2,
        "mediaType": container.INDEX,
        "manifests": [
            *index["manifests"],
            {
                "mediaType": container.MANIFEST,
                "digest": container.sha256(attestation),
                "size": len(attestation),
                "annotations": {"vnd.docker.reference.type": "attestation-manifest"},
            },
        ],
    }
    body = json.dumps(nested).encode()
    blobs[container.sha256(body)] = body
    top = {"manifests": [{"mediaType": container.INDEX, "digest": container.sha256(body)}]}
    (found,) = container.image_manifests(top, blobs, "a")
    assert found["digest"] == record["manifest"] and found["config"] == record["config"]
    with pytest.raises(container.ContainerError, match="is not in the archive"):
        container.image_manifests(
            {"manifests": [{"mediaType": container.MANIFEST, "digest": "sha256:1"}]}, blobs, "a"
        )


def test_container_compare_freezes(tmp_path):
    _images_folder(tmp_path / "amd64", "amd64", freeze="numpy==2.5.4\nscipy==1.18.1\n")
    _images_folder(tmp_path / "arm64", "arm64", freeze="numpy==2.5.4\nscipy==1.18.0\n")
    both = [tmp_path / "amd64", tmp_path / "arm64"]
    assert container.compare_freezes(CV, both) == [
        "larmorx on amd64 lacks scipy==1.18.0",
        "larmorx on arm64 lacks scipy==1.18.1",
        "larmorx-all on amd64 lacks scipy==1.18.0",
        "larmorx-all on arm64 lacks scipy==1.18.1",
    ]
    for image in container.IMAGES:
        (tmp_path / "arm64" / f"pip-freeze-{image}.txt").write_text(
            "scipy==1.18.1\nnumpy==2.5.4\n", encoding="utf-8"
        )
    assert container.compare_freezes(CV, both) == []
    (tmp_path / "arm64" / "pip-freeze-larmorx-all.txt").unlink()
    with pytest.raises(container.ContainerError, match="the check step writes it"):
        container.compare_freezes(CV, both)
    with pytest.raises(container.ContainerError, match=re.escape("not 0.3.0")):
        container.compare_freezes("0.3.0", both)


def test_container_names_and_labels():
    assert container.push_names(CV, latest=False) == [CV]
    assert container.push_names(CV, latest=True) == [CV, "latest"]
    assert container.push_names(f"{CV}-all", latest=True) == [f"{CV}-all", "latest-all"]
    assert container.tag(CV, "larmorx") == CV and container.tag(CV, "larmorx-all") == f"{CV}-all"
    assert container.created_from_epoch(86400 + 61) == "1970-01-02T00:01:01Z"
    plain = container.expected_labels(CV, "larmorx", "a" * 40, "t")
    assert plain["org.opencontainers.image.licenses"] == "Apache-2.0"
    assert plain["org.opencontainers.image.version"] == CV
    every = container.expected_labels(CV, "larmorx-all", "a" * 40, "t")
    assert every["org.opencontainers.image.licenses"] == (
        "Apache-2.0 AND GPL-3.0-only AND LicenseRef-FSL"
    )
    assert re.fullmatch(r"docker\.io/moby/buildkit:v[\d.]+@sha256:[0-9a-f]{64}", container.BUILDKIT)


def test_dockerfile_targets_labels_and_pins():
    text = (ROOT / "docker" / "Dockerfile").read_text(encoding="utf-8")
    base = re.search(r"^ARG BASE=(\S+)$", text, re.M)
    assert base and re.fullmatch(
        r"docker\.io/library/python:3\.12\.\d+-slim-trixie@sha256:[0-9a-f]{64}", base.group(1)
    )
    assert re.findall(r"^FROM (\S+) AS (\S+)$", text, re.M) == [
        ("${BASE}", "larmorx"),
        ("larmorx", "larmorx-all"),
    ]
    assert list(container.IMAGES) == ["larmorx", "larmorx-all"]
    stages = re.split(r"^FROM .*$", text, flags=re.M)[1:]
    for (image, (_, licence, companions)), stage in zip(
        container.IMAGES.items(), stages, strict=True
    ):
        assert f'org.opencontainers.image.licenses="{licence}"' in stage
        assert f'org.opencontainers.image.title="{image}"' in stage
        assert ("larmorx_nc" in stage) == companions
    assert "ENTRYPOINT" not in text.replace("No ENTRYPOINT", "")
    assert 'CMD ["larmorx", "--help"]' in text
    assert (ROOT / "docker" / "README.txt").is_file()


def test_release_workflow_container_jobs():
    """The images are built and checked on every run and pushed only on a release, after the
    three PyPI uploads; the push job is the only one that may write to the registry; nothing is
    published before the images passed; the GitHub Release comes after the push."""
    text = (ROOT / ".github" / "workflows" / "release.yml").read_text(encoding="utf-8")
    jobs = _workflow_jobs(text)
    build, push = jobs["container"], jobs["container-push"]
    assert "if: needs.check.outputs.publish" not in build
    assert re.findall(r"runner: ([\w.-]+)", build) == ["ubuntu-24.04", "ubuntu-24.04-arm"]
    assert "container.py build" in build and "container.py check" in build
    for word in ("skopeo", "container.py push", "docker push", "--push"):
        assert word not in build
    assert not re.search(r"^ +packages:", build, re.M)
    assert "if: needs.check.outputs.publish == 'true'" in push
    assert {"container", "publish-larmorx", "publish-nc", "publish-gpl"} <= _needs(push)
    permission = re.compile(r"^ +packages: write\b", re.M)
    assert permission.search(push) and len(permission.findall(text)) == 1
    assert "container.py push" in push and text.count("container.py push") == 1
    assert "container" in _needs(jobs["assemble"])
    assert "container-push" in _needs(jobs["github-release"])
    for publish in ("publish-larmorx", "publish-nc", "publish-gpl"):
        assert "assemble" in _needs(jobs[publish])


def test_release_workflow_smoke_scopes_and_toolchain():
    """The glibc 2.28 and macOS 11.0 floors are checked on the published files only (a build from
    the sdists links the runner's own glibc: dry run 1); the companions' golden tests run on the
    installed toolchain without rustup adding rust-toolchain.toml's components (dry run 1's
    clippy conflict on ubuntu-24.04-arm)."""
    text = (ROOT / ".github" / "workflows" / "release.yml").read_text(encoding="utf-8")
    jobs = _workflow_jobs(text)
    assert "--source-build" in jobs["sdist-install"]
    for name in ("test", "verify-pypi", "container"):
        assert "--source-build" not in jobs[name]
    assert "RUSTUP_TOOLCHAIN: ${{ env.RUST_TOOLCHAIN }}" in jobs["programs"]


smoke = load("smoke")


def test_smoke_macos_libraries_leave_out_the_files_own_install_name():
    """`otool -L` prints a library's own install name (its LC_ID_DYLIB) first: the extension's
    `@rpath/larmorx._core.abi3.so` is not a dependency (dry run 2's two macOS failures); every
    other library outside the system's is still refused."""
    so = "/venv/lib/python3.12/site-packages/larmorx/_core.abi3.so"
    otool_l = (
        f"{so}:\n"
        "\t@rpath/larmorx._core.abi3.so (compatibility version 0.0.0, current version 0.0.0)\n"
        "\t/usr/lib/libiconv.2.dylib (compatibility version 7.0.0, current version 7.0.0)\n"
        "\t/System/Library/Frameworks/Security.framework/Versions/A/Security (compatibility"
        " version 1.0.0, current version 61040.1.3)\n"
        "\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1345.0.0)\n"
    )
    otool_d = f"{so}:\n@rpath/larmorx._core.abi3.so\n"
    assert smoke.foreign_macos_libraries(otool_l, otool_d) == []
    # without the id left out, it would count: the check before the fix
    assert smoke.foreign_macos_libraries(otool_l, f"{so}:\n") == ["@rpath/larmorx._core.abi3.so"]
    # another library of our own, or one from elsewhere, is refused as before
    extra = otool_l + (
        "\t@rpath/libother.dylib (compatibility version 1.0.0, current version 1.0.0)\n"
        "\t/opt/homebrew/lib/libomp.dylib (compatibility version 5.0.0, current version 5.0.0)\n"
    )
    assert smoke.foreign_macos_libraries(extra, otool_d) == [
        "@rpath/libother.dylib",
        "/opt/homebrew/lib/libomp.dylib",
    ]
    # a program has no install name (`otool -D` prints the header only); a universal file's
    # headers name the architecture
    program = "/venv/bin/larmorx-nc"
    otool_l = (
        f"{program} (architecture arm64):\n"
        "\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1345.0.0)\n"
    )
    assert smoke.foreign_macos_libraries(otool_l, f"{program} (architecture arm64):\n") == []


def _elf_with_version_needs(needs: dict[str, list[str]]) -> bytes:
    """A minimal 64-bit little-endian ELF file holding only a `.dynstr` and a `.gnu.version_r`."""
    strtab = b"\0"
    offsets = {}
    for name in [*needs, *(v for vs in needs.values() for v in vs)]:
        offsets[name] = len(strtab)
        strtab += name.encode() + b"\0"
    verneed = b""
    for i, (lib, versions) in enumerate(needs.items()):
        last = i == len(needs) - 1
        size = 16 + 16 * len(versions)
        verneed += struct.pack("<HHIII", 1, len(versions), offsets[lib], 16, 0 if last else size)
        for j, v in enumerate(versions):
            following = 0 if j == len(versions) - 1 else 16
            verneed += struct.pack("<IHHII", 0, 0, 2 + j, offsets[v], following)
    strtab_at, verneed_at = 64, 64 + len(strtab)
    shoff = verneed_at + len(verneed)
    header = struct.pack(
        "<16sHHIQQQIHHHHHH", b"\x7fELF\x02\x01\x01" + bytes(9), 3, 62, 1, 0, 0, shoff, 0, 64, 0,
        0, 64, 3, 0,
    )  # fmt: skip
    sections = bytes(64) + struct.pack(
        "<IIQQQQIIQQ", 0, 3, 0, 0, strtab_at, len(strtab), 0, 0, 1, 0
    ) + struct.pack(
        "<IIQQQQIIQQ", 0, smoke.SHT_GNU_VERNEED, 0, 0, verneed_at, len(verneed), 1, len(needs), 8, 0
    )  # fmt: skip
    return header + strtab + verneed + sections


def test_smoke_reads_glibc_versions_without_objdump():
    """Where binutils is missing (a slim image: relay 4), the glibc floor's check reads the file's
    version-needs table itself: the same answer as `objdump -T`'s `GLIBC_x.y` names."""
    data = _elf_with_version_needs(
        {"libc.so.6": ["GLIBC_2.2.5", "GLIBC_2.28"], "libm.so.6": ["GLIBC_2.27", "GLIBC_PRIVATE"]}
    )
    assert smoke.elf_glibc_versions(data) == {(2, 2), (2, 28), (2, 27)}
    assert (
        smoke.elf_glibc_versions(_elf_with_version_needs({"libgcc_s.so.1": ["GCC_3.0"]})) == set()
    )
    for bad, why in (
        (b"MZ" + bytes(62), "not a 64-bit"),
        (b"\x7fELF\x01\x01" + bytes(58), "not a 64-bit"),
        (b"\x7fELF\x02\x01\x01" + bytes(57), "no section headers"),
    ):
        with pytest.raises(ValueError, match=why):
            smoke.elf_glibc_versions(bad)
    objdump = shutil.which("objdump")
    program = Path(sys.executable).resolve()
    if objdump is None or program.read_bytes()[:4] != b"\x7fELF":
        pytest.skip("objdump or an ELF Python needed for the comparison")
    printed = subprocess.run([objdump, "-T", str(program)], capture_output=True, text=True).stdout
    expected = {tuple(map(int, m)) for m in re.findall(r"GLIBC_(\d+)\.(\d+)", printed)}
    assert smoke.elf_glibc_versions(program.read_bytes()) == expected


check_install = load("check_install")


def test_check_install_names_and_source_archive(tmp_path, monkeypatch):
    """The check after a release (scripts/release/check_install.py): this machine's archives by
    the names archives.py gives them, GitHub's source archive unpacked without its top folder (and
    nothing outside it), a work folder that is not empty refused."""
    seen = set()
    for system, machine, triple in (
        ("Windows", "AMD64", "x86_64-pc-windows-msvc"),
        ("Windows", "ARM64", "aarch64-pc-windows-msvc"),
        ("Linux", "x86_64", "x86_64-unknown-linux-gnu"),
        ("Linux", "aarch64", "aarch64-unknown-linux-gnu"),
        ("Darwin", "arm64", "aarch64-apple-darwin"),
        ("Darwin", "x86_64", "x86_64-apple-darwin"),
    ):
        monkeypatch.setattr(check_install.platform, "system", lambda s=system: s)
        monkeypatch.setattr(check_install.platform, "machine", lambda m=machine: m)
        assert check_install.target_triple() == triple
        seen.add(triple)
    assert seen == set(archives.TARGETS)  # the six release targets, as archives.py names them
    monkeypatch.setattr(check_install.platform, "machine", lambda: "riscv64")
    with pytest.raises(SystemExit, match="no release files"):
        check_install.target_triple()
    names = check_install.archive_names("0.1.0rc1", "x86_64-pc-windows-msvc")
    assert names == [
        f"{p}-0.1.0rc1-x86_64-pc-windows-msvc.zip" for p in ("larmorx", "larmorx-nc", "larmorx-gpl")
    ]
    assert check_install.archive_names("0.1.0", "aarch64-apple-darwin")[1] == (
        "larmorx-nc-0.1.0-aarch64-apple-darwin.tar.gz"
    )
    data = io.BytesIO()
    with zipfile.ZipFile(data, "w") as z:
        z.writestr("larmorx-0.1.0rc1/pyproject.toml", "[project]\n")
        z.writestr("larmorx-0.1.0rc1/tests/golden/test_golden.py", "")
        z.writestr("larmorx-0.1.0rc1/crates-nc/larmorx-nc-cli/tests/golden.rs", "")
    source = check_install.unpack_source(data.getvalue(), tmp_path / "source")
    assert sorted(p.relative_to(source).as_posix() for p in source.rglob("*") if p.is_file()) == [
        "crates-nc/larmorx-nc-cli/tests/golden.rs",
        "pyproject.toml",
        "tests/golden/test_golden.py",
    ]
    evil = io.BytesIO()
    with zipfile.ZipFile(evil, "w") as z:
        z.writestr("top/pyproject.toml", "")
        z.writestr("top/../../outside.txt", "")
    with pytest.raises(check_install.Failure, match="outside"):
        check_install.unpack_source(evil.getvalue(), tmp_path / "evil")
    assert not (tmp_path / "outside.txt").exists()
    work = tmp_path / "work"
    work.mkdir()
    (work / "something").write_text("", encoding="utf-8")
    with pytest.raises(SystemExit):
        check_install.main(["--version", "0.1.0rc1", "--work", str(work)])
