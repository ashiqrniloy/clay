/* ============================================================================
   Quiet Instrument — shared interaction layer for the migration prototype set.

   This is the approved language's interaction layer
   (design-artifacts/approved/quiet-instrument-language/ds.js) with the
   migration-era review switches added: `?theme=`/`?width=`/`?scene=` overrides,
   the frame width toggle, and multi-state scenes. The behaviour it gives the
   screens is unchanged, so `workspace.html` and `agent-landing.html` are the
   approved screens with a review frame around them.

   It is loaded by every page in design-artifacts/prototypes/quiet-instrument-migration/
   and by the component specimen sheet; the geometry/materials come from
   components.css / ds-quiet.css, the color contract from theme.css.

   Responsibilities
     - theme selection (6 themes, persisted, mirrors Clay's appearance model:
       Modus Operandi on light OS, Modus Vivendi on dark)
     - density + reduced-motion preferences
     - overlays (palette, shortcuts, drawers) with focus restore
     - popovers (theme menu, model menu)
     - tabs with arrow-key navigation
     - keyboard tree (navigate / expand / filter)
     - generic row lists (skills, MCP, results) with roving tabindex
     - command palette engine (fuzzy filter, groups, hints)
     - toasts + live-region announcements so keyboard actions are visible

   Public surface: window.ClayDS
   ========================================================================= */
