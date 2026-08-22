#!/usr/bin/env python3
"""Generate the reduced court-card assets used by the declarer's plan layouts.

The vector-playing-cards court illustrations are enormous -- the twelve J/Q/K
SVGs account for ~98% of a declarer's plan PDF -- yet those layouts almost
never show a whole court card.  In the dummy stack only the top 18% of a
covered card is visible; in declarer's fan only a narrow strip down the left
edge is.  This tool derives two much smaller variants per court card:

  <card>_band.svg    the top 18% (plus margin), portrait artwork geometrically
                     clipped to that band -- used for covered dummy cards.
  <card>_corner.svg  card, border and the top-left index only, with no portrait
                     artwork at all -- used for covered cards in the fan, where
                     the portrait frame otherwise leaks a stray vertical rail
                     into the exposed wedge.

Both keep the original width/height/viewBox so the renderer's placement maths
is unchanged; they simply have empty space where artwork was removed.  The
card background and border are never clipped -- the covering card meets them
with antialiasing, and clipping them leaves visible hairline seams.

Run from the repository root:  python3 tools/make_card_variants.py
"""
import os
import re
import sys
import xml.etree.ElementTree as ET

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from svgpath import subpaths  # noqa: E402

SVG_NS = 'http://www.w3.org/2000/svg'
SVG = '{%s}' % SVG_NS

CARD_W = 167.0869141
CARD_H = 242.6669922

# Visible fraction of a covered card. DummyRenderer overlaps at 0.18 of card
# height, so the covering card's opaque background hides everything below that;
# the margin here only has to clear its antialiased top edge. Ink density rises
# steeply with depth into the band -- 5% of the slice at 4-6% down, 57% at
# 16-18% -- so the last couple of percent are the expensive ones, and 0.185
# costs a third less than 0.20 for a clip edge still 0.24mm out of sight.
BAND_FRACTION = 0.185
# A generous bound on the top-left index (rank glyph plus suit pip); used only
# to sanity-check the pip we pick, never to select it. The index pips vary more
# than you would expect -- the jack of diamonds reaches y=54.7 where most stop
# short of 51 -- so a containment test silently drops the odd one.
CORNER_W_FRACTION = 0.18
CORNER_H_FRACTION = 0.26

RANKS = ('jack', 'queen', 'king')
SUITS = ('clubs', 'diamonds', 'hearts', 'spades')

# The assets do not agree on child order, so every element is classified by
# what it contains and where it lands on the card rather than by its index.
PORTRAIT_MIN_BYTES = 10_000


# --------------------------------------------------------------------------
# Geometry
# --------------------------------------------------------------------------

def _split_cubic(p0, p1, p2, p3, t):
    """de Casteljau: -> (left cubic, right cubic), each as 4 points."""
    def lerp(a, b):
        return (a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t)
    a, b, c = lerp(p0, p1), lerp(p1, p2), lerp(p2, p3)
    d, e = lerp(a, b), lerp(b, c)
    f = lerp(d, e)
    return (p0, a, d, f), (f, e, c, p3)


def _cubic_y(p0, p1, p2, p3, t):
    u = 1 - t
    return (u * u * u * p0[1] + 3 * u * u * t * p1[1]
            + 3 * u * t * t * p2[1] + t * t * t * p3[1])


def _cubic_crossings(p0, p1, p2, p3, ylim, steps=48):
    """Parameters where the curve crosses y = ylim, ascending."""
    out = []
    prev_t, prev_v = 0.0, _cubic_y(p0, p1, p2, p3, 0.0) - ylim
    for i in range(1, steps + 1):
        t = i / steps
        v = _cubic_y(p0, p1, p2, p3, t) - ylim
        if prev_v == 0.0:
            out.append(prev_t)
        elif (prev_v < 0) != (v < 0):
            lo, hi = prev_t, t
            for _ in range(40):
                mid = (lo + hi) / 2
                if (_cubic_y(p0, p1, p2, p3, mid) - ylim < 0) == (prev_v < 0):
                    lo = mid
                else:
                    hi = mid
            out.append((lo + hi) / 2)
        prev_t, prev_v = t, v
    return [t for t in out if 1e-9 < t < 1 - 1e-9]


