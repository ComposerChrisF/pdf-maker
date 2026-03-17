//! Sentinel test for lopdf save_modern() + encryption bug.
//!
//! lopdf's `save_modern()` creates object streams (ObjStm) *after* encryption runs,
//! so those streams are never encrypted. This produces corrupt PDFs where objects
//! packed into ObjStm (pages, resources, etc.) become inaccessible to PDF readers.
//!
//! See: https://github.com/J-F-Liu/lopdf/issues/479
//!
//! While the bug exists, the sentinel test passes (asserting corruption).
//! When lopdf fixes the bug, the sentinel test will FAIL — signaling that:
//!   1. Flip the assertions (see the doc comment on the test).
//!   2. In src/main.rs `save_document()`, remove `|| encryption.is_some()` from
//!      the save condition so encrypted documents can benefit from save_modern().
//!   3. Update the test name/comments to reflect it now guards the fix.
//!
//! Note: lopdf's own `Document::load()` can't reliably reload encrypted PDFs it
//! writes (separate xref stream loader bug), so we use `pdf-dump --json` for
//! verification, which has its own PDF parser.

use lopdf::{dictionary, Document, Object, Stream, StringFormat};
use medpdf::{encrypt_document, EncryptionParams};
use std::process::Command;

const NUM_PAGES: u32 = 20;

/// Create a multi-page PDF with enough dictionary objects that save_modern()
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

        let pd = doc
            .get_object_mut(pages_id)
            .unwrap()
            .as_dict_mut()
            .unwrap();
        pd.get_mut(b"Kids")
            .unwrap()
            .as_array_mut()
            .unwrap()
            .push(Object::Reference(page_id));
        pd.set("Count", Object::Integer((i + 1) as i64));
    }

    doc
}

/// Query pdf-dump for the page count of a PDF file.
/// Returns None if pdf-dump is not available.
fn pdf_dump_page_count(path: &std::path::Path) -> Option<u32> {
    let output = Command::new("pdf-dump")
        .args([path.to_str().unwrap(), "--json"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    json.get("page_count")?.as_u64().map(|n| n as u32)
}

/// Query pdf-dump --validate for the warning count of a PDF file.
/// Returns None if pdf-dump is not available.
fn pdf_dump_warning_count(path: &std::path::Path) -> Option<u32> {
    let output = Command::new("pdf-dump")
        .args([path.to_str().unwrap(), "--validate", "--json"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    json.get("warning_count")?.as_u64().map(|n| n as u32)
}

/// Sentinel: detects that lopdf save_modern() still produces corrupt encrypted PDFs.
///
/// The corruption: save_modern() packs dictionary objects (pages, resources, etc.)
/// into ObjStm streams created *after* encryption. These ObjStm are never encrypted,
/// but readers try to decrypt them, producing garbage. Page objects become inaccessible.
///
/// This manifests as:
/// - 0 pages visible (page objects trapped in corrupt ObjStm)
/// - 21 "unreachable from trailer" warnings (content streams orphaned)
/// - Only ~22 of the expected ~64 objects are accessible
///
/// IF THIS TEST FAILS, the lopdf bug (issue #479) has been fixed!
/// When that happens:
///   1. Change the page_count assertion: `== 0` → `== NUM_PAGES`
///   2. Change the warning_count assertion: `> 0` → `== 0`
///   3. In src/main.rs `save_document()`, remove `|| encryption.is_some()` from
///      the save condition so encrypted documents benefit from save_modern().
///   4. Rename this test to `lopdf_save_modern_works_with_encryption` and update
///      the comments to reflect it now guards the fix.
#[test]
fn lopdf_save_modern_produces_corrupt_encrypted_pdf() {
    let mut doc = create_multipage_document();
    doc.compress();

    let params = EncryptionParams::new("user", "owner");
    encrypt_document(&mut doc, &params).unwrap();

    // save_modern() — the call under test
    let mut buf = Vec::new();
    doc.save_modern(&mut buf).unwrap();

    // Confirm ObjStm was actually created (otherwise the test isn't exercising the bug)
    let has_objstm = buf.windows(6).any(|w| w == b"ObjStm");
    assert!(
        has_objstm,
        "save_modern() didn't create ObjStm — test document needs more objects"
    );

    // Write to temp file for pdf-dump inspection
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), &buf).unwrap();

    // Verify corruption via pdf-dump
    let page_count = pdf_dump_page_count(tmp.path())
        .expect("pdf-dump must be on PATH to run this test");
    assert_eq!(
        page_count, 0,
        "lopdf save_modern() encryption bug appears to be FIXED! \
         pdf-dump found {page_count} pages (expected 0 while bug exists). \
         See this test's doc comment for next steps. \
         Upstream: https://github.com/J-F-Liu/lopdf/issues/479"
    );

    let warning_count = pdf_dump_warning_count(tmp.path()).unwrap();
    assert!(
        warning_count > 0,
        "lopdf save_modern() encryption bug appears to be FIXED! \
         pdf-dump found 0 warnings (expected unreachable-object warnings while bug exists). \
         See this test's doc comment for next steps. \
         Upstream: https://github.com/J-F-Liu/lopdf/issues/479"
    );
}

/// Control: traditional save() produces a valid encrypted PDF.
/// This should always pass regardless of the save_modern bug.
#[test]
fn lopdf_traditional_save_works_with_encryption() {
    let mut doc = create_multipage_document();
    doc.compress();

    let params = EncryptionParams::new("user", "owner");
    encrypt_document(&mut doc, &params).unwrap();

    let tmp = tempfile::NamedTempFile::new().unwrap();
    doc.save(tmp.path()).unwrap();

    // Verify via pdf-dump (lopdf's own loader has issues with encrypted xref streams)
    let page_count = pdf_dump_page_count(tmp.path())
        .expect("pdf-dump must be on PATH to run this test");
    assert_eq!(page_count, NUM_PAGES);

    let warning_count = pdf_dump_warning_count(tmp.path()).unwrap();
    assert_eq!(warning_count, 0);
}
