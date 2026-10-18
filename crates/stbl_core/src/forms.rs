//! Declarative forms and shared, deterministic submission validation.
use std::collections::{BTreeMap, BTreeSet};

use pulldown_cmark::{Event, Parser, Tag};
use serde::{Deserialize, Serialize};

pub const HONEYPOT: &str = "_stbl_website";

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: FieldType,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub placeholder: Option<String>,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    Text,
    Email,
    Textarea,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Form {
    pub action: String,
    #[serde(default = "default_submit")]
    pub submit: String,
    pub fields: Vec<Field>,
}

/// Public validation data shipped with the generated website.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FormManifest {
    pub version: u32,
    pub forms: BTreeMap<String, Form>,
}

fn default_submit() -> String {
    "Send message".into()
}

pub fn validate_fields(fields: &[Field]) -> Result<(), String> {
    if fields.is_empty() || fields.len() > 64 {
        return Err("forms need between 1 and 64 fields".into());
    }
    let mut names = BTreeSet::new();
    for field in fields {
        if field.name.is_empty()
            || field.name.len() > 64
            || field.name.starts_with('_')
            || !field
                .name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            || !names.insert(&field.name)
        {
            return Err(format!(
                "invalid or duplicate form field name: {}",
                field.name
            ));
        }
    }
    Ok(())
}

