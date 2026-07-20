// webChronicle external link interceptor + nav state reporter
// Runs inside served snapshot pages (same-origin as iframe, cross-origin to Tauri parent)
(function () {
  var origin = window.location.origin;

  // Report nav state to parent on every load
  function reportNav() {
    window.parent.postMessage({
      type: "wc-nav-state",
      canGoBack: history.length > 1,
      canGoForward: false,
      url: location.href
    }, "*");
  }

  // Intercept external links
  document.addEventListener("click", function (e) {
    var a = e.target.closest("a[href]");
    if (!a) return;
    var href = a.href;
    if (!href) return;
    if (href.startsWith(origin) || href.startsWith("/")) return;
    if (!href.startsWith("http://") && !href.startsWith("https://")) return;
    e.preventDefault();
    e.stopPropagation();
    window.parent.postMessage({ type: "wc-external-link", url: href }, "*");
  });

  reportNav();
})();