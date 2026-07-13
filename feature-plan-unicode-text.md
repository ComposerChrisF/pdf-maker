# Feature Plan: Unicode Text in Watermarks (Beyond WinAnsi)

> **Scope update (2026-07-10):** the root is **medpdf** — pdf-orchestrator’s `<AddText>` exhibits the byte-identical failure (verified: `La?i`, kahakō → `?`, embedded TrueType at `WinAnsiEncoding, CharRange: 32-255`), and both tools depend on the shared `medpdf` crate.  Implement this plan IN medpdf; pdf-maker and pdf-orchestrator inherit the fix.  Until it lands, no Hawaiian (‘okina OR kahakō) can pass through any `AddText`/watermark overlay in the portfolio.

## Problem

All text pdf-maker draws (`--watermark`) is encoded single-byte WinAnsi (CP1252), even when the font is an embedded system TTF that carries the needed glyphs.  Verified 2026-07-09:

- `--watermark 'text=Laʻi …'` (U+02BB, the canonical ‘okina) → literal `?` in the output PDF, with both `@Helvetica` (Type1, not embedded) and `Helvetica Neue` (embedded TrueType, `WinAnsiEncoding, CharRange: 32-255`).
- Kahakō vowels `ā ē ī ō ū` (Latin Extended-A) → `? ? ? ? ?` — so Hawaiian titles are unrepresentable today, full stop.
- Every case prints “Operation successful!” — **silent corruption**, contrary to the fail-loudly portfolio standard.

Any character outside CP1252 is affected (Hawaiian, Polish, Czech, IPA, arrows, …).  Chris’s immediate need is Hawaiian choral titles: ‘okina and kahakō.

## Proposed CLI

No new flags for the happy path — existing `--watermark` text should simply render full Unicode when the font supports it.  Behavior changes:

- **Embedded TTF/OTF fonts**: when text contains any non-WinAnsi character, switch that font’s embedding to a Type0 (CIDFontType2) composite font with Identity-H encoding and a ToUnicode CMap; keep the current single-byte path for pure-CP1252 text (smaller output, no regression).
- **Built-in Standard-14 fonts** (`@Helvetica`, `@Courier`, `@Times`): these are structurally WinAnsi-bound.  Non-representable text must **fail loudly** — exit 1 with a message naming the offending character(s) and suggesting a system font — never emit `?` silently.
- **Font lacks a glyph** (embedded font, char not in cmap): same loud failure naming the character and the font.
- Optional escape hatch: `--lossy-text` to restore best-effort substitution for anyone who truly wants it, with a stderr warning listing substituted characters.

## Implementation Notes

- Font embedding module: add a Type0/CIDFontType2 writer — CIDToGIDMap, `W` widths array from the font’s hmtx/cmap, ToUnicode CMap for text extraction, and glyph subsetting keyed by used codepoints (the current subsetter is byte-range-based and needs to become glyph-set-based).
- Content stream: text showing switches from single-byte strings to 2-byte GID strings under Identity-H.
- Decision point per (font, text) pair at layout time: scan text for non-CP1252 chars → choose simple vs composite embedding.  Mixed watermarks on one page may embed the same face twice (simple + composite) — acceptable, or always composite when any string needs it.
- Width/positioning code currently assumes the WinAnsi widths table; route through the font’s real advance widths for the composite path (needed for `h_align`/rotation math to stay correct).
- Text extraction (pdf-dump `--text`) must keep working — the ToUnicode CMap covers that; add a round-trip test: watermark `Laʻi ā ē ī ō ū`, extract, assert equality.
- Exit codes: unrepresentable-char failure is a tool-refusal at argument-processing time → exit 1 with the offending characters listed (consistent with the documented table; not exit 3, since nothing was inspected — the tool declined the job).

## Why Not Python

Text rendering is core pdf-maker capability; a Python post-processor cannot retrofit a CID font into an already-written content stream without reimplementing the font pipeline.  Portfolio rule: feature gaps get plans in the tool’s repo, never one-off scripts.  This also unblocks the ‘okina-codepoint decision documented in `~/Chris/Proj/Coding/okina-codepoint-verification.md`, and kahakō support is needed regardless of which ‘okina codepoint wins.
