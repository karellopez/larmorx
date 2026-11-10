# SPDX-License-Identifier: Apache-2.0
"""The standalone programs of one platform, as archives for the GitHub Release.

Three archives per platform (`.tar.gz`; `.zip` for Windows), made from the program wheels the
release built and tested (the same files: `larmorx_cli` (built for this only, not published),
`larmorx_nc`, `larmorx_gpl`), each with a top folder of its own name and a README.txt:

- `larmorx-<version>-<target>`: `bin/larmorx` and `bin/lx` (Unix: a symbolic link to
  `larmorx`, so the archive holds one copy of the program; Windows: `bin\\larmorx.exe` and
  `bin\\lx.cmd`, a one-line batch file that runs it), and larmorx's licences as its wheel
  carries them (the repository's `LICENSE`, `NOTICE` and `LICENSES/`, the third-party crates'
  texts included);
- `larmorx-nc-<version>-<target>`: `bin/larmorx-nc`, its FSL data in
  `share/larmorx-nc/fsl-data/` (beside `bin/`, where the program looks, as a wheel installs it),
  and every licence file its wheel carries: `LICENSE` (the FSL Licence), `NOTICE` and
  `LICENSES/` at the top, and under `source/` FSL's original source and the amended source (the
  Rust translation), which the FSL Licence requires in every copy;
- `larmorx-gpl-<version>-<target>`: `bin/larmorx-gpl` and its wheel's licence files (`LICENSE`,
  `NOTICE`, `LICENSES/`; under `source/` the program's own source); the release's
  `larmorx_gpl-<version>.tar.gz` is its Corresponding Source.

The companions' wheels also give their SBOM (`sbom/`). The entries are sorted, owned by 0 and
dated by `--epoch` (default: `SOURCE_DATE_EPOCH`, else 0), so the same files give the same bytes.

Usage:
    python -P scripts/release/archives.py --version 0.1.0rc1 --target x86_64-unknown-linux-gnu \\
        --wheels <dir with the three program wheels> --out <dir>
"""

from __future__ import annotations

import argparse
import gzip
import io
import os
import re
import sys
import tarfile
import time
import zipfile
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
#: The six release targets (Rust's target triples).
TARGETS = (
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
)
#: Each archive: the wheel's distribution name, its program(s).
PROGRAMS = {
    "larmorx": ("larmorx_cli", "larmorx"),
    "larmorx-nc": ("larmorx_nc", "larmorx-nc"),
    "larmorx-gpl": ("larmorx_gpl", "larmorx-gpl"),
}
#: The licence files that stay at an archive's top; the rest of a wheel's go under source/.
TOP = re.compile(r"^(LICENSE|NOTICE|LICENSES/.+)$")
LX_CMD = '@"%~dp0larmorx.exe" %*\r\n'

README = {
    "larmorx": """\
larmorx {version}: the standalone command line ({target})

bin/larmorx runs larmorx's tools with their original programs' arguments, without Python:
    larmorx ants antsRegistration ...      (lx is the same command, shorter)
    larmorx --help                         (the families and tools)
Put bin/ on your PATH, or call the program by its path. The tools of the Python package that are
written in Python (larmorx sdcflows, larmorprepx) need `pip install larmorx`.

The FSL replicas (larmorx-nc, non-commercial use only) and tedana's t2smap replica (larmorx-gpl,
GNU GPL version 3) are separate programs with their own archives; larmorx runs them when they are
on the PATH (or named by LARMORX_NC_BIN, LARMORX_GPL_BIN).

Licence: Apache-2.0 (LICENSE); the works it includes and their licences: NOTICE and LICENSES/
(the Rust crates it is built with: LICENSES/third-party-rust.txt).
Documentation: https://karellopez.github.io/larmorx_documentation/
Source: https://github.com/karellopez/larmorx/tree/v{version}
""",
    "larmorx-nc": """\
larmorx-nc {version}: the FSL replicas as a standalone program ({target})

NON-COMMERCIAL USE ONLY: this program is a translation of FSL's code, under the FSL Licence
(LICENSE).

bin/larmorx-nc holds bit-exact replicas of FSL tools (mcflirt, bet, flirt, fast, topup, fslmaths,
fslstats, prelude). It reads FSL's standard images from share/larmorx-nc/fsl-data/ beside bin/
(keep the two folders together; LARMORX_FSL_DATA or FSLDIR choose another copy). larmorx runs it
when it is on the PATH or named by LARMORX_NC_BIN; `pip install larmorx-nc` installs the same
program.

Licences: LICENSE (the FSL Licence), NOTICE and LICENSES/ (the other works it includes; the Rust
crates it is built with: LICENSES/third-party-rust.txt). source/ holds FSL's original source of
the replicated parts and the amended source (the Rust translation), as the FSL Licence requires;
the release's larmorx_nc-{version}.tar.gz is the complete source distribution.
Documentation: https://karellopez.github.io/larmorx_documentation/
""",
    "larmorx-gpl": """\
larmorx-gpl {version}: tedana's t2smap replica as a standalone program ({target})

bin/larmorx-gpl holds a bit-exact replica of tedana's t2smap (multi-echo fMRI). larmorx runs it
when it is on the PATH or named by LARMORX_GPL_BIN; `pip install larmorx-gpl` installs the same
program.

Licence: the GNU General Public License version 3 only (LICENSE); tedana's code, LGPL-2.1-only,
is distributed under the GPL version 3 (NOTICE). LICENSES/ holds the other works' texts (the Rust
crates it is built with: LICENSES/third-party-rust.txt); source/ holds this program's own source.
Its complete Corresponding Source is the release's larmorx_gpl-{version}.tar.gz.
Documentation: https://karellopez.github.io/larmorx_documentation/
""",
}


