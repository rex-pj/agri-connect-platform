<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import ProviderCard   from "$lib/components/organisms/ProviderCard.svelte";
  import ProductCard    from "$lib/components/organisms/ProductCard.svelte";
  import FilterChips    from "$lib/components/molecules/FilterChips.svelte";
  import CategoryGrid   from "$lib/components/organisms/CategoryGrid.svelte";
  import ContactSheet   from "$lib/components/organisms/ContactSheet.svelte";
  import AlertBanner    from "$lib/components/atoms/AlertBanner.svelte";
  import EmptyState     from "$lib/components/molecules/EmptyState.svelte";
  import SectionHeader  from "$lib/components/molecules/SectionHeader.svelte";
  import { PRODUCT_DISPLAY, SERVICE_DISPLAY, DEFAULT_DISPLAY } from '$lib/categoryIcons';
  import Spinner from '$lib/components/atoms/Spinner.svelte';
  import Heading from '$lib/components/atoms/Heading.svelte';
  import Text from '$lib/components/atoms/Text.svelte';
  import Button from '$lib/components/atoms/Button.svelte';
  import type { PageData, ActionData } from "./$types";
  import type { Provider } from "./+page.server";

  const { data, form }: { data: PageData; form: ActionData } = $props();

  let locating    = $state(false);
  let locError    = $state<string | null>(null);
  let selectedSvcId = $state(page.url.searchParams.get("service_id") ?? "");

  function detectLocation() {
    if (!navigator.geolocation) {
      locError = "Trình duyệt không hỗ trợ định vị. Vui lòng dùng Chrome hoặc Safari.";
      return;
    }
    locating = true;
    locError = null;
    navigator.geolocation.getCurrentPosition(
      ({ coords }) => {
        const params = new URLSearchParams(page.url.searchParams);
        params.set("lat", coords.latitude.toFixed(6));
        params.set("lng", coords.longitude.toFixed(6));
        if (selectedSvcId) params.set("service_id", selectedSvcId);
        goto(`/agri-connect?${params.toString()}`, { replaceState: false });
        locating = false;
      },
      (err) => {
        locating = false;
        locError = err.code === 1
          ? "Bạn đã chặn quyền định vị. Vui lòng cho phép trong cài đặt trình duyệt."
          : "Không lấy được vị trí. Hãy thử lại hoặc kiểm tra GPS.";
      },
      { timeout: 10_000, enableHighAccuracy: true },
    );
  }

  function handleFilter(id: string) {
    selectedSvcId = id;
    const params = new URLSearchParams(page.url.searchParams);
    if (id) params.set("service_id", id);
    else params.delete("service_id");
    goto(`/agri-connect?${params.toString()}`, { replaceState: false });
  }

  let activeProvider = $state<Provider | null>(null);
  let contactSuccess = $state(false);

  function openContact(p: Provider) { activeProvider = p; contactSuccess = false; }

  $effect(() => {
    if (form?.success) { contactSuccess = true; activeProvider = null; }
  });
</script>

<svelte:head>
  <title>Tìm dịch vụ gần bạn — Agri-Connect</title>
  <meta name="description" content="Tìm nhà cung cấp dịch vụ nông nghiệp gần bạn nhất: cơ giới, phân bón, thuốc BVTV, máy bay phun thuốc và nhiều hơn nữa. Bật định vị để lọc theo khoảng cách." />
</svelte:head>

