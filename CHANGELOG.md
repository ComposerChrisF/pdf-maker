# Changelog

All notable changes to `pdf-maker` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0/).

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