@dataclass
class Entry:
    data: bytes = b""
    executable: bool = False
    link: str | None = None  # a symbolic link's target (Unix archives only)


@dataclass
class Archive:
    name: str
    windows: bool
    entries: dict[str, Entry] = field(default_factory=dict)

    def add(self, path: str, entry: Entry) -> None:
        if path in self.entries:
            raise ValueError(f"{self.name}: {path} twice")
        self.entries[path] = entry

    @property
    def filename(self) -> str:
        return self.name + (".zip" if self.windows else ".tar.gz")


def find_wheel(wheels: Path, dist: str, version: str) -> Path:
    found = sorted(wheels.glob(f"{dist}-{version}-*.whl"))
    if len(found) != 1:
        names = ", ".join(p.name for p in found) or "none"
        raise ValueError(f"expected one {dist}-{version}-*.whl in {wheels}, found {names}")
    return found[0]


def wheel_parts(wheel: Path, dist: str, version: str) -> dict[str, dict[str, Entry]]:
    """A program wheel's scripts, data, licence files and SBOMs, by part, paths within it."""
    parts: dict[str, dict[str, Entry]] = {"scripts": {}, "data": {}, "licenses": {}, "sboms": {}}
    prefixes = {
        "scripts": f"{dist}-{version}.data/scripts/",
        "data": f"{dist}-{version}.data/data/",
        "licenses": f"{dist}-{version}.dist-info/licenses/",
        "sboms": f"{dist}-{version}.dist-info/sboms/",
    }
    with zipfile.ZipFile(wheel) as z:
        for info in z.infolist():
            if info.is_dir():
                continue
            for part, prefix in prefixes.items():
                if info.filename.startswith(prefix):
                    mode = info.external_attr >> 16
                    entry = Entry(z.read(info), executable=part == "scripts" or bool(mode & 0o111))
                    parts[part][info.filename[len(prefix) :]] = entry
    # A wheel built from a companion's sdist holds its licence files under the companion's
    # directory (crates-nc/..., the staged pyproject's paths): the archive drops that prefix.
    lic = parts["licenses"]
    tops = {p.split("/", 1)[0] for p in lic}
    if len(tops) == 1 and all("/" in p for p in lic):
        (top,) = tops
        parts["licenses"] = {p[len(top) + 1 :]: e for p, e in lic.items()}
    return parts


def program(parts: dict[str, dict[str, Entry]], name: str, windows: bool) -> Entry:
    exe = name + (".exe" if windows else "")
    if exe not in parts["scripts"]:
        raise ValueError(f"the wheel has no program {exe} (it has {sorted(parts['scripts'])})")
    return Entry(parts["scripts"][exe].data, executable=True)


