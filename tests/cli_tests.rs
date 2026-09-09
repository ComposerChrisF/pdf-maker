//! CLI integration tests for pdf-maker.
//!
//! These tests invoke the pdf-maker binary as a subprocess to verify
//! end-to-end behavior: argument parsing, pipeline execution, and output validity.
//!
//! Tests that verify PDF content use `pdf-dump --json` for inspection.

use lopdf::{Document, Object, Stream, StringFormat, dictionary};
use std::path::Path;
use std::process::Command;

/// Create a minimal valid PDF and write it to the given path.
fn create_test_pdf(path: &Path, num_pages: u32) {
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

    let id_bytes = b"test0123456789ab".to_vec();
    doc.trailer.set(
        "ID",
        Object::Array(vec![
            Object::String(id_bytes.clone(), StringFormat::Literal),
            Object::String(id_bytes, StringFormat::Literal),
        ]),
    );

    for i in 0..num_pages {
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
            "Resources" => dictionary! {},
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

    doc.save(path).unwrap();
}

fn pdf_maker_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_pdf-maker"))
}

/// Query pdf-dump for the page count of a PDF file.
fn pdf_dump_page_count(path: &Path) -> Option<u32> {
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

/// Check whether pdf-dump reports the PDF as encrypted. Passes the user
/// password so pdf-dump (0.23+) can decrypt and exit 0; without a password it
/// exits non-zero (code 3) on an encrypted file and reports only partial data.
fn pdf_dump_is_encrypted(path: &Path, password: &str) -> Option<bool> {
    let output = Command::new("pdf-dump")
        .args([
            path.to_str().unwrap(),
            "--password",
            password,
            "--detail",
            "security",
            "--json",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    json.get("encrypted")?.as_bool()
}

// --- Basic merge ---

#[test]
fn cli_merge_all_pages() {
    // Arrange
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 3);
    let output = tempfile::NamedTempFile::new().unwrap();

    // Act
    let status = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "all",
        ])
        .status()
        .unwrap();

    // Assert
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path()).expect("pdf-dump must be on PATH");
    assert_eq!(page_count, 3);
}

#[test]
fn cli_merge_page_range() {
    // Arrange
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 5);
    let output = tempfile::NamedTempFile::new().unwrap();

    // Act
    let status = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "2-4",
        ])
        .status()
        .unwrap();

    // Assert
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path()).expect("pdf-dump must be on PATH");
    assert_eq!(page_count, 3);
}

// --- Encryption (#5) ---

#[test]
fn cli_encryption_produces_encrypted_pdf() {
    // Arrange
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 2);
    let output = tempfile::NamedTempFile::new().unwrap();

    // Act
    let status = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "all",
            "--user-password",
            "userpass",
            "--owner-password",
            "ownerpass",
        ])
        .status()
        .unwrap();

    // Assert
    assert!(status.success());
    let encrypted =
        pdf_dump_is_encrypted(output.path(), "userpass").expect("pdf-dump must be on PATH");
    assert!(encrypted, "Output PDF should be encrypted");
}

#[test]
fn cli_encryption_aes256() {
    // Arrange
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 1);
    let output = tempfile::NamedTempFile::new().unwrap();

    // Act
    let status = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "all",
            "--user-password",
            "pass",
            "--owner-password",
            "owner",
            "--encryption-algorithm",
            "aes256",
        ])
        .status()
        .unwrap();

    // Assert
    assert!(status.success());
    let encrypted = pdf_dump_is_encrypted(output.path(), "pass").expect("pdf-dump must be on PATH");
    assert!(encrypted);
}

#[test]
fn cli_encryption_with_permissions() {
    // Arrange
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 1);
    let output = tempfile::NamedTempFile::new().unwrap();

    // Act
    let status = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "all",
            "--user-password",
            "pass",
            "--owner-password",
            "owner",
            "--permissions",
            "print,copy",
        ])
        .status()
        .unwrap();

    // Assert
    assert!(status.success());
    let encrypted = pdf_dump_is_encrypted(output.path(), "pass").expect("pdf-dump must be on PATH");
    assert!(encrypted);
}

// --- Padding ---

#[test]
fn cli_pad_to_multiple() {
    // Arrange: 3 pages, pad to multiple of 4 → expect 4
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 3);
    let output = tempfile::NamedTempFile::new().unwrap();

    // Act
    let status = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "all",
            "--pad-to",
            "4",
        ])
        .status()
        .unwrap();

    // Assert
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path()).expect("pdf-dump must be on PATH");
    assert_eq!(page_count, 4);
}

