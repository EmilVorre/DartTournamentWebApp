(function () {
  var ID_KEY = 'dart_tournament_id';
  var CODE_KEY = 'dart_tournament_edit_code';

  function pathOnly(url) {
    try {
      var u = typeof url === 'string' ? url : (url && url.url) || '';
      return u.split('?')[0];
    } catch (e) {
      return '';
    }
  }

  function shouldAttachCode(path, method) {
    var m = (method || 'GET').toUpperCase();
    if (m === 'GET' || m === 'HEAD') return false;
    if (!path.includes('/api/tournaments/')) {
      if (path === '/api/tournaments' && m === 'POST') return false;
      return false;
    }
    if (path.endsWith('/verify-edit-code')) return false;
    return true;
  }

  function updateUnlockBanner() {
    var b = document.getElementById('unlock-edit-banner');
    if (!b) return;
    var id = localStorage.getItem(ID_KEY);
    var code = localStorage.getItem(CODE_KEY);
    if (id && !code) b.classList.remove('hidden');
    else b.classList.add('hidden');
  }

  var origFetch = window.fetch;
  window.fetch = async function (input, init) {
    init = init || {};
    var method = (init.method || 'GET').toUpperCase();
    var path = pathOnly(typeof input === 'string' ? input : input && input.url);

    if (path === '/api/tournaments' && method === 'POST' && typeof init.body === 'string') {
      try {
        var obj = JSON.parse(init.body);
        var el = document.getElementById('tournament-edit-code-input');
        var v = el && el.value ? el.value.trim() : '';
        if (v.length) obj.edit_code = v;
        init.body = JSON.stringify(obj);
      } catch (e) {}
    }

    var headers = new Headers(init.headers || {});
    var code = localStorage.getItem(CODE_KEY);
    if (code && shouldAttachCode(path, method)) headers.set('X-Edit-Code', code);
    init.headers = headers;

    var res = await origFetch.call(this, input, init);

    if (path === '/api/tournaments' && method === 'POST' && res.ok) {
      try {
        var t = await res.clone().json();
        if (t && t.edit_code) {
          localStorage.setItem(CODE_KEY, t.edit_code);
          var elr = document.getElementById('organizer-code-reveal');
          if (elr) {
            elr.textContent =
              'Your organizer code: ' +
              t.edit_code +
              ' — save it; you need it to edit from another device or after clearing site data.';
            elr.classList.remove('hidden');
          }
        }
      } catch (e) {}
    }

    return res;
  };

  function wireUnlockModal() {
    var openBtn = document.getElementById('open-unlock-modal-btn');
    var modal = document.getElementById('unlock-organizer-modal');
    var inp = document.getElementById('unlock-organizer-input');
    var errEl = document.getElementById('unlock-organizer-error');
    var cancel = document.getElementById('unlock-organizer-cancel');
    var confirm = document.getElementById('unlock-organizer-confirm');
    if (!openBtn || !modal || !inp) return;

    function hideErr() {
      if (errEl) {
        errEl.classList.add('hidden');
        errEl.textContent = '';
      }
    }
    function showErr(msg) {
      if (errEl) {
        errEl.textContent = msg;
        errEl.classList.remove('hidden');
      }
    }

    openBtn.addEventListener('click', function () {
      hideErr();
      inp.value = '';
      modal.classList.remove('hidden');
    });
    if (cancel) {
      cancel.addEventListener('click', function () {
        modal.classList.add('hidden');
      });
    }
    if (confirm) {
      confirm.addEventListener('click', async function () {
        hideErr();
        var id = localStorage.getItem(ID_KEY);
        if (!id) {
          showErr('No tournament loaded');
          return;
        }
        var c = (inp.value || '').trim();
        if (c.length < 4) {
          showErr('Code must be at least 4 characters');
          return;
        }
        try {
          var r = await origFetch.call(window, '/api/tournaments/' + id + '/verify-edit-code', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ code: c }),
          });
          var data = await r.json().catch(function () {
            return {};
          });
          if (!r.ok) {
            showErr(data.error || 'Invalid code');
            return;
          }
          localStorage.setItem(CODE_KEY, c);
          modal.classList.add('hidden');
          updateUnlockBanner();
          if (typeof loadAndRender === 'function') await loadAndRender();
        } catch (e) {
          showErr((e && e.message) || 'Request failed');
        }
      });
    }
  }

  document.addEventListener('DOMContentLoaded', function () {
    wireUnlockModal();
    updateUnlockBanner();
    setInterval(updateUnlockBanner, 1500);
  });
})();
