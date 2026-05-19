<script lang="ts">
  import { getServiceDisplay } from "$lib/categoryIcons";
  import ContactSheet from "$lib/components/organisms/ContactSheet.svelte";
  import Button from "$lib/components/atoms/Button.svelte";
  import Heading from "$lib/components/atoms/Heading.svelte";
  import Text from "$lib/components/atoms/Text.svelte";
  import type { PageData, ActionData } from "./$types";
  import { ROUTES } from "$lib/routes";

  const { data, form }: { data: PageData; form: ActionData } = $props();
  const prov = data.provider;

  const telHref = `tel:${prov.phone}`;

  // ContactSheet expects services as string[]
  const contactProvider = $derived({
    id: prov.id,
    name: prov.name,
    services: prov.services.map((s) => s.name),
  });

  let sheetOpen = $state(false);
  let contactSuccess = $state(false);

  async function share() {
    const url = window.location.href;
    const text = `${prov.name} – dịch vụ nông nghiệp tại ${prov.address ?? "Đồng bằng sông Cửu Long"}`;
    if (navigator.share) {
      await navigator.share({ title: prov.name, text, url }).catch(() => {});
    } else {
      await navigator.clipboard.writeText(url);
      alert("Đã sao chép đường dẫn!");
    }
  }

  $effect(() => {
    if (form?.success) {
      contactSuccess = true;
      sheetOpen = false;
    }
  });
</script>

<svelte:head>
  <title>{prov.name} – Agri-Connect</title>
  <meta
    name="description"
    content={prov.description
      ? `${prov.description.slice(0, 150)}`
      : `Liên hệ ${prov.name} để biết thêm về dịch vụ nông nghiệp tại ${prov.address ?? "Đồng bằng sông Cửu Long"}.`}
  />
</svelte:head>

