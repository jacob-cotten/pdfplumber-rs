//! `pdfplumber annotate` — add highlights, notes, and links to a PDF.
//!
//! Requires the `write` feature on the `pdfplumber` crate.
//! Without it, the subcommand reports an error.

use std::path::PathBuf;

use pdfplumber::BBox;

#[cfg(feature = "write")]
use pdfplumber::write::{AnnotationColor, PdfWriter};

/// Annotation type from CLI.
#[derive(Debug, Clone)]
pub enum AnnotateMode {
    Highlight {
        page: usize,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        color: String,
        note: Option<String>,
    },
    Text {
        page: usize,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        text: String,
    },
    Link {
        page: usize,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        uri: String,
    },
}

pub fn run(
    input: &PathBuf,
    output: &PathBuf,
    page: usize,
    x0: f64, y0: f64, x1: f64, y1: f64,
    highlight: bool,
    text_note: Option<&str>,
    link_uri: Option<&str>,
    color: &str,
    note_contents: Option<&str>,
    password: Option<&str>,
) -> Result<(), i32> {
    #[cfg(not(feature = "write"))]
    {
        let _ = (input, output, page, x0, y0, x1, y1, highlight, text_note, link_uri, color, note_contents, password);
        eprintln!("error: the `write` feature is not enabled. Rebuild with --features write");
        return Err(1);
    }

    #[cfg(feature = "write")]
    {
        let file_bytes = std::fs::read(input).map_err(|e| {
            eprintln!("error reading {}: {e}", input.display());
            1i32
        })?;

        let pdf = crate::shared::open_pdf(input, password, false).map_err(|e| {
            eprintln!("error: {e}");
            1i32
        })?;

        let bbox = BBox { x0, y0, x1, y1 };
        let mut writer = PdfWriter::new(&pdf, &file_bytes);

        if highlight {
            let annot_color = parse_color(color);
            writer.add_highlight_with_comment(
                page.saturating_sub(1), // convert 1-based to 0-based
                bbox,
                annot_color,
                note_contents.unwrap_or(""),
                "",
            ).map_err(|e| {
                eprintln!("error adding highlight: {e}");
                1i32
            })?;
        } else if let Some(text) = text_note {
            writer.add_text_annotation(
                page.saturating_sub(1),
                bbox,
                text,
            ).map_err(|e| {
                eprintln!("error adding text annotation: {e}");
                1i32
            })?;
        } else if let Some(uri) = link_uri {
            writer.add_link_annotation(
                page.saturating_sub(1),
                bbox,
                uri,
            ).map_err(|e| {
                eprintln!("error adding link: {e}");
                1i32
            })?;
        } else {
            eprintln!("error: specify --highlight, --text-note <text>, or --link-uri <url>");
            return Err(1);
        }

        let updated_bytes = writer.write_incremental().map_err(|e| {
            eprintln!("error writing incremental update: {e}");
            1i32
        })?;

        std::fs::write(output, &updated_bytes).map_err(|e| {
            eprintln!("error writing {}: {e}", output.display());
            1i32
        })?;

        println!(
            "annotated PDF written to {} ({} bytes, +{} bytes incremental)",
            output.display(),
            updated_bytes.len(),
            updated_bytes.len().saturating_sub(file_bytes.len()),
        );

        Ok(())
    }
}

#[cfg(feature = "write")]
fn parse_color(s: &str) -> AnnotationColor {
    match s.to_lowercase().as_str() {
        "yellow" => AnnotationColor::Yellow,
        "cyan" | "blue" => AnnotationColor::Cyan,
        "green" => AnnotationColor::Green,
        "pink" | "red" => AnnotationColor::Pink,
        _ => AnnotationColor::Yellow, // default
    }
}
