use derive_new::new;
use ratatui::prelude::Stylize;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::style::Styled;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

fn markdown_parse_options() -> markdown::ParseOptions {
    markdown::ParseOptions {
        constructs: markdown::Constructs {
            attention: true,
            autolink: false,
            block_quote: false,
            character_escape: true,
            character_reference: false,
            code_indented: false,
            code_fenced: true,
            code_text: true,
            definition: false,
            frontmatter: false,
            gfm_autolink_literal: false,
            gfm_label_start_footnote: false,
            gfm_footnote_definition: false,
            gfm_strikethrough: false,
            gfm_table: false,
            gfm_task_list_item: false,
            hard_break_escape: false,
            hard_break_trailing: false,
            heading_atx: false,
            heading_setext: false,
            html_flow: false,
            html_text: false,
            label_start_image: false,
            label_start_link: false,
            label_end: false,
            list_item: false,
            math_flow: false,
            math_text: false,
            mdx_esm: false,
            mdx_expression_flow: false,
            mdx_expression_text: false,
            mdx_jsx_flow: false,
            mdx_jsx_text: false,
            thematic_break: false,
        },
        gfm_strikethrough_single_tilde: false,
        math_text_single_dollar: false,
        ..Default::default()
    }
}

pub fn parse_markdown<'a>(message: String) -> Vec<MarkdownContent<'a>> {
    let root_node = markdown::to_mdast(&message, &markdown_parse_options()).unwrap();
    log::debug!("Parsed markdown {:#?}", root_node);
    let mut result: Vec<MarkdownContent> = Default::default();
    process_markdown(root_node, &Default::default(), &mut result);

    result
}

#[derive(PartialEq, Clone, Copy, Eq, Hash)]
enum TextModifier {
    Strong,
    InlineCode,
}

#[derive(Debug, PartialEq, Clone, Eq, Hash, new)]
pub struct StyledText {
    content: String,
    style: Style,
}

#[derive(Debug, PartialEq, Clone, Eq, Hash, new)]
struct Code {
    content: String,
    language: Option<String>,
}

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub enum MarkdownContent<'a> {
    StyledText(StyledText),
    Code(Vec<Paragraph<'a>>),
}

impl<'a> MarkdownContent<'a> {
    pub fn into_paragraphs(value: Vec<MarkdownContent<'a>>) -> Vec<Paragraph<'a>> {
        let mut all_paragraphs: Vec<Paragraph> = vec![];
        let mut all_lines: Vec<ratatui::prelude::Line> = vec![];
        let mut paragraph_line: Vec<Span> = vec![];

        fn push_line<'a>(
            all_lines: &mut Vec<ratatui::prelude::Line<'a>>,
            paragraph_line: &mut Vec<Span<'a>>,
        ) {
            if paragraph_line.is_empty() {
                return;
            }
            all_lines.push(Line::from(paragraph_line.clone()));
            paragraph_line.clear();
        }

        fn push_paragraph<'a>(
            all_paragraphs: &mut Vec<Paragraph<'a>>,
            all_lines: &mut Vec<ratatui::prelude::Line<'a>>,
        ) {
            if all_lines.is_empty() {
                return;
            }
            all_paragraphs.push(Paragraph::new(all_lines.clone()));
            all_lines.clear();
        }

        for markdown in value.into_iter() {
            match markdown {
                Self::StyledText(styled_text) => {
                    for line in styled_text.content.split_inclusive("\n") {
                        let has_newline = line.contains("\n");
                        let line = line.strip_suffix('\n').unwrap_or(line);
                        let line = line.strip_suffix('\r').unwrap_or(line);

                        paragraph_line.push(Span::styled(line.to_owned(), styled_text.style));
                        if has_newline {
                            push_line(&mut all_lines, &mut paragraph_line);
                        }
                    }
                }
                Self::Code(mut code) => {
                    push_line(&mut all_lines, &mut paragraph_line);
                    push_paragraph(&mut all_paragraphs, &mut all_lines);
                    all_paragraphs.append(&mut code);
                }
            }
        }
        push_line(&mut all_lines, &mut paragraph_line);
        push_paragraph(&mut all_paragraphs, &mut all_lines);

        all_paragraphs
    }
}

#[derive(new)]
struct HighlightResult<'a> {
    lines: Vec<Line<'a>>,
    bg: ratatui::style::Color,
}

