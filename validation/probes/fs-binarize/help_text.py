# SPDX-License-Identifier: Apache-2.0
"""mri_binarize's usage and help texts from FreeSurfer 8.2.0's source, as Rust constants.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it reads FreeSurfer's source).

Reads ``mri_binarize/mri_binarize.cpp``'s ``print_usage`` and ``print_help`` and prints
``crates-freesurfer/larmorx-freesurfer-volumes/src/mri_binarize/help.rs``'s two constants: every
``printf`` literal decoded as C decodes it (escapes, ``%%``), in order; ``print_usage``'s first
line (``USAGE: %s \\n`` with the program's name) and its version line are the program's.

    python -P validation/probes/fs-binarize/help_text.py <freesurfer-8.2.0> > help.rs
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

LIT = re.compile(r'printf\("((?:[^"\\]|\\.)*)"\)\s*;')


def body(src: str, head: str) -> str:
    a = src.index(head)
    b = src.index("\n}", a)
    return src[a:b]


def decode(s: str) -> str:
    out = s.encode("latin-1").decode("unicode_escape")
    if "%" in out.replace("%%", ""):
        raise SystemExit(f"a conversion in {s!r}")
    return out.replace("%%", "%")


def rust(s: str) -> str:
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n") + '"'


def main() -> int:
    src = (Path(sys.argv[1]) / "mri_binarize" / "mri_binarize.cpp").read_text(encoding="latin-1")
    usage = body(src, "static void print_usage(void) {")
    usage = usage[usage.index('printf("USAGE: %s \\n",Progname) ;') + 32 :]
    usage = usage[: usage.index("std::cout << getVersion()")]
    help_ = body(src, "static void print_help(void) {")
    u = "".join(decode(m.group(1)) for m in LIT.finditer(usage))
    h = "".join(decode(m.group(1)) for m in LIT.finditer(help_))
    print("// SPDX-License-Identifier: LicenseRef-FreeSurfer")
    print("// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright")
    print("// 2026 Karel Lopez Vilaret.")
    print("//! `mri_binarize`'s usage and help texts (`print_usage`, `print_help`), FreeSurfer 8.2.0's")
    print("//! `printf` literals in order (made by `validation/probes/fs-binarize/help_text.py`).")
    print()
    print("/// `print_usage`'s lines after `USAGE: <program> ` and before the version.")
    print(f"pub const USAGE: &str = {rust(u)};")
    print()
    print("/// `print_help`'s lines after `print_usage`.")
    print(f"pub const HELP: &str = {rust(h)};")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