<div class="page-hero page-hero--detail">
  <Button href={ROUTES.SERVICES} class="back-link">
    <i class="fa-solid fa-arrow-left"></i> Dịch vụ
  </Button>
  <div class="hero-name-row">
    <Heading level={1} class="page-hero_title">{prov.name}</Heading>
    <div class="hero-actions">
      {#if prov.is_verified}
        <span class="verified-badge">
          <i class="fa-solid fa-circle-check"></i> Xác minh
        </span>
      {/if}
      <button
        type="button"
        class="share-btn"
        onclick={share}
        aria-label="Chia sẻ"
      >
        <i class="fa-solid fa-share-nodes"></i>
      </button>
    </div>
  </div>
  {#if prov.address}
    <Text class="hero-addr"
      ><i class="fa-solid fa-location-dot"></i> {prov.address}</Text
    >
  {/if}
</div>

<div class="detail-body">
  {#if contactSuccess}
    <div class="status-banner status-banner--success">
      <i class="fa-solid fa-circle-check"></i>
      <span>Đã gửi yêu cầu! Nhà cung cấp sẽ liên hệ lại sớm.</span>
    </div>
  {/if}

  <div class="section cta-card">
    <div class="phone-row">
      <i class="fa-solid fa-phone"></i>
      <strong>{prov.phone}</strong>
    </div>
    <div class="cta-row">
      <Button href={telHref} class="btn-call">
        <i class="fa-solid fa-phone"></i> Gọi ngay
      </Button>
      <button
        type="button"
        class="btn-contact"
        onclick={() => {
          sheetOpen = true;
          contactSuccess = false;
        }}
      >
        <i class="fa-solid fa-comment-dots"></i> Gửi yêu cầu
      </button>
    </div>
  </div>

  {#if prov.description}
    <div class="section">
      <Heading level={2} class="section__title">Giới thiệu</Heading>
      <Text class="section__body">{prov.description}</Text>
    </div>
  {/if}

  {#if prov.services.length > 0}
    <div class="section">
      <Heading level={2} class="section__title">Dịch vụ cung cấp</Heading>
      <div class="svc-list">
        {#each prov.services as svc}
          {@const d = getServiceDisplay(svc.category)}
          <div class="svc-item">
            <span class="svc-icon"><i class="fa-solid {d.icon}"></i></span>
            <div class="svc-info">
              <div class="svc-header">
                <strong class="svc-name">{svc.name}</strong>
                <span class="svc-cat">{svc.category}</span>
              </div>
              {#if svc.description}
                <Text class="svc-desc">{svc.description}</Text>
              {/if}
              {#if svc.price_note}
                <span class="svc-price">
                  <i class="fa-solid fa-tag"></i>
                  {svc.price_note}
                </span>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>

{#if sheetOpen}
  <ContactSheet
    provider={contactProvider}
    {form}
    onClose={() => (sheetOpen = false)}
  />
{/if}

<style>
  /* Hero additions */
  .hero-name-row {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    flex-wrap: wrap;
    margin-bottom: 8px;
  }
  .page-hero__title {
    flex: 1;
  }
  .hero-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }
  .verified-badge {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    background: rgba(52, 211, 153, 0.25);
    color: #a7f3d0;
    border: 1px solid rgba(167, 243, 208, 0.3);
    padding: 5px 12px;
    border-radius: 20px;
    font-size: 0.8rem;
    font-weight: 700;
    white-space: nowrap;
  }
  .verified-badge i {
    color: #6ee7b7;
    font-size: 0.9rem;
  }
  .share-btn {
    width: 44px;
    height: 44px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.15);
    color: #fff;
    border: 1.5px solid rgba(255, 255, 255, 0.25);
    border-radius: 50%;
    cursor: pointer;
    font-size: 0.95rem;
    transition: background 0.15s;
  }
  .share-btn:hover {
    background: rgba(255, 255, 255, 0.25);
  }
  :global(.hero-addr) {
    font-size: 0.85rem;
    margin: 0;
    opacity: 0.8;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  /* status banner */
  .status-banner {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 14px;
    border-radius: 12px;
    font-size: 0.9rem;
    font-weight: 500;
  }
  .status-banner--success {
    background: #e8f5e9;
    color: #1b5e20;
  }

  /* CTA card */
  .cta-card {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .phone-row {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 1.1rem;
    color: var(--text, #0c1a12);
  }
  .phone-row i {
    color: var(--p700, #065f46);
    font-size: 1rem;
  }
  .cta-row {
    display: flex;
    gap: 10px;
  }
  :global(.btn-call) {
    flex: 1;
    min-height: 52px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    background: var(--p700, #065f46);
    color: #fff;
    border-radius: 12px;
    text-decoration: none;
    font-size: 0.95rem;
    font-weight: 800;
    -webkit-tap-highlight-color: transparent;
  }
  :global(.btn-call):active {
    opacity: 0.88;
    color: #fff;
  }
  :global(.btn-contact) {
    flex: 1;
    min-height: 52px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    background: #fff;
    color: var(--p700, #065f46);
    border: 1.5px solid var(--p700, #065f46);
    border-radius: 12px;
    font-size: 0.95rem;
    font-weight: 700;
    cursor: pointer;
    -webkit-tap-highlight-color: transparent;
  }
  :global(.btn-contact):active {
    background: var(--p50, #ecfdf5);
    color: var(--p700, #065f46);
  }

  /* Service list */
  .svc-list {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .svc-item {
    display: flex;
    gap: 12px;
  }
  .svc-icon {
    width: 44px;
    height: 44px;
    flex-shrink: 0;
    background: var(--p50, #ecfdf5);
    border-radius: 12px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1.15rem;
    color: var(--p700, #065f46);
  }
  .svc-info {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
    flex: 1;
  }
  .svc-header {
    display: flex;
    align-items: baseline;
    gap: 8px;
    flex-wrap: wrap;
  }
  .svc-name {
    font-size: 0.95rem;
    font-weight: 700;
    color: var(--text, #0c1a12);
  }
  .svc-cat {
    font-size: 0.75rem;
    color: var(--muted, #5e7a68);
    background: #f3f4f6;
    padding: 2px 8px;
    border-radius: 8px;
    white-space: nowrap;
  }
  :global(.svc-desc) {
    font-size: 0.85rem;
    color: #4b5563;
    line-height: 1.5;
    margin: 0;
  }
  .svc-price {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--a600, #d97706);
    background: var(--a50, #fffbeb);
    padding: 3px 10px;
    border-radius: 8px;
    width: fit-content;
  }
</style>
