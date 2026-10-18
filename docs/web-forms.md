# Web forms

Forms work as ordinary URL-encoded HTML POSTs without JavaScript. All embedded
themes include responsive form styles. With JavaScript, forms validate required
fields and any supplied email, submit with `fetch`, and announce the result
inside the form. Failures preserve the entered data; the submit button is disabled
while sending and after confirmed success. Every form has `novalidate`: without
JavaScript, the server performs all validation and returns an HTML page.

## Inline Markdown

Use a standalone container, on ordinary pages or inside landing-page components:

```yaml
:::form
action: /cgi-bin/contact
submit: Send message
fields:
  - name: name
    type: text
    label: Your name
  - name: email
    type: email
    label: Email address
    placeholder: Optional
  - name: message
    type: textarea
    label: Your message
    required: true
:::
```

The rendered HTML includes a standard POST action and labeled controls:

```html
<form class="stbl-form" method="post" action="/cgi-bin/contact" novalidate>
  <label class="stbl-form-field"><span>Email address</span><input type="email" name="email" placeholder="Optional"></label>
  <!-- Other configured fields, honeypot, submit button, and live status region. -->
</form>
```

`::: form` also works. Supported field types are `text`, `email`, and `textarea`.
`required` defaults to **false**. Labels default to the field name; placeholders
default to empty; submit text defaults to `Send message`. Field names must be
unique, 1–64 ASCII letters, digits, underscores, or hyphens, and cannot start with
an underscore. A form supports 1–64 fields. Actions must be root-relative paths
or HTTPS URLs. Prefer a same-origin endpoint; cross-origin AJAX requires CORS
support from your receiver, which the included CGI does not provide.

Labels wrap their controls, required fields are identified in text and with
ARIA, and status messages use a live region. Controls fill the available width,
use the theme's palette and fonts, and have visible keyboard focus indicators.
Custom templates should load the `js/forms.js` asset through `asset_manifest`, as
the embedded base templates do; custom CSS should retain the `.stbl-form` styles.

## Reusable YAML definitions

Save the same YAML, **without** the `:::` fences, as
`assets/forms/contact.yaml`. Add it to a Markdown page with the form macro:

```markdown
@[form](path="forms/contact.yaml")
```

Paths are relative to the site's `assets/` directory, regardless of the page's
location. Definitions must live under `forms/` and end in `.yaml` or `.yml`.
Macros must be enabled (the default). Like other macros, unresolved references
produce a visible warning. Invalid definition assets cause planning/build errors.

The asset pipeline still copies each source definition to
`artifacts/forms/contact.yaml`. It also collects external definitions and inline
`:::form` blocks into the generated validation manifest described below. Source
field names/types/required flags are the single source of validation rules.
Forms sharing an action must agree on those rules; presentation labels may vary.
Code examples are excluded. Definition changes invalidate incremental rendering.

## What to deploy

Build both executables once, then build the website normally:

```sh
cargo build --release -p stbl_cli
target/release/stbl_cli --source-dir examples/forms build --out /tmp/stbl-contact
```

When a site has forms, its output includes:

```text
OUT/
  ...normal website pages and assets...
  stbl-forms/
    forms.yaml                # generated public validation definitions
    stbl-form-receiver        # executable CGI receiver
    receiver.yaml.example     # template for PRIVATE server configuration
    nginx-http.conf.example   # shared request zones in http {} (install once)
    nginx.conf.example        # location blocks for an EXISTING vhost
    README.md                 # deployment instructions, shipped with the output
```

Copy **the entire output** to the deployed website directory, preserving modes,
for example `rsync -a OUT/ /var/www/cpp-freelancer.com/`. No setup bundle, source
website, source YAML copies, wrapper script, or Rust toolchain is needed on the
server. The CGI executable is copied from `stbl-form-receiver` next to the build
CLI. Install/distribute both executables together; `cargo build -p stbl_cli`
builds both. Use a release build on an OS/architecture/libc compatible with the
server. Copying a Linux executable does not make it portable to other targets.

All six deployment files have stable paths even with asset cache busting enabled.
They are planned before execution and participate in incremental copying/pruning.
`scan` and `plan` do not write them. Sites without forms get no deployment files.
`stbl-forms/` is reserved for generated deployment files.

