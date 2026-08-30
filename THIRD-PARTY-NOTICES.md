# Third-party notices

The source code of `pbn-to-pdf` is released under the Unlicense (see
[LICENSE](LICENSE)). The bundled assets below come from third parties. All are
freely redistributable, but two of them carry notice requirements that the
Unlicense does not itself satisfy — hence this file.

## Card artwork — public domain

`assets/cards/*.svg` and the derived `assets/cards/variants/*.svg`.

From **vector-playing-cards** by Byron Knoll,
<https://code.google.com/archive/p/vector-playing-cards/> (2011). The project
describes the set as:

> A full set of poker playing cards created using vector graphics. The .SVG
> source for each card is available as well as a high resolution rasterized
> .PNG version. These images are released into the public domain — attribution
> is appreciated but not required.

Attribution is given here because it is appreciated, and each SVG retains the
source URL in a comment at the top of the file.

These assets have been modified: the corner rank indices, originally `<text>`
elements depending on a system Arial, were converted to vector paths (see
`tools/svg_text_to_paths.py`), and the reduced court-card variants in
`assets/cards/variants/` were derived from the originals (see
`tools/make_card_variants.py`).

## DejaVu Sans — Bitstream Vera Fonts Copyright

`assets/fonts/DejaVuSans-Suits.ttf`, a 5-glyph subset carrying only the four
suit symbols (♠ ♣ ♥ ♦).

This is the **only font compiled into the binary**, and the only font program
embedded in generated PDFs. Full terms: [assets/fonts/LICENSE-DejaVu.txt](assets/fonts/LICENSE-DejaVu.txt).

> Fonts are (c) Bitstream (see below). DejaVu changes are in public domain.
> Glyphs imported from Arev fonts are (c) Tavmjong Bah (see below)
>
> Copyright (c) 2003 by Bitstream, Inc. All Rights Reserved. Bitstream Vera is
> a trademark of Bitstream, Inc.

The licence is permissive but requires that this copyright and permission
notice accompany all copies of the font software. "Bitstream" and "Vera" are
reserved names for derived fonts; this subset keeps the name "DejaVu Sans".

## Arimo — SIL Open Font License 1.1

`assets/fonts/Arimo-CardRanks.ttf`, a 14-glyph subset (`0`–`9`, `A`, `J`, `K`,
`Q`) used by `tools/svg_text_to_paths.py`.

**Not compiled into the binary and not embedded in generated PDFs** — it is
carried only so the card-index conversion can be re-run reproducibly. Full
terms: [assets/fonts/LICENSE-Arimo.txt](assets/fonts/LICENSE-Arimo.txt).

> Copyright 2020 The Arimo Project Authors
> (<https://github.com/googlefonts/arimo>)
>
> This Font Software is licensed under the SIL Open Font License, Version 1.1.

Arimo declares no Reserved Font Name, so this modified subset retains the name.
Arimo is a trademark of Google Inc.

## PDF standard-14 fonts — not redistributed

Body text is drawn with Times and Helvetica, referenced by name as PDF
standard-14 builtin fonts. No font program for these is embedded in the output;
conforming PDF viewers supply them. Nothing is redistributed and no licence
applies to this project.
