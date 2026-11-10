# SPDX-License-Identifier: Apache-2.0
"""The release's smoke checks of the installed packages and the standalone archives.

`--installed`: in the Python environment that runs this script, the three packages' versions
(`importlib.metadata`, `larmorx.__version__`), every command's `--version` line (`larmorx`,
`lx`, `larmorprepx`, `python -m larmorx`, `larmorx-nc`, `larmorx-gpl`, from the environment's
scripts directory), and larmorx-nc's FSL data where the wheel installs it
(`<prefix>/share/larmorx-nc/fsl-data/`, every file of its MANIFEST.tsv with its SHA-256).

`--archives DIR --target T`: the three archives of that platform (`scripts/release/archives.py`)
unpacked into a temporary folder: their licence files, each program's `--version` (`lx` through
its link or `lx.cmd`), `lx --help` naming itself (Unix), larmorx-nc's FSL data beside `bin/`.

On every program and extension found: Linux, no glibc symbol newer than 2.28 (`objdump -T`: the
oldest glibc the release supports); macOS, only the system's libraries (`otool -L`) and a minimum
macOS of at most 11.0 (`otool -l`); Windows, no C runtime DLL and the runner's machine type
(`scripts/release/pe_imports.py`).

Last line `SMOKE ok: <n> checks` or `SMOKE FAILED: <n> of <m> checks`; in GitHub Actions each
failure is also an error annotation. Exit status 0 or 1.

Usage:
    python -P scripts/release/smoke.py --version 0.1.0rc1 --installed
    python -P scripts/release/smoke.py --version 0.1.0rc1 --archives archives --target <triple>
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.metadata
import importlib.util
import os
import platform
import re
import shutil
import subprocess
import sys
import sysconfig
import tarfile
import tempfile
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
GLIBC_FLOOR = (2, 28)
MACOS_FLOOR = (11, 0)
WINDOWS = os.name == "nt"
EXE = ".exe" if WINDOWS else ""


class Checks:
    def __init__(self) -> None:
        self.n, self.failed = 0, []

    def check(self, ok: bool, what: str, detail: str = "") -> bool:
        self.n += 1
        if ok:
            print(f"ok    {what}")
        else:
            message = f"{what}: {detail}" if detail else what
            self.failed.append(message)
            print(f"FAIL  {message}")
            if os.environ.get("GITHUB_ACTIONS") == "true":
                print(f"::error title=release smoke check::{message}")
        return ok

    def version_line(self, command: list[str], expected: str, label: str) -> None:
        try:
            r = subprocess.run(command, capture_output=True, text=True, timeout=120, check=False)
        except OSError as e:
            self.check(False, f"{label} --version", str(e))
            return
        got = r.stdout.strip()
        self.check(
            r.returncode == 0 and got == expected,
            f"{label} --version prints {expected!r}",
            f"exit status {r.returncode}, printed {got!r} {r.stderr.strip()[:300]!r}",
        )


def _manifest(checks: Checks, data: Path, label: str) -> None:
    """Every file of fsl-data's MANIFEST.tsv present with its size and SHA-256."""
    manifest = data / "MANIFEST.tsv"
    if not checks.check(manifest.is_file(), f"{label}: {manifest.name} present", str(manifest)):
        return
    rows = manifest.read_text(encoding="utf-8").splitlines()[1:]
    bad = []
    for row in rows:
        name, size, sha = row.split("\t")[:3]
        path = data / name
        if not path.is_file() or path.stat().st_size != int(size):
            bad.append(f"{name} missing or of another size")
        elif hashlib.sha256(path.read_bytes()).hexdigest() != sha:
            bad.append(f"{name} has another SHA-256")
    checks.check(bool(rows) and not bad, f"{label}: {len(rows)} FSL data files", "; ".join(bad))


def installed(checks: Checks, version: str) -> list[Path]:
    """The installed packages' checks; returns the binaries found."""
    for dist in ("larmorx", "larmorx-nc", "larmorx-gpl"):
        try:
            got = importlib.metadata.version(dist)
        except importlib.metadata.PackageNotFoundError:
            got = "not installed"
        checks.check(got == version, f"{dist} {version} installed", got)
    import larmorx

    checks.check(larmorx.__version__ == version, "larmorx.__version__", larmorx.__version__)
    scripts = Path(sysconfig.get_path("scripts"))
    binaries = []
    for name, expected in (
        ("larmorx", "larmorx"),
        ("lx", "larmorx"),
        ("larmorprepx", "larmorprepx"),
        ("larmorx-nc", "larmorx-nc"),
        ("larmorx-gpl", "larmorx-gpl"),
    ):
        found = shutil.which(name, path=str(scripts))
        if checks.check(found is not None, f"{name} in {scripts}"):
            checks.version_line([str(found), "--version"], f"{expected} {version}", name)
            if name.startswith("larmorx-"):
                binaries.append(Path(str(found)))
    checks.version_line(
        [sys.executable, "-m", "larmorx", "--version"], f"larmorx {version}", "python -m larmorx"
    )
    spec = importlib.util.find_spec("larmorx._core")
    if spec is not None and spec.origin:
        binaries.append(Path(spec.origin))
    data = Path(sysconfig.get_path("data")) / "share" / "larmorx-nc" / "fsl-data"
    _manifest(checks, data, "installed larmorx-nc")
    return binaries


def _unpack(archive: Path, into: Path) -> None:
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as z:
            z.extractall(into)
            for info in z.infolist():  # zipfile drops the permissions
                mode = info.external_attr >> 16
                if mode & 0o111 and not WINDOWS:
                    (into / info.filename).chmod(0o755)
    else:
        with tarfile.open(archive) as t:
            t.extractall(into, filter="data")


