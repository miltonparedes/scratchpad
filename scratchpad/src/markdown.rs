use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Context, Result};
use ansi_to_tui::IntoText as _;
use ratatui::{
    layout::Alignment,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};
use ratatui_core::{layout as core_layout, style as core_style, text as core_text};

pub fn render_markdown(content: &str, width: u16) -> Result<Text<'static>> {
    if content.trim().is_empty() {
        return Ok(Text::from(""));
    }

    let width = width.max(20);
    let mut child = Command::new("glow")
        .args([
            "-s",
            "auto",
            "-w",
            &width.to_string(),
            "-n",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn glow")?;

    {
        let mut stdin = child.stdin.take().context("Failed to open glow stdin")?;
        stdin
            .write_all(content.as_bytes())
            .context("Failed to write to glow stdin")?;
    }

    let output = child.wait_with_output().context("Failed to read glow output")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let msg = stderr.trim();
        let msg = if msg.is_empty() { "glow failed" } else { msg };
        return Err(anyhow!("{}", msg));
    }

    let text = output
        .stdout
        .into_text()
        .context("Failed to parse ANSI output from glow")?;

    Ok(convert_text(text))
}

fn convert_text(text: core_text::Text<'static>) -> Text<'static> {
    let lines = text.lines.into_iter().map(convert_line).collect();
    Text {
        alignment: text.alignment.map(convert_alignment),
        style: convert_style(text.style),
        lines,
    }
}

fn convert_line(line: core_text::Line<'static>) -> Line<'static> {
    Line {
        style: convert_style(line.style),
        alignment: line.alignment.map(convert_alignment),
        spans: line.spans.into_iter().map(convert_span).collect(),
    }
}

fn convert_span(span: core_text::Span<'static>) -> Span<'static> {
    Span {
        style: convert_style(span.style),
        content: span.content.into_owned().into(),
    }
}

fn convert_style(style: core_style::Style) -> Style {
    Style {
        fg: style.fg.map(convert_color),
        bg: style.bg.map(convert_color),
        add_modifier: convert_modifier(style.add_modifier),
        sub_modifier: convert_modifier(style.sub_modifier),
        ..Style::default()
    }
}

fn convert_color(color: core_style::Color) -> Color {
    match color {
        core_style::Color::Reset => Color::Reset,
        core_style::Color::Black => Color::Black,
        core_style::Color::Red => Color::Red,
        core_style::Color::Green => Color::Green,
        core_style::Color::Yellow => Color::Yellow,
        core_style::Color::Blue => Color::Blue,
        core_style::Color::Magenta => Color::Magenta,
        core_style::Color::Cyan => Color::Cyan,
        core_style::Color::Gray => Color::Gray,
        core_style::Color::DarkGray => Color::DarkGray,
        core_style::Color::LightRed => Color::LightRed,
        core_style::Color::LightGreen => Color::LightGreen,
        core_style::Color::LightYellow => Color::LightYellow,
        core_style::Color::LightBlue => Color::LightBlue,
        core_style::Color::LightMagenta => Color::LightMagenta,
        core_style::Color::LightCyan => Color::LightCyan,
        core_style::Color::White => Color::White,
        core_style::Color::Indexed(idx) => Color::Indexed(idx),
        core_style::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

fn convert_modifier(modifier: core_style::Modifier) -> Modifier {
    Modifier::from_bits_truncate(modifier.bits())
}

fn convert_alignment(alignment: core_layout::Alignment) -> Alignment {
    match alignment {
        core_layout::Alignment::Left => Alignment::Left,
        core_layout::Alignment::Center => Alignment::Center,
        core_layout::Alignment::Right => Alignment::Right,
    }
}
