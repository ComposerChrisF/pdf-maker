# A Non-ASCII `color=` Value Panics Instead of Being Refused

**Severity:** medium.  A panic on caller input: exit 101 with a Rust backtrace hint instead of exit 2 with a usage message.  No file is written, so nothing is lost, but the exit code falls outside the documented table, and a caller branching on it reads a crash where it should read “fix the command”.

## What Happens

`parse_color` (`src/spec_types/parse.rs`) strips an optional `#`, then dispatches on `hex.len()` — a **byte** length — and slices the string at fixed byte offsets (`&hex[0..1]`, `&hex[0..2]`, …).  A value whose byte length is 3, 6, or 8 but which holds a multibyte character puts one of those offsets inside the character, and the slice panics before `u8::from_str_radix` can reject it.

Reproduced on v0.24.3:

```
$ pdf-maker -o out.pdf --blank-page letter --draw-rect "x=10,y=10,w=10,h=10,color=é1"
thread 'main' panicked at src/spec_types/parse.rs:193:35:
end byte index 1 is not a char boundary; it is inside 'é' (bytes 0..2 of string)
(exit 101)

$ pdf-maker -o out.pdf --blank-page letter --watermark "text=X,font=@Helvetica,x=1,y=1,color=1éaaa"
thread 'main' panicked at src/spec_types/parse.rs:199:35:
end byte index 2 is not a char boundary; it is inside 'é' (bytes 1..3 of string)
(exit 101)

$ pdf-maker -o out.pdf --blank-page letter --draw-line "x1=1,y1=1,x2=5,y2=5,color=12345é7"
thread 'main' panicked at src/spec_types/parse.rs:207:35:
end byte index 6 is not a char boundary; it is inside 'é' (bytes 5..7 of string)
(exit 101)
```

Every flag that takes `color=` reaches it: `--watermark`, `--draw-rect`, `--draw-line`.  The ten slices at lines 193–208 share the fault; a value such as `ééé` (6 bytes, every offset on a boundary) happens not to panic and is refused correctly; the existing invalid-hex tests use only ASCII values, so none reaches the panic.

## The Rule It Breaks

No panic on input in production: a malformed value on the command line is a usage error, decidable from the argument text alone, and exits 2 through clap with a message naming the value.  `"#GG0000"` already does exactly that (exit 2, “Invalid hex color: ‘#GG0000’”); `"é1"` must too.

## Fix Direction

Refuse a non-ASCII value before slicing, at the one place the length is read: `if !hex.is_ascii() { return Err(format!("Invalid hex color: '{s}'")); }` ahead of `match hex.len()`.  Once the string is known to be ASCII every byte offset is a character boundary, so the existing slices stay as they are; each then carries `#[expect(clippy::string_slice, reason = "hex is ASCII, checked above, so every byte offset is a char boundary")]` on its statement (or one on `parse_color`, since no other slice is in it).

## The Test That Pins It

A unit test in `parse.rs`’s `mod tests`: `parse_color("é1")`, `parse_color("#1éaaa")` and the 8-byte `parse_color("12345é7")` each return `Err` containing `Invalid hex color`, rather than panicking.  Reverting the guard makes the test panic, so it fails.  A CLI test in `tests/cli_tests.rs` that runs `--draw-rect "…,color=é1"` and asserts exit code 2 (not 101) and no output file pins the user-visible half.
