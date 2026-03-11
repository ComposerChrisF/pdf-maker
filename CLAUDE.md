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

This is a standalone crate (no workspace). It depends on the sibling `medpdf` workspace via path dependencies.

```
pdf-merger/                    # Repository root
├── Cargo.toml                 # Crate manifest (path deps to ../medpdf/)
├── src/
│   ├── main.rs                # CLI args (clap), orchestrates pipeline
│   └── spec_types.rs          # CLI spec types with FromStr (WatermarkSpec, etc.)
└── tests/
    └── spec_types_tests.rs    # Integration tests (stub)
```

### Dependencies

- `medpdf` (path: `../medpdf/medpdf`) - Medium-level PDF library
- `medpdf-image` (path: `../medpdf/medpdf-image`) - Image embedding companion crate

## Architecture

**pdf-merger** is a CLI tool that uses medpdf for merging, overlaying, and watermarking PDFs.

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
| `spec_types` | CLI spec types with FromStr for clap integration: `WatermarkSpec`, `OverlaySpec`, `PadToSpec`, `PadFileSpec`, `DrawRectSpec`, `DrawLineSpec`, `DrawImageSpec`, `BlankPageSpec` |

### CLI Usage

```bash
pdf-merger -o out.pdf in1.pdf "1-3" in2.pdf "all" \
  --watermark "text=DRAFT,font=@Helvetica,size=24,x=1,y=1,units=in,color=#FF0000,alpha=0.5,rotation=45,h_align=center,pages=all" \
  --overlay "file=overlay.pdf,src_page=1,target_pages=1-5" \
  --pad-to 4
```
