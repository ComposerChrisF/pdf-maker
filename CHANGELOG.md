# Changelog

All notable changes to `pdf-maker` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0/).

## [0.21.2] — 2026-09-09
### Documentation
- **The spec-drift pass (bug-0012).**  README.md described a tool that had not
  existed since v0.10.0: imposition was wholly absent, and so were `--draw-rect`,
  `--draw-line`, `--draw-image`, `--blank-page`, `--no-subset`, half the
  watermark parameters, five of the eleven named colors, and every encryption
  flag but the two passwords.  It now documents all of them, plus `--nup`,
  `--booklet` and `--tile` with their key tables, the `flip`-by-orientation
  table, the tile coverage guards, the text escapes, a `--json` sample taken
  from a real run, and a seven-phase pipeline (it claimed five, omitting
  imposition and font subsetting).

  Two behaviors are documented as **current plus pending**, because their fixes
  are ruled but unlanded: `--draw-line` `width` is in points regardless of
  `units=` (bug-0002), and duplicate page numbers collapse (bug-0003).  The
  commit that lands either fix owes its README line.

### Added
- **`--watermark` and `--dry-run` gained `long_help`.**  Two of bug-0012’s items
  were about `--help`, not the README.  The watermark help now carries the full
  key table with defaults, the named-color set, and the **text escapes** — which
  had never been documented in this repo at all; the only description of them
  lived in an external skill file, outside the tool that defines them.  It says
  plainly that `\n` renders and `\t` does not: they decode alike, and only one
  has a rendering model.
- A drift guard for the watermark help, asserting it against `WATERMARK_KEYS`
  rather than a copy of the list — the mechanism bug-0005 built for the
  imposition flags, now covering four flags instead of three.

### Changed
- `--dry-run`’s help no longer says “validate”, which overstated it.  The flag
  branches at exactly one place, the save, so it runs the whole pipeline against
  the real document and skips compression, the application of encryption, and
  the file write — the failure class `lopdf_save_modern_bug.rs` guards.  A
  document that passes `--dry-run` can still fail to be written, and the help
  now says so.

## [0.21.1] — 2026-09-09
### Changed
- Build hygiene: removed the dead `window_w` and `src_w` fields from `TilePlan`
  (with a doc comment on `window_h` explaining why there is no `window_w`
  companion) and dropped the unused `TileMarks` re-export, clearing the two
  warnings the v0.21.0 `--tile` commit shipped.  No behavior change.

## [0.21.0] — 2026-09-09
### Added
- **`--tile`: split one large page across many sheets, with overlap for taping**
  (plan-0003).  The inverse of `--nup`, and the replacement for the one Microsoft
  Publisher capability with no other home in the portfolio — Publisher is removed
  from Microsoft 365 on 2026-10-01 and takes tiled banner printing with it.

  ```
  pdf-maker -o banner-tiled.pdf banner.pdf all \
    --tile "paper_w=11,paper_h=17,units=in,marks=labels"
  ```

  Keys: `paper` / `paper_w` / `paper_h`, `orientation`, `overlap`, `margin`,
  `pages`, `order`, `marks`, `scale`, `align`, `max_sheets`, `units`.  Full table
  in `--help`.  Notable defaults and why they are what they are:

  - **`overlap` defaults to 0.75in, not zero and not 0.5in.**  Consumer printers
    hold roughly a quarter inch at each edge they cannot print, so the overlap you
    actually get when taping is `overlap − 2 × that border`.  At 0.5in that is
    zero, and any drift opens a white line through the artwork.
  - **`orientation=auto` picks whichever sheet orientation yields fewer sheets.**
    Not cosmetic: a 48×12in banner is five tabloid sheets one way and six the
    other, and the conference banner that motivated this is four versus seven.
  - **`align=center`** spreads the grid’s surplus over both ends, so every sheet
    carries a similar amount rather than leaving one nearly blank.
  - **`max_sheets=400`** refuses a runaway run.  `--tile` is the first pdf-maker
    operation whose output size is _derived_ rather than stated, so a units
    mistake turns one page into hundreds of sheets.

  Guards, all exit 1 naming the arithmetic: an `overlap` at least as wide as the
  printable window (the grid would never terminate), and an overlap too small to
  cover a negative `margin` (the tiles would not meet, silently dropping a strip
  of artwork at every seam).  **Coverage, not sign, is the invariant** — a
  negative margin is legal exactly while the overlap still covers it.

  `marks=labels` captions each sheet `p1 R2C3 of 5x1`, naming the source page as
  well as the cell because `pages=all` interleaves several grids into one file.
  `marks=crop` draws the tile boundary for butting rather than lapping.

  `--tile` runs in the same pipeline slot as `--nup` and `--booklet` — after
  merge, before overlays and padding — so a watermark’s `pages=` spec addresses
  **sheets**, and `--pad-to` pads the sheet count.  It conflicts with both.

