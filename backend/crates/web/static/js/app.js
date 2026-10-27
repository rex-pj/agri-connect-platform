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

  // Account dropdown. Without JS the button is a link to /account.
  Alpine.data("profileMenu", () => ({
    open: false,

    toggle() {
      this.open = !this.open;
      if (this.open) {
        // Bubbles to the top bar, which closes the search row.
        this.$dispatch("profile-menu-opened");
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
  }));

  // Turns an in-page section into a bottom sheet. Without JS the section is
  // part of the page and its trigger is an anchor link to it. The server sets
  // data-open="true" when it re-shows a form with errors.
  Alpine.data("dialog", () => ({
    modal: false,
    isOpen: false,

    init() {
      this.modal = true;
      this.isOpen = this.$el.dataset.open === "true";
      if (this.isOpen) this.focusFirst();
    },
    show() {
      this.isOpen = true;
      this.focusFirst();
    },
    hide() {
      this.isOpen = false;
    },
    focusFirst() {
      this.$nextTick(() => this.$refs.first?.focus());
    },
    get sheetClass() {
      return this.isOpen ? "is-modal is-open" : "is-modal";
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
  // JS, only uncounted. Attributes: data-call-url, data-call-kind, data-call-id.
  Alpine.data("callBeacon", () => ({
    count() {
      const { callUrl, callKind, callId } = this.$el.dataset;
      const body = new Blob([JSON.stringify({ kind: callKind, id: Number(callId) })], {
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