// --- Blank pages ---

#[test]
fn cli_blank_page_named_size() {
    // Arrange
    let output = tempfile::NamedTempFile::new().unwrap();

    // Act
    let status = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            "--blank-page",
            "letter",
        ])
        .status()
        .unwrap();

    // Assert
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path()).expect("pdf-dump must be on PATH");
    assert_eq!(page_count, 1);
}

// --- Error cases ---

#[test]
fn cli_missing_output_flag() {
    // Arrange
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 1);

    // Act
    let output = pdf_maker_bin()
        .args([input.path().to_str().unwrap(), "all"])
        .output()
        .unwrap();

    // Assert
    assert!(!output.status.success());
}

#[test]
fn cli_nonexistent_input_file() {
    // Arrange
    let output = tempfile::NamedTempFile::new().unwrap();

    // Act
    let result = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            "/nonexistent/file.pdf",
            "all",
        ])
        .output()
        .unwrap();

    // Assert: a missing caller-asserted input is exit 1, naming the path.
    assert_eq!(result.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("/nonexistent/file.pdf"),
        "stderr must name the missing input: {stderr}"
    );
}

#[test]
fn cli_broad_compatibility() {
    // Arrange
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 1);
    let output = tempfile::NamedTempFile::new().unwrap();

    // Act
    let status = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "all",
            "--broad-compatibility",
        ])
        .status()
        .unwrap();

    // Assert
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path()).expect("pdf-dump must be on PATH");
    assert_eq!(page_count, 1);
}

// --- Drawing / overlay / imposition coverage (N6) ---

/// Minimal 1×1 red RGB PNG for --draw-image tests.
const RED_PIXEL_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0x99, 0x63, 0xF8, 0xCF, 0xC0, 0x00,
    0x00, 0x00, 0x03, 0x00, 0x01, 0xE3, 0xCE, 0xC5, 0x0E, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
    0x44, 0xAE, 0x42, 0x60, 0x82,
];

fn write_test_png(path: &Path) {
    std::fs::write(path, RED_PIXEL_PNG).unwrap();
}

#[test]
fn cli_blank_page_custom_dimensions() {
    let output = tempfile::NamedTempFile::new().unwrap();
    let status = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            "--blank-page",
            "w=8.5,h=11,units=in,count=3",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path()).expect("pdf-dump must be on PATH");
    assert_eq!(page_count, 3);
}

#[test]
fn cli_watermark_applied() {
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 2);
    let output = tempfile::NamedTempFile::new().unwrap();
    let status = pdf_maker_bin()
        .args([
            "-o", output.path().to_str().unwrap(),
            input.path().to_str().unwrap(), "all",
            "--watermark",
            "text=DRAFT,font=@Helvetica,size=24,x=1,y=1,units=in,color=red,alpha=0.4,h_align=center,v_align=center,pages=all",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path()).expect("pdf-dump must be on PATH");
    assert_eq!(page_count, 2);
}

#[test]
fn cli_draw_rect_applied() {
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 2);
    let output = tempfile::NamedTempFile::new().unwrap();
    let status = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "all",
            "--draw-rect",
            "x=72,y=72,w=200,h=100,color=blue,alpha=0.5,pages=all",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path()).expect("pdf-dump must be on PATH");
    assert_eq!(page_count, 2);
}

#[test]
fn cli_draw_line_applied() {
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 1);
    let output = tempfile::NamedTempFile::new().unwrap();
    let status = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "all",
            "--draw-line",
            "x1=72,y1=720,x2=540,y2=720,width=1.5,color=#444,pages=1",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path()).expect("pdf-dump must be on PATH");
    assert_eq!(page_count, 1);
}

#[test]
fn cli_draw_image_applied() {
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 2);
    let img = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
    write_test_png(img.path());
    let output = tempfile::NamedTempFile::new().unwrap();
    let status = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "all",
            "--draw-image",
            &format!(
                "file={},x=1,y=1,w=2,units=in,fit=contain,pages=all",
                img.path().display()
            ),
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path()).expect("pdf-dump must be on PATH");
    assert_eq!(page_count, 2);
}

#[test]
fn cli_overlay_applied() {
    let base = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(base.path(), 3);
    let overlay = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(overlay.path(), 1);
    let output = tempfile::NamedTempFile::new().unwrap();
    let status = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            base.path().to_str().unwrap(),
            "all",
            "--overlay",
            &format!(
                "file={},src_page=1,target_pages=1-3",
                overlay.path().display()
            ),
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path()).expect("pdf-dump must be on PATH");
    assert_eq!(page_count, 3);
}

