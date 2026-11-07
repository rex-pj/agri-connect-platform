// Alpine components for every page. This is the CSP build of Alpine: templates
// may only name a property or method registered here (x-on:click="toggle"),
// never write an expression (a convention test enforces it). Every component
// enhances markup that already works without JavaScript.
"use strict";

document.addEventListener("alpine:init", () => {
  const Alpine = window.Alpine;

  // Top-bar search. Without JS the toggle is a link to the search page.
  Alpine.data("navSearch", () => ({
    open: false,

    init() {
      // The server renders data-open="true" when the page shows a search.
      this.open = this.$el.dataset.open === "true";
    },
    toggle() {
      this.open = !this.open;
      if (this.open) {
        // The account menu closes itself on this.
        this.$dispatch("nav-search-opened");
        this.$nextTick(() => this.$refs.input.focus());
      }
    },
    close() {
      this.open = false;
    },
    get closed() {
      return !this.open;
    },
    get hidden() {
      return this.open ? "false" : "true";
    },
    get expanded() {
      return this.open ? "true" : "false";
    },
    get buttonClass() {
      return this.open ? "active" : "";
    },
    get rowClass() {
      return this.open ? "open" : "";
    },
    // Closed, the field is invisible; keep it out of the tab order.
    get tabIndex() {
      return this.open ? "0" : "-1";
    },
  }));

  // A top-bar dropdown. Opening it announces `openedEvent`, which bubbles to
  // the top bar (closing the search row) and reaches the other dropdown
  // (closing it), so one thing is open at a time.
  const topBarMenu = (openedEvent) => ({
    open: false,

    toggle() {
      this.open = !this.open;
      if (this.open) {
        this.$dispatch(openedEvent);
      }
    },
    close() {
      this.open = false;
    },
    get expanded() {
      return this.open ? "true" : "false";
    },
    get buttonClass() {
      return this.open ? "active" : "";
    },
    get chevronClass() {
      return this.open ? "rotated" : "";
    },
  });

  // Account dropdown. Without JS the button is a link to /account.
  Alpine.data("profileMenu", () => topBarMenu("profile-menu-opened"));

  // Language dropdown. Without JS the button is a link to the picker page.
  Alpine.data("languageMenu", () => topBarMenu("language-menu-opened"));

  // Turns an in-page section into a bottom sheet (a centred dialog on a wide
  // screen, by CSS). Without JS the section is part of the page and its
  // trigger is an anchor link to it. The server sets data-open="true" when
  // it re-shows a form with errors. The sheet takes its first state still;
  // only the visitor's show or hide animates (`animated`).
  Alpine.data("dialog", () => ({
    modal: false,
    isOpen: false,
    animated: false,

    init() {
      this.modal = true;
      this.isOpen = this.$el.dataset.open === "true";
      if (this.isOpen) this.focusFirst();
    },
    show() {
      this.animated = true;
      this.isOpen = true;
      this.focusFirst();
    },
    hide() {
      this.animated = true;
      this.isOpen = false;
    },
    focusFirst() {
      this.$nextTick(() => this.$refs.first?.focus());
    },
    get sheetClass() {
      return ["is-modal", this.animated && "is-animated", this.isOpen && "is-open"].filter(Boolean).join(" ");
    },
  }));

  // Shows the photo chosen in a file box before the form is sent (a blob:
  // URL, allowed by the page CSP), and hides the native box: the photo tile
  // (a label for it) opens the picker instead. Without JS the box works the
  // same, just without the preview. `data-has-current`: a photo is stored.
  Alpine.data("imagePreview", () => ({
    src: "",
    chosen: false,
    hasCurrent: false,

    init() {
      this.hasCurrent = this.$el.dataset.hasCurrent === "true";
    },
    get unchanged() {
      return !this.chosen;
    },
    get hasPhoto() {
      return this.chosen || this.hasCurrent;
    },
    get tileClass() {
      return this.hasPhoto ? "photo-picker__tile--filled" : "";
    },
    get inputClass() {
      return "visually-hidden";
    },
    pick() {
      if (this.src) URL.revokeObjectURL(this.src);
      const file = this.$el.files?.[0];
      this.chosen = Boolean(file);
      this.src = file ? URL.createObjectURL(file) : "";
    },
  }));

  // A select that saves its form as soon as it changes. Without JS the
  // form's own submit button does the same.
  // On a form: any change saves it. Its save button (x-show="withoutScript")
  // is only for visitors without JS.
  Alpine.data("autoSubmit", () => ({
    withoutScript: false,

    submit() {
      this.$root.requestSubmit();
    },
  }));

  // Folds a panel behind a toggle button. Without JS the panel stays open and
  // the button (x-cloak) stays hidden.
  Alpine.data("collapse", () => ({
    open: false,

    toggle() {
      this.open = !this.open;
    },
    close() {
      this.open = false;
    },
    get expanded() {
      return this.open ? "true" : "false";
    },
    get buttonClass() {
      return this.open ? "active" : "";
    },
    get panelClass() {
      return this.open ? "is-collapsible open" : "is-collapsible";
    },
  }));

  // Tabs that are links (?tab=…): without JS each one reloads the page with
  // that tab. With JS a click switches the panels in place and puts the link
  // in the address bar, so a reload or a shared URL keeps the tab. Tabs carry
  // data-tab, panels data-panel; the "all" tab shows every panel.
  Alpine.data("tabs", () => ({
    tab: "all",

    init() {
      this.tab = this.$el.dataset.tab || "all";
    },
    select(event) {
      const link = event.currentTarget;
      this.tab = link.dataset.tab;
      history.replaceState(null, "", link.href);
    },
    get current() {
      return this.$el.dataset.tab === this.tab ? "page" : false;
    },
    get panelHidden() {
      return !(this.tab === "all" || this.tab === this.$el.dataset.panel);
    },
  }));

  // Show/hide a password field. Without JS the field is a normal password
  // input and the button (x-cloak) stays hidden.
  Alpine.data("passwordToggle", () => ({
    shown: false,

    toggle() {
      this.shown = !this.shown;
    },
    get hidden() {
      return !this.shown;
    },
    get inputType() {
      return this.shown ? "text" : "password";
    },
    get pressed() {
      return this.shown ? "true" : "false";
    },
    // The button shows only with JS, so only then the box joins it.
    get groupClass() {
      return "password-group--joined";
    },
  }));

  // The category form's name and slug boxes. While the slug box is blank (or
  // was filled from here), typing a name fills it with the server's free slug
  // for that name; typing a slug checks it against the other categories and
  // offers a free one. Emptying the box hands it back to the name. Without JS
  // a blank slug is made on save, and the server checks every save anyway.
  // Attributes: data-url (the suggestion endpoint), data-id (the category
  // being edited), data-{free,numbered,taken,fix}-text (the notes).
  Alpine.data("slugField", () => ({
    // "", "free", "numbered" (filled, with a number added), "taken", "fix".
    state: "",
    suggestion: "",
    auto: false,
    timer: null,
    asked: 0,

    init() {
      this.auto = this.box("slug").value.trim() === "";
    },
    box(name) {
      return this.$root.querySelector(`[name="${name}"]`);
    },
    // Input events of both boxes bubble here.
    changed(event) {
      const { name, value } = event.target;
      if (name === "slug") {
        this.auto = value.trim() === "";
        if (this.auto) this.ask(this.box("name").value, "name");
        else this.ask(value, "slug");
      } else if (name === "name" && this.auto) {
        this.ask(value, "name");
      }
    },
    ask(text, from) {
      clearTimeout(this.timer);
      this.timer = setTimeout(() => this.suggest(text.trim(), from), 300);
    },
    async suggest(text, from) {
      const ticket = ++this.asked;
      const { url, id } = this.$root.dataset;
      const params = new URLSearchParams({ text });
      if (id) params.set("id", id);
      let answer;
      try {
        const response = await fetch(`${url}?${params}`, { headers: { Accept: "application/json" } });
        if (!response.ok) return;
        answer = await response.json();
      } catch {
        // Offline or signed out: the save still checks.
        return;
      }
      // A later keystroke has asked again; its answer wins.
      if (ticket !== this.asked) return;
      this.suggestion = answer.slug;
      if (from === "name") {
        if (!this.auto) return;
        this.box("slug").value = answer.slug;
        this.state = !answer.slug ? "" : answer.taken ? "numbered" : "free";
      } else if (answer.slug === text) {
        this.state = "free";
      } else {
        this.state = answer.taken ? "taken" : "fix";
      }
    },
    apply() {
      this.box("slug").value = this.suggestion;
      this.auto = false;
      this.state = "free";
    },
    get hasNote() {
      return this.state !== "";
    },
    get note() {
      return this.state ? this.$root.dataset[`${this.state}Text`] : "";
    },
    get noteClass() {
      return this.state === "free" || this.state === "numbered" ? "text-body-secondary" : "text-danger";
    },
    get offers() {
      return (this.state === "taken" || this.state === "fix") && this.suggestion !== "";
    },
  }));

  // Share the current page: the phone's share sheet, else copy the link.
  Alpine.data("shareButton", () => ({
    status: "",

    async share() {
      const { title, text, copied } = this.$el.closest("[data-title]").dataset;
      const url = window.location.href;
      if (navigator.share) {
        // Cancelling the share sheet rejects; that is not an error.
        await navigator.share({ title, text, url }).catch(() => {});
        return;
      }
      try {
        await navigator.clipboard.writeText(url);
        this.status = copied;
        setTimeout(() => (this.status = ""), 3000);
      } catch {
        // No clipboard (insecure origin, old browser): nothing to offer.
      }
    },
  }));

  // A tap on a public call button: counted with a beacon, which the browser
  // sends even as the phone app opens. The tel: link works the same without
  // JS, only uncounted. Attributes: data-call-url, data-call-kind, data-call-id,
  // and data-call-channel on a Zalo button ("zalo"; none is a call).
  Alpine.data("callBeacon", () => ({
    count() {
      const { callUrl, callKind, callId, callChannel } = this.$el.dataset;
      const target = { kind: callKind, id: Number(callId), channel: callChannel };
      const body = new Blob([JSON.stringify(target)], {
        type: "application/json",
      });
      if (navigator.sendBeacon) {
        navigator.sendBeacon(callUrl, body);
      } else {
        fetch(callUrl, { method: "POST", body, keepalive: true }).catch(() => {});
      }
    },
  }));
});
