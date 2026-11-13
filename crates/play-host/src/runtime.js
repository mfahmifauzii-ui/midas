// The Play runtime: applies the editor's commands to the page and reports user events.
// Handwritten on purpose (docs/FORK.md, the recorded deviation from Rust rule 4). Kept small: the
// behaviour is CSS (the state table) and the page only applies patches and reports events.
(function () {
  'use strict';
  // Attributes the editor's `set` may change (any other attribute is left alone).
  var MANAGED = ['class', 'href', 'target', 'rel', 'tabindex', 'type', 'spellcheck', 'value'];

  function post(msg) {
    try {
      window.ipc.postMessage(JSON.stringify(msg));
    } catch (e) { /* no host: nothing to tell */ }
  }

  function nodeOf(target) {
    var el = target && target.closest ? target.closest('[data-n]') : null;
    return el ? Number(el.getAttribute('data-n')) : null;
  }

  function send(name, node, value) {
    var m = { kind: 'event', node: node, event: name };
    if (value !== undefined) m.value = value;
    post(m);
  }

  function find(n) {
    return document.querySelector('[data-n="' + n + '"]');
  }

  window.fcLoad = function (html, css) {
    document.getElementById('fc-style').textContent = css;
    document.getElementById('fc-stage').innerHTML = html;
  };

  window.fcApply = function (cmds) {
    cmds.forEach(function (c) {
      if (c.op === 'set') {
        var el = find(c.n);
        if (!el) return;
        MANAGED.forEach(function (k) {
          if (!(k in c.attrs)) el.removeAttribute(k);
        });
        Object.keys(c.attrs).forEach(function (k) {
          if (k === 'value') {
            el.setAttribute('value', c.attrs[k]);
            if ('value' in el && el.value !== c.attrs[k]) el.value = c.attrs[k];
          } else {
            el.setAttribute(k, c.attrs[k]);
          }
        });
        el.setAttribute('style', c.style);
      } else if (c.op === 'replace') {
        var old = find(c.n);
        if (old) old.outerHTML = c.html;
      } else if (c.op === 'remove') {
        var gone = find(c.n);
        if (gone) gone.remove();
      }
    });
  };

  // Pointer and focus events, logged by the host as {node, event, value?}.
  document.addEventListener('click', function (e) {
    var n = nodeOf(e.target);
    if (n !== null) send('click', n);
  }, true);
  document.addEventListener('pointerdown', function (e) {
    var n = nodeOf(e.target);
    if (n !== null) send('down', n);
  }, true);
  document.addEventListener('pointerup', function (e) {
    var n = nodeOf(e.target);
    if (n !== null) send('up', n);
  }, true);
  document.addEventListener('mouseover', function (e) {
    var el = e.target && e.target.closest ? e.target.closest('[data-n]') : null;
    if (!el || (e.relatedTarget && el.contains(e.relatedTarget))) return;
    send('enter', Number(el.getAttribute('data-n')));
  }, true);
  document.addEventListener('mouseout', function (e) {
    var el = e.target && e.target.closest ? e.target.closest('[data-n]') : null;
    if (!el || (e.relatedTarget && el.contains(e.relatedTarget))) return;
    send('leave', Number(el.getAttribute('data-n')));
  }, true);
  document.addEventListener('focusin', function (e) {
    var n = nodeOf(e.target);
    if (n !== null) send('focus', n);
  }, true);
  document.addEventListener('focusout', function (e) {
    var n = nodeOf(e.target);
    if (n !== null) send('blur', n);
  }, true);
  document.addEventListener('input', function (e) {
    var n = nodeOf(e.target);
    if (n !== null) send('input', n, String(e.target.value));
  }, true);
  document.addEventListener('keydown', function (e) {
    if (e.key !== 'Enter' || e.target.tagName === 'TEXTAREA') return;
    var n = nodeOf(e.target);
    if (n !== null) send('submit', n, String(e.target.value === undefined ? '' : e.target.value));
  }, true);

  post({ kind: 'ready' });
})();
