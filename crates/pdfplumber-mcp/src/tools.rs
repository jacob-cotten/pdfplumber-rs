//! MCP tool dispatch — `pdf.*` tool implementations.
//!
//! Agent C owns this file. This stub compiles cleanly so the workspace builds
//! while C is in flight. Replace entirely with the real implementation.
//!
//! # Tool registry
//!
//! | Tool | Description |
//! |------|-------------|
//! | `pdf.extract_text` | Full text or single-page text extraction |
//! | `pdf.extract_tables` | Table detection and cell content |
//! | `pdf.extract_chars` | Character-level data with bbox + font metadata |
//! | `pdf.metadata` | Title, author, page count, tagged status |
//! | `pdf.layout` | Semantic layout inference (sections, headings, paragraphs) |
//! | `pdf.to_markdown` | PDF → Markdown via layout inference |
//! | `pdf.render_page` | Rasterize page to PNG (base64, requires `raster` feature) |
//! | `pdf.accessibility` | PDF/UA audit with violation list |
//! | `pdf.infer_tags` | Heuristic tag inference for untagged PDFs |

use serde_json::Value;

/// Return the `tools/list` response body.
pub fn list() -> Value {
    serde_json::json!({
        "tools": tool_definitions()
    })
}

/// Dispatch a `tools/call` request to the appropriate handler.
///
/// Returns a `{ content: [{ type, text }], isError: bool }` response body.
pub fn call(name: &str, args: &Value) -> Value {
    let result = match name {
        "pdf.extract_text" => extract_text(args),
        "pdf.extract_tables" => extract_tables(args),
        "pdf.extract_chars" => extract_chars(args),
        "pdf.metadata" => metadata(args),
        "pdf.layout" => layout(args),
        "pdf.to_markdown" => to_markdown(args),
        "pdf.render_page" => render_page(args),
        "pdf.accessibility" => accessibility(args),
        "pdf.infer_tags" => infer_tags(args),
        _ => Err(format!("unknown tool: {name}")),
    };
    match result {
        Ok(text) => serde_json::json!({
            "content": [{ "type": "text", "text": text }],
            "isError": false
        }),
        Err(e) => serde_json::json!({
            "content": [{ "type": "text", "text": e }],
            "isError": true
        }),
    }
}

// ── tool stubs (Agent C replaces these) ──────────────────────────────────────

fn open(args: &Value) -> Result<pdfplumber::Pdf, String> {
    let path = args.get("path").and_then(Value::as_str)
        .ok_or_else(|| "missing required argument: path".to_string())?;
    pdfplumber::Pdf::open_file(path, None).map_err(|e| e.to_string())
}

fn extract_text(args: &Value) -> Result<String, String> {
    let pdf = open(args)?;
    let opts = pdfplumber::TextOptions::default();
    if let Some(n) = args.get("page").and_then(Value::as_u64) {
        return Ok(pdf.page(n as usize).map_err(|e| e.to_string())?
            .extract_text(&opts));
    }
    let mut out = String::new();
    for result in pdf.pages_iter() {
        let page = result.map_err(|e| e.to_string())?;
        out.push_str(&page.extract_text(&opts));
        out.push('\n');
    }
    Ok(out)
}

fn extract_tables(args: &Value) -> Result<String, String> {
    let pdf = open(args)?;
    let settings = pdfplumber::TableSettings::default();
    let page_idx = args.get("page").and_then(Value::as_u64).unwrap_or(0) as usize;
    let page = pdf.page(page_idx).map_err(|e| e.to_string())?;
    let tables = page.find_tables(&settings);
    Ok(serde_json::to_string_pretty(&tables.iter().map(|t| {
        serde_json::json!({
            "rows": t.rows.len(),
            "cols": t.rows.first().map_or(0, |r| r.len()),
        })
    }).collect::<Vec<_>>()).unwrap_or_default())
}

fn extract_chars(args: &Value) -> Result<String, String> {
    let pdf = open(args)?;
    let page_idx = args.get("page").and_then(Value::as_u64).unwrap_or(0) as usize;
    let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(500) as usize;
    let page = pdf.page(page_idx).map_err(|e| e.to_string())?;
    let chars: Vec<_> = page.chars().iter().take(limit).map(|c| serde_json::json!({
        "text": c.text,
        "x0": c.bbox.x0, "top": c.bbox.top, "x1": c.bbox.x1, "bottom": c.bbox.bottom,
        "size": c.size,
        "fontname": c.fontname,
    })).collect();
    Ok(serde_json::to_string_pretty(&chars).unwrap_or_default())
}

