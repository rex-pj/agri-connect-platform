<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { untrack } from "svelte";
  import ProviderCard  from "$lib/components/organisms/ProviderCard.svelte";
  import GeoCta        from "$lib/components/molecules/GeoCta.svelte";
  import ContactSheet  from "$lib/components/organisms/ContactSheet.svelte";
  import PageHero      from "$lib/components/molecules/PageHero.svelte";
  import AlertBanner   from "$lib/components/atoms/AlertBanner.svelte";
  import EmptyState    from "$lib/components/molecules/EmptyState.svelte";
  import PaginationNav from "$lib/components/molecules/PaginationNav.svelte";
  import { SERVICE_DISPLAY, DEFAULT_DISPLAY } from "$lib/categoryIcons";
  import { ROUTES } from "$lib/routes";
  import type { PageData, ActionData } from "./$types";
  import type { BrowseProvider } from "./+page.server";

  const { data, form }: { data: PageData; form: ActionData } = $props();

  let activeProvider  = $state<BrowseProvider | null>(null);
  let contactSuccess  = $state(false);
  let searchQuery     = $state(untrack(() => data.searchQuery ?? ''));
  let serviceId       = $state(untrack(() => data.serviceId ?? ''));
  let sort            = $state(untrack(() => data.sort ?? ''));
  let filtersOpen     = $state(false);

  $effect(() => {
    if (form?.success) {
      contactSuccess = true;
      activeProvider = null;
    }
  });

  $effect(() => {
    searchQuery = data.searchQuery ?? '';
    serviceId   = data.serviceId   ?? '';
    sort        = data.sort        ?? '';
  });

  const activeFilterCount = $derived(
    (data.serviceId ? 1 : 0) +
    (data.sort      ? 1 : 0)
  );

  function buildParams(overrides: Record<string, string> = {}) {
    const params = new URLSearchParams();
    const q   = overrides.q          ?? searchQuery.trim();
    const svc = overrides.service_id ?? serviceId;
    const s   = overrides.sort       ?? sort;
    const pg  = overrides.page       ?? '1';
    if (q)   params.set('q', q);
    if (svc) params.set('service_id', svc);
    if (s)   params.set('sort', s);
    if (pg !== '1') params.set('page', pg);
    return params;
  }

  function submitSearch(e: SubmitEvent) {
    e.preventDefault();
    goto(`${ROUTES.SERVICES}?${buildParams()}`, { replaceState: false });
  }

  function applyFilters() {
    filtersOpen = false;
    goto(`${ROUTES.SERVICES}?${buildParams()}`, { replaceState: false });
  }

  function resetFilters() {
    serviceId = ''; sort = '';
    filtersOpen = false;
    const params = new URLSearchParams();
    if (searchQuery.trim()) params.set('q', searchQuery.trim());
    goto(`${ROUTES.SERVICES}${params.size ? '?' + params : ''}`, { replaceState: false });
  }

  function goToPage(p: number) {
    const params = new URLSearchParams(page.url.searchParams);
    params.set("page", String(p));
    goto(`${ROUTES.SERVICES}?${params.toString()}`, { replaceState: false });
  }

  function openContact(p: BrowseProvider) {
    activeProvider = p;
    contactSuccess = false;
  }

  const SORT_OPTIONS = [
    { value: '',              label: 'Ưu tiên xác minh' },
    { value: 'newest',        label: 'Mới nhất' },
    { value: 'name_asc',      label: 'A → Z' },
  ];
</script>

<PageHero
  title="Danh mục dịch vụ"
  icon="fa-border-all"
  sub="Tất cả nhà cung cấp — không cần bật định vị"
  backLink={{ href: ROUTES.HOME, label: "Trang chủ" }}
/>

