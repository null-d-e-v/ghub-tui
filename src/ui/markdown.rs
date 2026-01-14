use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

pub struct MarkdownRenderer {
    ps: SyntaxSet,
    ts: ThemeSet,
}

impl MarkdownRenderer {
    pub fn new() -> Self {
        Self {
            ps: SyntaxSet::load_defaults_newlines(),
            ts: ThemeSet::load_defaults(),
        }
    }

    pub fn render<'a>(&self, text: &'a str) -> Vec<Line<'a>> {
        let mut lines = Vec::new();
        let mut in_code_block = false;
        let mut current_extension = String::new();

        for line in text.lines() {
            if line.starts_with("```") {
                in_code_block = !in_code_block;
                if in_code_block {
                    current_extension = line.trim_start_matches("```").trim().to_string();
                }
                lines.push(Line::from(Span::styled(line.to_string(), Style::default().fg(Color::DarkGray))));
                continue;
            }

            if in_code_block && !current_extension.is_empty() {
                // Highlight code block
                let syntax = self.ps.find_syntax_by_extension(&current_extension)
                    .unwrap_or_else(|| self.ps.find_syntax_plain_text());
                let mut h = HighlightLines::new(syntax, &self.ts.themes["base16-ocean.dark"]);
                
                // Use a fixed line with newline to avoid temporary issue
                let line_with_nl = format!("{}\n", line);
                if let Ok(highlighted) = h.highlight_line(&line_with_nl, &self.ps) {
                    let spans: Vec<Span> = highlighted.into_iter().map(|(style, content)| {
                        Span::styled(content.trim_end_matches('\n').to_string(), convert_style(style))
                    }).collect();
                    lines.push(Line::from(spans));
                } else {
                    lines.push(Line::from(line.to_string()));
                }
            } else if line.starts_with("# ") {
               lines.push(Line::from(Span::styled(line.to_string(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))));
            } else if line.starts_with("## ") {
               lines.push(Line::from(Span::styled(line.to_string(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))));
            } else {
                lines.push(Line::from(line.to_string()));
            }
        }
        lines
    }
}

fn convert_style(style: syntect::highlighting::Style) -> Style {
    let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
    Style::default().fg(fg)
}
