# Bug Report: `--watermark` `\n`/`\t` escapes do not render as multiple lines

**Severity:** Medium — a documented feature (`\n`/`\t` text escapes) silently does not work
**Component:** `pdf-maker` — `--watermark text=…` (via `medpdf::add_text_params`); escape decoding in `src/spec_types/parse.rs::unescape_text`
**Category:** Documented behavior does not function; the fix location is a joint pdf-maker / medpdf decision.
**Filed:** 2026-07-23 (surfaced during medpdf bug-0032)

## Description

`pdf-maker --help` and the portfolio `pdf-tools.md` document watermark text escapes
including `\n` → newline and `\t` → tab.  `unescape_text` correctly decodes them to literal
control characters, but **medpdf renders a single line**: `add_text_params` does not
interpret `\n`, `\t`, or any control character — there is no line-splitting anywhere in the
watermark path, and pdf-maker calls `add_text_params` once per page with the whole string.

The result today, in medpdf:

- **WinAnsi (simple) path** — the control byte is emitted into the literal string at an
  undefined WinAnsiEncoding position, so it renders as **nothing**.  `text=Line1\nLine2`
  draws `Line1Line2` on one line, no break, no gap.
- **Composite (Type0) path** (text also contains non-WinAnsi characters, e.g. a Hawaiian
  ‘okina) — the control character has no glyph, so medpdf returns
  `UnrepresentableText` — a **hard error**.

So a user’s `--watermark "text=Line 1\nLine 2,…"` either silently collapses to one line or
errors out, depending on the rest of the text.  As of medpdf v0.11.14, medpdf at least
**warns** (naming the control characters) when text contains any, so the cause is now
discoverable in logs — but the feature still does not work.

## Reproduction

```
pdf-maker -o out.pdf in.pdf "1" \
  --watermark "text=Line 1\nLine 2,font=@Helvetica,x=1,y=5,units=in"
```

Expected (documented): two stacked lines.  Actual: one line reading `Line 1Line 2`
(the `\n` byte is dropped), plus a medpdf control-character warning on stderr.

## Where the fix should live (joint decision)

medpdf owns the font metrics (leading, ascent/descent, block height) needed to lay out
lines with correct spacing and vertical alignment, so the natural home for real multi-line
support is **medpdf**, not a pdf-maker-side `\n`-split that would re-derive those metrics.

The full analysis — including whether medpdf needs an API change (short answer: **no** for
basic `\n`-split with metrics-based leading and block alignment; **yes** for
caller-controlled leading, word-wrap, and truncation, which add `AddTextParams` fields) —
lives in:

**`~/Chris/App/Rust/Pdf/medpdf/feature-plan-multiline-watermark-text.md`**

Recommended path: implement Tier 1 (`\n`-split, no API change) in medpdf, after which
pdf-maker’s existing `\n` escape works with **no pdf-maker change**.  Until then, pdf-maker
options are: (a) wait for medpdf Tier 1; (b) document that `\n`/`\t` are not yet rendered;
or (c) a pdf-maker-side stopgap that splits on `\n` and calls `add_text_params` per line
with fixed leading (worse vertical alignment, duplicates metric logic — discouraged).

## Suggested fix (pdf-maker side, if not deferred to medpdf)

If medpdf Tier 1 lands, close this by verifying the escape renders multiple lines.  If
pdf-maker must ship first, at minimum update `--help` to state that `\n`/`\t` in watermark
text are not currently rendered as line breaks, so the documentation stops promising a
feature that does nothing.