#[test]
fn cli_nup_2x2() {
    // 8 input pages with cells_per_sheet = 4 → 2 output sheets.
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 8);
    let output = tempfile::NamedTempFile::new().unwrap();
    let status = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "all",
            "--nup",
            "cols=2,rows=2",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path()).expect("pdf-dump must be on PATH");
    assert_eq!(page_count, 2);
}

// --- Out-of-range page specs must ERROR, never be silently dropped ---
//
// Regression tests for the silent-wrong-output bug: `parse_page_spec` filtered
// pages beyond the document, so `1,99` against a 2-page PDF produced a 1-page
// PDF at exit 0.  Every case below asserts the exit CODE and, where a file could
// still be produced, the page count.

/// Run pdf-maker against an `n`-page input with the given page spec, returning
/// the exit code and the page count of any output actually produced.
fn merge_with_spec(source_pages: u32, spec: &str) -> (Option<i32>, Option<u32>) {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pdf");
    create_test_pdf(&input, source_pages);
    let output = dir.path().join("out.pdf");

    let result = pdf_maker_bin()
        .args([
            "-o",
            output.to_str().unwrap(),
            input.to_str().unwrap(),
            spec,
        ])
        .output()
        .unwrap();

    let pages = if output.exists() {
        pdf_dump_page_count(&output)
    } else {
        None
    };
    (result.status.code(), pages)
}

#[test]
fn cli_out_of_range_page_mixed_with_valid_page_errors() {
    // The headline bug: this used to exit 0 with a 1-page PDF.
    let (code, pages) = merge_with_spec(2, "1,99");
    assert_eq!(code, Some(1), "out-of-range page must be a tool error");
    assert_eq!(pages, None, "no output file may be produced");
}

#[test]
fn cli_out_of_range_page_names_the_page_and_the_page_count() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pdf");
    create_test_pdf(&input, 2);
    let output = dir.path().join("out.pdf");

    let result = pdf_maker_bin()
        .args([
            "-o",
            output.to_str().unwrap(),
            input.to_str().unwrap(),
            "1,99",
        ])
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("99"),
        "must name the offending page: {stderr}"
    );
    assert!(
        stderr.contains("2 page(s)"),
        "must name the document's real page count: {stderr}"
    );
}

#[test]
fn cli_wholly_out_of_range_spec_errors() {
    let (code, pages) = merge_with_spec(2, "99");
    assert_eq!(code, Some(1));
    assert_eq!(pages, None);
}

#[test]
fn cli_range_past_the_end_errors_instead_of_clamping() {
    // "1-100" against a 2-page doc used to yield 2 pages at exit 0.
    let (code, pages) = merge_with_spec(2, "1-100");
    assert_eq!(code, Some(1));
    assert_eq!(pages, None);
}

#[test]
fn cli_open_ended_range_past_the_end_errors() {
    // "5-" against a 2-page doc used to select nothing.
    let (code, pages) = merge_with_spec(2, "5-");
    assert_eq!(code, Some(1));
    assert_eq!(pages, None);
}

#[test]
fn cli_open_start_range_past_the_end_errors() {
    // A bare "-9" cannot reach the page-spec parser: clap eats a leading dash as
    // a flag and exits 2, which is correctly clap's territory.  "1,-9" carries
    // the same open-start range through argv, and must error on page 9.
    let (code, pages) = merge_with_spec(2, "1,-9");
    assert_eq!(code, Some(1));
    assert_eq!(pages, None);
}

#[test]
fn cli_open_ended_range_within_the_document_still_works() {
    let (code, pages) = merge_with_spec(3, "2-");
    assert_eq!(code, Some(0));
    assert_eq!(pages, Some(2));
}

#[test]
fn cli_all_still_works() {
    let (code, pages) = merge_with_spec(3, "all");
    assert_eq!(code, Some(0));
    assert_eq!(pages, Some(3));
}

#[test]
fn cli_out_of_range_page_in_a_second_input_errors() {
    // The whole second input used to be dropped silently, leaving a 1-page PDF.
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pdf");
    create_test_pdf(&input, 2);
    let output = dir.path().join("out.pdf");

    let result = pdf_maker_bin()
        .args([
            "-o",
            output.to_str().unwrap(),
            input.to_str().unwrap(),
            "1",
            input.to_str().unwrap(),
            "5-",
        ])
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(1));
    assert!(!output.exists(), "no output file may be produced");
}

