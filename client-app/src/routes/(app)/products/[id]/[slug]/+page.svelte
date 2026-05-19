<script lang="ts">
  import { getProductDisplay } from "$lib/categoryIcons";
  import Heading from "$lib/components/atoms/Heading.svelte";
  import Text from "$lib/components/atoms/Text.svelte";
  import Button from "$lib/components/atoms/Button.svelte";
  import type { PageData } from "./$types";
  import { ROUTES } from "$lib/routes";

  const { data }: { data: PageData } = $props();
  const p = $derived(data.product);

  const telHref = $derived(`tel:${p.vendor_phone}`);
  const catIcon = $derived(getProductDisplay(p.category).icon);

  function fmtPrice(v: number): string {
    return v.toLocaleString("vi-VN") + " ₫";
  }
</script>

<svelte:head>
  <title>{p.name} — Agri-Connect</title>
  <meta
    name="description"
    content={p.description
      ? `${p.description.slice(0, 150)}`
      : `Mua ${p.name} từ ${p.vendor_name} trên Agri-Connect. Liên hệ nhà cung cấp để biết giá chính xác.`}
  />
</svelte:head>

<div class="product-layout" class:product-layout--has-image={!!p.image_url}>
  <!-- Left: image (only when available) -->
  {#if p.image_url}
    <div class="img-hero">
      <Button href={ROUTES.PRODUCTS} class="back-link back-link--overlay">
        <i class="fa-solid fa-arrow-left"></i> Sản phẩm
      </Button>
      <img src={p.image_url} alt={p.name} class="img-hero__img" />
    </div>
  {/if}

  <!-- Right: hero + body -->
  <div class="product-panel">
    {#if p.image_url}
      <div class="page-hero page-hero--compact">
        <Heading level={1} class="page-hero__title">{p.name}</Heading>
        <span class="cat-badge">
          <i class="fa-solid {catIcon}"></i>
          {p.category}
        </span>
      </div>
    {:else}
      <div class="page-hero page-hero--detail">
        <Button href={ROUTES.PRODUCTS} class="back-link">
          <i class="fa-solid fa-arrow-left"></i> Sản phẩm
        </Button>
        <Heading level={1} class="page-hero__title">{p.name}</Heading>
        <span class="cat-badge">
          <i class="fa-solid {catIcon}"></i>
          {p.category}
        </span>
      </div>
    {/if}

    <div class="detail-body">
      <!-- Price row -->
      <div class="price-card">
        {#if !p.is_available}
          <span class="badge-oos">Hết hàng</span>
        {/if}
        <div class="price-row">
          {#if p.price != null}
            <span class="price">{fmtPrice(p.price)}</span>
            {#if p.unit}<span class="unit">/{p.unit}</span>{/if}
          {:else}
            <span class="price price--contact">Liên hệ giá</span>
          {/if}
        </div>
        <Text class="price-note"
          >Giá tham khảo — liên hệ nhà cung cấp để biết giá chính xác</Text
        >
      </div>

      <!-- Description -->
      {#if p.description}
        <div class="section">
          <Heading level={2} class="section__title">Mô tả sản phẩm</Heading>
          <Text class="section__body">{p.description}</Text>
        </div>
      {/if}

      <!-- Vendor -->
      <div class="section">
        <Heading level={2} class="section__title">Nhà cung cấp</Heading>
        <div class="vendor-row">
          <span class="vendor-icon"><i class="fa-solid fa-shop"></i></span>
          <div class="vendor-info">
            <strong class="vendor-name">{p.vendor_name}</strong>
            <span class="vendor-phone">{p.vendor_phone}</span>
          </div>
        </div>
        <Button href={telHref} class="call-btn">
          <i class="fa-solid fa-phone"></i> Gọi tư vấn ngay
        </Button>
      </div>
    </div>
  </div>
</div>

<style>
  /* ── Outer layout ───────────────────────────────────────────────────── */
  .product-layout {
    display: block;
  }
  @media (min-width: 768px) {
    .product-layout--has-image {
      display: grid;
      grid-template-columns: 55% 45%;
      align-items: stretch;
      max-width: 1280px;
      margin: 0 auto;
      min-height: 520px;
    }
    /* No-image variant: centre a narrow panel */
    .product-layout:not(.product-layout--has-image) .product-panel {
      max-width: 720px;
      margin: 0 auto;
    }
    /* Hero inside the grid: remove full-bleed radius, add top breathing room */
    .product-layout--has-image .page-hero {
      border-radius: 0;
      padding-top: 28px;
    }
    /* At viewports wider than 1280px the grid has side margins.
       Pull the hero past the grid's right boundary so its background
       bleeds to the viewport edge, then compensate with padding so
       the text stays at the original 24px inset. */
    @media (min-width: 1281px) {
      .product-layout--has-image .page-hero {
        margin-right: calc((1280px - 100vw) / 2);
        padding-right: calc((100vw - 1280px) / 2 + 24px);
      }
    }
    /* Align cards with hero: match the hero's 24px horizontal padding */
    .product-layout--has-image .detail-body {
      padding-left: 24px;
      padding-right: 24px;
      max-width: 520px;
    }
  }

  /* ── Image hero ─────────────────────────────────────────────────────── */
  .img-hero {
    position: relative;
    width: 100%;
    aspect-ratio: 16/9;
    max-height: 400px;
    overflow: hidden;
    background: #f3f4f6;
  }
  @media (min-width: 768px) {
    .img-hero {
      aspect-ratio: unset;
      max-height: none;
      height: 100%;
    }
  }
  .img-hero__img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  /* ── Category badge ─────────────────────────────────────────────────── */
  .cat-badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: rgba(255, 255, 255, 0.15);
    padding: 5px 14px;
    border-radius: 20px;
    font-size: 0.8rem;
    font-weight: 600;
  }
  .page-hero__title {
    margin-bottom: 10px;
  }

  /* ── Detail body ────────────────────────────────────────────────────── */
  .detail-body {
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  /* ── Price card ─────────────────────────────────────────────────────── */
  .price-card {
    background: #fff;
    border-radius: 16px;
    padding: 18px;
    box-shadow: 0 1px 6px rgba(0, 0, 0, 0.07);
    border: 1px solid var(--border, #e5e7eb);
  }
  .badge-oos {
    display: inline-block;
    background: #fee2e2;
    color: #b91c1c;
    padding: 4px 12px;
    border-radius: 6px;
    font-size: 0.8rem;
    font-weight: 700;
    margin-bottom: 10px;
  }
  .price-row {
    display: flex;
    align-items: baseline;
    gap: 6px;
    margin-bottom: 6px;
  }
  .price {
    font-size: 1.8rem;
    font-weight: 800;
    color: #ea580c;
  }
  .price--contact {
    font-size: 1.3rem;
    color: #b45309;
  }
  .unit {
    font-size: 0.95rem;
    color: #6b7280;
  }
  .price-note {
    font-size: 0.78rem;
    color: #9ca3af;
    margin: 0;
  }

  /* ── Section ────────────────────────────────────────────────────────── */
  .section {
    background: #fff;
    border-radius: 16px;
    padding: 18px;
    box-shadow: 0 1px 6px rgba(0, 0, 0, 0.07);
    border: 1px solid var(--border, #e5e7eb);
  }
  .section__title {
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted, #5e7a68);
    margin-bottom: 10px;
  }
  .section__body {
    font-size: 0.9rem;
    color: var(--text, #0c1a12);
    line-height: 1.6;
  }

  /* ── Vendor ─────────────────────────────────────────────────────────── */
  .vendor-row {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 14px;
  }
  .vendor-icon {
    width: 46px;
    height: 46px;
    background: var(--p50, #ecfdf5);
    border-radius: 12px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1.25rem;
    color: var(--p700, #065f46);
    flex-shrink: 0;
  }
  .vendor-info {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .vendor-name {
    font-size: 0.95rem;
    font-weight: 700;
    color: var(--text, #0c1a12);
  }
  .vendor-phone {
    font-size: 0.85rem;
    color: var(--muted, #5e7a68);
  }
  .call-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    min-height: 52px;
    background: var(--p700, #065f46);
    color: #fff;
    border-radius: 12px;
    text-decoration: none;
    font-size: 0.95rem;
    font-weight: 700;
    -webkit-tap-highlight-color: transparent;
  }
  .call-btn:active {
    opacity: 0.88;
  }
</style>
