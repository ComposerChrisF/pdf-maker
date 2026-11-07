# Plan: `tabloid` as a Named Paper Size

## Problem

The named paper sizes are `letter`, `a4`, and `legal` (`parse_paper_size` in `src/spec_types/parse.rs`).  Tabloid (11×17in) is the sheet most often used for imposition (a letter-size score printed as a booklet, an N-up proof sheet, banner tiles), but it has to be spelled `paper_w=…,paper_h=…`, and that spelling has a trap.

`--booklet` turns a **named** sheet to landscape, but takes custom dimensions exactly as given (`BookletSpec::from_str` in `src/spec_types/layout.rs`).  A portrait custom sheet is legal on purpose, since the `flip` table has a row for portrait sheets.  But the natural way to type tabloid is width 11, height 17, and on 6-Nov-2026 that was the first attempt at booklet-printing a 12-page letter-size score:

```
--booklet "paper_w=11,paper_h=17,units=in,flip=long_edge"
→ Booklet halves 396.0x1224.0pt on 792x1224pt sheets
```

Each letter page would have been scaled into a 5.5×17in half.  Nothing warned; only the `--dry-run` geometry line showed it.  The correct spelling is `paper_w=17,paper_h=11`, which nothing in the help text points to.

`--tile`’s help and the README already list `paper=tabloid` as the first thing a reader will try (“not a named size — use paper_w/paper_h”), and plan-0003’s draft used `paper=tabloid` in its own example before the size was left out at implementation.

## Proposed Change

Add `tabloid` to `parse_paper_size` as 11×17in (792×1224pt, portrait, like the other named sizes).  Because every spec resolves named sizes through that one function, the change reaches all four places:

- **`--booklet "paper=tabloid"`**: turned to landscape like any named size, giving 17×11in sheets with letter-size halves.  This is the case that motivated the plan.
- **`--nup "paper=tabloid"`**: `orientation` applies as usual.
- **`--tile "paper=tabloid"`**: `orientation=auto` already picks the better way round.
- **`--blank-page tabloid`**: an 11×17in portrait page.

Update every list of named sizes: the `--nup`, `--booklet`, and `--tile` help text in `src/main.rs`, the `--blank-page` help, the two error messages that enumerate the sizes (`parse.rs`, `misc.rs`), and the README’s `paper` rows.  Drop the “not a named size” example from `--tile`’s help and the README.

## Implementation Notes

- Two tests currently assert that `tabloid` is **rejected** (`parse.rs` `parse_paper_size("tabloid").is_err()`, `misc.rs` `BlankPageSpec::from_str("tabloid")`).  Turn them into acceptance tests, and keep a rejection test for some other unknown name.
- Add a booklet test: `paper=tabloid` gives a 1224×792pt sheet and 612×792pt cells.  Fail it by reverting the `parse_paper_size` arm.
- **`ledger`**: the same sheet, named for its landscape orientation.  Adding it as an alias is cheap, but it would be a second name for one size, and on `--booklet` and `--tile` the orientation is chosen by the tool, not the name.  The recommendation is `tabloid` only.
- **A3** is the ISO counterpart.  It is out of scope here and would be a separate one-line change if wanted.
- This plan does not change `--booklet`’s handling of a portrait custom sheet.  That stays legal, and the `--dry-run` geometry line remains the place it shows.

## Why Not a Workaround

The workaround is the custom spelling, and that spelling is the trap described above: two numbers whose order matters only for `--booklet`, with a plausible wrong order that runs silently.  A named size cannot be given in the wrong order.  The fix belongs in `parse_paper_size`, which every spec already shares.
