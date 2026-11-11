# SPDX-License-Identifier: Apache-2.0
"""The DLLs Windows programs and extensions import: none of the C runtime's (``+crt-static``).

The MSVC builds link Windows' C runtime statically (``.cargo/config.toml``), so the ``.exe``
programs (``larmorx``, ``lx``, ``larmorx-nc``, ``larmorx-gpl``) and the extension
(``larmorx/_core.pyd``) must not import the Visual C++ runtime (``VCRUNTIME140.dll``,
``VCRUNTIME140_1.dll``, ``MSVCP140.dll``, ...) or the Universal CRT (``ucrtbase.dll``,
``api-ms-win-crt-*.dll``), which a clean Windows may lack. This script reads each file's import
table and delay-load import table (PE32 and PE32+, any machine), from files, directories
(searched recursively) and the members of wheels and ``.zip`` archives, with Python's standard
library only: it runs anywhere, on downloaded files too, without Microsoft's tools.

Usage:
    python -P scripts/release/pe_imports.py [--machine x86_64|aarch64] [--quiet] PATH...

Prints each file's machine and imported DLLs. Exit status 0 when at least one PE file was found
and none imports a C runtime DLL (and, with ``--machine``, every file is for that machine);
1 otherwise; 2 when a file named directly, or a member with a PE suffix, is not a valid PE file.
The last line is ``PE-IMPORTS ok`` or ``PE-IMPORTS FAILED``.
"""

from __future__ import annotations

import argparse
import re
import struct
import sys
import zipfile
from dataclasses import dataclass, field
from pathlib import Path

#: The C runtime's DLLs (matched case-insensitively against the whole name).
FORBIDDEN = re.compile(
    r"(?i)^(vcruntime\d+(_\d+)?d?|msvcp\d+(_\w+)?d?|msvcr\d+d?|concrt\d+d?|vccorlib\d+d?"
    r"|vcomp\d+d?|ucrtbased?|api-ms-win-crt-[a-z0-9-]+)\.dll$"
)
#: The suffixes of PE files looked for in directories and archives.
SUFFIXES = (".exe", ".pyd", ".dll")
MACHINES = {
    0x014C: "i386",
    0x8664: "x86_64",
    0xAA64: "aarch64",
    0x01C4: "armnt",
    0xA641: "arm64ec",
}
IMPORT, DELAY_IMPORT = 1, 13


class NotPE(ValueError):
    """The bytes are not a PE file (or its tables are damaged)."""


@dataclass
class PEInfo:
    machine: str
    imports: list[str] = field(default_factory=list)
    delay_imports: list[str] = field(default_factory=list)

    @property
    def forbidden(self) -> list[str]:
        return sorted({d for d in self.imports + self.delay_imports if FORBIDDEN.match(d)})


def _u16(b: bytes, o: int) -> int:
    return struct.unpack_from("<H", b, o)[0]


def _u32(b: bytes, o: int) -> int:
    return struct.unpack_from("<I", b, o)[0]