## [0.20.0] — 2026-09-09
### Fixed
- **Security: asking to restrict a document without a password is now refused
  instead of silently ignored** (bug-0004).  `--permissions` and
  `--encryption-algorithm` were consumed inside the arm that only runs when a
  password is present, so without one they were read and discarded:
  `--permissions none` wrote an **unencrypted** PDF with **every** permission
  available, at exit 0 — the exact opposite of the stated intent, with no signal.
  Both flags now declare a clap dependency on `--user-password` /
  `--owner-password`, so the combination is rejected before any work begins, exit
  2, naming the missing password.
- **Permission names are validated at parse time.**  Previously an invalid name
  was only checked _inside_ the encryption arm, so without a password it was never
  checked at all, and with one it surfaced as a tool error (exit 1) from deep
  inside the run.  It is now a clap `value_parser`, so `--permissions bogus` is a
  usage error (exit 2) listing the valid names.
- **Statically-invalid invocations exit 2, not 1** (bug-0014).  An odd number of
  positional arguments is reported through clap, which also removes an
  inconsistency: a _single_ positional already exited 2 via clap’s own `num_args`
  floor, so the same mistake produced two different codes depending on arity.
  The distinction is machine-actionable — 1 means “the tool failed, retry or debug
  it”, 2 means “fix the command line” — and a test now pins it from both sides, by
  asserting an out-of-range page still exits 1.

### Changed
- `--encryption-algorithm` no longer carries a clap `default_value`; the aes128
  default is applied in code.  clap cannot distinguish “defaulted” from
  “user-supplied” for a `requires` check, so an eager default would have made the
  new gate fire on every run.  Behavior is unchanged: a password alone still
  encrypts with aes128.

## [0.19.0] — 2026-09-09
### Fixed
- **A source page carrying `/Rotate 90` or `/Rotate 270` is now scaled to the cell
  it is actually placed in** (bug-0018).  `get_page_media_box` is the _pre-rotation_
  box, while `place_page` honors `/Rotate` — so imposition sized each cell from
  portrait extents and then placed a landscape footprint.  Measured on a 612×792
  sheet with 306×396 cells: every placement came out 396pt wide in a 306pt cell, so
  the left column overlapped its neighbour and the right column ran 90pt off the
  paper, at exit 0.  Imposition now measures with `medpdf::placed_page_size`,
  computed from the same transform `place_page` emits, so the geometry planned
  against and the geometry that lands cannot drift.
- **`--pad-to` sizes its blank pages by the last page as _displayed_** (bug-0018,
  second site).  A `/Rotate 90` last page previously got portrait pad pages appended
  behind a page that displays landscape.

## [0.18.0] — 2026-09-09
### Fixed
- **Oversized margins and gutters no longer produce mirrored, shrunken pages at
  exit 0** (bug-0006).  A margin or gutter large enough to consume the sheet gave
  a negative cell, hence a negative scale, which `place_page` accepted — silent
  garbage in a plausible-looking file.  Imposition geometry is now computed by
  one checked constructor that refuses a non-positive cell, exit 1, naming the
  paper, margin, gutter, grid and the resulting cell dimension.  `--booklet`
  gained the matching guard for `binding_margin` against the paper width.
- **Out-of-range numeric spec values are rejected at parse time** (bug-0009)
  rather than silently clamped or silently degenerate.  `alpha` outside
  `[0, 1]` was the worst case: medpdf clamps it, so `alpha=5` lost the
  transparency entirely and `alpha=-0.5` produced a **fully invisible** mark at
  exit 0.  Now `alpha` must be a fraction, and `w`, `h`, `size`, `width`,
  `paper_w` and `paper_h` must be greater than zero, across every spec type.

### Added
- **Imposition reports its derived geometry**, on stderr and in `--json` as a new
  `imposition_geometry` object (`cell_width_pt`, `cell_height_pt`,
  `bleed_overhang_pt`).  This is the obligation attached to permitting negative
  offsets: a negative `margin` is a deliberate full bleed and stays legal, so a
  _typo’d_ negative margin is legal too — and the computed geometry is the only
  thing that distinguishes them before the job reaches paper.

