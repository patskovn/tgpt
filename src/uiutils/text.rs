use derive_new::new;
use ratatui::style::Style;

#[derive(Debug, PartialEq, Clone, Eq, Hash, new)]
pub struct StyledText {
    pub content: String,
    pub style: Style,
}

impl From<String> for StyledText {
    fn from(content: String) -> Self {
        Self::new(content, Default::default())
    }
}

#[derive(Debug, PartialEq, Clone, Eq, Hash, new)]
pub struct StyledLine {
    content: Vec<StyledText>,
}

#[derive(Debug, PartialEq, Clone, Eq, Hash, new)]
pub struct StyledParagraph {
    pub lines: Vec<StyledLine>,
    pub style: Style,
}

impl From<Vec<StyledLine>> for StyledParagraph {
    fn from(lines: Vec<StyledLine>) -> Self {
        Self::new(lines, Default::default())
    }
}

impl From<StyledLine> for StyledParagraph {
    fn from(line: StyledLine) -> Self {
        Self::new(vec![line], Default::default())
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
    pub fn append(&mut self, content: &mut Vec<StyledLine>) {
        self.lines.append(content)
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
