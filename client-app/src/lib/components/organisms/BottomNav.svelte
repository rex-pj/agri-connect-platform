<script lang="ts">
  import { page } from "$app/state";
  import { ROUTES } from "$lib/routes";

  interface Props {
    role?: string | null;
  }

  const { role = null }: Props = $props();

  const staticTabs = [
    { href: ROUTES.HOME, icon: "fa-compass", label: "Khám phá" },
    { href: ROUTES.EXPLORE, icon: "fa-location-dot", label: "Gần tôi" },
    { href: ROUTES.SERVICES, icon: "fa-border-all", label: "Dịch vụ" },
    { href: ROUTES.PRODUCTS, icon: "fa-bag-shopping", label: "Sản phẩm" },
  ];

  const accountTab = $derived(
    role === "provider"
      ? { href: ROUTES.PROVIDER_DASHBOARD, icon: "fa-store", label: "Dashboard" }
      : role === "admin"
        ? { href: ROUTES.ADMIN.DASHBOARD, icon: "fa-shield-halved", label: "Admin" }
        : { href: ROUTES.ACCOUNT, icon: "fa-circle-user", label: "Tài khoản" },
  );

  function isActive(href: string): boolean {
    if (href === "/") return page.url.pathname === "/";
    return page.url.pathname.startsWith(href);
  }
</script>

<nav class="bottom-nav" aria-label="Điều hướng chính">
  {#each staticTabs as tab}
    {@const active = isActive(tab.href)}
    <a
      href={tab.href}
      class="tab"
      class:tab--active={active}
      aria-current={active ? "page" : undefined}
    >
      <span class="tab__pill">
        <i class="fa-solid {tab.icon} tab__icon"></i>
      </span>
      <span class="tab__label">{tab.label}</span>
    </a>
  {/each}
  <a
    href={accountTab.href}
    class="tab"
    class:tab--active={isActive(accountTab.href)}
    aria-current={isActive(accountTab.href) ? "page" : undefined}
  >
    <span class="tab__pill">
      <i class="fa-solid {accountTab.icon} tab__icon"></i>
    </span>
    <span class="tab__label">{accountTab.label}</span>
  </a>
</nav>

<style>
  .bottom-nav {
    position: fixed;
    bottom: 0;
    left: 0;
    right: 0;
    z-index: 1000;
    display: flex;
    background: #fff;
    border-top: 1px solid var(--border, #c8d8ce);
    padding-bottom: env(safe-area-inset-bottom, 0px);
    box-shadow: 0 -4px 20px rgba(6, 78, 59, 0.08);
  }

  .tab {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 8px 8px 6px;
    text-decoration: none;
    color: var(--muted, #5e7a68);
    -webkit-tap-highlight-color: transparent;
    min-height: var(--nav-h, 64px);
    justify-content: center;
  }

  .tab__pill {
    width: 44px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 14px;
    transition: background 0.2s ease;
  }

  .tab--active .tab__pill {
    background: var(--p50, #ecfdf5);
  }

  .tab__icon {
    font-size: 1.2rem;
    transition:
      color 0.2s ease,
      transform 0.2s ease;
  }

  .tab--active .tab__icon {
    color: var(--p700, #065f46);
    transform: scale(1.1);
  }

  .tab__label {
    font-size: 0.85rem;
    font-weight: 600;
    letter-spacing: 0.01em;
    transition: color 0.2s ease;
  }

  .tab--active .tab__label {
    color: var(--p700, #065f46);
    font-weight: 700;
  }
</style>