def _segments(sub):
    """Subpath tokens -> (start_point, [('L', p1) | ('C', c1, c2, p1), ...])."""
    x = y = 0.0
    start = None
    segs = []
    for cmd, a in sub['tokens']:
        rel = cmd.islower()
        C = cmd.upper()
        if C == 'M':
            x, y = (x + a[0], y + a[1]) if rel else (a[0], a[1])
            start = (x, y)
        elif C == 'Z':
            if start and (x, y) != start:
                segs.append(('L', start))
            x, y = start if start else (x, y)
        elif C == 'H':
            x = x + a[0] if rel else a[0]
            segs.append(('L', (x, y)))
        elif C == 'V':
            y = y + a[0] if rel else a[0]
            segs.append(('L', (x, y)))
        elif C == 'L':
            x, y = (x + a[0], y + a[1]) if rel else (a[0], a[1])
            segs.append(('L', (x, y)))
        elif C == 'C':
            pts = []
            for j in range(0, 6, 2):
                pts.append((x + a[j], y + a[j + 1]) if rel else (a[j], a[j + 1]))
            segs.append(('C', pts[0], pts[1], pts[2]))
            x, y = pts[2]
        else:
            # S/Q/T/A do not occur in these assets; bail out rather than guess.
            raise ValueError('unsupported path command %r' % cmd)
    return start, segs


def clip_subpath_to_band(sub, ylim):
    """Sutherland-Hodgman clip of one filled subpath to the half-plane y <= ylim.

    Exact for a half-plane: the boundary below ylim is replaced by segments
    running along ylim, which leaves the filled area above it unchanged.
    Returns SVG path data, or None if nothing survives.
    """
    start, segs = _segments(sub)
    if start is None:
        return None
    inside = lambda p: p[1] <= ylim  # noqa: E731

    out = []          # emitted commands as (letter, points...)
    cur = start
    pen_down = inside(start)
    if pen_down:
        out.append(('M', start))

    def resume(p):
        """Re-enter the region at p, walking along the clip edge if needed."""
        nonlocal pen_down
        if not out:
            out.append(('M', p))
        else:
            out.append(('L', p))
        pen_down = True

    for seg in segs:
        if seg[0] == 'L':
            p1 = seg[1]
            a_in, b_in = inside(cur), inside(p1)
            if a_in and b_in:
                out.append(('L', p1))
            elif a_in and not b_in:
                t = (ylim - cur[1]) / (p1[1] - cur[1])
                out.append(('L', (cur[0] + (p1[0] - cur[0]) * t, ylim)))
                pen_down = False
            elif not a_in and b_in:
                t = (ylim - cur[1]) / (p1[1] - cur[1])
                resume((cur[0] + (p1[0] - cur[0]) * t, ylim))
                out.append(('L', p1))
            cur = p1
        else:
            c1, c2, p1 = seg[1], seg[2], seg[3]
            ts = _cubic_crossings(cur, c1, c2, p1, ylim)
            pieces = []
            p0i, p1i, p2i, p3i = cur, c1, c2, p1
            prev = 0.0
            for t in ts:
                tt = (t - prev) / (1 - prev)
                left, right = _split_cubic(p0i, p1i, p2i, p3i, tt)
                pieces.append(left)
                p0i, p1i, p2i, p3i = right
                prev = t
            pieces.append((p0i, p1i, p2i, p3i))
            for piece in pieces:
                mid_in = inside(((piece[0][0] + piece[3][0]) / 2,
                                 (_cubic_y(*piece, 0.5))))
                if mid_in:
                    if not pen_down:
                        resume(piece[0])
                    out.append(('C', piece[1], piece[2], piece[3]))
                else:
                    pen_down = False
            cur = p1

    if len(out) < 2:
        return None

    def n(v):
        s = '%.4f' % v
        s = s.rstrip('0').rstrip('.')
        return s if s not in ('', '-', '-0') else '0'

    parts = []
    for item in out:
        letter, pts = item[0], item[1:]
        parts.append(letter + ' '.join('%s,%s' % (n(p[0]), n(p[1])) for p in pts))
    return ''.join(parts) + 'z'


# --------------------------------------------------------------------------
# Asset rewriting
# --------------------------------------------------------------------------

def _affine(el):
    """Resolve an element's own transform to (a, b, c, d, e, f)."""
    tr = el.get('transform') or ''
    m = re.match(r'matrix\(([-\d.,eE+ ]+)\)', tr)
    if m:
        return [float(x) for x in re.split(r'[,\s]+', m.group(1).strip())]
    m = re.match(r'scale\(([-\d.,eE+ ]+)\)', tr)
    if m:
        v = [float(x) for x in re.split(r'[,\s]+', m.group(1).strip())]
        sx = v[0]
        sy = v[1] if len(v) > 1 else sx
        return [sx, 0.0, 0.0, sy, 0.0, 0.0]
    m = re.match(r'translate\(([-\d.,eE+ ]+)\)', tr)
    if m:
        v = [float(x) for x in re.split(r'[,\s]+', m.group(1).strip())]
        return [1.0, 0.0, 0.0, 1.0, v[0], (v[1] if len(v) > 1 else 0.0)]
    return [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]


