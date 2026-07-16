# bug-0005: `--help` advertises booklet keys that do not exist, and lists no keys at all for `--nup`

**Severity:** Medium — the help text actively misleads; following it verbatim produces an error.  For an agent-first CLI, `--help` is the spec surface an AI reads.
**Type:** Code bug (the help strings in `src/main.rs`).  The `--help` text _is_ the misleading spec here — fix the strings, not the parser: `flip` is the intended key name and `orientation` is intentionally not a booklet key.

## Description

Two defects in the clap help strings in `src/main.rs`:

1. The `--booklet` help says: “Spec keys: paper, orientation, duplex_flip, back, etc.”  But `BOOKLET_KEYS` in `src/spec_types/layout.rs` is `paper, paper_w, paper_h, binding_margin, units, flip, back`.  So the help names **two keys that are rejected** — `orientation` (not a booklet key at all) and `duplex_flip` (the real key is `flip`) — and hides four real ones behind “etc.”.
2. The `--nup` help lists **no spec keys whatsoever** (its whole text is `N-up imposition (multiple input pages per output sheet). Conflicts with --booklet`), leaving `n, cols, rows, paper, paper_w, paper_h, orientation, margin, gutter, units, order, border, repeat` undiscoverable from the binary.

Amusingly, the v0.10.0 commit message notes it “removes dead orientation field from NupSpec” — the help string for `--booklet` still advertises the concept the commit deleted.

## Reproduction (verified 2026-07-16, v0.13.1)

```bash
pdf-maker --help | grep -A2 booklet
# → "Spec keys: paper, orientation, duplex_flip, back, etc."
pdf-maker -o out.pdf four.pdf all --booklet "duplex_flip=short_edge"
# → error: invalid value ... Unknown booklet key: 'duplex_flip'   (exit 2)
pdf-maker -o out.pdf four.pdf all --booklet "orientation=landscape"
# → error: ... Unknown booklet key: 'orientation'   (exit 2)
```

Rust test sketch: run `pdf-maker --help`, assert the `--booklet` help mentions `flip` and `binding_margin` and does **not** mention `duplex_flip` or `orientation`; assert the `--nup` help mentions at least `n`, `cols`, `rows`, `margin`, `gutter`, `order`, `border`, `repeat`.  A stronger variant asserts every key in `BOOKLET_KEYS` / `NUP_KEYS` appears in the corresponding help string (exposing the constants to the test or duplicating the lists).

## Suggested fix

Rewrite the two help strings to enumerate the real keys:

- `--booklet`: “Spec keys: paper, paper_w, paper_h, binding_margin, units, flip (none|short_edge|long_edge), back.  Pass with no value for defaults.”
- `--nup`: “Spec keys: n or cols+rows (required); paper, paper_w, paper_h, orientation, margin, gutter, units, order (lrtb|rltb|tblr|tbrl), border, repeat.”

While there: the `--blank-page` help says the named sizes are “e.g. ‘letter’, ‘a4’” — `legal` is also accepted (`parse_paper_size`); list all three.

## Why this fix addresses the bug

The parser is correct and its error messages are good; only the advertised key list lies.  Making `--help` enumerate the exact accepted keys restores the property the portfolio rules require — that usage knowledge is discoverable from the binary — and the suggested test pins help and parser together so they cannot drift apart again.

## History

Help strings date to `56f7b01` (v0.10.0); the booklet key was evidently renamed from `duplex_flip` to `flip` during development and the help string kept the working name.  `526c924` (v0.12.3, “CLI help text” review pass) did not catch it.
