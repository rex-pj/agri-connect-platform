<script lang="ts">
  import ProductCard from "$lib/components/organisms/ProductCard.svelte";
  import ProviderCard from "$lib/components/organisms/ProviderCard.svelte";
  import CategoryGrid from "$lib/components/organisms/CategoryGrid.svelte";
  import GeoCta from "$lib/components/molecules/GeoCta.svelte";
  import SectionHeader from "$lib/components/molecules/SectionHeader.svelte";
  import { PRODUCT_DISPLAY, DEFAULT_DISPLAY } from "$lib/categoryIcons";
  import type { PageData } from "./$types";

  const { data }: { data: PageData } = $props();

  const gridCategories = $derived(
    data.categories.map((cat) => ({
      name: cat.name,
      icon: cat.slug
        ? (PRODUCT_DISPLAY[cat.slug] ?? DEFAULT_DISPLAY).icon
        : cat.icon,
      href: cat.href,
    })),
  );
</script>

<svelte:head>
  <title>Agri-Connect — Nền tảng nông nghiệp Đồng bằng sông Cửu Long</title>
  <meta
    name="description"
    content="Tìm nhà cung cấp dịch vụ và sản phẩm nông nghiệp uy tín tại Đồng bằng sông Cửu Long. Kết nối nhanh, minh bạch, không qua trung gian."
  />
</svelte:head>

<!-- Category quick-links -->
<div class="cat-section">
  <CategoryGrid items={gridCategories} />
</div>

<!-- Featured Products -->
{#if data.featuredProducts.length > 0}
  <section class="list-section">
    <SectionHeader title="Nông sản nổi bật" moreHref="/products" />
    <div class="row row-cols-2 row-cols-md-3 row-cols-lg-5 row-cols-xl-6 g-3">
      {#each data.featuredProducts.slice(0, 6) as product}
        <div class="col"><ProductCard {product} /></div>
      {/each}
    </div>
  </section>
{/if}

<!-- Geo-discovery CTA + featured providers -->
<section class="list-section">
  <GeoCta />
  {#if data.featuredProviders.length > 0}
    <div class="row row-cols-1 row-cols-md-2 row-cols-lg-3 g-3">
      {#each data.featuredProviders.slice(0, 3) as provider}
        <div class="col"><ProviderCard {provider} /></div>
      {/each}
    </div>
  {/if}
</section>

{#if data.featuredProducts.length === 0 && data.featuredProviders.length === 0}
  <div class="empty-home">
    <i class="fa-solid fa-seedling empty-home__icon"></i>
    <p class="empty-home__title">Nền tảng đang khởi động</p>
    <p class="empty-home__hint">
      Chưa có dữ liệu. Hãy thử tìm dịch vụ gần bạn.
    </p>
    <a href="/explore" class="empty-home__cta">
      <i class="fa-solid fa-location-dot"></i> Tìm thợ gần tôi
    </a>
  </div>
{/if}

<style>
  .cat-section {
    padding: 16px 12px;
  }

  .list-section {
    padding: 24px 16px 16px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  /* * Empty home state */
  .empty-home {
    text-align: center;
    padding: 48px 24px 64px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }

  .empty-home__icon {
    font-size: 3rem;
    color: var(--p300, #6ee7b7);
    margin-bottom: 8px;
  }

  .empty-home__title {
    font-size: 1.1rem;
    font-weight: 700;
    color: var(--text, #0c1a12);
    margin: 0;
  }

  .empty-home__hint {
    font-size: 0.9rem;
    color: var(--muted, #5e7a68);
    margin: 0;
  }

  .empty-home__cta {
    margin-top: 12px;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 12px 24px;
    border-radius: 12px;
    background: var(--p700, #065f46);
    color: #fff;
    font-size: 0.95rem;
    font-weight: 700;
    text-decoration: none;
  }
</style>
