// Hold the page until its fonts are ready (at most 1.2 s), then fade it in
// once, so text never paints in a fallback face and re-renders. Runs after
// the stylesheets (so the @font-face rules exist); skipped when the fonts are
// already cached.
(function () {
  var root = document.documentElement;
  if (!document.fonts || !document.fonts.load) return;
  var faces = ['16px "EB Garamond"', 'italic 16px "EB Garamond"', '16px "Courier Prime"', 'bold 16px "Courier Prime"'];
  if (faces.every(function (f) { return document.fonts.check(f); })) return;
  root.classList.add("fonts-loading");
  var reveal = function () { root.classList.remove("fonts-loading"); };
  Promise.all(faces.map(function (f) { return document.fonts.load(f); })).then(reveal, reveal);
  setTimeout(reveal, 1200);
})();
