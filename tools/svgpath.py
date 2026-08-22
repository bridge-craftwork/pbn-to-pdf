"""Minimal SVG path-data parser: split into subpaths and measure bounding boxes.

Written for tools/make_card_variants.py.  Handles the command set the
vector-playing-cards assets actually use (M L H V C S Q T A Z, absolute and
relative).  Bounding boxes use control points, which over-estimates a curve's
extent -- safe for deciding what to keep.
"""
import re

NUM = re.compile(r'[-+]?(?:\d*\.\d+|\d+\.?)(?:[eE][-+]?\d+)?')
CMD = re.compile(r'([MmLlHhVvCcSsQqTtAaZz])')

ARITY = {'M':2,'L':2,'T':2,'H':1,'V':1,'C':6,'S':4,'Q':4,'A':7,'Z':0}


def tokenize(d):
    """-> list of (command_letter, [floats]) with each command's arity applied."""
    out = []
    parts = CMD.split(d)
    i = 1
    while i < len(parts):
        cmd = parts[i]
        args = [float(x) for x in NUM.findall(parts[i + 1])] if i + 1 < len(parts) else []
        n = ARITY[cmd.upper()]
        if n == 0:
            out.append((cmd, []))
        elif not args:
            out.append((cmd, []))
        else:
            # Repeated argument groups: "M x y x y" means moveto then lineto.
            first = True
            for j in range(0, len(args) - n + 1, n):
                c = cmd
                if not first and cmd in 'Mm':
                    c = 'L' if cmd == 'M' else 'l'
                out.append((c, args[j:j + n]))
                first = False
        i += 2
    return out


def subpaths(d):
    """Split into subpaths.

    Returns a list of dicts: {'tokens', 'bbox': (x0,y0,x1,y1), 'start': (x,y)}.
    Coordinates are resolved to absolute so bboxes are comparable across
    subpaths, but the original token text is preserved for re-emission.
    """
    toks = tokenize(d)
    subs = []
    cur = None
    x = y = 0.0
    sx = sy = 0.0
    for cmd, a in toks:
        rel = cmd.islower()
        C = cmd.upper()
        pts = []
        if C == 'M':
            if cur is not None:
                subs.append(cur)
            x, y = (x + a[0], y + a[1]) if rel else (a[0], a[1])
            sx, sy = x, y
            cur = {'tokens': [], 'bbox': [x, y, x, y], 'start': (x, y)}
            pts = [(x, y)]
            # Absolutise the leading moveto.  A subpath's `m` is relative to
            # wherever the previous subpath ended, so a subpath list that has
            # been filtered or reordered would otherwise land in the wrong
            # place -- every later subpath shifting by the accumulated error.
            cur['tokens'].append(('M', [x, y]))
            continue
        elif C == 'Z':
            x, y = sx, sy
            pts = [(x, y)]
        elif C == 'H':
            x = x + a[0] if rel else a[0]
            pts = [(x, y)]
        elif C == 'V':
            y = y + a[0] if rel else a[0]
            pts = [(x, y)]
        elif C in ('L', 'T'):
            x, y = (x + a[0], y + a[1]) if rel else (a[0], a[1])
            pts = [(x, y)]
        elif C in ('C', 'S', 'Q'):
            base = (x, y)
            coords = []
            for j in range(0, len(a), 2):
                px, py = (base[0] + a[j], base[1] + a[j + 1]) if rel else (a[j], a[j + 1])
                coords.append((px, py))
            pts = coords
            x, y = coords[-1]
        elif C == 'A':
            x, y = (x + a[5], y + a[6]) if rel else (a[5], a[6])
            pts = [(x, y)]
        if cur is None:
            continue
        cur['tokens'].append((cmd, a))
        for px, py in pts:
            b = cur['bbox']
            b[0] = min(b[0], px); b[1] = min(b[1], py)
            b[2] = max(b[2], px); b[3] = max(b[3], py)
    if cur is not None:
        subs.append(cur)
    return subs
