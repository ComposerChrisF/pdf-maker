# Plan: Expose medpdf’s lossy-text fallback as `--lossy-text`

## Problem

Since medpdf v0.11.0, watermark text outside WinAnsi (CP1252) either renders correctly through a Type0 composite font (embedded fonts) or **fails the whole run** with exit 1 (built-in Standard-14 fonts, and embedded fonts missing a glyph).  Failing loudly was the right default — it replaced silent `?` substitution that made Hawaiian titles corrupt at exit 0 — and the contract is recorded in `CLAUDE.md` § Contract Invariants.

But medpdf also ships the escape hatch, and pdf-maker does not surface it.  `WatermarkParams::lossy_text` (public, `types.rs`, default `false`) restores best-effort substitution: `.notdef` on the composite path, `?` on the WinAnsi path, each with a logged warning naming the character.  pdf-maker never sets it, so there is no way to reach it from the CLI.

That leaves the error message writing a cheque the CLI cannot cash:

```
Error: text contains character(s) not representable with font 'Helvetica': U+02BB 'ʻ', U+0101 'ā', U+0113 'ē'. Use an embedded system font that includes these glyphs, or enable lossy text substitution.
```

“Or enable lossy text substitution” names no flag, because none exists.  A user who wants that behavior — a batch job where one stray character must not abort a hundred-page run, or a draft stamp where a mangled glyph beats no output — has no move but to change fonts.

## Proposed Change

A global `--lossy-text` flag, off by default, passed through to `WatermarkParams::lossy_text` for every watermark in the run:

```bash
pdf-maker -o out.pdf in.pdf all \
  --lossy-text \
  --watermark "text=La‘i,font=@Helvetica,size=24,x=1,y=5,units=in"
# stderr: warning: 'ʻ' (U+02BB) is not representable with font 'Helvetica'; substituted '?'
# exit 0
```

Contract:

- **Default stays `false`.**  The loud failure is the safe behavior and must remain what an unflagged invocation does.
- **Substitution is never silent.**  Each substituted character is named on stderr with its codepoint and the font — one warning per distinct character, not per occurrence.
- **stdout stays clean** for `--json`; the warnings go to stderr like all other progress output.
- `--json` gains a `lossy_substitutions` array (character, codepoint, font, count) so an orchestrator can gate on it mechanically instead of scraping stderr.  Empty or absent when the flag is off.
- **Exit code stays 0.**  This is the documented opt-in to degraded output, not a finding — and pdf-maker has no findings concept.

Open question for Chris: global flag, or a per-watermark `lossy=true` key in the `--watermark` spec?  Global is simpler and matches how the medpdf parameter is actually consumed; per-spec is finer-grained but invites a run where one watermark degrades silently while another aborts, which is harder to reason about.  Recommendation: **global**, and revisit only if a real case wants the mix.

## Implementation Notes

- `src/main.rs`: one `#[arg(long)] lossy_text: bool`, threaded into each `WatermarkParams` builder call via `.lossy_text(args.lossy_text)`.  No medpdf change needed — the parameter already exists and is honored on both encoding paths (`pdf_watermark.rs:593`, `:664`, `:682`).
- The warnings are emitted by medpdf through `log::warn!`, so they surface only if pdf-maker’s logger is wired to show warnings.  **Verify that before assuming the “never silent” clause holds** — if the log level swallows them, substitution becomes exactly the silent corruption this flag is meant to be an informed opt-out from, and the flag must collect and print them itself.
- The `--json` field needs medpdf to _return_ what it substituted, not just log it.  If that is more than a small change, ship the flag with stderr warnings first and file the JSON field separately rather than blocking the flag on it.
- Update `--help`, the README § Unicode text block (which currently quotes the unactionable error message), and the `CLAUDE.md` invariant’s closing sentence, which points here.
- Test: a built-in font plus non-WinAnsi text exits 1 without the flag and 0 with it, and the warning names the character.  Assert on both directions — a test that only checks the flag path would pass with the default silently flipped.

## Why Not a Workaround

The workaround today is “use a different font”, which is not always available: `@Helvetica` is the only font guaranteed present without embedding, and embedding a full composite face for a one-character stamp is a real size cost.  Nothing outside the tool can help — the substitution decision happens inside medpdf’s text encoder, mid-run, and no pre- or post-processing step can reach it.  Sanitizing the text before invocation is strictly worse: it moves the corruption upstream and loses the warning that says which characters were lost.
