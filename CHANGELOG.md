# Changelog

All notable changes to `pdf-maker` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0/).

## [0.24.0] — 2026-09-10
### Added
- **Duplicate pages in a page spec are honored (bug-0003).**  `"1,1"` yields two
  copies of page 1; `"1-2,2"` yields three pages in the order written.  A page
  spec is a list of pages to emit, not a set to select — useful for a facing-page
  layout or a duplicated insert, and previously expressible only by naming the
  same file twice on the command line.

  Before this, a repeated page number was silently absorbed: the caller asked for
  two pages and got one, at exit 0 — the same silent-drop class v0.13.0 shipped to
  eliminate for out-of-range pages, which had simply never been noticed for
  duplicates.

  **Repetition and out-of-range stay orthogonal.**  `"1,1,99"` against a two-page
  document still exits 1 naming page 99.  That invariant is pinned by a test here
  and by one in medpdf.

  The copies are independent page objects, so a `pages=` target addresses exactly
  one of them — a watermark on page 2 of `"1,1"` leaves page 1 alone.  Pinned by a
  test that draws on the second copy and asserts the first is untouched; a page
  count alone would prove nothing, since the malformed shape this replaced also
  reported two pages.

### Changed
- **`medpdf` floor raised to 0.15** — required, not optional.  Honoring duplicates
  needed two changes there: medpdf’s `plan-0006` (the parser stops collapsing
  repeats) and medpdf’s `bug-0040` (`copy_page_with_cache` stops handing back the
  same page object for a repeated page, which had put one object into `/Kids`
  twice with `/Count 2`).  bug-0040 was found and filed from this repo while
  adopting the first half.  A downgrade below 0.15 does not merely lose the
  feature — it reopens a malformed page tree.

### Known limitation
- A duplicated page that carries **annotations** shares those annotation objects
  between the copies, and each shared annotation’s `/P` points at the first copy.
  pdf-maker never edits annotations itself, so it hands the pair on rather than
  corrupting anything, and page content is unaffected.  Filed upstream as medpdf
  `bug-0041` with a reproduction from here.

## [0.23.0] — 2026-09-10
### Changed
- **BEHAVIOR CHANGE — `--pad-last-page-file` now requires `--pad-to` (bug-0011).**
  Alone, it parsed, had its `file=` existence-checked, and was then never read: the
  caller asked for a padding template and got an unchanged document at exit 0, with
  nothing on stderr.  The dependency is now declared in clap, so the lone flag is a
  usage error (exit 2) naming `--pad-to`.

- **BEHAVIOR CHANGE — `src_page=0` and `page=0` are usage errors (bug-0010).**
  Page numbers are 1-based, and a zero is wrong on its face, so it is rejected at
  parse (exit 2) instead of reaching the apply stage.

### Fixed
- **`--pad-last-page-file page=` is validated up front (bug-0010).**  It used to be
  checked only when padding actually happened, so a document already sitting on the
  `--pad-to` multiple accepted `page=99` in silence — a bad argument waiting for an
  input of a different length.  An argument’s validity must not depend on how much
  work it causes, and `--dry-run` should catch it either way.

- **Out-of-range page errors for `--overlay src_page` and `--pad-last-page-file
  page` now name the flag, the file, the page and the file’s real page count
  (bug-0010).**  Both used to surface medpdf’s `Page 99 not found in source
  document`, which named none of the four — unusable with several PDFs in one
  invocation.  Both now route through `page_spec::expand`, the same path every
  other caller-named page takes.

- **An unreadable input is no longer reported as missing (bug-0013).**  The probe
  used `path.exists()`, a bare bool that folds EACCES, EIO and ELOOP in with
  `NotFound`, so a file that existed but could not be stat-ed was announced as
  absent and sent the reader hunting for a typo that was not there.  It now answers
  three ways — present, provably absent, or “cannot be accessed”, naming the
  underlying error — per `positive-evidence-of-absence.md`.  The output-directory
  probe got the same treatment.  Nothing here gates a destructive action, so the
  stakes were only diagnostic; the message is now true either way.

- **XMP dates are ISO 8601 (bug-0015).**  `xmp:CreateDate`, `xmp:ModifyDate` and
  `xmp:MetadataDate` carried chrono’s `Display` form —
  `2026-09-10 07:02:57.068787 -10:00`, a space where the `T` belongs — which the
  XMP Date value type does not accept.  Every PDF pdf-maker had written carried
  three malformed dates.  They are now RFC 3339 (`2026-09-10T07:02:57-10:00`),
  pinned by a test that parses the value back rather than matching a shape.

## [0.22.0] — 2026-09-10
### Changed
- **BEHAVIOR CHANGE — `--draw-line`’s `width` now honors `units=` (bug-0002).**
  `--draw-line "x1=1,y1=1,x2=7,y2=1,width=0.02,units=in"` draws a line 0.02
  **inches** thick; before this release it drew 0.02 **points**, because the
  width was stored raw while the four coordinates beside it were converted.  One
  `units=` key now governs every distance in the spec.

  This also removes an inconsistency between siblings: `--draw-rect`’s `h` plays
  the same thickness role and has always converted, so the identical hairline
  drawn as a rect and as a line disagreed.

  **If you have a script passing `width=` together with a non-`pt` `units=`, its
  lines will thicken.**  Nothing in the portfolio does — the checked callers use
  `width=` with default `pt` units, which is unaffected — but a bare
  `width=1,units=in` that meant a hairline now means a one-inch bar.

### Fixed
- **A false `ERROR` on every imposition run (bug-0017).**  `init_document` built
  the XMP metadata stream with a struct literal instead of `Stream::new`, so the
  stream carried no `/Length` — the one entry the PDF spec requires of every
  stream.  Every path that ran `compress()` rewrote `/Length` on the way out and
  hid it; `impose_pages` round-trips through a raw `save_to` + `load_mem`, which
  does not, so `--nup`, `--booklet` and `--tile` each logged

  ```
  [ERROR lopdf::reader] stream dictionary of '2 0 R' is missing the Length entry
  ```

  and then exited 0.  The output files were always clean — the malformed object
  only ever existed in the intermediate buffer — so this was a false alarm rather
  than a corruption.  That is precisely why it was worth fixing: an ERROR line on
  a successful run teaches a reader, human or agent, to ignore the exact message
  that would announce a real `/Length` regression, which this repo pins other
  tests against.

  Pinned by a test that asserts on **stderr**, not on the exit code — the run
  succeeded before and after, so an exit-code assertion would have passed with
  the bug still in.

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
