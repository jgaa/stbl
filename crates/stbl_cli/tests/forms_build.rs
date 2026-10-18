use std::fs;
use std::path::Path;
use std::process::Command;

use stbl_cli::assets;
use stbl_core::assets::AssetRelPath;
use stbl_core::media::{ImagePlanInput, VideoPlanInput};
use stbl_core::model::Project;
use tempfile::TempDir;

const DEFINITION: &str = include_str!("../../../examples/forms/assets/forms/contact.yaml");

fn fixture(root: &Path) {
    fs::create_dir_all(root.join("articles")).unwrap();
    fs::create_dir_all(root.join("assets/forms")).unwrap();
    fs::write(
        root.join("stbl.yaml"),
        include_str!("../../../examples/forms/stbl.yaml"),
    )
    .unwrap();
    fs::write(root.join("assets/forms/contact.yaml"), DEFINITION).unwrap();
    fs::write(
        root.join("articles/index.md"),
        include_str!("../../../examples/forms/articles/index.md"),
    )
    .unwrap();
}

fn command(root: &Path, args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_stbl_cli"))
        .arg("--no-writeback")
        .arg("--source-dir")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn planned_definition_has_stable_name_and_changes_render_fingerprint() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fixture(root);
    let config = stbl_core::config::load_site_config(&root.join("stbl.yaml")).unwrap();
    let docs = stbl_cli::walk::walk_content(
        root,
        &root.join("articles"),
        stbl_core::header::UnknownKeyPolicy::Error,
        false,
    )
    .unwrap();
    let project = Project {
        root: root.into(),
        config,
        content: stbl_core::assemble::assemble_site(docs).unwrap(),
        image_alpha: Default::default(),
        image_variants: Default::default(),
        video_variants: Default::default(),
    };
    let plan = || {
        let (mut index, mut lookup) = assets::discover_assets_for_theme(root, "minimal").unwrap();
        stbl_cli::form_deployment::include_form_deployment(&project, &mut index, &mut lookup)
            .unwrap();
        stbl_core::plan::build_plan(
            &project,
            &index,
            &ImagePlanInput::default(),
            &VideoPlanInput::default(),
        )
    };
    let before = plan();
    for name in [
        "forms.yaml",
        "stbl-form-receiver",
        "receiver.yaml.example",
        "nginx.conf.example",
        "nginx-http.conf.example",
        "README.md",
    ] {
        let path = std::path::PathBuf::from(format!("stbl-forms/{name}"));
        assert!(
            before
                .tasks
                .iter()
                .any(|task| task.outputs.iter().any(|output| output.path == path))
        );
    }
    assert!(!root.join("stbl-forms").exists());
    let artifact = before.tasks.iter().find(|task| matches!(&task.kind, stbl_core::model::TaskKind::CopyAsset { rel, .. } if rel == &AssetRelPath("forms/contact.yaml".into()))).unwrap();
    assert_eq!(
        artifact.outputs[0].path,
        Path::new("artifacts/forms/contact.yaml")
    );
    fs::write(
        root.join("assets/forms/contact.yaml"),
        DEFINITION.replace("Your message", "Your feedback"),
    )
    .unwrap();
    let after = plan();
    let render = |plan: &stbl_core::model::BuildPlan| {
        plan.tasks
            .iter()
            .find(|task| matches!(task.kind, stbl_core::model::TaskKind::RenderPage { .. }))
            .unwrap()
            .inputs_fingerprint
            .clone()
    };
    assert_ne!(render(&before), render(&after));
    assert!(!root.join("artifacts").exists());
}