<!-- ── Search + filter panel ─────────────────────────────────────────────── -->
<div class="search-panel px-3 pt-3">

  <!-- Search row -->
  <form class="search-row" onsubmit={submitSearch}>
    <div class="search-input-wrap">
      <i class="fa-solid fa-magnifying-glass search-icon"></i>
      <input
        class="search-input"
        type="search"
        bind:value={searchQuery}
        placeholder="Tìm nhà cung cấp..."
        aria-label="Tìm nhà cung cấp"
        enterkeyhint="search"
      />
    </div>

    <button
      type="button"
      class="filter-toggle{filtersOpen ? ' active' : ''}"
      onclick={() => (filtersOpen = !filtersOpen)}
      aria-expanded={filtersOpen}
      aria-label="Bộ lọc nâng cao"
    >
      <i class="fa-solid fa-sliders"></i>
      {#if activeFilterCount > 0}
        <span class="filter-badge">{activeFilterCount}</span>
      {/if}
    </button>

    <button type="submit" class="search-btn" aria-label="Tìm kiếm">
      <i class="fa-solid fa-arrow-right"></i>
    </button>
  </form>

  <!-- Collapsible advanced filter panel -->
  <div class="filter-panel{filtersOpen ? ' open' : ''}" aria-hidden={!filtersOpen}>
    <div class="filter-panel-inner">
    <div class="filter-panel-body">

      <!-- Service category chips -->
      <div class="filter-section">
        <p class="filter-label">Loại dịch vụ</p>
        <div class="chip-row">
          <button
            type="button"
            class="chip{!serviceId ? ' active' : ''}"
            onclick={() => (serviceId = '')}
          >Tất cả</button>
          {#each data.categories as cat}
            {@const d = SERVICE_DISPLAY[cat.slug] ?? DEFAULT_DISPLAY}
            <button
              type="button"
              class="chip{serviceId === String(cat.id) ? ' active' : ''}"
              onclick={() => (serviceId = String(cat.id))}
            >
              <i class="fa-solid {d.icon}"></i>
              {cat.name}
            </button>
          {/each}
        </div>
      </div>

      <!-- Sort -->
      <div class="filter-section">
        <p class="filter-label">Sắp xếp</p>
        <div class="chip-row">
          {#each SORT_OPTIONS as opt}
            <button
              type="button"
              class="chip{sort === opt.value ? ' active' : ''}"
              onclick={() => (sort = opt.value)}
            >{opt.label}</button>
          {/each}
        </div>
      </div>

      <!-- Actions -->
      <div class="filter-actions">
        <button type="button" class="btn-reset" onclick={resetFilters}>
          Xóa bộ lọc
        </button>
        <button type="button" class="btn-apply" onclick={applyFilters}>
          Áp dụng
        </button>
      </div>

    </div>
    </div><!-- filter-panel-inner -->
  </div><!-- filter-panel -->
</div><!-- search-panel -->

<div class="px-3 pt-4">
  <GeoCta
    title="Tìm nhà cung cấp gần bạn"
    sub="Cơ khí · Phun thuốc · Múc bùn — trong 10 km"
    btn="Bật định vị"
  />
</div>

<!-- ── Results ────────────────────────────────────────────────────────────── -->
<section class="p-3">
  {#if data.error}
    <AlertBanner type="error" message={data.error} />
  {/if}
  {#if contactSuccess}
    <AlertBanner
      type="success"
      message="Đã gửi yêu cầu! Nhà cung cấp sẽ liên hệ lại với bạn sớm."
    />
  {/if}

  {#if data.result.items.length === 0 && !data.error}
    {#if data.searchQuery || data.serviceId}
      <EmptyState
        icon="fa-store"
        title="Không tìm thấy nhà cung cấp nào"
        hint="Thử từ khóa khác hoặc xóa bộ lọc để xem tất cả nhà cung cấp."
      />
    {:else}
      <EmptyState
        icon="fa-inbox"
        title="Chưa có nhà cung cấp nào"
        hint="Hãy thử chọn một loại dịch vụ bên trên."
      />
    {/if}
  {:else}
    <p class="small text-muted mb-3">
      <strong>{data.result.total}</strong> nhà cung cấp{data.searchQuery ? ` cho "${data.searchQuery}"` : ''}
    </p>

    <div class="row row-cols-1 row-cols-md-2 row-cols-lg-3 g-3">
      {#each data.result.items as provider (provider.id)}
        <div class="col">
          <ProviderCard {provider} onContact={() => openContact(provider)} />
        </div>
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

{#if activeProvider}
  <ContactSheet
    provider={activeProvider}
    {form}
    onClose={() => (activeProvider = null)}
  />
{/if}

<style>
  .search-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .search-input-wrap {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 10px;
    background: #f5f7f5;
    border: 1.5px solid #d1e7d9;
    border-radius: 100px;
    padding: 0 6px 0 14px;
    min-height: 44px;
    transition: border-color 0.15s, background 0.15s;
  }

  .search-input-wrap:focus-within {
    border-color: #16a34a;
    background: #fff;
  }

  .search-icon {
    color: #6b9e7a;
    font-size: 0.85rem;
    flex-shrink: 0;
  }

  .search-input {
    flex: 1;
    border: none;
    background: transparent;
    font-size: 0.92rem;
    color: #1a2e24;
    outline: none;
    min-width: 0;
  }

  .search-input::placeholder { color: #9ab5a1; }
  .search-input::-webkit-search-cancel-button { display: none; }

  .filter-toggle {
    position: relative;
    flex-shrink: 0;
    width: 44px;
    height: 44px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #f5f7f5;
    border: 1.5px solid #d1e7d9;
    border-radius: 50%;
    color: #4b7a5e;
    font-size: 0.92rem;
    cursor: pointer;
    transition: background 0.15s, border-color 0.15s, color 0.15s;
    -webkit-tap-highlight-color: transparent;
    appearance: none;
    -webkit-appearance: none;
  }

  .filter-toggle.active,
  .filter-toggle:hover {
    background: #16a34a;
    border-color: #16a34a;
    color: #fff;
  }

  .filter-badge {
    position: absolute;
    top: 2px;
    right: 2px;
    min-width: 16px;
    height: 16px;
    padding: 0 4px;
    border-radius: 100px;
    background: #dc2626;
    color: #fff;
    font-size: 0.65rem;
    font-weight: 700;
    line-height: 16px;
    text-align: center;
  }

  .search-btn {
    flex-shrink: 0;
    width: 44px;
    height: 44px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #16a34a;
    border: none;
    border-radius: 50%;
    color: #fff;
    font-size: 0.82rem;
    cursor: pointer;
    transition: background 0.15s;
    -webkit-tap-highlight-color: transparent;
    appearance: none;
    -webkit-appearance: none;
  }

  .search-btn:hover { background: #15803d; }
  .search-btn:active { background: #166534; }
  .search-btn:focus-visible { outline: 2px solid #16a34a; outline-offset: 2px; }

  /* ── Collapsible panel ──────────────────────────────────────────────────── */
  .filter-panel {
    display: grid;
    grid-template-rows: 0fr;
    transition: grid-template-rows 0.22s ease, margin-top 0.22s ease;
    margin-top: 0;
  }

  .filter-panel.open {
    grid-template-rows: 1fr;
    margin-top: 10px;
  }

  /* Collapse container — no visual styling so no border artifact at 0 height */
  .filter-panel-inner {
    min-height: 0;
    overflow: hidden;
  }

  /* Visual card — clipped by filter-panel-inner when collapsed */
  .filter-panel-body {
    background: #fff;
    border: 1.5px solid #d1e7d9;
    border-radius: 14px;
    padding: 14px;
  }

  .filter-section { margin-bottom: 14px; }
  .filter-section:last-of-type { margin-bottom: 0; }

  .filter-label {
    font-size: 0.78rem;
    font-weight: 600;
    color: #4b7a5e;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    margin-bottom: 8px;
  }

  .chip-row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 5px 12px;
    border-radius: 100px;
    border: 1.5px solid #d1e7d9;
    background: #f5f7f5;
    color: #4b7a5e;
    font-size: 0.82rem;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s, border-color 0.15s, color 0.15s;
    -webkit-tap-highlight-color: transparent;
    appearance: none;
    -webkit-appearance: none;
    min-height: 34px;
  }

  .chip:hover:not(.active) {
    background: #e8f5ec;
    border-color: #86efac;
  }

  .chip.active {
    background: #16a34a;
    border-color: #16a34a;
    color: #fff;
  }

  .filter-actions {
    display: flex;
    gap: 8px;
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid #e7ede9;
  }

  .btn-reset {
    flex: 1;
    height: 44px;
    border: 1.5px solid #d1e7d9;
    border-radius: 100px;
    background: #fff;
    color: #4b7a5e;
    font-size: 0.88rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s;
    -webkit-tap-highlight-color: transparent;
    appearance: none;
    -webkit-appearance: none;
  }

  .btn-reset:hover { background: #f0fdf4; }

  .btn-apply {
    flex: 2;
    height: 44px;
    border: none;
    border-radius: 100px;
    background: #16a34a;
    color: #fff;
    font-size: 0.9rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s;
    -webkit-tap-highlight-color: transparent;
    appearance: none;
    -webkit-appearance: none;
  }

  .btn-apply:hover { background: #15803d; }
  .btn-apply:active { background: #166534; }
</style>