#[test]
fn cli_watermark_targeting_a_nonexistent_page_errors() {
    // Used to exit 0 having applied no watermark at all.
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pdf");
    create_test_pdf(&input, 2);
    let output = dir.path().join("out.pdf");

    let result = pdf_maker_bin()
        .args([
            "-o",
            output.to_str().unwrap(),
            input.to_str().unwrap(),
            "all",
            "--watermark",
            "text=DRAFT,font=@Helvetica,x=1,y=1,pages=99",
        ])
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(1));
    assert!(!output.exists());
}

#[test]
fn cli_draw_rect_targeting_a_nonexistent_page_errors() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pdf");
    create_test_pdf(&input, 2);
    let output = dir.path().join("out.pdf");

    let result = pdf_maker_bin()
        .args([
            "-o",
            output.to_str().unwrap(),
            input.to_str().unwrap(),
            "all",
            "--draw-rect",
            "x=1,y=1,w=10,h=10,pages=7",
        ])
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(1));
    assert!(!output.exists());
}

#[test]
fn cli_overlay_targeting_a_nonexistent_page_errors() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pdf");
    create_test_pdf(&input, 2);
    let output = dir.path().join("out.pdf");

    let result = pdf_maker_bin()
        .args([
            "-o",
            output.to_str().unwrap(),
            input.to_str().unwrap(),
            "all",
            "--overlay",
            &format!("file={},src_page=1,target_pages=99", input.display()),
        ])
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(1));
    assert!(!output.exists());
}

// --- Path contract (~/.claude/rules/cli-exit-codes.md) ---

#[test]
fn cli_missing_output_directory_errors_and_is_not_created() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pdf");
    create_test_pdf(&input, 1);
    let missing_dir = dir.path().join("no-such-dir");
    let output = missing_dir.join("out.pdf");

    let result = pdf_maker_bin()
        .args([
            "-o",
            output.to_str().unwrap(),
            input.to_str().unwrap(),
            "all",
        ])
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("no-such-dir"),
        "stderr must name the missing output directory: {stderr}"
    );
    assert!(
        !missing_dir.exists(),
        "pdf-maker must never create the output directory"
    );
}

#[test]
fn cli_missing_overlay_file_errors_naming_it() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pdf");
    create_test_pdf(&input, 1);
    let output = dir.path().join("out.pdf");

    let result = pdf_maker_bin()
        .args([
            "-o",
            output.to_str().unwrap(),
            input.to_str().unwrap(),
            "all",
            "--overlay",
            "file=/nonexistent/overlay.pdf,src_page=1",
        ])
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("/nonexistent/overlay.pdf"), "{stderr}");
    assert!(!output.exists());
}

// --- --dry-run ---

#[test]
fn cli_dry_run_writes_no_file_and_exits_zero() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pdf");
    create_test_pdf(&input, 3);
    let output = dir.path().join("out.pdf");

    let result = pdf_maker_bin()
        .args([
            "-o",
            output.to_str().unwrap(),
            input.to_str().unwrap(),
            "all",
            "--dry-run",
        ])
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(0));
    assert!(!output.exists(), "--dry-run must create NO file");
}

#[test]
fn cli_dry_run_still_catches_an_out_of_range_page() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pdf");
    create_test_pdf(&input, 2);
    let output = dir.path().join("out.pdf");

    let result = pdf_maker_bin()
        .args([
            "-o",
            output.to_str().unwrap(),
            input.to_str().unwrap(),
            "1,99",
            "--dry-run",
        ])
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(1));
    assert!(!output.exists());
}

// --- --json ---

#[test]
fn cli_json_summary_reports_the_page_count() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pdf");
    create_test_pdf(&input, 3);
    let output = dir.path().join("out.pdf");

    let result = pdf_maker_bin()
        .args([
            "-o",
            output.to_str().unwrap(),
            input.to_str().unwrap(),
            "2-3",
            "--json",
        ])
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(0));
    let json: serde_json::Value = serde_json::from_slice(&result.stdout).expect("stdout is JSON");
    assert_eq!(json["page_count"], 2);
    assert_eq!(json["written"], true);
    assert_eq!(json["dry_run"], false);
    assert_eq!(json["inputs"][0]["pages"], serde_json::json!([2, 3]));
    assert!(output.exists());
}

