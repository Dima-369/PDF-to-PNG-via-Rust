mod cli;

use clap::Parser;
use pdfium_render::prelude::PdfiumError::PdfiumLibraryInternalError;
use pdfium_render::prelude::PdfiumInternalError::PasswordError;
use pdfium_render::prelude::*;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = cli::Args::parse();

    if args.text_only {
        let pdf_path = args.pdf_path.path().path();
        let bytes = fs::read(pdf_path)?;
        let text = pdf_extract::extract_text_from_mem(&bytes)?;
        print!("{}", text);
        return Ok(());
    }

    let pdfium = Pdfium::new(
        Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(
            args.library_directory.path(),
        ))
        .or_else(|_| Pdfium::bind_to_system_library())?,
    );
    let pdf_path = args.pdf_path.path().path();
    let document = match pdfium.load_pdf_from_file(pdf_path, args.password.as_deref()) {
        Ok(ok) => ok,
        Err(PdfiumLibraryInternalError(PasswordError)) => {
            if args.password.is_some() {
                eprint!("Passed PDF password is incorrect!");
            } else {
                eprint!("PDF is password protected!");
            }
            std::process::exit(3)
        }
        Err(e) => panic!("{}", e),
    };
    if !args.page_count_only {
        let render_config = PdfRenderConfig::new()
            .set_target_width(i32::from(args.resolution_pixels))
            .set_maximum_height(i32::from(args.resolution_pixels));
        let prefix = cli::get_prefix(pdf_path, &args);
        // render each page to a bitmap image, saving each image to a PNG file
        if let (Some(from_page), Some(to_page)) = (args.from_page, args.to_page) {
            if from_page > to_page {
                eprintln!("Warning: --from-page {from_page} is greater than --to-page {to_page}, no pages are converted.");
            }
        }
        // only load the requested pages, so a page range of a large PDF stays fast
        for index in page_range(args.from_page, args.to_page, document.pages().len()) {
            let page = document.pages().get(index)?;
            let file_stem = if args.first_page_only {
                prefix.clone()
            } else {
                format!("{prefix}-{index}")
            };
            if args.text_boxes {
                let json_path = args.output_directory.path().join(format!("{file_stem}.json"));
                write_atomically(&json_path, page_text_json(&page, index)?.as_bytes())?;
            }
            if args.no_images {
                if args.first_page_only {
                    break;
                }
                continue;
            }
            let final_path = args.output_directory.path().join(format!("{file_stem}.png"));
            // write to a temporary file first and rename it, so readers never see a half-written PNG
            let temp_path = temp_path_of(&final_path);
            let save_result = page.render_with_config(&render_config)?
                .as_image() // renders this page to an image::DynamicImage
                .as_rgba8() // convert to an image::Image
                .ok_or(PdfiumError::ImageError)?
                .save_with_format(&temp_path, image::ImageFormat::Png);
            save_result.map_err(|e| {
                let _ = fs::remove_file(&temp_path);
                e
            })?;
            fs::rename(&temp_path, &final_path).map_err(|e| {
                let _ = fs::remove_file(&temp_path);
                e
            })?;
            if args.first_page_only {
                break;
            }
        }
    }
    print!("{}", document.pages().len());
    Ok(())
}

/// The process id keeps the temporary file unique when several pdf2png processes write to the same directory.
/// Built from the final path, so a --prefix containing slashes keeps the temporary file in the same directory.
fn temp_path_of(final_path: &Path) -> PathBuf {
    let final_file_name = final_path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    final_path.with_file_name(format!(".{final_file_name}.{}.tmp", std::process::id()))
}

/// Writes to a temporary file first and renames it, so readers never see a half-written file.
fn write_atomically(final_path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let temp_path = temp_path_of(final_path);
    fs::write(&temp_path, bytes).and_then(|_| fs::rename(&temp_path, final_path)).map_err(|e| {
        let _ = fs::remove_file(&temp_path);
        e
    })
}

