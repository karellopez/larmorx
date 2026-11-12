# SPDX-License-Identifier: Apache-2.0
"""Build FreeSurfer 8.2.0's unit oracle: its ``utils`` library and the packages it links, from the
tag, with GCC 8.4.0 and the official build's flags, against ITK 4.13.2; then the harness
``fsunit`` (one library function per call) and three of FreeSurfer's programs built from the same
objects (the build's proof against the official binaries).

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: it builds FreeSurfer's source and mirrors its build
files (docs/freesurfer/code.md section 1).

    python -P validation/investigations/freesurfer-unit-oracle/build.py all [--jobs 10]
    python -P .../build.py toolchain | itk | freesurfer | info

Everything goes under ``<workspace>/oracles/freesurfer-8.2.0-unit/`` (never the repository):

- ``toolchain/``: the compiler, ``gcc``/``g++`` wrappers. By default (``--compiler ubuntu``) the
  official build's own compiler: Ubuntu 20.04's ``gcc-8`` 8.4.0-3ubuntu2 packages (every one of
  the official binaries' ``.comment`` names it), downloaded from Ubuntu's archive, checked
  against Ubuntu's signed package index and unpacked without administrator rights into
  ``<workspace>/oracles/toolchains/gcc-8.4-ubuntu`` (its own defaults: hardening flags, the
  multiarch headers, ``.init_array`` constructors; this machine's binutils, glibc 2.35 headers
  and libraries, as Ubuntu 22.04's official build had them: the binaries import
  ``exp@GLIBC_2.29`` and ``__libc_start_main@GLIBC_2.34``). ``--compiler conda`` uses the
  preparation's conda-forge GCC 8.4.0 (``oracles/toolchains/gcc-8.4``) behind wrappers that make it
  behave as Ubuntu's (the machine's headers and libraries, Ubuntu's default flags, no run path
  into the conda folder); its code is the same instruction for instruction, but its own start
  files and its ``.ctors`` constructors differ (README.md);
- ``src/InsightToolkit-4.13.2``: ITK's release tarball (its SHA-256 checked), and
  ``src/fs-shadow``: a tree of links to the tag's folders with this folder's ``top.cmake`` as its
  ``CMakeLists.txt`` (the official top-level file's settings for these targets, in its order), so
  FreeSurfer's own ``packages/``, ``utils/`` and programs' CMake files build unchanged;
- ``itk-4.13.2/``: ITK installed as ``packages/source/build_itk.sh`` builds it (static, Release,
  ``-msse2 -mfpmath=sse``, C++11, the six module groups and AnisotropicDiffusionLBR);
- ``build/fs/``: FreeSurfer's build tree (``utils/libutils.a``, ``packages/*/lib*.a``, the
  programs, ``fsunit``, ``compile_commands.json``);
- ``bin/``: ``fsunit`` and the three programs, ``BUILD_INFO.txt`` (what was built, from what, the
  compilers, every tool's SHA-256).

Logs: ``<workspace>/oracles/logs/freesurfer-unit-<step>.log``. Run it in a memory box
(``scripts/orchestration/boxed.sh 16``); ``--jobs`` sets the compile jobs (default 10).
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]

ITK_URL = (
    "https://github.com/InsightSoftwareConsortium/ITK/releases/download/v4.13.2/"
    "InsightToolkit-4.13.2.tar.gz"
)
ITK_SHA256 = "d8760b279de20497c432e7cdf97ed349277da1ae435be1f6f0f00fbe8d4938c1"
ITK_NAME = "InsightToolkit-4.13.2"

#: The official package's version and build stamp (``build-stamp.txt`` of
#: ``freesurfer_ubuntu22-8.2.0_amd64.deb``): configured into ``utils/version_info.h``.
FS_VERSION = "8.2.0"
BUILD_STAMP = "freesurfer-linux-ubuntu22_x86_64-8.2.0-20260314-d932c45"
#: When the official build compiled ``utils/version.cpp``: its ``__DATE__ " " __TIME__`` (``Mar 14
#: 2026 12:21:06``, the rodata and an immediate of ``getAllInfo``), given to GCC as
#: ``SOURCE_DATE_EPOCH`` (UTC): every ``__DATE__`` (MRIwrite's NIfTI ``descrip``, "FreeSurfer Mar 14
#: 2026") and that ``__TIME__`` as the official programs hold them.
BUILD_EPOCH = 1773490866

#: The tag's folders the build reads (linked into the shadow tree).
FS_DIRS = ("cmake", "include", "packages", "utils")
#: FreeSurfer programs built from the same objects: the queue's proof (item 3.1-02).
PROGRAMS = ("mri_add_xform_to_header", "mri_normalize", "mris_smooth")

#: Ubuntu 20.04's gcc-8 8.4.0-3ubuntu2, the official build's compiler: the packages (pool path,
#: SHA-256 as Ubuntu's focal package index lists them) and the signed index that lists them.
UBUNTU_ARCHIVE = "http://archive.ubuntu.com/ubuntu/"
UBUNTU_DEBS = {
    "pool/universe/g/gcc-8/gcc-8_8.4.0-3ubuntu2_amd64.deb": "dde03b929f05f6e8318387bf5818877b1d972de7a697a000fa4c516af5e68d7f",
    "pool/universe/g/gcc-8/g++-8_8.4.0-3ubuntu2_amd64.deb": "0ce2c4174400cbd32a3e7cec830331dab1338f1e82d35fc8f963fc20a7d77270",
    "pool/universe/g/gcc-8/cpp-8_8.4.0-3ubuntu2_amd64.deb": "133fb9dcba0e1ba4bd06deb458b746d6b370791699154d5aa4989db412ad2fec",
    "pool/universe/g/gcc-8/gcc-8-base_8.4.0-3ubuntu2_amd64.deb": "db96f03b486f76e2a49ed6c59365d1673b73c42bf172aaa29715478687d2ae6d",
    "pool/universe/g/gcc-8/libgcc-8-dev_8.4.0-3ubuntu2_amd64.deb": "8733725ae1d9d320f83253111eb116789dde67f83384fde7940287c3c147de6b",
    "pool/universe/g/gcc-8/libstdc++-8-dev_8.4.0-3ubuntu2_amd64.deb": "5c23053a1c1332b48c44984bb28d38cf487f83cfb08e938918e5854d750a9f0d",
    "pool/universe/g/gcc-8/libmpx2_8.4.0-3ubuntu2_amd64.deb": "9022934915c4ed8d39086f06a27a8c19be90436767f3b5dce88c9f7569e445bc",
    "pool/main/i/isl/libisl22_0.22.1-1_amd64.deb": "67cadab06a50a29cde11c1c1d02847afbae6f0465686481c9106577777b88a44",
}  # fmt: skip
#: The focal package indexes and their SHA-256 as the signed ``dists/focal/InRelease`` lists them.
UBUNTU_INDEXES = {
    "dists/focal/universe/binary-amd64/Packages.gz": "200acdc3421757fa8f8759c1cedacae2cf8f5821d7810ca59d8a313c5b3ae71e",
    "dists/focal/main/binary-amd64/Packages.gz": "f25bb719a900d962a4df25cbb20f0a54a23d9f16c3fcdc4f4872ead131f5a604",
}  # fmt: skip
UBUNTU_KEYRING = Path("/usr/share/keyrings/ubuntu-archive-keyring.gpg")

UBUNTU_WRAPPER = """#!/usr/bin/env bash
# Generated by validation/investigations/freesurfer-unit-oracle/build.py: Ubuntu's gcc-8
# (8.4.0-3ubuntu2) unpacked in {prefix}; its cc1 needs focal's libisl.so.22 beside it.
export LD_LIBRARY_PATH="{prefix}/usr/lib/x86_64-linux-gnu${{LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}}"
exec "{prefix}/usr/bin/{name}-8" "$@"
"""

#: Ubuntu's gcc-8 (8.4.0-3ubuntu2) turns these on by default; conda-forge's GCC 8.4.0 does not
#: (the official binaries hold endbr64 and stack canaries: variation.md section 1).
UBUNTU_DEFAULTS = (
    "-fstack-protector-strong",
    "-fstack-clash-protection",
    "-fcf-protection=full",
    "-Wformat",
    "-Wformat-security",
)

CONDA_WRAPPER = """#!/usr/bin/env bash
# Generated by validation/investigations/freesurfer-unit-oracle/build.py: conda-forge's GCC 8.4.0
# as Ubuntu's gcc-8 on this machine (its headers and libraries, Ubuntu's default flags).
fortify=()
for a in "$@"; do
  case "$a" in -O|-O[1-3s]|-Ofast) fortify=(-D_FORTIFY_SOURCE=2);; -O0) fortify=();; esac
