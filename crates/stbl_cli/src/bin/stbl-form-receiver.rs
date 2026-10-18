use clap::Parser;
use stbl_cli::form_receiver::{self, Request, Response, Sendmail};
use std::path::PathBuf;

/// A CGI receiver. Invoke from a server-owned wrapper with fixed config and site arguments.
#[derive(Parser)]
struct Args {
    #[arg(long, env = "STBL_FORM_CONFIG")]
    config: PathBuf,
    #[arg(long, env = "STBL_FORM_SITE")]
    site: String,
}

fn main() {
    let args = Args::parse();
    let env = |key| std::env::var(key).unwrap_or_default();
    let accept = env("HTTP_ACCEPT");
    let response = match form_receiver::load_config_for_site(&args.config, &args.site) {
        Ok(config) => {
            let request = Request {
                method: &env("REQUEST_METHOD"),
                endpoint: &env("SCRIPT_NAME"),
                content_type: &env("CONTENT_TYPE"),
                content_length: &env("CONTENT_LENGTH"),
                remote_addr: &env("REMOTE_ADDR"),
            };
            form_receiver::process(
                &config,
                &args.site,
                &request,
                std::io::stdin().lock(),
                &Sendmail {
                    path: &config.sendmail,
                    timeout: std::time::Duration::from_secs(config.sendmail_timeout_seconds),
                },
            )
        }
        Err(e) => {
            eprintln!("form receiver configuration: {e:#}");
            Response::error(
                500,
                "The form is unavailable. Please contact the site administrator.",
            )
        }
    };
    print!("{}", response.cgi(form_receiver::wants_json(&accept)));
}
