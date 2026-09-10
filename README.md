# pdf-maker

A command-line tool for advanced PDF manipulation: merge pages from multiple PDFs, impose them for booklet or N‑up printing, tile one oversized page across many sheets, apply overlays, draw watermarks and shapes, pad to page multiples, and encrypt the result.

## Installation

```bash
cargo install pdf-maker
```

Requires a Rust toolchain that supports edition 2024.

## Usage

```bash
pdf-maker -o <OUTPUT> <FILE> <PAGES> [<FILE> <PAGES>]... [OPTIONS]
```

### Basic Merging

Input files and page specifications come in pairs:

```bash
# Merge all pages from two PDFs
pdf-maker -o combined.pdf doc1.pdf "all" doc2.pdf "all"

# Merge specific pages
pdf-maker -o output.pdf report.pdf "1-5" appendix.pdf "2,4,6"
```

### Page Specifications

| Format | Description | Example |
|--------|-------------|---------|
| `all` | All pages | `"all"` |
| `N` | Single page | `"3"` |
| `N-M` | Page range (inclusive) | `"1-5"` |
| `N-` | From page N to end | `"10-"` |
| `-M` | From start to page M | `"-5"` |
| `N,M,P` | Specific pages | `"1,3,7"` |
| Mixed | Combine formats | `"1-3,5,8-10"` |

**A page the spec names but the document does not contain is an error** (exit 1), never a silent drop.  `"1,99"` against a 2-page PDF fails, naming page 99 and the real page count, instead of quietly producing a 1-page PDF.  The same holds for a range past the end (`"1-100"`), an open range past the end (`"5-"`), and the `pages=` target of any drawing or overlay flag.  Use `"all"` when you mean “however many there are”.

**Duplicates are collapsed, for now.**  `"1,1,2"` yields two pages, not three: a repeated page number is currently absorbed before pdf-maker sees the list.  This is a known limitation rather than a design decision — the intended behavior is to honor duplicates, which needs a change in the underlying `medpdf` page-spec API.  To repeat a page today, name the same file twice: `doc.pdf "1" doc.pdf "1"`.

### Blank Pages

`--blank-page` appends blank pages.  They are added **after every input page**, in flag order, and before imposition — so a blank page can be tiled or imposed like any other.

```bash
# One blank letter-size page
--blank-page letter

# Three blank pages of a custom size
--blank-page "w=8.5,h=11,units=in,count=3"
```

| Form | Description |
|------|-------------|
| `letter`, `a4`, `legal` | Named size, one page |
| `w=N,h=N[,units=…][,count=N]` | Custom size.  `units` is `pt` (default), `in`, `mm` or `cm`; `count` defaults to 1 |

A document may consist of nothing but blank pages; with no inputs and no `--blank-page`, pdf-maker exits 1 rather than writing an empty file.

## Options

### Output

| Option | Description |
|--------|-------------|
| `-o, --output <FILE>` | Output PDF path (required) |
| `--dry-run` | Run the whole pipeline, write no file (see below) |
| `--json` | Machine-readable summary on stdout (an error object on failure) |
| `--broad-compatibility` | Use traditional PDF format for older viewers |
| `--no-subset` | Embed full font files instead of subsetting them |

**What `--dry-run` does and does not check.**  It runs the **full pipeline** — merge, imposition, overlays, drawing commands, font subsetting, padding, and the resolution of every encryption parameter — and skips exactly one step: the save.  So it catches a bad spec, a missing input, an out-of-range page, an unrepresentable character, and a runaway tile count.  What it cannot catch is a failure inside the save itself: compression, the application of encryption, and the file write.  A document that validates under `--dry-run` can still fail to be written.

The single branch is deliberate.  Because a dry run and a real run differ at the save and nowhere else, they agree about geometry by construction — there is no second code path to drift.  Do not “optimize” `--dry-run` into a simulation.

### Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success — the output was written.  Also `--dry-run`, which writes nothing. |
| `1` | Tool error: I/O error (missing input PDF, missing output directory), malformed input PDF, encryption error, invalid spec, page index out of range, or a layout whose arithmetic does not work out (a non-positive N‑up cell, a tile overlap that would never terminate, a run over `max_sheets`). |
| `2` | Usage error: invalid command-line arguments (clap parse error). |

pdf-maker is a generator, not an analyzer: it has no findings concept, so codes 3 and 4 of the portfolio table are unused.

### Paths