done
exec "{compiler}" --sysroot=/ -B/usr/lib/x86_64-linux-gnu \\
  -idirafter /usr/include/x86_64-linux-gnu -specs="{specs}" \\
  {defaults} "${{fortify[@]}}" "$@" \\
  -L"{lib}" -L/usr/lib/x86_64-linux-gnu -L/lib/x86_64-linux-gnu
"""

#: conda's spec adds ``-rpath <conda>/lib`` to every link (``link_libgcc``): replaced by its
#: plain value, so the programs load the machine's libraries as the official ones do.
SPECS = "*link_libgcc:\n%D\n\n"


def workspace() -> Path:
    env = os.environ.get("LARMORX_WORKSPACE")
    if env:
        return Path(env)
    common = subprocess.run(
        ["git", "-C", str(REPO), "rev-parse", "--path-format=absolute", "--git-common-dir"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()
    return Path(common).parent.parent


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


class Paths:
    def __init__(
        self, ws: Path, out: Path, gcc: Path, tag: Path, official: Path, compiler: str
    ) -> None:
        self.ws, self.out, self.gcc, self.tag, self.official = ws, out, gcc, tag, official
        self.compiler = compiler
        self.ubuntu = ws / "oracles" / "toolchains" / "gcc-8.4-ubuntu"
        self.ubuntu_downloads = ws / "oracles" / "downloads" / "gcc-8-focal"
        self.toolchain = out / "toolchain"
        self.cc = self.toolchain / "bin" / "gcc"
        self.cxx = self.toolchain / "bin" / "g++"
        self.src = out / "src"
        self.itk_src = self.src / ITK_NAME
        self.shadow = self.src / "fs-shadow"
        self.itk_build = out / "build" / "itk"
        self.itk = out / "itk-4.13.2"
        self.fs_build = out / "build" / "fs"
        self.bin = out / "bin"
        self.logs = ws / "oracles" / "logs"
        self.tarball = ws / "oracles" / "downloads" / f"{ITK_NAME}.tar.gz"


def run(cmd: list[str], log: Path, *, cwd: Path | None = None, env: dict | None = None) -> None:
    log.parent.mkdir(parents=True, exist_ok=True)
    with log.open("a", encoding="utf-8") as f:
        f.write("$ " + " ".join(cmd) + "\n")
        f.flush()
        p = subprocess.run(cmd, cwd=cwd, env=env, stdout=f, stderr=subprocess.STDOUT, check=False)
        f.write(f"[exit {p.returncode}]\n")
    if p.returncode != 0:
        sys.exit(f"failed ({p.returncode}): {' '.join(cmd)}; log {log}")


def build_env(p: Paths) -> dict:
    env = dict(os.environ)
    env["SOURCE_DATE_EPOCH"] = str(BUILD_EPOCH)
    # FSL's own make is on some PATHs (env.sh drops it); the wrappers come first
    path = [d for d in env.get("PATH", "").split(":") if "/fsl" not in d]
    env["PATH"] = ":".join([str(p.toolchain / "bin"), *path])
    for v in ("CFLAGS", "CXXFLAGS", "LDFLAGS", "CPPFLAGS", "FSLDIR"):
        env.pop(v, None)
    env["CC"], env["CXX"] = str(p.cc), str(p.cxx)
    return env


def _download(url: str, dest: Path) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    tmp = dest.with_name(dest.name + ".part")
    with urllib.request.urlopen(url) as r, tmp.open("wb") as f:
        shutil.copyfileobj(r, f)
    tmp.rename(dest)


def _packages_sha256(index: Path, names: set[str]) -> dict[str, str]:
    """``Filename``'s base name -> ``SHA256`` from a Debian ``Packages.gz``."""
    out: dict[str, str] = {}
    cur: dict[str, str] = {}
    with gzip.open(index, "rt", encoding="utf-8", errors="replace") as f:
        for raw in [*f, "\n"]:
            line = raw.rstrip("\n")
            if not line:
                name = cur.get("Filename", "").rsplit("/", 1)[-1]
                if name in names:
                    out[name] = cur.get("SHA256", "")
                cur = {}
            elif ":" in line and not line.startswith(" "):
                k, v = line.split(":", 1)
                cur[k] = v.strip()
    return out