### Changed
- **`--draw-image max_dpi` takes `none` instead of `0`** for “no downsampling”,
  and a numeric value below 1.0 is now an error.  `max_dpi=0` was accepted with
  undefined downstream meaning; the sentinel now stays internal instead of
  appearing on the CLI surface.
- **Negative `margin`, `gutter` and `binding_margin` remain legal, deliberately.**
  Ruled 2026-09-09: an extent may not be negative, an offset may.  A negative
  margin is a full bleed — a real layout, verified working — and it cannot cause
  bug-0006’s fault, because it makes the cell _larger_.  A test pins this so a
  future pass applying the extent rule uniformly fails rather than quietly
  removing the capability.

## [0.17.0] — 2026-09-09
### Fixed
- **`--help` no longer advertises `--booklet` keys that do not exist, and now
  documents `--nup`’s keys at all** (bug-0005).  The booklet help named
  `orientation` and `duplex_flip` — the first is not a booklet key and the second
  was renamed to `flip` before v0.10.0 shipped — so following the help verbatim
  produced an error.  The `--nup` help listed no keys whatsoever, leaving all
  thirteen undiscoverable from the binary.  Both now carry a full key table under
  `long_help`, and `--blank-page` offers `legal` alongside `letter` and `a4`.
- The `--booklet` help gained a **table for choosing `flip`** by sheet
  orientation, graduated from bug-0001 — landscape takes `long_edge`, portrait
  takes `short_edge` — with the diagnostic that if the backs print upside down,
  the other value is the fix.

### Changed
- **An unknown spec key now lists the valid keys for that spec.**  Applies to
  every `key=value` flag (`--nup`, `--booklet`, `--watermark`, `--draw-*`,
  `--overlay`, `--pad-last-page-file`, `--blank-page`), not just the two whose
  help was wrong.  `--booklet "duplex_flip=..."` now answers with
  `Valid keys: paper, paper_w, paper_h, binding_margin, units, flip, back`,
  which is the only place a caller working from stale documentation will see the
  real name.

## [0.16.0] — 2026-09-09
### Fixed
- **`--booklet` duplex-flip compensation was inverted on landscape sheets**
  (bug-0001).  Confirmed by physical duplex print 2026-09-09: both `flip` values
  printed their back sides upside down, for opposite reasons.  The compensation
  counteracts the physical flip the duplexer performs, so it depends on the
  (sheet orientation, flip) **pair**, not on the flip alone — the axis that
  inverts content is the one parallel to the content’s horizontal, i.e. the long
  edge of a landscape sheet and the short edge of a portrait one.  The old rule
  rotated for `short_edge` unconditionally, which is the portrait rule applied to
  the default landscape sheet.  Now `long_edge` rotates on landscape and
  `short_edge` on portrait, and the custom-portrait “flip-book” case that the old
  code got right by accident still works.

### Changed
- **`--nup n=` now accepts only the canonical values `1, 2, 4, 6, 8, 9, 16`**
  (bug-0008).  A non-canonical `n` used to round up to a grid and fill every
  cell, so `n=3` silently produced 4-up, `n=5` 6-up and `n=7` 9-up — the flag
  said “input pages per sheet” and did something else.  It is now a usage error
  naming the accepted set and pointing at `cols=`/`rows=`, which expresses any
  grid exactly.
- **`--nup n=2` is now two pages side by side on a landscape sheet**, the
  print-dialog convention and what `--booklet` already did.  It was stacked on a
  portrait sheet — the only entry in the grid table that disagreed with the
  convention every other entry follows, costing 29 % of linear scale on letter
  sources (0.5 vs 0.647).  **The old layout is still available explicitly as
  `--nup "cols=1,rows=2"`**, which produces byte-identical output to the previous
  `n=2`.

## [0.15.0] — 2026-09-09
### Fixed
- **Multi-line watermark text now renders** (bug-0016).  `--watermark "text=Line
  1\nLine 2,..."` draws two baselines instead of one concatenated line.  Fixed
  upstream in medpdf 0.14.0; pdf-maker needed no change beyond the dependency
  floor, since `unescape_text` already decoded the escape correctly.  Note the
  pre-fix symptom differed by path: on the WinAnsi path the newline collapsed
  silently, while on the composite path (any non-WinAnsi character, e.g. a
  Hawaiian ‘okina) it raised `UnrepresentableText` and **failed the whole run**.
  Verified on both: `text=Line 1\nLine 2` at size 24 emits `(Line 1) Tj`,
  `0 -28.8 Td`, `(Line 2) Tj`; `text=Ka‘ū\nHawai‘i` with an embedded font
  extracts as two lines with diacritics intact.
