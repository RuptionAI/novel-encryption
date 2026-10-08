//! Render docs/WHITEPAPER.md into the site's white paper page at build time,
//! so the page needs no script to display it.
//!
//!     cargo run -p novel-encryption --example render_whitepaper -- <template.html> <out.html>

use pulldown_cmark::{html, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (template, out) = (&args[1], &args[2]);
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let md = std::fs::read_to_string(root.join("docs/WHITEPAPER.md")).unwrap();

    // First pass: heading texts, for anchor ids and the page title.
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let mut headings = Vec::new();
    let mut current: Option<(HeadingLevel, String)> = None;
    for ev in Parser::new_ext(&md, opts) {
        match ev {
            Event::Start(Tag::Heading { level, .. }) => current = Some((level, String::new())),
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, s)) = current.as_mut() {
                    s.push_str(&t);
                }
            }
            Event::End(TagEnd::Heading(_)) => headings.extend(current.take()),
            _ => {}
        }
    }
    let title = headings.iter().find(|(l, _)| *l == HeadingLevel::H1).map(|(_, t)| t.clone()).unwrap_or_default();

    // Second pass: give h2/h3 ids and wrap tables so they scroll on phones.
    let mut ids = headings.iter().map(|(_, t)| slug(t));
    let events = Parser::new_ext(&md, opts).flat_map(|ev| match ev {
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

    // Column alignment as classes, not inline styles (the site's CSP has no
    // 'unsafe-inline').
    for side in ["left", "center", "right"] {
        body = body.replace(&format!("style=\"text-align: {side}\""), &format!("class=\"align-{side}\""));
    }
    assert!(!body.contains("style=\""), "inline style left in the white paper");

    let page = std::fs::read_to_string(template).unwrap();
    assert!(page.contains("<!-- WHITEPAPER -->"), "template lacks the <!-- WHITEPAPER --> marker");
    let page = page
        .replace("<!-- WHITEPAPER -->", &body)
        .replace("<title>Novel Encryption White Paper</title>", &format!("<title>{title} · White Paper</title>"));
    std::fs::write(out, page).unwrap();
}
