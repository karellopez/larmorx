# SPDX-License-Identifier: Apache-2.0
"""The header fields in which two NIfTI-1 images differ, with both values (black-box: FSL's
output against the clean-room original's). ``python -P header_diff.py <a> <b>``."""

from __future__ import annotations

import gzip
import struct
import sys
from pathlib import Path

#: NIfTI-1's fields: (name, offset, struct format).
FIELDS = [
    ("sizeof_hdr", 0, "i"), ("data_type", 4, "10s"), ("db_name", 14, "18s"), ("extents", 32, "i"),
    ("session_error", 36, "h"), ("regular", 38, "c"), ("dim_info", 39, "B"), ("dim", 40, "8h"),
    ("intent_p1", 56, "f"), ("intent_p2", 60, "f"), ("intent_p3", 64, "f"), ("intent_code", 68, "h"),
    ("datatype", 70, "h"), ("bitpix", 72, "h"), ("slice_start", 74, "h"), ("pixdim", 76, "8f"),
    ("vox_offset", 108, "f"), ("scl_slope", 112, "f"), ("scl_inter", 116, "f"), ("slice_end", 120, "h"),
    ("slice_code", 122, "B"), ("xyzt_units", 123, "B"), ("cal_max", 124, "f"), ("cal_min", 128, "f"),
    ("slice_duration", 132, "f"), ("toffset", 136, "f"), ("glmax", 140, "i"), ("glmin", 144, "i"),
    ("descrip", 148, "80s"), ("aux_file", 228, "24s"), ("qform_code", 252, "h"), ("sform_code", 254, "h"),
    ("quatern_b", 256, "f"), ("quatern_c", 260, "f"), ("quatern_d", 264, "f"), ("qoffset_x", 268, "f"),
    ("qoffset_y", 272, "f"), ("qoffset_z", 276, "f"), ("srow_x", 280, "4f"), ("srow_y", 296, "4f"),
    ("srow_z", 312, "4f"), ("intent_name", 328, "16s"), ("magic", 344, "4s"),
]  # fmt: skip


def head(p: Path) -> bytes:
    raw = p.read_bytes()
    return gzip.decompress(raw)[:348] if raw[:2] == b"\x1f\x8b" else raw[:348]


def main(argv: list[str]) -> int:
    a, b = head(Path(argv[0])), head(Path(argv[1]))
    for name, off, fmt in FIELDS:
        size = struct.calcsize("<" + fmt)
        if a[off : off + size] != b[off : off + size]:
            va = struct.unpack("<" + fmt, a[off : off + size])
            vb = struct.unpack("<" + fmt, b[off : off + size])
            print(f"{name}: {va} | {vb}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
