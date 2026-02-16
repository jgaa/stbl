use std::fs;
use std::time::{Duration, Instant};

use stbl_cli::preview::{PreviewOpts, spawn_preview};
use tempfile::TempDir;

struct TerminalRestore;

impl Drop for TerminalRestore {
    fn drop(&mut self) {
        let _ = std::process::Command::new("stty")
            .arg("sane")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        let _ = std::process::Command::new("tput")
            .arg("cnorm")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
}

fn wait_for_ready(url: &str) {
    let start = Instant::now();
    loop {
        match ureq::get(url).call() {
            Ok(_) => return,
            Err(ureq::Error::Status(_, _)) => return,
            Err(_) => {
                if start.elapsed() > Duration::from_secs(2) {
                    panic!("preview server did not start in time");
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

#[test]
fn preview_serves_files() {
    let _terminal_restore = TerminalRestore;
    let temp = TempDir::new().expect("tempdir");
    let out_dir = temp.path().join("out");
    fs::create_dir_all(out_dir.join("artifacts/css")).expect("create dirs");
    fs::write(out_dir.join("index.html"), "<h1>Preview</h1>").expect("write index");
    fs::write(
        out_dir.join("artifacts/css/common.css"),
        "body { color: red; }",
    )
    .expect("write css");

    let handle = spawn_preview(PreviewOpts {
        site_dir: None,
        out_dir: Some(out_dir.clone()),
        host: "127.0.0.1".to_string(),
        port: 0,
        no_open: true,
        index: "index.html".to_string(),
    })
    .expect("spawn preview");

    let url = handle.url.clone();
    let result = std::panic::catch_unwind(|| {
        wait_for_ready(&url);
        let response = ureq::get(&url).call().expect("get /");
        assert_eq!(response.status(), 200);
        let body = response.into_string().expect("read body");
        assert!(body.contains("Preview"));

        let css_url = format!("{url}artifacts/css/common.css");
        let response = ureq::get(&css_url).call().expect("get css");
        assert_eq!(response.status(), 200);
        let body = response.into_string().expect("read css");
        assert!(body.contains("color: red"));
    });

    handle.stop().expect("stop preview");
    if let Err(err) = result {
        std::panic::resume_unwind(err);
    }
}
