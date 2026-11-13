# SPDX-License-Identifier: Apache-2.0
"""Writes crates/larmorx-io/src/xml/nametab.rs from expat 2.0.1's nametab.h.

Usage: python -P gen_nametab.py <expat-2.0.1>/expat/lib/nametab.h > crates/larmorx-io/src/xml/nametab.rs
then `cargo fmt -p larmorx-io` (which only re-flows the rows).
"""

import re
import sys


def main() -> None:
    with open(sys.argv[1], encoding="ascii") as f:
        s = f.read()

    def arr(name: str) -> list[int]:
        m = re.search(r"static const unsigned(?: char)? " + name + r"\[\] = \{(.*?)\};", s, re.S)
        return [int(x, 16) for x in re.findall(r"0x[0-9A-Fa-f]+", m.group(1))]

    out = [
        "// SPDX-License-Identifier: Apache-2.0 AND MIT",
        "// expat 2.0.1's nametab.h (Copyright (c) 1998, 1999, 2000 Thai Open Source Software Center Ltd",
        "// and Clark Cooper; MIT, PROVENANCE.md): the XML 1.0 name-character classes of characters below",
        "// U+10000, a bitmap of 32-character rows and two page tables, generated from the header by",
        "// validation/probes/fs-gifti/gen_nametab.py (the values are the header's).",
        "",
    ]
    for name, rust, ty, per in (
        ("namingBitmap", "NAMING_BITMAP", "u32", 4),
        ("nmstrtPages", "NMSTRT_PAGES", "u8", 8),
        ("namePages", "NAME_PAGES", "u8", 8),
    ):
        vals = arr(name)
        out.append(f"pub(super) static {rust}: [{ty}; {len(vals)}] = [")
        width = 8 if ty == "u32" else 2
        for i in range(0, len(vals), per):
            out.append("    " + " ".join(f"0x{v:0{width}X}," for v in vals[i : i + per]))
        out.append("];")
        if rust != "NAME_PAGES":
            out.append("")
    print("\n".join(out))


if __name__ == "__main__":
    main()