/// Intentionally supports ordinary ASCII mailboxes, without quoted local parts.
pub fn valid_email(value: &str) -> bool {
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && local.len() <= 64
        && value.len() <= 254
        && !local.starts_with('.')
        && !local.ends_with('.')
        && !local.contains("..")
        && local
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".!#$%&'*+-/=?^_`{|}~".contains(&b))
        && domain.contains('.')
        && domain.split('.').all(|part| {
            !part.is_empty()
                && part.len() <= 63
                && !part.starts_with('-')
                && !part.ends_with('-')
                && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}

pub fn validate_submission(
    fields: &[Field],
    values: &BTreeMap<String, String>,
) -> Result<(), String> {
    if values.get(HONEYPOT).is_none_or(|value| !value.is_empty()) {
        return Err("Submission rejected. Please leave the website field empty.".into());
    }
    if values
        .keys()
        .any(|name| name != HONEYPOT && !fields.iter().any(|f| &f.name == name))
    {
        return Err("Submission contains an unknown field.".into());
    }
    for field in fields {
        let value = values.get(&field.name).map(|s| s.trim()).unwrap_or("");
        let label = field.label.as_deref().unwrap_or(&field.name);
        if field.required && value.is_empty() {
            return Err(format!("{label} is required."));
        }
        if field.kind == FieldType::Email && !value.is_empty() && !valid_email(value) {
            return Err(format!("{label} must be a valid email address."));
        }
        if value
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
        {
            return Err(format!("{label} contains invalid characters."));
        }
    }
    Ok(())
}

/// Reusable definitions live in the normal asset pipeline, at assets/forms/*.yaml.
pub fn is_definition_asset(path: &str) -> bool {
    path.starts_with("forms/") && (path.ends_with(".yaml") || path.ends_with(".yml"))
}

pub fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub fn render_definition(form: &Form) -> Result<String, String> {
    validate_fields(&form.fields)?;
    let action = crate::url::UrlMapper::form_action(&form.action)
        .ok_or("form action must be a root-relative path or HTTPS URL")?;
    let mut out = format!(
        "\n<form class=\"stbl-form\" method=\"post\" action=\"{}\" novalidate>\n",
        escape(action)
    );
    for field in &form.fields {
        let label = field.label.as_deref().unwrap_or(&field.name);
        out.push_str(&format!(
            "<label class=\"stbl-form-field\"><span>{}{}</span>",
            escape(label),
            if field.required { " (required)" } else { "" }
        ));
        let attrs = format!(
            "name=\"{}\" placeholder=\"{}\"{}",
            escape(&field.name),
            escape(field.placeholder.as_deref().unwrap_or("")),
            if field.required {
                " required aria-required=\"true\""
            } else {
                ""
            }
        );
        match field.kind {
            FieldType::Textarea => {
                out.push_str(&format!("<textarea {attrs} rows=\"6\"></textarea>"))
            }
            kind => out.push_str(&format!(
                "<input type=\"{}\" {attrs}>",
                if kind == FieldType::Email {
                    "email"
                } else {
                    "text"
                }
            )),
        }
        out.push_str("</label>\n");
    }
    out.push_str(&format!("<label class=\"stbl-form-honeypot\" aria-hidden=\"true\">Leave this empty<input type=\"text\" name=\"{HONEYPOT}\" tabindex=\"-1\" autocomplete=\"off\"></label>\n<button type=\"submit\">{}</button>\n<p class=\"stbl-form-status\" role=\"status\" aria-live=\"polite\" tabindex=\"-1\"></p>\n</form>\n\n", escape(&form.submit)));
    Ok(out)
}

/// Expand only standalone containers outside Markdown code blocks.
pub fn expand(markdown: &str) -> Result<String, String> {
    transform(markdown, |form| render_definition(&form))
}

/// Collect the same inline definitions used by rendering, excluding code examples.
pub fn definitions(markdown: &str) -> Result<Vec<Form>, String> {
    let mut forms = Vec::new();
    transform(markdown, |form| {
        render_definition(&form)?;
        forms.push(form);
        Ok(String::new())
    })?;
    Ok(forms)
}

fn transform(
    markdown: &str,
    mut render: impl FnMut(Form) -> Result<String, String>,
) -> Result<String, String> {
    let code_ranges: Vec<_> = Parser::new(markdown)
        .into_offset_iter()
        .filter_map(|(event, range)| {
            matches!(event, Event::Start(Tag::CodeBlock(_))).then_some(range)
        })
        .collect();
    let mut out = String::new();
    let mut lines = markdown.split_inclusive('\n');
    let mut offset = 0;
    while let Some(line) = lines.next() {
        let start = offset;
        offset += line.len();
        if matches!(line.trim_end(), ":::form" | "::: form")
            && !code_ranges.iter().any(|r| r.contains(&start))
        {
            let mut yaml = String::new();
            let mut closed = false;
            for body in lines.by_ref() {
                offset += body.len();
                if body.trim() == ":::" {
                    closed = true;
                    break;
                }
                yaml.push_str(body);
            }
            if !closed {
                return Err("unclosed form container".into());
            }
            let form: Form =
                serde_yaml::from_str(&yaml).map_err(|e| format!("invalid form: {e}"))?;
            out.push_str(&render(form)?);
        } else {
            out.push_str(line);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    const FORM: &str = ":::form\naction: /cgi-bin/contact\nfields:\n  - name: email\n    type: email\n    label: Email address\n  - name: message\n    type: textarea\n    required: true\n:::\n";

    #[test]
    fn optional_by_default_and_accessible_post() {
        let html = expand(FORM).unwrap();
        assert!(html.contains("method=\"post\""));
        assert!(html.contains("novalidate"));
        assert!(html.contains("type=\"email\" name=\"email\" placeholder=\"\">"));
        assert!(html.contains("required aria-required=\"true\""));
        assert!(html.contains("<label"));
        assert!(html.contains("aria-live=\"polite\""));
    }

    #[test]
    fn rejects_invalid_definitions_and_escapes_html() {
        for invalid in [
            FORM.replace("/cgi-bin/contact", "javascript:alert(1)"),
            FORM.replace("type: email", "type: checkbox"),
            FORM.replace("name: message", "name: email"),
            FORM.replace("required: true", "required: yes"),
            FORM.replace(":::\n", ""),
        ] {
            assert!(expand(&invalid).is_err(), "{invalid}");
        }
        assert!(
            expand(&FORM.replace("Email address", "'<script> & hello'"))
                .unwrap()
                .contains("&lt;script&gt; &amp; hello")
        );
    }

    #[test]
    fn leaves_code_examples_alone_and_supports_multiple_forms() {
        let fenced = format!("````yaml\n{FORM}````\n");
        assert_eq!(expand(&fenced).unwrap(), fenced);
        assert!(definitions(&fenced).unwrap().is_empty());
        let collected = definitions(FORM).unwrap();
        assert_eq!(collected.len(), 1);
        assert!(!collected[0].fields[0].required);
        assert!(collected[0].fields[1].required);
        assert_eq!(
            expand(&format!("{FORM}\n{FORM}"))
                .unwrap()
                .matches("<form ")
                .count(),
            2
        );
    }

    #[test]
    fn validates_optional_email_required_fields_and_honeypot() {
        let form: Form = serde_yaml::from_str(
            FORM.strip_prefix(":::form\n")
                .unwrap()
                .strip_suffix(":::\n")
                .unwrap(),
        )
        .unwrap();
        let mut values = BTreeMap::from([
            (HONEYPOT.into(), "".into()),
            ("message".into(), "Hello".into()),
        ]);
        assert!(validate_submission(&form.fields, &values).is_ok());
        values.insert("email".into(), "bad".into());
        assert!(validate_submission(&form.fields, &values).is_err());
        values.insert("email".into(), "guest@example.org".into());
        assert!(validate_submission(&form.fields, &values).is_ok());
        values.insert("message".into(), "  ".into());
        assert!(validate_submission(&form.fields, &values).is_err());
        values.insert(HONEYPOT.into(), "spam".into());
        assert!(validate_submission(&form.fields, &values).is_err());
    }

    #[test]
    fn email_rejects_headers_and_malformed_addresses() {
        for value in [
            "x@example.org\r\nBcc: victim@example.org",
            "a@@example.org",
            "a@-example.org",
            "a..b@example.org",
            "a@localhost",
            "a b@example.org",
        ] {
            assert!(!valid_email(value), "{value}");
        }
        assert!(valid_email("visitor+contact@example.org"));
    }
}