def _mul(m, n):
    """Compose transforms: apply m, then n."""
    a, b, c, d, e, f = m
    A, B, C, D, E, F = n
    return [a * A + b * C, a * B + b * D,
            c * A + d * C, c * B + d * D,
            e * A + f * C + E, e * B + f * D + F]


def _apply(m, x, y):
    a, b, c, d, e, f = m
    return (a * x + c * y + e, b * x + d * y + f)


def element_bbox(el, parent=(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)):
    """Bounding box of everything an element draws, in card coordinates."""
    m = _mul(_affine(el), list(parent))
    box = None

    def grow(x0, y0, x1, y1):
        nonlocal box
        pts = [_apply(m, x0, y0), _apply(m, x1, y0),
               _apply(m, x0, y1), _apply(m, x1, y1)]
        xs = [p[0] for p in pts]
        ys = [p[1] for p in pts]
        cand = [min(xs), min(ys), max(xs), max(ys)]
        if box is None:
            box = cand
        else:
            box = [min(box[0], cand[0]), min(box[1], cand[1]),
                   max(box[2], cand[2]), max(box[3], cand[3])]

    if el.tag == SVG + 'path':
        for sub in subpaths(el.get('d', '')):
            grow(*sub['bbox'])
    elif el.tag == SVG + 'text':
        # Approximate: the glyph box from the anchor and font-size.
        try:
            x = float(el.get('x', 0)); y = float(el.get('y', 0))
        except ValueError:
            return None
        fs = 30.0
        ms = re.search(r'font-size:\s*([\d.]+)', el.get('style') or '')
        if ms:
            fs = float(ms.group(1))
        grow(x, y - fs, x + fs * 0.8, y + fs * 0.25)
    else:
        for child in el:
            sub = element_bbox(child, m)
            if sub:
                grow(*[0, 0, 0, 0]) if False else None
                if box is None:
                    box = list(sub)
                else:
                    box = [min(box[0], sub[0]), min(box[1], sub[1]),
                           max(box[2], sub[2]), max(box[3], sub[3])]
    return box


def classify(root):
    """-> dict of role -> element, for one court-card asset."""
    roles = {'pips': []}
    for el in root:
        tag = el.tag.replace(SVG, '')
        if tag in ('metadata', 'defs') or 'namedview' in el.tag:
            continue
        paths = list(el.iter(SVG + 'path')) if el.tag != SVG + 'path' else [el]
        dbytes = sum(len(p.get('d', '')) for p in paths)
        styles = ' '.join((p.get('style') or '') for p in paths)
        if 'fill:#FFFFFF' in styles or 'fill:#ffffff' in styles:
            roles['background'] = el
        elif 'fill:none' in styles:
            roles['frame'] = el
        elif dbytes >= PORTRAIT_MIN_BYTES:
            roles['portrait'] = el
        elif el.tag == SVG + 'text':
            key = 'text_mirror' if 'scale(-1' in (el.get('transform') or '') else 'text_index'
            roles[key] = el
        else:
            roles['pips'].append((el, element_bbox(el)))
    missing = {'background', 'portrait', 'text_index'} - set(roles)
    if missing:
        raise SystemExit('could not classify %s' % sorted(missing))
    return roles




def _clone_root(src):
    root = ET.Element(SVG + 'svg')
    for k in ('width', 'height', 'viewBox', 'version'):
        if src.get(k) is not None:
            root.set(k, src.get(k))
    return root


def build_corner(src_root, name=''):
    """Card, border and the top-left index only -- no portrait artwork."""
    roles = classify(src_root)
    root = _clone_root(src_root)
    root.append(roles['background'])
    root.append(roles['text_index'])
    root.append(index_pip(roles, name))
    return root


def index_pip(roles, name=''):
    """The suit pip under the top-left rank glyph.

    Picked by role, not by fitting a box: it is the leftmost pip in the card's
    top-left quadrant. The only other pip up there is the decorative one inside
    the portrait frame, which starts around x=25 where the index pip starts
    around x=2, so the two never come close to being confused.
    """
    candidates = [
        (el, bb) for el, bb in roles['pips']
        if bb and bb[0] < CARD_W / 2 and bb[1] < CARD_H / 2
    ]
    if not candidates:
        raise SystemExit('%s: found no index pip' % name)
    el, bb = min(candidates, key=lambda pair: pair[1][0])
    if bb[2] > CARD_W * CORNER_W_FRACTION or bb[3] > CARD_H * CORNER_H_FRACTION:
        raise SystemExit('%s: index pip %s escapes the corner box (%.1f x %.1f)'
                         % (name, [round(v, 1) for v in bb],
                            CARD_W * CORNER_W_FRACTION, CARD_H * CORNER_H_FRACTION))
    return el


