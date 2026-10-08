//! Render docs/WHITEPAPER.md to HTML at build time, so no page needs a script
//! to show it.
//!
//!     cargo run -p novel-encryption --example render_whitepaper -- <template.html> <out.html>
//!     cargo run -p novel-encryption --example render_whitepaper -- --print <template.html> <out.html>
//!
//! The web template has a `<!-- WHITEPAPER -->` marker. The print template
//! (for the PDF) has `<!-- COVER -->`, `<!-- ABSTRACT -->`, `<!-- TOC -->` and
//! `<!-- BODY -->`: the paper's `---` rules split it into the title block, the
//! abstract and the numbered sections.

use pulldown_cmark::{html, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

const OPTS: Options = Options::ENABLE_TABLES.union(Options::ENABLE_STRIKETHROUGH);

fn slug(text: &str) -> String {
    let mut s = String::new();
    for c in text.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else if !s.ends_with('-') {
            s.push('-');
        }
    }
    s.trim_matches('-').to_string()
}

/// Headings (level, text) in document order.
fn headings(md: &str) -> Vec<(HeadingLevel, String)> {
    let mut out = Vec::new();
    let mut current: Option<(HeadingLevel, String)> = None;
    for ev in Parser::new_ext(md, OPTS) {
        match ev {
            Event::Start(Tag::Heading { level, .. }) => current = Some((level, String::new())),
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, s)) = current.as_mut() {
                    s.push_str(&t);
                }
            }
            Event::End(TagEnd::Heading(_)) => out.extend(current.take()),
            _ => {}
        }
    }
    out
}

/// Markdown to HTML: headings get slug ids, tables scroll on phones, and
/// column alignment uses classes (the site's CSP forbids inline styles).
fn render(md: &str) -> String {
    let mut ids = headings(md).into_iter().map(|(_, t)| slug(&t));
    let events = Parser::new_ext(md, OPTS).flat_map(|ev| match ev {
        Event::Start(Tag::Heading { level, id: None, classes, attrs }) => {
            let id = ids.next().map(Into::into);
            vec![Event::Start(Tag::Heading { level, id, classes, attrs })]
        }
        Event::Start(Tag::Heading { .. }) => {
            ids.next();
            vec![ev]
        }
        Event::Start(Tag::Table(_)) => vec![Event::Html("<div class=\"table-scroll\">".into()), ev],
        Event::End(TagEnd::Table) => vec![ev, Event::Html("</div>".into())],
        _ => vec![ev],
    });
    let mut body = String::new();
    html::push_html(&mut body, events);
    for side in ["left", "center", "right"] {
        body = body.replace(&format!("style=\"text-align: {side}\""), &format!("class=\"align-{side}\""));
    }
    assert!(!body.contains("style=\""), "inline style left in the white paper");
    body
}

fn fill(template: &str, pairs: &[(&str, &str)]) -> String {
    let mut page = template.to_string();
    for (marker, value) in pairs {
        assert!(page.contains(marker), "template lacks the {marker} marker");
        page = page.replace(marker, value);
    }
    page
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let print = args.first().is_some_and(|a| a == "--print");
    if print {
        args.remove(0);
    }
    let (template, out) = (&args[0], &args[1]);
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let md = std::fs::read_to_string(root.join("docs/WHITEPAPER.md")).unwrap();
    let template = std::fs::read_to_string(template).unwrap();
    let title = headings(&md)
        .into_iter()
        .find(|(l, _)| *l == HeadingLevel::H1)
        .map(|(_, t)| t)
        .unwrap_or_default();

    let page = if !print {
        fill(&template, &[
            ("<!-- WHITEPAPER -->", &render(&md)),
            ("<title>Novel Encryption White Paper</title>", &format!("<title>{title} · White Paper</title>")),
        ])
    } else {
        let parts: Vec<&str> = md.split("\n---\n").collect();
        assert!(parts.len() >= 3, "expected title block, abstract and sections separated by ---");
        let sections = parts[2..].join("\n");
        let toc: String = headings(&sections)
            .into_iter()
            .filter(|(l, _)| matches!(l, HeadingLevel::H2 | HeadingLevel::H3))
            .map(|(l, t)| {
                let class = if l == HeadingLevel::H2 { "toc-2" } else { "toc-3" };
                {
                    let id = slug(&t);
                    format!(
                        "<li class=\"{class}\"><a href=\"#{id}\">{}</a><span class=\"toc-page\" data-for=\"{id}\"></span></li>\n",
                        html_escape(&t)
                    )
                }
            })
            .collect();
        fill(&template, &[
            ("<!-- COVER -->", &render(parts[0])),
            ("<!-- ABSTRACT -->", &render(parts[1])),
            ("<!-- TOC -->", &toc),
            ("<!-- BODY -->", &render(&sections)),
        ])
    };
    std::fs::write(out, page).unwrap();
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}
