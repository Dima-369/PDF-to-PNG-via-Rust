mod cli;

use clap::Parser;
use pdfium_render::prelude::PdfiumError::PdfiumLibraryInternalError;
use pdfium_render::prelude::PdfiumInternalError::PasswordError;
use pdfium_render::prelude::*;
use std::fs;

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
            let file_name = if args.first_page_only {
                format!("{prefix}.png")
            } else {
                format!("{prefix}-{index}.png")
            };
            let final_path = args.output_directory.path().join(&file_name);
            // write to a temporary file first and rename it, so readers never see a half-written PNG
            // the process id keeps the temporary file unique when several pdf2png processes write to the same directory
            // built from the final path, so a --prefix containing slashes keeps the temporary file in the same directory
            let final_file_name = final_path.file_name().and_then(|n| n.to_str()).unwrap_or(&file_name);
            let temp_path = final_path.with_file_name(format!(".{final_file_name}.{}.tmp", std::process::id()));
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

/// The 0-based page indices to convert: `from_page..=to_page`, clamped to the pages the PDF has.
fn page_range(from_page: Option<u16>, to_page: Option<u16>, page_count: u16) -> std::ops::Range<u16> {
    let start = from_page.unwrap_or(0);
    let end = to_page.map_or(page_count, |p| p.saturating_add(1).min(page_count));
    start..end
}

#[cfg(test)]
mod tests {
    use super::page_range;

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
