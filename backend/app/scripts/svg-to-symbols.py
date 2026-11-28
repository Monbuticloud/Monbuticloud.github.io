#!/usr/bin/env python3
"""Convert flat multi-piece SVG into an SVG sprite with <symbol> elements.

Groups SVG elements by X-coordinate proximity, calculates bounding boxes,
and wraps each cluster in a <symbol id="piece-N" viewBox="..."> tag.
"""

import re
import math
import sys

PIECE_NAMES = [
    "king", "queen", "rook", "bishop", "knight", "pawn",
]

def parse_path_d(d):
    """Crudely extract X,Y coordinates from an SVG path `d` string."""
    coords = re.findall(r'[-+]?\d*\.?\d+', d)
    coords = [float(c) for c in coords]
    points = []
    for i in range(0, len(coords)-1, 2):
        x, y = coords[i], coords[i+1]
        if -2000 < x < 2000 and -500 < y < 500:  # sanity filter
            points.append((x, y))
    return points

def get_coords_from_element(text):
    """Extract all X,Y coordinates from any SVG element text."""
    # Handle path d attribute
    d_match = re.search(r'd="([^"]*)"', text)
    if d_match:
        return parse_path_d(d_match.group(1))
    # Handle cx, cy (circles, ellipses)
    cx = re.search(r'cx="([^"]*)"', text)
    cy = re.search(r'cy="([^"]*)"', text)
    if cx and cy:
        return [(float(cx.group(1)), float(cy.group(1)))]
    # Handle x, y (rects, etc)
    x_match = re.search(r'x="([^"]*)"', text)
    y_match = re.search(r'y="([^"]*)"', text)
    if x_match and y_match:
        x = float(x_match.group(1))
        y = float(y_match.group(1))
        w = re.search(r'width="([^"]*)"', text)
        h = re.search(r'height="([^"]*)"', text)
        if w and h:
            return [(x, y), (x+float(w.group(1)), y+float(h.group(1)))]
        return [(x, y)]
    # Handle points (polygons, polylines)
    pts_match = re.search(r'points="([^"]*)"', text)
    if pts_match:
        pts = re.findall(r'[-+]?\d*\.?\d+', pts_match.group(1))
        pts = [float(p) for p in pts]
        return [(pts[i], pts[i+1]) for i in range(0, len(pts)-1, 2)]
    # Handle x1,y1,x2,y2 (lines)
    x1 = re.search(r'x1="([^"]*)"', text)
    y1 = re.search(r'y1="([^"]*)"', text)
    x2 = re.search(r'x2="([^"]*)"', text)
    y2 = re.search(r'y2="([^"]*)"', text)
    if all([x1, y1, x2, y2]):
        return [(float(x1.group(1)), float(y1.group(1))),
                (float(x2.group(1)), float(y2.group(1)))]
    return []

def compute_bounds(points, margin=2):
    if not points:
        return None
    xs = [p[0] for p in points]
    ys = [p[1] for p in points]
    xmin = min(xs) - margin
    xmax = max(xs) + margin
    ymin = min(ys) - margin
    ymax = max(ys) + margin
    return xmin, ymin, xmax - xmin, ymax - ymin

def element_center_x(text):
    """Get approximate center X of an element."""
    coords = get_coords_from_element(text)
    if not coords:
        return None
    xs = [c[0] for c in coords]
    return (min(xs) + max(xs)) / 2

with open(sys.argv[1], 'r') as f:
    content = f.read()

# Extract everything inside the <g> tag
g_match = re.search(r'<g>(.*?)</g>', content, re.DOTALL)
if not g_match:
    print("ERROR: No <g> found in SVG")
    sys.exit(1)

inner = g_match.group(1)

# Parse individual elements
# Match SVG elements: path, rect, circle, ellipse, line, polygon, polyline, g (nested)
element_re = re.compile(
    r'<(?:path|rect|circle|ellipse|line|polygon|polyline|g)\b[^>]*>'
    r'(?:.*?</(?:path|rect|circle|ellipse|line|polygon|polyline|g)>)?',
    re.DOTALL
)

elements = []
for match in element_re.finditer(inner):
    text = match.group(0).strip()
    if not text:
        continue
    center_x = element_center_x(text)
    if center_x is not None:
        elements.append((center_x, text))

# Group by X proximity (pieces are spread ~200-300 units apart)
elements.sort(key=lambda e: e[0])

groups = []
current_group = [elements[0]]
for i in range(1, len(elements)):
    gap = elements[i][0] - current_group[-1][0]
    if gap > 80:  # threshold for new piece
        groups.append(current_group)
        current_group = [elements[i]]
    else:
        current_group.append(elements[i])
if current_group:
    groups.append(current_group)

print(f"Found {len(groups)} piece groups", file=sys.stderr)

# Generate SVG sprite
print('<?xml version="1.0" encoding="utf-8"?>')
print('<svg xmlns="http://www.w3.org/2000/svg">')
print('  <!-- SVG sprite — Chess Pieces -->')

for idx, group in enumerate(groups):
    # Collect all coordinates for this group
    all_points = []
    for _, text in group:
        all_points.extend(get_coords_from_element(text))
    
    bounds = compute_bounds(all_points, margin=2)
    if not bounds:
        continue
    
    name = PIECE_NAMES[idx] if idx < len(PIECE_NAMES) else f"piece-{idx+1}"
    x, y, w, h = bounds
    # Round for cleanliness
    x, y = math.floor(x), math.floor(y)
    w, h = math.ceil(w), math.ceil(h)
    
    print(f'  <symbol id="{name}" viewBox="{x} {y} {w} {h}">')
    print(f'    <!-- Bounds: x={x} y={y} w={w} h={h} -->')
    for _, text in group:
        print(f'    {text}')
    print(f'  </symbol>')

print('</svg>')
