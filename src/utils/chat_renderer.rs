use crate::uiutils::dark_mode::is_dark_mode;
use crate::uiutils::text::default_highlight_style;
use crate::uiutils::text::StyledLine;
use crate::uiutils::text::StyledParagraph;
use crate::uiutils::text::StyledText;
use ratatui::prelude::Stylize;
use ratatui::style::Color;
use ratatui::style::Style;
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

pub fn parse_markdown(message: String) -> Vec<IntermediateMarkdownPassResult> {
    let root_node = markdown::to_mdast(&message, &markdown_parse_options()).unwrap();
    log::debug!("Parsed markdown {:#?}", root_node);
    let mut result: Vec<IntermediateMarkdownPassResult> = Default::default();
    process_markdown(root_node, &Default::default(), &mut result);

    result
}

#[derive(PartialEq, Clone, Copy, Eq, Hash)]
enum TextModifier {
    Strong,
    InlineCode,
}

pub enum IntermediateMarkdownPassResult {
    StyledText(StyledText),
    Code(Vec<StyledParagraph>),
}

impl IntermediateMarkdownPassResult {
    pub fn into_paragraphs(value: Vec<IntermediateMarkdownPassResult>) -> Vec<StyledParagraph> {
        let mut all_paragraphs: Vec<StyledParagraph> = vec![];
        let mut all_lines: Vec<StyledLine> = vec![];
        let mut paragraph_line: Vec<StyledText> = vec![];

        fn collect_into<F, T>(to: &mut Vec<T>, from: &mut Vec<F>)
        where
            T: FromIterator<F>,
        {
            if from.is_empty() {
                return;
            }
            to.push(T::from_iter(from.drain(..)));
        }

        for markdown in value.into_iter() {
            match markdown {
                Self::StyledText(styled_text) => {
                    for line in styled_text.content.split_inclusive("\n") {
                        let has_newline = line.contains("\n");
                        let line = line.strip_suffix('\n').unwrap_or(line);
                        let line = line.strip_suffix('\r').unwrap_or(line);

                        if line.is_empty() && paragraph_line.is_empty() {
                            collect_into(&mut all_paragraphs, &mut all_lines);
                            all_paragraphs.push(StyledParagraph::empty());
                            continue;
                        }

                        if paragraph_line.is_empty() || !line.is_empty() {
                            paragraph_line
                                .push(StyledText::new(line.to_owned(), styled_text.style));
                        }
                        if has_newline {
                            collect_into(&mut all_lines, &mut paragraph_line);
                        }
                    }
                }
                Self::Code(mut code) => {
                    collect_into(&mut all_lines, &mut paragraph_line);
                    collect_into(&mut all_paragraphs, &mut all_lines);
                    all_paragraphs.append(&mut code);
                }
            }
        }
        collect_into(&mut all_lines, &mut paragraph_line);
        collect_into(&mut all_paragraphs, &mut all_lines);
        all_paragraphs.push(StyledParagraph::empty());

        all_paragraphs
    }
}

fn highlight_syntax(language: Option<String>, content: String) -> StyledParagraph {
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
            StyledLine::new(styled_text.collect())
        })
        .collect();
    let highlight_style = if is_dark_mode() {
        default_highlight_style()
    } else {
        Style::default().bg(ratatui::style::Color::DarkGray)
    };

    StyledParagraph::new(lines, Style::default().bg(bg), highlight_style)
}

fn process_markdown(
    node: markdown::mdast::Node,
    modifiers: &std::collections::HashSet<TextModifier>,
    result: &mut Vec<IntermediateMarkdownPassResult>,
) {
    let process_node = { |n| process_markdown(n, modifiers, result) };
    match node {
        markdown::mdast::Node::Root(n) => n.children.into_iter().for_each(process_node),
        markdown::mdast::Node::Paragraph(n) => {
            n.children.into_iter().for_each(process_node);
            result.push(IntermediateMarkdownPassResult::StyledText(StyledText::new(
                "\n\n".to_string(),
                Default::default(),
            )));
        }
        markdown::mdast::Node::Code(n) => {
            let all_paragraphs = vec![
                // Top fence + lang id
                StyledParagraph::from(vec![StyledLine::from(
                    n.lang
                        .clone()
                        .map_or("```".to_string(), |lang| "```".to_string() + &lang),
                )]),
                // Code contents
                highlight_syntax(n.lang, n.value),
                // Bottom fence
                StyledParagraph::from(StyledLine::from("```")),
                // Padding newline should be in separate paragraph to properly support highlight!
                StyledParagraph::from(StyledLine::from(" ")),
            ];

            result.push(IntermediateMarkdownPassResult::Code(all_paragraphs))
        }
        markdown::mdast::Node::InlineCode(n) => {
            result.push(IntermediateMarkdownPassResult::StyledText(process_text(
                n.value,
                &modifiers
                    .iter()
                    .copied()
                    .chain(std::iter::once(TextModifier::InlineCode))
                    .collect(),
            )))
        }
        markdown::mdast::Node::Text(text) => result.push(
            IntermediateMarkdownPassResult::StyledText(process_text(text.value, modifiers)),
        ),
        markdown::mdast::Node::Emphasis(n) => n.children.into_iter().for_each(|child| {
            process_markdown(
                child,
                &modifiers
                    .iter()
                    .copied()
                    .chain(std::iter::once(TextModifier::Strong))
                    .collect(),
                result,
            )
        }),
        markdown::mdast::Node::Strong(n) => n.children.into_iter().for_each(|child| {
            process_markdown(
                child,
                &modifiers
                    .iter()
                    .copied()
                    .chain(std::iter::once(TextModifier::Strong))
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
