/* Progressive enhancement: ordinary POST remains available without this script. */
(() => {
  if (!window.fetch || !window.URLSearchParams || !window.FormData || !window.AbortController) return;
  document.querySelectorAll('form.stbl-form').forEach(form => {
    let pending = false;
    let sent = false;
    const status = form.querySelector('.stbl-form-status');
    const button = form.querySelector('button[type="submit"]');
    form.addEventListener('submit', async event => {
      event.preventDefault();
      if (pending || sent) return;
      status.textContent = '';
      const fields = Array.from(form.querySelectorAll('input:not([name="_stbl_website"]), textarea'));
      fields.forEach(field => field.removeAttribute('aria-invalid'));
      for (const field of fields) {
        const value = field.value.trim();
        // Match the receiver's intentionally simple ASCII mailbox syntax.
        const email = /^[A-Za-z0-9.!#$%&'*+\/=?^_`{|}~-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)+$/;
        const local = value.split('@')[0];
        const domain = value.split('@')[1] || '';
        const invalidEmail = field.type === 'email' && value &&
          (!email.test(value) || value.length > 254 || local.length > 64 ||
           local.startsWith('.') || local.endsWith('.') || local.includes('..') ||
           domain.split('.').some(part => part.length > 63 || part.startsWith('-') || part.endsWith('-')));
        if ((field.required && !value) || invalidEmail) {
          const label = field.closest('label').querySelector('span').textContent.replace(/ \(required\)$/, '');
          status.textContent = invalidEmail ? `${label} must be a valid email address.` : `${label} is required.`;
          field.setAttribute('aria-invalid', 'true');
          field.focus();
          return;
        }
      }
      pending = true;
      button.disabled = true;
      form.setAttribute('aria-busy', 'true');
      status.textContent = 'Sending…';
      const controller = new AbortController();
      const timeout = setTimeout(() => controller.abort(), 30000);
      try {
        const response = await fetch(form.action, {
          method: 'POST',
          headers: { 'Accept': 'application/json' },
          body: new URLSearchParams(new FormData(form)),
          signal: controller.signal
        });
        if (response.status === 429) {
          throw new Error("Too many submissions. Please wait a minute and try again.");
        }
        const result = await response.json();
        if (!response.ok || result.success !== true) {
          throw new Error(result.message || `Submission failed (${response.status}). Please try again.`);
        }
        sent = true;
        status.textContent = result.message || 'Thank you. Your message has been queued.';
      } catch (error) {
        status.textContent = error.name === 'AbortError'
          ? 'The request timed out. Delivery is uncertain; please wait before trying again.'
          : (error instanceof TypeError || error instanceof SyntaxError)
            ? 'Unable to confirm delivery. Please check your connection and try again.'
            : error.message;
      } finally {
        clearTimeout(timeout);
        pending = false;
        button.disabled = sent;
        form.removeAttribute('aria-busy');
        status.focus();
      }
    });
  });
})();
