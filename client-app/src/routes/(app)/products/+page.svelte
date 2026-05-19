<script lang="ts">
    import { goto } from '$app/navigation';
    import { page } from '$app/state';
    import { untrack } from 'svelte';
    import ProductCard   from '$lib/components/organisms/ProductCard.svelte';
    import PageHero      from '$lib/components/molecules/PageHero.svelte';
    import AlertBanner   from '$lib/components/atoms/AlertBanner.svelte';
    import EmptyState    from '$lib/components/molecules/EmptyState.svelte';
    import PaginationNav from '$lib/components/molecules/PaginationNav.svelte';
    import { PRODUCT_DISPLAY, DEFAULT_DISPLAY } from '$lib/categoryIcons';
    import { ROUTES } from '$lib/routes';
    import Text from '$lib/components/atoms/Text.svelte';
    import type { PageData } from './$types';

    const { data }: { data: PageData } = $props();

    // ── Local filter state (mirrors current URL, updated on apply) ──────────────
    let searchQuery  = $state(untrack(() => data.searchQuery ?? ''));
    let categoryId   = $state(untrack(() => data.categoryId ?? ''));
    let minPrice     = $state(untrack(() => data.minPrice ?? ''));
    let maxPrice     = $state(untrack(() => data.maxPrice ?? ''));
    let sort         = $state(untrack(() => data.sort ?? ''));
    let filtersOpen  = $state(false);

    // Sync local state when server data changes (navigation)
    $effect(() => {
        searchQuery = data.searchQuery ?? '';
        categoryId  = data.categoryId  ?? '';
        minPrice    = data.minPrice    ?? '';
        maxPrice    = data.maxPrice    ?? '';
        sort        = data.sort        ?? '';
    });

    const activeFilterCount = $derived(
        (data.categoryId ? 1 : 0) +
        (data.minPrice || data.maxPrice ? 1 : 0) +
        (data.sort ? 1 : 0)
    );

    function buildParams(overrides: Record<string, string> = {}) {
        const params = new URLSearchParams();
        const q   = overrides.q         ?? searchQuery.trim();
        const cat = overrides.category_id ?? categoryId;
        const min = overrides.min_price  ?? minPrice.trim();
        const max = overrides.max_price  ?? maxPrice.trim();
        const s   = overrides.sort       ?? sort;
        const pg  = overrides.page       ?? '1';
        if (q)   params.set('q', q);
        if (cat) params.set('category_id', cat);
        if (min) params.set('min_price', min);
        if (max) params.set('max_price', max);
        if (s)   params.set('sort', s);
        if (pg !== '1') params.set('page', pg);
        return params;
    }

    function submitSearch(e: SubmitEvent) {
        e.preventDefault();
        goto(`${ROUTES.PRODUCTS}?${buildParams()}`, { replaceState: false });
    }

    function applyFilters() {
        filtersOpen = false;
        goto(`${ROUTES.PRODUCTS}?${buildParams()}`, { replaceState: false });
    }

    function resetFilters() {
        categoryId = ''; minPrice = ''; maxPrice = ''; sort = '';
        filtersOpen = false;
        const params = new URLSearchParams();
        if (searchQuery.trim()) params.set('q', searchQuery.trim());
        goto(`${ROUTES.PRODUCTS}${params.size ? '?' + params : ''}`, { replaceState: false });
    }

    function goToPage(p: number) {
        const params = new URLSearchParams(page.url.searchParams);
        params.set('page', String(p));
        goto(`${ROUTES.PRODUCTS}?${params.toString()}`, { replaceState: false });
    }

    const SORT_OPTIONS = [
        { value: '',           label: 'Mới nhất' },
        { value: 'price_asc',  label: 'Giá tăng dần' },
        { value: 'price_desc', label: 'Giá giảm dần' },
    ];
</script>

<svelte:head>
    <title>Sản phẩm nông nghiệp — Agri-Connect</title>
    <meta name="description" content="Khám phá các sản phẩm nông nghiệp từ nhà cung cấp uy tín tại Đồng bằng sông Cửu Long: gạo, trái cây, phân bón, thuốc bảo vệ thực vật và nhiều hơn nữa." />
</svelte:head>

<PageHero
    title="Sản phẩm nông nghiệp"
    icon="fa-bag-shopping"
    sub="{data.categories.map(c => c.name).join(' · ')} — từ các nhà cung cấp uy tín"
    backLink={{ href: ROUTES.HOME, label: "Trang chủ" }}
/>

