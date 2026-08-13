//! Sentinel + control for the lopdf `save_modern()` + encryption bug (issue #479).
//!
//! lopdf's `save_modern()` creates object streams (ObjStm) *after* encryption
//! runs, so those ObjStm bytes are never encrypted — even though a reader must
//! decrypt them. The dictionary objects packed into the ObjStm (pages, catalog,
//! resources) are therefore corrupt. While the bug exists, pdf-maker's
//! `save_document()` deliberately falls back to traditional `save()` for
//! encrypted documents (the `|| encryption.is_some()` clause).
//!
//! See: https://github.com/J-F-Liu/lopdf/issues/479
//!
//! ## How these tests detect the bug
//!
//! They inspect the saved bytes DIRECTLY — no external tool. The bug's
//! signature is that the ObjStm's stream body is *unencrypted*: decoding it
//! (Flate, if compressed) yields readable PDF object syntax (`/Type`,
//! `/MediaBox`, …). When the ObjStm is correctly encrypted, that plaintext
//! signature disappears.
//!
//! An earlier version shelled out to `pdf-dump` to count pages. That broke:
//! `pdf-dump` 0.21.0's overview path mis-parses encrypted PDFs (reports
//! `encrypted: false` / 0 objects / 0 pages with no error), which is unrelated
//! to issue #479. The dependency on an external tool's encrypted-PDF behavior
//! was the wrong design; these checks are now self-contained. (A bug report for
//! the `pdf-dump` regression lives in that repo.)
//!
//! ## Not to be confused with the overlay `/Length` bug (fixed)
//!
//! A separate defect once made overlay output unreadable in a way that looks
//! similar from the outside — a stream whose body silently vanishes on reload —
//! and the two were conflated once already. That one was NOT an ObjStm or an
//! encryption problem: `medpdf`'s `modify_content_stream` assigned re-encoded
//! bytes straight to `Stream::content`, bypassing `set_content` and leaving
//! `/Length` stale, so lopdf's length-based reader could not find `endstream`
//! and dropped the object to a bare dictionary. It reproduced on plain,
//! unencrypted output. Fixed in medpdf (`pdf_overlay_helpers.rs` now uses
//! `set_content`) and pinned there by `overlay_length_regression_tests.rs` plus
//! `no_raw_stream_content_assignment.rs`, which fails if a raw `.content =`
//! assignment reappears. pdf-maker needed no change beyond the dependency bump.
//!
//! The distinguishing question: does it reproduce WITHOUT encryption? If yes it
//! is a `/Length` problem, not #479.
//!
//! ## Canary behavior — READ THIS IF `save_modern_objstm_is_unencrypted` FAILS
//!
//! That test PASSES while the bug exists. If it FAILS, the ObjStm is no longer
//! plaintext, i.e. lopdf has fixed #479. When that happens:
//!   1. In `src/main.rs` `save_document()`, remove `|| encryption.is_some()`
//!      from the save condition so encrypted documents benefit from
//!      `save_modern()`.
//!   2. Invert this test to assert the ObjStm IS encrypted, and rename it to
//!      e.g. `save_modern_objstm_is_encrypted`.
//!   3. Update this module's docs.

use lopdf::{Document, Object, Stream, StringFormat, dictionary};
use medpdf::{EncryptionParams, encrypt_document};

const NUM_PAGES: u32 = 20;

