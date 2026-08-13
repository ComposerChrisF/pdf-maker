# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
cargo build --release    # Build optimized binary
cargo check              # Fast type checking
cargo test               # Run all tests
```

Never commit changes to git without permission from the user.

## Repo Structure

This is a standalone crate (no workspace).  It depends on `medpdf` and `medpdf-image`, sibling crates developed in the adjacent `../medpdf` workspace and consumed here as `version + path` dependencies: local builds compile the path checkout, while the version requirement governs a crates.io publish.  See `PUBLISHING.md` (and `../medpdf/PUBLISHING.md`) for how the family is released.

```
pdf-maker/                    # Repository root
├── Cargo.toml                 # Crate manifest
├── src/
│   ├── main.rs                # CLI args (clap), orchestrates pipeline
│   ├── imposition.rs          # N-up and booklet layout
│   ├── page_spec.rs           # Bounds-checked page-spec expansion (see below)
│   ├── paths.rs               # Input/output path-contract checks
│   └── spec_types.rs          # CLI spec types with FromStr (WatermarkSpec, etc.)
└── tests/
    ├── cli_tests.rs              # CLI integration tests
    └── lopdf_save_modern_bug.rs  # Sentinel test for lopdf encryption bug
```

### Dependencies

- `medpdf` - Medium-level PDF library
- `medpdf-image` - Image embedding companion crate

## Architecture

**pdf-maker** is a CLI tool that uses medpdf for merging, overlaying, and watermarking PDFs.

### 5-Phase Processing Pipeline (src/main.rs)

1. **Merge Pages** - Parse input file/page specs, load documents, copy selected pages
2. **Apply Overlays** - Overlay content from other PDFs with resource renaming
3. **Apply Drawing Commands** - Add watermarks, rectangles, lines, and images (under layer first, then over)
4. **Padding** - Pad document to multiple of N pages
5. **Save** - Compress and write output

### Module Responsibilities

| Module | Purpose |
|--------|---------|
| `main` | CLI args (clap), orchestrates pipeline |
| `spec_types` | CLI spec types with FromStr for clap integration: `WatermarkSpec`, `OverlaySpec`, `PadToSpec`, `PadFileSpec`, `DrawRectSpec`, `DrawLineSpec`, `DrawImageSpec`, `BlankPageSpec`, `NupSpec`, `BookletSpec` |
| `imposition` | N-up and booklet layout engine: page placement, scaling, and sheet generation |
| `page_spec` | Bounds-checked page-spec expansion.  **Always use `page_spec::expand`, never `medpdf::parse_page_spec` directly** |
| `paths` | Path-contract checks: caller-asserted inputs must exist; the output directory must exist and is never created |

### Contract Invariants (do not regress)

- **Never silently drop a requested page.**  `medpdf::parse_page_spec` _filters_ pages beyond the document, which is why `pdf-maker -o out.pdf two.pdf "1,99"` once produced a 1-page PDF at exit 0 — a plausible-looking output that was not what the caller asked for.  `page_spec::expand` wraps it and makes an out-of-range page a **tool error (exit 1)** naming the page and the document’s real page count.  A page the caller named but the document lacks is a caller-claim/world mismatch, exactly like a nonexistent input path, so it is 1 and not clap’s 2 (`~/.claude/rules/cli-exit-codes.md` § Input and Output Paths).  Any new consumer of a page spec must route through `page_spec::expand`.
- **Never silently substitute an unrepresentable character.**  Before medpdf v0.11.0 every character outside WinAnsi (CP1252) — the Hawaiian ‘okina, kahakō vowels, Polish, Czech, IPA, arrows — was drawn as `?` while the run printed “Operation successful!”, so Hawaiian titles were unrepresentable and the corruption was invisible.  Watermark text now takes one of two paths, chosen per call: an **embedded** font (system name or file path) switches to a Type0/CIDFontType2 composite font with Identity-H encoding and a ToUnicode CMap, so the text renders _and_ extracts round-trip; a **built-in Standard-14** font (`@Helvetica`, `@Courier`, `@Times`) is structurally WinAnsi-bound and therefore **fails loudly** — exit 1 naming the offending characters and the font, never a `?`.  A missing glyph in an embedded font fails the same way.  The encoding lives in medpdf (`pdf_font_composite.rs`), so pdf-maker inherits this by depending on it: a medpdf downgrade below 0.11.0 would silently reopen the corruption.  medpdf exposes an opt-out (`WatermarkParams::lossy_text`) that pdf-maker deliberately does not surface yet — see `plans/plan-0001-lossy-text-fallback.md`.
- **Never create a directory.**  Only the terminal output file is written (`--create-destination=none`).  A missing output directory is exit 1 naming it.
- **`--dry-run` writes no file**, and still performs the full validation pass.
- **stdout belongs to `--json`.**  All human progress output goes to stderr.

### CLI Usage

```bash
pdf-maker -o out.pdf in1.pdf "1-3" in2.pdf "all" \
  --watermark "text=DRAFT,font=@Helvetica,size=24,x=1,y=1,units=in,color=#FF0000,alpha=0.5,rotation=45,h_align=center,pages=all" \
  --overlay "file=overlay.pdf,src_page=1,target_pages=1-5" \
  --pad-to 4
```
