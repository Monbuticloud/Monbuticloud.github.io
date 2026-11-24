#!/usr/bin/env python3
"""Split the multi-piece SVG sprite into individual symbol definitions.

Each symbol gets a computed viewBox from its path coordinates.
"""
import re, sys, math

with open(sys.argv[1]) as f:
    lines = f.readlines()

# Piece regions identified by line ranges (0-indexed) from structural analysis
# Format: (start_line, end_line, name)
REGIONS = [
    (5, 68,    "wk", "white king"),
    (68, 133,  "wq", "white queen"),
    (133, 164, "wr", "white rook"),
    (164, 251, "wb", "white bishop"),
    (251, 285, "bn", "black knight"),
    (285, 319, "wn", "white knight"),
    (319, 332, "wp", "white pawn"),
    (332, 368, "wn2", "white knight (mirror)"),
    (368, 403, "bn2", "black knight (mirror)"),
]

def get_coords(text):
    """Extract all x,y coordinate pairs from SVG element text."""
    coords = []
    # Path d="..." - find all numbers after M, C, L, etc
    for m in re.finditer(r'[MCL]\s*(-?\d+\.?\d*)\s+(-?\d+\.?\d*)', text):
        coords.append((float(m.group(1)), float(m.group(2))))
    # points="x,y x,y ..."
    for m in re.finditer(r'points="([^"]*)"', text):
        nums = re.findall(r'-?\d+\.?\d*', m.group(1))
        for i in range(0, len(nums)-1, 2):
            coords.append((float(nums[i]), float(nums[i+1])))
    # cx, cy
    cx = re.search(r'cx="([^"]*)"', text)
    cy = re.search(r'cy="([^"]*)"', text)
    if cx and cy:
        coords.append((float(cx.group(1)), float(cy.group(1))))
    # x, y (with width/height for rect)
    xm = re.search(r'x="([^"]*)"', text)
    ym = re.search(r'y="([^"]*)"', text)
    wm = re.search(r'width="([^"]*)"', text)
    hm = re.search(r'height="([^"]*)"', text)
    if xm and ym and wm and hm:
        x, y = float(xm.group(1)), float(ym.group(1))
        w, h = float(wm.group(1)), float(hm.group(1))
        coords.extend([(x, y), (x+w, y+h)])
    # x1,y1,x2,y2 (lines)
    x1 = re.search(r'x1="([^"]*)"', text)
    y1 = re.search(r'y1="([^"]*)"', text)
    x2 = re.search(r'x2="([^"]*)"', text)
    y2 = re.search(r'y2="([^"]*)"', text)
    if all([x1, y1, x2, y2]):
        coords.extend([(float(x1.group(1)), float(y1.group(1))),
                      (float(x2.group(1)), float(y2.group(1)))])
    return coords


def compute_bounds(text_lines, margin=2):
    """Compute bounding box from all SVG elements in the given lines."""
    all_coords = []
    for line in text_lines:
        all_coords.extend(get_coords(line))
    if not all_coords:
        return None
    xs = [c[0] for c in all_coords]
    ys = [c[1] for c in all_coords]
    xmin = min(xs) - margin
    ymin = min(ys) - margin
    xmax = max(xs) + margin
    ymax = max(ys) + margin
    return (math.floor(xmin), math.floor(ymin),
            math.ceil(xmax - xmin), math.ceil(ymax - ymin))


print('<?xml version="1.0" encoding="utf-8"?>')
print('<svg xmlns="http://www.w3.org/2000/svg">')
print('  <!-- Chess piece sprite — auto-generated from Che‌ss-Pieces.svg -->')

for start, end, piece_id, desc in REGIONS:
    piece_lines = lines[start:end]
    bounds = compute_bounds(piece_lines)
    if bounds is None:
        print(f'  <!-- {desc}: no coords found, skipping -->')
        continue
    x, y, w, h = bounds
    text = ''.join(piece_lines).strip()
    print(f'')
    print(f'  <!-- {desc} -->')
    print(f'  <symbol id="{piece_id}" viewBox="{x} {y} {w} {h}">')
    # Indent each line
    for pl in piece_lines:
        stripped = pl.rstrip()
        if stripped:
            print(f'    {stripped}')
    print(f'  </symbol>')

print('')
print('</svg>')
