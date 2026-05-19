<script module lang="ts">
  export interface ProviderCardData {
    id: number;
    name: string;
    phone: string;
    address: string | null;
    services: string[];
    is_verified: boolean;
    distance_m?: number;
  }
</script>

<script lang="ts">
  import Badge from "$lib/components/atoms/Badge.svelte";
  import Tag from "$lib/components/atoms/Tag.svelte";
  import Button from "$lib/components/atoms/Button.svelte";
  import Heading from "$lib/components/atoms/Heading.svelte";
  import { slugify } from "$lib/slugify";

  const {
    provider,
    onContact,
  }: {
    provider: ProviderCardData;
    onContact?: () => void;
  } = $props();

  const telHref = $derived(`tel:${provider?.phone ?? ""}`);
  const detailHref = $derived(
    `/providers/${provider.id}/${slugify(provider.name)}`,
  );

  function fmtDist(m: number): string {
    return m >= 1000 ? `${(m / 1000).toFixed(1)} km` : `${Math.round(m)} m`;
  }

  function districtFromAddress(addr: string | null): string | null {
    if (!addr) return null;
    const parts = addr
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean);
    return parts.length >= 2 ? parts.slice(-2).join(", ") : (parts[0] ?? null);
  }

  const district = $derived(districtFromAddress(provider.address));
</script>

<div
  class="card h-100 provider-card hover-lift"
  class:provider-card--verified={provider.is_verified}
>
  <div class="card-body d-flex flex-column p-3 gap-2">
    <!-- Name + verified badge -->
    <div class="d-flex align-items-start gap-2">
      <Heading
        level={3}
        class="card-title fs-6 fw-bold mb-0 flex-grow-1 provider-card__name"
      >
        {provider.name}
      </Heading>
      {#if provider.is_verified}
        <Badge
          variant="primary"
          icon="fa-circle-check"
          class="d-inline-flex align-items-center gap-1 flex-shrink-0 py-1 px-2"
        >
          <span class="provider-card__badge-text">Xác minh</span>
        </Badge>
      {/if}
    </div>

    <!-- Distance + district -->
    {#if provider.distance_m != null}
      <p
        class="text-muted small mb-0 d-flex align-items-center gap-1 provider-card__loc"
      >
        <i class="fa-solid fa-location-dot"></i>
        Cách bạn <strong class="ms-1">{fmtDist(provider.distance_m)}</strong>
        {#if district}
          <span class="provider-card__sep">—</span>
          <span class="text-truncate">{district}</span>
        {/if}
      </p>
    {:else if district}
      <p
        class="text-muted small mb-0 d-flex align-items-center gap-1 provider-card__loc"
      >
        <i class="fa-solid fa-location-dot"></i>
        <span class="text-truncate">{district}</span>
      </p>
    {/if}

    <!-- Service tags -->
    <div class="d-flex flex-wrap gap-1 mt-1">
      {#each provider.services as svc}
        <Tag text={svc} />
      {/each}
    </div>

    <!-- Footer: phone + CTAs -->
    <div class="mt-auto pt-3 border-top">
      <div class="d-flex align-items-center justify-content-between mb-2">
        <small class="text-muted provider-card__phone-label"
          >Số điện thoại</small
        >
        <strong class="fs-6" title="Nhấn để sao chép: {provider.phone}"
          >{provider.phone}</strong
        >
      </div>
      <div class="d-grid gap-2 d-md-flex">
        <Button href={telHref} variant="primary" class="flex-md-grow-1">
          <i class="fa-solid fa-phone"></i> Gọi ngay
        </Button>
        <Button href={detailHref} variant="outline" class="flex-md-grow-1">
          Chi tiết <i class="fa-solid fa-chevron-right provider-card__chevron"
          ></i>
        </Button>
      </div>
    </div>
  </div>
</div>

<style>
  .provider-card {
    border-top: 3px solid var(--border, #e5e7eb) !important;
  }
  .provider-card--verified {
    border-top-color: var(--p600, #047857) !important;
  }
  :global(.provider-card__name) {
    line-height: 1.25;
  }
  .provider-card__badge-text {
    font-size: 0.7rem;
  }
  .provider-card__phone-label {
    font-size: 0.75rem;
  }
  .provider-card__chevron {
    font-size: 0.76rem;
  }
  .provider-card__loc {
    min-width: 0;
  }
  .provider-card__sep {
    flex-shrink: 0;
    opacity: 0.5;
  }
</style>
