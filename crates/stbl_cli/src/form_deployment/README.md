# Deploy this website's forms

Copy the entire built website to the server, preserving executable permissions
(e.g. `rsync -a OUT/ /var/www/example/`). This directory contains the receiver
executable, forms.yaml validation manifest, and configuration examples. No source
site, Rust installation, temporary setup bundle, or wrapper script is needed on
the server. The executable must match the server's OS/architecture/libc; build on
a compatible Linux machine. Release builds produce smaller executables.

1. Install a local MTA providing /usr/sbin/sendmail, plus nginx and fcgiwrap.
   On Ubuntu/Debian: `sudo apt install postfix nginx fcgiwrap`, configure Postfix,
   then `sudo systemctl enable --now fcgiwrap.socket`.
2. Copy receiver.yaml.example OUTSIDE the web root to /etc/stbl/forms.yaml.
   Set website_root to the absolute deployed website directory, replace the site
   key `example`, and configure sender/recipient/subject for each form endpoint.
   Optionally set reply_to_field: email when that form has an email field.
   The receiver reads website_root/stbl-forms/forms.yaml for validation. Never
   edit that generated manifest; edit the source form and rebuild the website.
3. Create private rate storage: `sudo install -d -o www-data -g www-data -m 0700
   /var/lib/stbl-forms`. Keep the config root-owned, readable by www-data (0640,
   with a traversable parent). Keep site files owned by the deployment user and
   not writable by the CGI user. Preserve mode 0755 on stbl-form-receiver.
4. Install nginx-http.conf.example ONCE in nginx's http {} context (on Debian/
   Ubuntu copy it to /etc/nginx/conf.d/stbl-forms-rate-limit.conf). It defines
   bounded-memory per-IP and per-vhost request zones before CGI starts. Tune its
   limits for your traffic. Add the blocks from nginx.conf.example to the existing server {} serving this
   website. Keep the /stbl-forms/ deny block. Match each form's action in an exact
   CGI location; change STBL_FORM_CONFIG and STBL_FORM_SITE to your private config
   and site key. $document_root must match website_root. If your vhost uses alias
   or proxies preview, use an explicit SCRIPT_FILENAME path to the built receiver.
   Then run `sudo nginx -t` and `sudo systemctl reload nginx`.
5. Open the contact page on that vhost and submit. Test without JavaScript too.
   A GET to /cgi-bin/contact with Accept: application/json should return HTTP 405
   with JSON (and send no mail). Successful POST means the local MTA queued mail;
   inspect Postfix logs/queue to verify delivery. Do not auto-submit test messages.

Adding another form: rebuild/deploy the site and add its endpoint/mail policy to
private receiver config plus an exact nginx location. Endpoints not explicitly
configured on the server are rejected. Mail addresses never come from forms.yaml
or POST data. A manifest edit cannot change the server's recipients.

For preview, keep nginx's existing location / proxy to stbl preview, add the same
CGI/deny locations, set website_root to the preview output, and use that output's
absolute receiver path in SCRIPT_FILENAME. Browse the nginx port, not stbl's port.
After source edits, rebuild/restart preview; do not copy a second preview website.

nginx/FastCGI: https://nginx.org/en/docs/http/ngx_http_fastcgi_module.html
fcgiwrap: https://manpages.ubuntu.com/manpages/jammy/man8/fcgiwrap.8.html

Receiver limits: requests_per_minute defaults to 5 per IP/site/form (maximum
1000); global_requests_per_minute defaults to 1000 across all sites sharing
rate_dir (maximum 10000). State uses a locked, size-limited current-minute JSON
file; use a private local filesystem. Corrupt state fails closed. Legacy
stbl-rate-* token files are ignored; remove them only after retiring old receivers.
sendmail_timeout_seconds defaults to 10 (range 1..30), covering writes and waiting.
Timeout kills/reaps the mail process group; delivery may already have happened,
so do not immediately retry. Keep nginx's FastCGI timeout above this deadline.

Upgrade: rebuild both binaries and the website, then redeploy the output. Existing
private configs use the defaults above. Install the new http-context nginx
snippet before adding the location limits, then test/reload nginx. Only the
selected site's validation manifest is read during a CGI request.