/// Create a multi-page PDF with enough dictionary objects that `save_modern()`
/// will pack them into ObjStm (object streams).
fn create_multipage_document() -> Document {
    let mut doc = Document::with_version("1.7");

    let pages_id = doc.new_object_id();
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![],
            "Count" => Object::Integer(0),
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));

    // Trailer /ID is required for encryption
    let id_bytes = b"0123456789abcdef".to_vec();
    doc.trailer.set(
        "ID",
        Object::Array(vec![
            Object::String(id_bytes.clone(), StringFormat::Literal),
            Object::String(id_bytes, StringFormat::Literal),
        ]),
    );

    for i in 0..NUM_PAGES {
        let resources_id = doc.add_object(dictionary! {});
        let content = Stream::new(
            dictionary! {},
            format!("BT /F1 12 Tf 100 700 Td (Page {}) Tj ET", i + 1).into_bytes(),
        );
        let content_id = doc.add_object(content);

        let page = dictionary! {
            "Type" => "Page",
            "Parent" => Object::Reference(pages_id),
            "MediaBox" => vec![
                Object::Real(0.0), Object::Real(0.0),
                Object::Real(612.0), Object::Real(792.0),
            ],
            "Resources" => Object::Reference(resources_id),
            "Contents" => Object::Reference(content_id),
        };
        let page_id = doc.add_object(page);

        let pd = doc.get_object_mut(pages_id).unwrap().as_dict_mut().unwrap();
        pd.get_mut(b"Kids")
            .unwrap()
            .as_array_mut()
            .unwrap()
            .push(Object::Reference(page_id));
        pd.set("Count", Object::Integer((i + 1) as i64));
    }

    doc
}

/// Index of the first occurrence of `needle` within `haystack`.
fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Extracts the raw (undecoded) stream body of the first ObjStm object in a
/// saved PDF buffer by locating the `ObjStm` marker and the following
/// `stream` … `endstream` keywords.
fn extract_first_objstm_raw_content(buf: &[u8]) -> Option<Vec<u8>> {
    let marker = find_subslice(buf, b"ObjStm")?;
    let stream_kw = marker + find_subslice(&buf[marker..], b"stream")?;
    let mut start = stream_kw + b"stream".len();
    // The byte after the `stream` keyword is an EOL (CRLF or LF) per spec.
    if buf.get(start) == Some(&b'\r') {
        start += 1;
    }
    if buf.get(start) == Some(&b'\n') {
        start += 1;
    }
    let end = start + find_subslice(&buf[start..], b"endstream")?;
    // Trim the single EOL that precedes `endstream`.
    let mut end = end;
    if end > start && buf[end - 1] == b'\n' {
        end -= 1;
    }
    if end > start && buf[end - 1] == b'\r' {
        end -= 1;
    }
    Some(buf[start..end].to_vec())
}

/// True if `bytes` contain readable PDF object syntax — the tell-tale of an
/// *unencrypted* object stream.
fn looks_like_plaintext_pdf_objects(bytes: &[u8]) -> bool {
    find_subslice(bytes, b"/Type").is_some()
        || find_subslice(bytes, b"/MediaBox").is_some()
        || find_subslice(bytes, b"/Parent").is_some()
}

/// Decides whether an ObjStm's raw content is *unencrypted*. An ObjStm may or
/// may not be Flate-compressed, so try a Flate decode and also inspect the raw
/// bytes; either revealing PDF object syntax means the content is plaintext
/// (the bug). Encrypted content is neither valid Flate nor tokenful, so this
/// returns false once lopdf encrypts the ObjStm.
fn objstm_content_is_unencrypted(raw: &[u8]) -> bool {
    if looks_like_plaintext_pdf_objects(raw) {
        return true;
    }
    let probe = Stream::new(dictionary! { "Filter" => "FlateDecode" }, raw.to_vec());
    matches!(probe.decompressed_content(), Ok(plain) if looks_like_plaintext_pdf_objects(&plain))
}

