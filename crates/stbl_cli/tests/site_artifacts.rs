use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

fn fixture(root: &Path, icon: Option<&str>) {
    fs::create_dir_all(root.join("articles")).unwrap();
    fs::create_dir_all(root.join("artifacts")).unwrap();
    let icon = icon
        .map(|path| format!("  icon: {path}\n"))
        .unwrap_or_default();
    fs::write(root.join("stbl.yaml"), format!(
        "site:\n  id: artifacts-test\n  title: Artifacts\n  base_url: https://example.com/blog/\n  language: en\n{icon}assets:\n  cache_busting: true\n"
    )).unwrap();
    fs::write(
        root.join("articles/index.md"),
        "---\ntitle: Home\n---\nHello\n",
    )
    .unwrap();
}

fn command(root: &Path, args: &[&str]) -> Output {
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
    output
}

fn build(root: &Path) -> Output {
    command(
        root,
        &[
            "build",
            "--out",
            "out",
            "--cache-path",
            "cache",
            "--precompress",
            "false",
        ],
    )
}

#[test]
fn blog_homepage_description_updates_in_cached_builds() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fixture(root, None);
    let config = stbl_core::config::load_site_config(&root.join("stbl.yaml")).unwrap();
    let output = root.join("out").join(
        stbl_core::url::UrlMapper::new(&config)
            .map("index")
            .primary_output,
    );
    for (header, expected) in [
        (
            "description: C++ & performance\nabstract: Listing summary\n",
            Some("C++ &amp; performance"),
        ),
        (
            "description: \nabstract: Listing summary\n",
            Some("Listing summary"),
        ),
        ("", None),
    ] {
        let source = format!("---\ntitle: Home\ntemplate: blog_index\n{header}---\nHello\n");
        fs::write(root.join("articles/index.md"), &source).unwrap();
        build(root);
        let html = fs::read_to_string(&output).unwrap();
        let href = stbl_core::url::UrlMapper::new(&config).map("index").href;
        let canonical = stbl_core::url::UrlMapper::absolute_href(&config.site.base_url, &href)
            .replace('/', "&#x2f;");
        assert!(html.contains(&format!("<link rel=\"canonical\" href=\"{canonical}\">")));
        if let Some(expected) = expected {
            assert!(html.contains(&format!(
                "<meta name=\"description\" content=\"{expected}\">"
            )));
        } else {
            assert!(!html.contains("name=\"description\""));
        }
        assert_eq!(
            fs::read_to_string(root.join("articles/index.md")).unwrap(),
            source
        );
    }
}

