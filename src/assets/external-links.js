// webChronicle external link interceptor
// Runs inside served snapshot pages. Intercepts external links and
// posts a message to the parent Tauri window to open them externally.
(function () {
  var origin = window.location.origin;
  document.addEventListener("click", function (e) {
    var a = e.target.closest("a[href]");
    if (!a) return;
    var href = a.href;
    if (!href) return;
    // Internal: same origin or relative
    if (href.startsWith(origin) || href.startsWith("/")) return;
    // Skip non-http(s) links
    if (!href.startsWith("http://") && !href.startsWith("https://")) return;
    e.preventDefault();
    e.stopPropagation();
    window.parent.postMessage({ type: "wc-external-link", url: href }, "*");
  });
})();