#[test]
fn cli_json_dry_run_reports_written_false() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pdf");
    create_test_pdf(&input, 2);
    let output = dir.path().join("out.pdf");

    let result = pdf_maker_bin()
        .args([
            "-o",
            output.to_str().unwrap(),
            input.to_str().unwrap(),
            "all",
            "--json",
            "--dry-run",
        ])
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(0));
    let json: serde_json::Value = serde_json::from_slice(&result.stdout).expect("stdout is JSON");
    assert_eq!(json["written"], false);
    assert_eq!(json["dry_run"], true);
    assert_eq!(json["page_count"], 2);
    assert_eq!(json["bytes"], serde_json::Value::Null);
    assert!(!output.exists());
}

#[test]
fn cli_json_error_object_on_an_out_of_range_page() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.pdf");
    create_test_pdf(&input, 2);
    let output = dir.path().join("out.pdf");

    let result = pdf_maker_bin()
        .args([
            "-o",
            output.to_str().unwrap(),
            input.to_str().unwrap(),
            "1,99",
            "--json",
        ])
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&result.stdout).expect("stdout is JSON");
    assert_eq!(json["exit_code"], 1);
    let error = json["error"].as_str().expect("error is a string");
    assert!(error.contains("99"), "{error}");
}

#[test]
fn cli_booklet_default() {
    // 4 input pages → 2 booklet sheets (front + back).
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 4);
    let output = tempfile::NamedTempFile::new().unwrap();
    let status = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "all",
            "--booklet",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let page_count = pdf_dump_page_count(output.path()).expect("pdf-dump must be on PATH");
    assert_eq!(page_count, 2);
}

/// Collect the `re` (rectangle) operands from one page's content streams.
///
/// `place_page` emits its clip rectangle as exactly `(x, y, placed_w, placed_h)`,
/// so this one operator pins the whole placement contract — where the page landed
/// and how big it is — with no rendering and no fixtures.
fn placement_rects(path: &Path, page_number: u32) -> Vec<(f64, f64, f64, f64)> {
    let doc = Document::load(path).expect("output must load");
    let page_id = *doc.get_pages().get(&page_number).expect("page must exist");
    let content = doc
        .get_and_decode_page_content(page_id)
        .expect("page content must decode");
    content
        .operations
        .iter()
        .filter(|op| op.operator == "re")
        .map(|op| {
            let n = |i: usize| op.operands[i].as_float().unwrap_or_default() as f64;
            (n(0), n(1), n(2), n(3))
        })
        .collect()
}

/// A rotated booklet back page must land on exactly the same rectangle as an
/// unrotated front page — same slot, differing only in the 180° rotation.
///
/// Regression test for the medpdf 0.13.0 adoption. `apply_booklet` used to pass
/// `(cx + w, cy + h)` for the rotated back side, hand-compensating the pre-0.13.0
/// contract in which a 180° placement landed at `[x-w, x] x [y-h, y]`. Since
/// medpdf 0.13.0 (its bug-0023/bug-0024) `place_page` anchors the placed bounding
/// box at `(x, y)` for any rotation, so that arithmetic double-compensated and
/// threw every back page a full page width and height off the sheet — a blank
/// back side at exit 0.
///
/// Note what this asserts and why: the `cm` scale coefficients are UNCHANGED by
/// that fault (only the translation moved), so an assertion on the sign of the
/// scale — the obvious way to test "was the 180° applied?" — passes in both the
/// broken and the fixed state. Assert on the destination rectangle instead.
#[test]
fn cli_booklet_back_pages_land_on_the_sheet() {
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 4);
    let output = tempfile::NamedTempFile::new().unwrap();
    let status = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "all",
            "--booklet",
            "flip=short_edge",
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let fronts = placement_rects(output.path(), 1);
    let backs = placement_rects(output.path(), 2);
    assert_eq!(fronts.len(), 2, "front sheet should carry two placements");
    assert_eq!(backs.len(), 2, "back sheet should carry two placements");
    assert_eq!(
        fronts, backs,
        "rotated back placements must occupy the same rectangles as the fronts"
    );

    // And they must actually be on the paper: default booklet sheet is 792x612.
    for (x, y, w, h) in backs {
        assert!(
            x >= 0.0 && y >= 0.0 && x + w <= 792.5 && y + h <= 612.5,
            "back placement ({x}, {y}, {w}, {h}) falls outside the 792x612 sheet"
        );
    }
}

