/**
 * TEMP site-wide password gate. To remove later:
 * - delete this file
 * - in src/bin/web.rs serve_index_async: remove the script injection for site-gate-temp.js
 * - in index.html: remove the #site-gate-overlay block (search site-gate-temp)
 * - remove SiteGate + middleware + SITE_GATE_PLAIN from web.rs
 */
(function () {
  var STORAGE_KEY = 'dart_site_gate_token';
  var HEADER = 'X-Dart-Site-Gate';

  var origFetch = window.fetch;
  window.fetch = function (input, init) {
    init = init || {};
    var headers =
      init.headers != null ? new Headers(init.headers) : new Headers();
    var t = sessionStorage.getItem(STORAGE_KEY);
    if (t) headers.set(HEADER, t);
    return origFetch(input, Object.assign({}, init, { headers: headers }));
  };

  function overlay() {
    return document.getElementById('site-gate-overlay');
  }
  function showErr(msg) {
    var e = document.getElementById('site-gate-error');
    if (!e) return;
    e.textContent = msg || '';
    e.classList.toggle('hidden', !msg);
  }

  async function tryUnlock() {
    var r = await fetch('/api/site-gate/check', { credentials: 'same-origin' });
    if (r.status === 204) {
      var o = overlay();
      if (o) {
        o.classList.add('hidden');
        o.setAttribute('aria-hidden', 'true');
      }
      return true;
    }
    var o = overlay();
    if (o) {
      o.classList.remove('hidden');
      o.setAttribute('aria-hidden', 'false');
    }
    return false;
  }

  async function submit() {
    var input = document.getElementById('site-gate-password');
    if (!input) return;
    showErr('');
    var r = await fetch('/api/site-gate', {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ password: input.value }),
    });
    if (r.status === 200) {
      var data = await r.json().catch(function () {
        return {};
      });
      if (data.token) {
        sessionStorage.setItem(STORAGE_KEY, data.token);
      }
      var o = overlay();
      if (o) {
        o.classList.add('hidden');
        o.setAttribute('aria-hidden', 'true');
      }
      window.dispatchEvent(new CustomEvent('dart-site-gate-ok'));
      return;
    }
    var err = await r.json().catch(function () {
      return {};
    });
    showErr(err.error || 'Wrong password');
  }

  function init() {
    tryUnlock().then(function (ok) {
      if (ok) return;
      var btn = document.getElementById('site-gate-submit');
      var input = document.getElementById('site-gate-password');
      if (btn) btn.addEventListener('click', submit);
      if (input) {
        input.addEventListener('keydown', function (e) {
          if (e.key === 'Enter') submit();
        });
      }
    });
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