/// SENTINEL (canary). `save_modern()` packs dictionary objects into an ObjStm
/// created *after* encryption, leaving that ObjStm unencrypted. We detect that
/// directly: the ObjStm content decodes to plaintext PDF objects.
///
/// IF THIS TEST FAILS, lopdf issue #479 is fixed — see the module-level docs
/// for the three follow-up steps (remove the workaround, invert this test,
/// update comments).
#[test]
fn save_modern_objstm_is_unencrypted() {
    let mut doc = create_multipage_document();
    doc.compress();

    let params = EncryptionParams::new("user", "owner");
    encrypt_document(&mut doc, &params).unwrap();

    let mut buf = Vec::new();
    doc.save_modern(&mut buf).unwrap();

    // The scenario must actually exercise the bug: save_modern must pack objects
    // into an ObjStm. (Otherwise the test is vacuous.)
    assert!(
        find_subslice(&buf, b"ObjStm").is_some(),
        "save_modern() didn't create an ObjStm — test document needs more objects"
    );

    let objstm = extract_first_objstm_raw_content(&buf)
        .expect("could not locate the ObjStm stream body in save_modern() output");

    assert!(
        objstm_content_is_unencrypted(&objstm),
        "ObjStm content is no longer plaintext — lopdf #479 appears to be FIXED! \
         The ObjStm is now encrypted. See this module's docs for next steps. \
         Upstream: https://github.com/J-F-Liu/lopdf/issues/479"
    );
}

/// CONTROL. Traditional `save()` does not pack objects into a post-encryption
/// ObjStm, so the bug cannot occur on this path. The output is a valid
/// encrypted PDF (declares `/Encrypt`, content streams are ciphertext) and
/// contains no ObjStm. This should always pass regardless of #479.
#[test]
fn traditional_save_is_unaffected_by_objstm_bug() {
    let mut doc = create_multipage_document();
    doc.compress();

    let params = EncryptionParams::new("user", "owner");
    encrypt_document(&mut doc, &params).unwrap();

    let mut buf = Vec::new();
    doc.save_to(&mut buf).unwrap(); // traditional (classic xref) serializer

    // Encryption is actually configured.
    assert!(
        find_subslice(&buf, b"/Encrypt").is_some(),
        "traditional save() of an encrypted document should declare /Encrypt"
    );
    // Content streams are encrypted: the embedded page text is not plaintext.
    assert!(
        find_subslice(&buf, b"(Page 1)").is_none(),
        "page content appears unencrypted in traditional save() output"
    );
    // Traditional save sidesteps the ObjStm-after-encryption code path entirely.
    assert!(
        find_subslice(&buf, b"ObjStm").is_none(),
        "traditional save() unexpectedly produced an ObjStm"
    );
}

/// Proves the canary's detector flips correctly: it reports *plaintext* for an
/// unencrypted (Flate-compressed) object stream and *not plaintext* for an
/// encrypted-looking one. This is what guarantees `save_modern_objstm_is_unencrypted`
/// will FAIL for the right reason — i.e. only when the ObjStm becomes encrypted.
#[test]
fn detector_distinguishes_plaintext_from_encrypted_objstm() {
    // A realistic, compressible plaintext object-stream payload.
    let mut payload = Vec::new();
    for i in 0..NUM_PAGES {
        payload.extend_from_slice(
            format!(
                "<</Type/Page/Parent 1 0 R/MediaBox[0 0 612 792]/Contents {} 0 R>>",
                i + 2
            )
            .as_bytes(),
        );
    }

    let mut compressed_stream = Stream::new(dictionary! {}, payload);
    compressed_stream.compress().unwrap();
    let compressed = compressed_stream.content.clone();

    // Sanity: it really compressed, hiding the literal tokens in the Flate stream.
    assert!(
        compressed_stream.dict.get(b"Filter").is_ok(),
        "test payload should have Flate-compressed"
    );
    assert!(
        find_subslice(&compressed, b"/Type").is_none(),
        "compressed bytes should not expose plaintext tokens"
    );

    // Unencrypted (compressed plaintext) → detected as plaintext.
    assert!(
        objstm_content_is_unencrypted(&compressed),
        "Flate-compressed plaintext ObjStm must be detected as unencrypted"
    );

    // Encrypted simulation: scramble so the bytes are neither valid Flate nor
    // contain any PDF tokens.
    let scrambled: Vec<u8> = compressed.iter().rev().map(|b| b ^ 0x5A).collect();
    assert!(
        !objstm_content_is_unencrypted(&scrambled),
        "encrypted (scrambled) ObjStm must NOT be detected as unencrypted"
    );
}
