// Dependency-free behavioral checks for the progressive enhancement script.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
const source = fs.readFileSync(new URL('../crates/stbl_embedded_assets/assets/templates/stbl/js/forms.js', import.meta.url), 'utf8');
function setup(fetch) {
  const field = (name, type, required, value) => ({ name, type, required, value,
    attrs: {}, focused: false,
    setAttribute(key, value) { this.attrs[key] = value; },
    removeAttribute(key) { delete this.attrs[key]; },
    focus() { this.focused = true; },
    closest() { return { querySelector: () => ({ textContent: name }) }; }
  });
  const fields = [field('email', 'email', false, ''), field('message', 'textarea', true, 'Hello')];
  const status = { textContent: '', focus() {} };
  const button = { disabled: false };
  let submit;
  const form = { action: 'https://example.org/cgi-bin/contact',
    querySelector(selector) { return selector === '.stbl-form-status' ? status : button; },
    querySelectorAll() { return fields; },
    addEventListener(name, fn) { assert.equal(name, 'submit'); submit = fn; },
    setAttribute() {}, removeAttribute() {} };
  const context = { window: { fetch, URLSearchParams, FormData: class {}, AbortController },
    document: { querySelectorAll: () => [form] },
    fetch, URLSearchParams, FormData: class { constructor() { return [['email', fields[0].value], ['message', fields[1].value], ['_stbl_website', '']]; } },
    AbortController, setTimeout, clearTimeout, TypeError, SyntaxError };
  vm.runInNewContext(source, context);
  return { fields, status, button, submit: () => submit({ preventDefault() {} }) };
}
let calls = 0;
const ok = async (_url, options) => { calls++; assert.equal(options.method, 'POST'); assert.equal(options.headers.Accept, 'application/json'); assert.ok(options.body instanceof URLSearchParams); return { ok: true, json: async () => ({ success: true, message: 'Queued.' }) }; };
let fixture = setup(ok);
fixture.fields[1].value = ' ';
await fixture.submit();
assert.equal(calls, 0);
assert.match(fixture.status.textContent, /required/);
assert.equal(fixture.fields[1].attrs['aria-invalid'], 'true');
assert.equal(fixture.fields[1].focused, true);
fixture.fields[1].value = 'Hello';
for (const email of ['invalid', 'x@-example.org', 'x..y@example.org', 'x@example.org\nBcc: y@example.org']) {
  fixture.fields[0].value = email;
  await fixture.submit();
  assert.equal(calls, 0);
  assert.match(fixture.status.textContent, /valid email/);
}
fixture.fields[0].value = '';
await fixture.submit();
assert.equal(calls, 1);
assert.equal(fixture.status.textContent, 'Queued.');
assert.equal(fixture.button.disabled, true);
await fixture.submit();
assert.equal(calls, 1);
fixture = setup(ok);
fixture.fields[0].value = 'visitor+contact@example.org';
await fixture.submit();
assert.equal(calls, 2);
fixture = setup(async () => ({ ok: false, status: 400, json: async () => ({ success: false, message: 'Server validation failed.' }) }));
await fixture.submit();
assert.equal(fixture.status.textContent, 'Server validation failed.');
assert.equal(fixture.fields[1].value, 'Hello');
assert.equal(fixture.button.disabled, false);
let resolve;
fixture = setup(() => { calls++; return new Promise(done => { resolve = done; }); });
const pending = fixture.submit();
assert.equal(fixture.button.disabled, true);
await fixture.submit();
assert.equal(calls, 3);
resolve({ ok: true, json: async () => ({ success: true }) });
await pending;
for (const error of [new TypeError('network'), new SyntaxError('bad JSON')]) {
  fixture = setup(async () => { throw error; });
  await fixture.submit();
  assert.match(fixture.status.textContent, /Unable to confirm delivery/);
  assert.equal(fixture.fields[1].value, 'Hello');
  assert.equal(fixture.button.disabled, false);
}
fixture = setup(async () => ({ ok: false, status: 429,
  json: async () => { throw new Error('nginx HTML must not be parsed as JSON'); } }));
await fixture.submit();
assert.match(fixture.status.textContent, /Too many submissions/);
assert.equal(fixture.fields[1].value, 'Hello');
assert.equal(fixture.button.disabled, false);
console.log('Form client behavior checks passed.');