(function () {
  'use strict';

  var THEMES = [
    { id: 'modus-operandi', label: 'Modus Operandi', mode: 'light' },
    { id: 'modus-vivendi', label: 'Modus Vivendi', mode: 'dark' },
    { id: 'gruvbox-material-dark', label: 'Gruvbox Material Dark', mode: 'dark' },
    { id: 'gruvbox-material-light', label: 'Gruvbox Material Light', mode: 'light' }
  ];
  /* Kanagawa Wave and Catppuccin Latte were proposal-only: they are not shipped
     themes and have no palette in theme.css. Never reintroduce them here. */

  var STORE = 'clay-ds-prefs';
  var IS_MAC = /mac|iphone|ipad/i.test(navigator.platform || navigator.userAgent);
  var PLATFORM = {
    mod: IS_MAC ? '\u2318' : 'Ctrl',
    alt: IS_MAC ? '\u2325' : 'Alt',
    shift: IS_MAC ? '\u21e7' : 'Shift'
  };
  var MOD = PLATFORM.mod;
  var GLYPH = { enter: '\u21b5', esc: 'Esc', up: '\u2191', down: '\u2193', left: '\u2190', right: '\u2192', tab: 'Tab' };

  /// Format a shortcut spec (`mod+shift+z`) for the current platform.
  function key(spec) {
    var parts = spec.split('+').map(function (part) {
      if (PLATFORM[part]) return PLATFORM[part];
      if (GLYPH[part]) return GLYPH[part];
      return part.length === 1 ? part.toUpperCase() : part;
    });
    return parts.join(IS_MAC ? '' : '+');
  }

  function store() {
    try { return JSON.parse(localStorage.getItem(STORE)) || {}; } catch (e) { return {}; }
  }
  function save(patch) {
    var next = Object.assign(store(), patch);
    try { localStorage.setItem(STORE, JSON.stringify(next)); } catch (e) { /* private mode */ }
  }

  var prefs = store();
  // layout regions the user actively toggled in this page (only these are saved)
  var touched = {};
  if (!prefs.theme || !THEMES.some(function (t) { return t.id === prefs.theme; })) {
    var osDark = window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches;
    prefs.theme = osDark ? 'modus-vivendi' : 'modus-operandi';
  }
  if (!prefs.density) prefs.density = 'comfortable';
  if (!prefs.motion) prefs.motion = 'full';
  if (!prefs.frame) prefs.frame = 'wide';

  /* `?theme=`/`?width=`/`?scene=` put a page in a known state for a capture tool
     or a reviewer without touching stored preferences: a query theme is never
     written back, so reviewing Modus Vivendi does not change anyone's default. */
  var QUERY = new URLSearchParams(window.location.search);
  var forcedTheme = QUERY.get('theme');
  var forcedScene = QUERY.get('scene');
  if (forcedTheme && THEMES.some(function (t) { return t.id === forcedTheme; })) prefs.theme = forcedTheme;
  if (QUERY.get('width') === 'narrow' || QUERY.get('width') === 'wide') prefs.frame = QUERY.get('width');

  var root = document.documentElement;

  /* ---------- live region + toasts ------------------------------------- */

  var live = document.createElement('p');
  live.className = 'sr-only';
  live.setAttribute('role', 'status');
  live.setAttribute('aria-live', 'polite');

  var toastHost = null;
  function toasts() {
    if (!toastHost) {
      toastHost = document.querySelector('[data-toasts]');
      if (!toastHost) {
        toastHost = document.createElement('div');
        toastHost.dataset.toasts = '';
        toastHost.className = 'toast-host';
        document.body.appendChild(toastHost);
      }
    }
    return toastHost;
  }

  function toast(message, opts) {
    opts = opts || {};
    var el = document.createElement('div');
    el.className = 'toast';
    var text = document.createElement('span');
    text.className = 'toast-text';
    text.textContent = message;
    el.appendChild(text);
    if (opts.kbd) {
      var k = document.createElement('kbd');
      k.className = 'kbd';
      k.textContent = opts.kbd;
      el.appendChild(k);
    }
    if (opts.tone) el.dataset.tone = opts.tone;
    toasts().appendChild(el);
    requestAnimationFrame(function () { el.classList.add('is-in'); });
    setTimeout(function () {
      el.classList.remove('is-in');
      setTimeout(function () { el.remove(); }, 260);
    }, opts.ms || 2200);
    return el;
  }

  function announce(message) {
    if (!live.isConnected) document.body.appendChild(live);
    live.textContent = message;
  }

  /* ---------- theme / density / motion --------------------------------- */

  function themeById(id) {
    return THEMES.filter(function (t) { return t.id === id; })[0] || THEMES[0];
  }

  /* Prototype links carry the review state: a reviewer who switches palette and
     then follows the tab's view switcher must not land back on the default one.
     Sibling-page hrefs get the current theme and width; nothing else changes. */
  function syncLinks() {
    document.querySelectorAll('a[href$=".html"]').forEach(function (a) {
      var url = new URL(a.getAttribute('href'), window.location.href);
      url.searchParams.set('theme', prefs.theme);
      url.searchParams.set('width', prefs.frame);
      a.setAttribute('href', url.pathname.split('/').pop() + url.search);
    });
  }

  function applyTheme(id, opts) {
    opts = opts || {};
    var theme = themeById(id);
    root.dataset.theme = theme.id;
    prefs.theme = theme.id;
    if (!opts.forced) save({ theme: theme.id });

    document.querySelectorAll('[data-set-theme]').forEach(function (btn) {
      var on = btn.dataset.setTheme === theme.id;
      btn.setAttribute('aria-checked', on ? 'true' : 'false');
      btn.dataset.active = on ? 'true' : 'false';
    });
    document.querySelectorAll('[data-theme-label]').forEach(function (el) {
      el.textContent = theme.label;
    });
    document.querySelectorAll('[data-theme-mode]').forEach(function (el) {
      el.textContent = theme.mode;
    });
    var themeColor = getComputedStyle(document.body).backgroundColor;
    document.querySelectorAll('meta[name="theme-color"]').forEach(function (m) {
      m.setAttribute('content', themeColor);
    });
    if (!opts.silent) {
      announce('Theme ' + theme.label);
      toast(theme.label, { kbd: 'Alt ' + (THEMES.indexOf(theme) + 1), tone: 'quiet' });
    }
    syncLinks();
    document.dispatchEvent(new CustomEvent('clay:theme', { detail: { theme: theme } }));
  }

  function cycleTheme(step) {
    var i = THEMES.findIndex(function (t) { return t.id === prefs.theme; });
    applyTheme(THEMES[(i + step + THEMES.length) % THEMES.length].id);
  }

  function applyPrefs() {
    applyTheme(prefs.theme, { silent: true, forced: !!forcedTheme });
    root.dataset.density = prefs.density;
    root.dataset.motion = prefs.motion;
    document.querySelectorAll('[data-set-density]').forEach(function (btn) {
      btn.dataset.active = btn.dataset.setDensity === prefs.density ? 'true' : 'false';
      btn.setAttribute('aria-checked', btn.dataset.active === 'true' ? 'true' : 'false');
    });
    document.querySelectorAll('[data-set-motion]').forEach(function (btn) {
      btn.dataset.active = btn.dataset.setMotion === prefs.motion ? 'true' : 'false';
      btn.setAttribute('aria-checked', btn.dataset.active === 'true' ? 'true' : 'false');
    });
  }

  function setDensity(value) {
    prefs.density = value;
    save({ density: value });
    applyPrefs();
    announce('Density ' + value);
  }

  function setMotion(value) {
    prefs.motion = value;
    save({ motion: value });
    applyPrefs();
    announce('Motion ' + value);
  }

  /* ---------- review switches (frame width, surface state) -------------- */

  function setFrame(value) {
    prefs.frame = value === 'narrow' ? 'narrow' : 'wide';
    root.dataset.frame = prefs.frame;
    document.querySelectorAll('[data-set-width]').forEach(function (btn) {
      var on = btn.dataset.setWidth === prefs.frame;
      btn.dataset.active = on ? 'true' : 'false';
      btn.setAttribute('aria-checked', on ? 'true' : 'false');
    });
    syncLinks();
    announce('Frame ' + prefs.frame);
  }

  /* A page may ship several states of one surface (the agent lands, runs, fails,
     resumes; an overlay sits on a full or a reduced veil). `data-scene` names
     the ones on show. This is a review device: it never persists. */
  function setScene(name, opts) {
    // the default state is a scene like any other: it may have no block of its
    // own (the page's own markup is it), so the switcher's buttons count too
    var known = Array.prototype.map.call(document.querySelectorAll('[data-scene], [data-set-scene]'), function (el) {
      return el.dataset.scene || el.dataset.setScene;
    });
    if (known.indexOf(name) === -1) return false;
    root.dataset.scene = name;
    document.querySelectorAll('[data-scene]').forEach(function (el) { el.hidden = el.dataset.scene !== name; });
    document.querySelectorAll('[data-set-scene]').forEach(function (btn) {
      var on = btn.dataset.setScene === name;
      btn.dataset.active = on ? 'true' : 'false';
      btn.setAttribute('aria-checked', on ? 'true' : 'false');
    });
    if (!opts || !opts.silent) announce('State ' + name);
    // a page whose states are real interaction (the launcher picks rows) listens
    // for this instead of shipping duplicate markup per state
    document.dispatchEvent(new CustomEvent('clay:scene', { detail: { name: name } }));
    return true;
  }

  /* ---------- overlays -------------------------------------------------- */

  var overlayStack = [];

  function overlayOf(name) { return document.querySelector('[data-overlay="' + name + '"]'); }

  function openOverlay(name, opts) {
    var el = overlayOf(name);
    if (!el) return;
    opts = opts || {};
    el.dataset.open = 'true';
    if (el.dataset.modal !== 'false') el.setAttribute('aria-modal', 'true');
    var restore = opts.trigger || document.activeElement;
    overlayStack.push({ name: name, el: el, restore: restore });
    var focusTarget = opts.focus && el.querySelector(opts.focus);
    if (!focusTarget) focusTarget = el.querySelector('[autofocus]') || focusTarget;
    if (!focusTarget) focusTarget = el.querySelector('input, button, [tabindex]:not([tabindex="-1"])');
    if (focusTarget) focusTarget.focus();
    document.dispatchEvent(new CustomEvent('clay:overlay', { detail: { name: name, open: true } }));
  }

  function closeOverlay(name) {
    var el = name ? overlayOf(name) : (overlayStack[overlayStack.length - 1] || {}).el;
    if (!el) return;
    el.dataset.open = 'false';
    el.removeAttribute('aria-modal');
    for (var i = overlayStack.length - 1; i >= 0; i--) {
      if (overlayStack[i].el === el) {
        var entry = overlayStack.splice(i, 1)[0];
        if (entry.restore && entry.restore.isConnected) entry.restore.focus();
        break;
      }
    }
    document.dispatchEvent(new CustomEvent('clay:overlay', { detail: { name: el.dataset.overlay, open: false } }));
  }

  function closeTopOverlay() {
    if (!overlayStack.length) return false;
    closeOverlay(overlayStack[overlayStack.length - 1].name);
    return true;
  }

  function trapTab(event) {
    var top = overlayStack[overlayStack.length - 1];
    if (!top || event.key !== 'Tab') return;
    var nodes = top.el.querySelectorAll('a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])');
    var list = Array.prototype.filter.call(nodes, visible);
    if (!list.length) return;
    var first = list[0], last = list[list.length - 1];
    if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
    else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
  }

  /* ---------- popovers -------------------------------------------------- */

  function closePopovers(except) {
    document.querySelectorAll('[data-popover][data-open="true"]').forEach(function (p) {
      if (p !== except) {
        p.dataset.open = 'false';
        var trigger = document.querySelector('[data-popover-toggle="' + p.dataset.popover + '"]');
        if (trigger) trigger.setAttribute('aria-expanded', 'false');
      }
    });
  }

  function togglePopover(name, trigger) {
    var panel = document.querySelector('[data-popover="' + name + '"]');
    if (!panel) return;
    var open = panel.dataset.open !== 'true';
    closePopovers(panel);
    panel.dataset.open = open ? 'true' : 'false';
    if (trigger) trigger.setAttribute('aria-expanded', open ? 'true' : 'false');
    if (open) {
      var first = panel.querySelector('[data-autofocus], [role="menuitem"], button, input');
      if (first && trigger && trigger.dataset.focusFirst === 'true') first.focus();
    }
  }

  /* ---------- tabs ------------------------------------------------------ */

  function initTabs(scope) {
    scope.querySelectorAll('[data-tabs]').forEach(function (group) {
      if (group.dataset.tabsReady === 'true') return;
      group.dataset.tabsReady = 'true';
      var tabs = Array.prototype.slice.call(group.querySelectorAll('[role="tab"]'));
      var panels = Array.prototype.slice.call(group.querySelectorAll('[role="tabpanel"]'));
      var key = group.dataset.tabKey;

      function select(index, focus) {
        tabs.forEach(function (tab, i) {
          var on = i === index;
          tab.setAttribute('aria-selected', on ? 'true' : 'false');
          tab.tabIndex = on ? 0 : -1;
          if (on && focus) tab.focus();
        });
        panels.forEach(function (panel, i) {
          panel.hidden = i !== index;
        });
        if (key) {
          var saved = store();
          saved.tabs = saved.tabs || {};
          saved.tabs[key] = index;
          save({ tabs: saved.tabs });
        }
        var active = panels[index];
        if (active) document.dispatchEvent(new CustomEvent('clay:tab', { detail: { panel: active } }));
      }

      tabs.forEach(function (tab, i) {
        tab.addEventListener('click', function () { select(i, false); });
        tab.addEventListener('keydown', function (event) {
          var last = tabs.length - 1;
          if (event.key === 'ArrowRight') { event.preventDefault(); select((i + 1) % tabs.length, true); }
          else if (event.key === 'ArrowLeft') { event.preventDefault(); select((i - 1 + tabs.length) % tabs.length, true); }
          else if (event.key === 'Home') { event.preventDefault(); select(0, true); }
          else if (event.key === 'End') { event.preventDefault(); select(last, true); }
        });
      });

      // precedence: persisted choice -> declared default -> first tab
      var initial = Number(group.dataset.tabDefault || 0) || 0;
      var saved = store();
      if (key && saved.tabs && typeof saved.tabs[key] === 'number' && saved.tabs[key] < tabs.length) initial = saved.tabs[key];
      select(initial, false);
    });
  }

  /* ---------- tree ------------------------------------------------------ */

  function initTree(tree) {
    if (tree.dataset.treeReady === 'true') return;
    tree.dataset.treeReady = 'true';
    var collapsed = new Set();
    var rows = function () {
      return Array.prototype.filter.call(tree.querySelectorAll('[data-tree-item]'), function (row) {
        return !row.hidden && visible(row);
      });
    };

    function rowFor(id) { return tree.querySelector('[data-tree-item][data-id="' + id + '"]'); }

    function isDir(row) { return row.dataset.type === 'dir'; }

    function expand(row) {
      if (!isDir(row)) return false;
      collapsed.delete(row.dataset.id);
      return applyCollapse(row);
    }

    function collapse(row) {
      if (!isDir(row)) return false;
      collapsed.add(row.dataset.id);
      return applyCollapse(row);
    }

    function applyCollapse(dirRow) {
      var id = dirRow.dataset.id;
      var kids = [];
      var cursor = dirRow.nextElementSibling;
      while (cursor && Number(cursor.dataset.depth) > Number(dirRow.dataset.depth)) {
        kids.push(cursor);
        cursor = cursor.nextElementSibling;
      }
      var hide = collapsed.has(id);
      kids.forEach(function (kid) {
        var parentCollapsed = false;
        var p = kid.dataset.parent;
        while (p) {
          if (collapsed.has(p)) { parentCollapsed = true; break; }
          var pr = rowFor(p);
          p = pr ? pr.dataset.parent : null;
        }
        kid.hidden = hide || parentCollapsed;
      });
      var caret = dirRow.querySelector('[data-caret]');
      if (caret) caret.dataset.open = hide ? 'false' : 'true';
      dirRow.setAttribute('aria-expanded', hide ? 'false' : 'true');
      return true;
    }

    tree.querySelectorAll('[data-tree-item][data-type="dir"]').forEach(function (row) {
      row.setAttribute('aria-expanded', 'true');
    });

    function focusRow(row, opts) {
      if (!row) return;
      tree.querySelectorAll('[data-tree-item]').forEach(function (r) { r.tabIndex = -1; });
      row.tabIndex = 0;
      row.focus();
      row.scrollIntoView({ block: 'nearest' });
      if (opts && opts.flash) flash(row);
    }

    tree.addEventListener('click', function (event) {
      var row = event.target.closest('[data-tree-item], [data-file]');
      if (!row) return;
      var id = row.dataset.id;
      if (event.target.closest('[data-caret]') && isDir(row)) {
        if (collapsed.has(id)) expand(row); else collapse(row);
        focusRow(row);
        return;
      }
      if (isDir(row)) {
        if (collapsed.has(id)) { expand(row); } else { collapse(row); }
        focusRow(row);
        announce(themeLabel(row) + (collapsed.has(id) ? ' collapsed' : ' expanded'));
      } else {
        focusRow(row);
        openFileRow(row);
      }
    });

    function themeLabel(row) {
      var name = row.querySelector('.tree-name');
      return name ? name.textContent.trim() : row.dataset.id;
    }

    function openFileRow(row) {
      document.dispatchEvent(new CustomEvent('clay:file', { detail: { id: row.dataset.id, row: row } }));
    }

    tree.addEventListener('keydown', function (event) {
      var row = event.target.closest('[data-tree-item]');
      if (!row) return;
      var list = rows();
      var index = list.indexOf(row);
      if (event.key === 'ArrowDown') { event.preventDefault(); focusRow(list[Math.min(index + 1, list.length - 1)]); }
      else if (event.key === 'ArrowUp') { event.preventDefault(); focusRow(list[Math.max(index - 1, 0)]); }
      else if (event.key === 'Home') { event.preventDefault(); focusRow(list[0]); }
      else if (event.key === 'End') { event.preventDefault(); focusRow(list[list.length - 1]); }
      else if (event.key === 'ArrowRight') {
        if (isDir(row) && collapsed.has(row.dataset.id)) { event.preventDefault(); expand(row); announce(themeLabel(row) + ' expanded'); }
      } else if (event.key === 'ArrowLeft') {
        if (isDir(row) && !collapsed.has(row.dataset.id)) { event.preventDefault(); collapse(row); }
        else if (row.dataset.parent) {
          event.preventDefault();
          var parent = rowFor(row.dataset.parent);
          if (parent) focusRow(parent);
        }
      } else if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        if (isDir(row)) {
          if (collapsed.has(row.dataset.id)) expand(row); else collapse(row);
        } else {
          openFileRow(row);
        }
      }
    });

    document.querySelectorAll('[data-filter-for="' + tree.dataset.tree + '"]').forEach(function (input) {
      input.addEventListener('input', function () {
        var query = input.value.trim().toLowerCase();
        var matches = [];
        tree.querySelectorAll('[data-tree-item]').forEach(function (row) {
          if (!query) {
            row.hidden = false;
            if (row.dataset.type === 'dir') applyCollapse(row);
            return;
          }
          var haystack = (row.dataset.id || '') + ' ' + (row.dataset.keywords || '');
          row.hidden = haystack.toLowerCase().indexOf(query) === -1;
        });
        if (query) {
          // a matching file keeps its ancestors visible, so the row never
          // appears detached from the folder it lives in
          tree.querySelectorAll('[data-tree-item]:not([hidden])').forEach(function (row) {
            if (row.dataset.type === 'dir') return;
            matches.push(row);
            var parent = row.dataset.parent;
            while (parent) {
              var parentRow = rowFor(parent);
              if (!parentRow) break;
              parentRow.hidden = false;
              parent = parentRow.dataset.parent;
            }
          });
        }
        var count = (tree.closest('.side') || tree).querySelector('[data-filter-count]')
          || document.querySelector('[data-filter-count]');
        if (count) {
          count.textContent = query ? matches.length + (matches.length === 1 ? ' match' : ' matches') : '';
        }
        document.dispatchEvent(new CustomEvent('clay:filter', { detail: { query: query, matches: matches.length } }));
      });
      input.addEventListener('keydown', function (event) {
        if (event.key === 'Escape') {
          event.preventDefault();
          input.value = '';
          input.dispatchEvent(new Event('input'));
          announce('Filter cleared');
        } else if (event.key === 'Enter') {
          var first = rows().filter(function (r) { return r.dataset.type !== 'dir'; })[0];
          if (first) { event.preventDefault(); focusRow(first); openFileRow(first); }
        } else if (event.key === 'ArrowDown') {
          event.preventDefault();
          var firstRow = rows()[0];
          if (firstRow) focusRow(firstRow);
        }
      });
    });
  }

  /* ---------- generic list filter --------------------------------------- */

  function initListFilter(input) {
    var target = document.querySelector(input.dataset.filterList);
    if (!target) return;
    var rows = Array.prototype.slice.call(target.querySelectorAll('[data-row]'));
    var count = input.closest('[data-panel]')
      ? input.closest('[data-panel]').querySelector('[data-filter-count]')
      : document.querySelector('[data-filter-count]');
    input.addEventListener('input', function () {
      var query = input.value.trim().toLowerCase();
      var hits = 0;
      rows.forEach(function (row) {
        var hit = !query || row.textContent.toLowerCase().indexOf(query) !== -1;
        row.hidden = !hit;
        if (hit) hits++;
      });
      if (count) count.textContent = query ? hits + ' of ' + rows.length : String(rows.length);
      var empty = target.querySelector('[data-filter-empty]');
      if (empty) empty.hidden = hits !== 0;
      document.dispatchEvent(new CustomEvent('clay:filter', { detail: { query: query, matches: hits } }));
    });
    input.addEventListener('keydown', function (event) {
      if (event.key === 'Escape') {
        event.preventDefault();
        input.value = '';
        input.dispatchEvent(new Event('input'));
      }
    });
  }

  /* ---------- inspector ------------------------------------------------- */

  function toggleInspector(force) {
    var app = document.querySelector('[data-app]');
    if (!app) return;
    var next = typeof force === 'boolean' ? force : app.dataset.inspector !== 'collapsed';
    app.dataset.inspector = next ? 'collapsed' : 'expanded';
    touched.inspector = app.dataset.inspector;
    var trigger = document.querySelector('[data-toggle="inspector"]');
    if (trigger) trigger.setAttribute('aria-expanded', next ? 'false' : 'true');
    var label = document.querySelector('[data-inspector-toggle-label]');
    if (label) label.textContent = next ? 'Show inspector' : 'Hide inspector';
    announce(next ? 'Inspector hidden' : 'Inspector shown');
  }

  /* ---------- generic row lists ----------------------------------------- */

  function initRows(list) {
    if (list.dataset.rowsReady === 'true') return;
    list.dataset.rowsReady = 'true';
    var focusables = function () {
      return Array.prototype.filter.call(list.querySelectorAll('[data-row]'), visible);
    };
    list.addEventListener('keydown', function (event) {
      var row = event.target.closest('[data-row]');
      if (!row) return;
      var items = focusables();
      var i = items.indexOf(row);
      if (event.key === 'ArrowDown') { event.preventDefault(); (items[i + 1] || row).focus(); }
      else if (event.key === 'ArrowUp') { event.preventDefault(); (items[i - 1] || row).focus(); }
      else if (event.key === 'Home') { event.preventDefault(); items[0].focus(); }
      else if (event.key === 'End') { event.preventDefault(); items[items.length - 1].focus(); }
      else if (event.key === 'Escape') { event.preventDefault(); row.blur(); }
    });
  }

  /* ---------- command palette ------------------------------------------- */

  var palette = {
    commands: [],
    filtered: [],
    index: 0,
    filter: 'all',
    recent: (store().recent || [])
  };

  function setCommands(commands) {
    palette.commands = commands || [];
  }

  function score(query, target) {
    if (!query) return 1;
    var q = query.toLowerCase(), t = target.toLowerCase();
    if (t.indexOf(q) === 0) return 1000 - t.length;
    if (t.indexOf(q) > 0) return 500 - t.indexOf(q);
    var qi = 0, streak = 0, points = 0;
    for (var i = 0; i < t.length && qi < q.length; i++) {
      if (t[i] === q[qi]) { qi++; streak++; points += streak; } else { streak = 0; }
    }
    return qi === q.length ? points : -1;
  }

  function paletteCommands() {
    var query = palette.query || '';
    var pool = palette.commands.filter(function (cmd) {
      if (palette.filter !== 'all' && cmd.group !== palette.filter) return false;
      return true;
    });
    var scored = [];
    pool.forEach(function (cmd) {
      var target = cmd.title + ' ' + (cmd.keywords || '') + ' ' + (cmd.group || '') + ' ' + (cmd.meta || '');
      var s = score(query, target);
      if (s < 0) return;
      var recency = palette.recent.indexOf(cmd.id);
      if (!query && recency !== -1) s += 400 - recency * 10;
      scored.push({ cmd: cmd, s: s });
    });
    scored.sort(function (a, b) { return b.s - a.s; });
    var seen = {};
    return scored.map(function (row) { return row.cmd; }).filter(function (cmd) {
      if (seen[cmd.id]) return false;
      seen[cmd.id] = true;
      return true;
    });
  }

  function renderPalette() {
    var list = document.querySelector('[data-pal-list]');
    var empty = document.querySelector('[data-pal-empty]');
    var count = document.querySelector('[data-pal-count]');
    if (!list) return;
    palette.filtered = paletteCommands();
    palette.index = Math.max(0, Math.min(palette.index, palette.filtered.length - 1));
    if (count) count.textContent = palette.filtered.length + (palette.filtered.length === 1 ? ' result' : ' results');
    if (empty) empty.hidden = palette.filtered.length !== 0;
    list.innerHTML = '';
    var group = null;
    palette.filtered.slice(0, 80).forEach(function (cmd, i) {
      if (cmd.group !== group) {
        group = cmd.group;
        var head = document.createElement('div');
        head.className = 'pal-group';
        head.textContent = group || 'Commands';
        list.appendChild(head);
      }
      var item = document.createElement('button');
      item.type = 'button';
      item.className = 'pal-item';
      item.dataset.index = String(i);
      item.setAttribute('role', 'option');
      item.setAttribute('aria-selected', i === palette.index ? 'true' : 'false');
      if (i === palette.index) item.dataset.active = 'true';
      var title = document.createElement('span');
      title.className = 'pal-title';
      title.textContent = cmd.title;
      item.appendChild(title);
      if (cmd.meta) {
        var meta = document.createElement('span');
        meta.className = 'pal-meta';
        meta.textContent = cmd.meta;
        item.appendChild(meta);
      }
      if (cmd.hint) {
        var hint = document.createElement('kbd');
        hint.className = 'kbd';
        hint.textContent = cmd.hint;
        item.appendChild(hint);
      }
      item.addEventListener('mousemove', function () {
        if (palette.index === i) return;
        palette.index = i;
        renderPaletteActive();
      });
      item.addEventListener('click', function () { runCommand(cmd); });
      list.appendChild(item);
    });
  }

  function renderPaletteActive() {
    var list = document.querySelector('[data-pal-list]');
    if (!list) return;
    list.querySelectorAll('.pal-item').forEach(function (item) {
      var on = Number(item.dataset.index) === palette.index;
      item.dataset.active = on ? 'true' : 'false';
      item.setAttribute('aria-selected', on ? 'true' : 'false');
      if (on) item.scrollIntoView({ block: 'nearest' });
    });
  }

  function runCommand(cmd) {
    if (!cmd) return;
    palette.recent = [cmd.id].concat(palette.recent.filter(function (id) { return id !== cmd.id; })).slice(0, 6);
    save({ recent: palette.recent });
    closeOverlay('palette');
    announce(cmd.title);
    if (typeof cmd.run === 'function') cmd.run();
  }

  function openPalette(opts) {
    opts = opts || {};
    palette.index = 0;
    palette.filter = opts.filter || 'all';
    var input = document.querySelector('[data-pal-input]');
    if (input) input.value = opts.query || '';
    palette.query = opts.query || '';
    var label = document.querySelector('[data-pal-scope]');
    if (label) {
      label.textContent = opts.scopeLabel || (palette.filter === 'all' ? 'All commands' : palette.filter);
      label.dataset.scope = palette.filter;
    }
    renderPalette();
    openOverlay('palette', { focus: '[data-pal-input]', trigger: opts.trigger });
  }

  document.addEventListener('input', function (event) {
    if (event.target.matches('[data-pal-input]')) {
      palette.query = event.target.value.trim();
      palette.index = 0;
      renderPalette();
    }
  });

  document.addEventListener('keydown', function (event) {
    if (!event.target.matches('[data-pal-input]')) return;
    var last = palette.filtered.length - 1;
    if (event.key === 'ArrowDown') { event.preventDefault(); palette.index = Math.min(palette.index + 1, last); renderPaletteActive(); }
    else if (event.key === 'ArrowUp') { event.preventDefault(); palette.index = Math.max(palette.index - 1, 0); renderPaletteActive(); }
    else if (event.key === 'PageDown') { event.preventDefault(); palette.index = Math.min(palette.index + 8, last); renderPaletteActive(); }
    else if (event.key === 'PageUp') { event.preventDefault(); palette.index = Math.max(palette.index - 8, 0); renderPaletteActive(); }
    else if (event.key === 'Home') { event.preventDefault(); palette.index = 0; renderPaletteActive(); }
    else if (event.key === 'End') { event.preventDefault(); palette.index = last; renderPaletteActive(); }
    else if (event.key === 'Enter') { event.preventDefault(); runCommand(palette.filtered[palette.index]); }
  });

  document.addEventListener('click', function (event) {
    var scope = event.target.closest('[data-pal-scope-btn]');
    if (scope) {
      palette.filter = scope.dataset.palScopeBtn;
      palette.index = 0;
      document.querySelectorAll('[data-pal-scope-btn]').forEach(function (btn) {
        btn.dataset.active = btn === scope ? 'true' : 'false';
      });
      var label = document.querySelector('[data-pal-scope]');
      if (label) label.textContent = scope.textContent.trim();
      renderPalette();
      var input = document.querySelector('[data-pal-input]');
      if (input) input.focus();
      return;
    }
    var themeBtn = event.target.closest('[data-set-theme]');
    if (themeBtn) {
      applyTheme(themeBtn.dataset.setTheme);
      return;
    }
    var popTrigger = event.target.closest('[data-popover-toggle]');
    if (popTrigger) {
      event.preventDefault();
      togglePopover(popTrigger.dataset.popoverToggle, popTrigger);
      return;
    }
    var opener = event.target.closest('[data-open]');
    if (opener) {
      event.preventDefault();
      openOverlay(opener.dataset.open, { trigger: opener });
      return;
    }
    var closer = event.target.closest('[data-close]');
    if (closer) {
      event.preventDefault();
      closeOverlay(closer.dataset.close);
      return;
    }
    if (event.target.matches('[data-overlay]')) { closeOverlay(event.target.dataset.overlay); return; }
    if (!event.target.closest('[data-popover]')) closePopovers();
  });

  /* ---------- global keys ----------------------------------------------- */

  function isTyping(el) {
    if (!el) return false;
    return el.isContentEditable || /^(input|textarea|select)$/i.test(el.tagName);
  }

  document.addEventListener('keydown', function (event) {
    if (event.key === 'Tab') { trapTab(event); return; }

    var mod = event.metaKey || event.ctrlKey;

    if (event.key === 'Escape') {
      if (closeTopOverlay()) { event.preventDefault(); return; }
      if (document.querySelector('[data-popover][data-open="true"]')) { closePopovers(); event.preventDefault(); return; }
      document.dispatchEvent(new CustomEvent('clay:escape'));
      return;
    }

    if (mod && event.key.toLowerCase() === 'k') {
      event.preventDefault();
      openPalette({ trigger: document.activeElement });
      return;
    }

    if (event.altKey && !mod && !event.shiftKey) {
      var digit = parseInt(event.key, 10);
      if (digit >= 1 && digit <= THEMES.length) { event.preventDefault(); applyTheme(THEMES[digit - 1].id); return; }
      if (event.key === '`' || event.key === '~') { event.preventDefault(); cycleTheme(event.shiftKey ? -1 : 1); return; }
      if (event.key.toLowerCase() === 't') {
        event.preventDefault();
        var btn = document.querySelector('[data-theme-trigger]');
        if (btn) togglePopover('theme', btn);
        return;
      }
    }

    document.querySelectorAll('[data-shortcut]').forEach(function (node) {
      var spec = node.dataset.shortcut.split('|');
      spec.forEach(function (one) {
        var parts = one.split('+');
        var key = parts.pop();
        var needMod = parts.indexOf('mod') !== -1;
        var needShift = parts.indexOf('shift') !== -1;
        var needAlt = parts.indexOf('alt') !== -1;
        var keyOk = key === event.key.toLowerCase() || (key === '/' && event.key === '/') || (key === '?' && event.key === '?');
        if (!keyOk) return;
        if (needMod !== mod || needShift !== !!event.shiftKey || needAlt !== !!event.altKey) return;
        var allowTyping = node.dataset.shortcutTyping === 'true';
        if (isTyping(document.activeElement) && !allowTyping && !needMod) return;
        event.preventDefault();
        node.click();
      });
    });
  });

  /* ---------- focus helpers -------------------------------------------- */

  function visible(el) { return el.getClientRects().length > 0; }

  function flash(node) {
    if (!node) return;
    node.classList.remove('is-flash');
    void node.offsetWidth;
    node.classList.add('is-flash');
    setTimeout(function () { node.classList.remove('is-flash'); }, 620);
  }

  function focusZone(target) {
    document.querySelectorAll('[data-focus-zone]').forEach(function (zone) {
      zone.dataset.focused = zone.matches(target) ? 'true' : 'false';
    });
    var node = document.querySelector(target);
    if (node) {
      var focusable = node.querySelector('[data-zone-focus]') || node;
      if (focusable && focusable.focus) focusable.focus();
    }
  }

  function toggleSidebar(force) {
    var app = document.querySelector('[data-app]');
    if (!app) return;
    var next = typeof force === 'boolean' ? force : app.dataset.sidebar !== 'collapsed';
    app.dataset.sidebar = next ? 'collapsed' : 'expanded';
    touched.sidebar = app.dataset.sidebar;
    var trigger = document.querySelector('[data-toggle="sidebar"]');
    if (trigger) trigger.setAttribute('aria-expanded', next ? 'false' : 'true');
    var after = document.querySelector('[data-sidebar-toggle-label]');
    if (after) after.textContent = next ? 'Show sidebar' : 'Hide sidebar';
    announce(next ? 'Sidebar hidden' : 'Sidebar shown');
  }

  /* ---------- boot ------------------------------------------------------ */

  function init() {
    applyPrefs();
    setFrame(prefs.frame);
    if (forcedScene) setScene(forcedScene, { silent: true });
    /* the strip marks the page being reviewed, so a screenshot says where it is */
    var here = window.location.pathname.split('/').pop() || 'index.html';
    document.querySelectorAll('[data-nav] a').forEach(function (link) {
      if (link.getAttribute('href') === here) link.setAttribute('aria-current', 'page');
    });
    initTabs(document); // initTabs(scope) finds every [data-tabs] group inside scope
    document.querySelectorAll('[data-tree]').forEach(initTree);
    document.querySelectorAll('[data-rows]').forEach(initRows);
    document.querySelectorAll('[data-filter-list]').forEach(initListFilter);

    var app = document.querySelector('[data-app]');
    if (app && store().sidebar === 'collapsed') app.dataset.sidebar = 'collapsed';

    document.querySelectorAll('[data-toggle="sidebar"]').forEach(function (btn) {
      btn.addEventListener('click', function () { toggleSidebar(); });
    });

    document.querySelectorAll('[data-toggle="inspector"]').forEach(function (btn) {
      btn.addEventListener('click', function () { toggleInspector(); });
    });

    var appEl = document.querySelector('[data-app]');
    var prefsNow = store();
    if (appEl && prefsNow.inspector === 'collapsed') appEl.dataset.inspector = 'collapsed';
    if (appEl && !prefsNow.inspector && window.innerWidth < 1240) appEl.dataset.inspector = 'collapsed';
    if (appEl && !prefsNow.sidebar && window.innerWidth <= 1000) appEl.dataset.sidebar = 'collapsed';

    document.querySelectorAll('[data-set-density]').forEach(function (btn) {
      btn.addEventListener('click', function () { setDensity(btn.dataset.setDensity); });
    });
    document.querySelectorAll('[data-set-motion]').forEach(function (btn) {
      btn.addEventListener('click', function () { setMotion(btn.dataset.setMotion); });
    });
    document.querySelectorAll('[data-set-width]').forEach(function (btn) {
      btn.addEventListener('click', function () { setFrame(btn.dataset.setWidth); });
    });
    document.querySelectorAll('[data-set-scene]').forEach(function (btn) {
      btn.addEventListener('click', function () { setScene(btn.dataset.setScene); });
    });

    document.addEventListener('clay:overlay', function (event) {
      if (event.detail.name === 'palette' && event.detail.open) renderPalette();
    });

    var paletteInput = document.querySelector('[data-pal-input]');
    if (paletteInput) {
      paletteInput.setAttribute('autocomplete', 'off');
      paletteInput.setAttribute('spellcheck', 'false');
    }

    document.querySelectorAll('[data-kbd-mod]').forEach(function (el) { el.textContent = PLATFORM.mod; });
    document.querySelectorAll('[data-kbd-alt]').forEach(function (el) { el.textContent = PLATFORM.alt; });
    document.querySelectorAll('[data-kbd-shift]').forEach(function (el) { el.textContent = PLATFORM.shift; });
    document.querySelectorAll('[data-kbd-platform]').forEach(function (el) { el.textContent = PLATFORM.mod; });
    /* declarative chips: data-kbd="mod+shift+z" renders for this platform */
    document.querySelectorAll('[data-kbd]').forEach(function (el) { el.textContent = key(el.dataset.kbd); });

    /* Layout state is per surface, and only the *user* has an opinion about it:
       a surface where the user never touched the toggle keeps whatever was
       stored. Inferring the state from the DOM on unload let a page with no
       sidebar (the launcher) or a hidden drawer (the inspector below 1240px)
       write a collapse the next surface then inherited — and win over its own
       grid. */
    window.addEventListener('beforeunload', function () {
      if (Object.keys(touched).length) save(touched);
    });
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }

  window.ClayDS = {
    themes: THEMES,
    mod: MOD,
    key: key,
    toast: toast,
    announce: announce,
    setTheme: applyTheme,
    cycleTheme: cycleTheme,
    currentTheme: function () { return prefs.theme; },
    setDensity: setDensity,
    setMotion: setMotion,
    setFrame: setFrame,
    setScene: setScene,
    setCommands: setCommands,
    openPalette: openPalette,
    openOverlay: openOverlay,
    closeOverlay: closeOverlay,
    togglePopover: togglePopover,
    toggleSidebar: toggleSidebar,
    toggleInspector: toggleInspector,
    flash: flash,
    focusZone: focusZone
  };
  /* ==========================================================================
     Markdown-source highlighter + line-numbered editor surface.
     Shared by both Workspace screens. The editor keeps the raw markdown text
     (backticks and all) exactly like Clay's current editor does; it only adds
     structure: per-line blocks, a synced number gutter, a current-line mark and
     an outline derived from headings.
     ====================================================================== */

  /* ==========================================================================
     Workspace file tree — one markup, two skins (geometry differs in CSS only).
     ====================================================================== */

  var ICONS = {
    file: '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true"><path d="M6 3h8l4 4v14H6z"/><path d="M14 3v4h4"/></svg>',
    dir: '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true"><path d="M4 7h5l2 2h9v10H4z"/></svg>',
    caret: '<svg width="9" height="9" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" aria-hidden="true"><path d="M8 6l8 6-8 6"/></svg>'
  };

  function walkTree(nodes, depth, parent, out) {
    nodes.forEach(function (node) {
      node.depth = depth;
      node.parent = parent;
      out.push(node);
      if (node.children) walkTree(node.children, depth + 1, node.id, out);
    });
    return out;
  }

  function renderTree(host, nodes, options) {
    options = options || {};
    var order = walkTree(nodes, 0, '', []);
    var html = '';
    order.forEach(function (node) {
      var isDir = node.type === 'dir';
      var expandable = !!(node.children && node.children.length);
      var active = options.selected === node.id;
      var leaf = node.id.split('/').filter(Boolean).pop();
      var bundled = options.sources && options.sources[node.id];
      html += '<div class="tree-row" role="treeitem" data-tree-item data-id="' + node.id + '"'
        + ' data-type="' + node.type + '" data-depth="' + node.depth + '"'
        + (node.parent ? ' data-parent="' + node.parent + '"' : '')
        + (expandable ? ' aria-expanded="true"' : '')
        + (active ? ' aria-selected="true" data-active="true" tabindex="0"' : ' tabindex="-1"')
        + ' style="padding-left:' + (6 + node.depth * 14) + 'px"'
        + ' title="' + node.id + '">'
        + '<span class="tree-caret"' + (expandable ? ' data-caret data-open="true"' : '') + '>'
        + (expandable ? ICONS.caret : '') + '</span>'
        + '<span class="tree-icon">' + (isDir ? ICONS.dir : ICONS.file) + '</span>'
        + '<span class="tree-name">' + leaf + (isDir ? '/' : '') + '</span>'
        + (node.count ? '<span class="tree-count">' + node.count + '</span>' : '')
        + (isDir && !expandable ? '' : '')
        + '</div>';
      void bundled;
    });
    host.innerHTML = html;
    initTree(host);
    return html;
  }

  var MD = (function () {
    function esc(text) {
      return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    }

    function inline(text, base) {
      var out = esc(text);
      out = out.replace(/`([^`]+)`/g, '<span class="md-code">`$1`</span>');
      out = out.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<span class="md-link">[$1]<span class="md-url">($2)</span></span>');
      out = out.replace(/(\*\*[^*]+\*\*)/g, '<span class="md-strong">$1</span>');
      return out;
    }

    function line(raw, index) {
      var base = 'md-p';
      var body = raw;
      var marker = '';
      var head = /^(#{1,6})(\s+)(.*)$/.exec(raw);
      var out;
      if (head) {
        var level = head[1].length;
        base = 'md-h md-h' + level;
        marker = '<span class="md-mark">' + head[1] + head[2] + '</span>';
        var rest = head[3];
        var stamp = /^((?:\d{2}-\d{2}-\d{2}(?: \d{2}:\d{2})?)(\s+—\s+)?)(.*)$/.exec(rest);
        if (stamp && level >= 2) {
          out = marker
            + '<span class="md-ts">' + esc(stamp[1]) + (stamp[2] ? esc(stamp[2]) : '') + '</span>'
            + inline(stamp[3], base);
        } else {
          out = marker + inline(rest, base);
        }
      } else if (/^(\s*)([-*+])\s+/.test(raw)) {
        out = raw.replace(/^(\s*)([-*+])(\s+)(.*)$/,
          '<span class="md-mark">$1$2$3</span>' + inline('$4', 'md-li'));
        base = 'md-li';
      } else if (/^\s*>/.test(raw)) {
        out = raw.replace(/^(\s*>)(\s*)(.*)$/,
          '<span class="md-mark">$1$2</span>' + inline('$3', 'md-quote'));
        base = 'md-quote';
      } else if (/^\s*$/.test(raw)) {
        out = '';
        base = 'md-blank';
      } else {
        out = inline(raw, base);
      }
      return '<span class="ln ' + base + '" data-line="' + index + '">' + out + '</span>';
    }

    function render(text) {
      return text.replace(/\r\n?/g, '\n').split('\n').map(line).join('');
    }

    function outline(text) {
      var items = [];
      text.replace(/\r\n?/g, '\n').split('\n').forEach(function (raw, index) {
        var m = /^(#{1,6})\s+(.*)$/.exec(raw);
        if (!m) return;
        items.push({
          line: index,
          level: m[1].length,
          title: m[2].replace(/\s*—\s*/, ' — '),
          stamp: (/^(\d{2}-\d{2}-\d{2} \d{2}:\d{2})/.exec(m[2]) || [])[1] || ''
        });
      });
      return items;
    }

    return { render: render, outline: outline, escape: esc };
  })();

  function createEditor(options) {
    var canvas = options.canvas;
    var gutter = options.gutter;
    var value = options.value || '';
    var saved = value;
    var past = [];
    var future = [];
    var lineNodes = [];
    var numberNodes = [];
    var currentLine = -1;

    function lines() { return canvas.querySelectorAll('.ln'); }

    function syncGutter() {
      lineNodes = Array.prototype.slice.call(lines());
      numberNodes = Array.prototype.slice.call(gutter.querySelectorAll('.gln'));
      lineNodes.forEach(function (node, i) {
        var num = numberNodes[i];
        if (num) num.style.height = node.offsetHeight + 'px';
        if (num) num.dataset.line = node.dataset.line;
      });
    }

    function render() {
      canvas.innerHTML = MD.render(value);
      var total = canvas.querySelectorAll('.ln').length;
      var html = '';
      for (var i = 0; i < total; i++) html += '<span class="gln" data-line="' + i + '">' + (i + 1) + '</span>';
      gutter.innerHTML = html;
      syncGutter();
      paintCurrent();
    }

    function markCurrent(node) {
      if (!node) return;
      var index = Number(node.dataset.line);
      if (index === currentLine) return;
      lineNodes.forEach(function (n) { delete n.dataset.current; });
      numberNodes.forEach(function (n) { delete n.dataset.current; });
      node.dataset.current = 'true';
      var num = gutter.querySelector('.gln[data-line="' + index + '"]');
      if (num) num.dataset.current = 'true';
      currentLine = index;
    }

    function paintCurrent() {
      var sel = window.getSelection();
      if (!sel || !sel.rangeCount) return;
      var node = sel.anchorNode;
      if (!node || !canvas.contains(node)) return;
      if (node.nodeType === 3) node = node.parentElement;
      var lineEl = node.closest ? node.closest('.ln') : null;
      if (lineEl) markCurrent(lineEl);
    }

    function emit() {
      document.dispatchEvent(new CustomEvent('clay:editor', {
        detail: {
          dirty: isDirty(),
          canUndo: past.length > 0,
          canRedo: future.length > 0,
          length: value.length
        }
      }));
    }

    function snapshot() {
      if (past[past.length - 1] === value) return;
      past.push(value);
      if (past.length > 60) past.shift();
      future.length = 0;
    }

    var snapshotTimer = null;
    function scheduleSnapshot() {
      clearTimeout(snapshotTimer);
      snapshotTimer = setTimeout(snapshot, 350);
    }

    function isDirty() { return value !== saved; }

    canvas.setAttribute('contenteditable', 'true');
    canvas.setAttribute('spellcheck', 'false');
    canvas.setAttribute('role', 'textbox');
    canvas.setAttribute('aria-multiline', 'true');
    canvas.setAttribute('aria-label', options.label || 'Document source');

    canvas.addEventListener('input', function () {
      value = canvas.innerText.replace(/\r\n?/g, '\n').replace(/\n$/, '');
      syncGutter();
      scheduleSnapshot();
      emit();
    });

    canvas.addEventListener('keyup', paintCurrent);
    canvas.addEventListener('mouseup', paintCurrent);
    canvas.addEventListener('keydown', function (event) {
      if (event.key === 'Tab') {
        event.preventDefault();
        document.execCommand('insertText', false, '  ');
      }
      if (event.key === 'Enter') {
        setTimeout(function () { syncGutter(); paintCurrent(); emit(); }, 0);
      }
    });

    document.addEventListener('selectionchange', function () {
      if (document.activeElement === canvas || canvas.contains(document.activeElement)) paintCurrent();
    });

    if (window.ResizeObserver) {
      new ResizeObserver(function () { syncGutter(); }).observe(canvas);
    } else {
      window.addEventListener('resize', syncGutter);
    }

    var api = {
      get value() { return value; },
      get dirty() { return isDirty(); },
      outline: function () { return MD.outline(value); },
      setValue: function (next, opts) {
        opts = opts || {};
        if (!opts.noHistory) snapshot();
        value = next;
        render();
        if (opts.saved) saved = next;
        emit();
      },
      markSaved: function () { saved = value; emit(); },
      save: function () {
        saved = value;
        emit();
        return saved;
      },
      undo: function () {
        if (!past.length) return false;
        future.push(value);
        var prev = past.pop();
        value = prev;
        render();
        emit();
        return true;
      },
      redo: function () {
        if (!future.length) return false;
        past.push(value);
        value = future.pop();
        render();
        emit();
        return true;
      },
      gotoLine: function (index, opts) {
        var node = canvas.querySelector('.ln[data-line="' + index + '"]');
        if (!node) return;
        node.scrollIntoView({ block: 'start', behavior: (options.smooth === false ? 'auto' : 'smooth') });
        markCurrent(node);
        flash(node);
      },
      refresh: function () { render(); },
      focus: function () { canvas.focus(); }
    };

    render();
    emit();
    return api;
  }

  window.ClayDS.markdown = MD;
  window.ClayDS.createEditor = createEditor;
  window.ClayDS.renderTree = renderTree;
  window.ClayDS.icons = ICONS;
})();