def _local(d: Path, rel: str) -> Path:
    """Where a downloaded archive file is kept: an index by its whole path, a package by name."""
    return d / rel.replace("/", "_") if rel.startswith("dists/") else d / rel.rsplit("/", 1)[-1]


def install_ubuntu_gcc(p: Paths) -> None:
    """Ubuntu's gcc-8 8.4.0-3ubuntu2, checked and unpacked (no administrator rights)."""
    d = p.ubuntu_downloads
    for rel in (*UBUNTU_DEBS, *UBUNTU_INDEXES, "dists/focal/InRelease"):
        if not _local(d, rel).exists():
            _download(UBUNTU_ARCHIVE + rel, _local(d, rel))
    # the chain: Ubuntu's archive key -> InRelease -> Packages.gz -> each package
    inrelease = _local(d, "dists/focal/InRelease")
    if UBUNTU_KEYRING.exists() and shutil.which("gpgv"):
        v = subprocess.run(
            ["gpgv", "--keyring", str(UBUNTU_KEYRING), str(inrelease)],
            capture_output=True,
            text=True,
            check=False,
        )
        if v.returncode != 0:
            sys.exit(f"{inrelease}: Ubuntu's signature does not verify:\n{v.stderr}")
    release = inrelease.read_text(encoding="utf-8", errors="replace")
    names = {rel.rsplit("/", 1)[-1] for rel in UBUNTU_DEBS}
    listed: dict[str, str] = {}
    for rel, want in UBUNTU_INDEXES.items():
        f = _local(d, rel)
        got = sha256(f)
        if got != want or f"{want} " not in release:
            sys.exit(f"{f}: SHA-256 {got}, not the one focal's InRelease lists ({want})")
        listed.update(_packages_sha256(f, names))
    for rel, want in UBUNTU_DEBS.items():
        f = _local(d, rel)
        got = sha256(f)
        if got != want or listed.get(f.name) != want:
            sys.exit(f"{f}: SHA-256 {got}; pinned {want}; focal's index {listed.get(f.name)}")
    if not (p.ubuntu / "usr" / "bin" / "g++-8").exists():
        p.ubuntu.mkdir(parents=True, exist_ok=True)
        for rel in UBUNTU_DEBS:
            subprocess.run(["dpkg-deb", "-x", str(_local(d, rel)), str(p.ubuntu)], check=True)
    # the packages' links to the runtime libraries point into the unpacked tree's multiarch
    # folder: there they name this machine's own (the official programs load those)
    lib = p.ubuntu / "usr" / "lib" / "x86_64-linux-gnu"
    lib.mkdir(parents=True, exist_ok=True)
    for link in (p.ubuntu / "usr" / "lib" / "gcc" / "x86_64-linux-gnu" / "8").iterdir():
        if link.is_symlink() and not link.exists():
            name = Path(os.readlink(link)).name
            host = Path("/usr/lib/x86_64-linux-gnu") / name
            dest = lib / name
            if host.exists() and not dest.is_symlink():
                dest.symlink_to(host)


