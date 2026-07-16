# bug-0004: `--permissions` and `--encryption-algorithm` are silently ignored when no password is given

**Severity:** High — a security intent is silently discarded; the output is unencrypted and unrestricted at exit 0.
**Type:** Code bug.  The spec is not the problem, but it is silent on the interaction (README’s Encryption section documents only the two password flags — see bug-0012).

## Description

In `src/main.rs::run`, the encryption parameters are built only when at least one of `--user-password` / `--owner-password` is present (`match (&args.user_password, &args.owner_password) { (None, None) => None, ... }`).  `--permissions` and `--encryption-algorithm` are consumed _inside_ that arm, so without a password they are never read at all:

- `pdf-maker -o out.pdf in.pdf all --permissions none` writes an **unencrypted** PDF with **every** permission available, exit 0.  The caller explicitly asked to restrict the document and got the opposite, silently.
- `--encryption-algorithm aes256` alone likewise produces an unencrypted file, exit 0.
- Corollary: an invalid permission name (`--permissions bogus`) is not even validated when no password is present, because `parse_permissions` is only reached inside the encryption arm.

## Reproduction (verified 2026-07-16, v0.13.1)

```bash
pdf-maker -o two.pdf --blank-page "w=612,h=792,count=2"
pdf-maker -o out.pdf two.pdf all --permissions none --json          # exit 0, "encrypted": false
pdf-maker -o out.pdf two.pdf all --encryption-algorithm aes256 --json  # exit 0, "encrypted": false
```

Rust test sketch: run each command via `assert_cmd`-style `Command` (see `tests/cli_tests.rs::pdf_maker_bin`); currently both exit 0 and `pdf_dump_is_encrypted` reports false.  After the fix, assert a non-zero exit and an error naming the missing password.

## Suggested fix

Make the dependency explicit at the clap layer so the combination is rejected as a usage error (exit 2):

1. Declare an `ArgGroup` (e.g. `encryption_credentials`) containing `user_password` and `owner_password`, `multiple = true`, not required.
2. On `permissions`: `requires = "encryption_credentials"`.
3. Change `encryption_algorithm` to `Option<EncryptionAlgo>` with `requires = "encryption_credentials"`, and apply the `Aes128` default in code (`unwrap_or`) inside the encryption arm.  The eager `default_value` must go, because clap cannot distinguish “defaulted” from “user-supplied” for the `requires` check.

Document the passwords-gate in `--help` for both flags, and while in this code, document the existing default that an empty permission list means `Permissions::all()` (medpdf’s `parse_permissions` returns all permissions for an empty slice) — that default is currently invisible to users (bug-0012 carries the README half).

## Why this fix addresses the bug

The failure is a caller intent (“restrict this document”) that the invocation cannot honor without a password.  Rejecting the combination at argument-parse time converts the silent discard into a loud, zero-cost usage error before any work begins — the same principle as the path checks in `src/paths.rs` — and clap ownership gives it the correct exit code 2 per `~/.claude/rules/cli-exit-codes.md`.

## Related

bug-0014 covers the exit-code half of `--permissions bogus` _with_ a password (currently exit 1, should be 2); fixing permissions validation via a clap `value_parser` would resolve both at once.
