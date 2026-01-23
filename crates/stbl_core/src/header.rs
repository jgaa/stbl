//! STBL header block parsing

use chrono::{NaiveDateTime, TimeZone, Utc};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub uuid: Uuid,
    pub title: Option<String>,
    pub tags: Vec<String>,
    pub updated: Option<i64>,
    pub abstract_text: Option<String>,
    pub template: Option<String>,
    pub content_type: Option<String>,
    pub menu: Option<String>,
    pub banner: Option<String>,
    pub banner_credits: Option<String>,
    pub comments: Option<String>,
    pub part: Option<String>,
    pub sitemap_priority: Option<String>,
    pub sitemap_changefreq: Option<String>,
    pub published: Option<i64>,
    pub is_published: bool,
    pub expires: Option<i64>,
    pub authors: Vec<String>,
    pub author: Option<String>,
}

impl Default for Header {
    fn default() -> Self {
        Self {
            uuid: Uuid::new_v4(),
            title: None,
            tags: Vec::new(),
            updated: None,
            abstract_text: None,
            template: None,
            content_type: None,
            menu: None,
            banner: None,
            banner_credits: None,
            comments: None,
            part: None,
            sitemap_priority: None,
            sitemap_changefreq: None,
            published: None,
            is_published: true,
            expires: None,
            authors: Vec::new(),
            author: None,
        }
    }
}

#[derive(Debug, Error)]
pub enum HeaderError {
    #[error("invalid header line: {0}")]
    InvalidLine(String),
    #[error("invalid header key: {0}")]
    InvalidKey(String),
    #[error("invalid uuid: {0}")]
    InvalidUuid(#[from] uuid::Error),
    #[error("invalid datetime for {key}: {value}")]
    InvalidDatetime { key: String, value: String },
}

pub fn parse_header(input: &str) -> Result<Header, HeaderError> {
    let mut header = Header::default();
    for raw_line in input.lines() {
        if raw_line.trim().is_empty() {
            continue;
        }
        let trimmed_start = raw_line.trim_start();
        if trimmed_start.starts_with('#') {
            continue;
        }
        let line = strip_inline_comment(raw_line);
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (key, value) = line
            .split_once(':')
            .ok_or_else(|| HeaderError::InvalidLine(line.to_string()))?;
        let key = key.trim();
        if key.is_empty() || !key.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-') {
            return Err(HeaderError::InvalidKey(key.to_string()));
        }
        let value = value.trim();
        match key {
            "uuid" => {
                if !value.is_empty() {
                    header.uuid = Uuid::parse_str(value)?;
                }
            }
            "title" => header.title = non_empty(value),
            "tags" => header.tags = split_list(value),
            "updated" => header.updated = parse_datetime(value, key)?,
            "abstract" => header.abstract_text = non_empty(value),
            "template" => header.template = non_empty(value),
            "type" => header.content_type = non_empty(value),
            "menu" => header.menu = non_empty(value),
            "banner" => header.banner = non_empty(value),
            "banner-credits" => header.banner_credits = non_empty(value),
            "comments" => header.comments = non_empty(value),
            "part" => header.part = non_empty(value),
            "sitemap-priority" => header.sitemap_priority = non_empty(value),
            "sitemap-changefreq" => header.sitemap_changefreq = non_empty(value),
            "published" => {
                if value == "false" || value == "no" {
                    header.is_published = false;
                    header.published = None;
                } else {
                    header.published = parse_datetime(value, key)?;
                    header.is_published = true;
                }
            }
            "expires" => header.expires = parse_datetime(value, key)?,
            "authors" => header.authors = split_list(value),
            "author" => header.author = non_empty(value),
            _ => return Err(HeaderError::InvalidKey(key.to_string())),
        }
    }

    if let Some(author) = header.author.clone().filter(|value| !value.is_empty()) {
        header.authors.insert(0, author);
    }

    Ok(header)
}

