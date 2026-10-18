//! CGI request processing and local-MTA delivery. No delivery policy comes from POST data.
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use stbl_core::forms::{self, Field, FieldType};

mod delivery;
mod rate_limit;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiverConfig {
    pub rate_dir: PathBuf,
    #[serde(default = "default_max_bytes")]
    pub max_request_bytes: usize,
    #[serde(default = "default_limit")]
    pub requests_per_minute: u32,
    #[serde(default = "default_global_limit")]
    pub global_requests_per_minute: u32,
    #[serde(default = "default_mail_timeout")]
    pub sendmail_timeout_seconds: u64,
    #[serde(default = "default_sendmail")]
    pub sendmail: PathBuf,
    pub sites: BTreeMap<String, ReceiverSite>,
}
#[derive(Deserialize)]
#[serde(untagged)]
pub enum ReceiverSite {
    Website(WebsiteReceiverSite),
    Legacy(BTreeMap<String, ReceiverForm>),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebsiteReceiverSite {
    pub website_root: PathBuf,
    pub forms: BTreeMap<String, ReceiverForm>,
}

impl ReceiverSite {
    fn forms(&self) -> &BTreeMap<String, ReceiverForm> {
        match self {
            Self::Website(site) => &site.forms,
            Self::Legacy(forms) => forms,
        }
    }
    fn forms_mut(&mut self) -> &mut BTreeMap<String, ReceiverForm> {
        match self {
            Self::Website(site) => &mut site.forms,
            Self::Legacy(forms) => forms,
        }
    }
}

fn default_max_bytes() -> usize {
    32768
}
fn default_limit() -> u32 {
    5
}
fn default_global_limit() -> u32 {
    1000
}
fn default_mail_timeout() -> u64 {
    10
}
fn default_sendmail() -> PathBuf {
    "/usr/sbin/sendmail".into()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiverForm {
    pub sender: String,
    pub recipient: String,
    pub subject: String,
    #[serde(default)]
    pub reply_to_field: Option<String>,
    #[serde(default)]
    pub fields: Vec<Field>,
    /// A deployed public form definition; relative to the server config file.
    #[serde(default)]
    pub definition: Option<PathBuf>,
}

/// Validate all sites, for explicit configuration checks.
pub fn load_config(path: &Path) -> Result<ReceiverConfig> {
    let mut config = read_config(path)?;
    for site in config.sites.values_mut() {
        validate_site(path, site)?;
    }
    Ok(config)
}

/// Request handling must not load manifests belonging to unrelated websites.
pub fn load_config_for_site(path: &Path, selected: &str) -> Result<ReceiverConfig> {
    let mut config = read_config(path)?;
    let mut site = config
        .sites
        .remove(selected)
        .context("unknown configured site")?;
    validate_site(path, &mut site)?;
    config.sites = BTreeMap::from([(selected.to_string(), site)]);
    Ok(config)
}

fn read_config(path: &Path) -> Result<ReceiverConfig> {
    let config: ReceiverConfig = serde_yaml::from_str(&fs::read_to_string(path)?)?;
    if config.max_request_bytes == 0
        || config.max_request_bytes > 1024 * 1024
        || config.requests_per_minute == 0
        || config.requests_per_minute > 1000
        || config.global_requests_per_minute == 0
        || config.global_requests_per_minute > rate_limit::MAX_GLOBAL_LIMIT
        || !(1..=30).contains(&config.sendmail_timeout_seconds)
        || !config.rate_dir.is_absolute()
        || !config.sendmail.is_absolute()
    {
        bail!("invalid receiver size, rate, timeout, or paths");
    }
    Ok(config)
}

fn validate_site(path: &Path, site: &mut ReceiverSite) -> Result<()> {
    let manifest = match site {
        ReceiverSite::Website(site) => {
            if !site.website_root.is_absolute() {
                bail!("website_root must be an absolute deployed website directory");
            }
            let manifest_path = site.website_root.join("stbl-forms/forms.yaml");
            let manifest: forms::FormManifest = serde_yaml::from_str(
                &fs::read_to_string(&manifest_path)
                    .with_context(|| format!("failed to read {}", manifest_path.display()))?,
            )?;
            if manifest.version != 1 {
                bail!("unsupported forms.yaml version {}", manifest.version);
            }
            Some(manifest)
        }
        ReceiverSite::Legacy(_) => None,
    };
    for (endpoint, form) in site.forms_mut() {
        if !endpoint.starts_with('/') || endpoint.contains('?') {
            bail!("invalid endpoint");
        }
        if let Some(manifest) = &manifest {
            if form.definition.is_some() || !form.fields.is_empty() {
                bail!("website_root forms must use the generated forms.yaml schema");
            }
            let public = manifest
                .forms
                .get(endpoint)
                .with_context(|| format!("endpoint {endpoint} is absent from forms.yaml"))?;
            forms::render_definition(public).map_err(anyhow::Error::msg)?;
            if public.action != *endpoint {
                bail!("manifest action does not match configured endpoint");
            }
            form.fields = public.fields.clone();
        } else if let Some(definition) = &form.definition {
            if !form.fields.is_empty() {
                bail!("configure fields or definition, not both");
            }
            let definition_path = path.parent().unwrap_or(Path::new(".")).join(definition);
            let public: forms::Form = serde_yaml::from_str(&fs::read_to_string(definition_path)?)?;
            // Validate public syntax as well as the shared field rules.
            forms::render_definition(&public).map_err(anyhow::Error::msg)?;
            if public.action != *endpoint {
                bail!("form action does not match configured endpoint");
            }
            form.fields = public.fields;
        }
        forms::validate_fields(&form.fields).map_err(anyhow::Error::msg)?;
        if !forms::valid_email(&form.sender)
            || !forms::valid_email(&form.recipient)
            || form.subject.is_empty()
            || !form.subject.is_ascii()
            || form.subject.chars().any(char::is_control)
        {
            bail!("invalid configured mail headers");
        }
        if let Some(name) = &form.reply_to_field {
            if !form
                .fields
                .iter()
                .any(|f| &f.name == name && f.kind == FieldType::Email)
            {
                bail!("reply_to_field must name a configured email field");
            }
        }
    }
    Ok(())
}

pub struct Request<'a> {
    pub method: &'a str,
    pub endpoint: &'a str,
    pub content_type: &'a str,
    pub content_length: &'a str,
    pub remote_addr: &'a str,
}

pub struct Response {
    pub status: u16,
    pub success: bool,
    pub message: String,
}
impl Response {
    pub fn error(status: u16, message: impl Into<String>) -> Self {
        Self {
            status,
            success: false,
            message: message.into(),
        }
    }
    pub fn cgi(&self, json: bool) -> String {
        let body = if json {
            serde_json::json!({"success": self.success, "message": self.message}).to_string()
        } else {
            format!(
                "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Form submission</title><main><h1>{}</h1><p>{}</p>{}</main></html>",
                if self.success {
                    "Thank you"
                } else {
                    "Unable to send"
                },
                forms::escape(&self.message),
                if self.success {
                    ""
                } else {
                    "<p>Use your browser’s Back button to return to the form.</p>"
                }
            )
        };
        let reason = match self.status {
            200 => "OK",
            400 => "Bad Request",
            404 => "Not Found",
            405 => "Method Not Allowed",
            413 => "Content Too Large",
            415 => "Unsupported Media Type",
            429 => "Too Many Requests",
            503 => "Service Unavailable",
            _ => "Internal Server Error",
        };
        format!(
            "Status: {} {reason}\r\nContent-Type: {}; charset=utf-8\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'none'; frame-ancestors 'none'\r\n{}\r\n{body}\n",
            self.status,
            if json {
                "application/json"
            } else {
                "text/html"
            },
            if self.status == 429 {
                "Retry-After: 60\r\n"
            } else if self.status == 405 {
                "Allow: POST\r\n"
            } else {
                ""
            }
        )
    }
}

pub fn wants_json(accept: &str) -> bool {
    accept.split(',').any(|part| {
        part.split(';')
            .next()
            .unwrap_or("")
            .trim()
            .eq_ignore_ascii_case("application/json")
    })
}

/// Decode strictly: the permissive URL decoder alone does not reject bad percent escapes.
pub fn decode_body(body: &[u8]) -> Result<BTreeMap<String, String>> {
    let body = std::str::from_utf8(body)?;
    let decode = |value: &str| -> Result<String> {
        let bytes = value.as_bytes();
        for (i, b) in bytes.iter().enumerate() {
            if *b == b'%'
                && (i + 2 >= bytes.len()
                    || !bytes[i + 1].is_ascii_hexdigit()
                    || !bytes[i + 2].is_ascii_hexdigit())
            {
                bail!("bad percent encoding");
            }
        }
        Ok(urlencoding::decode(&value.replace('+', " "))?.into_owned())
    };
    let mut values = BTreeMap::new();
    for pair in body.split('&') {
        let (name, value) = pair.split_once('=').context("missing equals sign")?;
        let name = decode(name)?;
        let value = decode(value)?;
        if values.len() >= 65 || name.is_empty() || values.insert(name, value).is_some() {
            bail!("duplicate or excessive fields");
        }
    }
    Ok(values)
}

pub trait Forwarder {
    fn forward(&self, form: &ReceiverForm, values: &BTreeMap<String, String>) -> Result<()>;
}

pub struct Sendmail<'a> {
    pub path: &'a Path,
    pub timeout: Duration,
}
impl Forwarder for Sendmail<'_> {
    fn forward(&self, form: &ReceiverForm, values: &BTreeMap<String, String>) -> Result<()> {
        let mut message = format!(
            "From: {}\nTo: {}\nSubject: {}\n",
            form.sender, form.recipient, form.subject
        );
        if let Some(email) = form
            .reply_to_field
            .as_ref()
            .and_then(|key| values.get(key))
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            if !forms::valid_email(email) {
                bail!("invalid reply-to");
            }
            message.push_str(&format!("Reply-To: {email}\n"));
        }
        message.push_str("MIME-Version: 1.0\nContent-Type: text/plain; charset=UTF-8\nContent-Transfer-Encoding: 8bit\n\n");
        for (index, field) in form.fields.iter().enumerate() {
            if index > 0 {
                message.push_str("\n\n");
            }
            let mut label = field.name.replace(['_', '-'], " ");
            if let Some(first) = label.get_mut(..1) {
                first.make_ascii_uppercase();
            }
            message.push_str(&label);
            message.push_str(if field.kind == FieldType::Textarea {
                ":\n"
            } else {
                ": "
            });
            message.push_str(values.get(&field.name).map(String::as_str).unwrap_or(""));
        }
        message.push('\n');
        delivery::send(
            self.path,
            &form.sender,
            &form.recipient,
            message.as_bytes(),
            self.timeout,
        )
    }
}