/// The characters of a page with their boxes in points from the top left corner, as JSON:
/// `{"width":595.28,"height":841.89,"chars":[["H",72,90.5,80.1,104.2],...]}`.
/// Spaces and line breaks which pdfium adds between text objects have an empty box.
fn page_text_json(page: &PdfPage, page_index: u16) -> Result<String, PdfiumError> {
    let height = page.height().value;
    let mut json = format!(
        "{{\"width\":{},\"height\":{},\"chars\":[",
        round_points(page.width().value),
        round_points(height)
    );
    // the boxes are in the coordinates of the unrotated page, which would not match the rotated image
    if matches!(page.rotation(), Ok(PdfPageRenderRotation::None)) {
        let text = page.text()?;
        let mut is_first = true;
        for char in text.chars().iter() {
            let Some(unicode) = char.unicode_char() else { continue };
            let char_text = match unicode {
                // pdfium ends lines with \r\n, the \n is enough
                '\r' => String::new(),
                // pdfium marks a hyphen at the end of a line like this
                '\u{2}' => "-".to_string(),
                _ => unicode.to_string(),
            };
            let (left, top, right, bottom) = match char.loose_bounds() {
                Ok(rect) => (rect.left().value, height - rect.top().value, rect.right().value, height - rect.bottom().value),
                Err(_) => (0.0, 0.0, 0.0, 0.0),
            };
            if !is_first {
                json.push(',');
            }
            is_first = false;
            // writing to a String can not fail
            let _ = write!(
                json,
                "[{},{},{},{},{}]",
                json_string(&char_text),
                round_points(left),
                round_points(top),
                round_points(right),
                round_points(bottom)
            );
        }
    } else {
        eprintln!("Warning: page {page_index} is rotated, it gets no text boxes.");
    }
    json.push_str("]}");
    Ok(json)
}

/// Two decimals are far below a pixel, and keep the JSON small.
fn round_points(value: f32) -> f32 {
    (value * 100.0).round() / 100.0
}

fn json_string(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len() + 2);
    escaped.push('"');
    for c in text.chars() {
        match c {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\t' => escaped.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(escaped, "\\u{:04x}", c as u32);
            }
            c => escaped.push(c),
        }
    }
    escaped.push('"');
    escaped
}

/// The 0-based page indices to convert: `from_page..=to_page`, clamped to the pages the PDF has.
fn page_range(from_page: Option<u16>, to_page: Option<u16>, page_count: u16) -> std::ops::Range<u16> {
    let start = from_page.unwrap_or(0);
    let end = to_page.map_or(page_count, |p| p.saturating_add(1).min(page_count));
    start..end
}

#[cfg(test)]
mod tests {
    use super::{json_string, page_range, round_points};

    #[test]
    fn json_string_escapes_quotes_backslashes_and_control_characters() {
        assert_eq!(json_string("a"), "\"a\"");
        assert_eq!(json_string("\""), "\"\\\"\"");
        assert_eq!(json_string("\\"), "\"\\\\\"");
        assert_eq!(json_string("\n"), "\"\\n\"");
        assert_eq!(json_string("\u{1}"), "\"\\u0001\"");
        assert_eq!(json_string("ä"), "\"ä\"");
    }

    #[test]
    fn round_points_keeps_two_decimals() {
        assert_eq!(round_points(12.3456), 12.35);
        assert_eq!(round_points(0.0), 0.0);
    }

    #[test]
    fn page_range_defaults_to_all_pages() {
        assert_eq!(page_range(None, None, 5), 0..5);
    }

    #[test]
    fn page_range_is_inclusive_of_to_page() {
        assert_eq!(page_range(Some(1), Some(3), 5), 1..4);
        assert_eq!(page_range(Some(2), Some(2), 5), 2..3);
    }

    #[test]
    fn page_range_clamps_to_page_count() {
        assert_eq!(page_range(None, Some(100), 5), 0..5);
        assert_eq!(page_range(None, Some(u16::MAX), 5), 0..5);
    }

    #[test]
    fn page_range_is_empty_when_out_of_bounds_or_reversed() {
        assert!(page_range(Some(7), None, 5).is_empty());
        assert!(page_range(Some(3), Some(1), 5).is_empty());
        assert!(page_range(None, None, 0).is_empty());
    }
}