def build(version: str, target: str, wheels: Path) -> list[Archive]:
    """The three archives of one platform."""
    if target not in TARGETS:
        raise ValueError(f"{target!r} is not a release target ({', '.join(TARGETS)})")
    windows = "windows" in target
    exe = ".exe" if windows else ""
    out = []
    for package, (dist, name) in PROGRAMS.items():
        parts = wheel_parts(find_wheel(wheels, dist, version), dist, version)
        a = Archive(f"{package}-{version}-{target}", windows)
        a.add(f"bin/{name}{exe}", program(parts, name, windows))
        if package == "larmorx":
            if windows:
                a.add("bin/lx.cmd", Entry(LX_CMD.encode("ascii")))
            else:
                a.add("bin/lx", Entry(link="larmorx"))
            for path in ("LICENSE", "NOTICE", *_listed(ROOT / "LICENSES")):
                a.add(path, Entry((ROOT / path).read_bytes()))
        else:
            for path, entry in sorted(parts["data"].items()):
                a.add(path, entry)
            if not parts["licenses"]:
                raise ValueError(f"the {dist} wheel carries no licence files")
            for path, entry in sorted(parts["licenses"].items()):
                a.add(path if TOP.match(path) else f"source/{path}", entry)
            for path, entry in sorted(parts["sboms"].items()):
                a.add(f"sbom/{path}", entry)
        for required in ("LICENSE", "NOTICE", "LICENSES/third-party-rust.txt"):
            if required not in a.entries:
                raise ValueError(f"{a.name}: no {required}")
        text = README[package].format(version=version, target=target)
        a.add("README.txt", Entry(text.replace("\n", "\r\n" if windows else "\n").encode()))
        out.append(a)
    return out


def _listed(directory: Path) -> list[str]:
    return sorted(p.relative_to(ROOT).as_posix() for p in directory.rglob("*") if p.is_file())


def write(archive: Archive, out: Path, epoch: int) -> Path:
    path = out / archive.filename
    top = archive.name
    if archive.windows:
        stamp = time.gmtime(max(epoch, 315532800))[:6]  # zip's dates start in 1980
        with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
            for name in sorted(archive.entries):
                e = archive.entries[name]
                if e.link is not None:
                    raise ValueError(f"{archive.name}: no symbolic links in a zip ({name})")
                info = zipfile.ZipInfo(f"{top}/{name}", date_time=stamp)
                info.external_attr = (0o100755 if e.executable else 0o100644) << 16
                info.compress_type = zipfile.ZIP_DEFLATED
                z.writestr(info, e.data, compresslevel=9)
        return path
    raw = io.BytesIO()
    with tarfile.open(fileobj=raw, mode="w", format=tarfile.PAX_FORMAT) as t:
        for d in sorted(_parents({f"{top}/{n}".rsplit("/", 1)[0] for n in archive.entries})):
            info = _tarinfo(d, epoch)
            info.type, info.mode = tarfile.DIRTYPE, 0o755
            t.addfile(info)
        for name in sorted(archive.entries):
            e = archive.entries[name]
            info = _tarinfo(f"{top}/{name}", epoch)
            if e.link is not None:
                info.type, info.linkname, info.mode = tarfile.SYMTYPE, e.link, 0o777
                t.addfile(info)
            else:
                info.size, info.mode = len(e.data), 0o755 if e.executable else 0o644
                t.addfile(info, io.BytesIO(e.data))
    with path.open("wb") as f, gzip.GzipFile("", "wb", 9, f, mtime=0) as g:
        g.write(raw.getvalue())
    return path


def _parents(dirs: set[str]) -> set[str]:
    out = set()
    for d in dirs:
        parts = d.split("/")
        out.update("/".join(parts[: i + 1]) for i in range(len(parts)))
    return out


def _tarinfo(name: str, epoch: int) -> tarfile.TarInfo:
    info = tarfile.TarInfo(name)
    info.mtime, info.uid, info.gid, info.uname, info.gname = epoch, 0, 0, "", ""
    return info


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--version", required=True, help="the release's version (PEP 440)")
    parser.add_argument("--target", required=True, choices=TARGETS)
    parser.add_argument("--wheels", type=Path, required=True, help="the three program wheels")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--epoch", type=int, default=int(os.environ.get("SOURCE_DATE_EPOCH", 0)))
    args = parser.parse_args(argv)
    try:
        archives = build(args.version, args.target, args.wheels)
    except (ValueError, OSError) as e:
        print(f"archives: {e}", file=sys.stderr)
        return 1
    args.out.mkdir(parents=True, exist_ok=True)
    for a in archives:
        path = write(a, args.out, args.epoch)
        print(f"archives: {path.name}: {len(a.entries)} files, {path.stat().st_size:,} bytes")
    return 0


if __name__ == "__main__":
    sys.exit(main())