pub fn process(
    config: &ReceiverConfig,
    site: &str,
    request: &Request<'_>,
    input: impl Read,
    forwarder: &dyn Forwarder,
) -> Response {
    if request.method != "POST" {
        return Response::error(405, "Use POST to submit this form.");
    }
    let Some(form) = config
        .sites
        .get(site)
        .and_then(|site| site.forms().get(request.endpoint))
    else {
        return Response::error(404, "Unknown form endpoint.");
    };
    if !request
        .content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .eq_ignore_ascii_case("application/x-www-form-urlencoded")
    {
        return Response::error(415, "Use application/x-www-form-urlencoded.");
    }
    let length = match request.content_length.parse::<usize>() {
        Ok(n) if n <= config.max_request_bytes => n,
        Ok(_) => return Response::error(413, "Your message is too large."),
        Err(_) => return Response::error(400, "Invalid request length."),
    };
    // REMOTE_ADDR is supplied by the server; never trust forwarded visitor headers.
    let client = match request.remote_addr.parse::<std::net::IpAddr>() {
        Ok(client) => client,
        Err(_) => return Response::error(400, "Missing client address."),
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    match rate_limit::check(
        &config.rate_dir,
        &format!("{site}:{}:{client}", request.endpoint),
        config.requests_per_minute,
        config.global_requests_per_minute,
        now,
    ) {
        Ok(true) => {}
        Ok(false) => {
            return Response::error(
                429,
                "Too many submissions. Please wait a minute and try again.",
            );
        }
        Err(e) => {
            eprintln!("form rate limit: {e}");
            return Response::error(
                503,
                "The form is temporarily unavailable. Please try again later.",
            );
        }
    }
    // CGI servers need not send EOF after the body; never read beyond CONTENT_LENGTH.
    let mut body = vec![0; length];
    let mut input = input;
    if input.read_exact(&mut body).is_err() {
        return Response::error(400, "Incomplete or malformed request body.");
    }
    let values = match decode_body(&body) {
        Ok(values) => values,
        Err(_) => return Response::error(400, "Malformed form submission."),
    };
    if let Err(message) = forms::validate_submission(&form.fields, &values) {
        return Response::error(400, message);
    }
    match forwarder.forward(form, &values) {
        Ok(()) => Response {
            status: 200,
            success: true,
            message: "Thank you. Your message has been queued.".into(),
        },
        Err(e) => {
            eprintln!("form forwarding: {e}");
            Response::error(
                503,
                if e.is::<delivery::DeliveryTimeout>() {
                    "Mail queuing timed out. Delivery is uncertain; please wait before trying again."
                } else {
                    "Your message could not be queued. Please try again later."
                },
            )
        }
    }
}