def step_toolchain(p: Paths) -> None:
    (p.toolchain / "bin").mkdir(parents=True, exist_ok=True)
    if p.compiler == "ubuntu":
        install_ubuntu_gcc(p)
        for name in ("gcc", "g++"):
            w = p.toolchain / "bin" / name
            w.write_text(UBUNTU_WRAPPER.format(prefix=p.ubuntu, name=name), encoding="utf-8")
            w.chmod(0o755)
    else:
        conda_wrappers(p)
    for alias, name in (("cc", "gcc"), ("c++", "g++")):
        a = p.toolchain / "bin" / alias
        if a.is_symlink() or a.exists():
            a.unlink()
        a.symlink_to(name)
    print(f"toolchain ({p.compiler}): {p.toolchain} ({_version(p.cxx)})")


def conda_wrappers(p: Paths) -> None:
    compilers = {
        "gcc": p.gcc / "bin" / "x86_64-conda-linux-gnu-gcc",
        "g++": p.gcc / "bin" / "x86_64-conda-linux-gnu-g++",
    }
    for c in compilers.values():
        if not c.exists():
            sys.exit(f"GCC 8.4.0 not found: {c}")
    lib = p.toolchain / "lib"
    lib.mkdir(parents=True, exist_ok=True)
    specs = p.toolchain / "host.specs"
    specs.write_text(SPECS, encoding="utf-8")
    # the machine's OpenMP and libgcc_s, as the official programs load them (libgcc.a: GCC 8.4.0's)
    links = {
        "libgomp.so": Path("/usr/lib/x86_64-linux-gnu/libgomp.so.1"),
        "libgcc_s.so": Path("/lib/x86_64-linux-gnu/libgcc_s.so.1"),
    }
    for name, target in links.items():
        if not target.exists():
            sys.exit(f"missing {target}")
        dest = lib / name
        if dest.is_symlink() or dest.exists():
            dest.unlink()
        dest.symlink_to(target)
    for name, compiler in compilers.items():
        w = p.toolchain / "bin" / name
        w.write_text(
            CONDA_WRAPPER.format(
                compiler=compiler, specs=specs, lib=lib, defaults=" ".join(UBUNTU_DEFAULTS)
            ),
            encoding="utf-8",
        )
        w.chmod(0o755)


