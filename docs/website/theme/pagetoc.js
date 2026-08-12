/* Right-hand page outline.
 *
 * mdBook ships a sidebar of chapters and nothing that shows where you are
 * inside one. On a reference page listing eighteen commands or twenty-nine
 * environment variables, that is the navigation people actually want.
 *
 * Built from the headings already in the DOM, so it cannot disagree with the
 * page. h2 and h3 only: h4 is used here for asides and would make the outline
 * longer than the section it describes.
 */
(function () {
  'use strict';

  function build() {
    var main = document.querySelector('.content main');
    if (!main) return;

    var headings = main.querySelectorAll('h2[id], h3[id]');
    // Two entries is a list, not an outline. Below that it costs more attention
    // than it saves.
    if (headings.length < 3) return;

    var toc = document.createElement('nav');
    toc.className = 'pagetoc';
    toc.setAttribute('aria-label', 'On this page');

    var label = document.createElement('div');
    label.className = 'pagetoc-label';
    label.textContent = 'On this page';
    toc.appendChild(label);

    var links = [];
    headings.forEach(function (h) {
      var a = document.createElement('a');
      a.href = '#' + h.id;
      a.className = h.tagName.toLowerCase();
      // The heading's own anchor link is appended by mdBook; textContent picks
      // it up as a stray character, so read the text nodes instead.
      a.textContent = Array.prototype.filter
        .call(h.childNodes, function (n) { return n.nodeType === 3 || !n.classList || !n.classList.contains('header'); })
        .map(function (n) { return n.textContent; })
        .join('')
        .trim();
      toc.appendChild(a);
      links.push({ el: a, target: h });
    });

    document.querySelector('.content').appendChild(toc);

    // Highlight the heading currently at the top of the viewport. rootMargin
    // pulls the trigger line down from the very top so a heading counts as
    // "current" while its section is being read, not only as it crosses.
    if (!('IntersectionObserver' in window)) return;

    var visible = new Set();
    var observer = new IntersectionObserver(function (entries) {
      entries.forEach(function (e) {
        if (e.isIntersecting) visible.add(e.target); else visible.delete(e.target);
      });
      var current = null;
      links.forEach(function (l) { if (visible.has(l.target)) { current = current || l; } });
      links.forEach(function (l) { l.el.classList.toggle('active', l === current); });
    }, { rootMargin: '-80px 0px -70% 0px' });

    links.forEach(function (l) { observer.observe(l.target); });
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', build);
  } else {
    build();
  }
})();
