//! LaTeX reconstruction from a cluster of [`Char`]s.
//!
//! Strategy:
//! 1. Process chars in reading order (left-to-right, with vertical offsets
//!    detected for super/subscript).
//! 2. For each character, look up its LaTeX mapping via [`crate::symbols`].
//! 3. Detect and wrap sub/superscripts with `^{…}` and `_{…}`.
//! 4. Detect simple fractions by vertical stacking (a char directly above
//!    another char at a similar x-position).
//!
//! The output is intentionally best-effort — we aim for correctness on
//! simple inline and display equations and graceful degradation on complex ones.

use pdfplumber_core::Char;

use crate::symbols::to_latex;

/// Threshold (as a fraction of the baseline font size) above/below which
/// a character is considered a superscript or subscript.
const SCRIPT_THRESHOLD: f64 = 0.3;

/// Reconstruct a LaTeX string from an ordered slice of characters.
///
/// Characters should be sorted in reading order (left-to-right for LTR text).
pub fn reconstruct_latex(chars: &[&Char]) -> String {
    if chars.is_empty() {
        return String::new();
    }

    // Compute median baseline y for script detection
    let baseline_y = median_baseline(chars);
    let median_size = median_size(chars);

    let mut out = String::new();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];
        let y_offset = ch.bbox.y0 - baseline_y;

        // Detect superscript: character is rendered above the baseline
        if y_offset < -(median_size * SCRIPT_THRESHOLD) {
            // Collect all consecutive superscript chars
            let (script_str, consumed) =
                collect_script(chars, i, baseline_y, median_size, ScriptDir::Super);
            out.push_str(&format!("^{{{script_str}}}"));
            i += consumed;
            continue;
        }

        // Detect subscript: character is rendered below the baseline
        if y_offset > median_size * SCRIPT_THRESHOLD {
            let (script_str, consumed) =
                collect_script(chars, i, baseline_y, median_size, ScriptDir::Sub);
            out.push_str(&format!("_{{{script_str}}}"));
            i += consumed;
            continue;
        }

        // Normal character
        let sym = to_latex(ch.text.chars().next().unwrap_or(' '));
        out.push_str(&sym);
        i += 1;
    }

    // Post-process: collapse trivial spaces (PDF sometimes has gap chars)
    collapse_spaces(&out)
}

// ── Script collection ────────────────────────────────────────────────────────

#[derive(PartialEq)]
enum ScriptDir {
    Super,
    Sub,
}

/// Collect consecutive chars that are in the same script direction (super or sub).
///
/// Returns `(latex_string, chars_consumed)`.
fn collect_script(
    chars: &[&Char],
    start: usize,
    baseline_y: f64,
    median_size: f64,
    dir: ScriptDir,
) -> (String, usize) {
    let mut s = String::new();
    let mut count = 0;

    for ch in &chars[start..] {
        let y_offset = ch.bbox.y0 - baseline_y;
        let is_script = match dir {
            ScriptDir::Super => y_offset < -(median_size * SCRIPT_THRESHOLD),
            ScriptDir::Sub => y_offset > median_size * SCRIPT_THRESHOLD,
        };
        if !is_script {
            break;
        }
        s.push_str(&to_latex(ch.text.chars().next().unwrap_or(' ')));
        count += 1;
    }

    (s, count.max(1))
}

// ── Statistical helpers ──────────────────────────────────────────────────────

fn median_baseline(chars: &[&Char]) -> f64 {
    let mut ys: Vec<f64> = chars.iter().map(|c| c.bbox.y0).collect();
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = ys.len();
    if n == 0 {
        return 0.0;
    }
    if n % 2 == 0 {
        (ys[n / 2 - 1] + ys[n / 2]) / 2.0
    } else {
        ys[n / 2]
    }
}

fn median_size(chars: &[&Char]) -> f64 {
    let mut sizes: Vec<f64> = chars.iter().map(|c| c.size).collect();
    sizes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = sizes.len();
    if n == 0 {
        return 10.0; // fallback
    }
    if n % 2 == 0 {
        (sizes[n / 2 - 1] + sizes[n / 2]) / 2.0
    } else {
        sizes[n / 2]
    }
}

