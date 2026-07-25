//! Bounds-checked page-spec expansion.
//!
//! Historically `medpdf::parse_page_spec` **filtered** pages beyond the document
//! (`1,99` against a 2-page PDF quietly yielded `[1]`), so pdf-maker used to emit
//! a plausible-looking 1-page PDF and exit 0 — the silent-wrong-output failure
//! class.  medpdf now errors on out-of-range pages itself (bug-0021), but on the
//! *first* one only.  This module keeps its own up-front scan so it can name
//! *every* out-of-range page the caller requested, together with the document's
//! real page count and the `what` context, before delegating grammar validation
//! and expansion to medpdf.
//!
//! A page a caller explicitly named but the document does not contain is a
//! caller-claim/world mismatch (the same shape as a nonexistent input path), so
//! it is a **tool error, exit 1** — see `~/.claude/rules/cli-exit-codes.md`
//! § Input and Output Paths.  It is not a clap usage error (2): the argument is
//! syntactically valid, and only the loaded document can contradict it.

use medpdf::{MedpdfError, Result, parse_page_spec};

/// Expand a page spec against a document of `page_count` pages, erroring rather
/// than silently dropping any page the spec names beyond the document.
///
/// `what` describes the spec's origin for the error message, e.g.
/// `page spec '1,99' for input PDF 'in.pdf'` or `--watermark pages='99'`.
///
/// Grammar validation (syntax, page 0, inverted ranges) is delegated to
/// [`medpdf::parse_page_spec`]; only the bounds check is added here.
pub fn expand(spec: &str, page_count: u32, what: &str) -> Result<Vec<u32>> {
    // Scan for out-of-range pages FIRST, before delegating to medpdf. medpdf's
    // `parse_page_spec` itself errors on the first out-of-range page (bug-0021), which
    // would lose both the remaining offending pages and the `what` context. Doing
    // our own scan up front lets us keep naming *every* out-of-range page with that
    // context. Every integer literal in the page-spec grammar is a page number, so
    // scanning digit runs finds exactly the pages the caller named -- open ranges ("5-",
    // "-9") included, because their explicit bound is one of these runs.
    let mut out_of_range: Vec<u32> = Vec::new();
    for run in spec
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
    {
        let Ok(page) = run.parse::<u32>() else {
            return Err(MedpdfError::new(format!(
                "{what}: page number '{run}' is out of range (too large)"
            )));
        };
        if page > page_count && !out_of_range.contains(&page) {
            out_of_range.push(page);
        }
    }

    if !out_of_range.is_empty() {
        let list = out_of_range
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let (noun, verb) = if out_of_range.len() == 1 {
            ("page", "is")
        } else {
            ("pages", "are")
        };
        return Err(MedpdfError::new(format!(
            "{what}: {noun} {list} {verb} out of range; the document has {page_count} page(s). \
             Requested pages are never silently dropped -- correct the spec (or use 'all')."
        )));
    }

    // No out-of-range pages: medpdf validates the grammar (syntax, page 0, inverted
    // ranges) and expands. With the scan above clean, medpdf will not itself hit its
    // out-of-range error path here.
    let pages = parse_page_spec(spec, page_count)?;

    if pages.is_empty() {
        return Err(MedpdfError::new(format!(
            "{what}: selects no pages; the document has {page_count} page(s)."
        )));
    }

    Ok(pages)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHAT: &str = "page spec";

    fn err_msg(spec: &str, page_count: u32) -> String {
        match expand(spec, page_count, WHAT) {
            Ok(pages) => panic!("expected an error, got {pages:?}"),
            Err(e) => e.to_string(),
        }
    }

    #[test]
    fn in_range_specs_expand_as_before() {
        assert_eq!(expand("1,2", 2, WHAT).unwrap(), vec![1, 2]);
        assert_eq!(expand("all", 3, WHAT).unwrap(), vec![1, 2, 3]);
        assert_eq!(expand("2-", 2, WHAT).unwrap(), vec![2]);
        assert_eq!(expand("-2", 2, WHAT).unwrap(), vec![1, 2]);
        assert_eq!(expand("1-3", 3, WHAT).unwrap(), vec![1, 2, 3]);
        assert_eq!(expand("3,1", 3, WHAT).unwrap(), vec![3, 1]);
    }

    #[test]
    fn mixed_valid_and_out_of_range_page_errors() {
        // The headline bug: 1,99 on a 2-page document used to yield [1].
        let msg = err_msg("1,99", 2);
        assert!(msg.contains("99"), "message must name the page: {msg}");
        assert!(
            msg.contains("2 page(s)"),
            "message must name the real page count: {msg}"
        );
    }

    #[test]
    fn wholly_out_of_range_spec_errors() {
        assert!(err_msg("99", 2).contains("99"));
    }

    #[test]
    fn range_past_the_end_errors_instead_of_clamping() {
        assert!(err_msg("1-100", 2).contains("100"));
    }

    #[test]
    fn open_ended_range_past_the_end_errors() {
        assert!(err_msg("5-", 2).contains("5"));
    }

    #[test]
    fn open_start_range_past_the_end_errors() {
        assert!(err_msg("-9", 2).contains("9"));
    }

    #[test]
    fn every_out_of_range_page_is_named() {
        let msg = err_msg("1,99,100", 2);
        assert!(msg.contains("99") && msg.contains("100"), "{msg}");
    }

    #[test]
    fn all_on_an_empty_document_errors_rather_than_selecting_nothing() {
        assert!(err_msg("all", 0).contains("selects no pages"));
    }

    #[test]
    fn grammar_errors_still_come_from_medpdf() {
        assert!(expand("0", 5, WHAT).is_err()); // page 0
        assert!(expand("5-3", 5, WHAT).is_err()); // inverted
        assert!(expand("bogus", 5, WHAT).is_err()); // syntax
        assert!(expand("", 5, WHAT).is_err()); // empty
    }
}
