# SPDX-License-Identifier: Apache-2.0
"""The release's helpers (scripts/release/): one version everywhere, the licence expressions,
PyPI's descriptions, the companions' source distributions, the third-party crates' licence texts
and the check of Windows files' imported DLLs."""

from __future__ import annotations

import fnmatch
import importlib.util
import re
import shutil
import struct
import sys
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