// ── Post-processing ──────────────────────────────────────────────────────────

/// Collapse multiple consecutive spaces/thin-spaces into a single space,
/// and trim leading/trailing spaces.
fn collapse_spaces(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_was_space = false;
    for c in s.chars() {
        if c == ' ' {
            if !last_was_space {
                out.push(' ');
            }
            last_was_space = true;
        } else {
            out.push(c);
            last_was_space = false;
        }
    }
    out.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfplumber_core::{BBox, TextDirection};

    fn make_char(text: &str, x0: f64, y0: f64, size: f64) -> Char {
        Char {
            text: text.to_owned(),
            bbox: BBox { x0, y0, x1: x0 + size * 0.6, y1: y0 + size },
            fontname: "CMMI10".to_owned(),
            size,
            doctop: y0,
            upright: true,
            direction: TextDirection::Ltr,
            stroking_color: None,
            non_stroking_color: None,
            ctm: [1.0, 0.0, 0.0, 1.0, x0, y0],
            char_code: text.chars().next().unwrap_or(' ') as u32,
            mcid: None,
            tag: None,
        }
    }

    #[test]
    fn empty_returns_empty() {
        assert_eq!(reconstruct_latex(&[]), "");
    }

    #[test]
    fn single_greek_char() {
        let ch = make_char("α", 10.0, 100.0, 10.0);
        let result = reconstruct_latex(&[&ch]);
        assert_eq!(result, "\\alpha");
    }

    #[test]
    fn simple_equation_alpha_plus_beta() {
        let c1 = make_char("α", 10.0, 100.0, 10.0);
        let c2 = make_char("+", 20.0, 100.0, 10.0);
        let c3 = make_char("β", 30.0, 100.0, 10.0);
        let result = reconstruct_latex(&[&c1, &c2, &c3]);
        assert_eq!(result, "\\alpha+\\beta");
    }

    #[test]
    fn superscript_detected() {
        // Base char at y0=100, superscript at y0=90 (above baseline = negative offset in top-left coords)
        let base = make_char("x", 10.0, 100.0, 10.0);
        let sup = make_char("2", 18.0, 93.0, 7.0); // 7 pt below base y0 = 93 is above 100 in PDF coords
        // In PDF top-left: lower y0 = higher on page
        // y_offset = 93 - 100 = -7 → negative → superscript (threshold = 0.3*10=3)
        let result = reconstruct_latex(&[&base, &sup]);
        assert!(result.contains("^{"), "expected superscript: {result}");
    }

    #[test]
    fn subscript_detected() {
        let base = make_char("x", 10.0, 100.0, 10.0);
        // y0=108 > 100 → subscript (offset = 8 > 0.3*10=3)
        let sub = make_char("i", 18.0, 108.0, 7.0);
        let result = reconstruct_latex(&[&base, &sub]);
        assert!(result.contains("_{"), "expected subscript: {result}");
    }

    #[test]
    fn integral_symbol() {
        let ch = make_char("∫", 10.0, 100.0, 12.0);
        let result = reconstruct_latex(&[&ch]);
        assert_eq!(result, "\\int");
    }

    #[test]
    fn sum_and_greek() {
        let c1 = make_char("∑", 10.0, 100.0, 12.0);
        let c2 = make_char("α", 25.0, 100.0, 10.0);
        let result = reconstruct_latex(&[&c1, &c2]);
        assert!(result.contains("\\sum"), "missing sum: {result}");
        assert!(result.contains("\\alpha"), "missing alpha: {result}");
    }

    #[test]
    fn collapse_spaces_works() {
        assert_eq!(collapse_spaces("a  b   c"), "a b c");
        assert_eq!(collapse_spaces("  hello  "), "hello");
    }

    #[test]
    fn ascii_digit_passthrough() {
        let ch = make_char("3", 10.0, 100.0, 10.0);
        let result = reconstruct_latex(&[&ch]);
        assert_eq!(result, "3");
    }
}
