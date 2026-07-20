// webChronicle external link interceptor + nav state reporter
(function () {
  var origin = window.location.origin;

  function reportNav() {
    var state = history.state;
    var pos = (state && typeof state.navPos === "number") ? state.navPos : null;
    if (pos === null) pos = history.length - 1;
    history.replaceState({ navPos: pos }, "");
    window.parent.postMessage({
      type: "wc-nav-state",
      canGoBack: pos > 0,
      canGoForward: pos < history.length - 1,
      url: location.href
    }, "*");
  }

  // Neutralize target="_top"|"_parent"|"_blank" — keep all nav inside iframe
  document.addEventListener("click", function (e) {
    var a = e.target.closest("a[href]");
    if (!a) return;
    // Force internal links to stay in iframe
    if (a.target === "_top" || a.target === "_parent" || a.target === "_blank") {
      a.target = "_self";
    }
    var href = a.href;
    if (!href) return;
    // External link → open in system browser
    if (!href.startsWith(origin) && !href.startsWith("/")) {
      if (href.startsWith("http://") || href.startsWith("https://")) {
        e.preventDefault();
        e.stopPropagation();
        window.parent.postMessage({ type: "wc-external-link", url: href }, "*");
      }
    }
  }, true);

  window.addEventListener("message", function (e) {
    if (e.data?.type === "wc-go-back") history.back();
    if (e.data?.type === "wc-go-forward") history.forward();
    if (e.data?.type === "wc-go-home" && e.data?.url) location.href = e.data.url;
  });

  reportNav();
})();