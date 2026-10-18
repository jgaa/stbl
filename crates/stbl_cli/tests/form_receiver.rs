#![cfg(unix)]
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};

use stbl_cli::form_receiver::{self, Request, Sendmail};
use tempfile::TempDir;

struct Fixture {
    dir: TempDir,
}
impl Fixture {
    fn new(limit: u32) -> Self {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::create_dir(root.join("rate")).unwrap();
        let sendmail = root.join("sendmail");
        fs::write(
            &sendmail,
            "#!/bin/sh\ncat > \"$0.message\"\nprintf '%s\\n' \"$@\" > \"$0.args\"\n",
        )
        .unwrap();
        fs::set_permissions(&sendmail, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(root.join("contact.yaml"), "action: /cgi-bin/contact\nsubmit: Send\nfields:\n  - name: name\n    type: text\n  - name: email\n    type: email\n  - name: message\n    type: textarea\n    required: true\n").unwrap();
        fs::write(root.join("receiver.yaml"), format!("rate_dir: {}\nrequests_per_minute: {limit}\nmax_request_bytes: 1024\nsendmail: {}\nsites:\n  example:\n    /cgi-bin/contact:\n      sender: forms@example.org\n      recipient: owner@example.org\n      subject: Contact form\n      reply_to_field: email\n      definition: contact.yaml\n  other-site:\n    /cgi-bin/feedback:\n      sender: forms@other.org\n      recipient: owner@other.org\n      subject: Feedback\n      fields:\n        - name: feedback\n          type: text\n          required: true\n", root.join("rate").display(), sendmail.display())).unwrap();
        Self { dir }
    }
    fn command(&self, body: &str, accept: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_stbl-form-receiver"));
        command
            .args([
                "--config",
                self.dir.path().join("receiver.yaml").to_str().unwrap(),
                "--site",
                "example",
            ])
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("REQUEST_METHOD", "POST")
            .env("SCRIPT_NAME", "/cgi-bin/contact")
            .env("CONTENT_TYPE", "application/x-www-form-urlencoded")
            .env("CONTENT_LENGTH", body.len().to_string())
            .env("REMOTE_ADDR", "127.0.0.1")
            .env("HTTP_ACCEPT", accept)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped());
        command
    }
    fn cgi(&self, body: &str, accept: &str) -> String {
        let mut child = self.command(body, accept).spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(body.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout).unwrap()
    }
}

#[test]
fn cgi_ajax_and_ordinary_post_queue_mail_with_optional_fields() {
    let fixture = Fixture::new(10);
    let html = fixture.cgi("message=Hello+world&_stbl_website=", "text/html");
    assert!(html.starts_with("Status: 200 OK"), "{html}");
    assert!(html.contains("Content-Type: text/html"));
    assert!(html.contains("Your message has been queued."));
    let mail = fs::read_to_string(fixture.dir.path().join("sendmail.message")).unwrap();
    assert!(mail.contains("To: owner@example.org\n"));
    assert!(mail.contains("Hello world"));
    assert!(!mail.contains("Reply-To:"));
    let json = fixture.cgi(
        "email=guest%2Bcontact%40example.org&message=Hello&_stbl_website=",
        "application/json",
    );
    assert!(json.contains("\"success\":true"), "{json}");
    let mail = fs::read_to_string(fixture.dir.path().join("sendmail.message")).unwrap();
    assert!(mail.contains("Reply-To: guest+contact@example.org\n"));
    assert_eq!(
        fs::read_to_string(fixture.dir.path().join("sendmail.args")).unwrap(),
        "-i\n-f\nforms@example.org\n--\nowner@example.org\n"
    );
}

#[test]
fn cgi_rejects_required_invalid_optional_email_spam_and_destination_overrides() {
    let fixture = Fixture::new(20);
    for body in [
        "message=+&_stbl_website=",
        "message=Hello&email=invalid&_stbl_website=",
        "message=Hello&email=x%40example.org%0ABcc%3Ay%40example.org&_stbl_website=",
        "message=Hello&_stbl_website=spam",
        "message=Hello",
        "message=Hello&recipient=attacker%40example.org&_stbl_website=",
        "message=one&message=two&_stbl_website=",
    ] {
        let response = fixture.cgi(body, "application/json");
        assert!(response.starts_with("Status: 400"), "{body}: {response}");
        assert!(response.contains("\"success\":false"));
        assert!(!fixture.dir.path().join("sendmail.message").exists());
    }
    let html = fixture.cgi("message=&_stbl_website=", "text/html");
    assert!(html.contains("message is required."));
    assert!(html.contains("Back button"));
}

#[test]
fn reports_queue_failure_and_rate_limit_across_processes() {
    let fixture = Fixture::new(1);
    fs::write(fixture.dir.path().join("sendmail"), "#!/bin/sh\nexit 75\n").unwrap();
    let failed = fixture.cgi("message=Hello&_stbl_website=", "application/json");
    assert!(failed.starts_with("Status: 503"), "{failed}");
    assert!(failed.contains("could not be queued"));
    let limited = fixture.cgi("message=Hello&_stbl_website=", "text/html");
    assert!(limited.starts_with("Status: 429"), "{limited}");
    assert!(limited.contains("Retry-After: 60"));
}