def fetch_itk(p: Paths) -> None:
    if not p.tarball.exists():
        p.tarball.parent.mkdir(parents=True, exist_ok=True)
        tmp = p.tarball.with_suffix(".part")
        with urllib.request.urlopen(ITK_URL) as r, tmp.open("wb") as f:
            shutil.copyfileobj(r, f)
        tmp.rename(p.tarball)
    got = sha256(p.tarball)
    if got != ITK_SHA256:
        sys.exit(f"{p.tarball}: SHA-256 {got}, expected {ITK_SHA256}")
    if not (p.itk_src / "CMakeLists.txt").exists():
        p.src.mkdir(parents=True, exist_ok=True)
        with tarfile.open(p.tarball) as t:
            t.extractall(p.src, filter="data")


def official_itk_source_dir(official: Path) -> str:
    """The folder the official build compiled ITK 4.13.2 in, from the ``__FILE__`` strings of
    VNL's messages in one of its programs (read from the binary, never written into the
    repository): ``-fmacro-prefix-map`` gives ITK's messages and their lengths (immediates in the
    code) the official text."""
    data = (official / "bin" / "mri_add_xform_to_header").read_bytes()
    m = re.search(
        rb"\x00(/[\x21-\x7e]+?)/Modules/ThirdParty/VNL/src/vxl/core/vnl/algo/vnl_svd\.hxx", data
    )
    if not m:
        sys.exit(f"no ITK __FILE__ path in {official}/bin/mri_add_xform_to_header")
    return m.group(1).decode()


