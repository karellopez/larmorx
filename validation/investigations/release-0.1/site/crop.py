# SPDX-License-Identifier: Apache-2.0
"""Crop a full-page screenshot to the rows y0..y1 (relay 5b's close views of long pages).

Usage: python -P crop.py <screenshot.png> <out.png> <y0> <y1>   (needs Pillow)
"""

import sys

from PIL import Image

src, out, y0, y1 = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4])
im = Image.open(src)
im.crop((0, y0, im.width, min(y1, im.height))).save(out)