#[test]
fn forms_build_with_every_theme_and_definition_edits_rebuild_cached_html() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("source");
    let out = temp.path().join("out");
    let cache = temp.path().join("cache.sqlite");
    fixture(&root);
    // A landing container with both an external definition and an inline form.
    let inline = format!(":::form\n{DEFINITION}:::\n");
    let markdown = format!(
        "---\ntitle: Contact\nlayout: landing\n---\n::: section\n@[form](path=\"forms/contact.yaml\")\n\n{inline}:::\n"
    );
    fs::write(root.join("articles/index.md"), &markdown).unwrap();
    command(&root, &["scan"]);
    command(&root, &["plan"]);
    assert_eq!(
        fs::read_to_string(root.join("articles/index.md")).unwrap(),
        markdown
    );
    assert!(!out.exists());
    for theme in ["stbl", "minimal", "paper", "mono", "liberty", "product"] {
        command(
            &root,
            &[
                "build",
                "--theme",
                theme,
                "--out",
                out.to_str().unwrap(),
                "--cache-path",
                cache.to_str().unwrap(),
                "--precompress",
                "false",
            ],
        );
        let html = fs::read_to_string(out.join("index.html")).unwrap();
        assert_eq!(html.matches("<form ").count(), 2, "{theme}: {html}");
        assert!(!html.contains("@[form]"));
        assert!(html.contains("novalidate"));
        assert!(html.contains("forms.")); // cache-busted script
        assert!(html.contains("<section"));
        assert_eq!(
            fs::read_to_string(out.join("artifacts/forms/contact.yaml")).unwrap(),
            DEFINITION
        );
        let css = fs::read_dir(out.join("artifacts/css"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("common.")
            })
            .unwrap();
        assert!(fs::read_to_string(css).unwrap().contains(".stbl-form"));
    }
    fs::write(
        root.join("assets/forms/contact.yaml"),
        DEFINITION.replace("Your message", "Your feedback"),
    )
    .unwrap();
    command(
        &root,
        &[
            "build",
            "--theme",
            "product",
            "--out",
            out.to_str().unwrap(),
            "--cache-path",
            cache.to_str().unwrap(),
            "--precompress",
            "false",
        ],
    );
    let html = fs::read_to_string(out.join("index.html")).unwrap();
    assert!(html.contains("Your feedback"));
    assert!(html.contains("Your message")); // Inline definition remains independent.
}

#[test]
fn malformed_inline_forms_fail_assembly_and_bad_assets_fail_discovery() {
    let temp = TempDir::new().unwrap();
    fixture(temp.path());
    fs::write(
        temp.path().join("articles/index.md"),
        "---\ntitle: Contact\n---\n:::form\naction: /contact\nfields: []\n:::\n",
    )
    .unwrap();
    let docs = stbl_cli::walk::walk_content(
        temp.path(),
        &temp.path().join("articles"),
        stbl_core::header::UnknownKeyPolicy::Error,
        false,
    )
    .unwrap();
    assert!(stbl_core::assemble::assemble_site(docs).is_err());
    fs::write(
        temp.path().join("assets/forms/contact.yaml"),
        "action: /contact\nfields: []\n",
    )
    .unwrap();
    assert!(assets::discover_assets_for_theme(temp.path(), "stbl").is_err());
}

