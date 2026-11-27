// Alpine components for every page. This is the CSP build of Alpine: templates
// may only name a property or method registered here (x-on:click="toggle"),
// never write an expression (a convention test enforces it). Every component
// enhances markup that already works without JavaScript.
"use strict";

// The slug a suggestion endpoint gives `text` (`id`: the record edited):
// `{slug, taken}`, or null offline or signed out (the save still checks).
// Where the Markdown editor remembers that this browser writes side by side.
const EDITOR_MODE_KEY = "markdown-editor-mode";

async function askSlug(url, text, id) {
  const params = new URLSearchParams({ text });
  if (id) params.set("id", id);
  try {
    const response = await fetch(`${url}?${params}`, { headers: { Accept: "application/json" } });
    return response.ok ? await response.json() : null;
  } catch {
    return null;
  }
}

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

  // A record header's "more" menu (organisms/record_header): a `<details>`, so
  // it opens without JS; this adds closing on Escape, on a tap outside and
  // once an item is chosen.
  Alpine.data("recordMenu", () => topBarMenu("record-menu-opened"));

  // Turns an in-page section into a bottom sheet (a centred dialog on a wide
  // screen, by CSS). Without JS the section is part of the page and its
  // trigger is an anchor link to it. The server sets data-open="true" when
  // it re-shows a form with errors. The sheet takes its first state still;
  // only the visitor's show or hide animates (`animated`). A trigger may sit
  // anywhere on the page (a record header's button or menu): a plain link to
  // the sheet's id opens it, by the URL hash, and closing it clears the hash
  // so the same link opens it again.
  Alpine.data("dialog", () => ({
    modal: false,
    isOpen: false,
    animated: false,
    sheetId: "",

    init() {
      this.modal = true;
      this.isOpen = this.$el.dataset.open === "true";
      const own = Array.from(this.$el.querySelectorAll(".contact-sheet[id]")).find(
        (sheet) => sheet.closest("[x-data='dialog']") === this.$el,
      );
      this.sheetId = own ? own.id : "";
      const fromHash = () => {
        if (this.sheetId && window.location.hash === `#${this.sheetId}`) this.show();
      };
      window.addEventListener("hashchange", fromHash);
      if (this.isOpen) this.focusFirst();
      else fromHash();
    },
    show() {
      this.animated = true;
      this.isOpen = true;
      this.focusFirst();
    },
    hide() {
      this.animated = true;
      this.isOpen = false;
      if (this.sheetId && window.location.hash === `#${this.sheetId}`) {
        window.history.replaceState(null, "", window.location.pathname + window.location.search);
      }
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

  // The settings page's identity tab: its preview follows the form as it is
  // typed or a file chosen, before anything is saved. The root's data-*
  // carry what is saved and the defaults; a chosen file shows from a blob:
  // URL (img-src allows blob:). Without JS the preview is what is saved.
  // The features tab's preview of the menus (pages/admin/settings): each
  // feature's link shown while its place is ticked and it is not off, marked
  // while in preview; each bar's room counted (the wide bar's, the phone's),
  // its other boxes greyed once it is full (the server refuses one more
  // either way).
  Alpine.data("navPreview", () => ({
    topRoom: "",
    phoneRoom: "",

    init() {
      this.update();
    },
    update() {
      const form = this.$el.querySelector("form");
      const boxes = form.elements;
      for (const item of this.$el.querySelectorAll("[data-place-key]")) {
        const placed = boxes.namedItem(item.dataset.placeKey)?.checked;
        const stage = form.querySelector(
          `input[name="${CSS.escape(item.dataset.stageKey)}"]:checked`,
        )?.value;
        item.hidden = !placed || stage === "off";
        item.classList.toggle("is-preview", stage === "preview");
      }
      // An off feature's places are faded and said to count for nothing.
      for (const places of this.$el.querySelectorAll("[data-places-of]")) {
        const stage = form.querySelector(
          `input[name="${CSS.escape(places.dataset.placesOf)}"]:checked`,
        )?.value;
        places.classList.toggle("is-off", stage === "off");
      }
      // No more features on a bar than its room, the first kept, as the
      // frame draws it.
      for (const [place, max] of [
        ["top", Number(this.$el.dataset.topMax)],
        ["phone", Number(this.$el.dataset.phoneMax)],
      ]) {
        let taken = 0;
        for (const item of this.$el.querySelectorAll(`.nav-preview--${place} [data-place-key]`)) {
          if (item.hidden) continue;
          item.hidden = taken >= max;
          taken += 1;
        }
      }
      this.topRoom = this.room(form, "top", Number(this.$el.dataset.topMax));
      this.phoneRoom = this.room(form, "phone", Number(this.$el.dataset.phoneMax));
    },
    // One bar's room line, counted as the server counts it: a ticked box of a
    // feature that is not off. Once full, the unticked boxes of the shown
    // features are greyed; an off feature's never are (its tick takes no
    // room until it is back on, which the server then checks).
    room(form, place, max) {
      const isOff = (box) =>
        box.closest("[data-places-of]")?.classList.contains("is-off") ?? false;
      const bar = [...form.querySelectorAll(`[data-room="${place}"]`)];
      const used = bar.filter((box) => box.checked && !isOff(box)).length;
      for (const box of bar) box.disabled = !box.checked && !isOff(box) && used >= max;
      const text = used > max ? this.$el.dataset.roomOverText : this.$el.dataset.roomText;
      return text
        .replace("{used}", String(used))
        .replace("{max}", String(max));
    },
  }));

  Alpine.data("brandPreview", () => ({
    withoutScript: false,
    name: "",
    tagline: "",
    description: "",
    nameFits: true,
    logoChosen: "",
    iconChosen: "",

    init() {
      this.saved = { ...this.$el.dataset };
      this.read();
    },
    // The boxes as they stand; then whether the name fits the phone bar.
    read() {
      const boxes = this.$el.querySelector("form").elements;
      this.name = boxes.namedItem("brand.name")?.value ?? "";
      this.tagline = boxes.namedItem("brand.tagline")?.value ?? "";
      this.description = boxes.namedItem("brand.description")?.value ?? "";
      this.$nextTick(() => {
        const bar = this.$refs.barName;
        this.nameFits = !bar || bar.scrollWidth <= bar.clientWidth;
      });
    },
    update(event) {
      const box = event.target;
      if (box.type === "file") {
        const file = box.files?.[0];
        const url = file ? URL.createObjectURL(file) : "";
        if (box.name === "brand.logo") {
          if (this.logoChosen) URL.revokeObjectURL(this.logoChosen);
          this.logoChosen = url;
        } else if (box.name === "brand.icon") {
          if (this.iconChosen) URL.revokeObjectURL(this.iconChosen);
          this.iconChosen = url;
        }
      }
      this.read();
    },
    get inputClass() {
      return "visually-hidden";
    },
    get shownName() {
      return this.name.trim() || this.saved.defaultName;
    },
    get shownTagline() {
      return this.tagline.trim() || this.saved.defaultTagline;
    },
    get googleTitle() {
      return `${this.shownName} | ${this.shownTagline}`;
    },
    get googleDescription() {
      return this.description.trim() || this.saved.defaultDescription;
    },
    // The logo shown: the file chosen, else the one saved; "" the built-in.
    get logoUrl() {
      return this.logoChosen || this.saved.logo;
    },
    get logoCustom() {
      return Boolean(this.logoUrl);
    },
    get logoBuiltin() {
      return !this.logoCustom;
    },
    get logoTile() {
      return this.logoUrl || this.saved.builtinIcon;
    },
    get logoSaved() {
      return !this.logoChosen;
    },
    // The browser icon: its own, else the logo, else the built-in one.
    get iconUrl() {
      return this.iconChosen || this.saved.icon || this.logoUrl || this.saved.builtinIcon;
    },
    get iconSaved() {
      return !this.iconChosen;
    },
    // The home-screen icon: the large copies the phone is given.
    get homeUrl() {
      return (
        this.iconChosen ||
        this.saved.iconHome ||
        this.logoChosen ||
        this.saved.logoHome ||
        this.saved.builtinHome
      );
    },
  }));

  // A box's characters against its maxlength (`field::input` `counter`):
  // amber, with a short note, while under its advised length.
  Alpine.data("charCount", () => ({
    count: 0,
    max: 0,
    advised: 0,

    init() {
      const box = document.getElementById(this.$el.dataset.for);
      if (!box) return;
      this.max = box.maxLength;
      this.advised = Number(this.$el.dataset.advised || 0);
      const read = () => {
        this.count = [...box.value].length;
      };
      read();
      box.addEventListener("input", read);
    },
    get tooShort() {
      return this.advised > 0 && this.count > 0 && this.count < this.advised;
    },
    get label() {
      const count = `${this.count}/${this.max}`;
      return this.tooShort ? `${count} · ${this.$el.dataset.short}` : count;
    },
    get tone() {
      return this.tooShort ? "text-warning-emphasis" : "";
    },
  }));

  // A colour box (form_field.html color_select): one row of round swatches,
  // "no colour", the suggested colours, then "another colour" (the
  // browser's colour input, a rainbow until a colour of its own is chosen).
  // Without JS only that input shows, with a "no colour" tick; with JS the
  // tick hides but is still what the form sends. The swatches are painted
  // from their data-color through the CSSOM, which the CSP allows (a style
  // A category tile's icon in the category's own colour (category_item.html):
  // set through the CSSOM, which the CSP allows (a style attribute it does
  // not); the class goes on only with the colour, so without JS the tile
  // keeps its kind's.
  Alpine.data("tint", () => ({
    init() {
      this.$el.style.setProperty("--cat-tint", this.$el.dataset.color);
      this.$el.classList.add("cat-item__icon--tinted");
    },
  }));

  // attribute it does not).
  Alpine.data("colorSelect", () => ({
    value: "",
    none: false,
    custom: false,
    withoutScript: false,

    init() {
      this.value = this.$refs.picker.value;
      this.none = this.$refs.none.checked;
      for (const swatch of this.swatches()) {
        swatch.style.backgroundColor = swatch.dataset.color;
      }
      this.mark();
    },
    swatches() {
      return [...this.$el.querySelectorAll("[data-color]")];
    },
    set(color) {
      this.value = color.toLowerCase();
      this.$refs.picker.value = this.value;
      this.none = false;
      this.$refs.none.checked = false;
      this.mark();
    },
    picked() {
      this.set(this.$refs.picker.value);
    },
    choose(event) {
      this.set(event.currentTarget.dataset.color);
    },
    clear() {
      this.none = true;
      this.$refs.none.checked = true;
      this.mark();
    },
    toggled() {
      this.none = this.$refs.none.checked;
      this.mark();
    },
    // The chosen swatch says so (aria-pressed draws its ring); a colour no
    // swatch has is shown and ringed on "another colour".
    mark() {
      let preset = false;
      for (const swatch of this.swatches()) {
        const on = !this.none && swatch.dataset.color === this.value;
        preset = preset || on;
        swatch.setAttribute("aria-pressed", on ? "true" : "false");
      }
      this.$refs.noneSwatch.setAttribute("aria-pressed", this.none ? "true" : "false");
      this.custom = !this.none && !preset;
    },
    get pickTone() {
      return { "color-swatch--rainbow": !this.custom, "color-swatch--chosen": this.custom };
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

  // The admin list toolbar: any change of a filter sends it (its apply button,
  // in a <noscript>, is for visitors without JS), and its filters fold behind
  // a button, open from the start when one is on (data-open="true"). Without
  // JS the panel stays open and the button (x-cloak) stays hidden.
  Alpine.data("adminToolbar", () => ({
    open: false,

    init() {
      this.open = this.$root.dataset.open === "true";
    },
    submit() {
      this.$root.requestSubmit();
    },
    toggle() {
      this.open = !this.open;
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

  // The admin sidebar. Its groups are <details> (they fold without JS); this
  // remembers the folded ones in this browser, the current page's group
  // always open. The search box (x-cloak: only with JS) keeps the entries
  // whose name, group or keywords (data-keywords: the lists on their tabs)
  // hold every word typed, accents ignored, every group open meanwhile;
  // Enter opens the first one. The menu scrolls on its own, so it keeps its
  // place from page to page in this tab, then makes sure the current page's
  // link is in view. Storage may be blocked: the menu then starts unfolded
  // at the top, as without JS.
  const MENU_SCROLL_KEY = "admin-menu-scroll";
  const MENU_FOLDED_KEY = "admin-menu-folded";
  Alpine.data("adminMenu", () => ({
    words: [],
    folded: [],

    init() {
      const menu = this.$el;
      const current = menu.querySelector('[aria-current="page"]');
      const currentGroup = current?.closest("[data-group]")?.dataset.group;
      try {
        const stored = JSON.parse(localStorage.getItem(MENU_FOLDED_KEY));
        if (Array.isArray(stored)) this.folded = stored.filter((key) => key !== currentGroup);
      } catch {}
      // After the frame's bindings, so the phone drawer is already the
      // scrolling box and the folded groups are folded.
      this.$nextTick(() => {
        try {
          menu.scrollTop = Number(sessionStorage.getItem(MENU_SCROLL_KEY)) || 0;
        } catch {}
        if (!current) return;
        const box = menu.getBoundingClientRect();
        // The search box stays at the top, over the links.
        const top = box.top + this.$refs.searchBox.offsetHeight;
        const link = current.getBoundingClientRect();
        if (link.top < top) menu.scrollTop += link.top - top;
        else if (link.bottom > box.bottom) menu.scrollTop += link.bottom - box.bottom;
      });
    },
    remember() {
      try {
        sessionStorage.setItem(MENU_SCROLL_KEY, String(this.$el.scrollTop));
      } catch {}
    },
    links() {
      return [...this.$root.querySelectorAll(".ash-link")];
    },
    matches(link) {
      const group = link.closest("[data-group]")?.querySelector("summary")?.textContent || "";
      const text = fold(`${link.textContent} ${link.dataset.keywords || ""} ${group}`);
      return this.words.every((word) => text.includes(word));
    },
    search(event) {
      this.words = fold(event.target.value).split(/\s+/).filter(Boolean);
    },
    openFirst() {
      if (!this.searching) return;
      const first = this.links().find((link) => this.matches(link));
      if (first) window.location.assign(first.href);
    },
    // On a group: the visitor folded or unfolded it. While searching, the
    // groups are only held open, which is not remembered.
    groupToggled() {
      if (this.searching) return;
      const key = this.$el.dataset.group;
      const others = this.folded.filter((other) => other !== key);
      this.folded = this.$el.open ? others : [...others, key];
      try {
        localStorage.setItem(MENU_FOLDED_KEY, JSON.stringify(this.folded));
      } catch {}
    },
    get searching() {
      return this.words.length > 0;
    },
    get nothingFound() {
      return this.searching && !this.links().some((link) => this.matches(link));
    },
    // On a link.
    get linkShown() {
      return this.matches(this.$el);
    },
    // On a group.
    get groupOpen() {
      return this.searching || !this.folded.includes(this.$el.dataset.group);
    },
    get groupShown() {
      return [...this.$el.querySelectorAll(".ash-link")].some((link) => this.matches(link));
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

  // Lowercase, without Vietnamese accents or the d stroke: typed without
  // accents, a name still matches.
  const fold = (text) =>
    text.normalize("NFD").replace(/[\u0300-\u036f]/g, "").replace(/\u0111/gi, "d").toLowerCase();

  // Tom Select's line when nothing matches: the element's data-no-results.
  const noResults = (el) => () => {
    const note = document.createElement("div");
    note.className = "no-results";
    note.textContent = el.dataset.noResults;
    return note;
  };

  // A long select (provinces and their wards) that takes typing: Tom Select
  // over the select itself, which stays what the form sends. Every word typed
  // must appear in the option or its group (the province), accents ignored.
  // The words typed together ("ca mau") rank first, and the province of the
  // best match leads; equal matches keep the official order. A page without
  // the library keeps the plain select. data-no-results: the words when
  // nothing matches; data-allow-empty: the empty option ("every area") stays
  // a choice.
  //
  // An option with data-name (molecules/area_filter.html) is drawn as a
  // tree: the whole province with a map icon, each ward (data-ward) indented
  // on a guide line under it with a pin; chosen, the box shows its full name
  // (data-full).
  Alpine.data("searchableSelect", () => ({
    init() {
      if (typeof TomSelect === "undefined") return;
      const select = this.$el;
      const option = (data, escape) => {
        if (!data.name) return `<div>${escape(data.text)}</div>`;
        const [kind, icon] = data.ward
          ? ["area-choice--ward", "fa-location-dot"]
          : ["area-choice--whole", "fa-map"];
        return (
          `<div class="area-choice ${kind}"><i class="fa-solid ${icon} area-choice__icon" aria-hidden="true"></i>` +
          `<span>${escape(data.name)}</span></div>`
        );
      };
      const item = (data, escape) => `<div>${escape(data.full || data.text)}</div>`;
      new TomSelect(select, {
        // Every option, as a plain select scrolls through them all (the
        // provinces alphabetical, each ward by name: application::admin_units).
        maxOptions: null,
        allowEmptyOption: select.dataset.allowEmpty === "true",
        lockOptgroupOrder: false,
        sortField: [{ field: "$score", direction: "desc" }, { field: "$order" }],
        // Called on the Tom Select instance: an option's group is an id into
        // its optgroups, whose label is the province.
        score(query) {
          const words = fold(query).split(/\s+/).filter(Boolean);
          const phrase = words.join(" ");
          const groups = this.optgroups;
          return (data) => {
            const province = groups[data.optgroup]?.label || "";
            const text = fold(`${province} ${data.text}`);
            if (!words.every((w) => text.includes(w))) return 0;
            return text.includes(phrase) ? 2 : 1;
          };
        },
        render: { option, item, no_results: noResults(select) },
      });
    },
  }));

  // The icon picker (molecules/form_field.html icon_select): the text box
  // that holds the icon's name becomes a one-item Tom Select whose options
  // come from the server as the name is typed (data-url: GET
  // /api/admin/icons?q=, `{icons: [...]}` best first, Tom Select waits for a
  // pause in the typing). Each is drawn with its glyph beside its Font
  // Awesome name; the server's order is kept. The clear button (data-clear:
  // its title) leaves no icon. Without the library the box takes the name
  // typed, and the server checks it.
  Alpine.data("iconSelect", () => ({
    init() {
      if (typeof TomSelect === "undefined") return;
      const input = this.$el;
      const drawn = (data, escape) =>
        `<div class="icon-choice"><i class="fa-solid ${escape(data.value)} icon-choice__glyph" aria-hidden="true"></i>` +
        `<span>${escape(data.value.replace(/^fa-/, ""))}</span></div>`;
      new TomSelect(input, {
        maxItems: 1,
        create: false,
        persist: false,
        valueField: "value",
        labelField: "value",
        searchField: [],
        // The server has picked and ordered the matches: show them all, as sent.
        score: () => () => 1,
        sortField: [{ field: "$order" }],
        preload: "focus",
        shouldLoad: () => true,
        load(query, callback) {
          const url = new URL(input.dataset.url, window.location.origin);
          url.searchParams.set("q", query);
          fetch(url, { credentials: "same-origin", headers: { Accept: "application/json" } })
            .then((response) => (response.ok ? response.json() : { icons: [] }))
            .then((body) => {
              this.clearOptions();
              callback(body.icons.map((value) => ({ value })));
            })
            .catch(() => callback());
        },
        plugins: { clear_button: { title: input.dataset.clear } },
        render: { option: drawn, item: drawn, no_results: noResults(input) },
      });
    },
  }));

  // A ward found by typing its name (pages/job_new.html): a one-item Tom
  // Select whose choices come from the server as the name is typed (data-url:
  // GET /api/places/wards?q=, `{wards: [{value, label}]}` best first, from
  // data-min letters), so a farmer's page never carries every ward. The box
  // sits in a [data-js-only] block, hidden until this shows it: without JS
  // the area presets beside it are the choice.
  Alpine.data("wardSearch", () => ({
    init() {
      if (typeof TomSelect === "undefined") return;
      const select = this.$el;
      const min = Number(select.dataset.min) || 1;
      new TomSelect(select, {
        maxItems: 1,
        valueField: "value",
        labelField: "label",
        searchField: [],
        // The server has matched and ordered them: show them as sent.
        score: () => () => 1,
        sortField: [{ field: "$order" }],
        shouldLoad: (query) => query.trim().length >= min,
        load(query, callback) {
          const url = new URL(select.dataset.url, window.location.origin);
          url.searchParams.set("q", query);
          fetch(url, { credentials: "same-origin", headers: { Accept: "application/json" } })
            .then((response) => (response.ok ? response.json() : { wards: [] }))
            .then((body) => {
              this.clearOptions();
              callback(body.wards);
            })
            .catch(() => callback());
        },
        render: { no_results: noResults(select) },
      });
      const block = select.closest("[data-js-only]");
      if (block) block.hidden = false;
    },
  }));

  // The chosen option of a ward select drawn with points
  // (molecules/form_field.html area_select): the one carrying data-preset,
  // data-lat, data-lng, whether Tom Select draws the select or not.
  const wardOption = (select) => {
    const kept = select.selectedOptions[0];
    const option = kept && kept.dataset.preset ? kept : select.tomselect?.options[select.value]?.$option;
    return option && option.dataset.preset ? option : null;
  };

  // A location preset's create form: choosing a ward fills the name and
  // point, which stay editable. Without it the server fills the boxes left
  // blank.
  Alpine.data("wardFill", () => ({
    filled(event) {
      if (event.target.name !== "ward") return;
      const option = wardOption(event.target);
      if (!option) return;
      const form = this.$root;
      form.querySelector('[name="label"]').value = option.dataset.preset;
      form.querySelector('[name="lat"]').value = option.dataset.lat;
      form.querySelector('[name="lng"]').value = option.dataset.lng;
    },
  }));

  // The "add several areas" sheet (organisms/ward_checklist.html): typing
  // keeps the wards whose name holds every word, accents ignored; "tick
  // every one shown" ticks (or clears) the free wards still shown; the
  // button says how many are ticked and waits for one. Without it every ward
  // shows and the button always sends.
  Alpine.data("wardChecklist", () => ({
    ticked: 0,
    shown: 0,

    init() {
      this.count();
    },
    rows() {
      return [...this.$root.querySelectorAll("[data-ward]")];
    },
    free(row) {
      const box = row.querySelector("input");
      return box && !box.disabled ? box : null;
    },
    narrow(event) {
      const words = fold(event.target.value).split(/\s+/).filter(Boolean);
      for (const row of this.rows()) {
        const text = fold(row.dataset.ward);
        row.hidden = !words.every((word) => text.includes(word));
      }
      this.count();
    },
    tickShown(event) {
      for (const row of this.rows()) {
        const box = this.free(row);
        if (box && !row.hidden) box.checked = event.target.checked;
      }
      this.count();
    },
    count() {
      const rows = this.rows();
      const visible = rows.filter((row) => !row.hidden);
      this.shown = visible.length;
      this.ticked = rows.filter((row) => this.free(row)?.checked).length;
      const all = this.$refs.all;
      if (all) {
        const free = visible.map((row) => this.free(row)).filter(Boolean);
        all.checked = free.length > 0 && free.every((box) => box.checked);
        all.indeterminate = !all.checked && free.some((box) => box.checked);
      }
    },
    get noMatch() {
      return this.shown === 0;
    },
    get nothing() {
      return this.ticked === 0;
    },
    get submitText() {
      const data = this.$root.dataset;
      return this.ticked ? data.submitCount.replace("{n}", String(this.ticked)) : data.submit;
    },
  }));

  // Leaflet (static/vendor/leaflet-*), loaded once, the first time a map is
  // opened: a page that never shows one never downloads it.
  let leaflet = null;
  const loadLeaflet = (js, css) => {
    const load = (element) =>
      new Promise((resolve, reject) => {
        element.onload = resolve;
        element.onerror = reject;
        document.head.append(element);
      });
    if (!leaflet) {
      const link = Object.assign(document.createElement("link"), { rel: "stylesheet", href: css });
      const script = Object.assign(document.createElement("script"), { src: js });
      leaflet = Promise.all([load(link), load(script)]).then(() => {
        if (!window.L) throw new Error("leaflet did not load");
        return window.L;
      });
      // A failed download (weak signal) may be tried again.
      leaflet.catch(() => {
        leaflet = null;
      });
    }
    return leaflet;
  };

  // A point as people paste it, read as the server reads it
  // (web::pages::forms::typed_point, whose tests list the forms): "10.4048,
  // 106.1180", comma decimals ("10,4048 106,118"), or a Google Maps link (a
  // place's own pin `!3d…!4d…` before the map's centre `@…`, then `?q=`,
  // `?query=`, `?ll=`). A pair written longitude first is turned. Returns
  // [lat, lng], "" for an empty box, or null for text that is no point.
  const typedPoint = (raw) => {
    const text = raw.trim();
    if (!text) return "";
    const pairAt = (s) => {
      const m = s.match(/^(-?\d+(?:\.\d+)?)(?:,|%2C)[+ ]*(-?\d+(?:\.\d+)?)/i);
      return m ? [Number(m[1]), Number(m[2])] : null;
    };
    const fromLink = () => {
      const pin = text.match(/!3d(-?[\d.]+)!4d(-?[\d.]+)/);
      if (pin) return [Number(pin[1]), Number(pin[2])];
      for (const key of ["?q=", "&q=", "?query=", "&query=", "?ll=", "&ll=", "@"]) {
        const at = text.indexOf(key);
        const pair = at >= 0 ? pairAt(text.slice(at + key.length)) : null;
        if (pair) return pair;
      }
      return null;
    };
    const plain = () => {
      const decimal = (s) => {
        const t = s.replace(",", ".");
        return /^-?\d+(\.\d+)?$/.test(t) ? Number(t) : null;
      };
      const parts = text.split(/[ ;\t]+/).map((p) => p.replace(/,+$/, "")).filter(Boolean);
      const halves = parts.length === 1 ? parts[0].split(",") : parts;
      if (halves.length !== 2) return null;
      const [lat, lng] = halves.map(decimal);
      return lat !== null && lng !== null ? [lat, lng] : null;
    };
    const point = fromLink() || plain();
    if (!point) return null;
    const [a, b] = point;
    return Math.abs(a) > 90 && Math.abs(b) <= 90 ? [b, a] : [a, b];
  };
  // A Google Maps short link: it holds no point until opened.
  const SHORT_MAP_LINK = /(maps\.app\.goo\.gl|goo\.gl\/maps)/i;

  // How long the coordinate boxes stay still before their ward is asked.
  const PLACE_LOOKUP_PAUSE_MS = 400;
  // Decimals kept of a point: about 10 cm, more than any GPS gives.
  const PLACE_DECIMALS = 6;

  // A business's location box (organisms/business_fields.html). The ward
  // list, the GPS island, a paste and the map all end in the one point box
  // (`coords`: "10.404800, 106.118000"), which the server reads with
  // `typed_point`; the line on top says the place in words. Choosing a ward
  // puts the point at its centre (approximate). A GPS fill, a pasted point or
  // link, a tap on the map or a dragged pin put it there
  // and ask the server which ward that is (data-lookup), which is then
  // chosen in the list without moving the point again. The map is loaded
  // only when opened, its tiles from our own route (data-tiles). Without JS
  // the list and the boxes work alone: the server places a ward chosen
  // without a point at its centre. The map and its pin live outside Alpine's
  // reactive data: Leaflet breaks when its objects are proxied.
  Alpine.data("placePicker", () => {
    let map = null;
    let pin = null;
    let pause = 0;
    let asked = 0;
    return {
      init() {
        const open = this.$root.querySelector("[data-place-open]");
        if (open) open.hidden = false;
      },
      box(name) {
        return this.$root.querySelector(`[name="${name}"]`);
      },
      point() {
        const point = typedPoint(this.box("coords").value);
        return Array.isArray(point) ? point : null;
      },
      setPoint([lat, lng]) {
        this.box("coords").value = `${lat.toFixed(PLACE_DECIMALS)}, ${lng.toFixed(PLACE_DECIMALS)}`;
      },
      say(label, note = "", none = false) {
        const status = this.$root.querySelector("[data-place-status]");
        status.querySelector("[data-place-label]").textContent = label;
        status.querySelector("[data-place-note]").textContent = note;
        status.classList.toggle("place-status--none", none);
      },
      // The ward in the list, without its change moving the point.
      pickWard(value) {
        const select = this.box("ward");
        if (select.tomselect) select.tomselect.setValue(value, true);
        else select.value = value;
      },
      // A ward chosen in the list: its centre.
      chosen(event) {
        if (event.target.name !== "ward") return;
        const option = wardOption(event.target);
        if (!option) return;
        const point = [parseFloat(option.dataset.lat), parseFloat(option.dataset.lng)];
        this.setPoint(point);
        const province = option.closest("optgroup")?.label;
        const name = option.textContent.trim();
        this.say(province ? `${name}, ${province}` : name, this.$root.dataset.textApproximate);
        this.movePin(point, Number(this.$root.dataset.wardZoom));
      },
      // The point box typed or pasted in, or the GPS island's two hidden
      // boxes filled (it fills one, then the other: the pause covers both).
      typed(event) {
        const gps = "placeGps" in event.target.dataset;
        if (!gps && event.target.name !== "coords") return;
        clearTimeout(pause);
        pause = setTimeout(() => {
          const data = this.$root.dataset;
          if (gps) {
            const [lat, lng] = [...this.$root.querySelectorAll("[data-place-gps]")].map((b) => Number(b.value));
            if (!Number.isFinite(lat) || !Number.isFinite(lng)) return;
            this.setPoint([lat, lng]);
          }
          const typed = typedPoint(this.box("coords").value);
          if (typed === null) {
            const short = SHORT_MAP_LINK.test(this.box("coords").value);
            this.say(short ? data.textShortLink : data.textUnreadable, "", true);
            return;
          }
          const point = typed || null;
          if (point) this.movePin(point, Number(data.pointZoom));
          this.lookUp(point);
        }, PLACE_LOOKUP_PAUSE_MS);
      },
      // Which ward a point lies in, said on top and chosen in the list.
      lookUp(point) {
        const data = this.$root.dataset;
        if (!point) {
          this.say(data.textNone, "", true);
          return;
        }
        this.say(data.textLooking);
        const mine = ++asked;
        const url = new URL(data.lookup, window.location.origin);
        url.searchParams.set("lat", point[0].toFixed(PLACE_DECIMALS));
        url.searchParams.set("lng", point[1].toFixed(PLACE_DECIMALS));
        fetch(url, { credentials: "same-origin", headers: { Accept: "application/json" } })
          .then((response) => (response.ok ? response.json() : Promise.reject(response.status)))
          .then((place) => {
            if (mine !== asked) return;
            if (!place.area) {
              this.say(data.textOutside, "", true);
              this.pickWard("");
              return;
            }
            this.say(place.label, data.textExact);
            this.pickWard(place.area);
          })
          .catch(() => {
            // The point stays in the boxes and is saved; only its name is unknown.
            if (mine === asked) this.say(`${point[0].toFixed(5)}, ${point[1].toFixed(5)}`, data.textExact);
          });
      },
      openMap() {
        const data = this.$root.dataset;
        const holder = this.$root.querySelector("[data-place-map]");
        const hint = this.$root.querySelector("[data-place-map-hint]");
        const open = this.$root.querySelector("[data-place-open]");
        holder.hidden = false;
        hint.hidden = false;
        open.hidden = true;
        loadLeaflet(data.leafletJs, data.leafletCss)
          .then((L) => this.drawMap(L, holder))
          .catch(() => {
            holder.hidden = true;
            hint.textContent = data.textFailed;
            open.hidden = false;
          });
      },
      drawMap(L, holder) {
        if (map) return;
        const data = this.$root.dataset;
        const [min, max] = [Number(data.zoomMin), Number(data.zoomMax)];
        const point = this.point();
        map = L.map(holder, { minZoom: min, maxZoom: max });
        map.attributionControl.setPrefix(false);
        // Leaflet takes the credit line as markup: build the link, never
        // concatenate the words into HTML.
        const credit = Object.assign(document.createElement("a"), {
          href: data.attributionHref,
          target: "_blank",
          rel: "noopener",
          textContent: data.attribution,
        });
        L.tileLayer(data.tiles, { minZoom: min, maxZoom: max, attribution: credit.outerHTML }).addTo(map);
        if (point) {
          map.setView(point, Number(data.pointZoom));
          this.placePin(L, point);
        } else {
          map.setView([Number(data.startLat), Number(data.startLng)], Number(data.startZoom));
        }
        map.on("click", (event) => {
          this.placePin(L, event.latlng);
          this.dropped(event.latlng);
        });
        // Shown just now: Leaflet measures its box again.
        setTimeout(() => map.invalidateSize());
      },
      // The pin, made on the first point (the map opens without one when
      // nothing is placed yet), dragged afterwards.
      placePin(L, latlng) {
        if (pin) {
          pin.setLatLng(latlng);
          return;
        }
        const label = this.$root.dataset.pinLabel;
        const icon = L.divIcon({
          className: "place-pin",
          html: '<i class="fa-solid fa-location-dot" aria-hidden="true"></i>',
          iconSize: [36, 36],
          iconAnchor: [18, 34],
        });
        pin = L.marker(latlng, { draggable: true, icon, title: label, alt: label }).addTo(map);
        pin.on("dragend", () => this.dropped(pin.getLatLng()));
      },
      dropped(latlng) {
        const point = [latlng.lat, latlng.lng];
        this.setPoint(point);
        this.lookUp(point);
      },
      // The map follows a point set elsewhere (the list, the GPS, typing).
      movePin(point, zoom) {
        if (!map) return;
        this.placePin(window.L, point);
        map.setView(point, zoom);
      },
    };
  });

  // The category form's name and slug boxes. While the slug box is blank (or
  // was filled from here), typing a name fills it with the server's free slug
  // for that name; typing a slug checks it against the other categories and
  // offers a free one. Emptying the box hands it back to the name. Without JS
  // a blank slug is made on save, and the server checks every save anyway.
  // Attributes: data-url (the suggestion endpoint), data-id (the record
  // being edited), data-{free,numbered,taken,fix}-text (the notes), and
  // data-source / data-target: the boxes' names when they are not `name`
  // and `slug` (an information page's `title` and `code`).
  Alpine.data("slugField", () => ({
    // "", "free", "numbered" (filled, with a number added), "taken", "fix".
    state: "",
    suggestion: "",
    auto: false,
    timer: null,
    asked: 0,

    init() {
      this.source = this.$root.dataset.source || "name";
      this.target = this.$root.dataset.target || "slug";
      this.auto = this.box(this.target).value.trim() === "";
    },
    box(name) {
      return this.$root.querySelector(`[name="${name}"]`);
    },
    // Input events of both boxes bubble here.
    changed(event) {
      const { name, value } = event.target;
      if (name === this.target) {
        this.auto = value.trim() === "";
        if (this.auto) this.ask(this.box(this.source).value, "name");
        else this.ask(value, "slug");
      } else if (name === this.source && this.auto) {
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
      const answer = await askSlug(url, text, id);
      if (!answer) return;
      // A later keystroke has asked again; its answer wins.
      if (ticket !== this.asked) return;
      this.suggestion = answer.slug;
      if (from === "name") {
        if (!this.auto) return;
        this.box(this.target).value = answer.slug;
        this.state = !answer.slug ? "" : answer.taken ? "numbered" : "free";
      } else if (answer.slug === text) {
        this.state = "free";
      } else {
        this.state = answer.taken ? "taken" : "fix";
      }
    },
    apply() {
      this.box(this.target).value = this.suggestion;
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

  // An article's address as its title is typed: `data-prefix` (the address
  // up to its id, or a placeholder before the article exists) and the slug
  // the server makes of the title (`data-url`). Without JS the address is
  // shown once saved.
  Alpine.data("addressPreview", () => ({
    slug: "",
    timer: null,
    asked: 0,

    init() {
      this.slug = this.$root.dataset.slug || "";
    },
    changed(event) {
      if (event.target.name !== "title") return;
      clearTimeout(this.timer);
      const text = event.target.value.trim();
      this.timer = setTimeout(() => this.suggest(text), 300);
    },
    async suggest(text) {
      const ticket = ++this.asked;
      const answer = await askSlug(this.$root.dataset.url, text);
      if (answer && ticket === this.asked) this.slug = answer.slug;
    },
    get address() {
      return `${this.$root.dataset.prefix}${this.slug}`;
    },
  }));

  // The Markdown editor (organisms/markdown_editor.html) over a plain
  // textarea that works alone: a toolbar (bold, italic, headings, lists,
  // quote, link, rule) and its shortcuts (Ctrl+B, Ctrl+I, Ctrl+K), the
  // characters used, and a "preview" tab the server renders (`data-url`, the
  // one renderer the page uses), beside the text on a wide screen. The
  // words a button inserts come from the template (`data-sample-*`): no
  // words in code. Without JS the form's own preview button does the same.
  Alpine.data("markdownEditor", () => ({
    // "write" (the default), "preview", or "split": the preview beside the
    // text, chosen on a wide screen and remembered by this browser.
    mode: "write",
    html: "",
    status: "",
    count: 0,
    timer: null,
    asked: 0,

    init() {
      this.area = this.$refs.area;
      this.count = this.area.value.length;
      let remembered = "";
      try {
        remembered = window.localStorage.getItem(EDITOR_MODE_KEY) || "";
      } catch {
        // Storage refused (a private window): the default is fine.
      }
      if (remembered === "split" && window.matchMedia("(min-width: 992px)").matches) {
        this.mode = "split";
        this.render();
      }
    },
    remember() {
      try {
        window.localStorage.setItem(EDITOR_MODE_KEY, this.mode === "split" ? "split" : "");
      } catch {
        // Not kept: the next page opens on the default.
      }
    },
    get writeTabClass() {
      return this.mode === "write" ? "is-current" : "";
    },
    get previewTabClass() {
      return this.mode === "preview" ? "is-current" : "";
    },
    get splitTabClass() {
      return this.mode === "split" ? "is-current" : "";
    },
    get writing() {
      return this.mode === "write";
    },
    get previewing() {
      return this.mode === "preview";
    },
    get splitting() {
      return this.mode === "split";
    },
    // Which panes show (app.css): the text, the preview, or both.
    get paneClass() {
      return `shows-${this.mode}`;
    },
    get countLabel() {
      return `${this.count}/${this.area ? this.area.maxLength : ""}`;
    },
    get loading() {
      return this.status === "loading";
    },
    get failed() {
      return this.status === "failed";
    },
    showWrite() {
      this.mode = "write";
      this.remember();
      this.$nextTick(() => this.area.focus());
    },
    showPreview() {
      this.mode = "preview";
      this.remember();
      this.render();
    },
    showSplit() {
      this.mode = "split";
      this.remember();
      this.render();
    },
    typed() {
      this.count = this.area.value.length;
      // Side by side, the preview follows the text.
      if (this.mode === "split") {
        clearTimeout(this.timer);
        this.timer = setTimeout(() => this.render(), 600);
      }
    },
    async render() {
      const ticket = ++this.asked;
      this.status = "loading";
      try {
        const response = await fetch(this.$root.dataset.url, {
          method: "POST",
          headers: { "Content-Type": "application/json", Accept: "application/json" },
          body: JSON.stringify({ text: this.area.value }),
        });
        if (!response.ok) throw new Error(String(response.status));
        const answer = await response.json();
        if (ticket !== this.asked) return;
        // The server's renderer writes no markup of the author's.
        this.html = answer.html;
        this.status = "";
      } catch {
        if (ticket === this.asked) this.status = "failed";
      }
    },
    // A toolbar button: its `data-md` names what it does.
    format(event) {
      this.apply(event.currentTarget.dataset.md);
    },
    keys(event) {
      if (!(event.ctrlKey || event.metaKey)) return;
      const kind = { b: "bold", i: "italic", k: "link" }[event.key.toLowerCase()];
      if (!kind) return;
      event.preventDefault();
      this.apply(kind);
    },
    apply(kind) {
      const area = this.area;
      const data = this.$root.dataset;
      const start = area.selectionStart;
      const end = area.selectionEnd;
      const chosen = area.value.slice(start, end);
      const wrap = (mark, sample) => {
        const text = chosen || sample;
        this.replace(start, end, `${mark}${text}${mark}`, start + mark.length, start + mark.length + text.length);
      };
      const lines = (prefix) => {
        // Whole lines: from the start of the first one chosen.
        const from = area.value.lastIndexOf("\n", start - 1) + 1;
        const block = area.value.slice(from, end) || data.sampleLine;
        const out = block
          .split("\n")
          .map((line, i) => (prefix === "1. " ? `${i + 1}. ${line}` : `${prefix}${line}`))
          .join("\n");
        this.replace(from, end, out, from, from + out.length);
      };
      switch (kind) {
        case "bold":
          return wrap("**", data.sampleBold);
        case "italic":
          return wrap("_", data.sampleItalic);
        case "h2":
          return lines("## ");
        case "h3":
          return lines("### ");
        case "ul":
          return lines("- ");
        case "ol":
          return lines("1. ");
        case "quote":
          return lines("> ");
        case "link": {
          const text = chosen || data.sampleLink;
          const out = `[${text}](https://)`;
          // The address chosen, ready to be typed over.
          const at = start + text.length + 3;
          return this.replace(start, end, out, at, at + "https://".length);
        }
        case "hr":
          return this.replace(start, end, `${chosen}\n\n---\n\n`, end + 7, end + 7);
        default:
          return undefined;
      }
    },
    // Puts `text` over [start, end) keeping the browser's undo where it can,
    // then selects [from, to).
    replace(start, end, text, from, to) {
      const area = this.area;
      area.focus();
      area.setSelectionRange(start, end);
      const kept = document.execCommand && document.execCommand("insertText", false, text);
      if (!kept) area.setRangeText(text, start, end, "end");
      area.setSelectionRange(from, to);
      area.dispatchEvent(new Event("input", { bubbles: true }));
    },
  }));

  // Share the current page: the phone's share sheet, else copy the link.
  Alpine.data("shareButton", () => ({
    status: "",

    async share() {
      const { title, text, copied, url: given } = this.$el.closest("[data-title]").dataset;
      // `data-url` shares another page (a review link); none is this one.
      const url = given ? new URL(given, window.location.href).href : window.location.href;
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