Input paths (input PDFs, `--overlay file`, `--draw-image file`, `--pad-last-page-file file`) must exist; a missing one exits 1 naming it.  The output **directory** must already exist — pdf-maker writes the output file but never creates a directory.

**A path that cannot be read is not a path that is missing**, and pdf-maker says which: an input it cannot stat (permissions, a broken link, an I/O error) reports “cannot be accessed”, naming the underlying error, rather than claiming the file does not exist.  The two send you to different fixes.

### Machine-Readable Output

`--json` prints a summary object on stdout (all human progress output stays on stderr):

```json
{
  "tool": "pdf-maker",
  "version": "0.21.1",
  "output": "out.pdf",
  "written": true,
  "dry_run": false,
  "page_count": 2,
  "bytes": 1241,
  "encrypted": false,
  "imposition": "none",
  "imposition_geometry": null,
  "tile": null,
  "inputs": [{ "file": "in.pdf", "spec": "1-2", "source_page_count": 5, "pages": [1, 2] }],
  "operations": { "blank_pages": 0, "watermarks": 0, "overlays": 0, "draw_rects": 0, "draw_lines": 0, "draw_images": 0 }
}
```

`imposition` is one of `none`, `nup`, `booklet` or `tile`.  Two objects are null unless the matching mode ran:

- `imposition_geometry` (`--nup` and `--booklet`) reports the derived cell size and, for a negative margin, the deliberate bleed overhang: `{"cell_width_pt": 306.0, "cell_height_pt": 396.0, "bleed_overhang_pt": 0.0}`.
- `tile` reports the sheet count and grid: `{"sheets": 5, "sheet_width_pt": 792.0, "sheet_height_pt": 1224.0, "grids": [{"cols": 5, "rows": 1}]}`.  One grid entry per tiled source page.

Both exist because the numbers are **derived** rather than stated: the caller never typed the cell size or the sheet count, so reporting them is the only way a units mistake is visible before it reaches paper.

On failure it prints `{"error": "...", "exit_code": 1}` instead.  (A clap usage error, exit 2, is reported as plain text — clap owns that path.)

## Imposition

Three mutually exclusive modes rearrange pages onto sheets.  All three run in the same pipeline slot — **after the merge, before overlays, drawing and padding**.  That ordering has a consequence worth internalizing: once a page has been imposed, the `pages=` target of every watermark, rectangle, line, image and overlay addresses **output sheets**, not source pages, and `--pad-to` pads the sheet count.

### `--nup` — many pages on one sheet

```bash
pdf-maker -o handout.pdf slides.pdf "all" --nup "n=4,paper=letter,margin=0.5,border=true"
```

| Key | Default | Description |
|-----|---------|-------------|
| `n` | — | Pages per sheet: one of `1`, `2`, `4`, `6`, `8`, `9`, `16`.  `n=2` is two pages **side by side** on a landscape sheet.  Anything else is a usage error naming the set. |
| `cols`, `rows` | — | Explicit grid, e.g. `cols=3,rows=1`.  Mutually exclusive with `n`, and the way to express a layout `n` does not name — `cols=1,rows=2` is the stacked-portrait two-up. |
| `paper` | `letter` | Named sheet size: `letter`, `a4`, `legal` |
| `paper_w`, `paper_h` | — | Custom sheet size in `units`.  Use instead of `paper`, not alongside it. |
| `orientation` | `auto` | `portrait`, `landscape`, or `auto` (landscape when `cols` > `rows`) |
| `margin` | 0 | Blank margin around the whole sheet |
| `gutter` | 0 | Space between cells |
| `units` | `in` | `pt`, `in`, `mm`, `cm` |
| `order` | `lrtb` | Cell fill order: `lrtb`, `rltb`, `tblr`, `tbrl` |
| `border` | `false` | Draw a thin rule around each placed page |
| `repeat` | 1 | Repeat each source page N times, or `auto` to fill one whole sheet per source page |

A margin or gutter large enough to leave no room is an error (exit 1) naming the whole arithmetic, not a silently empty sheet.  A **negative** margin is legal — it is how a deliberate bleed is expressed — and the resulting overhang is reported in `--json`.

### `--booklet` — saddle-stitched signatures

Reorders pages so that printing two-sided and folding the stack in half yields a booklet.  Pass the flag with no value for all defaults.

```bash
pdf-maker -o booklet.pdf program.pdf "all" --booklet "paper=letter,flip=long_edge,back=1"
```