def parse(data: bytes) -> PEInfo:
    """The machine, the imported DLLs and the delay-loaded DLLs of a PE image."""
    try:
        if data[:2] != b"MZ":
            raise NotPE("no MZ header")
        pe = _u32(data, 0x3C)
        if data[pe : pe + 4] != b"PE\0\0":
            raise NotPE("no PE signature")
        coff = pe + 4
        machine = _u16(data, coff)
        n_sections = _u16(data, coff + 2)
        opt_size = _u16(data, coff + 16)
        opt = coff + 20
        magic = _u16(data, opt)
        if magic == 0x10B:  # PE32
            image_base, n_dirs_at, dirs_at = _u32(data, opt + 28), opt + 92, opt + 96
        elif magic == 0x20B:  # PE32+
            image_base = struct.unpack_from("<Q", data, opt + 24)[0]
            n_dirs_at, dirs_at = opt + 108, opt + 112
        else:
            raise NotPE(f"unknown optional header magic {magic:#x}")
        n_dirs = _u32(data, n_dirs_at)
        sections = []
        for k in range(n_sections):
            s = opt + opt_size + 40 * k
            vsize, va, raw_size, raw_ptr = struct.unpack_from("<IIII", data, s + 8)
            sections.append((va, max(vsize, raw_size), raw_ptr, raw_size))

        def offset(rva: int) -> int:
            for va, size, raw_ptr, raw_size in sections:
                if va <= rva < va + size and rva - va < raw_size:
                    return raw_ptr + rva - va
            raise NotPE(f"RVA {rva:#x} is in no section")

        def name_at(rva: int) -> str:
            o = offset(rva)
            end = data.index(b"\0", o)
            return data[o:end].decode("ascii")

        def directory(index: int) -> tuple[int, int]:
            if index >= n_dirs:
                return 0, 0
            return struct.unpack_from("<II", data, dirs_at + 8 * index)

        info = PEInfo(MACHINES.get(machine, f"{machine:#06x}"))
        rva, size = directory(IMPORT)
        if rva and size:
            o = offset(rva)
            while True:
                lookup, _, _, name_rva, thunk = struct.unpack_from("<IIIII", data, o)
                if not (lookup or name_rva or thunk):
                    break
                info.imports.append(name_at(name_rva))
                o += 20
        rva, size = directory(DELAY_IMPORT)
        if rva and size:
            o = offset(rva)
            while True:
                attributes, name_rva, *rest = struct.unpack_from("<8I", data, o)
                if not (attributes or name_rva or any(rest)):
                    break
                # Version 1 descriptors hold RVAs (bit 0 set); the old form held addresses.
                info.delay_imports.append(
                    name_at(name_rva if attributes & 1 else name_rva - image_base)
                )
                o += 32
        return info
    except (struct.error, IndexError, ValueError, UnicodeDecodeError) as e:
        if isinstance(e, NotPE):
            raise
        raise NotPE(f"damaged PE tables ({e})") from e


def files(path: Path) -> list[tuple[str, bytes]]:
    """(label, bytes) of every PE file ``path`` names, holds (a directory) or contains (an archive)."""
    if path.is_dir():
        found = []
        for p in sorted(path.rglob("*")):
            if p.is_file() and (p.suffix.lower() in SUFFIXES or p.suffix in (".whl", ".zip")):
                found += files(p)
        return found
    if path.suffix.lower() in (".whl", ".zip"):
        with zipfile.ZipFile(path) as z:
            return [
                (f"{path}!{n}", z.read(n))
                for n in sorted(z.namelist())
                if n.lower().endswith(SUFFIXES)
            ]
    return [(str(path), path.read_bytes())]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("paths", nargs="+", type=Path, help="files, directories, wheels, zips")
    parser.add_argument("--machine", choices=sorted(set(MACHINES.values())), help="required")
    parser.add_argument("--quiet", action="store_true", help="print problems and the summary")
    args = parser.parse_args(argv)
    found, problems, invalid = 0, [], False
    for path in args.paths:
        if not path.exists():
            problems.append(f"{path}: no such file or directory")
            continue
        for label, data in files(path):
            try:
                info = parse(data)
            except NotPE as e:
                problems.append(f"{label}: not a valid PE file: {e}")
                invalid = True
                continue
            found += 1
            if not args.quiet:
                delay = f"; delay-loaded: {', '.join(info.delay_imports)}" * bool(
                    info.delay_imports
                )
                print(f"{label}: {info.machine}: {', '.join(info.imports) or '(none)'}{delay}")
            if info.forbidden:
                problems.append(f"{label}: imports the C runtime: {', '.join(info.forbidden)}")
            if args.machine and info.machine != args.machine:
                problems.append(f"{label}: built for {info.machine}, not {args.machine}")
    if not found:
        problems.append("no PE file found in " + ", ".join(map(str, args.paths)))
    for p in problems:
        print(p, file=sys.stderr)
    print(
        f"PE-IMPORTS {'FAILED' if problems else 'ok'}: {found} PE files, {len(problems)} problems"
    )
    return 2 if invalid else (1 if problems else 0)


if __name__ == "__main__":
    sys.exit(main())
