(function () {
  "use strict";

  var mobile = window.matchMedia("(max-width: 767px)");
  document.querySelectorAll('.landing-card[data-folded="yes"], .landing-card[data-folded="no"], .landing-card[data-folded="auto"]').forEach(function (card, index) {
    var content = card.querySelector(".landing-card__content");
    var heading = content && content.firstElementChild;
    // A heading is required so a compact card always has a meaningful title.
    if (!heading || !/^H[1-6]$/.test(heading.tagName)) return;

    var header = document.createElement("div");
    header.className = "landing-card__header";
    var button = document.createElement("button");
    button.type = "button";
    button.className = "landing-card__toggle";
    button.setAttribute("aria-label", heading.textContent.trim());
    var body = document.createElement("div");
    body.className = "landing-card__body";
    var id = "landing-card-body-" + index;
    while (document.getElementById(id)) id += "-body";
    body.id = id;
    button.setAttribute("aria-controls", id);

    // Keep the original heading (including its anchor) and all body nodes.
    content.insertBefore(header, heading);
    header.appendChild(heading);
    header.appendChild(button);
    while (header.nextSibling) body.appendChild(header.nextSibling);
    content.appendChild(body);

    function setFolded(folded) {
      // Retain focus on the toggle when a breakpoint hides focused body content.
      if (folded && body.contains(document.activeElement)) button.focus();
      body.hidden = folded;
      card.classList.toggle("is-folded", folded);
      button.setAttribute("aria-expanded", String(!folded));
    }
    button.addEventListener("click", function () {
      setFolded(!body.hidden);
    });
    card.classList.add("is-foldable");
    var mode = card.getAttribute("data-folded");
    var automatic = mode === "auto";
    setFolded(automatic ? mobile.matches : mode === "yes");
    if (automatic) {
      mobile.addEventListener("change", function () {
        setFolded(mobile.matches);
      });
    }
  });
})();
