# Plan: Adopt cli-contract

## Problem

pdf-maker hand-rolls the portfolio’s path contract in `src/paths.rs`, and its exit codes as bare literals in `main.rs` (`std::process::exit(1)`, and `"exit_code": 1` in the `--json` error object).  The checks are good ones: three-state probes, fixed in bug-0013.  But they are a second copy of what `cli-contract` now implements and tests once for the whole portfolio, and a second copy drifts.  One sign is already there: `check_output_path` opens with a bare `output.is_dir()`, the shape bug-0013 removed from the probe below it.  Until 31-Oct-2026, pdf-maker could not adopt the crate, because a crates.io release cannot depend on a path or git dependency.  cli-contract 0.6.0 is now on crates.io, and making this adoption possible was the main reason it was published (cli-contract-history’s plan-0003 § Phase 6, step 5).

## Proposed Change

Depend on `cli-contract = "0.6"` from crates.io, with no `path`.  Then:

- **Inputs:** `check_input_file` becomes `cli_contract::require_input_path`, keeping the `what` label in the message pdf-maker prints.
- **The output file:** `check_output_path` becomes `cli_contract::check_output_file` under `--dry-run` and `prepare_output_file` otherwise, with `Clobber::Allow`, today’s behavior.  Whether to offer a `--no-clobber` is a separate question, not part of this plan.
- **Exit codes:** `cli_contract::TOOL_ERROR` and the rest in place of literals, and the `--json` error object’s `exit_code` read from the same constant.
- **`--help`:** keep pdf-maker’s own `EXIT_STATUS_HELP`, which explains each code in pdf-maker’s terms.  Check it against `cli_contract::HELP_BLOCK` so that the two cannot disagree.

`src/paths.rs` goes away, or shrinks to the label wrapping.

## Implementation Notes

- Read cli-contract’s 0.6.0 `CHANGELOG.md` and its `paths` module docs first.  `require_input_path` refuses a dangling symlink, which matches today’s `symlink_metadata` choice.
- **Messages:** `PathError`’s `Display` wording differs from pdf-maker’s.  Decide whether to keep pdf-maker’s text by mapping it, or to adopt the crate’s, and update the tests that pin the wording either way.
- **Errors:** `MedpdfError` wraps the error, so a `From<PathError>` (or a `map_err` at the five call sites in `main.rs`, lines 864–875) keeps `run`’s `Result` type.
- **The exit code** stays 1 for every path failure, so the `--help` table does not change.
- **BrokenPipe:** pdf-maker writes only its summary to stdout, so check whether `reset_sigpipe` or `ignore_broken_pipe` applies to it (a `pdf-maker … --json | head -c1` probe).

## Why Not a Workaround

A workaround is what exists today: a hand-written copy of a contract that the portfolio has already centralised.  `cli-contract` is the deterministic home for it (`strategy-defaults.md`, home 4: a contract between tools), and since 0.6.0 it is on crates.io, so pdf-maker can use it without giving up its own publishing.
