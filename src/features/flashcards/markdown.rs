use pulldown_cmark::{html, Options, Parser};
use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

fn get_display_math_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\\\[(?s)(.*?)\\\]").unwrap())
}

fn get_inline_math_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\\\((?s)(.*?)\\\)").unwrap())
}

/// Convert markdown to HTML, preserving LaTeX math expressions.
///
/// This function:
/// 1. Protects LaTeX math expressions (\[...\] and \(...\)) from markdown processing
/// 2. Converts markdown to HTML using pulldown-cmark
/// 3. Restores the LaTeX expressions in the final HTML
///
/// The LaTeX expressions are preserved so they can be rendered by MathJax or similar
/// LaTeX rendering engines in the final display environment.
pub fn markdown_to_html(markdown: &str) -> String {
    // Step 1: Extract and protect LaTeX math expressions
    let mut math_map = HashMap::new();
    let mut counter = 0;

    // Protect display math \[ ... \] first (do this before inline to avoid conflicts)
    let display_math_re = get_display_math_regex();
    let protected_text = display_math_re.replace_all(markdown, |caps: &regex::Captures| {
        let placeholder = format!("DISPLAYMATH{}", counter);
        math_map.insert(placeholder.clone(), caps[0].to_string());
        counter += 1;
        placeholder
    });

    // Protect inline math \( ... \)
    let inline_math_re = get_inline_math_regex();
    let protected_text = inline_math_re.replace_all(&protected_text, |caps: &regex::Captures| {
        let placeholder = format!("INLINEMATH{}", counter);
        math_map.insert(placeholder.clone(), caps[0].to_string());
        counter += 1;
        placeholder
    });

    // Step 2: Convert markdown to HTML
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(&protected_text, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    // Step 3: Restore LaTeX math expressions
    for (placeholder, original) in math_map {
        html_output = html_output.replace(&placeholder, &original);
    }

    html_output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_markdown() {
        let input = "**bold** and *italic*";
        let output = markdown_to_html(input);
        assert!(output.contains("<strong>bold</strong>"));
        assert!(output.contains("<em>italic</em>"));
    }

    #[test]
    fn test_latex_display_math() {
        let input = r"Some text \[ x^2 + y^2 = z^2 \] more text";
        let output = markdown_to_html(input);
        assert!(output.contains(r"\[ x^2 + y^2 = z^2 \]"));
    }

    #[test]
    fn test_latex_inline_math() {
        let input = r"The formula \( a^2 \) is important";
        let output = markdown_to_html(input);
        assert!(output.contains(r"\( a^2 \)"));
    }

    #[test]
    fn test_mixed_markdown_and_latex() {
        let input = r"**Theorem:** The equation \[ E = mc^2 \] shows *energy-mass* equivalence.";
        let output = markdown_to_html(input);
        assert!(output.contains("<strong>Theorem:</strong>"));
        assert!(output.contains(r"\[ E = mc^2 \]"));
        assert!(output.contains("<em>energy-mass</em>"));
    }

    #[test]
    fn test_code_blocks() {
        let input = "```rust\nfn main() {}\n```";
        let output = markdown_to_html(input);
        // Code blocks generate <pre><code> structure
        assert!(output.contains("<pre>") || output.contains("<code>"));
    }

    #[test]
    fn test_tables() {
        let input = "| A | B |\n|---|---|\n| 1 | 2 |";
        let output = markdown_to_html(input);
        assert!(output.contains("<table>"));
    }
}