<!-- ── Search + filter panel ─────────────────────────────────────────────── -->
<div class="search-panel px-2 px-md-3 pt-2 pt-md-3">

    <!-- Search row -->
    <form class="search-row" onsubmit={submitSearch}>
        <div class="search-input-wrap">
            <i class="fa-solid fa-magnifying-glass search-icon"></i>
            <input
                class="search-input"
                type="search"
                bind:value={searchQuery}
                placeholder="Tìm sản phẩm..."
                aria-label="Tìm sản phẩm"
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

            <!-- Category chips -->
            <div class="filter-section">
                <p class="filter-label">Danh mục</p>
                <div class="chip-row">
                    <button
                        type="button"
                        class="chip{!categoryId ? ' active' : ''}"
                        onclick={() => (categoryId = '')}
                    >Tất cả</button>
                    {#each data.categories as cat}
                        {@const d = PRODUCT_DISPLAY[cat.slug] ?? DEFAULT_DISPLAY}
                        <button
                            type="button"
                            class="chip{categoryId === String(cat.id) ? ' active' : ''}"
                            onclick={() => (categoryId = String(cat.id))}
                        >
                            <i class="fa-solid {d.icon}"></i>
                            {cat.name}
                        </button>
                    {/each}
                </div>
            </div>

            <!-- Price range -->
            <div class="filter-section">
                <p class="filter-label">Khoảng giá (đồng)</p>
                <div class="price-row">
                    <input
                        class="price-input"
                        type="number"
                        min="0"
                        placeholder="Từ"
                        bind:value={minPrice}
                        aria-label="Giá tối thiểu"
                    />
                    <span class="price-sep">—</span>
                    <input
                        class="price-input"
                        type="number"
                        min="0"
                        placeholder="Đến"
                        bind:value={maxPrice}
                        aria-label="Giá tối đa"
                    />
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

        </div><!-- filter-panel-body -->
        </div><!-- filter-panel-inner -->
    </div><!-- filter-panel -->
</div>

<!-- ── Results ────────────────────────────────────────────────────────────── -->
<section class="p-2 p-md-3">
    {#if data.error}
        <AlertBanner type="error" message={data.error} />
    {/if}

    {#if data.result.items.length === 0 && !data.error}
        {#if data.searchQuery || data.categoryId || data.minPrice || data.maxPrice}
            <EmptyState
                icon="fa-bag-shopping"
                title="Không tìm thấy sản phẩm nào"
                hint="Thử từ khóa khác hoặc xóa bộ lọc để xem tất cả sản phẩm."
            />
        {:else}
            <EmptyState icon="fa-inbox" title="Chưa có sản phẩm nào" hint="Hãy thử chọn một danh mục bên trên." />
        {/if}
    {:else}
        <Text class="small text-muted mb-2">
            <strong>{data.result.total}</strong> sản phẩm{data.searchQuery ? ` cho "${data.searchQuery}"` : ''}
        </Text>

        <div class="row row-cols-2 row-cols-md-3 row-cols-lg-5 row-cols-xl-6 g-2 g-md-3">
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

<style>
    /* ── Search row ─────────────────────────────────────────────────────────── */
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

    /* ── Collapsible filter panel ───────────────────────────────────────────── */
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

    .filter-section {
        margin-bottom: 14px;
    }

    .filter-section:last-of-type {
        margin-bottom: 0;
    }

    .filter-label {
        font-size: 0.78rem;
        font-weight: 600;
        color: #4b7a5e;
        text-transform: uppercase;
        letter-spacing: 0.04em;
        margin-bottom: 8px;
    }

    /* ── Category / sort chips ──────────────────────────────────────────────── */
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

    /* ── Price range ────────────────────────────────────────────────────────── */
    .price-row {
        display: flex;
        align-items: center;
        gap: 8px;
    }

    .price-input {
        flex: 1;
        min-width: 0;
        height: 40px;
        padding: 0 12px;
        border: 1.5px solid #d1e7d9;
        border-radius: 8px;
        background: #f5f7f5;
        font-size: 0.88rem;
        color: #1a2e24;
        outline: none;
        transition: border-color 0.15s, background 0.15s;
    }

    .price-input:focus {
        border-color: #16a34a;
        background: #fff;
    }

    .price-input::placeholder { color: #9ab5a1; }

    .price-sep {
        color: #9ab5a1;
        font-size: 0.85rem;
        flex-shrink: 0;
    }

    /* ── Apply / reset buttons ──────────────────────────────────────────────── */
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
