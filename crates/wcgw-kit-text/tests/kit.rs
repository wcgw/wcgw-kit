//! Renderings as wcgw-kit documents: what `<CR>` on a Link's cells opens, and code coloured by the application.

use ratatui_core::style::{Color, Style};
use wcgw_kit::doc::{Cursor, Layout};
use wcgw_kit_text::{CellSize, Document, Options, Rendering, Styles, TextFormat, kit};

const WIDTH: u16 = 16;

fn render(document: &Document) -> Rendering {
    document.render(&Options {
        width: WIDTH,
        cell: CellSize {
            width: 10,
            height: 20,
        },
        pixel_ratio: 1.0,
        max_picture_rows: 20,
        pictures: &[],
    })
}

/// The URL `<CR>` opens with the cursor on the first cell saying `needle`, the rendering's lines being `rows`.
fn opened(document: &Document, needle: &str) -> Option<String> {
    let rendering = render(document);
    let rows: Vec<String> = rendering
        .lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect()
        })
        .collect();
    let linked = kit::linked_document(
        document,
        &rendering,
        "body",
        |_| String::new(),
        |_, link| Some(link.url.clone()),
    );
    let (row, col) = rows
        .iter()
        .enumerate()
        .find_map(|(row, text)| text.find(needle).map(|col| (row, col)))
        .unwrap_or_else(|| panic!("no line says {needle:?}: {rows:?}"));
    let layout = Layout::new(&linked, WIDTH);
    Cursor::at_screen(&layout, row, col)
        .target(&layout)
        .cloned()
}

#[test]
fn a_link_s_text_and_number_open_it_even_wrapped() {
    let document = Document::html(
        "<p>See <a href=\"https://a.example/x\">the long link text</a>, and \
         <a href=\"https://b.example\">b</a>.</p>",
        &Styles::default(),
    );
    let a = Some("https://a.example/x".to_owned());
    let b = Some("https://b.example".to_owned());

    assert_eq!(opened(&document, "See"), None);
    assert_eq!(opened(&document, "the"), a, "the line before the number");
    assert_eq!(opened(&document, "text"), a);
    assert_eq!(opened(&document, "[1]"), a);
    assert_eq!(opened(&document, "and"), None);
    assert_eq!(opened(&document, "b[2]"), b);
    assert_eq!(opened(&document, "[2]"), b);
}

#[test]
fn lines_shaded_after_their_links_are_found_still_open_them() {
    let document = Document::html(
        "<p>See <a href=\"https://x.example\">the page</a> now.</p>",
        &Styles::default(),
    );
    let rendering = render(&document);
    let shade = Style::new().fg(Color::Black).bg(Color::Red);

    let mut lines = kit::linked_lines(
        &document,
        &rendering,
        |_| String::new(),
        |_, link| Some(link.url.clone()),
    );
    for line in &mut lines {
        if let wcgw_kit::doc::Line::Text(spans) = line {
            for span in spans {
                span.style = Some(span.style.unwrap_or_default().patch(shade));
            }
        }
    }
    let mut shaded = wcgw_kit::doc::Document::new();
    shaded.push_back("body", wcgw_kit::doc::Content::from(lines));

    let layout = Layout::new(&shaded, WIDTH);
    let col = |needle: &str| {
        let text: String = layout.segments(0).map(|(text, _)| text).collect();
        text.find(needle).expect(needle)
    };
    let opened = |needle| {
        Cursor::at_screen(&layout, 0, col(needle))
            .target(&layout)
            .cloned()
    };
    assert_eq!(opened("page"), Some("https://x.example".to_owned()));
    assert_eq!(opened("See"), None);
}

#[test]
fn a_plain_address_opens_itself_cut_across_lines() {
    let document = Document::plain(
        "Go to https://example.com/a/very/long/path now",
        TextFormat::Fixed,
        &Styles::default(),
    );
    let url = Some("https://example.com/a/very/long/path".to_owned());

    assert_eq!(opened(&document, "Go"), None);
    assert_eq!(opened(&document, "https"), url);
    assert_eq!(opened(&document, "path"), url, "the address's last line");
    assert_eq!(opened(&document, "now"), None);
}

#[test]
fn code_is_coloured_as_the_application_says_and_cut_still_coloured() {
    let red = Style::new().fg(Color::Red);
    for (source, language) in [
        ("```rust\nlet x = 1; let y = 2;\n```\n", "rust"),
        (
            "<pre lang=\"rust\"><code>let x = 1; let y = 2;</code></pre>",
            "rust",
        ),
        (
            "<div class=\"highlight highlight-source-python\"><pre>let x = 1; let y = 2;</pre></div>",
            "python",
        ),
    ] {
        let mut document = if source.starts_with('`') {
            Document::markdown(source, &Styles::default())
        } else {
            Document::html(source, &Styles::default())
        };
        let heard = std::cell::RefCell::new(None);
        document.highlight_code(|lang, code| {
            *heard.borrow_mut() = Some((lang.map(str::to_owned), code.to_owned()));
            let at = code.find("let y").expect("the second let");
            Some(vec![
                (code[..at].to_owned(), Style::new()),
                ("let".to_owned(), red),
                (code[at + 3..].to_owned(), Style::new()),
            ])
        });
        let rendering = render(&document);

        let (lang, code) = heard.into_inner().expect("asked to colour");
        assert_eq!(lang.as_deref(), Some(language), "{source}");
        assert_eq!(code.trim_end(), "let x = 1; let y = 2;");
        let second = rendering
            .lines
            .iter()
            .flat_map(|line| &line.spans)
            .find(|span| span.content.starts_with("let") && span.style.fg == Some(Color::Red));
        assert!(second.is_some(), "{source}: {:?}", rendering.lines);
        assert!(rendering.lines.len() >= 2, "cut, at {WIDTH} cells");
    }
}

#[test]
fn code_the_application_leaves_alone_stays_as_it_was() {
    let mut document = Document::markdown("```\nplain\n```\n", &Styles::default());
    let before = render(&document).lines;

    document.highlight_code(|lang, _| {
        assert_eq!(lang, None);
        None
    });

    assert_eq!(render(&document).lines, before);
}
