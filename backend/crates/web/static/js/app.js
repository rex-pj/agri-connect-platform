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
  Alpine.data("brandPreview", () => ({
    withoutScript: false,
    name: "",
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
    get googleTitle() {
      return `${this.shownName} — ${this.saved.homeTitle}`;
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

  // A location preset's create form: choosing a ward (an option carrying
  // data-preset, data-lat, data-lng) fills the name and point, which stay
  // editable. Without it the server fills the boxes left blank.
  Alpine.data("wardFill", () => ({
    filled(event) {
      if (event.target.name !== "ward") return;
      const select = event.target;
      const kept = select.selectedOptions[0];
      const option = kept && kept.dataset.preset ? kept : select.tomselect?.options[select.value]?.$option;
      if (!option || !option.dataset.preset) return;
      const form = this.$root;
      form.querySelector('[name="label"]').value = option.dataset.preset;
      form.querySelector('[name="lat"]').value = option.dataset.lat;
      form.querySelector('[name="lng"]').value = option.dataset.lng;
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
