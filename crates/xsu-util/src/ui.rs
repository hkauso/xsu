//! Ui utilities
use std::collections::HashSet;

use pulldown_cmark::{Parser, Options};
use ammonia::Builder;

/// Render markdown input into HTML
pub fn render_markdown(input: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);
    options.insert(Options::ENABLE_GFM);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);

    let parser = Parser::new_ext(input, options);
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, parser);

    let mut allowed_attributes = HashSet::new();
    allowed_attributes.insert("id");
    allowed_attributes.insert("class");
    allowed_attributes.insert("ref");
    allowed_attributes.insert("aria-label");
    allowed_attributes.insert("lang");
    allowed_attributes.insert("title");

    Builder::default()
        .generic_attributes(allowed_attributes)
        .clean(&html)
        .to_string()
}
