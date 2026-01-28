use pulldown_cmark::{Options, Parser, html};

pub fn render_markdown_to_html(md: &str) -> String {
    let options = Options::empty();
    let parser = Parser::new_ext(md, options);
    let mut html_out = String::new();
    html::push_html(&mut html_out, parser);
    html_out
}

#[cfg(test)]
mod tests {
    use super::render_markdown_to_html;

    #[test]
    fn renders_basic_markdown() {
        let html = render_markdown_to_html("# Title\n\nHello **world**.\n");
        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("<p>Hello <strong>world</strong>.</p>"));
    }
}
