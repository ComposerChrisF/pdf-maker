# bug-0014: Two statically-invalid invocations exit 1 instead of 2

**Severity:** Low-Medium (exit-code contract; agents branch on the 1-vs-2 distinction mechanically)
**Type:** Code bug.  The spec is not the problem — README, `--help`, and `~/.claude/rules/cli-exit-codes.md` all define 2 as “usage error: invalid command-line arguments”, and both cases below are invalid command lines regardless of what any document contains.

## Description

Two argument errors are detected inside `run()` and therefore take the generic tool-error path (exit 1) even though nothing about the world — no file, no page count — is involved:

1. **Odd positional count.**  Three positional args (`in1.pdf all in2.pdf`) pass clap (`num_args = 2..` is satisfied) and hit the manual pairing check in `run()`: `Error: Input arguments must be in pairs of file paths and page specifications.` — exit 1.  (A _single_ positional arg happens to exit 2 because clap’s own `num_args` floor catches it; the same mistake gets two different codes depending on arity.)
2. **Invalid permission name with a password present.**  `--user-password pw --permissions bogus` reaches `medpdf::parse_permissions` deep in `run()` and exits 1, though the name is statically checkable at parse time.  (Without a password the name is never checked at all — bug-0004.)

Per the portfolio table, exit 1 must mean “the tool itself failed”, so an orchestrator reading 1 retries or debugs the tool; both cases here should instead tell it to fix the command line (2).

## Reproduction (verified 2026-07-16, v0.13.1)

```bash
pdf-maker -o out.pdf two.pdf all four.pdf          # exit 1, "must be in pairs"
pdf-maker -o out.pdf two.pdf all --user-password pw --permissions bogus   # exit 1
pdf-maker -o out.pdf --pad-to 0 two.pdf all         # contrast: exit 2 (clap value error)
```

Rust test sketch: assert `status.code() == Some(2)` for both commands (currently `Some(1)`), and that stderr names the problem.  Keep a companion assertion that a _world_ mismatch (out-of-range page) still exits 1, so the distinction is pinned from both sides.

## Suggested fix

1. **Pairing:** validate the pairing right after `Args::parse()` and report through clap so the exit code and formatting are clap’s: `Args::command().error(clap::error::ErrorKind::WrongNumberOfValues, "...pairs...").exit()`.
2. **Permissions:** validate names at parse time with a per-element `value_parser` on the `permissions` arg (each element checked against the known set, mirroring medpdf’s list), so clap rejects `bogus` at exit 2.  This also closes bug-0004’s corollary that the names go unchecked without a password.  Keep calling `medpdf::parse_permissions` for the actual bit-combining.

## Why this fix addresses the bug

Both errors become clap errors, which by construction carry exit 2 — restoring the invariant that 1 is reserved for caller-claim/world mismatches and genuine tool failures, which is what makes the code machine-actionable.  Routing through clap also removes the arity inconsistency in case 1 (one code for the same mistake at any odd arity).
