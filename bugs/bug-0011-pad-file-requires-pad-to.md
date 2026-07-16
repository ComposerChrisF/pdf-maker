# bug-0011: `--pad-last-page-file` without `--pad-to` is silently ignored

**Severity:** Medium (a caller-stated intent does nothing, exit 0)
**Type:** Code bug.  The spec is not the problem: README presents the flag strictly as a companion to `--pad-to` (“Optionally use a specific page for the last padding page”), and the code simply fails to enforce the dependency.

## Description

`apply_padding` in `src/main.rs` consults `pad_last_page_file` only inside the `if let Some(spec) = pad_to` branch.  Given alone, the flag parses, its `file=` path is dutifully existence-checked by `check_paths` — and then it is never read again.  The caller asked for a padding template and receives an output unchanged by it, exit 0, with nothing on stderr.

## Reproduction (verified 2026-07-16, v0.13.1)

```bash
pdf-maker -o two.pdf --blank-page "w=612,h=792,count=2"
pdf-maker -o one.pdf --blank-page letter
pdf-maker -o out.pdf two.pdf all --pad-last-page-file "file=one.pdf,page=1" --json
# exit 0, page_count 2 — one.pdf was never used
```

Rust test sketch: run the above via the `tests/cli_tests.rs` harness; currently exits 0.  After the fix, assert exit 2 and that the error names both flags.

## Suggested fix

Declare the dependency in clap, in `src/main.rs`:

```rust
#[arg(long, requires = "pad_to", help = "...")]
pad_last_page_file: Option<PadFileSpec>,
```

clap then rejects the lone flag as a usage error (exit 2) with a message naming the missing `--pad-to`.  Mention the dependency in the flag’s help string too.

## Why this fix addresses the bug

The flag has no meaning without `--pad-to`; making the dependency structural converts a silently inert argument into an immediate, zero-cost usage error — the correct exit-code class (2) per `~/.claude/rules/cli-exit-codes.md`, since the invocation is malformed independent of any document’s contents.
