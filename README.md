# Convert PDF files to JPEG files, one per page

I created this since this implementation with Rust is about 2x times faster than what I used previously: https://github.com/Dima-369/pdf2png-mac

Only tested on macOS 14.0.

Download the libpdfium.dylib file from https://github.com/bblanchon/pdfium-binaries and add it next to the compiled binary

# Compile

```bash
cargo build --release
```

Then the executable will be under `target/release/pdf2jpeg`.
