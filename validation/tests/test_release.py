# SPDX-License-Identifier: Apache-2.0
"""The release's helpers (scripts/release/): one version everywhere, the licence expressions,
PyPI's descriptions, the companions' source distributions, the third-party crates' licence texts,
the check of Windows files' imported DLLs, and the release workflow's own: the release notes, the
standalone archives, the set of files with SHA256SUMS, PyPI's state, the tagged commit's CI, and
the workflow's environments and pins."""

from __future__ import annotations

import fnmatch
import hashlib
import importlib.util
import re
import shutil
import struct
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
    assert text.count("if: needs.check.outputs.publish == 'true'") == 5
