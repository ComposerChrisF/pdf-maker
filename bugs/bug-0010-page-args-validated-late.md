# bug-0010: `--overlay src_page=` and `--pad-last-page-file page=` are validated late (or never), with errors that name nothing

**Severity:** Medium
**Type:** Code bug.  The spec is not the problem — README, CLAUDE.md, and `--help` all promise that a page the caller names but the document lacks errors “naming the page and the document’s real page count”, and these two arguments fall short of that promise.

## Description

Three related defects, all stemming from these page numbers bypassing `page_spec::expand` and its up-front discipline:

1. **`--pad-last-page-file page=N` beyond the pad file is only checked when padding actually occurs.**  `apply_padding` (`src/main.rs`) loads the pad file and calls `medpdf::copy_page(doc, &pad_doc, spec.page)` only inside the `pages_to_add > 0` branch.  When the document already sits at the pad multiple, an out-of-range `page=99` passes silently (exit 0) — a latent invalid argument that will detonate on a different input length later.  This also weakens the `--dry-run` promise of a “full validation pass”.
2. **When the check does fire, the error names nothing.**  Both `copy_page` and `overlay_page` funnel into medpdf’s `get_page_object_id_from_doc`, whose message is `Page 99 not found in source document` — no flag, no file name, no real page count.  Compare the `page_spec::expand` message, which names all three.  With several PDFs in one invocation, the caller cannot tell which file or flag is at fault.
3. **`src_page=0` and `page=0` are accepted at parse** (`u32` in `src/spec_types/misc.rs`) and only fail at apply time with the same anonymous message.  Page numbers are 1-based; zero is rejectable at parse for free.

## Reproduction (verified 2026-07-16, v0.13.1)

```bash
pdf-maker -o two.pdf  --blank-page "w=612,h=792,count=2"
pdf-maker -o four.pdf --blank-page "w=612,h=792,count=4"
pdf-maker -o one.pdf  --blank-page letter

# (1) latent: 4 pages is already a multiple of 4 — page=99 never checked
pdf-maker -o out.pdf four.pdf all --pad-to 4 --pad-last-page-file "file=one.pdf,page=99"   # exit 0

# (2) anonymous error when padding is needed
pdf-maker -o out.pdf two.pdf all --pad-to 4 --pad-last-page-file "file=one.pdf,page=99"
# → Error: Page 99 not found in source document        (exit 1)

# (2/3) overlay: same anonymous message; 0 accepted at parse
pdf-maker -o out.pdf two.pdf all --overlay "file=one.pdf,src_page=99"   # same message
pdf-maker -o out.pdf two.pdf all --overlay "file=one.pdf,src_page=0"    # Page 0 not found...
```

Rust test sketches: (a) the latent case — currently exit 0, should exit 1 naming `--pad-last-page-file`, the file, page 99, and the real count 1; (b) assert the overlay error message contains the file path and `1 page(s)`; (c) `OverlaySpec::from_str("file=f.pdf,src_page=0")` and `PadFileSpec::from_str("file=f.pdf,page=0")` should be parse errors.

## Suggested fix

- **Parse:** reject `0` for `src_page` and `page` in `src/spec_types/misc.rs` (clap then reports it, exit 2), matching the existing `count=0`/`repeat=0` precedent.
- **Validate up-front, unconditionally:** when `--pad-last-page-file` is given (with `--pad-to` — see bug-0011), load the pad file once during the existing check phase (or at the start of the run) and verify `spec.page <= page_count`, erroring in the `page_spec::expand` message style: flag, file, page, real count.  Same for each `--overlay`: after `Document::load`, check `src_page` against the overlay’s page count before entering the per-target loop, with the same message shape.  (The overlay file loads at apply time anyway; the point is the named, counted error — and that it fires even when `target_pages` is empty of work.)

## Why this fix addresses the bug

It brings the last two caller-named page numbers under the same contract the v0.13.0 work (`425b9f6`) established for page specs: every mismatch between what the invocation asserts and what the document contains is loud, immediate, and self-describing.  Rejecting zero at parse removes an entire error class before any I/O, and the unconditional check eliminates the latent-argument case that currently depends on the input’s page count modulo `--pad-to`.
