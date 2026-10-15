<script lang="ts">
  import { page } from "$app/state";
  import type { Snippet } from "svelte";
  import Button from "$lib/components/atoms/Button.svelte";
  import { ROUTES } from "$lib/routes";

  const {
    children,
    username = "Admin",
  }: {
    children: Snippet;
    username?: string;
  } = $props();

  interface MenuItem {
    name: string;
    href: string;
    icon: string;
  }

  interface MenuGroup {
    name: string;
    icon: string;
    items: MenuItem[];
  }

  type MenuEntry = MenuItem | MenuGroup;

  function isGroup(e: MenuEntry): e is MenuGroup {
    return "items" in e;
  }

  const menus: MenuEntry[] = [
    {
      name: "Dashboard",
      href: ROUTES.ADMIN.DASHBOARD,
      icon: "fa-solid fa-gauge",
    },
    {
      name: "Nội dung",
      icon: "fa-solid fa-folder-open",
      items: [
        {
          name: "Nhà cung cấp",
          href: ROUTES.ADMIN.EXPLORE,
          icon: "fa-solid fa-store",
        },
        {
          name: "Sản phẩm",
          href: ROUTES.ADMIN.PRODUCTS,
          icon: "fa-solid fa-box-open",
        },
        {
          name: "Dịch vụ",
          href: ROUTES.ADMIN.SERVICES,
          icon: "fa-solid fa-screwdriver-wrench",
        },
        {
          name: "Danh mục",
          href: ROUTES.ADMIN.CATEGORIES,
          icon: "fa-solid fa-tags",
        },
        {
          name: "Yêu cầu",
          href: ROUTES.ADMIN.INQUIRIES,
          icon: "fa-solid fa-envelope-open-text",
        },
        {
          name: "Đánh giá",
          href: ROUTES.ADMIN.REVIEWS,
          icon: "fa-solid fa-star",
        },
        {
          name: "Khu vực",
          href: ROUTES.ADMIN.LOCATIONS,
          icon: "fa-solid fa-map-location-dot",
        },
      ],
    },
    {
      name: "Hệ thống",
      icon: "fa-solid fa-gears",
      items: [
        {
          name: "Người dùng",
          href: ROUTES.ADMIN.USERS,
          icon: "fa-solid fa-users",
        },
        {
          name: "Vai trò",
          href: ROUTES.ADMIN.ROLES,
          icon: "fa-solid fa-shield-halved",
        },
        {
          name: "Quyền hạn",
          href: ROUTES.ADMIN.PERMISSIONS,
          icon: "fa-solid fa-key",
        },
      ],
    },
  ];

  let currentPath = $derived(page.url.pathname);

  function isActive(href: string): boolean {
    return currentPath === href || currentPath.startsWith(href + "/");
  }

  function groupHasActive(group: MenuGroup): boolean {
    return group.items.some((i) => isActive(i.href));
  }
</script>

<div class="ash-layout">
  <!-- ── Top header ──────────────────────────────────────────────────── -->
  <header class="ash-header">
    <a href={ROUTES.ADMIN.DASHBOARD} class="ash-logo-link d-flex align-items-center gap-3">
      <i class="fa-solid fa-seedling ash-logo-icon"></i>
      <span class="ash-logo-text">Agri Admin</span>
    </a>
    <div class="d-flex align-items-center gap-2">
      <span class="ash-welcome d-none d-md-inline">
        <i class="fa-regular fa-circle-user me-1"></i>{username}
      </span>
      <Button
        href={ROUTES.HOME}
        class="btn btn-sm btn-outline-light d-inline-flex align-items-center"
      >
        <i class="fa-solid fa-house me-1"></i>
        <span class="d-none d-sm-inline">Trang chủ</span>
      </Button>
      <form method="POST" action={ROUTES.ADMIN.LOGOUT} class="d-flex">
        <Button
          type="submit"
          class="btn btn-sm btn-outline-danger d-inline-flex align-items-center"
        >
          <i class="fa-solid fa-right-from-bracket me-1"></i>
          <span class="d-none d-sm-inline">Đăng xuất</span>
        </Button>
      </form>
    </div>
  </header>

  <!-- ── Sidebar ─────────────────────────────────────────────────────── -->
  <nav class="ash-sidebar">
    {#each menus as entry}
      {#if isGroup(entry)}
        <div class="ash-group">
          <div
            class="ash-group-label {groupHasActive(entry)
              ? ' ash-group-label--active'
              : ''}"
          >
            <i class="{entry.icon} ash-icon"></i>
            <span class="flex-grow-1">{entry.name}</span>
          </div>
          <div class="ash-group-items">
            {#each entry.items as item}
              <Button
                href={item.href}
                class="ash-link ash-link--sub{isActive(item.href)
                  ? ' ash-link--active'
                  : ''}"
              >
                <i class="{item.icon} ash-icon"></i>
                {item.name}
              </Button>
            {/each}
          </div>
        </div>
      {:else}
        <Button
          href={entry.href}
          class="ash-link{isActive(entry.href) ? ' ash-link--active' : ''}"
        >
          <i class="{entry.icon} ash-icon"></i>
          {entry.name}
        </Button>
      {/if}
    {/each}
  </nav>

  <!-- ── Main content ────────────────────────────────────────────────── -->
  <main class="ash-content">
    {@render children()}
  </main>
</div>

<style>
  /* Reset padding that the farmer-facing app layout adds to body */
  :global(body) {
    padding-top: 0 !important;
    padding-bottom: 0 !important;
  }

  :global(.ash-layout) {
    display: grid;
    grid-template-rows: auto 1fr;
    grid-template-columns: 220px 1fr;
    height: 100dvh;
    overflow: hidden;
  }

  :global(.ash-header) {
    grid-column: 1 / -1;
    background: #022c22;
    color: #fff;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 16px;
    min-height: 54px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }
  :global(.ash-logo-link) {
    text-decoration: none;
    color: inherit;
  }
  :global(.ash-logo-link:hover) {
    opacity: 0.85;
  }
  :global(.ash-logo-icon) {
    font-size: 1.15rem;
    color: #6ee7b7;
  }
  :global(.ash-logo-text) {
    font-weight: 800;
    font-size: 1rem;
    letter-spacing: -0.01em;
  }
  :global(.ash-welcome) {
    font-size: 0.85rem;
    color: rgba(255, 255, 255, 0.7);
  }

  :global(.ash-sidebar) {
    background: #f4f7f5;
    border-right: 1px solid #e5e7eb;
    padding: 12px 8px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  :global(.ash-link) {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 10px;
    border-radius: 8px;
    color: #374151;
    text-decoration: none;
    font-size: 0.875rem;
    font-weight: 500;
    transition:
      background 0.12s,
      color 0.12s;
  }
  :global(.ash-link:hover) {
    background: #e6f0ec;
    color: #065f46;
  }
  :global(.ash-link--active) {
    background: #d1fae5;
    color: #065f46;
    font-weight: 700;
  }
  :global(.ash-link--sub) {
    padding-left: 28px;
    font-size: 0.83rem;
  }

  :global(.ash-group-label) {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 10px 4px;
    color: #9ca3af;
    font-size: 0.7rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    cursor: default;
    user-select: none;
    margin-top: 8px;
  }

  :global(.ash-group-label--active) {
    color: #065f46;
  }
  :global(.ash-group-items) {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  :global(.ash-icon) {
    width: 16px;
    text-align: center;
    flex-shrink: 0;
  }

  :global(.ash-content) {
    overflow-y: auto;
    background: #f0f4f2;
  }
</style>
