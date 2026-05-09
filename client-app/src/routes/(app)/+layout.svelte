<script lang="ts">
  import { afterNavigate, goto } from "$app/navigation";
  import { enhance } from "$app/forms";
  import type { LayoutProps } from "./$types";
  import BottomNav from "$lib/components/organisms/BottomNav.svelte";
  import Button from "$lib/components/atoms/Button.svelte";
  import { ROUTES } from "$lib/routes";
  const { children, data }: LayoutProps = $props();

  let searchQuery = $state("");
  let searchOpen = $state(false);
  let searchInput = $state<HTMLInputElement | null>(null);
  let profileOpen = $state(false);

  afterNavigate(() => {
    document.documentElement.style.overflow = "";
    document.body.style.overflow = "";
    profileOpen = false;
  });

  function toggleSearch() {
    searchOpen = !searchOpen;
    if (searchOpen) {
      profileOpen = false;
      setTimeout(() => searchInput?.focus(), 30);
    } else {
      searchQuery = "";
    }
  }

  function toggleProfile() {
    profileOpen = !profileOpen;
    if (profileOpen) searchOpen = false;
  }

  function submitSearch(e: SubmitEvent) {
    e.preventDefault();
    const q = searchQuery.trim();
    if (!q) return;
    goto(`${ROUTES.SERVICES}?service=${encodeURIComponent(q)}`, {
      replaceState: false,
    });
    searchQuery = "";
    searchOpen = false;
  }
</script>

