/* Responsive posters for the browser's native video player. */
(function () {
  "use strict";
  document.querySelectorAll("video[data-video-poster]").forEach(function (video) {
    var data;
    try { data = JSON.parse(video.getAttribute("data-video-poster")); }
    catch (_) { return; }
    var fallback = video.getAttribute("poster");
    var picture = document.createElement("picture");
    picture.hidden = true;
    picture.style.display = "none";
    picture.setAttribute("aria-hidden", "true");
    var sources = (data.sources || []).map(function (variant) {
      var source = document.createElement("source");
      source.type = variant.type;
      source.srcset = variant.srcset;
      picture.appendChild(source);
      return source;
    });
    var image = document.createElement("img");
    image.alt = "";
    image.decoding = "async";
    image.onload = function () { video.poster = image.currentSrc || image.src; };
    image.onerror = function () { video.poster = fallback; };
    var started = false;
    function resize() {
      var width = video.getBoundingClientRect().width;
      if (!width) return;
      var sizes = Math.ceil(width) + "px";
      sources.forEach(function (source) { source.sizes = sizes; });
      image.sizes = sizes;
    }
    function start() {
      if (started) return;
      started = true;
      resize();
      if (data.srcset) image.srcset = data.srcset;
      image.src = data.src;
      picture.appendChild(image);
      video.parentNode.appendChild(picture);
      if (window.ResizeObserver) new ResizeObserver(resize).observe(video);
      window.addEventListener("resize", resize, { passive: true });
    }
    if (window.IntersectionObserver) {
      var observer = new IntersectionObserver(function (entries) {
        if (entries.some(function (entry) { return entry.isIntersecting; })) {
          start();
          observer.disconnect();
        }
      }, { rootMargin: "200px" });
      observer.observe(video);
    } else { start(); }
  });
})();
