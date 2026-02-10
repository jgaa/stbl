use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{Context, Result};
use tiny_http::{Header, Method, Response, Server, StatusCode};

pub struct PreviewOpts {
    pub site_dir: Option<PathBuf>,
    pub out_dir: Option<PathBuf>,
    pub host: String,
    pub port: u16,
    pub no_open: bool,
    pub index: String,
}

#[allow(dead_code)]
pub struct PreviewHandle {
    pub url: String,
    shutdown: Arc<AtomicBool>,
    join: JoinHandle<Result<()>>,
}

impl PreviewHandle {
    #[allow(dead_code)]
    pub fn stop(self) -> Result<()> {
        self.shutdown.store(true, Ordering::SeqCst);
        match self.join.join() {
            Ok(result) => result,
            Err(_) => anyhow::bail!("preview thread panicked"),
        }
    }
}

pub fn run_preview(opts: PreviewOpts) -> Result<()> {
    let cwd = std::env::current_dir().context("failed to resolve current directory")?;
    let site_dir = resolve_site_dir(&cwd, opts.site_dir)?;
    let out_dir = resolve_out_dir(&cwd, &site_dir, opts.out_dir)?;
    validate_out_dir(&out_dir)?;

    let (server, addr) = bind_server(&opts.host, opts.port)?;
    let url = preview_url(&opts.host, addr);

    println!("Preview: {url}");
    println!("Serving: {}", out_dir.display());

    if !opts.no_open {
        if let Err(err) = webbrowser::open(&url) {
            eprintln!("warning: failed to open browser: {err}");
        }
    }

    serve_loop(server, out_dir, opts.index, None)
}

#[allow(dead_code)]
pub fn spawn_preview(opts: PreviewOpts) -> Result<PreviewHandle> {
    let cwd = std::env::current_dir().context("failed to resolve current directory")?;
    let site_dir = resolve_site_dir(&cwd, opts.site_dir)?;
    let out_dir = resolve_out_dir(&cwd, &site_dir, opts.out_dir)?;
    validate_out_dir(&out_dir)?;

    let (server, addr) = bind_server(&opts.host, opts.port)?;
    let url = preview_url(&opts.host, addr);
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_thread = shutdown.clone();
    let index = opts.index.clone();
    let join = thread::spawn(move || serve_loop(server, out_dir, index, Some(shutdown_thread)));

    Ok(PreviewHandle { url, shutdown, join })
}

fn resolve_site_dir(cwd: &Path, site_dir: Option<PathBuf>) -> Result<PathBuf> {
    let site_dir = site_dir.unwrap_or_else(|| cwd.to_path_buf());
    if site_dir.is_absolute() {
        Ok(site_dir)
    } else {
        Ok(cwd.join(site_dir))
    }
}

fn resolve_out_dir(cwd: &Path, site_dir: &Path, out_dir: Option<PathBuf>) -> Result<PathBuf> {
    Ok(match out_dir {
        Some(path) => {
            if path.is_absolute() {
                path
            } else {
                cwd.join(path)
            }
        }
        None => site_dir.join("out"),
    })
}

fn validate_out_dir(out_dir: &Path) -> Result<()> {
    if !out_dir.exists() || !out_dir.is_dir() {
        anyhow::bail!("Output dir does not exist. Run `stbl_cli build` first or pass --out.");
    }
    Ok(())
}

fn bind_server(host: &str, port: u16) -> Result<(Server, SocketAddr)> {
    let addr = format!("{host}:{port}");
    let server = Server::http(&addr)
        .map_err(|err| anyhow::anyhow!("failed to bind to {addr}: {err}"))?;
    let actual = server
        .server_addr()
        .to_ip()
        .ok_or_else(|| anyhow::anyhow!("failed to resolve socket address"))?;
    Ok((server, actual))
}

fn preview_url(host: &str, addr: SocketAddr) -> String {
    format!("http://{host}:{}/", addr.port())
}

fn serve_loop(
    server: Server,
    out_dir: PathBuf,
    index: String,
    shutdown: Option<Arc<AtomicBool>>,
) -> Result<()> {
    loop {
        if let Some(flag) = &shutdown {
            if flag.load(Ordering::SeqCst) {
                break;
            }
        }

        let request = match server.recv_timeout(Duration::from_millis(200)) {
            Ok(Some(request)) => request,
            Ok(None) => continue,
            Err(err) => return Err(err.into()),
        };

        let response = match handle_request(&request, &out_dir, &index) {
            Ok(response) => response,
            Err(err) => {
                eprintln!("warning: {err}");
                Response::from_string("Internal Server Error")
                    .with_status_code(StatusCode(500))
                    .boxed()
            }
        };

        if let Err(err) = request.respond(response) {
            eprintln!("warning: failed to send response: {err}");
        }
    }
    Ok(())
}

