# bug-0013: An unreadable input is reported as “does not exist”

**Severity:** Low (fails safe — the tool refuses to run — but the diagnostic lies)
**Type:** Code bug (`src/paths.rs`).  The spec is not the problem.

## Description

`check_input_file` probes with `path.exists()` and `path.is_dir()` — bare bools that fold every stat failure (EACCES on an unsearchable parent directory, EIO, ELOOP) into `false`.  An input PDF that exists but cannot be stat-ed is therefore reported as `input PDF does not exist: ...`, sending the user hunting for a typo when the actual problem is permissions.  This is the two-state-probe shape `~/.claude/rules/positive-evidence-of-absence.md` warns about; here it gates a refusal rather than a destruction, so the stakes are only diagnostic accuracy — but the rule’s three-answer discipline (present / provably absent / unknown) is still the right shape, and the fix is small.

`check_output_path` has the milder mirror image: `parent.is_dir()` on an unreadable parent claims the output directory does not exist.

## Reproduction

```bash
mkdir -p /tmp/locked && cp two.pdf /tmp/locked/ && chmod 000 /tmp/locked
pdf-maker -o out.pdf /tmp/locked/two.pdf all
# → Error: input PDF does not exist: /tmp/locked/two.pdf   (it exists; it is unreadable)
chmod 755 /tmp/locked   # cleanup
```

Rust test sketch: create a temp dir, place a file, `set_permissions(0o000)` on the dir, call `paths::check_input_file`, assert the error message mentions access/permissions rather than nonexistence; restore permissions in a drop guard so the temp dir cleans up.  (Skip on platforms where the test runs as root, which bypasses the permission check.)

## Suggested fix

Probe with `std::fs::symlink_metadata` and match the error kind three ways:

```rust
match std::fs::symlink_metadata(path) {
    Ok(m) if m.is_dir() => Err(...is a directory...),
    Ok(_) => Ok(()),
    Err(e) if e.kind() == std::io::ErrorKind::NotFound =>
        Err(...does not exist...),
    Err(e) => Err(format!("{what} cannot be accessed: {} ({e})", path.display()).into()),
}
```

Same treatment for the output-parent probe.  Exit code stays 1 in every failing arm.

## Why this fix addresses the bug

Only a provable `NotFound` may claim absence; every other stat failure now names itself, so the diagnostic matches reality.  Behavior for the existing tested cases (missing file, directory-as-input, missing parent) is unchanged — the fix only splits the previously conflated “could not look” answer out of “looked, not there”.