def step_itk(p: Paths, jobs: int) -> None:
    fetch_itk(p)
    # build_itk.sh's flags; the prefix map changes only the text of __FILE__
    flags = (
        f"-msse2 -mfpmath=sse -fmacro-prefix-map={p.itk_src}={official_itk_source_dir(p.official)}"
    )
    log = p.logs / "freesurfer-unit-itk.log"
    env = build_env(p)
    p.itk_build.mkdir(parents=True, exist_ok=True)
    # configured every time, so a changed setting reaches the cache
    # packages/source/build_itk.sh, with Ninja instead of Make (the generator sets no flag)
    run(
        [
            "cmake",
            str(p.itk_src),
            "-G",
            "Ninja",
            f"-DCMAKE_C_COMPILER={p.cc}",
            f"-DCMAKE_CXX_COMPILER={p.cxx}",
            f"-DCMAKE_INSTALL_PREFIX:PATH={p.itk}",
            "-DCMAKE_BUILD_TYPE:STRING=Release",
            f"-DCMAKE_CXX_FLAGS:STRING={flags}",
            f"-DCMAKE_C_FLAGS:STRING={flags}",
            "-DCMAKE_CXX_STANDARD:STRING=11",
            "-DCMAKE_C_STANDARD:STRING=11",
            "-DITK_BUILD_DEFAULT_MODULES:BOOL=OFF",
            "-DITKGroup_Core:BOOL=ON",
            "-DITKGroup_Filtering:BOOL=ON",
            "-DITKGroup_Numerics:BOOL=ON",
            "-DITKGroup_Registration:BOOL=ON",
            "-DITKGroup_Segmentation:BOOL=ON",
            "-DITKGroup_IO:BOOL=ON",
            "-DModule_AnisotropicDiffusionLBR:BOOL=ON",
            "-DBUILD_TESTING:BOOL=OFF",
            "-DCMAKE_EXPORT_COMPILE_COMMANDS:BOOL=ON",
        ],
        log,
        cwd=p.itk_build,
        env=env,
    )
    run(["cmake", "--build", ".", "--", f"-j{jobs}"], log, cwd=p.itk_build, env=env)
    run(["cmake", "--build", ".", "--target", "install"], log, cwd=p.itk_build, env=env)
    print(f"ITK 4.13.2 installed: {p.itk}")


def make_shadow(p: Paths) -> None:
    p.shadow.mkdir(parents=True, exist_ok=True)
    for d in (*FS_DIRS, *PROGRAMS):
        link = p.shadow / d
        if link.is_symlink():
            link.unlink()
        link.symlink_to(p.tag / d)
    top = p.shadow / "CMakeLists.txt"
    top.write_text((HERE / "top.cmake").read_text(encoding="utf-8"), encoding="utf-8")


def official_source_dir(official: Path) -> str:
    """The folder the official build compiled in, from the ``__FILE__`` strings of one of its
    programs (``.../utils/mri.cpp``): FreeSurfer's messages that print a source path then print
    the official one (read from the binary, never written into the repository)."""
    data = (official / "bin" / "mri_add_xform_to_header").read_bytes()
    m = re.search(rb"\x00(/[\x21-\x7e]+?)/utils/mri\.cpp\x00", data)
    if not m:
        sys.exit(f"no __FILE__ path in {official}/bin/mri_add_xform_to_header")
    return m.group(1).decode()


def step_freesurfer(p: Paths, jobs: int) -> None:
    make_shadow(p)
    log = p.logs / "freesurfer-unit-fs.log"
    env = build_env(p)
    p.fs_build.mkdir(parents=True, exist_ok=True)
    itk_dir = next((p.itk / "lib" / "cmake").glob("ITK-*"))
    run(
        [
            "cmake",
            str(p.shadow),
            "-G",
            "Ninja",
            f"-DCMAKE_C_COMPILER={p.cc}",
            f"-DCMAKE_CXX_COMPILER={p.cxx}",
            f"-DITK_DIR={itk_dir}",
            f"-DFS_VERSION={FS_VERSION}",
            f"-DBUILD_STAMP={BUILD_STAMP}",
            f"-DFSUNIT_HARNESS_DIR={HERE / 'harness'}",
            f"-DFSUNIT_PROGRAMS={';'.join(PROGRAMS)}",
            f"-DFSUNIT_OFFICIAL_SOURCE_DIR={official_source_dir(p.official)}",
            "-DCMAKE_EXPORT_COMPILE_COMMANDS:BOOL=ON",
        ],
        log,
        cwd=p.fs_build,
        env=env,
    )
    # only what is used: the library, its packages (as dependencies), the programs, the harness
    run(
        ["cmake", "--build", ".", "--target", "utils", "fsunit", *PROGRAMS, "--", f"-j{jobs}"],
        log,
        cwd=p.fs_build,
        env=env,
    )
    p.bin.mkdir(parents=True, exist_ok=True)
    for exe in ("fsunit", *PROGRAMS):
        found = [f for f in p.fs_build.rglob(exe) if f.is_file() and os.access(f, os.X_OK)]
        if len(found) != 1:
            sys.exit(f"{exe}: found {found}")
        shutil.copy2(found[0], p.bin / exe)
    write_info(p)


