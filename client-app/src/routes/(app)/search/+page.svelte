<script lang="ts">
  import { goto } from '$app/navigation';
  import ProviderCard from '$lib/components/organisms/ProviderCard.svelte';
  import ProductCard  from '$lib/components/organisms/ProductCard.svelte';
  import EmptyState   from '$lib/components/molecules/EmptyState.svelte';
  import { ROUTES }   from '$lib/routes';
  import type { PageData } from './$types';

  const { data }: { data: PageData } = $props();

  let searchQuery = $state(data.q);
  let activeTab = $state<'all' | 'services' | 'products'>('all');

  const PREVIEW = 6;
  const providers = $derived(data.providers.slice(0, PREVIEW));
  const products  = $derived(data.products.slice(0, PREVIEW));

  function submitSearch(e: SubmitEvent) {
    e.preventDefault();
    const q = searchQuery.trim();
    if (!q) return;
    activeTab = 'all';
    goto(`${ROUTES.SEARCH}?q=${encodeURIComponent(q)}`, { replaceState: false });
  }

  $effect(() => {
    searchQuery = data.q;
    activeTab = 'all';
  });
</script>

<svelte:head>
  {#if data.q}
    <title>Tìm kiếm "{data.q}" — Agri-Connect</title>
  {:else}
    <title>Tìm kiếm — Agri-Connect</title>
  {/if}
  <meta name="robots" content="noindex" />
</svelte:head>

<!-- Inline search bar (allows query refinement without touching the navbar) -->
<div class="search-bar-row">
  <form class="search-bar-form" onsubmit={submitSearch}>
    <i class="fa-solid fa-magnifying-glass search-bar-icon"></i>
    <input
      class="search-bar-input"
      type="search"
      bind:value={searchQuery}
      placeholder="Tìm dịch vụ, nông sản..."
      aria-label="Tìm kiếm"
      enterkeyhint="search"
    />
    <button type="submit" class="search-bar-btn" aria-label="Tìm kiếm">
      <i class="fa-solid fa-arrow-right"></i>
    </button>
  </form>
</div>

{#if !data.q}
  <div class="p-3 mt-2">
    <EmptyState
      icon="fa-magnifying-glass"
      title="Nhập từ khóa để tìm kiếm"
      hint="Tìm nhà cung cấp dịch vụ và sản phẩm nông nghiệp."
    />
  </div>
{:else}
  <!-- Tab pills -->
  <div class="search-tabs" role="tablist">
    <button
      class="search-tab{activeTab === 'all' ? ' active' : ''}"
      onclick={() => (activeTab = 'all')}
      role="tab"
      aria-selected={activeTab === 'all'}
    >
      Tất cả
    </button>
    <button
      class="search-tab{activeTab === 'services' ? ' active' : ''}"
      onclick={() => (activeTab = 'services')}
      role="tab"
      aria-selected={activeTab === 'services'}
    >
      Dịch vụ
      {#if data.providerTotal > 0}
        <span class="search-tab__badge">{data.providerTotal}</span>
      {/if}
    </button>
    <button
      class="search-tab{activeTab === 'products' ? ' active' : ''}"
      onclick={() => (activeTab = 'products')}
      role="tab"
      aria-selected={activeTab === 'products'}
    >
      Sản phẩm
      {#if data.productTotal > 0}
        <span class="search-tab__badge">{data.productTotal}</span>
      {/if}
    </button>
  </div>

  <!-- Providers section -->
  {#if activeTab === 'all' || activeTab === 'services'}
    <section class="px-3 pt-3" role="tabpanel">
      <div class="d-flex align-items-baseline justify-content-between mb-2">
        <h2 class="fs-6 fw-bold mb-0">
          <i class="fa-solid fa-store me-1 text-success"></i>Nhà cung cấp
          <span class="text-muted fw-normal small ms-1">({data.providerTotal})</span>
        </h2>
        {#if data.providerTotal > PREVIEW}
          <a
            href="{ROUTES.SERVICES}?q={encodeURIComponent(data.q)}"
            class="small text-success text-decoration-none"
          >
            Xem thêm <i class="fa-solid fa-chevron-right" style="font-size:0.65rem"></i>
          </a>
        {/if}
      </div>

      {#if providers.length > 0}
        <div class="row row-cols-1 row-cols-md-2 row-cols-lg-3 g-3">
          {#each providers as provider (provider.id)}
            <div class="col"><ProviderCard {provider} /></div>
          {/each}
        </div>
      {:else}
        <EmptyState
          icon="fa-store"
          title="Không có nhà cung cấp phù hợp"
          hint="Thử từ khóa khác hoặc xem toàn bộ danh mục dịch vụ."
        />
      {/if}
    </section>
  {/if}

  <!-- Products section -->
  {#if activeTab === 'all' || activeTab === 'products'}
    <section class="px-3 pt-4 pb-3" role="tabpanel">
      <div class="d-flex align-items-baseline justify-content-between mb-2">
        <h2 class="fs-6 fw-bold mb-0">
          <i class="fa-solid fa-bag-shopping me-1 text-success"></i>Sản phẩm
          <span class="text-muted fw-normal small ms-1">({data.productTotal})</span>
        </h2>
        {#if data.productTotal > PREVIEW}
          <a
            href="{ROUTES.PRODUCTS}?q={encodeURIComponent(data.q)}"
            class="small text-success text-decoration-none"
          >
            Xem thêm <i class="fa-solid fa-chevron-right" style="font-size:0.65rem"></i>
          </a>
        {/if}
      </div>

      {#if products.length > 0}
        <div class="row row-cols-2 row-cols-md-3 row-cols-lg-5 g-2 g-md-3">
          {#each products as product (product.id)}
            <div class="col"><ProductCard {product} /></div>
          {/each}
        </div>
      {:else}
        <EmptyState
          icon="fa-bag-shopping"
          title="Không có sản phẩm phù hợp"
          hint="Thử từ khóa khác hoặc xem toàn bộ danh mục sản phẩm."
        />
      {/if}
    </section>
  {/if}
{/if}

<style>
  .search-bar-row {
    padding: 20px 16px 0;
  }

  .search-bar-form {
    display: flex;
    align-items: center;
    gap: 10px;
    background: #f5f7f5;
    border: 1.5px solid #d1e7d9;
    border-radius: 100px;
    padding: 0 6px 0 14px;
    min-height: 44px;
    transition: border-color 0.15s;
  }

  .search-bar-form:focus-within {
    border-color: #16a34a;
    background: #fff;
  }

  .search-bar-icon {
    color: #6b9e7a;
    font-size: 0.85rem;
    flex-shrink: 0;
  }

  .search-bar-input {
    flex: 1;
    border: none;
    background: transparent;
    font-size: 0.95rem;
    color: #1a2e24;
    outline: none;
    min-width: 0;
  }

  .search-bar-input::placeholder {
    color: #9ab5a1;
  }

  /* remove native clear button so our submit btn is the only action */
  .search-bar-input::-webkit-search-cancel-button {
    display: none;
  }

  .search-bar-btn {
    flex-shrink: 0;
    width: 34px;
    height: 34px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #16a34a;
    border: none;
    border-radius: 50%;
    color: #fff;
    font-size: 0.78rem;
    cursor: pointer;
    transition: background 0.15s;
    -webkit-tap-highlight-color: transparent;
    appearance: none;
    -webkit-appearance: none;
  }

  .search-bar-btn:hover {
    background: #15803d;
  }

  .search-bar-btn:active {
    background: #166534;
  }

  .search-bar-btn:focus-visible {
    outline: 2px solid #16a34a;
    outline-offset: 2px;
  }

  /* Tab pills */
  .search-tabs {
    display: flex;
    gap: 8px;
    padding: 12px 16px 0;
    overflow-x: auto;
    -webkit-overflow-scrolling: touch;
    scrollbar-width: none;
  }

  .search-tabs::-webkit-scrollbar {
    display: none;
  }

  .search-tab {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 6px 14px;
    border-radius: 100px;
    border: 1.5px solid #d1e7d9;
    background: #fff;
    color: #4b7a5e;
    font-size: 0.85rem;
    font-weight: 500;
    cursor: pointer;
    white-space: nowrap;
    transition:
      background 0.15s,
      border-color 0.15s,
      color 0.15s;
    -webkit-tap-highlight-color: transparent;
    appearance: none;
    -webkit-appearance: none;
    min-height: 36px;
  }

  .search-tab.active {
    background: #16a34a;
    border-color: #16a34a;
    color: #fff;
  }

  .search-tab:hover:not(.active) {
    background: #f0fdf4;
    border-color: #86efac;
  }

  .search-tab__badge {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 18px;
    height: 18px;
    padding: 0 5px;
    border-radius: 100px;
    font-size: 0.72rem;
    font-weight: 700;
    background: rgba(0, 0, 0, 0.12);
    color: inherit;
    line-height: 1;
  }

  .search-tab.active .search-tab__badge {
    background: rgba(255, 255, 255, 0.25);
  }
</style>
