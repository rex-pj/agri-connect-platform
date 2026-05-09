<script lang="ts">
  import ProductCard    from "$lib/components/organisms/ProductCard.svelte";
  import ProviderCard   from "$lib/components/organisms/ProviderCard.svelte";
  import CategoryGrid   from "$lib/components/organisms/CategoryGrid.svelte";
  import GeoCta         from "$lib/components/molecules/GeoCta.svelte";
  import SectionHeader  from "$lib/components/molecules/SectionHeader.svelte";
  import { PRODUCT_DISPLAY, DEFAULT_DISPLAY } from "$lib/categoryIcons";
  import type { PageData } from "./$types";

  const { data }: { data: PageData } = $props();

  const gridCategories = $derived(data.categories.map(cat => ({
    name: cat.name,
    icon: cat.slug ? (PRODUCT_DISPLAY[cat.slug] ?? DEFAULT_DISPLAY).icon : cat.icon,
    href: cat.href,
  })));
</script>

<svelte:head>
  <title>Agri-Connect — Nền tảng nông nghiệp Đồng bằng sông Cửu Long</title>
  <meta name="description" content="Tìm nhà cung cấp dịch vụ và sản phẩm nông nghiệp uy tín tại Đồng bằng sông Cửu Long. Kết nối nhanh, minh bạch, không qua trung gian." />
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
    <div class="provider-preview">
      {#each data.featuredProviders.slice(0, 3) as provider}
        <ProviderCard {provider} />
      {/each}
    </div>
  {/if}
</section>

<style>
  .cat-section {
    padding: 20px 16px 4px;
  }

  .list-section {
    padding: 24px 16px 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .provider-preview {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 14px;
    padding-bottom: 8px;
  }
</style>
