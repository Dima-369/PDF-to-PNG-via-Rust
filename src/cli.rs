use clap::ArgAction;
use clap::Parser;
use clio::{ClioPath, Input};
use std::path::Path;

/// Convert a PDF to image files, one image file per PDF page.
/// It uses a default target width/height of 2000px per resulting image.
/// This overrides existing image files in the output directory.
/// Prints the PDF page count to stdout.
/// If the PDF is password protected or if the password is incorrect, exit with code 3.
#[derive(Parser, Debug)]
pub struct Args {
    /// Convert only first page without adding -0 suffix and also print page count to stdout.
    #[clap(short, long, action = ArgAction::SetTrue)]
    pub first_page_only: bool,

    /// Print PDF page count to stdout and quit without converting to PNG.
    #[clap(long, action = ArgAction::SetTrue)]
    pub page_count_only: bool,

    /// Extract and print text content from the PDF, then exit.
    #[clap(long, action = ArgAction::SetTrue)]
    pub text_only: bool,

    /// The PDF file to convert to images.
    #[clap(value_parser)]
    pub pdf_path: Input,

    /// The PDF password.
    #[arg(short, long)]
    pub password: Option<String>,

    /// The file prefix of the PNG files meaning the "foo" part for "foo-0.png" when converting "foo.pdf".
    /// The prefix can be changed here. If missing, the file name without the extension from the PDF file is taken.
    #[arg(long)]
    pub prefix: Option<String>,

    /// The output directory where all the image files are saved to.
    #[clap(short, long, value_parser = clap::value_parser!(ClioPath).exists().is_dir(), default_value = ".")]
    pub output_directory: ClioPath,

    /// The directory which contains the libpdfium.dylib file.
    #[clap(short, long, value_parser = clap::value_parser!(ClioPath).exists().is_dir(), default_value = ".")]
    pub library_directory: ClioPath,

    /// The target width and maximum height in pixels. The width and height of the PNG files will not exceed this value.
    #[arg(short, long, default_value_t = 2000)]
    pub resolution_pixels: u16,
}

pub fn get_prefix(pdf_path: &Path, args: &Args) -> String {
    if let Some(prefix) = &args.prefix {
        return prefix.clone();
    }
    let pdf_path_str: &str = pdf_path.file_name()
        .unwrap_or_else(|| {
            eprintln!("Passed PDF file path should have file name!");
            std::process::exit(1)
        })
        .to_str()
        .unwrap_or_else(|| {
            eprintln!("Passed PDF file path can not be converted to a string!");
            std::process::exit(1)
        });
    pdf_path.extension().map_or_else(
            || pdf_path_str.to_string(),
            |ext| pdf_path_str[..pdf_path_str.len() - (ext.len() + 1)].to_string())
}