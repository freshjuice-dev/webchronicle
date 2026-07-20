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

  window.addEventListener("message", function (e) {
    if (e.data?.type === "wc-go-back") history.back();
    if (e.data?.type === "wc-go-forward") history.forward();
  });

  reportNav();
})();