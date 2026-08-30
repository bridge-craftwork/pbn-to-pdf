#!/usr/bin/env python3
"""Replace the <text> rank indices in the card assets with vector paths.

Why: every card SVG draws its two corner rank indices ("J", "10", ...) as
<text font-family:Arial>.  usvg resolves that through a system font database,
which means

  * the glyphs render in whatever the host substitutes for Arial, so the same
    input produces different-looking cards on macOS, Linux and CI; and
  * they vanish entirely in wasm builds -- printpdf hardcodes an empty
    usvg::Options on wasm32 and exposes no way to supply fonts.

Baking the outlines into the assets removes the runtime font dependency without
changing what lands in the PDF: printpdf sets svg2pdf's embed_text = false, so
these glyphs were already being flattened to paths on every render.  The PDF
gains no text operators it did not have and no new objects -- the same outlines
simply arrive from the asset instead of from the host's font stack.

The font is Arimo (SIL OFL), which is metrically identical to Arial -- every
glyph used here has a bit-identical advance width -- so the indices keep their
intended shape and position.  assets/fonts/Arimo-CardRanks.ttf is a 14-glyph
subset carried in the repo purely so this tool can be re-run reproducibly; it
is not compiled into the binary.

Each <text> becomes a single <path> carrying the original element's id and a
class of "rank-index" (top-left) or "rank-index mirror" (the scale(-1,-1)
copy), which is how tools/make_card_variants.py identifies them afterwards.
Element transforms are baked into the coordinates so the paths need none, which
keeps bounding-box maths in that tool honest.

Run from the repository root, then regenerate the derived variants:

    python3 tools/svg_text_to_paths.py
    python3 tools/make_card_variants.py
"""
import argparse
import os
import re
import sys
import xml.etree.ElementTree as ET

from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.misc.transform import Transform
from fontTools.ttLib import TTFont

SVG_NS = 'http://www.w3.org/2000/svg'
SVG = '{%s}' % SVG_NS

# Namespaces the assets use; needed to parse a <text> block in isolation.
NS_DECLS = (
    'xmlns="http://www.w3.org/2000/svg" '
    'xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd" '
    'xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape"'
)

TEXT_BLOCK = re.compile(r'([ \t]*)(<text\b.*?</text>)', re.S)
DEFAULT_FONT_SIZE = 32.0


def style_prop(style, prop):
    """Value of `prop` in an SVG style attribute, or None."""
    if not style:
        return None
    m = re.search(r'(?:^|;)\s*%s\s*:\s*([^;]+)' % re.escape(prop), style)
    return m.group(1).strip() if m else None


def num(v, default=0.0):
    try:
        return float(re.sub(r'[a-z%]+$', '', (v or '').strip()))
    except (TypeError, ValueError):
        return default


def fmt(n):
    """2dp, trailing zeros stripped -- matches the density of the existing art."""
    s = '%.2f' % n
    s = s.rstrip('0').rstrip('.')
    return '0' if s in ('', '-0') else s


def glyph_path(font, glyphset, upm, char, x, y, size, mirrored):
    """SVG path data for `char` placed at the text anchor (x, y).

    Fonts are y-up and SVG is y-down, hence the -size scale on y.  A
    transform="scale(-1,-1)" on the source element is folded in here rather
    than emitted, so the resulting path stands alone.
    """
    cmap = font.getBestCmap()
    if ord(char) not in cmap:
        raise SystemExit('font has no glyph for %r' % char)
    s = size / upm
    if mirrored:
        t = Transform(-s, 0, 0, s, -x, -y)
    else:
        t = Transform(s, 0, 0, -s, x, y)
    pen = SVGPathPen(glyphset, ntos=fmt)
    glyphset[cmap[ord(char)]].draw(TransformPen(pen, t))
    return pen.getCommands()


def convert_block(block, font, glyphset, upm):
    """One <text> element -> one <path> element, or None if it draws nothing."""
    root = ET.fromstring('<root %s>%s</root>' % (NS_DECLS, block))
    text = root[0]
    tspans = list(text.iter(SVG + 'tspan'))
    if not tspans:
        return None
    tspan = tspans[0]
    chars = ''.join(t.text or '' for t in tspans).strip()
    if not chars:
        return None

    outer_style = text.get('style') or ''
    size = num(style_prop(outer_style, 'font-size'), DEFAULT_FONT_SIZE)
    # The tspan's fill wins where it sets one; the assets set it in both places.
    fill = (style_prop(tspan.get('style') or '', 'fill')
            or style_prop(outer_style, 'fill') or '#000000')
    x = num(tspan.get('x', text.get('x')))
    y = num(tspan.get('y', text.get('y')))
    mirrored = 'scale(-1' in (text.get('transform') or '')

    d = []
    advance = 0.0
    for ch in chars:
        d.append(glyph_path(font, glyphset, upm, ch, x + advance, y, size, mirrored))
        gid = font.getBestCmap()[ord(ch)]
        advance += font['hmtx'][gid][0] * size / upm
    if not any(d):
        return None

    el = ET.Element(SVG + 'path')
    el.set('d', ' '.join(p for p in d if p))
    el.set('style', 'fill:%s;fill-opacity:1;stroke:none' % fill)
    if text.get('id'):
        el.set('id', text.get('id'))
    el.set('class', 'rank-index mirror' if mirrored else 'rank-index')
    # register_namespace keeps the element unprefixed (<path>, not <ns0:path>);
    # the root already declares the SVG namespace, so strip the re-declaration
    # a standalone serialisation adds.
    ET.register_namespace('', SVG_NS)
    out = ET.tostring(el, encoding='unicode')
    return out.replace(' xmlns="%s"' % SVG_NS, '', 1)


def convert_file(path, font, glyphset, upm, dry_run=False):
    src = open(path, encoding='utf-8').read()
    converted = [0]

    def repl(m):
        indent, block = m.group(1), m.group(2)
        out = convert_block(block, font, glyphset, upm)
        if out is None:
            return m.group(0)
        converted[0] += 1
        return indent + out

    new = TEXT_BLOCK.sub(repl, src)
    if converted[0] and not dry_run:
        open(path, 'w', encoding='utf-8').write(new)
    return converted[0], len(src), len(new)


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--font', default=os.path.join(here, '..', 'assets', 'fonts',
                                                   'Arimo-CardRanks.ttf'))
    ap.add_argument('--cards', default=os.path.join(here, '..', 'assets', 'cards'))
    ap.add_argument('--dry-run', action='store_true')
    args = ap.parse_args()

    font = TTFont(args.font)
    glyphset = font.getGlyphSet()
    upm = font['head'].unitsPerEm

    names = sorted(n for n in os.listdir(args.cards) if n.endswith('.svg'))
    if not names:
        raise SystemExit('no SVGs in %s' % args.cards)

    total, before, after = 0, 0, 0
    for name in names:
        n, b, a = convert_file(os.path.join(args.cards, name), font, glyphset, upm,
                               args.dry_run)
        total += n
        before += b
        after += a
        if n:
            print('  %-28s %2d text -> path  %+d B' % (name, n, a - b))
    if not total:
        print('no <text> elements found -- already converted?')
        return
    print('\n%d elements in %d files, %+.1f KB (%.1f -> %.1f KB)%s'
          % (total, len(names), (after - before) / 1024, before / 1024, after / 1024,
             '  [dry run]' if args.dry_run else ''))
    print('now run: python3 tools/make_card_variants.py')


if __name__ == '__main__':
    main()
