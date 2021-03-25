use crate::compositor::{Component, Compositor, Context, EventResult};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use tui::{
    buffer::Buffer as Surface,
    layout::Rect,
    style::{Color, Style},
    text::Text,
};

use std::borrow::Cow;

use helix_core::Position;
use helix_view::{Editor, Theme};

pub struct Markdown {
    contents: String,
}

// TODO: pre-render and self reference via Pin
// better yet, just use Tendril + subtendril for references

impl Markdown {
    pub fn new(contents: String) -> Self {
        Self { contents }
    }
}

fn parse<'a>(contents: &'a str, theme: Option<&Theme>) -> tui::text::Text<'a> {
    use pulldown_cmark::{CodeBlockKind, CowStr, Event, Options, Parser, Tag};
    use tui::text::{Span, Spans, Text};

    // also 2021-03-04T16:33:58.553 helix_lsp::transport [INFO] <- {"contents":{"kind":"markdown","value":"\n```rust\ncore::num\n```\n\n```rust\npub const fn saturating_sub(self, rhs:Self) ->Self\n```\n\n---\n\n```rust\n```"},"range":{"end":{"character":61,"line":101},"start":{"character":47,"line":101}}}
    let text = "\n```rust\ncore::iter::traits::iterator::Iterator\n```\n\n```rust\nfn collect<B: FromIterator<Self::Item>>(self) -> B\nwhere\n        Self: Sized,\n```\n\n---\n\nTransforms an iterator into a collection.\n\n`collect()` can take anything iterable, and turn it into a relevant\ncollection. This is one of the more powerful methods in the standard\nlibrary, used in a variety of contexts.\n\nThe most basic pattern in which `collect()` is used is to turn one\ncollection into another. You take a collection, call [`iter`](https://doc.rust-lang.org/nightly/core/iter/traits/iterator/trait.Iterator.html) on it,\ndo a bunch of transformations, and then `collect()` at the end.\n\n`collect()` can also create instances of types that are not typical\ncollections. For example, a [`String`](https://doc.rust-lang.org/nightly/core/iter/std/string/struct.String.html) can be built from [`char`](type@char)s,\nand an iterator of [`Result<T, E>`](https://doc.rust-lang.org/nightly/core/result/enum.Result.html) items can be collected\ninto `Result<Collection<T>, E>`. See the examples below for more.\n\nBecause `collect()` is so general, it can cause problems with type\ninference. As such, `collect()` is one of the few times you'll see\nthe syntax affectionately known as the 'turbofish': `::<>`. This\nhelps the inference algorithm understand specifically which collection\nyou're trying to collect into.\n\n# Examples\n\nBasic usage:\n\n```rust\nlet a = [1, 2, 3];\n\nlet doubled: Vec<i32> = a.iter()\n                         .map(|&x| x * 2)\n                         .collect();\n\nassert_eq!(vec![2, 4, 6], doubled);\n```\n\nNote that we needed the `: Vec<i32>` on the left-hand side. This is because\nwe could collect into, for example, a [`VecDeque<T>`](https://doc.rust-lang.org/nightly/core/iter/std/collections/struct.VecDeque.html) instead:\n\n```rust\nuse std::collections::VecDeque;\n\nlet a = [1, 2, 3];\n\nlet doubled: VecDeque<i32> = a.iter().map(|&x| x * 2).collect();\n\nassert_eq!(2, doubled[0]);\nassert_eq!(4, doubled[1]);\nassert_eq!(6, doubled[2]);\n```\n\nUsing the 'turbofish' instead of annotating `doubled`:\n\n```rust\nlet a = [1, 2, 3];\n\nlet doubled = a.iter().map(|x| x * 2).collect::<Vec<i32>>();\n\nassert_eq!(vec![2, 4, 6], doubled);\n```\n\nBecause `collect()` only cares about what you're collecting into, you can\nstill use a partial type hint, `_`, with the turbofish:\n\n```rust\nlet a = [1, 2, 3];\n\nlet doubled = a.iter().map(|x| x * 2).collect::<Vec<_>>();\n\nassert_eq!(vec![2, 4, 6], doubled);\n```\n\nUsing `collect()` to make a [`String`](https://doc.rust-lang.org/nightly/core/iter/std/string/struct.String.html):\n\n```rust\nlet chars = ['g', 'd', 'k', 'k', 'n'];\n\nlet hello: String = chars.iter()\n    .map(|&x| x as u8)\n    .map(|x| (x + 1) as char)\n    .collect();\n\nassert_eq!(\"hello\", hello);\n```\n\nIf you have a list of [`Result<T, E>`](https://doc.rust-lang.org/nightly/core/result/enum.Result.html)s, you can use `collect()` to\nsee if any of them failed:\n\n```rust\nlet results = [Ok(1), Err(\"nope\"), Ok(3), Err(\"bad\")];\n\nlet result: Result<Vec<_>, &str> = results.iter().cloned().collect();\n\n// gives us the first error\nassert_eq!(Err(\"nope\"), result);\n\nlet results = [Ok(1), Ok(3)];\n\nlet result: Result<Vec<_>, &str> = results.iter().cloned().collect();\n\n// gives us the list of answers\nassert_eq!(Ok(vec![1, 3]), result);\n```";

    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(contents, options);

    // TODO: if possible, render links as terminal hyperlinks: https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda
    let mut tags = Vec::new();
    let mut spans = Vec::new();
    let mut lines = Vec::new();

    fn to_span(text: pulldown_cmark::CowStr) -> Span {
        use std::ops::Deref;
        Span::raw::<std::borrow::Cow<_>>(match text {
            CowStr::Borrowed(s) => s.to_string().into(), // could retain borrow
            CowStr::Boxed(s) => s.to_string().into(),
            CowStr::Inlined(s) => s.deref().to_owned().into(),
        })
    }

    let text_style = Style::default().fg(Color::Rgb(164, 160, 232)); // lavender
    let code_style = Style::default().fg(Color::Rgb(255, 255, 255)); // white
    let heading_style = Style::default().fg(Color::Rgb(219, 191, 239)); // lilac

    for event in parser {
        match event {
            Event::Start(tag) => tags.push(tag),
            Event::End(tag) => {
                tags.pop();
                match tag {
                    Tag::Heading(_) | Tag::Paragraph | Tag::CodeBlock(CodeBlockKind::Fenced(_)) => {
                        // whenever code block or paragraph closes, new line
                        let spans = std::mem::replace(&mut spans, Vec::new());
                        if !spans.is_empty() {
                            lines.push(Spans::from(spans));
                        }
                        lines.push(Spans::default());
                    }
                    _ => (),
                }
            }
            Event::Text(text) => {
                // TODO: temp workaround
                if let Some(Tag::CodeBlock(CodeBlockKind::Fenced(language))) = tags.last() {
                    if let Some(theme) = theme {
                        use helix_core::syntax::{self, HighlightEvent, Syntax};
                        use helix_core::Rope;

                        let rope = Rope::from(text.as_ref());
                        let syntax = syntax::LOADER
                            .get()
                            .unwrap()
                            .language_config_for_scope(&format!("source.{}", language))
                            .and_then(|config| config.highlight_config(theme.scopes()))
                            .map(|config| Syntax::new(&rope, config));

                        if let Some(mut syntax) = syntax {
                            // if we have a syntax available, highlight_iter and generate spans
                            let mut highlights = Vec::new();

                            for event in syntax.highlight_iter(rope.slice(..), None, None, |_| None)
                            {
                                match event.unwrap() {
                                    HighlightEvent::HighlightStart(span) => {
                                        highlights.push(span);
                                    }
                                    HighlightEvent::HighlightEnd => {
                                        highlights.pop();
                                    }
                                    HighlightEvent::Source { start, end } => {
                                        let style = match highlights.first() {
                                            Some(span) => {
                                                theme.get(theme.scopes()[span.0].as_str())
                                            }
                                            None => Style::default().fg(Color::Rgb(164, 160, 232)), // lavender
                                        };

                                        let mut slice = &text[start..end];
                                        while let Some(end) = slice.find('\n') {
                                            // emit span up to newline
                                            let text = &slice[..end];
                                            let span = Span::styled(text.to_owned(), style);
                                            spans.push(span);

                                            // truncate slice to after newline
                                            slice = &slice[end + 1..];

                                            // make a new line
                                            let spans = std::mem::replace(&mut spans, Vec::new());
                                            lines.push(Spans::from(spans));
                                        }

                                        // if there's anything left, emit it too
                                        if !slice.is_empty() {
                                            let span = Span::styled(slice.to_owned(), style);
                                            spans.push(span);
                                        }
                                    }
                                }
                            }
                        } else {
                            for line in text.lines() {
                                let mut span = Span::styled(line.to_string(), code_style);
                                lines.push(Spans::from(span));
                            }
                        }
                    } else {
                        for line in text.lines() {
                            let mut span = Span::styled(line.to_string(), code_style);
                            lines.push(Spans::from(span));
                        }
                    }
                } else if let Some(Tag::Heading(_)) = tags.last() {
                    let mut span = to_span(text);
                    span.style = heading_style;
                    spans.push(span);
                } else {
                    let mut span = to_span(text);
                    span.style = text_style;
                    spans.push(span);
                }
            }
            Event::Code(text) | Event::Html(text) => {
                let mut span = to_span(text);
                span.style = code_style;
                spans.push(span);
            }
            Event::SoftBreak | Event::HardBreak => {
                // let spans = std::mem::replace(&mut spans, Vec::new());
                // lines.push(Spans::from(spans));
                spans.push(Span::raw(" "));
            }
            Event::Rule => {
                lines.push(Spans::from("---"));
                lines.push(Spans::default());
            }
            // TaskListMarker(bool) true if checked
            _ => (),
        }
        // build up a vec of Paragraph tui widgets
    }

    if !spans.is_empty() {
        lines.push(Spans::from(spans));
    }

    // if last line is empty, remove it
    if let Some(line) = lines.last() {
        if line.0.is_empty() {
            lines.pop();
        }
    }

    Text::from(lines)
}
impl Component for Markdown {
    fn render(&self, area: Rect, surface: &mut Surface, cx: &mut Context) {
        use tui::widgets::{Paragraph, Widget, Wrap};

        let text = parse(&self.contents, Some(&cx.editor.theme));

        let par = Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .scroll((cx.scroll.unwrap_or_default() as u16, 0));

        let area = Rect::new(area.x + 1, area.y + 1, area.width - 2, area.height - 2);
        par.render(area, surface);
    }

    fn required_size(&mut self, viewport: (u16, u16)) -> Option<(u16, u16)> {
        let contents = parse(&self.contents, None);
        let padding = 2;
        let width = std::cmp::min(contents.width() as u16 + padding, viewport.0);
        let height = std::cmp::min(contents.height() as u16 + padding, viewport.1);
        Some((width, height))
    }
}