<nav class="app-nav">
  <div class="app-nav__top">
    <Button href={ROUTES.HOME} class="app-nav__brand">
      <span class="app-nav__logo">
        <i class="fa-solid fa-seedling"></i>
      </span>
      Agri-Connect
    </Button>

    <div class="app-nav__actions">
      <Button
        type="button"
        class="app-nav__search-toggle{searchOpen ? ' active' : ''}"
        onclick={toggleSearch}
        aria-label={searchOpen ? "Đóng tìm kiếm" : "Tìm kiếm"}
      >
        <i class="fa-solid {searchOpen ? 'fa-xmark' : 'fa-magnifying-glass'}"
        ></i>
      </Button>

      {#if data.isLoggedIn}
        <div class="app-nav__profile">
          <Button
            type="button"
            class="app-nav__login{profileOpen ? ' active' : ''}"
            onclick={toggleProfile}
            aria-expanded={profileOpen}
            aria-haspopup="true"
          >
            <i class="fa-solid fa-circle-user"></i>
            Tài khoản
            <i
              class="fa-solid fa-chevron-down app-nav__chevron{profileOpen
                ? ' rotated'
                : ''}"
            ></i>
          </Button>

          {#if profileOpen}
            <div class="app-nav__dropdown" role="menu">
              <Button
                href={ROUTES.ACCOUNT}
                class="app-nav__dropdown-item"
                role="menuitem"
              >
                <i class="fa-solid fa-id-card"></i>
                Hồ sơ
              </Button>
              <div class="app-nav__dropdown-divider"></div>
              <form method="POST" action="/account?/logout" use:enhance>
                <Button
                  type="submit"
                  class="app-nav__dropdown-item app-nav__dropdown-item--danger"
                  role="menuitem"
                >
                  <i class="fa-solid fa-right-from-bracket"></i>
                  Đăng xuất
                </Button>
              </form>
            </div>
            <button
              class="app-nav__backdrop"
              onclick={() => (profileOpen = false)}
              aria-hidden="true"
            ></button>
          {/if}
        </div>
      {:else}
        <div class="app-nav__profile">
          <Button
            type="button"
            class="app-nav__login{profileOpen ? ' active' : ''}"
            onclick={toggleProfile}
            aria-expanded={profileOpen}
            aria-haspopup="true"
            aria-label="Tài khoản"
          >
            <i class="fa-solid fa-circle-user"></i>
            <i
              class="fa-solid fa-chevron-down app-nav__chevron{profileOpen
                ? ' rotated'
                : ''}"
            ></i>
          </Button>

          {#if profileOpen}
            <div class="app-nav__dropdown" role="menu">
              <Button
                href={ROUTES.LOGIN}
                class="app-nav__dropdown-item"
                role="menuitem"
              >
                <i class="fa-solid fa-right-to-bracket"></i>
                Đăng nhập
              </Button>
              <div class="app-nav__dropdown-divider"></div>
              <Button
                href={ROUTES.REGISTER}
                class="app-nav__dropdown-item"
                role="menuitem"
              >
                <i class="fa-solid fa-user-plus"></i>
                Đăng ký
              </Button>
            </div>
            <button
              class="app-nav__backdrop"
              onclick={() => (profileOpen = false)}
              aria-hidden="true"
            ></button>
          {/if}
        </div>
      {/if}
    </div>
  </div>

  <div
    class="app-nav__search-row"
    class:open={searchOpen}
    aria-hidden={!searchOpen}
  >
    <div class="app-nav__search-inner">
      <form class="app-nav__search" onsubmit={submitSearch}>
        <i class="fa-solid fa-magnifying-glass app-nav__search-icon"></i>
        <input
          bind:this={searchInput}
          class="app-nav__search-input"
          type="search"
          placeholder="Tìm dịch vụ, nông sản..."
          bind:value={searchQuery}
          aria-label="Tìm kiếm"
          tabindex={searchOpen ? 0 : -1}
        />
        {#if searchQuery}
          <Button
            type="submit"
            class="app-nav__search-btn"
            aria-label="Tìm kiếm"
          >
            <i class="fa-solid fa-arrow-right"></i>
          </Button>
        {/if}
      </form>
    </div>
  </div>
</nav>

<main class="app-main" style="padding-top: {searchOpen ? '98px' : '52px'}">
  {@render children()}
</main>

<BottomNav />

<style>
  :global(html) {
    scrollbar-gutter: stable;
  }

  .app-nav {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    z-index: 1030;
    display: flex;
    flex-direction: column;
    padding: 10px 16px;
    background: linear-gradient(
      135deg,
      var(--p900, #022c22) 0%,
      var(--p800, #064e3b) 55%,
      var(--p700, #065f46) 100%
    );
    box-shadow: 0 2px 16px rgba(2, 44, 34, 0.35);
  }

  .app-nav__top {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  :global(.app-nav__brand) {
    display: flex;
    align-items: center;
    gap: 8px;
    color: #fff;
    text-decoration: none;
    font-size: 1.1rem;
    font-weight: 700;
    letter-spacing: -0.02em;
  }

  .app-nav__logo {
    width: 32px;
    height: 32px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.12);
    backdrop-filter: blur(4px);
  }

  .app-nav__actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  :global(.app-nav__search-toggle) {
    width: 36px;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.1);
    border: 1px solid rgba(255, 255, 255, 0.25);
    border-radius: 20px;
    color: #fff;
    font-size: 0.88rem;
    cursor: pointer;
    transition: background 0.15s;
    -webkit-tap-highlight-color: transparent;
  }

  :global(.app-nav__search-toggle.active),
  :global(.app-nav__search-toggle:hover) {
    background: rgba(255, 255, 255, 0.2);
  }

  :global(.app-nav__login) {
    display: flex;
    align-items: center;
    gap: 6px;
    color: #fff;
    font-size: 0.82rem;
    font-weight: 600;
    text-decoration: none;
    padding: 5px 12px;
    border-radius: 20px;
    border: 1px solid rgba(255, 255, 255, 0.3);
    background: rgba(255, 255, 255, 0.1);
    transition: background 0.2s ease;
    min-height: 32px;
    white-space: nowrap;
  }

  :global(.app-nav__login:hover) {
    background: rgba(255, 255, 255, 0.2);
    color: #fff;
  }

  /* Collapsible search row */
  .app-nav__search-row {
    display: grid;
    grid-template-rows: 0fr;
    margin-top: 0;
    transition:
      grid-template-rows 0.22s ease,
      margin-top 0.22s ease;
  }

  .app-nav__search-row.open {
    grid-template-rows: 1fr;
    margin-top: 8px;
  }

  .app-nav__search-inner {
    min-height: 0;
    overflow: hidden;
  }

  .app-nav__search {
    display: flex;
    align-items: center;
    gap: 10px;
    background: rgba(255, 255, 255, 0.12);
    border: 1px solid rgba(255, 255, 255, 0.2);
    border-radius: 100px;
    padding: 0 14px;
    min-height: 38px;
    transition:
      background 0.15s,
      border-color 0.15s;
  }

  .app-nav__search:focus-within {
    background: rgba(255, 255, 255, 0.18);
    border-color: rgba(255, 255, 255, 0.45);
  }

  .app-nav__search-icon {
    color: rgba(255, 255, 255, 0.65);
    font-size: 0.85rem;
    flex-shrink: 0;
  }

  .app-nav__search-input {
    flex: 1;
    border: none;
    background: transparent;
    font-size: 0.9rem;
    color: #fff;
    outline: none;
    min-width: 0;
  }

  .app-nav__search-input::placeholder {
    color: rgba(255, 255, 255, 0.5);
  }

  :global(.app-nav__search-btn) {
    flex-shrink: 0;
    width: 26px;
    height: 26px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.9);
    color: var(--p800, #064e3b);
    border: none;
    border-radius: 50%;
    font-size: 0.75rem;
    cursor: pointer;
    transition:
      background 0.15s,
      transform 0.1s;
  }

  :global(.app-nav__search-btn:active) {
    transform: scale(0.92);
  }

  .app-main {
    transition: padding-top 0.22s ease;
    padding-bottom: calc(var(--nav-h, 64px) + 12px);
  }

  .app-nav__profile {
    position: relative;
  }

  :global(button.app-nav__login) {
    cursor: pointer;
  }

  .app-nav__chevron {
    font-size: 0.65rem;
    transition: transform 0.18s ease;
    margin-left: 2px;
  }

  .app-nav__chevron.rotated {
    transform: rotate(180deg);
  }

  .app-nav__dropdown {
    position: absolute;
    top: calc(100% + 8px);
    right: 0;
    min-width: 160px;
    background: #fff;
    border-radius: 10px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.18);
    overflow: hidden;
    z-index: 1040;
  }

  :global(.app-nav__dropdown-item) {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 12px 16px;
    font-size: 0.88rem;
    font-weight: 500;
    color: #1a2e24;
    text-decoration: none;
    background: none;
    border: none;
    cursor: pointer;
    text-align: left;
    transition: background 0.15s;
    -webkit-tap-highlight-color: transparent;
  }

  :global(.app-nav__dropdown-item:hover) {
    background: #f0fdf4;
  }

  :global(.app-nav__dropdown-item--danger) {
    color: #dc2626;
  }

  :global(.app-nav__dropdown-item--danger:hover) {
    background: #fff1f1;
  }

  .app-nav__dropdown-divider {
    height: 1px;
    background: #e7ede9;
    margin: 0;
  }

  .app-nav__backdrop {
    position: fixed;
    inset: 0;
    z-index: 1039;
    background: transparent;
    border: none;
    cursor: default;
  }
</style>