| Key | Default | Description |
|-----|---------|-------------|
| `paper` | `letter` | Named sheet size, **forced to landscape** |
| `paper_w`, `paper_h` | — | Custom sheet size in `units` |
| `binding_margin` | 0 | Gutter at the spine, split between the two halves |
| `units` | `in` | `pt`, `in`, `mm`, `cm` |
| `flip` | `none` | Which edge your printer flips on: `none`, `long_edge`, `short_edge` |
| `back` | 0 | Keep the last N pages at the end (a back cover): the blanks needed to reach a multiple of 4 are inserted **before** them instead of after |

**Choosing `flip` is the one thing to get right**, and it follows from the sheet orientation rather than from preference — it compensates for the physical turn the duplexer performs:

| Sheet | Use | Why |
|-------|-----|-----|
| **Landscape** (the default) | `flip=long_edge` | The long edge is horizontal, so the duplexer turns the sheet top to bottom and the backs need a 180° turn |
| **Portrait** (e.g. `paper_w=8.5,paper_h=11`) | `flip=short_edge` | Same reasoning, other axis |
| Single-sided output | `flip=none` | No compensation at all |

**If the backs print upside down, the other value is the fix.**  Confirmed by physical duplex print; any advice about `flip` from before v0.16.0 — this tool’s included — is inverted and wrong.

### `--tile` — one page across many sheets

The inverse of `--nup`, for banners and posters: split one oversized page into a grid of printable sheets, with overlap so they can be taped together.

```bash
pdf-maker -o banner-tiled.pdf banner.pdf "all" \
  --tile "paper_w=11,paper_h=17,units=in,marks=labels"
```

| Key | Default | Description |
|-----|---------|-------------|
| `paper` | `letter` | Named sheet size: `letter`, `a4`, `legal` (tabloid is not named — use `paper_w`/`paper_h`) |
| `paper_w`, `paper_h` | — | Custom sheet size in `units` |
| `orientation` | `auto` | `portrait`, `landscape`, or `auto`, which picks whichever yields **fewer sheets** |
| `overlap` | 0.75in | Overlap between adjacent tiles.  Do not set this to 0 — see below |
| `margin` | 0 | Border the printer cannot print, held clear on each sheet |
| `pages` | `all` | Which source pages to tile |
| `order` | `row` | `row` (left to right, then down) or `col` |
| `marks` | `none` | `none`, `labels`, `crop`, `both` / `labels+crop` |
| `scale` | 1.0 | Output scale, applied **before** the grid is computed.  This is not fit-to-page; the point of tiling is 1:1 output |
| `align` | `center` | Where the grid’s surplus goes: `center` spreads it over both ends, `start` leaves it all at the far edge |
| `max_sheets` | 400 | Refuse a run larger than this |
| `units` | `in` | `pt`, `in`, `mm`, `cm` |

**Why the overlap default is 0.75in and not zero.**  Consumer printers hold roughly a quarter inch at each edge that they cannot print, so the overlap you actually get when taping is `overlap − 2 × that border`.  At 0.5in that is nothing, and any drift opens a white line through the artwork.  0.75in leaves about a quarter inch of real overlap; raise it if your printer’s border is wider.

**Why `orientation=auto` is not cosmetic.**  A 48×12in banner is five tabloid sheets one way and six the other; the conference banner this feature was written for is four versus seven.

`marks=labels` captions each sheet `p1 R2C3 of 5x1`, naming the source page as well as the cell, because `pages=all` interleaves several grids into one file.  `marks=crop` draws the tile boundary for butting rather than lapping.

Two guards refuse a run rather than produce quietly wrong output, both exit 1 naming the arithmetic: an `overlap` at least as wide as the printable window (the grid would never terminate), and an overlap too small to cover a negative `margin` (the tiles would not meet, dropping a strip of artwork at every seam).  **Coverage, not sign, is the invariant** — a negative margin is legal exactly while the overlap still covers it.  `max_sheets` catches the other failure: `--tile` is the one pdf-maker operation whose output size is derived rather than stated, so a units mistake turns one page into hundreds of sheets.

## Watermarks

Add text watermarks on top of page content:

```bash
--watermark "text=DRAFT,font=@Helvetica,size=48,x=1,y=1,units=in,pages=all"
```

Add watermarks behind page content using `layer=under`:

```bash
--watermark "text=CONFIDENTIAL,font=@Courier,size=36,x=0.5,y=0.5,units=in,layer=under"
```

**Watermark parameters:**