- `\t` is **not** fixed by the above and is not a line break — there is no
  tab-stop model, and a decoded tab is dropped on the WinAnsi path and rejected
  on the composite one.  Do not describe the two escapes as one feature
  (bug-0012 carries the doc item).
- **`--booklet flip=short_edge` placed every back page completely off the sheet.**
  Introduced by adopting medpdf 0.13.0 and caught before release.  `apply_booklet`
  passed `(cx + w, cy + h)` for the rotated back side, hand-compensating the
  pre-0.13.0 contract in which a 180° placement landed at `[x-w, x] x [y-h, y]`.
  medpdf 0.13.0 anchors the placed bounding box at `(x, y)` for any rotation, so
  that arithmetic double-compensated and displaced each back page by exactly one
  placed width and height — a blank back side at exit 0.  The rotated branch now
  passes the same `(cx, cy)` as the unrotated one.  Pinned by
  `cli_booklet_back_pages_land_on_the_sheet`, which asserts on the destination
  rectangle: the `cm` scale coefficients are unchanged by this fault, so the
  obvious “was the 180° applied?” assertion passes in both the broken and the
  fixed state.
- Imposition no longer leaks one orphaned zero-byte stream per output sheet
  (bug-0007), fixed upstream in medpdf 0.13.0.

### Changed
- Adopt medpdf 0.14.0 and raise the requirement to `0.14`.  Single-line output is
  unchanged — verified by regenerating a 12-watermark, 4-page document and
  diffing every text-positioning operator against the 0.13.2 output: identical.
- Adopt medpdf 0.13.0 and raise the requirement to `0.13`.  `place_page` now
  places by **visible bounding box** — `(x, y, scale)` alone determines where a
  page lands, for any MediaBox origin and any rotation — and honors the source
  page’s `/Rotate` (medpdf bug-0023, bug-0024).  A build against an older 0.12.x
  silently restores the previous placement, hence the floor.

### Known issue
- Imposition still sizes cells from the **pre-rotation** MediaBox, so a source
  carrying `/Rotate 90` or `/Rotate 270` is now placed upright but mis-scaled and
  mis-centred (bug-0018).  Unrotated sources — every fixture in this repo and the
  overwhelmingly common case — are unaffected.

## [0.13.2] - 2026-07-24
### Changed
- Adapt to medpdf 0.12.0: `parse_page_spec` now errors on out-of-range pages, so
  `page_spec::expand` scans for out-of-range pages before delegating to medpdf,
  preserving its “name every out-of-range page” error message.  Bump the
  `medpdf` dependency to 0.12.

## [0.13.1] - 2026-07-15
### Added
- Adopt `medpdf` 0.11.0, gaining Unicode text via composite fonts — watermark
  and `--text` content can now carry the full Unicode range (Hawaiian ‘okina,
  kahakō) through the built-in font path, including `\uXXXX` / `\U{XXXXX}`
  escapes.

## [0.13.0] - 2026-07-14
### Changed
- A requested page beyond the end of a document is now a **tool error** (exit 1)
  that names the page and the document’s real page count, instead of being
  silently dropped.  This also covers a wholly out-of-range input spec and the
  `pages=` key of `--watermark` / `--draw-rect` / `--draw-line` / `--draw-image`
  / `--overlay`.  Use `all` for “however many pages there are”.
### Added
- `--dry-run` (full pipeline and validation, writes no file) and `--json`
  summary output.

## [0.12.4] - 2026-06-29
### Changed
- Bump `lopdf` 0.39 → 0.42 (toolchain-wide coordinated bump).

## [0.12.2] - 2026-03-16
### Fixed
- Encrypted PDFs written via `save_modern()` produced corrupt output.
### Added
- CLI integration tests.

## [0.12.1] - 2026-03-15
### Fixed
- Error handling, stderr output discipline, and encryption cleanup from code
  review.

## [0.12.0] - 2026-03-15
### Added
- `--booklet` back-matter support.

## [0.11.0] - 2026-03-15
### Added
- `--nup` `repeat` option for duplicating pages across cells.

## [0.10.0] - 2026-03-15
### Added
- N-up and booklet imposition (`--nup`, `--booklet`).

## [0.9.3] - 2026-03-13
### Changed
- Renamed from `pdf-merger` to `pdf-maker` across the codebase (first release
  under the new name; split out of the `medpdf` workspace).

Earlier history under the `pdf-merger` name is in the git log.

[Unreleased]: https://github.com/ComposerChrisF/pdf-maker/compare/pdf-maker-v0.13.1...HEAD