<!-- ═══════════════════════════════════════════════════════════════════
     LOCATION ONBOARDING — shown when no lat/lng yet
     ═══════════════════════════════════════════════════════════════════ -->
{#if !data.hasLocation}
  <section class="loc-hero">
    <div class="loc-hero__inner">
      <div class="loc-ripple">
        <span class="ripple r1"></span>
        <span class="ripple r2"></span>
        <span class="ripple r3"></span>
        <span class="loc-icon">
          <i class="fa-solid fa-location-dot loc-icon__glyph"></i>
        </span>
      </div>

      <Heading level={2} class="loc-hero__title">Tìm thợ gần bạn</Heading>
      <Text class="loc-hero__sub">
        Cho phép xác định vị trí để tìm nhà cung cấp<br/>
        trong bán kính <strong class="loc-hero__em">10 km</strong> từ bạn
      </Text>

      <Button type="button" class="loc-btn" onclick={detectLocation} disabled={locating}>
        {#if locating}
          <Spinner /> Đang xác định vị trí…
        {:else}
          <i class="fa-solid fa-crosshairs"></i> XÁC ĐỊNH VỊ TRÍ CỦA TÔI
        {/if}
      </Button>

      {#if locError}
        <Text class="loc-error"><i class="fa-solid fa-triangle-exclamation"></i> {locError}</Text>
      {/if}
      <Text class="loc-privacy"><i class="fa-solid fa-shield-halved"></i> Vị trí chỉ dùng để tìm thợ, không được lưu lại</Text>
    </div>
  </section>
{/if}

<!-- ═══════════════════════════════════════════════════════════════════
     LOCATION HERO — compact bar when position is known
     ═══════════════════════════════════════════════════════════════════ -->
{#if data.hasLocation}
  <div class="page-hero">
    <div class="page-hero__row">
      <Heading level={1} class="page-hero__title">
        <i class="fa-solid fa-location-dot hero-icon--yellow"></i> Tìm dịch vụ gần tôi
      </Heading>
      <Button type="button" class="page-hero__refresh" onclick={detectLocation} disabled={locating} aria-label="Cập nhật vị trí">
        {#if locating}
          <Spinner />
        {:else}
          <i class="fa-solid fa-rotate"></i>
        {/if}
        Cập nhật
      </Button>
    </div>
    <Text class="page-hero__sub">
      <i class="fa-solid fa-circle-check hero-icon--check"></i>Đã xác định vị trí
    </Text>
  </div>

  <FilterChips
    options={data.serviceCategories.map(c => ({ value: String(c.id), label: c.name }))}
    selected={selectedSvcId}
    onSelect={handleFilter}
  />
{/if}

<!-- ═══════════════════════════════════════════════════════════════════
     SERVICE CATEGORY GRID
     ═══════════════════════════════════════════════════════════════════ -->
{#if data.serviceCategories.length > 0}
<div class="cat-grid-wrap">
  <Text class="cat-label">Danh mục dịch vụ</Text>
  <CategoryGrid items={data.serviceCategories.map(cat => {
    const d = SERVICE_DISPLAY[cat.slug] ?? DEFAULT_DISPLAY;
    return { name: cat.name, icon: d.icon, active: selectedSvcId === String(cat.id), onSelect: () => handleFilter(selectedSvcId === String(cat.id) ? '' : String(cat.id)) };
  })} />
</div>
{/if}

<!-- ═══════════════════════════════════════════════════════════════════
     GEO-SEARCH RESULTS
     ═══════════════════════════════════════════════════════════════════ -->
{#if data.hasLocation}
  <section class="p-3">
    {#if data.error}
      <AlertBanner type="error" message={data.error} />
    {/if}
    {#if contactSuccess}
      <AlertBanner type="success" message="Đã gửi yêu cầu! Nhà cung cấp sẽ liên hệ lại với bạn sớm." />
    {/if}

    {#if data.providers.length === 0 && !data.error}
      <EmptyState
        icon="fa-magnifying-glass"
        title="Không tìm thấy dịch vụ trong bán kính 10 km"
        hint="Hãy thử bỏ bộ lọc hoặc mở rộng phạm vi."
      />
    {:else}
      <Text class="small text-muted mb-3"><strong>{data.providers.length}</strong> nhà cung cấp gần bạn</Text>
      <div class="row row-cols-1 row-cols-md-2 row-cols-lg-3 g-3">
        {#each data.providers as provider (provider.id)}
          <div class="col">
            <ProviderCard {provider} onContact={() => openContact(provider)} />
          </div>
        {/each}
      </div>
    {/if}
  </section>
{/if}

<!-- ═══════════════════════════════════════════════════════════════════
     FEATURED PROVIDERS — only shown when no location is set
     ═══════════════════════════════════════════════════════════════════ -->
{#if !data.hasLocation && data.featured_providers.length > 0}
  <section class="p-3">
    <SectionHeader title="Nhà cung cấp nổi bật" moreHref="/services" />
    <div class="row row-cols-1 row-cols-md-2 row-cols-lg-3 g-3">
      {#each data.featured_providers as provider (provider.id)}
        <div class="col"><ProviderCard {provider} /></div>
      {/each}
    </div>
  </section>
{/if}

<!-- ═══════════════════════════════════════════════════════════════════
     PRODUCT CATEGORY GRID
     ═══════════════════════════════════════════════════════════════════ -->
{#if data.productCategories.length > 0}
<div class="cat-grid-wrap">
  <Text class="cat-label">Danh mục sản phẩm</Text>
  <CategoryGrid items={data.productCategories.map(cat => {
    const d = PRODUCT_DISPLAY[cat.slug] ?? DEFAULT_DISPLAY;
    return { name: cat.name, icon: d.icon, href: `/products/category/${cat.id}/${cat.slug}` };
  })} />
</div>
{/if}

<!-- ═══════════════════════════════════════════════════════════════════
     FEATURED PRODUCTS
     ═══════════════════════════════════════════════════════════════════ -->
{#if data.featured_products.length > 0}
  <section class="p-3">
    <SectionHeader title="Sản phẩm nông nghiệp" moreHref="/products" />
    <div class="row row-cols-2 row-cols-md-3 row-cols-lg-4 g-3">
      {#each data.featured_products as product (product.id)}
        <div class="col"><ProductCard {product} /></div>
      {/each}
    </div>
  </section>
{/if}

<!-- ═══════════════════════════════════════════════════════════════════
     CONTACT SHEET
     ═══════════════════════════════════════════════════════════════════ -->
{#if activeProvider}
  <ContactSheet provider={activeProvider} {form} onClose={() => activeProvider = null} />
{/if}

<style>
  /* ── Location ripple ──────────────────────────────── */
  .loc-ripple {
    position: relative;
    width: 88px; height: 88px;
    display: flex; align-items: center; justify-content: center;
    margin: 0 auto 24px;
  }
  .ripple {
    position: absolute;
    border-radius: 50%;
    border: 3px solid var(--p500, #059669);
    animation: pulse 2.4s ease-out infinite;
  }
  .r1 { width: 88px;  height: 88px;  animation-delay: 0s;   }
  .r2 { width: 120px; height: 120px; animation-delay: .7s;  }
  .r3 { width: 152px; height: 152px; animation-delay: 1.4s; }
  @keyframes pulse {
    0%   { opacity: .8; transform: scale(.75); }
    100% { opacity: 0;  transform: scale(1.1); }
  }
  .loc-icon {
    width: 68px; height: 68px;
    background: var(--p50, #ECFDF5);
    border-radius: 50%;
    display: flex; align-items: center; justify-content: center;
    box-shadow: 0 4px 24px rgba(5,150,105,.3);
    z-index: 1;
  }
  .loc-icon__glyph { font-size: 2.4rem; color: var(--p700, #065F46); }
  .loc-hero__em    { color: #1B6B3A; }
  .hero-icon--yellow { color: #FCD34D; }
  .hero-icon--check  { color: #6EE7B7; margin-right: 4px; }

  /* ── Location onboarding hero ─────────────────────── */
  .loc-hero {
    background: #fff;
    text-align: center;
    padding: 40px 24px 32px;
    border-bottom: 1px solid var(--border, #C8D8CE);
    box-shadow: 0 2px 12px rgba(0,0,0,.06);
  }
  .loc-hero__inner { max-width: 420px; margin: 0 auto; }
  :global(.loc-hero__title) { font-size: 1.6rem; font-weight: 800; margin: 0 0 8px; letter-spacing: -.02em; }
  :global(.loc-hero__sub)   { font-size: .9rem; color: #5E7A68; margin: 0 0 24px; line-height: 1.6; }
  :global(.loc-btn) {
    width: 100%; min-height: 52px;
    display: flex; align-items: center; justify-content: center; gap: 10px;
    background: #F59E0B; color: #1C1917;
    border: none; border-radius: 14px;
    font-size: 1rem; font-weight: 800; letter-spacing: .04em;
    cursor: pointer;
    transition: transform .1s;
  }
  :global(.loc-btn):active { transform: scale(.98); }
  :global(.loc-btn):disabled { opacity: .7; cursor: not-allowed; }
  :global(.loc-error)   { font-size: .85rem; color: #B91C1C; margin: 12px 0 0; display: flex; align-items: center; justify-content: center; gap: 6px; }
  :global(.loc-privacy) { font-size: .8rem; color: #5E7A68; margin: 12px 0 0; display: flex; align-items: center; justify-content: center; gap: 6px; }

  /* ── Location known hero: refresh button only ────────── */
  :global(.page-hero__refresh) {
    flex-shrink: 0;
    display: inline-flex; align-items: center; gap: 6px;
    padding: 8px 14px; min-height: 40px;
    background: rgba(255,255,255,.12); color: #fff;
    border: 1.5px solid rgba(255,255,255,.25); border-radius: 10px;
    font-size: .82rem; font-weight: 600;
    cursor: pointer; white-space: nowrap;
    transition: background .15s;
  }
  :global(.page-hero__refresh):disabled { opacity: .6; cursor: not-allowed; }

  /* ── Category grid ────────────────────────────────── */
  .cat-grid-wrap { padding: 16px 16px 0; }
  :global(.cat-label) {
    font-size: .68rem; font-weight: 700;
    color: var(--muted, #5E7A68);
    text-transform: uppercase; letter-spacing: .08em;
    margin: 0 0 12px;
  }
</style>