/// Collect the uniform-scale coefficients (a, d) from each `cm` on a page.
///
/// A 180° placement shows up as a negative pair; an unrotated one as positive.
fn placement_scales(path: &Path, page_number: u32) -> Vec<(f64, f64)> {
    let doc = Document::load(path).expect("output must load");
    let page_id = *doc.get_pages().get(&page_number).expect("page must exist");
    doc.get_and_decode_page_content(page_id)
        .expect("page content must decode")
        .operations
        .iter()
        .filter(|op| op.operator == "cm" && op.operands.len() == 6)
        .map(|op| {
            let n = |i: usize| op.operands[i].as_float().unwrap_or_default() as f64;
            (n(0), n(3))
        })
        .collect()
}

fn booklet_backs_rotated(paper: &str, flip: &str) -> bool {
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 4);
    let output = tempfile::NamedTempFile::new().unwrap();
    let status = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "all",
            "--booklet",
            &format!("{paper},flip={flip}"),
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let backs = placement_scales(output.path(), 2);
    assert!(!backs.is_empty(), "back sheet should carry placements");
    let rotated = backs[0].0 < 0.0 && backs[0].1 < 0.0;
    // The whole sheet must agree; a half-rotated sheet is nonsense.
    for (a, d) in &backs {
        assert_eq!(
            (*a < 0.0 && *d < 0.0),
            rotated,
            "placements on one sheet disagree about rotation"
        );
    }
    rotated
}

/// Duplex-flip compensation depends on the (sheet orientation, flip) PAIR, not on
/// the flip alone — the axis that inverts content is the one parallel to the
/// content's horizontal: the long edge of a landscape sheet, the short edge of a
/// portrait one.
///
/// Regression test for bug-0001, confirmed by physical duplex print 2026-09-09:
/// the previous rule rotated for `short_edge` unconditionally, which is the
/// PORTRAIT rule applied to the default landscape sheet, so both settings printed
/// their backs upside down for opposite reasons.
///
/// Note this asserts on the `cm` sign, the opposite of
/// `cli_booklet_back_pages_land_on_the_sheet`, which asserts on the destination
/// rectangle and would pass here either way. The two faults move different
/// quantities: that one moved the translation and left the linear part alone,
/// this one moves the linear part and leaves the rectangle alone. Neither test
/// substitutes for the other.
#[test]
fn cli_booklet_duplex_flip_is_orientation_aware() {
    // Default booklet paper is landscape (792x612).
    assert!(
        booklet_backs_rotated("paper=letter", "long_edge"),
        "landscape + long_edge: the duplexer flips top-to-bottom, so backs need the 180"
    );
    assert!(
        !booklet_backs_rotated("paper=letter", "short_edge"),
        "landscape + short_edge: top stays top, so backs must NOT be rotated"
    );

    // A portrait sheet inverts the rule — the top-fold "flip-book" booklet.
    assert!(
        booklet_backs_rotated("paper_w=612,paper_h=792,units=pt", "short_edge"),
        "portrait + short_edge: needs the 180"
    );
    assert!(
        !booklet_backs_rotated("paper_w=612,paper_h=792,units=pt", "long_edge"),
        "portrait + long_edge: must NOT be rotated"
    );

    // `flip=none` never compensates, whatever the paper.
    assert!(!booklet_backs_rotated("paper=letter", "none"));
}

