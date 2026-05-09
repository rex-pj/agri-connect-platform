<script lang="ts">
    import { getProductDisplay } from '$lib/categoryIcons';
    import Button from '$lib/components/atoms/Button.svelte';
    import Heading from '$lib/components/atoms/Heading.svelte';
    import Text from '$lib/components/atoms/Text.svelte';
    import type { PageData } from './$types';
    import { ROUTES } from '$lib/routes';

    const { data }: { data: PageData } = $props();
    const p = data.product;

    const telHref  = `tel:${p.vendor_phone}`;
    const catIcon  = getProductDisplay(p.category).icon;

    function fmtPrice(v: number): string {
        return v.toLocaleString('vi-VN') + ' ₫';
    }
</script>

<!-- Hero image or gradient banner -->
{#if p.image_url}
    <div class="img-hero">
        <Button href={ROUTES.PRODUCTS} class="back-link back-link--overlay">
            <i class="fa-solid fa-arrow-left"></i> Sản phẩm
        </Button>
        <img src={p.image_url} alt={p.name} class="img-hero__img" />
    </div>
    <div class="page-hero page-hero--compact">
        <Heading level={1} class="page-hero__title">{p.name}</Heading>
        <span class="cat-badge">
            <i class="fa-solid {catIcon}"></i> {p.category}
        </span>
    </div>
{:else}
    <div class="page-hero page-hero--detail">
        <Button href={ROUTES.PRODUCTS} class="back-link">
            <i class="fa-solid fa-arrow-left"></i> Sản phẩm
        </Button>
        <Heading level={1} class="page-hero__title">{p.name}</Heading>
        <span class="cat-badge">
            <i class="fa-solid {catIcon}"></i> {p.category}
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
        <Text class="price-note">Giá tham khảo — liên hệ nhà cung cấp để biết giá chính xác</Text>
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
            <i class="fa-solid fa-phone"></i> GỌI TƯ VẤN NGAY
        </Button>
    </div>

</div>

<style>
/* Image hero variant */
.img-hero     { position: relative; width: 100%; aspect-ratio: 16/9; overflow: hidden; background: #f3f4f6; }
.img-hero__img { width: 100%; height: 100%; object-fit: cover; display: block; }

/* Category badge in hero */
.cat-badge { display: inline-flex; align-items: center; gap: 6px; background: rgba(255,255,255,.15); padding: 5px 14px; border-radius: 20px; font-size: .8rem; font-weight: 600; }
.page-hero__title { margin-bottom: 10px; }

/* Price card */
.price-card { background: #fff; border-radius: 16px; padding: 18px; box-shadow: 0 1px 6px rgba(0,0,0,.07); border: 1px solid var(--border, #e5e7eb); }
.badge-oos  { display: inline-block; background: #fee2e2; color: #b91c1c; padding: 4px 12px; border-radius: 6px; font-size: .8rem; font-weight: 700; margin-bottom: 10px; }
.price-row  { display: flex; align-items: baseline; gap: 6px; margin-bottom: 6px; }
.price          { font-size: 1.8rem; font-weight: 800; color: #ea580c; }
.price--contact { font-size: 1.3rem; color: #b45309; }
.unit           { font-size: .95rem; color: #6b7280; }
:global(.price-note)     { font-size: .78rem; color: #9ca3af; margin: 0; }

/* Vendor */
.vendor-row  { display: flex; align-items: center; gap: 12px; margin-bottom: 14px; }
.vendor-icon { width: 46px; height: 46px; background: var(--p50, #ecfdf5); border-radius: 12px; display: flex; align-items: center; justify-content: center; font-size: 1.25rem; color: var(--p700, #065f46); flex-shrink: 0; }
.vendor-info { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
.vendor-name  { font-size: .95rem; font-weight: 700; color: var(--text, #0c1a12); }
.vendor-phone { font-size: .85rem; color: var(--muted, #5e7a68); }
:global(.call-btn) { display: flex; align-items: center; justify-content: center; gap: 8px; min-height: 52px; background: var(--p700, #065f46); color: #fff; border-radius: 12px; text-decoration: none; font-size: .95rem; font-weight: 800; -webkit-tap-highlight-color: transparent; }
:global(.call-btn):active { opacity: .88; }
</style>
