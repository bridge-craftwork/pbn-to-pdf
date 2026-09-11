// svgo pass for the card art -- step 2 of the "Card asset pipeline" in
// CLAUDE.md, between tools/svg_text_to_paths.py and tools/make_card_variants.py.
// Run from the repository root:
//
//   npx --yes svgo@4.1.0 --config tools/svgo-cards.config.mjs -f assets/cards -o assets/cards
//
// `-f` does not descend into assets/cards/variants/; make_card_variants.py
// regenerates those from the rounded bases. Idempotent: re-running it on its own
// output changes nothing.
//
// Path-coordinate precision only.
//
// `cleanupNumericValues` is deliberately NOT here. It rewrites width/height
// into px and drops the unit -- `167.0869141pt` becomes `222.78` -- which
// leaves the card declaring an intrinsic size 4/3 larger than its own viewBox
// and the renderer drawing it a third too big. Nothing is gained by it either:
// 98.8% of these files is path data, which `convertPathData` owns.
//
// Every structural plugin is off: `make_card_variants.py` classifies elements
// by geometry and finds the rank indices by their class, so collapsing groups
// or dropping "unused" ids would break the pipeline rather than just the diff.
//
// The shorthand conversions are off for the same reason — `tools/svgpath.py`
// parses the path data itself and does not implement `s`/`t`, so letting svgo
// emit them fails the variant build. They are worth little here anyway: the
// weight is in the digits, not the commands.
export default {
  multipass: true,
  plugins: [
    {
      name: 'convertPathData',
      params: {
        floatPrecision: 2,
        transformPrecision: 4,
        applyTransforms: false,
        makeArcs: false,
        straightCurves: false,
        convertToQ: false,
        lineShorthands: false,
        curveSmoothShorthands: false,
        forceAbsolutePath: false,
        utilizeAbsolute: false,
        removeUseless: false,
        collapseRepeated: false,
      },
    },
  ],
};
