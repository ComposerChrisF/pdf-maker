# bug-0003: Duplicate pages in a page spec are silently dropped

**Severity:** High — this is the exact silent-drop failure class v0.13.0 was shipped to eliminate, still present for duplicates.
**Type:** Code bug relative to the written contract, **with a remedy decision required from Chris** (error vs. honor duplicates).  The spec is _not_ wrong — README line 43 and the CLAUDE.md invariant say “never silently drop a requested page”, and the code violates them — but the spec is _incomplete_: it never says what a duplicate page in a spec means, so the fixing agent must not pick a meaning unilaterally.

## Description

`medpdf::parse_page_spec` deduplicates pages (“Duplicates are dropped (first occurrence wins)” per its doc comment).  `page_spec::expand` wraps it but only adds the out-of-range bounds check, so a spec that names the same page twice silently yields it once.  The caller asked for two pages and got one, with exit 0 and a plausible-looking output — precisely the failure shape the v0.13.0 work (`425b9f6`) called out and fixed for the out-of-range case.

## Reproduction (verified 2026-07-16, v0.13.1)

```bash
pdf-maker -o two.pdf --blank-page "w=612,h=792,count=2"
pdf-maker -o out.pdf two.pdf "1,1" --json     # exit 0, page_count 1, pages [1]
pdf-maker -o out.pdf four.pdf "1-2,2" --json  # exit 0, page_count 2 — the range/single overlap also dedups
```

Rust test sketch (mirrors `tests/cli_tests.rs::merge_with_spec`): run with spec `"1,1"` against a 2-page input; currently exits 0 with a 1-page output.  After the fix, assert whichever ruling Chris makes (see below).

## Suggested fix — decision required

Two remedies satisfy the invariant; they differ in meaning:

1. **Error on duplicates (recommended).**  `expand` (or medpdf) rejects a spec that names a page more than once, naming the page — symmetrical with the out-of-range error (“correct the spec, or use `all`”).  Recommended because it is small, and because remedy 2 is really a new feature in disguise.
2. **Honor duplicates** — emit the page twice.  Legitimate use (two copies of a form), but **not implementable by just removing the dedup**: `copy_page_with_cache` would return the same object ID from its cache, putting one page object into `/Kids` twice.  A page object appearing twice in a page tree is malformed PDF (a `/Page` has a single `/Parent` and most tools assume tree nodes are distinct).  Honoring duplicates requires genuinely copying the page again (or a shallow re-copy), i.e. a medpdf-side feature.

Either way the implementation needs medpdf’s cooperation, because pdf-maker cannot currently _see_ the duplicates: `parse_page_spec` returns the already-deduped list, and re-scanning the spec text in pdf-maker cannot catch overlaps like `1-3,2`.  Cleanest shape: add a medpdf variant that returns the expansion _with_ duplicates preserved (or a duplicate count), and let `page_spec::expand` decide per the ruling.  Update README/`--help` page-spec docs to state the chosen semantics (feeds bug-0012).

## Why this fix addresses the bug

The invariant is “what the caller named is what they get, or a loud error”.  Remedy 1 restores it by making the mismatch loud; remedy 2 restores it by making the output match the request.  Both close the silent path; the detection has to live where the duplicates are still visible (the parser), which is why the medpdf API change is part of the fix rather than an implementation detail.

## History

`425b9f6` (v0.13.0) fixed the out-of-range half of this class and wrote the “never silently drop” contract into README, CLAUDE.md, and `--help`.  The duplicate half was never mentioned — the contract was written as if dedup did not exist, and no test covers duplicates.
