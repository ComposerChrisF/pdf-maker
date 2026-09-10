# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
cargo build --release    # Build optimized binary
cargo check              # Fast type checking
cargo test               # Run all tests
```

## Repo Structure

This is a standalone crate (no workspace).  It depends on `medpdf` and `medpdf-image`, sibling crates developed in the adjacent `../medpdf` workspace and consumed here as `version + path` dependencies: local builds compile the path checkout, while the version requirement governs a crates.io publish.  See `PUBLISHING.md` (and `../medpdf/PUBLISHING.md`) for how the family is released.

```
pdf-maker/                    # Repository root
├── Cargo.toml                 # Crate manifest
├── src/
│   ├── main.rs                # CLI args (clap), long_help text, orchestrates pipeline
│   ├── imposition.rs          # N-up, booklet and tile layout
│   ├── page_spec.rs           # Bounds-checked page-spec expansion (see below)
│   ├── paths.rs               # Input/output path-contract checks
│   └── spec_types/            # CLI spec types with FromStr, grouped by kind
│       ├── mod.rs             # Re-exports; the only public surface
│       ├── parse.rs           # KvParser, colors, units, escapes, paper sizes
│       ├── drawing.rs         # WatermarkSpec, DrawRectSpec, DrawLineSpec, DrawImageSpec
│       ├── layout.rs          # NupSpec, BookletSpec, TileSpec (+ their key lists)
│       └── misc.rs            # OverlaySpec, PadToSpec, PadFileSpec, BlankPageSpec
└── tests/
    ├── cli_tests.rs              # CLI integration tests
    └── lopdf_save_modern_bug.rs  # Sentinel test for lopdf encryption bug
