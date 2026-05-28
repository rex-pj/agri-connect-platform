<script lang="ts">
  import type { PageData } from "./$types";
  import type { AdminStats } from "./+page.server";

  import Heading from "$lib/components/atoms/Heading.svelte";
  import Text from "$lib/components/atoms/Text.svelte";
  import Button from "$lib/components/atoms/Button.svelte";

  const { data }: { data: PageData } = $props();

  interface StatCard {
    label: string;
    value: number | string;
    icon: string;
    color: string;
    bg: string;
  }

  const stats: StatCard[] = data.stats
    ? [
        {
          label: "Tổng nhà cung cấp",
          value: data.stats.total_businesses,
          icon: "fa-solid fa-store",
          color: "#065F46",
          bg: "#D1FAE5",
        },
        {
          label: "Đã xác minh",
          value: data.stats.verified_businesses,
          icon: "fa-solid fa-circle-check",
          color: "#1D4ED8",
          bg: "#DBEAFE",
        },
        {
          label: "Sản phẩm đang bán",
          value: data.stats.active_products,
          icon: "fa-solid fa-box",
          color: "#92400E",
          bg: "#FEF3C7",
        },
        {
          label: "Tài khoản hệ thống",
          value: data.stats.total_users,
          icon: "fa-solid fa-users",
          color: "#6D28D9",
          bg: "#EDE9FE",
        },
      ]
    : [];

  interface FeatureCard {
    href: string;
    icon: string;
    title: string;
    desc: string;
    color: string;
  }

  const features: FeatureCard[] = [
    {
      href: "/admin/explore",
      icon: "fa-solid fa-store",
      title: "Nhà cung cấp",
      desc: "Xem danh sách, xác minh và quản lý nhà cung cấp trên nền tảng.",
      color: "#065F46",
    },
    // Reserved slots for future modules
    {
      href: "#",
      icon: "fa-solid fa-star",
      title: "Đánh giá",
      desc: "Quản lý đánh giá và phản hồi từ nông dân (sắp ra mắt).",
      color: "#92400E",
    },
    {
      href: "#",
      icon: "fa-solid fa-bullhorn",
      title: "Quảng cáo",
      desc: "Quản lý banner quảng cáo và listing ưu tiên (sắp ra mắt).",
      color: "#1D4ED8",
    },
  ];
</script>

<svelte:head>
  <title>Dashboard — Agri Admin</title>
</svelte:head>

<!-- ── Page header ────────────────────────────────────────────────────── -->
<div class="db-header">
  <div>
    <Heading level={1} class="db-title">
      <i class="fa-solid fa-gauge me-2 text-success"></i>Dashboard
    </Heading>
    <Text class="db-sub">Tổng quan hoạt động nền tảng Agri-Connect</Text>
  </div>
</div>

<div class="db-body">
  <!-- ── Stats error ─────────────────────────────────────────────────── -->
  {#if data.statsError}
    <div class="alert alert-warning d-flex align-items-center gap-2">
      <i class="fa-solid fa-triangle-exclamation"></i>
      {data.statsError}
    </div>
  {/if}

  <!-- ── Stat cards ──────────────────────────────────────────────────── -->
  {#if stats.length}
    <div class="row g-3 g-md-4 mb-4">
      {#each stats as s}
        <div class="col-6 col-lg-3">
          <div class="stat-card">
            <div
              class="stat-card__icon"
              style="background:{s.bg}; color:{s.color}"
            >
              <i class={s.icon}></i>
            </div>
            <div class="stat-card__body">
              <Text class="stat-card__label">{s.label}</Text>
              <Text class="stat-card__value"
                >{s.value.toLocaleString("vi-VN")}</Text
              >
            </div>
          </div>
        </div>
      {/each}
    </div>
  {/if}

  <!-- ── Feature cards ──────────────────────────────────────────────── -->
  <Heading level={2} class="mgmt-label">Công cụ quản lý</Heading>
  <div class="row g-3">
    {#each features as f}
      <div class="col-md-4">
        <Button
          href={f.href}
          class="feature-card text-decoration-none{f.href === '#'
            ? ' feature-card--disabled'
            : ''}"
        >
          <div class="feature-card__icon" style="color:{f.color}">
            <i class={f.icon}></i>
          </div>
          <Heading level={3} class="feature-card__title">{f.title}</Heading>
          <Text class="feature-card__desc">{f.desc}</Text>
        </Button>
      </div>
    {/each}
  </div>
</div>

<style>
  /* ── Header ──────────────────────────────────────── */
  .db-header {
    padding: 28px 24px 16px;
    border-bottom: 1px solid #e5e7eb;
    background: #fff;
  }
  :global(.db-title) {
    font-size: 1.4rem;
    font-weight: 800;
    margin: 0 0 4px;
    color: #0c1a12;
  }
  :global(.db-sub) {
    font-size: 0.875rem;
    color: #5e7a68;
    margin: 0;
  }

  /* ── Body ────────────────────────────────────────── */
  .db-body {
    padding: 24px;
  }

  /* ── Stat card ───────────────────────────────────── */
  :global(.stat-card) {
    background: #fff;
    border-radius: 14px;
    border: 1px solid #e5e7eb;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.05);
    padding: 16px;
    display: flex;
    align-items: center;
    gap: 14px;
  }
  :global(.stat-card__icon) {
    width: 48px;
    height: 48px;
    border-radius: 12px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1.25rem;
    flex-shrink: 0;
  }
  :global(.stat-card__body) {
    min-width: 0;
  }
  :global(.stat-card__label) {
    font-size: 0.75rem;
    color: #5e7a68;
    margin: 0 0 4px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  :global(.stat-card__value) {
    font-size: 1.5rem;
    font-weight: 800;
    color: #0c1a12;
    margin: 0;
    line-height: 1;
  }

  /* — Management section label ————————————————————— */
  :global(.mgmt-label) {
    font-size: 0.72rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: #5e7a68;
    margin: 0 0 20px;
    padding-top: 4px;
  }

  /* — Feature card ————————————————————————————————— */
  :global(.feature-card) {
    display: flex !important;
    flex-direction: column;
    align-items: flex-start;
    width: 100%;
    background: #fff;
    border-radius: 16px;
    border: 1px solid #e5e7eb;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.05);
    padding: 24px;
    transition:
      transform 0.15s,
      box-shadow 0.15s;
    cursor: pointer;
    text-align: left;
    white-space: normal !important;
  }

  :global(.feature-card:hover:not(.feature-card--disabled)) {
    transform: translateY(-4px);
    box-shadow: 0 8px 24px rgba(6, 78, 59, 0.12);
  }
  :global(.feature-card--disabled) {
    opacity: 0.55;
    cursor: default;
  }

  :global(.feature-card__icon) {
    font-size: 2rem;
    margin-bottom: 14px;
  }
  :global(.feature-card__title) {
    font-size: 1rem;
    font-weight: 700;
    color: #0c1a12;
    margin: 0 0 8px;
  }
  :global(.feature-card__desc) {
    font-size: 0.875rem;
    color: #5e7a68;
    margin: 0;
    line-height: 1.55;
  }
</style>
