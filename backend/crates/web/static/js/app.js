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
});
