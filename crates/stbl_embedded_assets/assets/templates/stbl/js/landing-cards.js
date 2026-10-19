(function () {
  "use strict";

  var mobile = window.matchMedia("(max-width: 767px)");
  document.querySelectorAll('.landing-card[data-folded], .landing-features[data-folded], .landing-section[data-folded]').forEach(function (card, index) {
    var mode = card.getAttribute("data-folded");
    if (!["yes", "no", "auto"].includes(mode)) return;
    var isCard = card.classList.contains("landing-card");
    var content = isCard ? card.querySelector(":scope > .landing-card__content") : card;
    var heading = content && content.firstElementChild;
    // Section metadata images precede its introductory Markdown.
    if (!isCard && heading && heading.matches(".landing-section__icon, .landing-section__image")) {
      heading = heading.nextElementSibling;
    }
    // Split section introductions keep their heading in the text column.
    if (!isCard && heading && heading.matches(".landing-section__intro")) {
      heading = heading.querySelector(":scope > .landing-hero__content > :first-child");
    }
    // Never use a nested card's title as its parent's title.
    if (!heading || !/^H[1-6]$/.test(heading.tagName)) return;

    var header = document.createElement("div");
    header.className = "landing-fold__header" + (isCard ? " landing-card__header" : "");
    var button = document.createElement("button");
    button.type = "button";
    button.className = "landing-fold__toggle" + (isCard ? " landing-card__toggle" : "");
    button.setAttribute("aria-label", heading.textContent.trim());
    var body = document.createElement("div");
    body.className = "landing-fold__body" + (isCard ? " landing-card__body" : "");
    var id = "landing-fold-body-" + index;
    while (document.getElementById(id)) id += "-body";
    body.id = id;
    button.setAttribute("aria-controls", id);

    // Keep the original heading (including its anchor) and all body nodes.
    if (isCard) {
      content.insertBefore(header, heading);
      header.appendChild(heading);
      header.appendChild(button);
      while (header.nextSibling) body.appendChild(header.nextSibling);
      content.appendChild(body);
    } else {
      // Keep expanded section layouts intact by leaving a slot for the heading.
      var headingSlot = document.createComment("folding heading");
      heading.before(headingSlot);
      while (content.firstChild) body.appendChild(content.firstChild);
      content.appendChild(header);
      content.appendChild(body);
      header.appendChild(button);
    }

    function setFolded(folded) {
      // Retain focus on the toggle when a breakpoint hides focused body content.
      if (folded && body.contains(document.activeElement)) button.focus();
      if (!isCard) {
        if (folded) header.insertBefore(heading, button);
        else headingSlot.after(heading);
      }
      body.hidden = folded;
      card.classList.toggle("is-folded", folded);
      button.setAttribute("aria-expanded", String(!folded));
    }
    button.addEventListener("click", function () {
      setFolded(!body.hidden);
    });
    card.classList.add("is-foldable");
    var automatic = mode === "auto";
    setFolded(automatic ? mobile.matches : mode === "yes");
    if (automatic) {
      mobile.addEventListener("change", function () {
        setFolded(mobile.matches);
      });
    }
  });
})();