def archives(checks: Checks, version: str, folder: Path, target: str, tmp: Path) -> list[Path]:
    """The three archives' checks; returns the programs found."""
    ext = ".zip" if "windows" in target else ".tar.gz"
    binaries = []
    for package in ("larmorx", "larmorx-nc", "larmorx-gpl"):
        name = f"{package}-{version}-{target}"
        archive = folder / f"{name}{ext}"
        if not checks.check(archive.is_file(), f"{archive.name} present", str(folder)):
            continue
        _unpack(archive, tmp)
        top = tmp / name
        for required in ("LICENSE", "NOTICE", "LICENSES/third-party-rust.txt", "README.txt"):
            checks.check((top / required).is_file(), f"{name}: {required}")
        program = top / "bin" / f"{package}{EXE}"
        if not checks.check(program.is_file(), f"{name}: bin/{program.name}"):
            continue
        binaries.append(program)
        checks.version_line([str(program), "--version"], f"{package} {version}", program.name)
        if package == "larmorx":
            if WINDOWS:
                lx = top / "bin" / "lx.cmd"
                if checks.check(lx.is_file(), f"{name}: bin/lx.cmd"):
                    command = [os.environ.get("COMSPEC", "cmd.exe"), "/c", str(lx), "--version"]
                    checks.version_line(command, f"larmorx {version}", "lx.cmd")
            else:
                lx = top / "bin" / "lx"
                checks.check(lx.is_symlink(), f"{name}: bin/lx is a link to larmorx")
                checks.version_line([str(lx), "--version"], f"larmorx {version}", "lx")
                r = subprocess.run([str(lx), "--help"], capture_output=True, text=True, timeout=60)
                checks.check("Usage: lx " in r.stdout, f"{name}: lx --help names lx")
        if package == "larmorx-nc":
            _manifest(checks, top / "share" / "larmorx-nc" / "fsl-data", name)
            checks.check(
                (top / "source" / "fsl-source" / "MANIFEST.tsv").is_file(),
                f"{name}: FSL's original source (source/fsl-source/)",
            )
            checks.check(
                any((top / "source").glob("larmorx-nc-*/src/*.rs")),
                f"{name}: the amended source (source/larmorx-nc-*/src/)",
            )
        if package == "larmorx-gpl":
            checks.check(
                any((top / "source").glob("larmorx-gpl-*/src/*.rs")),
                f"{name}: the program's source (source/larmorx-gpl-*/src/)",
            )
    return binaries


def _run(command: list[str]) -> str:
    return subprocess.run(command, capture_output=True, text=True, check=True).stdout


def binaries(checks: Checks, paths: list[Path]) -> None:
    system = platform.system()
    if system == "Linux" and not checks.check(shutil.which("objdump") is not None, "objdump"):
        return
    for path in paths:
        if system == "Linux":
            versions = {
                tuple(map(int, m.groups()))
                for m in re.finditer(r"GLIBC_(\d+)\.(\d+)", _run(["objdump", "-T", str(path)]))
            }
            newest = max(versions, default=(0, 0))
            checks.check(
                newest <= GLIBC_FLOOR,
                f"{path.name}: glibc symbols at most {GLIBC_FLOOR[0]}.{GLIBC_FLOOR[1]}",
                f"needs GLIBC_{newest[0]}.{newest[1]}",
            )
        elif system == "Darwin":
            deps = _run(["otool", "-L", str(path)]).splitlines()[1:]
            libs = [d.strip().split(" (")[0] for d in deps if d.strip()]
            foreign = [x for x in libs if not x.startswith(("/usr/lib/", "/System/Library/"))]
            checks.check(not foreign, f"{path.name}: only the system's libraries", str(foreign))
            minos = re.findall(r"\bminos (\d+)\.(\d+)", _run(["otool", "-l", str(path)]))
            floor = max((tuple(map(int, m)) for m in minos), default=(99, 0))
            checks.check(
                floor <= MACOS_FLOOR,
                f"{path.name}: runs on macOS {MACOS_FLOOR[0]}.{MACOS_FLOOR[1]}",
                f"minimum macOS {floor[0]}.{floor[1]}",
            )
        elif system == "Windows":
            spec = importlib.util.spec_from_file_location("release_pe", HERE / "pe_imports.py")
            assert spec is not None and spec.loader is not None
            pe = importlib.util.module_from_spec(spec)
            sys.modules[spec.name] = pe
            spec.loader.exec_module(pe)
            info = pe.parse(path.read_bytes())
            machine = {"AMD64": "x86_64", "ARM64": "aarch64"}.get(platform.machine().upper())
            checks.check(not info.forbidden, f"{path.name}: no C runtime DLL", str(info.forbidden))
            checks.check(info.machine == machine, f"{path.name}: {machine}", info.machine)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--version", required=True, help="the release's version (PEP 440)")
    parser.add_argument("--installed", action="store_true", help="check this environment")
    parser.add_argument("--archives", type=Path, help="the folder of the standalone archives")
    parser.add_argument("--target", help="the archives' target triple (with --archives)")
    args = parser.parse_args(argv)
    if not args.installed and not args.archives:
        parser.error("nothing to check: --installed and/or --archives")
    if args.archives and not args.target:
        parser.error("--archives needs --target")
    checks = Checks()
    found: list[Path] = []
    with tempfile.TemporaryDirectory(prefix="larmorx-smoke-") as tmp:
        if args.installed:
            found += installed(checks, args.version)
        if args.archives:
            found += archives(checks, args.version, args.archives, args.target, Path(tmp))
        binaries(checks, found)
    if checks.failed:
        print(f"SMOKE FAILED: {len(checks.failed)} of {checks.n} checks")
        return 1
    print(f"SMOKE ok: {checks.n} checks")
    return 0


if __name__ == "__main__":
    sys.exit(main())