fn strip_inline_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    for (idx, byte) in bytes.iter().enumerate() {
        if *byte == b'#' && idx > 0 && bytes[idx - 1].is_ascii_whitespace() {
            return line[..idx].trim_end();
        }
    }
    line
}

fn split_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn parse_datetime(value: &str, key: &str) -> Result<Option<i64>, HeaderError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let parsed = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M").map_err(|_| {
        HeaderError::InvalidDatetime {
            key: key.to_string(),
            value: trimmed.to_string(),
        }
    })?;
    Ok(Some(Utc.from_utc_datetime(&parsed).timestamp()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_fields() {
        let input = "\
uuid: 4f234d4a-5c88-4fb8-9f55-3f75a8efc2c0
title: My Title
tags: rust, yaml
updated: 2024-01-02 03:04
abstract: Summary
template: post
type: article
menu: main
banner: hero.png
banner-credits: Jane Doe
comments: on
part: 2
sitemap-priority: 0.5
sitemap-changefreq: weekly
published: 2024-01-03 04:05
expires: 2024-02-03 04:05
authors: Alice, Bob
author: Carol
";
        let header = parse_header(input).expect("parse should succeed");
        assert_eq!(
            header.uuid,
            Uuid::parse_str("4f234d4a-5c88-4fb8-9f55-3f75a8efc2c0").unwrap()
        );
        assert_eq!(header.title.as_deref(), Some("My Title"));
        assert_eq!(header.tags, vec!["rust".to_string(), "yaml".to_string()]);
        assert_eq!(header.abstract_text.as_deref(), Some("Summary"));
        assert_eq!(header.template.as_deref(), Some("post"));
        assert_eq!(header.content_type.as_deref(), Some("article"));
        assert_eq!(header.menu.as_deref(), Some("main"));
        assert_eq!(header.banner.as_deref(), Some("hero.png"));
        assert_eq!(header.banner_credits.as_deref(), Some("Jane Doe"));
        assert_eq!(header.comments.as_deref(), Some("on"));
        assert_eq!(header.part.as_deref(), Some("2"));
        assert_eq!(header.sitemap_priority.as_deref(), Some("0.5"));
        assert_eq!(header.sitemap_changefreq.as_deref(), Some("weekly"));
        assert!(header.is_published);
        assert_eq!(header.authors[0], "Carol");
        assert_eq!(header.authors[1], "Alice");
        assert_eq!(header.authors[2], "Bob");
        assert!(header.published.is_some());
        assert!(header.updated.is_some());
        assert!(header.expires.is_some());
    }

    #[test]
    fn list_parsing_trims_correctly() {
        let input = "\
tags:  rust,  yaml , ,  cli
authors: Alice,  Bob,Charlie  , 
";
        let header = parse_header(input).expect("parse should succeed");
        assert_eq!(
            header.tags,
            vec!["rust".to_string(), "yaml".to_string(), "cli".to_string()]
        );
        assert_eq!(
            header.authors,
            vec!["Alice".to_string(), "Bob".to_string(), "Charlie".to_string()]
        );
    }

    #[test]
    fn comment_behavior() {
        let input = "\
# full line comment
title: Hello # inline comment
banner: https://example.com/#frag
";
        let header = parse_header(input).expect("parse should succeed");
        assert_eq!(header.title.as_deref(), Some("Hello"));
        assert_eq!(
            header.banner.as_deref(),
            Some("https://example.com/#frag")
        );
    }

    #[test]
    fn published_false_and_no() {
        let header_false = parse_header("published: false\n").expect("parse should succeed");
        assert!(!header_false.is_published);
        assert!(header_false.published.is_none());

        let header_no = parse_header("published: no\n").expect("parse should succeed");
        assert!(!header_no.is_published);
        assert!(header_no.published.is_none());
    }

    #[test]
    fn missing_uuid_generates_one() {
        let header = parse_header("title: Hello\n").expect("parse should succeed");
        assert!(!header.uuid.is_nil());
    }
}