def build_band(src_root):
    """The top BAND_FRACTION of the card, portrait artwork clipped to it."""
    roles = classify(src_root)
    root = _clone_root(src_root)
    ylim_card = CARD_H * BAND_FRACTION

    defs = ET.SubElement(root, SVG + 'defs')
    cp = ET.SubElement(defs, SVG + 'clipPath', {'id': 'band'})
    ET.SubElement(cp, SVG + 'rect', {
        'x': '0', 'y': '0',
        'width': '%.4f' % CARD_W, 'height': '%.4f' % ylim_card})

    # The background and border are never clipped: the covering card meets
    # them with antialiasing, and clipping them leaves hairline seams.
    root.append(roles['background'])
    clipped = ET.SubElement(root, SVG + 'g', {'clip-path': 'url(#band)'})

    # Everything else keeps its original document order -- the frame is
    # stroked over the portrait, so reordering leaves a seam along its edge.
    pips = {id(el): bb for el, bb in roles['pips']}
    for el in src_root:
        if el is roles['background'] or el is roles.get('text_mirror'):
            continue
        if el.tag == SVG + 'defs' or el.tag == SVG + 'metadata' or 'namedview' in el.tag:
            continue
        if el is roles['portrait']:
            _append_clipped_portrait(clipped, el, ylim_card)
        elif id(el) in pips:
            bb = pips[id(el)]
            if bb and bb[1] < ylim_card:
                clipped.append(el)
        else:
            clipped.append(el)
    return root


def _append_clipped_portrait(parent, portrait, ylim_card):
    a, b, c, d, e, f = _affine(portrait)
    if b or c:
        raise SystemExit('portrait transform is not axis-aligned')
    ylim_local = (ylim_card - f) / d

    new_g = ET.SubElement(parent, SVG + 'g', {'transform': portrait.get('transform')})
    for path in portrait:
        if path.tag != SVG + 'path':
            continue
        kept = []
        for sub in subpaths(path.get('d', '')):
            y0, y1 = sub['bbox'][1], sub['bbox'][3]
            if y0 > ylim_local:
                continue                       # wholly below the band
            if y1 <= ylim_local:
                kept.append(_reemit(sub))      # wholly inside
            else:
                piece = clip_subpath_to_band(sub, ylim_local)
                if piece:
                    kept.append(piece)
        if kept:
            np_ = ET.SubElement(new_g, SVG + 'path')
            for k, v in path.attrib.items():
                if k != 'd':
                    np_.set(k, v)
            np_.set('d', ''.join(kept))


def _reemit(sub):
    def n(v):
        s = '%.4f' % v
        s = s.rstrip('0').rstrip('.')
        return s if s not in ('', '-', '-0') else '0'
    parts = []
    for cmd, a in sub['tokens']:
        parts.append(cmd + ','.join(n(v) for v in a))
    return ''.join(parts)


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    cards = os.path.join(here, '..', 'assets', 'cards')
    outdir = os.path.join(cards, 'variants')
    os.makedirs(outdir, exist_ok=True)
    ET.register_namespace('', SVG_NS)

    total_src = total_band = total_corner = 0
    for rank in RANKS:
        for suit in SUITS:
            name = '%s_of_%s' % (rank, suit)
            src = os.path.join(cards, name + '.svg')
            root = ET.parse(src).getroot()

            band = ET.ElementTree(build_band(root))
            corner = ET.ElementTree(build_corner(ET.parse(src).getroot(), name))
            bp = os.path.join(outdir, name + '_band.svg')
            cpth = os.path.join(outdir, name + '_corner.svg')
            band.write(bp, encoding='utf-8', xml_declaration=True)
            corner.write(cpth, encoding='utf-8', xml_declaration=True)

            s, b, c = (os.path.getsize(src), os.path.getsize(bp), os.path.getsize(cpth))
            total_src += s; total_band += b; total_corner += c
            print('%-22s %9s -> band %8s  corner %7s' % (name, f'{s:,}', f'{b:,}', f'{c:,}'))
    print('\ntotal  %s -> band %s + corner %s  (%.1f%% of original)'
          % (f'{total_src:,}', f'{total_band:,}', f'{total_corner:,}',
             (total_band + total_corner) / total_src * 100))


if __name__ == '__main__':
    main()