| Parameter | Required | Default | Description |
|-----------|----------|---------|-------------|
| `text` | Yes | — | Watermark text (escapes below) |
| `font` | Yes | — | Font specification (see below) |
| `x` | Yes | — | X position, in `units` |
| `y` | Yes | — | Y position, in `units` |
| `size` | No | 48 | Font size in points |
| `units` | No | `in` | Position units: `pt`, `in`, `mm`, `cm` |
| `pages` | No | `all` | Pages to watermark (page-spec grammar; **sheets** after imposition) |
| `color` | No | `black` | Named color or hex (see below) |
| `alpha` | No | from `color` | Opacity, 0.0 (transparent) to 1.0 (opaque).  Overrides any alpha in `color` |
| `rotation` | No | 0 | Rotation in degrees |
| `h_align` | No | `left` | Horizontal anchor at `x`: `left`, `center`, `right` |
| `v_align` | No | `baseline` | Vertical anchor at `y`: `top`, `cap_top`, `center`, `baseline`, `descent_bottom`, `bottom` |
| `strikeout` | No | `false` | `true` or `false` |
| `underline` | No | `false` | `true` or `false` |
| `weight` | No | normal | `thin`, `extra_light`, `light`, `normal`, `medium`, `semi_bold`, `bold`, `extra_bold`, `black`, or a number 1–1000.  Selects among faces of an embedded family |
| `style` | No | `normal` | `normal`, `italic`, `oblique` |
| `layer` | No | `over` | `over` (on top of page content) or `under` (behind it) |

**Colors** — every drawing flag takes the same set.  Named: `black`, `white`, `red`, `blue`, `green`, `yellow`, `cyan`, `magenta`, `orange`, `purple`, `gray` (or `grey`).  Hex: `#RGB`, `#RRGGBB`, or `#RRGGBBAA` for straight-to-hex opacity; the `#` is optional.

**Text escapes** — a watermark’s `text` value passes through an unescaping step:

| Escape | Meaning |
|--------|---------|
| `\,` | A literal comma (commas otherwise separate keys) |
| `\n` | A line break |
| `\\` | A literal backslash |
| `\uXXXX` | A Unicode character, 4 hex digits (BMP) |
| `\U{XXXXX}` | A Unicode character, 1–6 hex digits (full range) |

Any other `\X` is left alone.  A value may also be double-quoted, in which case commas inside the quotes are literal.

**`\n` and `\t` are not a pair.**  `\n` renders: the text is split on newlines and each line is drawn on its own baseline, with leading taken from the embedded face (or 1.2 × the font size for a built-in), and a trailing newline yields a trailing empty line, so the block height does not depend on invisible whitespace.  `\t` does **not** render: there is no tab-stop model, and a decoded tab is dropped or refused depending on the text path.  Leading is not caller-settable, and there is no wrapping or truncation.

**Font specifications:**

- `@Helvetica` — PDF built-in font (prefix with `@`)
- `@Courier`, `@Times-Roman`, `@Symbol`, `@ZapfDingbats` — other built-ins
- `Arial` — system font name (searched via font-kit)
- `/path/to/font.ttf` — direct path to a TTF file

Built-in fonts (PDF 1.7): `Times-Roman`, `Helvetica`, `Courier`, `Symbol`, `Times-Bold`, `Helvetica-Bold`, `Courier-Bold`, `ZapfDingbats`, `Times-Italic`, `Helvetica-Oblique`, `Courier-Oblique`, `Times-BoldItalic`, `Helvetica-BoldOblique`, `Courier-BoldOblique`

**Unicode text beyond WinAnsi:**

Watermark text is not limited to Latin-1.  An **embedded** font — a system font name or a `.ttf`/`.otf` path — renders the full Unicode range its glyphs cover, via a Type0/CIDFontType2 composite font with Identity-H encoding and a ToUnicode CMap, so the text also extracts cleanly afterward:

```bash
pdf-maker -o out.pdf in.pdf all \
  --watermark "text=La‘i ā ē ī ō ū,font=/Library/Fonts/CrimsonPro-Black.ttf,size=24,x=1,y=5,units=in"

pdf-dump out.pdf --text        # → La‘i ā ē ī ō ū
```

The **built-in** fonts (`@Helvetica`, `@Courier`, `@Times`, …) are structurally bound to WinAnsi (CP1252) and cannot carry these characters.  Rather than substitute `?`, pdf-maker refuses the job — exit 1, naming every offending character and the font:

```
Error: text contains character(s) not representable with font 'Helvetica': U+02BB 'ʻ', U+0101 'ā', U+0113 'ē'. Use an embedded system font that includes these glyphs, or enable lossy text substitution.
```

