/* Put the manual back inside the site it is served from (#477).
 *
 * The palette and the fonts already match, which made the seam invisible until
 * someone tried to leave: nothing on any page linked back to
 * omni.weekndlabs.com, so `Docs` in the site navbar was a one-way door.
 *
 * Done here rather than by forking `index.hbs`, because owning mdBook's whole
 * page template in order to add a header is a bad trade on every future upgrade.
 * `.menu-title` and `.right-buttons` are the classes `theme/omni.css` already
 * styles, so this adds to markup the theme is already committed to.
 *
 * Relative links, not absolute ones. `mdbook serve` on localhost has no site
 * around it, and hardcoding the production origin would send a local reader to
 * production. `../` from /docs/ is the site root and is also simply wrong-free
 * when the book is opened from disk: it fails to navigate rather than navigating
 * somewhere misleading.
 */
(function () {
  'use strict';

  // Where a reader of the manual actually goes next. Deliberately short: the
  // sidebar is the navigation, this is the way out.
  var LINKS = [
    { href: '../', text: 'Site' },
    { href: '../releases', text: 'Releases' },
    { href: '../blog', text: 'Journal' },
    { href: 'https://discord.gg/zHTuvZhF2M', text: 'Discord', external: true },
  ];

  // The manual is English only, and the README is not. A reader who arrived
  // through the Indonesian or Japanese README used to land here with no way back
  // to their own language, so this points at the translations that already exist
  // and are already maintained rather than pretending 35 pages are translated.
  //
  // Absolute GitHub URLs on purpose. These files live in the omni repository, not
  // in the site, so there is no relative path from /docs/ that reaches them.
  var REPO = 'https://github.com/fajarhide/omni/blob/main/';
  var LANGS = [
    { code: 'EN', label: 'English', href: REPO + 'README.md' },
    { code: 'ID', label: 'Bahasa Indonesia', href: REPO + 'i18n/README-id.md' },
    { code: 'JA', label: '日本語', href: REPO + 'i18n/README-ja.md' },
    { code: 'ZH', label: '简体中文', href: REPO + 'i18n/README-zh.md' },
    { code: 'KO', label: '한국어', href: REPO + 'i18n/README-ko.md' },
    { code: 'VI', label: 'Tiếng Việt', href: REPO + 'i18n/README-vi.md' },
    { code: 'AR', label: 'العربية', href: REPO + 'i18n/README-ar.md' },
  ];

  function depth() {
    // mdBook stamps the page's own distance from the book root as
    // `path_to_root`: "" at the root, "../" one level down. Taking it from there
    // beats probing a link, which is what the first draft did and got wrong: it
    // read the sidebar, and the sidebar is rendered client-side by toc.js, so at
    // DOMContentLoaded there was nothing to read and every page resolved as the
    // root. Counting slashes in the URL is no better, because Vercel serves
    // these paths without the .html there is nothing to discount.
    //
    // Read as a bare identifier, not off `window`. mdBook declares it as
    // `const path_to_root = "../";` in a classic script, and a top-level `const`
    // lives in the global lexical environment without becoming a property of
    // `window`, so `window.path_to_root` is undefined while `path_to_root`
    // resolves. Both scripts being classic is what makes that work; `typeof`
    // keeps it from throwing if a future mdBook stops emitting it.
    return typeof path_to_root === 'string' ? path_to_root : '';
  }

  function build() {
    var bar = document.querySelector('.right-buttons');
    var title = document.querySelector('.menu-title');
    if (!bar && !title) return;

    var root = depth();

    // The wordmark is the way home on every site that has one, so make it one
    // here too rather than adding a fifth link that says Home.
    if (title && !title.querySelector('a')) {
      var home = document.createElement('a');
      home.href = root + '../';
      home.textContent = title.textContent.trim();
      home.className = 'site-home';
      title.textContent = '';
      title.appendChild(home);
    }

    if (!bar) return;
    var nav = document.createElement('nav');
    nav.className = 'site-links';
    nav.setAttribute('aria-label', 'Site');
    LINKS.forEach(function (l) {
      var a = document.createElement('a');
      a.href = l.external ? l.href : root + l.href;
      a.textContent = l.text;
      if (l.external) {
        a.target = '_blank';
        a.rel = 'noopener noreferrer';
      }
      nav.appendChild(a);
    });
    // Before mdBook's icon buttons, so print/repo/edit stay where a returning
    // reader expects them at the far right.
    bar.insertBefore(nav, bar.firstChild);
    bar.insertBefore(languageMenu(), nav.nextSibling);
  }

  /// A `<details>` rather than a scripted dropdown.
  ///
  /// Click-to-open, click-outside-to-close, Escape and keyboard focus are all
  /// behaviour the element already has, and every line of JS that reimplements
  /// them is a line that can get the accessibility wrong. The only script here is
  /// closing it after a choice, which a plain `<details>` does not do because the
  /// link navigates away in a new tab.
  function languageMenu() {
    var details = document.createElement('details');
    details.className = 'lang-menu';

    var summary = document.createElement('summary');
    summary.setAttribute('aria-label', 'Choose a language');
    summary.title = 'Read the introduction in another language';
    summary.textContent = 'EN';
    details.appendChild(summary);

    var list = document.createElement('div');
    list.className = 'lang-list';
    LANGS.forEach(function (l) {
      var a = document.createElement('a');
      a.href = l.href;
      a.textContent = l.label;
      a.target = '_blank';
      a.rel = 'noopener noreferrer';
      a.addEventListener('click', function () {
        details.open = false;
      });
      list.appendChild(a);
    });
    details.appendChild(list);
    return details;
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', build);
  } else {
    build();
  }
})();
