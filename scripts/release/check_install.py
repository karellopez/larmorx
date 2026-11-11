# SPDX-License-Identifier: Apache-2.0
"""A published release checked on a machine of one's own, as a user installs it.

What the release workflow's `verify-pypi` jobs do on GitHub's runners, for any other machine
(RELEASING.md: Windows on a laptop after rc1's upload, through
`scripts/release/check_install_windows.ps1`; Linux and macOS the same way), with the standard
library only and nothing installed outside the work folder:

1. the machine: system, release, machine type, Python; on Windows whether Visual C++'s runtime
   DLLs are installed system-wide (only for the record: the release's programs need none);
2. a fresh venv in `<work>/venv` (`python -m venv`, then its pip upgraded);
3. `pip install --only-binary larmorx,larmorx-nc,larmorx-gpl "larmorx[all]==<version>"` from PyPI
   (retried while PyPI's CDN catches up), or from the wheels in `--find-links DIR` (their
   dependencies still from PyPI), then `pip check`;
4. the release's source tree, for its tests and release scripts (the golden tests also read the
   replicas' lists of golden cases, `crates*/*/tests/*golden.rs`): `--source DIR`, or GitHub's
   archive of the tag `v<version>` unpacked (`--source-ref` names another commit, branch or
   tag);
5. `scripts/release/smoke.py --installed` in the venv (the packages' versions, every program's
   `--version`, larmorx-nc's FSL data, the binaries' platform checks) and, unless
   `--no-archives`, `--archives` on this platform's three standalone archives, downloaded from
   the GitHub Release (or taken from `--archives DIR`);
6. the test group and `pytest tests/python tests/golden` on the installed packages, with
   LARMORX_NC_BIN and LARMORX_GPL_BIN at the installed programs (the golden suite: every
   platform reproduces larmorx's Linux x86_64 output).

Every step is reported with its time; the last line is `CHECK-INSTALL ok: <n> steps` or
`CHECK-INSTALL FAILED: <steps>`, the exit status 0 or 1. The work folder must be new (or
empty) and is left in place: the caller removes it (the Windows wrapper does). The temporary
files of every step (`TMPDIR`, `TEMP`, `TMP`: pip's builds and downloads, the tests' folders)
and pip's cache (`PIP_CACHE_DIR`) are kept in it too (`<work>/tmp`, `<work>/pip-cache`), so that
a small temporary space (Apptainer's `--containall` keeps `/tmp` and the home folder in 64 MiB)
is enough.

Usage:
    python -P scripts/release/check_install.py --version 0.1.0rc1 --work <new folder>
    python -P scripts/release/check_install.py --version 0.1.0rc1 --work <dir> \\
        --find-links <wheels> --source <checkout> --archives <archives>
"""

from __future__ import annotations

import argparse
import io
import os
import platform
import subprocess
import sys
import time
import urllib.error
import urllib.request
import zipfile
from pathlib import Path

REPOSITORY = "https://github.com/karellopez/larmorx"
PACKAGES = "larmorx,larmorx-nc,larmorx-gpl"
WINDOWS = os.name == "nt"


def target_triple() -> str:
    """This machine's target triple, as the release's archives name it."""
    machine = platform.machine().lower()
    arch = {"amd64": "x86_64", "x86_64": "x86_64", "arm64": "aarch64", "aarch64": "aarch64"}.get(
        machine
    )
    system = platform.system()
    if arch is None or system not in ("Linux", "Darwin", "Windows"):
        raise SystemExit(f"no release files for {system} {machine}")
    return {
        "Linux": f"{arch}-unknown-linux-gnu",
        "Darwin": f"{arch}-apple-darwin",
        "Windows": f"{arch}-pc-windows-msvc",
    }[system]


def archive_names(version: str, triple: str) -> list[str]:
    ext = ".zip" if "windows" in triple else ".tar.gz"
    return [f"{p}-{version}-{triple}{ext}" for p in ("larmorx", "larmorx-nc", "larmorx-gpl")]