Keep executable mode **0755** on `stbl-forms/stbl-form-receiver`. The website and
receiver must be owned by the deployment account, readable/executable but **not
writable** by the CGI user. The nginx deny block below prevents downloading the
deployment directory through HTTP. The generated examples contain placeholders,
not your real delivery configuration. Never put the actual private config in OUT.

A complete source example is in [`examples/forms`](../examples/forms/).

## Private receiver configuration

Install a local MTA providing `/usr/sbin/sendmail`; configure its queue/delivery.
On Ubuntu/Debian, install `postfix` and configure it for direct delivery or your
existing SMTP relay. Also install `nginx` and `fcgiwrap`, then enable the CGI socket:

```sh
sudo apt install postfix nginx fcgiwrap
sudo systemctl enable --now fcgiwrap.socket
sudo install -d -o root -g www-data -m 0750 /etc/stbl
sudo install -d -o www-data -g www-data -m 0700 /var/lib/stbl-forms
```

Copy `OUT/stbl-forms/receiver.yaml.example` to `/etc/stbl/forms.yaml`, then edit it.
For the C++ Freelancer site:

```yaml
rate_dir: /var/lib/stbl-forms
max_request_bytes: 32768
requests_per_minute: 5
global_requests_per_minute: 1000
sendmail_timeout_seconds: 10
sendmail: /usr/sbin/sendmail
sites:
  cpp-freelancer:
    website_root: /var/www/cpp-freelancer.com
    forms:
      /cgi-bin/contact:
        sender: contact@lastviking.eu
        recipient: jgaa@jgaa.com
        subject: Website contact form
        reply_to_field: email
```

Then protect the private file:

```sh
sudo chown root:www-data /etc/stbl/forms.yaml
sudo chmod 0640 /etc/stbl/forms.yaml
```

`website_root` is the **absolute directory containing the deployed website**.
The CGI reads `website_root/stbl-forms/forms.yaml`; no source-directory paths or
per-form definition paths are needed. The manifest is regenerated by stbl; edit
source Markdown/YAML and rebuild rather than editing the output. Deploy pages,
manifest, and receiver together. Missing/invalid manifests, unsupported manifest
versions, or configured endpoints absent from the manifest fail closed. Each CGI
request loads only the selected site's manifest; a missing or broken manifest
for another site does not disable this site.

Server configuration is an explicit allowlist of endpoint/mail policies. A form
in the manifest is not enabled until it is listed under the selected site's
`forms:`. Recipients, sender, subject, and forwarding settings always come from
this private configuration. Visitor POST fields and the public manifest cannot
change destinations. `reply_to_field` is optional and must name an email field;
a validated nonempty address becomes Reply-To. Empty optional email adds no header.

Add further site keys for multiple websites, each with its own `website_root` and
mail policies. Each vhost selects a **fixed** server-configured site key. Never
select a site from visitor-supplied Host headers, query parameters, or form fields.
The CGI uses `SCRIPT_NAME` to select an endpoint within that site's allowlist.

For compatibility, earlier configurations mapping a site directly to endpoints
with `definition:` or `fields:` still work. With `website_root`, omit both:
validation comes exclusively from the generated manifest.

## nginx: add blocks to the existing vhost

Install `stbl-forms/nginx-http.conf.example` **once** in nginx's `http {}` context,
before the vhosts. On Debian/Ubuntu, copy it to
`/etc/nginx/conf.d/stbl-forms-rate-limit.conf` (normally included from `http {}`).
It contains:

```nginx
limit_req_zone $binary_remote_addr zone=stbl_forms_per_ip:1m rate=10r/m;
limit_req_zone $server_name zone=stbl_forms_per_site:1m rate=60r/m;
```