fn highlight_syntax<'a>(language: Option<String>, content: String) -> HighlightResult<'a> {
    let syntax_set = SyntaxSet::load_defaults_newlines();
    let theme_set = ThemeSet::load_defaults();
    let empty_vec: Vec<&str> = vec![];
    let extensions = language
        .clone()
        .and_then(|lang| crate::utils::language_extensions::LANGUAGE_EXTENSIONS.get(&lang))
        .unwrap_or(&empty_vec);

    let syntax = extensions
        .iter()
        .find_map(|ext| syntax_set.find_syntax_by_extension(ext))
        .unwrap_or(syntax_set.find_syntax_plain_text());
    log::debug!("Highlighting {:#?}: {:#?}", extensions, syntax.name);

    let mut h = HighlightLines::new(syntax, &theme_set.themes["base16-ocean.dark"]);
    let mut bg = ratatui::style::Color::DarkGray;
    let lines = LinesWithEndings::from(&content)
        .map(|line| {
            let ranges = h.highlight_line(line, &syntax_set).unwrap_or_default();
            let styled_text = ranges.into_iter().map(|(style, content)| {
                bg = Color::Rgb(style.background.r, style.background.g, style.background.b);
                StyledText::new(
                    content.to_string(),
                    Style::default().fg(Color::Rgb(
                        style.foreground.r,
                        style.foreground.g,
                        style.foreground.b,
                    )),
                )
            });

            Line::from(
                styled_text
                    .map(ratatui::prelude::Span::from)
                    .collect::<Vec<_>>(),
            )
        })
        .collect();

    HighlightResult::new(lines, bg)
}

impl<'a> From<StyledText> for ratatui::prelude::Span<'a> {
    fn from(value: StyledText) -> Self {
        Span::styled(value.content, value.style)
    }
}

fn process_markdown(
    node: markdown::mdast::Node,
    modifiers: &std::collections::HashSet<TextModifier>,
    result: &mut Vec<MarkdownContent>,
) {
    let process_node = { |n| process_markdown(n, modifiers, result) };
    match node {
        markdown::mdast::Node::Root(n) => n.children.into_iter().for_each(process_node),
        markdown::mdast::Node::Paragraph(n) => {
            n.children.into_iter().for_each(process_node);
            result.push(MarkdownContent::StyledText(StyledText::new(
                "\n\n".to_string(),
                Default::default(),
            )));
        }
        markdown::mdast::Node::Code(n) => {
            let mut all_paragraphs: Vec<Paragraph> = vec![];
            let mut lines: Vec<Line> = vec![];
            lines.push(Line::from(
                n.lang
                    .clone()
                    .map_or("```".to_string(), |lang| "```".to_string() + &lang),
            ));
            lines.push(Line::from("\n"));
            let mut highlighted_code = highlight_syntax(n.lang, n.value);
            lines.append(&mut highlighted_code.lines);
            lines.push(Line::from("\n"));
            lines.push(Line::from("```"));

            all_paragraphs.push(
                Paragraph::new(lines).set_style(Style::default().gray().bg(highlighted_code.bg)),
            );
            all_paragraphs.push(Paragraph::new("\n"));
            result.push(MarkdownContent::Code(all_paragraphs))
        }
        markdown::mdast::Node::InlineCode(n) => {
            result.push(MarkdownContent::StyledText(process_text(
                n.value,
                &modifiers
                    .union(&maplit::hashset! {TextModifier::InlineCode})
                    .cloned()
                    .collect(),
            )))
        }
        markdown::mdast::Node::Text(text) => result.push(MarkdownContent::StyledText(
            process_text(text.value, modifiers),
        )),
        markdown::mdast::Node::Emphasis(n) => n.children.into_iter().for_each(|child| {
            process_markdown(
                child,
                &modifiers
                    .union(&maplit::hashset! {TextModifier::Strong})
                    .cloned()
                    .collect(),
                result,
            )
        }),
        markdown::mdast::Node::Strong(n) => n.children.into_iter().for_each(|child| {
            process_markdown(
                child,
                &modifiers
                    .union(&maplit::hashset! {TextModifier::Strong})
                    .cloned()
                    .collect(),
                result,
            )
        }),
        _ => (),
    }
}

fn process_text(text: String, modifiers: &std::collections::HashSet<TextModifier>) -> StyledText {
    let mut span_style = Style::default();
    if modifiers.contains(&TextModifier::Strong) {
        span_style = span_style.bold();
    }
    let mut text = text;
    if modifiers.contains(&TextModifier::InlineCode) {
        text = "`".to_string() + &text + "`";
        span_style = span_style.blue().italic();
    }
    StyledText {
        content: text,
        style: span_style,
    }
}