/// The derived imposition geometry must reach `--json`, not just stderr.
///
/// This is the obligation attached to permitting negative offsets (bug-0006): a
/// bleed is a legal layout, so a typo'd `margin=-0.5` is legal too, and the only
/// thing separating them is whether the caller can see what was computed. An
/// orchestrator reads `--json`, not the progress block.
#[test]
fn cli_json_reports_imposition_geometry() {
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 4);
    let output = tempfile::NamedTempFile::new().unwrap();
    let out = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "all",
            "--nup",
            "n=4,margin=-0.25,units=in",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json = String::from_utf8(out.stdout).unwrap();

    for field in ["cell_width_pt", "cell_height_pt", "bleed_overhang_pt"] {
        assert!(json.contains(field), "--json must report {field}: {json}");
    }
    // -0.25in = -18pt, so content bleeds 18pt past each edge.
    assert!(
        json.contains("18.0"),
        "bleed overhang should be reported as 18.0pt: {json}"
    );

    // An ordinary inset layout reports no overhang at all.
    let plain = tempfile::NamedTempFile::new().unwrap();
    let out2 = pdf_maker_bin()
        .args([
            "-o",
            plain.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "all",
            "--nup",
            "n=4,margin=0.25,units=in",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(out2.status.success());
    let json2 = String::from_utf8(out2.stdout).unwrap();
    assert!(
        json2.contains("\"bleed_overhang_pt\": 0.0"),
        "an inset layout must report zero overhang: {json2}"
    );
}

/// Write a copy of `src` with `/Rotate 90` stamped on every page.
///
/// pdf-maker cannot produce a rotated PDF itself, which is why bug-0018 sat as a
/// code trace until `medpdf::set_page_rotation` turned the fixture into four lines.
fn rotate_all_pages(src: &Path, dest: &Path, degrees: u32) {
    let mut doc = Document::load(src).expect("source must load");
    let ids: Vec<_> = doc.get_pages().values().copied().collect();
    for id in ids {
        medpdf::set_page_rotation(&mut doc, id, degrees).expect("set rotation");
    }
    doc.save(dest).expect("save rotated copy");
}

/// A `/Rotate 90` source must be scaled to the cell it is PLACED in, not to its
/// pre-rotation MediaBox.
///
/// Regression test for bug-0018. `place_page` honors `/Rotate` (medpdf 0.13.0), so
/// the placed footprint of a rotated portrait page is landscape — but imposition
/// used to compute `scale` from the raw MediaBox extents, which are still portrait.
/// Measured before the fix, on a 612x792 sheet with 306x396 cells: every placement
/// came out 396 wide in a 306-wide cell, so the left column overlapped its
/// neighbour and the right column ran 90pt off the paper, at exit 0.
///
/// The assertion is containment, not a magic number: each placement must fit inside
/// the cell it was assigned and stay on the sheet. That survives a change of paper
/// or grid, where a hardcoded rect would not.
#[test]
fn cli_rotated_source_is_scaled_to_its_placed_footprint() {
    let upright = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(upright.path(), 4);
    let rotated = tempfile::NamedTempFile::new().unwrap();
    rotate_all_pages(upright.path(), rotated.path(), 90);

    let output = tempfile::NamedTempFile::new().unwrap();
    let status = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            rotated.path().to_str().unwrap(),
            "all",
            "--nup",
            "cols=2,rows=2,paper=letter,orientation=portrait",
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let rects = placement_rects(output.path(), 1);
    assert_eq!(rects.len(), 4, "2x2 grid should carry four placements");

    // Letter portrait, no margin or gutter: cells are 306 x 396.
    const SHEET_W: f64 = 612.0;
    const SHEET_H: f64 = 792.0;
    const CELL_W: f64 = 306.0;
    const CELL_H: f64 = 396.0;
    const EPS: f64 = 0.5;

    for (x, y, w, h) in rects {
        assert!(
            w <= CELL_W + EPS && h <= CELL_H + EPS,
            "placement {w}x{h} overflows its {CELL_W}x{CELL_H} cell (bug-0018)"
        );
        assert!(
            x >= -EPS && y >= -EPS && x + w <= SHEET_W + EPS && y + h <= SHEET_H + EPS,
            "placement ({x}, {y}, {w}, {h}) falls outside the {SHEET_W}x{SHEET_H} sheet"
        );
        // The rotated page is landscape, so it fits the cell's width and leaves
        // height spare — the opposite of what the unrotated source would do.
        assert!(
            w > h,
            "a /Rotate 90 portrait page should be placed landscape"
        );
    }
}

/// `--pad-to` must size its blank pages by the last page as DISPLAYED.
///
/// Second site of bug-0018: a `/Rotate 90` last page used to get portrait pad pages
/// appended behind a page that displays landscape.
#[test]
fn cli_pad_pages_match_a_rotated_last_page() {
    let upright = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(upright.path(), 1);
    let rotated = tempfile::NamedTempFile::new().unwrap();
    rotate_all_pages(upright.path(), rotated.path(), 90);

    let output = tempfile::NamedTempFile::new().unwrap();
    let status = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            rotated.path().to_str().unwrap(),
            "all",
            "--pad-to",
            "2",
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let doc = Document::load(output.path()).unwrap();
    let pages = doc.get_pages();
    assert_eq!(pages.len(), 2);
    let pad_id = *pages.get(&2).unwrap();
    let (pad_w, pad_h) = medpdf::get_page_effective_size(&doc, pad_id).unwrap();
    assert!(
        pad_w > pad_h,
        "pad page {pad_w}x{pad_h} should be landscape to match the rotated last page"
    );
}

/// Run pdf-maker and return (exit code, stderr).
fn run_expecting_failure(args: &[&str]) -> (i32, String) {
    let out = pdf_maker_bin().args(args).output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// Asking to restrict a document without supplying a password must FAIL, not
/// silently produce an unrestricted file.
///
/// bug-0004, severity High: `--permissions` and `--encryption-algorithm` were
/// consumed inside the arm that only runs when a password is present, so without one
/// they were read and discarded. `--permissions none` wrote an **unencrypted** PDF
/// with **every** permission available, at exit 0 — the exact opposite of the stated
/// intent, with no signal. A security intent must never be silently downgraded.
#[test]
fn cli_restriction_without_a_password_is_refused() {
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 2);
    let output = tempfile::NamedTempFile::new().unwrap();
    let i = input.path().to_str().unwrap().to_string();
    let o = output.path().to_str().unwrap().to_string();

    for flag in [
        vec!["--permissions", "none"],
        vec!["--permissions", "print"],
        vec!["--encryption-algorithm", "aes256"],
    ] {
        let mut args = vec!["-o", &o, &i, "all"];
        args.extend(flag.iter());
        let (code, stderr) = run_expecting_failure(&args);
        assert_eq!(code, 2, "{flag:?} without a password must be a usage error");
        assert!(
            stderr.contains("password"),
            "the error must name the missing password: {stderr}"
        );
    }
}

/// The gate must not block the legitimate combination, or it would trade a silent
/// wrong output for a loud wrong refusal.
#[test]
fn cli_restriction_with_a_password_still_works() {
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 2);
    let output = tempfile::NamedTempFile::new().unwrap();
    let status = pdf_maker_bin()
        .args([
            "-o",
            output.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "all",
            "--owner-password",
            "secret",
            "--permissions",
            "print",
            "--encryption-algorithm",
            "aes256",
        ])
        .status()
        .unwrap();
    assert!(
        status.success(),
        "the documented combination must still work"
    );

    // And the file really is encrypted — the point of the whole exercise.
    let bytes = std::fs::read(output.path()).unwrap();
    let haystack = String::from_utf8_lossy(&bytes);
    assert!(
        haystack.contains("/Encrypt"),
        "output should declare /Encrypt"
    );

    // The default algorithm still applies when unspecified (it moved from a clap
    // default_value to an in-code unwrap_or, because an eager default would have
    // made the `requires` gate fire on every run).
    let plain = tempfile::NamedTempFile::new().unwrap();
    let status = pdf_maker_bin()
        .args([
            "-o",
            plain.path().to_str().unwrap(),
            input.path().to_str().unwrap(),
            "all",
            "--user-password",
            "openme",
        ])
        .status()
        .unwrap();
    assert!(status.success(), "a password alone must still encrypt");
    let bytes = std::fs::read(plain.path()).unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("/Encrypt"));
}

