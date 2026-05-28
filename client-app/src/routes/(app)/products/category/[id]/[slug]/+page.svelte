<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import ProductCard from "$lib/components/organisms/ProductCard.svelte";
  import PageHero from "$lib/components/molecules/PageHero.svelte";
  import AlertBanner from "$lib/components/molecules/AlertBanner.svelte";
  import EmptyState from "$lib/components/molecules/EmptyState.svelte";
  import PaginationNav from "$lib/components/molecules/PaginationNav.svelte";
  import { PRODUCT_DISPLAY, DEFAULT_DISPLAY } from "$lib/categoryIcons";
  import { ROUTES } from "$lib/routes";
  import type { PageData } from "./$types";

  const { data }: { data: PageData } = $props();

  const catDisplay = $derived(
    data.currentCategory ? (PRODUCT_DISPLAY[data.currentCategory.slug] ?? DEFAULT_DISPLAY) : DEFAULT_DISPLAY
  );

  function goToPage(p: number) {
    const params = new URLSearchParams(page.url.searchParams);
    params.set("page", String(p));
    goto(`?${params.toString()}`, { replaceState: false });
  }
</script>

<svelte:head>
  <title
    >{data.currentCategory?.name ?? "Danh mục sản phẩm"} — Agri-Connect</title
  >
  <meta
    name="description"
    content="Xem các sản phẩm nông nghiệp trong danh mục {data.currentCategory
      ?.name ?? ''} từ các nhà cung cấp uy tín tại Đồng bằng sông Cửu Long."
  />
</svelte:head>

<PageHero
  title={data.currentCategory?.name ?? "Danh mục sản phẩm"}
  icon={catDisplay.icon}
  sub={catDisplay.desc ? `${catDisplay.desc}${data.result.total > 0 ? ` · ${data.result.total} sản phẩm` : ''}` : undefined}
  backLink={{ href: ROUTES.PRODUCTS, label: "Sản phẩm" }}
/>

<section class="p-3">
  {#if data.error}
    <AlertBanner type="error" message={data.error} />
  {/if}

  {#if data.result.items.length === 0 && !data.error}
    <EmptyState
      icon="fa-inbox"
      title="Chưa có sản phẩm nào trong danh mục này"
      hint="Hãy thử chọn danh mục khác hoặc xem tất cả sản phẩm."
    />
  {:else}
    <p class="small text-muted mb-3">
      <strong>{data.result.total}</strong> sản phẩm
    </p>

    <div class="row row-cols-2 row-cols-md-3 row-cols-lg-5 row-cols-xl-6 g-3">
      {#each data.result.items as product (product.id)}
        <div class="col"><ProductCard {product} /></div>
      {/each}
    </div>

    {#if data.result.total_pages > 1}
      <PaginationNav
        currentPage={data.result.page}
        totalPages={data.result.total_pages}
        onPageChange={goToPage}
      />
    {/if}
  {/if}
</section>