#[test]
fn open_graph_images_are_planned_published_and_updated_in_cached_builds() {
    use stbl_core::url::UrlMapper;
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fixture(root, None);
    fs::create_dir_all(root.join("images")).unwrap();
    for name in [
        "social.png",
        "page.png",
        "default.png",
        "front-a.png",
        "front-b.png",
    ] {
        image::RgbImage::new(8, 8)
            .save(root.join("images").join(name))
            .unwrap();
    }
    let config_source = fs::read_to_string(root.join("stbl.yaml")).unwrap();
    let config = stbl_core::config::load_site_config(&root.join("stbl.yaml")).unwrap();
    let mapper = UrlMapper::new(&config);
    let post_output = root.join("out").join(mapper.map("post").primary_output);
    let home_output = root.join("out").join(mapper.map("index").primary_output);
    let home = "---\ntitle: Home\ntemplate: blog_index\nbanner: front-a.png\n---\nWelcome\n";
    fs::write(root.join("articles/index.md"), home).unwrap();
    let cases = [
        (
            "social-image: social.png\nbanner: page.png\n",
            "{}",
            Some("images/social.png"),
        ),
        ("banner: page.png\n", "{}", Some("images/page.png")),
        ("", "{}", None),
        (
            "",
            "{default_image: default.png}",
            Some("images/default.png"),
        ),
        (
            "",
            "{default_image: default.png, use_frontpage_banner: true}",
            Some("images/default.png"),
        ),
        (
            "",
            "{use_frontpage_banner: true}",
            Some("images/front-a.png"),
        ),
    ];
    for (header, settings, expected) in cases {
        let source =
            format!("---\ntitle: Post\ndescription: A post description\n{header}---\nBody\n");
        fs::write(root.join("articles/post.md"), &source).unwrap();
        fs::write(
            root.join("stbl.yaml"),
            format!("{config_source}seo:\n  open_graph: {settings}\n"),
        )
        .unwrap();
        command(root, &["plan"]);
        assert_eq!(
            fs::read_to_string(root.join("articles/post.md")).unwrap(),
            source
        );
        build(root);
        let html = fs::read_to_string(&post_output).unwrap();
        assert!(html.contains("<meta property=\"og:type\" content=\"article\">"));
        assert!(html.contains("<meta property=\"og:description\" content=\"A post description\">"));
        if let Some(path) = expected {
            let absolute =
                UrlMapper::absolute_href(&config.site.base_url, path).replace('/', "&#x2f;");
            assert!(html.contains(&format!(
                "<meta property=\"og:image\" content=\"{absolute}\">"
            )));
            assert!(root.join("out").join(path).is_file());
        } else {
            assert!(!html.contains("property=\"og:image\""));
        }
        let home_html = fs::read_to_string(&home_output).unwrap();
        assert!(home_html.contains("<meta property=\"og:type\" content=\"website\">"));
        assert!(home_html.contains("images&#x2f;front-a.png"));
    }
    // The post source stays unchanged while its inherited frontpage image changes.
    fs::write(
        root.join("articles/index.md"),
        home.replace("front-a.png", "front-b.png"),
    )
    .unwrap();
    build(root);
    let html = fs::read_to_string(&post_output).unwrap();
    assert!(html.contains("images&#x2f;front-b.png"));
    assert!(!html.contains("images&#x2f;front-a.png"));

    fs::write(
        root.join("articles/post.md"),
        "---\ntitle: Post\nsocial-image: https://cdn.example.com/social.png\n---\nBody\n",
    )
    .unwrap();
    build(root);
    let html = fs::read_to_string(&post_output).unwrap();
    assert!(html.contains(
        "<meta property=\"og:image\" content=\"https:&#x2f;&#x2f;cdn.example.com&#x2f;social.png\">"
    ));
}

#[test]
fn generates_robots_and_warns_for_missing_favicon() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fixture(root, None);
    let plan = command(root, &["plan"]);
    assert!(String::from_utf8_lossy(&plan.stdout).contains("GenerateRobots"));
    assert!(!root.join("out").exists());
    assert!(!root.join("artifacts/robots.txt").exists());
    let output = build(root);
    assert!(String::from_utf8_lossy(&output.stderr).contains("warning: no favicon found"));
    assert_eq!(
        fs::read_to_string(root.join("out/robots.txt")).unwrap(),
        "User-agent: *\nAllow: /\n\nSitemap: https://example.com/blog/sitemap.xml\n"
    );
    let config = fs::read_to_string(root.join("stbl.yaml")).unwrap();
    fs::write(
        root.join("stbl.yaml"),
        config.replace("https://example.com/blog/", "https://example.org/"),
    )
    .unwrap();
    build(root);
    assert!(
        fs::read_to_string(root.join("out/robots.txt"))
            .unwrap()
            .contains("https://example.org/sitemap.xml")
    );
}

#[test]
fn copies_artifact_overrides_and_updates_cached_builds() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fixture(root, None);
    build(root);
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect width="32" height="32"/></svg>"#;
    fs::write(root.join("artifacts/favicon.svg"), svg).unwrap();
    fs::write(root.join("artifacts/favicon.ico"), b"ico bytes").unwrap();
    fs::write(
        root.join("artifacts/robots.txt"),
        "User-agent: *\nDisallow: /private/\n",
    )
    .unwrap();
    fs::create_dir(root.join("artifacts/favicon-directory")).unwrap();
    fs::write(root.join("artifacts/unrelated.txt"), "ignored").unwrap();
    let plan = command(root, &["plan"]);
    assert!(!String::from_utf8_lossy(&plan.stdout).contains("GenerateRobots"));
    let output = build(root);
    assert!(!String::from_utf8_lossy(&output.stderr).contains("no favicon found"));
    assert_eq!(
        fs::read_to_string(root.join("out/robots.txt")).unwrap(),
        "User-agent: *\nDisallow: /private/\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("out/favicon.svg")).unwrap(),
        svg
    );
    assert_eq!(
        fs::read(root.join("out/favicon.ico")).unwrap(),
        b"ico bytes"
    );
    assert!(!root.join("out/favicon-directory").exists());
    assert!(!root.join("out/unrelated.txt").exists());
    let html = fs::read_to_string(root.join("out/index.html")).unwrap();
    assert!(html.contains("favicon.svg"));
    assert!(!html.contains("rel=\"icon\" type=\"image/png\""));
    fs::write(
        root.join("artifacts/robots.txt"),
        "User-agent: *\nDisallow: /\n",
    )
    .unwrap();
    build(root);
    assert_eq!(
        fs::read_to_string(root.join("out/robots.txt")).unwrap(),
        "User-agent: *\nDisallow: /\n"
    );
    fs::remove_file(root.join("artifacts/robots.txt")).unwrap();
    fs::remove_file(root.join("artifacts/favicon.svg")).unwrap();
    fs::remove_file(root.join("artifacts/favicon.ico")).unwrap();
    let output = build(root);
    assert!(String::from_utf8_lossy(&output.stderr).contains("no favicon found"));
    assert!(
        fs::read_to_string(root.join("out/robots.txt"))
            .unwrap()
            .contains("Sitemap:")
    );
    assert!(!root.join("out/favicon.svg").exists());
    assert!(
        !fs::read_to_string(root.join("out/index.html"))
            .unwrap()
            .contains("favicon.svg")
    );
}