def _version(compiler: Path) -> str:
    out = subprocess.run([str(compiler), "--version"], capture_output=True, text=True, check=False)
    return out.stdout.splitlines()[0] if out.stdout else "?"


def write_info(p: Paths) -> None:
    head = subprocess.run(
        ["git", "-C", str(REPO), "rev-parse", "HEAD"], capture_output=True, text=True, check=False
    ).stdout.strip()
    dirty = subprocess.run(
        ["git", "-C", str(REPO), "status", "--porcelain", "--", str(HERE)],
        capture_output=True,
        text=True,
        check=False,
    ).stdout.strip()
    info = {
        "what": "FreeSurfer 8.2.0's unit oracle (larmorx queue item 3.1-02)",
        "freesurfer_tag": str(p.tag),
        "freesurfer_tag_sha256": (p.tag / ".upstream-sha256").read_text(encoding="utf-8").strip()
        if (p.tag / ".upstream-sha256").exists()
        else "",
        "itk_tarball_sha256": ITK_SHA256,
        "compiler": _version(p.cxx),
        "binutils": _version(Path(shutil.which("ld", path=build_env(p)["PATH"]) or "ld")),
        "compiler_package": "Ubuntu gcc-8 8.4.0-3ubuntu2" if p.compiler == "ubuntu" else str(p.gcc),
        "fs_version": FS_VERSION,
        "build_stamp": BUILD_STAMP,
        "scripts_commit": head + ("-dirty" if dirty else ""),
        "programs": {
            exe.name: sha256(exe) for exe in sorted(p.bin.iterdir()) if exe.name != "BUILD_INFO.txt"
        },
        "libraries": {
            str(a.relative_to(p.fs_build)): sha256(a) for a in sorted(p.fs_build.rglob("lib*.a"))
        },
    }
    (p.bin / "BUILD_INFO.txt").write_text(json.dumps(info, indent=1) + "\n", encoding="utf-8")
    print(f"built: {p.bin} ({len(info['programs'])} programs)")


def main(argv: list[str] | None = None) -> int:
    ws = workspace()
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("step", choices=("toolchain", "itk", "freesurfer", "all", "info"))
    ap.add_argument("--out", type=Path, default=ws / "oracles" / "freesurfer-8.2.0-unit")
    ap.add_argument("--gcc", type=Path, default=ws / "oracles" / "toolchains" / "gcc-8.4")
    ap.add_argument("--tag", type=Path, default=ws / "tags" / "freesurfer-8.2.0")
    ap.add_argument(
        "--official",
        type=Path,
        default=ws
        / "oracles"
        / "freesurfer-8.2.0"
        / "root"
        / "usr"
        / "local"
        / "freesurfer"
        / "8.2.0",
        help="the official 8.2.0 installation (its programs' __FILE__ strings)",
    )
    ap.add_argument(
        "--compiler",
        choices=("ubuntu", "conda"),
        default="ubuntu",
        help="Ubuntu's gcc-8 package (the official compiler) or conda-forge's GCC 8.4.0",
    )
    ap.add_argument("--jobs", type=int, default=10)
    a = ap.parse_args(argv)
    p = Paths(
        ws, a.out.resolve(), a.gcc.resolve(), a.tag.resolve(), a.official.resolve(), a.compiler
    )
    if a.step in ("toolchain", "all"):
        step_toolchain(p)
    if a.step in ("itk", "all"):
        step_itk(p, a.jobs)
    if a.step in ("freesurfer", "all"):
        step_freesurfer(p, a.jobs)
    if a.step == "info":
        write_info(p)
    return 0


if __name__ == "__main__":
    sys.exit(main())