#[test]
fn rejects_bad_request_metadata_size_and_incomplete_bodies() {
    let fixture = Fixture::new(30);
    let config = form_receiver::load_config(&fixture.dir.path().join("receiver.yaml")).unwrap();
    let mut request = Request {
        method: "POST",
        endpoint: "/cgi-bin/contact",
        content_type: "application/x-www-form-urlencoded; charset=UTF-8",
        content_length: "32",
        remote_addr: "::1",
    };
    let sendmail = Sendmail {
        path: &config.sendmail,
        timeout: std::time::Duration::from_secs(config.sendmail_timeout_seconds),
    };
    let check = |r: &Request<'_>, body: &[u8], site: &str| {
        form_receiver::process(&config, site, r, body, &sendmail).status
    };
    request.method = "GET";
    assert_eq!(check(&request, b"", "example"), 405);
    request.method = "POST";
    request.content_type = "application/json";
    assert_eq!(check(&request, b"", "example"), 415);
    request.content_type = "application/x-www-form-urlencoded";
    request.content_length = "1025";
    assert_eq!(check(&request, b"", "example"), 413);
    request.content_length = "unknown";
    assert_eq!(check(&request, b"", "example"), 400);
    request.content_length = "32";
    assert_eq!(check(&request, b"short", "example"), 400);
    assert_eq!(check(&request, b"", "other-site"), 404);
    request.remote_addr = "";
    assert_eq!(check(&request, b"", "example"), 400);
}

#[test]
fn strict_urlencoded_parser_and_config_reject_malformed_data() {
    for body in [
        b"x=%".as_slice(),
        b"x=%GG",
        b"x=%FF",
        b"x=a&x=b",
        b"x",
        b"=a",
        b"x=1&",
    ] {
        assert!(form_receiver::decode_body(body).is_err(), "{body:?}");
    }
    assert_eq!(
        form_receiver::decode_body(b"x=a%2Bb+c").unwrap()["x"],
        "a+b c"
    );
    let fixture = Fixture::new(10);
    let path = fixture.dir.path().join("receiver.yaml");
    let original = fs::read_to_string(&path).unwrap();
    for bad in [
        original.replace(
            "subject: Contact form",
            "subject: \"Contact\\nBcc: victim@example.org\"",
        ),
        original.replace("forms@example.org", "bad address"),
        original.replace("reply_to_field: email", "reply_to_field: message"),
        original.replace("max_request_bytes: 1024", "max_request_bytes: 0"),
        format!("global_requests_per_minute: 0\n{original}"),
        format!("global_requests_per_minute: 10001\n{original}"),
        format!("sendmail_timeout_seconds: 0\n{original}"),
        format!("sendmail_timeout_seconds: 31\n{original}"),
    ] {
        fs::write(&path, bad).unwrap();
        assert!(form_receiver::load_config(&path).is_err());
    }
}

#[test]
fn mail_body_uses_compact_field_names_and_multiline_message() {
    let fixture = Fixture::new(10);
    fs::write(fixture.dir.path().join("contact.yaml"), "action: /cgi-bin/contact\nfields:\n  - name: message\n    type: textarea\n    label: Your message\n    required: true\n  - name: email\n    type: email\n    label: Your email address\n").unwrap();
    let response = fixture.cgi(
        "message=Hello%0ASecond+line&email=visitor%40example.org&_stbl_website=",
        "application/json",
    );
    assert!(response.starts_with("Status: 200"), "{response}");
    let mail = fs::read_to_string(fixture.dir.path().join("sendmail.message")).unwrap();
    let (_, body) = mail.split_once("\n\n").unwrap();
    assert_eq!(
        body,
        "Message:\nHello\nSecond line\n\nEmail: visitor@example.org\n"
    );
    assert!(mail.contains("Reply-To: visitor@example.org\n"));

    let response = fixture.cgi("message=Hello&email=&_stbl_website=", "text/html");
    assert!(response.starts_with("Status: 200"), "{response}");
    let mail = fs::read_to_string(fixture.dir.path().join("sendmail.message")).unwrap();
    let (_, body) = mail.split_once("\n\n").unwrap();
    assert_eq!(body, "Message:\nHello\n\nEmail: \n");
    assert!(!mail.contains("Reply-To:"));
}

#[test]
fn complete_cgi_body_does_not_require_eof() {
    use std::time::{Duration, Instant};
    let fixture = Fixture::new(10);
    let body = "message=Hello&_stbl_website=";
    let mut child = fixture.command(body, "application/json").spawn().unwrap();
    let mut input = child.stdin.take().unwrap();
    input.write_all(body.as_bytes()).unwrap();
    input.flush().unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while child.try_wait().unwrap().is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    let completed = child.try_wait().unwrap().is_some();
    if !completed {
        child.kill().unwrap();
    }
    let output = child.wait_with_output().unwrap();
    drop(input);
    assert!(
        completed,
        "receiver waited for EOF after the complete declared body"
    );
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("\"success\":true")
    );
}