def work_env(work: Path, environ: dict[str, str]) -> dict[str, str]:
    """The environment of every step: ``environ`` without the variables that would change
    what runs, with the temporary files and pip's cache in ``work`` (made here)."""
    env = dict(environ, PIP_DISABLE_PIP_VERSION_CHECK="1", PYTHONUTF8="1")
    for name in ("PYTHONPATH", "PYTHONHOME", "VIRTUAL_ENV", "LARMORX_NC_BIN", "LARMORX_GPL_BIN"):
        env.pop(name, None)
    tmp, cache = work / "tmp", work / "pip-cache"
    tmp.mkdir(parents=True, exist_ok=True)
    cache.mkdir(parents=True, exist_ok=True)
    env.update(TMPDIR=str(tmp), TEMP=str(tmp), TMP=str(tmp), PIP_CACHE_DIR=str(cache))
    return env


def venv_python(venv: Path) -> Path:
    return venv / ("Scripts/python.exe" if WINDOWS else "bin/python")


def venv_program(venv: Path, name: str) -> Path:
    return venv / ("Scripts" if WINDOWS else "bin") / f"{name}{'.exe' if WINDOWS else ''}"


class Failure(Exception):
    """A step that cannot go on (a download, an archive)."""


def download(url: str, tries: int = 3) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": "larmorx-check-install"})
    for attempt in range(1, tries + 1):
        try:
            with urllib.request.urlopen(request, timeout=120) as r:
                return r.read()
        except urllib.error.HTTPError as e:
            if e.code == 404 or attempt == tries:
                raise Failure(f"{url}: HTTP {e.code}") from None
        except OSError as e:
            if attempt == tries:
                raise Failure(f"{url}: {e}") from None
        time.sleep(10)
    raise AssertionError("unreachable")


def unpack_source(data: bytes, into: Path) -> Path:
    """GitHub's archive of the repository, without the archive's top folder."""
    with zipfile.ZipFile(io.BytesIO(data)) as z:
        names = z.namelist()
        top = names[0].split("/", 1)[0] + "/"
        for name in names:
            rel = name[len(top) :] if name.startswith(top) else ""
            if not rel or name.endswith("/"):
                continue
            path = into.joinpath(*rel.split("/"))
            if into.resolve() not in path.resolve().parents:
                raise Failure(f"{name}: outside the archive's folder")
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(z.read(name))
    if not (into / "pyproject.toml").is_file():
        raise Failure("the source archive holds no pyproject.toml")
    return into


class Steps:
    def __init__(self) -> None:
        self.done: list[str] = []
        self.failed: list[str] = []

    def run(self, label: str, command: list[str | Path], **kwargs) -> bool:
        print(f"== {label}", flush=True)
        start = time.monotonic()
        r = subprocess.run([str(c) for c in command], check=False, **kwargs)
        ok = r.returncode == 0
        self.note(label, ok, time.monotonic() - start, f"exit status {r.returncode}")
        return ok

    def note(self, label: str, ok: bool, seconds: float = 0.0, detail: str = "") -> bool:
        if ok:
            self.done.append(label)
            print(f"STEP ok ({seconds:.0f} s): {label}", flush=True)
        else:
            self.failed.append(label)
            print(
                f"STEP FAILED ({seconds:.0f} s): {label}{': ' + detail if detail else ''}",
                flush=True,
            )
        return ok


