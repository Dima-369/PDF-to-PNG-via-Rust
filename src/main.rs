mod cli;

use clap::Parser;
use pdfium_render::error::PdfiumError::PdfiumLibraryInternalError;
use pdfium_render::prelude::*;
use pdfium_render::prelude::PdfiumInternalError::PasswordError;
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
        Pdfium::bind_to_library(
            Pdfium::pdfium_platform_library_name_at_path(args.library_directory.path()))
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
        Err(e) => panic!("{}", e)
    };
    if !args.page_count_only {
        let render_config = PdfRenderConfig::new()
            .set_target_width(i32::from(args.resolution_pixels))
            .set_maximum_height(i32::from(args.resolution_pixels));
        let prefix = cli::get_prefix(pdf_path, &args);
        // render each page to a bitmap image, saving each image to a PNG file
        for (index, page) in document.pages().iter().enumerate() {
            let file_name = if args.first_page_only {
                format!("{prefix}.png")
            } else {
                format!("{prefix}-{index}.png")
            };
            let final_path = args.output_directory.path().join(file_name);
            page.render_with_config(&render_config)?
                .as_image() // renders this page to an image::DynamicImage
                .as_rgba8() // convert to an image::Image
                .ok_or(PdfiumError::ImageError)?
                .save_with_format(final_path, image::ImageFormat::Png)
                .map_err(|_| PdfiumError::ImageError)?;
            if args.first_page_only {
                break;
            }
        }
    }
    print!("{}", document.pages().len());
    Ok(())
}