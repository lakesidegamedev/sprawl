"""Urban Sprawl mark: isometric cel-shaded blocks along a road on a dark tile. Writes mark.svg."""
import math, sys
S = 512
C, SN = math.cos(math.radians(30)), 0.5
OX, OY = 256, 318
INK = "#0B1622"
SQUARE = "--square" in sys.argv

def p(gx, gy, z=0.0):
    return (OX + (gx - gy) * C, OY + (gx + gy) * SN - z)

def poly(pts, fill, sw=7):
    d = " ".join(f"{x:.1f},{y:.1f}" for x, y in pts)
    return f'<polygon points="{d}" fill="{fill}" stroke="{INK}" stroke-width="{sw}" stroke-linejoin="round"/>'

def box(gx, gy, w, d, h, top, left, right, roof=None):
    a, b, c, e = (gx, gy), (gx + w, gy), (gx + w, gy + d), (gx, gy + d)
    out = []
    out.append(poly([p(*e), p(*c), p(*c, h), p(*e, h)], left))        # front-left face (y max)
    out.append(poly([p(*b), p(*c), p(*c, h), p(*b, h)], right))       # front-right face (x max)
    out.append(poly([p(*a, h), p(*b, h), p(*c, h), p(*e, h)], top))
    # window bands on the faces (cel style: flat darker stripes)
    for k in range(1, int(h // 34) + 1):
        z = k * 34 - 12
        if z + 10 > h: break
        x0, x1 = 0.18, 0.82
        out.append(poly([p(gx + w * x0, gy + d, z), p(gx + w * x1, gy + d, z), p(gx + w * x1, gy + d, z + 12), p(gx + w * x0, gy + d, z + 12)], "#13314A", 3))
        out.append(poly([p(gx + w, gy + d * x0, z), p(gx + w, gy + d * x1, z), p(gx + w, gy + d * x1, z + 12), p(gx + w, gy + d * x0, z + 12)], "#E2B04A", 3))
    return out


def house(gx, gy, w, d, h, r, wall_l, wall_r, roof_l, roof_r):
    a, b, c, e = (gx, gy), (gx + w, gy), (gx + w, gy + d), (gx, gy + d)
    m0, m1 = (gx, gy + d / 2), (gx + w, gy + d / 2)
    out = [poly([p(*e), p(*c), p(*c, h), p(*e, h)], wall_l),
           poly([p(*b), p(*c), p(*c, h), p(*m1, h + r), p(*b, h)], wall_r),
           poly([p(*a, h), p(*b, h), p(*m1, h + r), p(*m0, h + r)], roof_r),
           poly([p(*e, h), p(*c, h), p(*m1, h + r), p(*m0, h + r)], roof_l)]
    out.append(poly([p(gx + w * 0.35, gy + d, 0), p(gx + w * 0.6, gy + d, 0), p(gx + w * 0.6, gy + d, h * 0.7), p(gx + w * 0.35, gy + d, h * 0.7)], "#13314A", 3))
    return out

parts = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {S} {S}" width="{S}" height="{S}">',
         f'<rect width="{S}" height="{S}" rx="{0 if SQUARE else 104}" fill="{INK if not SQUARE else "#13212F"}"/>']
if not SQUARE:
    parts.append('<rect x="14" y="14" width="484" height="484" rx="92" fill="#13212F"/>')
# ground diamond
G = 158
parts.append(poly([p(-G, -G), p(G, -G), p(G, G), p(-G, G)], "#3F8F6A", 8))
# main road along x through the middle, and a branch road along y
R = 22
parts.append(poly([p(-G, -R), p(G, -R), p(G, R), p(-G, R)], "#E8EEF5", 6))
parts.append(poly([p(-R, R), p(R, R), p(R, G), p(-R, G)], "#E8EEF5", 6))
# lane dashes
for t in range(-G + 14, G - 10, 36):
    parts.append(poly([p(t, -2.5), p(t + 16, -2.5), p(t + 16, 2.5), p(t, 2.5)], "#E2B04A", 0))
# buildings (back to front so painter order is right)
blocks = [
    # (kind, gx, gy, w, d, h)
    ("box", -92, -100, 62, 66, 168),   # tower at the crossroads
    ("box", -150, -96, 44, 60, 92),
    ("box", 34, -104, 64, 70, 112),
    ("box", 108, -96, 40, 58, 58),
    ("box", -96, 34, 62, 52, 70),
    ("house", -150, 36, 42, 42, 26, 22),
    ("house", -138, 104, 40, 40, 24, 20),
    ("house", -78, 104, 40, 40, 24, 20),
    ("box", 36, 34, 58, 58, 52),
    ("house", 108, 38, 40, 40, 24, 20),
    ("house", 46, 106, 40, 40, 24, 20),
    ("house", 108, 106, 40, 40, 24, 20),
]
blocks.sort(key=lambda b: (b[1] + b[3] / 2) + (b[2] + b[4] / 2))
for b in blocks:
    if b[0] == "box":
        parts += box(*b[1:], "#F2F5F8", "#8FA9C0", "#4F6C88")
    else:
        parts += house(*b[1:], "#E9DCC6", "#B79F82", "#C8574F", "#E5605E")
parts.append("</svg>")
open([a for a in sys.argv[1:] if not a.startswith("--")][0], "w").write("\n".join(parts))