def machine_report() -> None:
    print(f"system: {platform.system()} {platform.release()} ({platform.version()})")
    print(f"machine: {platform.machine()}, target {target_triple()}")
    print(f"python: {sys.version.split()[0]} {platform.python_implementation()} ({sys.executable})")
    if WINDOWS:
        system32 = Path(os.environ.get("SYSTEMROOT", r"C:\Windows")) / "System32"
        dlls = ("vcruntime140.dll", "vcruntime140_1.dll", "msvcp140.dll")
        present = [d for d in dlls if (system32 / d).is_file()]
        print(f"Visual C++ runtime DLLs in System32 (for the record): {present or 'none'}")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--version", required=True, help="the release's version (PEP 440)")
    parser.add_argument("--work", type=Path, required=True, help="a new (or empty) folder")
    parser.add_argument("--find-links", type=Path, help="install our wheels from this folder")
    parser.add_argument("--source", type=Path, help="the release's source tree (a checkout)")
    parser.add_argument("--source-ref", help="the commit, branch or tag of GitHub's archive")
    parser.add_argument("--archives", type=Path, help="the folder of the standalone archives")
    parser.add_argument("--no-archives", action="store_true", help="check no archives")
    parser.add_argument(
        "--target", help="the archives' target triple (default: this machine's, as released)"
    )
    parser.add_argument("--tries", type=int, default=20, help="pip install from PyPI (30 s apart)")
    args = parser.parse_args(argv)
    if args.source and args.source_ref:
        parser.error("--source or --source-ref, not both")
    if args.archives and args.no_archives:
        parser.error("--archives or --no-archives, not both")
    work = args.work.resolve()
    if work.exists() and any(work.iterdir()):
        parser.error(f"{work} is not empty")
    work.mkdir(parents=True, exist_ok=True)
    version, triple, steps = args.version, args.target or target_triple(), Steps()
    print(f"check-install of larmorx {version} in {work}")
    machine_report()
    env = work_env(work, dict(os.environ))

    venv = work / "venv"
    py = venv_python(venv)
    if not steps.run(
        "a fresh venv", [sys.executable, "-m", "venv", venv], env=env
    ) or not steps.run(
        "its pip upgraded", [py, "-m", "pip", "install", "-q", "--upgrade", "pip"], env=env
    ):
        return finish(steps)
    install = [py, "-m", "pip", "install", "--only-binary", PACKAGES]
    if args.find_links:
        install += ["--find-links", args.find_links.resolve()]
    install.append(f"larmorx[all]=={version}")
    label = f"pip install larmorx[all]=={version}" + (
        f" (our wheels from {args.find_links})" if args.find_links else " from PyPI"
    )
    tries = 1 if args.find_links else max(1, args.tries)
    start = time.monotonic()
    for attempt in range(1, tries + 1):
        print(f"== {label} (try {attempt} of {tries})", flush=True)
        if subprocess.run([str(c) for c in install], check=False, env=env).returncode == 0:
            steps.note(label, True, time.monotonic() - start)
            break
        if attempt < tries:
            time.sleep(30)
    else:
        steps.note(label, False, time.monotonic() - start, f"{tries} tries")
        return finish(steps)
    steps.run("pip check", [py, "-m", "pip", "check"], env=env)
    subprocess.run([str(py), "-m", "pip", "list"], check=False, env=env)

    if args.source:
        source = args.source.resolve()
        steps.note(f"the source tree: {source}", (source / "pyproject.toml").is_file())
    else:
        ref = args.source_ref or f"v{version}"
        url = f"{REPOSITORY}/archive/{ref}.zip"
        start = time.monotonic()
        try:
            source = unpack_source(download(url), work / "source")
            steps.note(f"the source tree from {url}", True, time.monotonic() - start)
        except Failure as e:
            steps.note(f"the source tree from {url}", False, time.monotonic() - start, str(e))
            return finish(steps)
    smoke = [
        py,
        "-P",
        source / "scripts" / "release" / "smoke.py",
        "--version",
        version,
        "--installed",
    ]
    if not args.no_archives:
        archives = args.archives.resolve() if args.archives else work / "archives"
        if not args.archives:
            archives.mkdir()
            start = time.monotonic()
            try:
                for name in archive_names(version, triple):
                    url = f"{REPOSITORY}/releases/download/v{version}/{name}"
                    (archives / name).write_bytes(download(url))
                steps.note(
                    "the standalone archives from the GitHub Release",
                    True,
                    time.monotonic() - start,
                )
            except Failure as e:
                steps.note("the standalone archives from the GitHub Release", False, 0, str(e))
        smoke += ["--archives", archives, "--target", triple]
    steps.run(
        "smoke.py: the installed packages" + ("" if args.no_archives else " and the archives"),
        smoke,
        env=env,
        cwd=work,
    )

    if steps.run(
        "the test group", [py, "-m", "pip", "install", "-q", "--group", "test"], env=env, cwd=source
    ):
        test_env = dict(
            env,
            LARMORX_NC_BIN=str(venv_program(venv, "larmorx-nc")),
            LARMORX_GPL_BIN=str(venv_program(venv, "larmorx-gpl")),
            LARMORX_GOLDEN_MISMATCHES=str(work / "golden-mismatches"),
            PYTHONDONTWRITEBYTECODE="1",
        )
        steps.run(
            "pytest tests/python tests/golden",
            [py, "-m", "pytest", "-q", "-p", "no:cacheprovider", "tests/python", "tests/golden"],
            env=test_env,
            cwd=source,
        )
    return finish(steps)


def finish(steps: Steps) -> int:
    if steps.failed:
        print(f"CHECK-INSTALL FAILED: {'; '.join(steps.failed)}")
        return 1
    print(f"CHECK-INSTALL ok: {len(steps.done)} steps")
    return 0


if __name__ == "__main__":
    sys.exit(main())