fn metadata(args: &Value) -> Result<String, String> {
    let pdf = open(args)?;
    let m = pdf.metadata();
    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "title":         m.title,
        "author":        m.author,
        "subject":       m.subject,
        "keywords":      m.keywords,
        "creator":       m.creator,
        "producer":      m.producer,
        "creation_date": m.creation_date,
        "mod_date":      m.mod_date,
        "page_count":    pdf.page_count(),
    })).unwrap_or_default())
}

#[cfg(feature = "layout")]
fn layout(args: &Value) -> Result<String, String> {
    use pdfplumber_layout::Document;
    let pdf = open(args)?;
    let doc = Document::from_pdf(&pdf);
    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "sections": doc.sections().len(),
        "word_count": doc.word_count(),
    })).unwrap_or_default())
}

#[cfg(not(feature = "layout"))]
fn layout(_args: &Value) -> Result<String, String> {
    Err("layout feature not enabled — rebuild with --features layout".into())
}

#[cfg(feature = "layout")]
fn to_markdown(args: &Value) -> Result<String, String> {
    use pdfplumber_layout::Document;
    let pdf = open(args)?;
    Ok(Document::from_pdf(&pdf).to_markdown())
}

#[cfg(not(feature = "layout"))]
fn to_markdown(_args: &Value) -> Result<String, String> {
    Err("layout feature not enabled — rebuild with --features layout".into())
}

#[cfg(feature = "raster")]
fn render_page(args: &Value) -> Result<String, String> {
    use pdfplumber_raster::{RasterOptions, Rasterizer};
    let pdf = open(args)?;
    let page_idx = args.get("page").and_then(Value::as_u64).unwrap_or(0) as usize;
    let scale = args.get("scale").and_then(Value::as_f64).unwrap_or(2.0) as f32;
    let opts = RasterOptions { scale, ..Default::default() };
    let rasterizer = Rasterizer::new(opts);
    let result = rasterizer.render_page_index(&pdf, page_idx)
        .ok_or_else(|| "page index out of range".to_string())?
        .map_err(|e| e.to_string())?;
    Ok(base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &result.png))
}

#[cfg(not(feature = "raster"))]
fn render_page(_args: &Value) -> Result<String, String> {
    Err("raster feature not enabled — rebuild with --features raster".into())
}

fn accessibility(_args: &Value) -> Result<String, String> {
    Err("pdfplumber-a11y not yet wired into this binary — coming soon".into())
}

fn infer_tags(_args: &Value) -> Result<String, String> {
    Err("pdfplumber-a11y not yet wired into this binary — coming soon".into())
}

// ── tool schema definitions ───────────────────────────────────────────────────

/// Build the tool list with full JSON Schema definitions.
///
/// Separate from the const because `serde_json::json!` isn't const-compatible.
/// Called once by `list()`.
pub fn tool_definitions() -> Vec<Value> {
    vec![
        serde_json::json!({
            "name": "pdf.extract_text",
            "description": "Extract all text from a PDF, or a single page if `page` is given.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Absolute path to the PDF file" },
                    "page": { "type": "integer", "description": "0-based page index (omit for all pages)" }
                },
                "required": ["path"]
            }
        }),
        serde_json::json!({
            "name": "pdf.extract_tables",
            "description": "Detect and extract tables from a PDF page.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "page": { "type": "integer", "description": "0-based page index (default: 0)" }
                },
                "required": ["path"]
            }
        }),
        serde_json::json!({
            "name": "pdf.extract_chars",
            "description": "Extract character-level data: bbox, font name, size.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "page": { "type": "integer", "description": "0-based page index (default: 0)" },
                    "limit": { "type": "integer", "description": "Max chars to return (default: 500)" }
                },
                "required": ["path"]
            }
        }),
        serde_json::json!({
            "name": "pdf.metadata",
            "description": "Return document metadata: title, author, page count, tagged status, PDF version.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }
        }),
        serde_json::json!({
            "name": "pdf.layout",
            "description": "Run semantic layout inference: sections, headings, paragraphs, lists, figures. Requires `layout` feature.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }
        }),
        serde_json::json!({
            "name": "pdf.to_markdown",
            "description": "Convert PDF to structured Markdown via layout inference. Requires `layout` feature.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }
        }),
        serde_json::json!({
            "name": "pdf.render_page",
            "description": "Rasterize a PDF page to PNG, returned as base64. Requires `raster` feature.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "page": { "type": "integer", "description": "0-based page index (default: 0)" },
                    "scale": { "type": "number", "description": "Render scale factor (default: 2.0)" }
                },
                "required": ["path"]
            }
        }),
        serde_json::json!({
            "name": "pdf.accessibility",
            "description": "Run a PDF/UA accessibility audit. Returns violations with severity and remediation hints.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }
        }),
        serde_json::json!({
            "name": "pdf.infer_tags",
            "description": "Heuristic tag inference for untagged PDFs — approximates heading/paragraph/figure structure.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }
        }),
    ]
}