#[test]
fn configured_icon_takes_precedence_over_artifact_favicons() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fixture(root, Some("icon.svg"));
    fs::write(root.join("icon.svg"), r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect width="32" height="32"/></svg>"#).unwrap();
    fs::write(root.join("artifacts/favicon.ico"), b"fallback").unwrap();
    let output = build(root);
    assert!(!String::from_utf8_lossy(&output.stderr).contains("no favicon found"));
    assert!(!root.join("out/favicon.ico").exists());
    assert!(
        fs::read_to_string(root.join("out/index.html"))
            .unwrap()
            .contains("site-icon.")
    );
}

#[test]
fn links_favicon_with_a_size_in_its_name() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fixture(root, None);
    fs::write(root.join("artifacts/favicon-32x32.png"), b"png bytes").unwrap();
    let output = build(root);
    assert!(!String::from_utf8_lossy(&output.stderr).contains("no favicon found"));
    assert_eq!(
        fs::read(root.join("out/favicon-32x32.png")).unwrap(),
        b"png bytes"
    );
    assert!(
        fs::read_to_string(root.join("out/index.html"))
            .unwrap()
            .contains("favicon-32x32.png")
    );
}

#[test]
fn copies_well_known_files_recursively_and_updates_cached_builds() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fixture(root, None);
    let source = root.join("artifacts/well-known");
    fs::create_dir_all(source.join("acme-challenge")).unwrap();
    fs::create_dir_all(root.join("assets/.well-known")).unwrap();
    fs::write(root.join("assets/.well-known/security.txt"), "overridden").unwrap();
    fs::write(
        source.join("security.txt"),
        "Contact: mailto:security@example.com\n",
    )
    .unwrap();
    fs::write(source.join("acme-challenge/token"), b"challenge\x00bytes").unwrap();
    let plan = command(root, &["plan"]);
    assert!(String::from_utf8_lossy(&plan.stdout).contains(".well-known/security.txt"));
    assert!(String::from_utf8_lossy(&plan.stdout).contains(".well-known/acme-challenge/token"));
    assert!(!root.join("out").exists());
    build(root);
    let output = root.join("out/.well-known");
    assert_eq!(
        fs::read_to_string(output.join("security.txt")).unwrap(),
        "Contact: mailto:security@example.com\n"
    );
    assert_eq!(
        fs::read(output.join("acme-challenge/token")).unwrap(),
        b"challenge\x00bytes"
    );
    assert!(!root.join("out/artifacts/.well-known").exists());
    assert!(!root.join("out/well-known").exists());
    fs::write(
        source.join("security.txt"),
        "Contact: mailto:updated@example.com\n",
    )
    .unwrap();
    fs::remove_file(source.join("acme-challenge/token")).unwrap();
    build(root);
    assert_eq!(
        fs::read_to_string(output.join("security.txt")).unwrap(),
        "Contact: mailto:updated@example.com\n"
    );
    assert!(!output.join("acme-challenge/token").exists());
    fs::remove_file(source.join("security.txt")).unwrap();
    build(root);
    assert_eq!(
        fs::read_to_string(output.join("security.txt")).unwrap(),
        "overridden"
    );
}