#[cfg(unix)]
#[test]
fn deployed_output_runs_cgi_using_manifest_without_source_or_wrapper() {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Stdio;

    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source");
    fixture(&source);
    let mut page = fs::read_to_string(source.join("articles/index.md")).unwrap();
    page.push_str("\n:::form\naction: /cgi-bin/feedback\nfields:\n  - name: feedback\n    type: text\n    required: true\n:::\n");
    fs::write(source.join("articles/index.md"), page).unwrap();
    let out = temp.path().join("out");
    command(
        &source,
        &[
            "build",
            "--out",
            out.to_str().unwrap(),
            "--no-cache",
            "--precompress",
            "false",
        ],
    );
    let deployed = temp.path().join("deployed");
    fs::rename(&out, &deployed).unwrap();
    fs::remove_dir_all(&source).unwrap();
    let receiver = deployed.join("stbl-forms/stbl-form-receiver");
    assert_ne!(
        fs::metadata(&receiver).unwrap().permissions().mode() & 0o111,
        0
    );
    let manifest_path = deployed.join("stbl-forms/forms.yaml");
    let manifest = fs::read_to_string(&manifest_path).unwrap();
    let parsed: stbl_core::forms::FormManifest = serde_yaml::from_str(&manifest).unwrap();
    assert_eq!(parsed.version, 1);
    assert_eq!(parsed.forms.len(), 2);
    assert!(parsed.forms.contains_key("/cgi-bin/contact"));
    assert!(parsed.forms.contains_key("/cgi-bin/feedback"));
    let nginx = fs::read_to_string(deployed.join("stbl-forms/nginx.conf.example")).unwrap();
    assert!(nginx.contains("location ^~ /stbl-forms/"));
    assert!(nginx.contains("STBL_FORM_CONFIG"));
    assert!(
        !nginx
            .lines()
            .any(|line| line.trim_start().starts_with("server {"))
    );
    let rate = temp.path().join("rate");
    fs::create_dir(&rate).unwrap();
    let sendmail = temp.path().join("sendmail");
    fs::write(&sendmail, "#!/bin/sh\ncat > \"$0.message\"\n").unwrap();
    fs::set_permissions(&sendmail, fs::Permissions::from_mode(0o700)).unwrap();
    let private_path = temp.path().join("private.yaml");
    let private = format!(
        "rate_dir: {}\nsendmail: {}\nrequests_per_minute: 20\nsites:\n  example:\n    website_root: {}\n    forms:\n      /cgi-bin/contact:\n        sender: forms@example.org\n        recipient: owner@example.org\n        subject: Contact\n        reply_to_field: email\n      /cgi-bin/feedback:\n        sender: forms@example.org\n        recipient: feedback@example.org\n        subject: Feedback\n",
        rate.display(),
        sendmail.display(),
        deployed.display()
    );
    fs::write(&private_path, &private).unwrap();
    let invoke = |method: &str, endpoint: &str, body: &str| {
        let mut child = Command::new(&receiver)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("STBL_FORM_CONFIG", &private_path)
            .env("STBL_FORM_SITE", "example")
            .env("SCRIPT_NAME", endpoint)
            .env("REQUEST_METHOD", method)
            .env("CONTENT_TYPE", "application/x-www-form-urlencoded")
            .env("CONTENT_LENGTH", body.len().to_string())
            .env("REMOTE_ADDR", "127.0.0.1")
            .env("HTTP_ACCEPT", "application/json")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(body.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout).unwrap()
    };
    assert!(invoke("GET", "/cgi-bin/contact", "").starts_with("Status: 405"));
    assert!(
        invoke(
            "POST",
            "/cgi-bin/contact",
            "message=Hello&email=bad&_stbl_website="
        )
        .starts_with("Status: 400")
    );
    assert!(
        invoke(
            "POST",
            "/cgi-bin/contact",
            "message=Hello&email=visitor%40example.org&_stbl_website="
        )
        .contains("\"success\":true")
    );
    let mail = fs::read_to_string(temp.path().join("sendmail.message")).unwrap();
    assert!(mail.contains("To: owner@example.org\n"));
    assert!(mail.contains("Reply-To: visitor@example.org\n"));
    assert!(
        invoke(
            "POST",
            "/cgi-bin/feedback",
            "feedback=Thanks&_stbl_website="
        )
        .contains("\"success\":true")
    );
    assert!(
        fs::read_to_string(temp.path().join("sendmail.message"))
            .unwrap()
            .contains("To: feedback@example.org\n")
    );
    assert!(
        invoke("POST", "/cgi-bin/unknown", "message=Hello&_stbl_website=")
            .starts_with("Status: 404")
    );

    // Generated manifests cannot replace private delivery settings or be silently ignored.
    fs::write(&manifest_path, manifest.replace("version: 1", "version: 2")).unwrap();
    assert!(stbl_cli::form_receiver::load_config(&private_path).is_err());
    fs::write(&manifest_path, &manifest).unwrap();
    for invalid in [
        private.replace("/cgi-bin/contact:", "/cgi-bin/missing:"),
        private.replace(&deployed.to_string_lossy().to_string(), "relative/site"),
        private.replace(
            "subject: Contact",
            "subject: Contact\n        fields: []\n        definition: duplicate.yaml",
        ),
    ] {
        fs::write(&private_path, invalid).unwrap();
        assert!(stbl_cli::form_receiver::load_config(&private_path).is_err());
    }
}

#[test]
fn conflicting_definitions_for_one_endpoint_fail_build() {
    let temp = TempDir::new().unwrap();
    fixture(temp.path());
    fs::write(
        temp.path().join("articles/index.md"),
        format!(
            "---\ntitle: Contact\n---\n:::form\n{}:::\n",
            DEFINITION.replace("required: true", "required: false")
        ),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_stbl_cli"))
        .arg("--source-dir")
        .arg(temp.path())
        .arg("plan")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("conflicting validation rules"));
}

#[test]
fn sites_without_forms_do_not_get_deployment_files() {
    let temp = TempDir::new().unwrap();
    fixture(temp.path());
    fs::remove_file(temp.path().join("assets/forms/contact.yaml")).unwrap();
    fs::write(
        temp.path().join("articles/index.md"),
        "---\ntitle: Plain page\n---\nNo forms here.\n",
    )
    .unwrap();
    let out = temp.path().join("out");
    command(
        temp.path(),
        &[
            "build",
            "--out",
            out.to_str().unwrap(),
            "--no-cache",
            "--precompress",
            "false",
        ],
    );
    assert!(!out.join("stbl-forms").exists());
}