fn handle_request(
    request: &tiny_http::Request,
    out_dir: &Path,
    index: &str,
) -> Result<Response<Box<dyn Read + Send>>> {
    if request.method() != &Method::Get && request.method() != &Method::Head {
        return Ok(Response::from_string("Method Not Allowed")
            .with_status_code(StatusCode(405))
            .boxed());
    }

    let rel_path = match sanitize_path(request.url(), index) {
        Some(path) => path,
        None => {
            return Ok(Response::from_string("Not Found")
                .with_status_code(StatusCode(404))
                .boxed());
        }
    };

    let mut full_path = out_dir.join(&rel_path);
    if full_path.is_dir() {
        full_path = full_path.join(index);
    }
    if !full_path.exists() || full_path.is_dir() {
        return Ok(Response::from_string("Not Found")
            .with_status_code(StatusCode(404))
            .boxed());
    }

    let mut file = File::open(&full_path)
        .with_context(|| format!("failed to open {}", full_path.display()))?;
    let file_size = file
        .metadata()
        .with_context(|| format!("failed to stat {}", full_path.display()))?
        .len();

    let content_type = content_type_header(&full_path);
    let accept_ranges = Header::from_bytes("Accept-Ranges", "bytes").expect("valid header");

    if let Some((start, end)) = parse_range_header(request, file_size)? {
        let length = end.saturating_sub(start).saturating_add(1);
        let content_range = Header::from_bytes(
            "Content-Range",
            format!("bytes {start}-{end}/{file_size}"),
        )
        .expect("valid header");
        let headers = vec![content_type, accept_ranges, content_range];

        if request.method() == &Method::Head {
            let response = Response::new(
                StatusCode(206),
                headers,
                std::io::empty(),
                Some(length as usize),
                None,
            )
            .boxed();
            return Ok(response);
        }

        file.seek(SeekFrom::Start(start))
            .with_context(|| format!("failed to seek {}", full_path.display()))?;
        let reader = file.take(length);
        let response = Response::new(
            StatusCode(206),
            headers,
            Box::new(reader) as Box<dyn Read + Send>,
            Some(length as usize),
            None,
        )
        .boxed();
        return Ok(response);
    }

    if request.method() == &Method::Head {
        let headers = vec![content_type, accept_ranges];
        return Ok(Response::new(
            StatusCode(200),
            headers,
            std::io::empty(),
            Some(file_size as usize),
            None,
        )
        .boxed());
    }

    let response = Response::from_file(file)
        .with_header(content_type)
        .with_header(accept_ranges)
        .boxed();
    Ok(response)
}

fn sanitize_path(url: &str, index: &str) -> Option<PathBuf> {
    let path = url.split('?').next().unwrap_or(url);
    let decoded = urlencoding::decode(path).ok()?;
    if decoded.contains('\\') {
        return None;
    }
    let trimmed = decoded.trim_start_matches('/');
    let effective = if trimmed.is_empty() { index } else { trimmed };
    let rel_path = Path::new(effective);

    let mut clean = PathBuf::new();
    for component in rel_path.components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    if clean.as_os_str().is_empty() {
        None
    } else {
        Some(clean)
    }
}

fn content_type_for(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "mp4" => "video/mp4",
        _ => "application/octet-stream",
    }
}

fn content_type_header(path: &Path) -> Header {
    Header::from_bytes("Content-Type", content_type_for(path)).expect("valid header")
}

fn parse_range_header(
    request: &tiny_http::Request,
    file_size: u64,
) -> Result<Option<(u64, u64)>> {
    let header = match request.headers().iter().find(|h| h.field.equiv("Range")) {
        Some(header) => header,
        None => return Ok(None),
    };
    let value = header.value.as_str().trim();
    let ranges = match value.strip_prefix("bytes=") {
        Some(ranges) => ranges,
        None => return Ok(None),
    };
    let (start_raw, end_raw) = match ranges.split_once('-') {
        Some(parts) => parts,
        None => return Ok(None),
    };

    let (start, end) = if start_raw.is_empty() {
        let suffix: u64 = match end_raw.parse() {
            Ok(value) => value,
            Err(_) => return Ok(None),
        };
        if suffix == 0 {
            return Ok(None);
        }
        if suffix >= file_size {
            (0, file_size.saturating_sub(1))
        } else {
            (file_size - suffix, file_size.saturating_sub(1))
        }
    } else {
        let start: u64 = match start_raw.parse() {
            Ok(value) => value,
            Err(_) => return Ok(None),
        };
        let end = if end_raw.is_empty() {
            file_size.saturating_sub(1)
        } else {
            match end_raw.parse() {
                Ok(value) => value,
                Err(_) => return Ok(None),
            }
        };
        (start, end.min(file_size.saturating_sub(1)))
    };

    if file_size == 0 || start >= file_size || end < start {
        return Ok(None);
    }
    Ok(Some((start, end)))
}