An embedded font that lacks a needed glyph fails the same way.  Two current limits: a `.ttc`/`.otc` font _collection_ cannot be embedded (supply a single-face `.ttf`/`.otf`), and composite fonts are embedded in full — `--no-subset` is moot for them, so a Unicode watermark carries the whole face.

## Drawing

Three flags draw directly onto pages.  All of them share `color`, `alpha`, `pages`, `units` and `layer` with `--watermark`, with one difference worth noting: **`units` defaults to `pt` here**, where `--watermark` defaults to `in`.  Each flag may be repeated.

### `--draw-rect`

```bash
--draw-rect "x=1,y=1,w=3,h=2,units=in,color=#FF0000,alpha=0.3,layer=under"
```

| Parameter | Required | Default | Description |
|-----------|----------|---------|-------------|
| `x`, `y` | Yes | — | Lower-left corner, in `units` |
| `w`, `h` | Yes | — | Width and height, in `units` (must be positive) |
| `color` | No | `black` | Fill color |
| `alpha` | No | from `color` | Opacity 0.0–1.0 |
| `pages` | No | `all` | Which pages |
| `units` | No | `pt` | `pt`, `in`, `mm`, `cm` |
| `layer` | No | `over` | `over` or `under` |

### `--draw-line`

```bash
--draw-line "x1=0.5,y1=0.5,x2=8,y2=0.5,units=in,width=0.02,color=gray"
```

| Parameter | Required | Default | Description |
|-----------|----------|---------|-------------|
| `x1`, `y1`, `x2`, `y2` | Yes | — | Endpoints, in `units` |
| `width` | No | 1 | Line width, in `units` — `units` governs **every** distance in the spec, coordinates and width alike.  (Before v0.21.3 the width alone was always points, which made `units=in` mean two different things in one spec.) |
| `color` | No | `black` | Stroke color |
| `alpha` | No | from `color` | Opacity 0.0–1.0 |
| `pages` | No | `all` | Which pages |
| `units` | No | `pt` | `pt`, `in`, `mm`, `cm` |
| `layer` | No | `over` | `over` or `under` |

### `--draw-image`

```bash
--draw-image "file=logo.png,x=0.5,y=9,w=2,units=in,fit=contain,max_dpi=300"
```

| Parameter | Required | Default | Description |
|-----------|----------|---------|-------------|
| `file` | Yes | — | Image file to embed (must exist) |
| `x`, `y` | Yes | — | Lower-left corner, in `units` |
| `w`, `h` | At least one | — | Box width and/or height, in `units`.  Supplying only one derives the other from the image’s aspect |
| `fit` | No | `contain` | `contain`, `cover`, `stretch` |
| `max_dpi` | No | 300 | Downsample above this resolution, or `none` for no downsampling at all.  A number below 1 is an error — `none` is the spelling for “no limit”, so no caller has to know that a sentinel exists |
| `rotation` | No | 0 | Rotation in degrees |
| `alpha` | No | 1.0 | Opacity 0.0–1.0 |
| `pages` | No | `all` | Which pages |
| `units` | No | `pt` | `pt`, `in`, `mm`, `cm` |
| `layer` | No | `over` | `over` or `under` |

## Overlays

Overlay content from another PDF onto pages:

```bash
--overlay "file=letterhead.pdf,src_page=1,target_pages=all"
```

**Overlay parameters:**

| Parameter | Required | Default | Description |
|-----------|----------|---------|-------------|
| `file` | Yes | — | Source PDF file |
| `src_page` | Yes | — | Page number from source PDF to overlay (1-based; `0` is a usage error) |
| `target_pages` | No | `all` | Destination pages to apply overlay |

An out-of-range `src_page` is an error naming the flag, the file, the page and the file’s real page count — the same contract as any other caller-named page.

## Padding

Pad the document to a multiple of N pages (useful for booklet printing).  Padding runs **last**, after imposition, so with `--nup`, `--booklet` or `--tile` it pads the sheet count.

```bash
--pad-to 4
```

Optionally use a specific page for the last padding page:

```bash
--pad-to 4 --pad-last-page-file "file=back-cover.pdf,page=1"
```

**Pad file parameters:**

| Parameter | Required | Default | Description |
|-----------|----------|---------|-------------|
| `file` | Yes | — | PDF file for last padding page |
| `page` | No | 1 | Page number to use from file (1-based; `0` is a usage error) |

