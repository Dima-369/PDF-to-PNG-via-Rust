use clap::Parser;
use clio::{ClioPath, Input};
use pdfium_render::prelude::*;

/// Convert a PDF to image files, one image file per PDF page.
/// It uses a default target width/height of 2000px per resulting image.
/// This overrides existing image files in the output directory.
/// Prints the PDF page count to stdout.
#[derive(Parser, Debug)]
struct Args {
    /// The PDF file to convert to images.
    #[clap(value_parser)]
    pdf_path: Input,

    /// The PDF password.
    #[arg(short, long)]
    password: Option<String>,

    /// The output directory where all the image files are saved to.
    #[clap(long, short, value_parser = clap::value_parser ! (ClioPath).exists().is_dir(), default_value = ".")]
    output_directory: ClioPath,

    /// The directory to the libpdfium.dylib file.
    #[clap(long, short, value_parser = clap::value_parser ! (ClioPath).exists().is_dir(), default_value = ".")]
    library_directory: ClioPath,

    /// The target width and height pixel size. The width and height of the PNG files will not exceed this value.
    #[arg(short, long, default_value_t = 2000)]
    resolution_pixels: u16,
}

/// Renders each page in the PDF file at the given path to a separate JPEG file.
///  Bind to a Pdfium library in the same directory as our Rust executable;
/// failing that, fall back to using a Pdfium library provided by the operating system.
fn main() {
    let args = Args::parse();
    let pdfium = Pdfium::new(
        Pdfium::bind_to_library(
            Pdfium::pdfium_platform_library_name_at_path(args.library_directory.path()))
            .or_else(|_| Pdfium::bind_to_system_library()).unwrap(),
    );
    let document = pdfium.load_pdf_from_file(args.pdf_path.path().path(), args.password.as_deref()).unwrap();
    let render_config = PdfRenderConfig::new()
        .set_target_width(args.resolution_pixels as Pixels)
        .set_maximum_height(args.resolution_pixels as Pixels)
        .rotate_if_landscape(PdfPageRenderRotation::Degrees90, true);
    // render each page to a bitmap image, saving each image to a PNG file
    for (index, page) in document.pages().iter().enumerate() {
        let to_path = args.output_directory.path().join(format!("test-page-{}.png", index));
        page.render_with_config(&render_config).unwrap()
            .as_image() // renders this page to an image::DynamicImage
            .as_rgba8() // convert to an image::Image
            .ok_or(PdfiumError::ImageError).unwrap()
            .save_with_format(to_path, image::ImageFormat::Png)
            .map_err(|_| PdfiumError::ImageError).unwrap();
    }
    print!("{}", document.pages().len());
}
