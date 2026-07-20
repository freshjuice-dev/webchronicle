// webChronicle tracker blocker — blocks known tracker domains, lets legit CDN through
(function () {
  var origin = window.location.origin;
  var TRACKERS = typeof WC_TRACKER_DOMAINS !== "undefined" ? WC_TRACKER_DOMAINS : new Set();

  function isTracker(url) {
    if (!url) return false;
    if (url.startsWith(origin) || url.startsWith("/") || url.startsWith("data:") || url.startsWith("blob:")) return false;
    if (!url.startsWith("http://") && !url.startsWith("https://")) return false;
    try {
      var h = new URL(url).hostname.toLowerCase();
      // Check exact match and parent domain match
      if (TRACKERS.has(h)) return true;
      var parts = h.split(".");
      for (var i = 1; i < parts.length; i++) {
        var parent = parts.slice(i).join(".");
        if (TRACKERS.has(parent)) return true;
      }
    } catch (e) {}
    return false;
  }

  // 1. Strip external <script> elements pointing to tracker domains
  document.addEventListener("DOMContentLoaded", function () {
    document.querySelectorAll("script[src]").forEach(function (s) {
      if (isTracker(s.src)) s.remove();
    });
  });

  // 2. Block fetch to tracker domains
  var origFetch = window.fetch;
  if (origFetch) {
    window.fetch = function (input) {
      var url = typeof input === "string" ? input : (input && input.url) || "";
      if (isTracker(url)) return Promise.reject(new Error("Blocked by webChronicle: " + url));
      return origFetch.apply(this, arguments);
    };
  }

  // 3. Block XMLHttpRequest to tracker domains
  var origOpen = XMLHttpRequest.prototype.open;
  XMLHttpRequest.prototype.open = function (method, url) {
    if (isTracker(url)) {
      this.send = function () {};
      this.open = function () {};
      return;
    }
    return origOpen.apply(this, arguments);
  };

  // 4. Block navigator.sendBeacon to tracker domains
  if (navigator.sendBeacon) {
    var origBeacon = navigator.sendBeacon.bind(navigator);
    navigator.sendBeacon = function (url) {
      if (isTracker(url)) return false;
      return origBeacon.apply(navigator, arguments);
    };
  }

  // 5. Block tracker image beacons (tracking pixels)
  var origImgSrc = Object.getOwnPropertyDescriptor(HTMLImageElement.prototype, "src");
  if (origImgSrc && origImgSrc.set) {
    Object.defineProperty(HTMLImageElement.prototype, "src", {
      set: function (v) {
        if (isTracker(v)) return;
        origImgSrc.set.call(this, v);
      },
      get: function () { return origImgSrc.get.call(this); }
    });
  }
})();
