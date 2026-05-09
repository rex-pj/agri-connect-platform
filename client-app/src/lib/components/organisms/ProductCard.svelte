<script context="module" lang="ts">
  export interface ProductCardData {
    id: number;
    name: string;
    description: string | null;
    price: number | null;
    unit: string | null;
    image_url: string | null;
    category: string;
    vendor_name: string;
    vendor_phone: string;
    is_available: boolean;
  }
</script>

<script lang="ts">
  import Badge from '$lib/components/atoms/Badge.svelte';
  import Button from '$lib/components/atoms/Button.svelte';
  import Text from '$lib/components/atoms/Text.svelte';
  import Heading from '$lib/components/atoms/Heading.svelte';
  import { PRODUCT_NAME_ICON } from '$lib/categoryIcons';
  import { slugify } from '$lib/slugify';

  const { product }: { product: ProductCardData } = $props();
  const telHref    = $derived(`tel:${product?.vendor_phone ?? ''}`);
  const icon       = $derived(PRODUCT_NAME_ICON[product?.category ?? ''] ?? 'fa-tag');
  const detailHref = $derived(`/products/${product.id}/${slugify(product.name)}`);

  function fmtPrice(p: number): string {
    return p.toLocaleString('vi-VN') + ' ₫';
  }
</script>

<div class="card h-100 product-card hover-lift" class:opacity-75={!product.is_available}>
  <!-- 1:1 image / placeholder -->
  <div class="ratio ratio-1x1 product-card__img position-relative">
    {#if product.image_url}
      <img
        src={product.image_url}
        alt={product.name}
        class="object-fit-cover w-100 h-100"
        loading="lazy"
      />
    {:else}
      <div class="d-flex align-items-center justify-content-center bg-light">
        <i class="fa-solid {icon} fs-1 text-secondary opacity-50"></i>
      </div>
    {/if}
    {#if !product.is_available}
      <Badge
        variant="danger"
        class="position-absolute top-0 end-0 m-2"
      >Hết hàng</Badge>
    {/if}
  </div>

  <div class="card-body d-flex flex-column p-3 gap-2">
    <!-- Product name — stretched-link covers the whole card -->
    <Heading level={3} class="card-title fs-6 fw-bold mb-0 product-card__name">
      <Button href={detailHref} class="stretched-link text-decoration-none text-dark">
        {product.name}
      </Button>
    </Heading>

    <!-- Price -->
    <div class="d-flex align-items-baseline gap-1 mt-auto">
      {#if product.price != null}
        <span class="fw-bold product-card__price">
          {fmtPrice(product.price)}
        </span>
        {#if product.unit}
          <small class="text-muted">/{product.unit}</small>
        {/if}
      {:else}
        <span class="fw-bold product-card__price-contact">Liên hệ giá</span>
      {/if}
    </div>

    <!-- Vendor -->
    <Text class="text-muted small mb-0 d-flex align-items-center gap-1">
      <i class="fa-solid fa-shop"></i>
      <span class="text-truncate">{product.vendor_name}</span>
    </Text>

    <!-- CTA — raised above the stretched-link overlay -->
    <Button
      href={telHref}
      class="btn btn-success mt-1 d-flex align-items-center justify-content-center gap-2 cta-btn"
    >
      <i class="fa-solid fa-phone"></i> GỌI TƯ VẤN
    </Button>
  </div>
</div>

<style>
  .product-card {
    position: relative;
    overflow: hidden;
  }
  .product-card__img {
    border-bottom: 1px solid var(--border, #e5e7eb);
  }
  .product-card__name {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    line-height: 1.3;
  }
  .product-card__price {
    font-size: 1.15rem;
    color: #ea580c;
  }
  .product-card__price-contact {
    color: #b45309;
  }
  .cta-btn {
    position: relative;
    z-index: 2;
  }
</style>