```

### Dependencies

- `medpdf` - Medium-level PDF library
- `medpdf-image` - Image embedding companion crate

## Architecture

**pdf-maker** is a CLI tool that uses medpdf for merging, imposing, overlaying, and watermarking PDFs.

### 7-Phase Processing Pipeline (`run()` in src/main.rs)

1. **Merge Pages** - Parse input file/page specs, load documents, copy selected pages, then append `--blank-page` pages
2. **Impose** - `--nup`, `--booklet` or `--tile` (clap-exclusive, at most one), rearranging pages onto sheets
3. **Apply Overlays** - Overlay content from other PDFs with resource renaming
4. **Apply Drawing Commands** - Add watermarks, rectangles, lines, and images (under layer first, then over)
5. **Subset Fonts** - Unless `--no-subset`
6. **Padding** - Pad document to multiple of N pages
7. **Save** - Compress, encrypt, and write output (the one step `--dry-run` skips)

Phase 2 is the load-bearing one for everything after it: once an imposition mode has run, the `pages=` target of every drawing and overlay flag addresses **sheets**, not source pages, and `--pad-to` pads the sheet count.  Keep any new page-rearranging mode in this same slot.

### Module Responsibilities

| Module | Purpose |
|--------|---------|
| `main` | CLI args (clap), orchestrates pipeline |
| `spec_types` | CLI spec types with FromStr for clap integration, in four submodules over a shared `KvParser`: `drawing` (`WatermarkSpec`, `DrawRectSpec`, `DrawLineSpec`, `DrawImageSpec`), `layout` (`NupSpec`, `BookletSpec`, `TileSpec`), `misc` (`OverlaySpec`, `PadToSpec`, `PadFileSpec`, `BlankPageSpec`), `parse` (private helpers).  The `*_KEYS` constants are the single source of truth for which keys exist — the `--help` drift guards assert against them, never against a copy |
| `imposition` | N-up, booklet and tile layout engine: page placement, scaling, and sheet generation.  `CellGeometry::compute` is the only constructor, so construction _is_ validation |
| `page_spec` | Bounds-checked page-spec expansion.  **Always use `page_spec::expand`, never `medpdf::parse_page_spec` directly** |
| `paths` | Path-contract checks: caller-asserted inputs must exist; the output directory must exist and is never created |

### Contract Invariants (do not regress)

- **Never silently drop a requested page.**  `medpdf::parse_page_spec` _filters_ pages beyond the document, which is why `pdf-maker -o out.pdf two.pdf "1,99"` once produced a 1-page PDF at exit 0 — a plausible-looking output that was not what the caller asked for.  `page_spec::expand` wraps it and makes an out-of-range page a **tool error (exit 1)** naming the page and the document’s real page count.  A page the caller named but the document lacks is a caller-claim/world mismatch, exactly like a nonexistent input path, so it is 1 and not clap’s 2 (`~/.claude/rules/cli-exit-codes.md` § Input and Output Paths).  Any new consumer of a page spec must route through `page_spec::expand`.
- **A page spec is a list, not a set.**  Duplicates are emitted, in the order written (`"1,1"` is two pages) — and this must not weaken the bounds check above: `"1,1,99"` on a two-page document is still exit 1 naming 99.  Repetition legal, out-of-range not, and orthogonal.  Both halves live in medpdf ≥ 0.15: its plan-0006 stopped the parser collapsing repeats, and its bug-0040 (filed from here) stopped `copy_page_with_cache` returning one page object for a repeated page — which would put one object into `/Kids` twice, so that a per-page edit to the second copy also hit the first.  **The floor of `medpdf = "0.15"` is load-bearing for correctness, not just for the feature**; a downgrade reopens a malformed page tree rather than merely losing duplicates.
- **Never silently substitute an unrepresentable character.**  Before medpdf v0.11.0 every character outside WinAnsi (CP1252) — the Hawaiian ‘okina, kahakō vowels, Polish, Czech, IPA, arrows — was drawn as `?` while the run printed “Operation successful!”, so Hawaiian titles were unrepresentable and the corruption was invisible.  Watermark text now takes one of two paths, chosen per call: an **embedded** font (system name or file path) switches to a Type0/CIDFontType2 composite font with Identity-H encoding and a ToUnicode CMap, so the text renders _and_ extracts round-trip; a **built-in Standard-14** font (`@Helvetica`, `@Courier`, `@Times`) is structurally WinAnsi-bound and therefore **fails loudly** — exit 1 naming the offending characters and the font, never a `?`.  A missing glyph in an embedded font fails the same way.  The encoding lives in medpdf (`pdf_font_composite.rs`), so pdf-maker inherits this by depending on it: a medpdf downgrade below 0.11.0 would silently reopen the corruption.  medpdf exposes an opt-out (`WatermarkParams::lossy_text`) that pdf-maker deliberately does not surface yet — see `plans/plan-0001-lossy-text-fallback.md`.
- **Never create a directory.**  Only the terminal output file is written — pdf-maker offers no create mode at all (`paths.rs`).  A missing output directory is exit 1 naming it.
- **`--dry-run` writes no file**, and branches at exactly one place: the save (`run()`, near the end).  Merge, imposition, overlays, drawing, subsetting, padding and every encryption parameter — `parse_permissions` included — all run first, against the real document.  Say “runs the full **pipeline** and skips only the save”, not “full validation pass”: what a dry run cannot exercise is `save_document` itself — compression, the application of encryption, the file write — which is the failure class the `lopdf_save_modern_bug.rs` sentinel guards.  **Keep it a single branch.**  A dry run and a real run agree about geometry by construction only because there is no second code path; the sibling pdf-orchestrator simulates against a substituted page size and carries five parity bugs for it.
- **Report derived geometry the caller never stated.**  `--nup`/`--booklet` emit `imposition_geometry` (cell size, bleed overhang) and `--tile` emits `tile` (sheet count, sheet size, per-page grids) in `--json`, plus the same numbers on stderr.  A typo’d margin and a deliberate bleed are both legal; this is the only place the difference is visible before it reaches paper.
- **An output size that is _derived_ needs a ceiling.**  `--tile` is the first such operation — a units mistake turns one page into hundreds of sheets — hence `max_sheets` (default 400), refused at exit 1 rather than printed.
- **stdout belongs to `--json`.**  All human progress output goes to stderr.

### CLI Usage

```bash
pdf-maker -o out.pdf in1.pdf "1-3" in2.pdf "all" \
  --watermark "text=DRAFT,font=@Helvetica,size=24,x=1,y=1,units=in,color=#FF0000,alpha=0.5,rotation=45,h_align=center,pages=all" \
  --overlay "file=overlay.pdf,src_page=1,target_pages=1-5" \
  --pad-to 4

# Imposition (mutually exclusive)
pdf-maker -o handout.pdf slides.pdf all --nup "n=4,paper=letter,margin=0.5,border=true"
pdf-maker -o booklet.pdf program.pdf all --booklet "paper=letter,flip=long_edge,back=1"
pdf-maker -o banner-tiled.pdf banner.pdf all --tile "paper_w=11,paper_h=17,units=in,marks=labels"
```

The full key tables live in `--help` (`NUP_HELP`, `BOOKLET_HELP`, `TILE_HELP` in `main.rs`) and in README.md; both are drift-guarded against the parser’s key constants, so add a key and the build fails until it is documented.
