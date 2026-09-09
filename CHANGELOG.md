# Changelog

All notable changes to `pdf-maker` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0/).

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