`--pad-last-page-file` **requires `--pad-to`** — alone it has no meaning, so it is refused as a usage error (exit 2) rather than silently ignored.  Its `page` is validated up front, whether or not the document actually needs padding: an argument’s validity should not depend on how much work it happens to cause.

## Encryption

```bash
pdf-maker -o secure.pdf report.pdf "all" \
  --user-password "viewpassword" \
  --owner-password "editpassword" \
  --encryption-algorithm aes256 \
  --permissions "print,copy"
```

| Option | Default | Description |
|--------|---------|-------------|
| `--user-password <PW>` | — | Password required to open the document |
| `--owner-password <PW>` | the user password | Password required to change permissions and restrictions |
| `--encryption-algorithm <ALG>` | `aes128` | `aes256`, `aes128`, or `rc4` |
| `--permissions <LIST>` | everything allowed | Comma-separated: `print`, `modify`, `copy`, `annotate`, `fill`, `accessibility`, `assemble`, `print_hq`, plus `all` and `none` |

**`--encryption-algorithm` and `--permissions` require a password.**  Asking to restrict a document without one is refused as a usage error (exit 2), not quietly ignored — an unencrypted PDF carries no permissions to enforce, so accepting the flags would have written an unprotected file while reporting success.

## Examples

### Merge with page selection

```bash
pdf-maker -o report.pdf \
  cover.pdf "1" \
  content.pdf "all" \
  appendix.pdf "1-3,7"
```

### Add watermark to all pages

```bash
pdf-maker -o draft.pdf document.pdf "all" \
  --watermark "text=DRAFT,font=@Helvetica-Bold,size=72,x=2,y=5,units=in"
```

### Apply letterhead overlay

```bash
pdf-maker -o branded.pdf document.pdf "all" \
  --overlay "file=letterhead.pdf,src_page=1,target_pages=all"
```

### Four slides to a page

```bash
pdf-maker -o handout.pdf slides.pdf "all" \
  --nup "n=4,paper=letter,margin=0.5,gutter=0.25,border=true"
```

### Print a saddle-stitched program

```bash
pdf-maker -o booklet.pdf program.pdf "all" \
  --booklet "paper=letter,flip=long_edge,binding_margin=0.25,back=1"
```

### Tile a 48×12in banner onto tabloid sheets

```bash
pdf-maker -o banner-tiled.pdf banner.pdf "all" \
  --tile "paper_w=11,paper_h=17,units=in,overlap=0.75,marks=labels"
```

Four sheets, landscape, each captioned with its cell — check the count with `--dry-run --json` before committing paper to it.

### Complex workflow

```bash
pdf-maker -o final.pdf \
  intro.pdf "1-2" \
  main.pdf "all" \
  appendix.pdf "5-" \
  --overlay "file=template.pdf,src_page=1,target_pages=1" \
  --watermark "text=v1.0,font=@Courier,size=12,x=0.5,y=0.25,units=in,pages=1" \
  --watermark "text=CONFIDENTIAL,font=@Helvetica,size=48,x=3,y=5,units=in,pages=2-,layer=under" \
  --pad-to 4
```

## Processing Pipeline

The tool processes PDFs in seven phases, in this order:

1. **Merge** — copy selected pages from input files, then append `--blank-page` pages
2. **Impose** — `--nup`, `--booklet` or `--tile` (at most one), rearranging pages onto sheets
3. **Overlay** — apply PDF overlays with resource deduplication
4. **Draw** — watermarks, rectangles, lines and images (the `under` layer first, then `over`)
5. **Subset** — subset embedded fonts, unless `--no-subset`
6. **Pad** — add blank pages to reach the `--pad-to` multiple
7. **Save** — compress, encrypt, and write the output (skipped by `--dry-run`)

Phase 2 is why `pages=` targets in phases 3 and 4, and the count in phase 6, refer to **output sheets** rather than source pages whenever an imposition mode is active.

## Acknowledgments

Built on [medpdf](https://github.com/ComposerChrisF/medpdf), a medium-level PDF API over the excellent [lopdf](https://github.com/J-F-Liu/lopdf) rust crate.

## Related Projects

- [pdf-dump](https://github.com/ComposerChrisF/pdf-dump) — CLI tool for inspecting and debugging PDF internals.  Essential for debugging medpdf and pdf-maker
- [medpdf](https://github.com/ComposerChrisF/medpdf) — Medium-level PDF Rust API (includes medpdf-image for image embedding) as a higher-level abstraction over lopdf

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
