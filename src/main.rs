use std::path::Path;
use std::process::exit;

use clap::ArgAction;
use clap::Parser;
use clio::{ClioPath, Input};
use pdfium_render::error::PdfiumError::PdfiumLibraryInternalError;
use pdfium_render::prelude::*;
use pdfium_render::prelude::PdfiumInternalError::PasswordError;

/// Convert a PDF to image files, one image file per PDF page.
/// It uses a default target width/height of 2000px per resulting image.
/// This overrides existing image files in the output directory.
/// Prints the PDF page count to stdout.
/// If the PDF is password protected or if the password is incorrect, exit with code 3.
#[derive(Parser, Debug)]
struct Args {
    /// Convert only first page without adding -0 suffix and also print page count to stdout.
    #[clap(short, long, action = ArgAction::SetTrue)]
    first_page_only: bool,

    /// Print PDF page count to stdout and quit without converting to PNG.
    #[clap(long, action = ArgAction::SetTrue)]
    page_count_only: bool,

    /// The PDF file to convert to images.
    #[clap(value_parser)]
    pdf_path: Input,

    /// The PDF password.
    #[arg(short, long)]
    password: Option<String>,

    /// The file prefix of the PNG files meaning the "foo" part for "foo-0.png" when converting "foo.pdf".
    /// The prefix can be changed here. If missing, the file name without the extension from the PDF file is taken.
    #[arg(long)]
    prefix: Option<String>,

    /// The output directory where all the image files are saved to.
    #[clap(short, long, value_parser = clap::value_parser ! (ClioPath).exists().is_dir(), default_value = ".")]
    output_directory: ClioPath,

    /// The directory which contains the libpdfium.dylib file.
    #[clap(short, long, value_parser = clap::value_parser ! (ClioPath).exists().is_dir(), default_value = ".")]
    library_directory: ClioPath,

    /// The target width and maximum height in pixels. The width and height of the PNG files will not exceed this value.
    #[arg(short, long, default_value_t = 2000)]
    resolution_pixels: u16,
}

fn get_prefix(pdf_path: &Path, args: &Args) -> String {
    if let Some(prefix) = &args.prefix {
        return prefix.clone();
    }
    let pdf_path_str: &str = pdf_path.file_name()
        .unwrap_or_else(|| {
            eprintln!("Passed PDF file path should have file name!");
            exit(1)
        })
        .to_str()
        .unwrap_or_else(|| {
            eprintln!("Passed PDF file path can not be converted to a string!");
            exit(1)
        });
    pdf_path.extension().map_or_else(
            || pdf_path_str.to_string(),
            |ext| pdf_path_str[..pdf_path_str.len() - (ext.len() + 1)].to_string())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
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
            exit(3)
        }
        Err(e) => panic!("{}", e)
    };
    if !args.page_count_only {
        let render_config = PdfRenderConfig::new()
            .set_target_width(i32::from(args.resolution_pixels))
            .set_maximum_height(i32::from(args.resolution_pixels));
        let prefix = get_prefix(pdf_path, &args);
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
