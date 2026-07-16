# bug-0008: `--nup n=` silently rounds up to a full grid — `n=3` produces 4-up output

**Severity:** Medium-High (silent wrong output: the caller asked for 3 pages per sheet and got 4)
**Type:** **Decision required from Chris** — the spec never defines what non-standard `n` values mean, so this is a spec gap first and a code change second.  An agent must not “just fix the code” here: erroring, honoring, and documenting are all coherent resolutions, and they produce different behavior.

## Description

`NupSpec::from_str` maps `n=` through `auto_grid` (`src/spec_types/layout.rs`), which returns a (cols, rows) grid — `auto_grid(3)` → 2×2, `auto_grid(5)` → 3×2, `auto_grid(7)` → 3×3.  `apply_nup` then fills **every** cell of each sheet (`cells_per_sheet = cols * rows`), so `n` stops meaning “input pages per output sheet” (which is exactly what `--help` says it means) the moment it is not a product of the chosen grid: `n=3` yields 4 pages per sheet, `n=5` yields 6, `n=7` yields 9.  Exit 0, no warning.

A related design choice worth ruling on at the same time: `auto_grid(2)` is (1, 2) — two pages **stacked** on a portrait sheet — while the canonical “2-up” of every print dialog is two pages **side by side** on a landscape sheet.  The stacked layout wastes scale (0.50 vs. 0.65 for letter-size sources) and will surprise anyone expecting standard 2-up.  Same function, same decision area.

## Reproduction (verified 2026-07-16, v0.13.1)

```bash
pdf-maker -o four.pdf --blank-page "w=612,h=792,count=4"
pdf-maker -o out.pdf four.pdf all --nup "n=3" --json    # exit 0, page_count 1
```

One sheet containing all four pages; honoring `n=3` would give 2 sheets (3 + 1).  For the 2-up note: `--nup "n=2"` yields sheets with MediaBox `[0 0 612 792]` (portrait, stacked).

Rust test sketch: run with `n=3` against a 4-page input and assert on the JSON `page_count` — currently 1; the correct value depends on the ruling below.

## Resolution options (Chris to pick)

1. **Restrict `n` to the values with a canonical grid** — `1, 2, 4, 6, 8, 9, 16` — and error otherwise, telling the caller to use `cols=`/`rows=` for anything else.  Recommended: arbitrary `n` has no standard layout, and the explicit grid syntax already exists for full control.
2. **Honor arbitrary `n`** by leaving `cols*rows − n` cells blank on every sheet.  Faithful to the flag’s wording, but produces odd-looking sheets and still requires choosing which cells stay blank.
3. **Document the rounding** (spec-only fix): define `n` as “a grid of at least n cells, all of which are filled”.  Cheapest, but it leaves `--help`’s current wording wrong and the n=3→4-up surprise in place.

And separately: keep or change `auto_grid(2)`’s stacked-portrait layout (changing to side-by-side landscape alters existing output; it is also the layout every other tool produces).

## Why the fix addresses the bug

The defect is a silent gap between the flag’s stated meaning and its behavior.  Option 1 closes it by refusing the undefined inputs loudly; option 2 closes it by making behavior match the words; option 3 closes it by making the words match behavior.  What is not acceptable is the current state, where the words say one thing and the tool silently does another.

## History

`56f7b01` (v0.10.0) introduced `auto_grid` with the fallback branch for arbitrary `n`; nothing in the commit or the (nonexistent — bug-0012) docs defines the fallback’s semantics.