#[test]
fn selected_site_ignores_unrelated_missing_or_broken_manifest() {
    let fixture = Fixture::new(10);
    let root = fixture.dir.path();
    fs::create_dir(root.join("stbl-forms")).unwrap();
    let public: stbl_core::forms::Form =
        serde_yaml::from_str(&fs::read_to_string(root.join("contact.yaml")).unwrap()).unwrap();
    let manifest = stbl_core::forms::FormManifest {
        version: 1,
        forms: std::collections::BTreeMap::from([("/cgi-bin/contact".into(), public)]),
    };
    let own_manifest = root.join("stbl-forms/forms.yaml");
    fs::write(&own_manifest, serde_yaml::to_string(&manifest).unwrap()).unwrap();
    let private = root.join("receiver.yaml");
    fs::write(&private, format!("rate_dir: {}\nsendmail: {}\nsites:\n  example:\n    website_root: {}\n    forms:\n      /cgi-bin/contact:\n        sender: forms@example.org\n        recipient: owner@example.org\n        subject: Contact\n  other-site:\n    website_root: {}\n    forms:\n      /cgi-bin/contact:\n        sender: forms@example.org\n        recipient: other@example.org\n        subject: Other\n", root.join("rate").display(), root.join("sendmail").display(), root.display(), root.join("missing").display())).unwrap();
    assert!(form_receiver::load_config(&private).is_err());
    assert!(
        fixture
            .cgi("message=Hello&_stbl_website=", "application/json")
            .starts_with("Status: 200")
    );
    fs::create_dir_all(root.join("missing/stbl-forms")).unwrap();
    fs::write(root.join("missing/stbl-forms/forms.yaml"), "not a manifest").unwrap();
    assert!(
        fixture
            .cgi("message=Hello&_stbl_website=", "application/json")
            .starts_with("Status: 200")
    );
    fs::write(own_manifest, "not a manifest").unwrap();
    assert!(
        fixture
            .cgi("message=Hello&_stbl_website=", "application/json")
            .starts_with("Status: 500")
    );
}

#[test]
fn sendmail_deadline_covers_full_pipe_and_wait_and_reaps_children() {
    use std::time::{Duration, Instant};
    for reads_input in [false, true] {
        let fixture = Fixture::new(10);
        let root = fixture.dir.path();
        let private = root.join("receiver.yaml");
        let config = fs::read_to_string(&private)
            .unwrap()
            .replace("max_request_bytes: 1024", "max_request_bytes: 1048576");
        fs::write(&private, format!("sendmail_timeout_seconds: 1\n{config}")).unwrap();
        let script = if reads_input {
            "#!/bin/sh\ncat > /dev/null\necho \"$$\" > \"$0.pid\"\nexec sleep 30\n"
        } else {
            "#!/bin/sh\necho \"$$\" > \"$0.pid\"\nsleep 30 &\necho \"$!\" > \"$0.helper\"\nwait\n"
        };
        fs::write(root.join("sendmail"), script).unwrap();
        let body = format!("message={}&_stbl_website=", "x".repeat(128 * 1024));
        let start = Instant::now();
        let response = fixture.cgi(&body, "application/json");
        assert!(response.starts_with("Status: 503"), "{response}");
        assert!(response.contains("Delivery is uncertain"), "{response}");
        assert!(start.elapsed() < Duration::from_secs(3));
        let pid = fs::read_to_string(root.join("sendmail.pid")).unwrap();
        assert!(
            !std::path::Path::new(&format!("/proc/{}", pid.trim())).exists(),
            "direct mailer child was not reaped"
        );
        let helper = root.join("sendmail.helper");
        if helper.exists() {
            let pid = fs::read_to_string(helper).unwrap();
            if let Ok(stat) = fs::read_to_string(format!("/proc/{}/stat", pid.trim())) {
                assert!(
                    stat.split_once(") ").unwrap().1.starts_with('Z'),
                    "helper still running: {stat}"
                );
            }
        }
        fs::write(root.join("sendmail"), "#!/bin/sh\ncat > \"$0.message\"\n").unwrap();
        assert!(
            fixture
                .cgi("message=Recovered&_stbl_website=", "application/json")
                .starts_with("Status: 200")
        );
    }
}

#[test]
fn global_rate_cap_applies_across_processes_and_client_addresses() {
    let fixture = Fixture::new(20);
    let path = fixture.dir.path().join("receiver.yaml");
    let original = fs::read_to_string(&path).unwrap();
    fs::write(&path, format!("global_requests_per_minute: 2\n{original}")).unwrap();
    let body = "message=&_stbl_website=";
    for index in 1..=4 {
        let mut child = fixture
            .command(body, "application/json")
            .env("REMOTE_ADDR", format!("192.0.2.{index}"))
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(body.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        let response = String::from_utf8(output.stdout).unwrap();
        let expected = if index <= 2 {
            "Status: 400"
        } else {
            "Status: 429"
        };
        assert!(response.starts_with(expected), "{response}");
    }
    assert!(!fixture.dir.path().join("sendmail.message").exists());
}