/// Statically-invalid invocations exit 2, not 1 (bug-0014).
///
/// The distinction is machine-actionable: exit 1 means "the tool failed, retry or
/// debug it"; exit 2 means "fix the command line". An orchestrator branches on it.
#[test]
fn cli_static_usage_errors_exit_2() {
    let input = tempfile::NamedTempFile::new().unwrap();
    create_test_pdf(input.path(), 2);
    let output = tempfile::NamedTempFile::new().unwrap();
    let i = input.path().to_str().unwrap().to_string();
    let o = output.path().to_str().unwrap().to_string();

    // An odd number of positionals. Previously exit 1, while a SINGLE positional
    // exited 2 via clap's own num_args floor — the same mistake, two codes.
    let (code, stderr) = run_expecting_failure(&["-o", &o, &i, "all", &i]);
    assert_eq!(code, 2, "odd positional count is a usage error: {stderr}");
    assert!(stderr.contains("pairs"), "{stderr}");

    // An invalid permission name, with a password present. Previously exit 1 from
    // deep inside run(); now rejected by the value_parser at parse time.
    let (code, stderr) = run_expecting_failure(&[
        "-o",
        &o,
        &i,
        "all",
        "--user-password",
        "pw",
        "--permissions",
        "bogus",
    ]);
    assert_eq!(code, 2, "bad permission name is a usage error: {stderr}");
    assert!(stderr.contains("bogus"), "{stderr}");

    // The other side of the contract, so the distinction is pinned from both ends:
    // a WORLD mismatch stays exit 1.
    let (code, _) = run_expecting_failure(&["-o", &o, &i, "1,99"]);
    assert_eq!(code, 1, "an out-of-range page is a tool error, not usage");
}
