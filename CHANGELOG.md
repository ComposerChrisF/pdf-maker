# Changelog

All notable changes to `pdf-maker` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0/).

## [Unreleased]

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
