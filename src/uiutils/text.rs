use derive_new::new;
use ratatui::style::Style;

#[derive(Debug, PartialEq, Clone, Eq, Hash, new)]
pub struct StyledText {
    pub content: String,
    pub style: Style,
}

impl StyledText {
    fn is_empty_render(&self) -> bool {
        self.content == " " || self.content.is_empty()
    }
}

impl From<String> for StyledText {
    fn from(content: String) -> Self {
        Self::new(content, Default::default())
    }
}

#[derive(Debug, PartialEq, Clone, Eq, Hash, new)]
pub struct StyledLine {
    pub content: Vec<StyledText>,
}

impl StyledLine {
    fn is_empty_render(&self) -> bool {
        self.content.is_empty() || self.content.iter().all(|t| t.is_empty_render())
    }
}

#[derive(Debug, PartialEq, Clone, Eq, Hash, new)]
pub struct StyledParagraph {
    pub lines: Vec<StyledLine>,
    pub style: Style,
    pub highlighted_style: Style,
}

pub fn default_highlight_style() -> Style {
    Style::default().bg(if crate::uiutils::dark_mode::is_dark_mode() {
        let gray = 88_u8;
        ratatui::style::Color::Rgb(gray, gray, gray)
    } else {
        ratatui::style::Color::Gray
    })
}

impl From<Vec<StyledLine>> for StyledParagraph {
    fn from(lines: Vec<StyledLine>) -> Self {
        Self::new(lines, Default::default(), default_highlight_style())
    }
}

impl From<StyledLine> for StyledParagraph {
    fn from(line: StyledLine) -> Self {
        Self::new(vec![line], Default::default(), default_highlight_style())
    }
}

impl<T> FromIterator<T> for StyledParagraph
where
    T: Into<StyledLine>,
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self::from(iter.into_iter().map(Into::into).collect::<Vec<_>>())
    }
}

impl StyledParagraph {
    pub fn empty() -> Self {
        Self::from(StyledLine::from(" "))
    }

    pub fn append(&mut self, content: &mut Vec<StyledLine>) {
        self.lines.append(content)
    }

    pub fn lines(&self) -> impl Iterator<Item = ratatui::text::Line> {
        self.lines.iter().map(ratatui::text::Line::from)
    }

    pub fn is_empty_render(&self) -> bool {
        if self.lines.is_empty() {
            true
        } else if self.lines.len() == 1 {
            self.lines[0].is_empty_render()
        } else {
            false
        }
    }
}

impl From<StyledText> for ratatui::text::Span<'_> {
    fn from(value: StyledText) -> Self {
        Self::styled(value.content, value.style)
    }
}

impl<'a> From<&'a StyledText> for ratatui::text::Span<'a> {
    fn from(value: &'a StyledText) -> Self {
        Self::from(&value.content).style(value.style)
    }
}

impl From<StyledLine> for ratatui::text::Line<'_> {
    fn from(value: StyledLine) -> Self {
        Self::from(
            value
                .content
                .into_iter()
                .map(ratatui::text::Span::from)
                .collect::<Vec<_>>(),
        )
    }
}

impl<'a> From<&'a StyledLine> for ratatui::text::Line<'a> {
    fn from(value: &'a StyledLine) -> Self {
        Self::from(
            value
                .content
                .iter()
                .map(ratatui::text::Span::from)
                .collect::<Vec<_>>(),
        )
    }
}

impl From<String> for StyledLine {
    fn from(s: String) -> Self {
        Self::new(vec![StyledText::from(s)])
    }
}

impl<'a> From<&'a str> for StyledLine {
    fn from(s: &'a str) -> Self {
        Self::from(s.to_owned())
    }
}

impl From<Vec<StyledText>> for StyledLine {
    fn from(spans: Vec<StyledText>) -> Self {
        Self::new(spans)
    }
}

impl From<StyledText> for StyledLine {
    fn from(span: StyledText) -> Self {
        Self::from(vec![span])
    }
}

impl From<StyledLine> for String {
    fn from(line: StyledLine) -> Self {
        line.content.iter().fold(Self::new(), |mut acc, s| {
            acc.push_str(&s.content);
            acc
        })
    }
}

impl<T> FromIterator<T> for StyledLine
where
    T: Into<StyledText>,
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self::from(iter.into_iter().map(Into::into).collect::<Vec<_>>())
    }
}

impl<'a> From<&'a StyledParagraph> for ratatui::widgets::Paragraph<'a> {
    fn from(value: &'a StyledParagraph) -> Self {
        Self::new(
            value
                .lines
                .iter()
                .map(ratatui::text::Line::from)
                .collect::<Vec<_>>(),
        )
        .style(value.style)
    }
}