These bounded-memory zones reject floods before a CGI process starts. Tune rates
and bursts for expected traffic and users sharing an IP. See nginx's
[request limiting documentation](https://nginx.org/en/docs/http/ngx_http_limit_req_module.html).

Leave the existing `server {}`, TLS configuration, `root`, and normal page handling
in place. Add these locations to that same vhost, or adapt the generated
`stbl-forms/nginx.conf.example`:

```nginx
# Block HTTP downloads of receiver/deployment files.
location ^~ /stbl-forms/ {
    return 404;
}

location = /cgi-bin/contact {
    limit_req zone=stbl_forms_per_ip burst=5 nodelay;
    limit_req zone=stbl_forms_per_site burst=10 nodelay;
    limit_req_status 429;
    include fastcgi_params;
    fastcgi_pass unix:/run/fcgiwrap.socket;
    fastcgi_param SCRIPT_FILENAME $document_root/stbl-forms/stbl-form-receiver;
    fastcgi_param STBL_FORM_CONFIG /etc/stbl/forms.yaml;
    fastcgi_param STBL_FORM_SITE cpp-freelancer;
    fastcgi_intercept_errors off;
}
```

This is **not** a replacement vhost and does not add a listener or change normal
site serving. Set `STBL_FORM_SITE` to the key in private configuration and
`STBL_FORM_CONFIG` to that file's absolute path. These FastCGI parameters are
server-owned environment values; the executable also accepts explicit `--config`
and `--site` arguments for CLI/CGI testing. No executable shell wrapper is needed.

`$document_root` must point to the same directory as `website_root`. For vhosts
using `alias`, symlink layouts, or preview proxies, use the actual absolute
receiver path in `SCRIPT_FILENAME`. Keep it fixed; never construct an executable
path from the request URI. Add one exact CGI location per form endpoint, with the
same executable/config/site parameters. Match the form action exactly.
`include fastcgi_params` must supply `SCRIPT_NAME`, `REQUEST_METHOD`,
`CONTENT_TYPE`, `CONTENT_LENGTH`, and `REMOTE_ADDR`; nginx forwards Accept for JSON
negotiation. If maintaining a custom parameter file, pass `HTTP_ACCEPT $http_accept`.
Behind a trusted proxy, configure nginx's real client address handling rather
than trusting arbitrary forwarded headers inside the receiver.

On Ubuntu/Debian the packaged fcgiwrap socket is `/run/fcgiwrap.socket`. Verify
its permissions allow nginx to connect and the CGI user to read private config
and deployed definitions, execute the binary, and write the rate directory.
Some hardened service configurations use a private `/tmp`; use a persistent
server path for production website files.

```sh
sudo nginx -t
sudo systemctl reload nginx
```

See the official [nginx FastCGI documentation](https://nginx.org/en/docs/http/ngx_http_fastcgi_module.html)
and [fcgiwrap manual](https://manpages.ubuntu.com/manpages/jammy/man8/fcgiwrap.8.html).

## stbl preview with nginx

The same receiver/manifest lives in the preview output. For example:

```sh
target/release/stbl_cli --source-dir /var/local/blog/cpp-freelancer.com \
  build --out /tmp/cpp-freelancer.com --preview --preview-port 8080
```

In private server config set `website_root: /tmp/cpp-freelancer.com`. In nginx's
exact CGI location set
`SCRIPT_FILENAME /tmp/cpp-freelancer.com/stbl-forms/stbl-form-receiver` and the same
private config/site parameters as above. Keep the deny location. Your existing
nginx preview vhost can serve the rest through:

```nginx
location / {
    proxy_pass http://127.0.0.1:8080;
}
```

Browse the **nginx vhost's URL**, e.g. `http://127.0.0.1:8088/contact.html`, so both
ordinary POST and fetch reach its CGI location. Opening stbl's port directly
bypasses CGI. After editing source, stop/rebuild/restart preview and refresh;
preview does not watch or rebuild automatically. No copying to another preview
web root is needed. Rebuild after temporary-directory cleanup before submitting.

## Test and troubleshoot deployment

A GET invokes the receiver without sending email:

```sh
curl -i https://cpp-freelancer.com/cgi-bin/contact -H 'Accept: application/json'
```

Expect HTTP **405** with JSON saying to use POST. Then submit the form normally
and with JavaScript disabled. A CLI POST test sends a real email:

```sh
curl -i https://cpp-freelancer.com/cgi-bin/contact \
  -H 'Accept: application/json' \
  --data-urlencode 'message=Contact form deployment test' \
  --data-urlencode 'email=' \
  --data-urlencode '_stbl_website='
```

Expect HTTP 200 with `"success":true`, meaning queued by the MTA, not final inbox
delivery. Use `sudo postqueue -p` and your Postfix logs to check delivery. For
HTTP 500 JSON, check CGI stderr in nginx/fcgiwrap logs for invalid config, a wrong
website_root, or missing manifest. HTTP 502/503 HTML from nginx usually means the
FastCGI socket or executable is unavailable. Check `journalctl -u fcgiwrap.service`
and nginx's error log. Keep `fastcgi_intercept_errors off` to preserve receiver
JSON errors. If fetch fails but ordinary POST works, check that the page and
endpoint share the same origin and that you're browsing nginx's URL.

## Receiver behavior

Only `application/x-www-form-urlencoded` POST is accepted. The request-size limit
is enforced before reading the body (default 32 KiB, configurable up to 1 MiB).
Missing or invalid lengths, short bodies, invalid UTF-8/percent encodings,
duplicate fields, unknown fields, invalid email, and missing required values are
rejected. The automatically rendered `_stbl_website` honeypot must be present and
empty. Optional fields may be omitted. Email accepts ordinary ASCII mailbox
addresses with a dotted domain; quoted local parts and internationalized
addresses are not supported. Whitespace is trimmed for validation and Reply-To.

The default rate is five attempts per IP, site, form, and minute, including failed
validation and delivery attempts (`requests_per_minute`, range 1..1000). A shared
cap of 1000 attempts per minute applies across all sites using the same `rate_dir`
(`global_requests_per_minute`, range 1..10000). These are fixed minute windows.
A private local directory holds a kernel lock and an atomically replaced JSON
state file, with at most one fixed pending file. There are no per-client files or
directory scans. State is capped at 1 MiB and expires when the minute changes.
Lock waits are bounded to one second; corrupt/unavailable state fails closed with
503. Repair corrupt state only while receivers are stopped; removing it resets
that minute's limits. No database is required.

On upgrade, legacy `stbl-rate-*` token files are ignored. Remove those files from
the private rate directory only after old receiver processes have been retired.
Existing private configuration uses the new defaults without edits. Rebuild both
binaries and the site, redeploy the output, and install the http-context nginx
snippet before enabling the new location limits. nginx may return its own HTML
429; the browser enhancement displays a rate-limit message and preserves input.

The receiver sends plain-text UTF-8 mail using `sendmail -i -f SENDER -- RECIPIENT`.
Recipients and the envelope sender come exclusively from server configuration;
the subject must be printable ASCII. Validated visitor email is used only for
`Reply-To`. Headers reject line breaks; user content appears in the mail body.
Mail body headings use field names with an initial capital and underscores or
hyphens replaced by spaces, independently of the labels shown on the page.
Textarea values start on the next line; other values follow the heading on the
same line, for example:

```text
Message:
Hello, I would like to discuss a project.

Email: visitor@example.org
```

`sendmail_timeout_seconds` defaults to 10 (range 1..30). The deadline covers both
writing to sendmail and waiting for its exit. On failure or timeout, an unfinished
submission's process group is killed and the direct child is reaped. A timeout
returns 503 and explicitly reports uncertain delivery: the MTA might already
have queued the message. Avoid immediate retries. Keep nginx's
`fastcgi_read_timeout` above the configured sendmail deadline.

The MTA handles retries and delivery. Validation/request processing call a
separate `Forwarder` interface, allowing a later NextApp adapter.

Requests with `Accept: application/json` receive `{ "success": true|false,
"message": "..." }`; ordinary requests receive an escaped HTML confirmation or
error page. Responses disable caching. Errors use HTTP 400 (validation), 404
(endpoint), 405 (method), 413 (size), 415 (media type), 429 (rate, with
`Retry-After: 60`), or 503 (rate storage/queue failure). Configuration errors return
500. Internal errors go to CGI stderr, not the visitor. On ordinary POST errors,
use the browser's Back button to return to the form. On AJAX errors, values stay
in place and submission can be retried. A timeout or lost connection can leave
delivery uncertain; avoid immediately retrying.

## Checks

```sh
cargo test
node scripts/test-forms.mjs
```

Tests cover definition parsing, shared validation, theme rendering, planned
artifacts, definition rebuilds, browser enhancement behavior, real CGI invocations
with HTML/JSON negotiation, and a sendmail substitute that records queued mail
and can simulate queue failure, blocked mail writes, and stalled mail processes.
Regression tests cover bounded/concurrent rate state, invalid storage, selected
site isolation, and complete request bodies with stdin held open. To check your deployed MTA, submit the example
through your CGI-enabled web server with and without JavaScript, then inspect
your MTA's queue/logs.
