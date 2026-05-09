<script lang="ts">
    import { goto } from '$app/navigation';
    import { page } from '$app/state';
    import ProductCard    from '$lib/components/organisms/ProductCard.svelte';
    import CategoryGrid   from '$lib/components/organisms/CategoryGrid.svelte';
    import PageHero       from '$lib/components/molecules/PageHero.svelte';
    import AlertBanner    from '$lib/components/atoms/AlertBanner.svelte';
    import EmptyState     from '$lib/components/molecules/EmptyState.svelte';
    import PaginationNav  from '$lib/components/molecules/PaginationNav.svelte';
    import { PRODUCT_DISPLAY, DEFAULT_DISPLAY } from '$lib/categoryIcons';
    import Text from '$lib/components/atoms/Text.svelte';
    import type { PageData } from './$types';

    const { data }: { data: PageData } = $props();

    function goToPage(p: number) {
        const params = new URLSearchParams(page.url.searchParams);
        params.set('page', String(p));
        goto(`/products?${params.toString()}`, { replaceState: false });
    }
</script>

<svelte:head>
    <title>Sản phẩm nông nghiệp — Agri-Connect</title>
    <meta name="description" content="Khám phá các sản phẩm nông nghiệp từ nhà cung cấp uy tín tại Đồng bằng sông Cửu Long: gạo, trái cây, phân bón, thuốc bảo vệ thực vật và nhiều hơn nữa." />
</svelte:head>

<PageHero
    title="Sản phẩm nông nghiệp"
    icon="fa-bag-shopping"
    sub="{data.categories.map(c => c.name).join(' · ')} — từ các nhà cung cấp uy tín"
/>

{#if data.categories.length > 0}
<div class="px-3 pt-3">
    <CategoryGrid items={data.categories.map(cat => {
        const d = PRODUCT_DISPLAY[cat.slug] ?? DEFAULT_DISPLAY;
        return { name: cat.name, icon: d.icon, href: `/products/category/${cat.id}/${cat.slug}` };
    })} />
</div>
{/if}

<section class="p-3">
    {#if data.error}
        <AlertBanner type="error" message={data.error} />
    {/if}

    {#if data.result.products.length === 0 && !data.error}
        <EmptyState icon="fa-inbox" title="Chưa có sản phẩm nào" hint="Hãy thử chọn một danh mục bên trên." />
    {:else}
        <Text class="small text-muted mb-3"><strong>{data.result.total}</strong> sản phẩm</Text>

        <div class="row row-cols-2 row-cols-md-3 row-cols-lg-5 row-cols-xl-6 g-3">
            {#each data.result.products as product (product.id)}
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
