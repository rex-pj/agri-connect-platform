<script lang="ts">
  import { enhance } from "$app/forms";
  import Heading from "$lib/components/atoms/Heading.svelte";
  import Text from "$lib/components/atoms/Text.svelte";
  import type { ActionData } from "./$types";

  const { form }: { form: ActionData } = $props();

  let seeding  = $state(false);
  let skipping = $state(false);

  const seedItems = [
    { icon: 'fa-solid fa-store',   label: '6 nhà cung cấp',          desc: 'Vùng Cái Lậy, Tiền Giang' },
    { icon: 'fa-solid fa-wrench',  label: '6 dịch vụ nông nghiệp',   desc: 'Múc bùn, phun thuốc, sửa máy' },
    { icon: 'fa-solid fa-box',     label: '10 sản phẩm',             desc: 'Nông sản, phân bón, nông cụ' },
  ];
</script>

<svelte:head>
  <title>Nạp dữ liệu mẫu — Agri-Connect</title>
  <meta name="robots" content="noindex, nofollow" />
</svelte:head>

<div class="auth-shell">
  <div class="auth-card">

    <div class="auth-brand">
      <i class="fa-solid fa-seedling auth-brand__icon"></i>
      <span class="auth-brand__name">Agri Admin</span>
    </div>

    <!-- Step indicator -->
    <div class="steps">
      <div class="steps__item steps__item--done">
        <span class="steps__dot"><i class="fa-solid fa-check"></i></span>
        <span class="steps__label">Tài khoản</span>
      </div>
      <div class="steps__line steps__line--done"></div>
      <div class="steps__item steps__item--active">
        <span class="steps__dot">2</span>
        <span class="steps__label">Dữ liệu mẫu</span>
      </div>
    </div>

    <Heading level={1} class="auth-title">Nạp dữ liệu mẫu</Heading>
    <Text class="auth-sub">
      Bước 2/2 — Thêm dữ liệu mẫu để kiểm thử giao diện. Bạn có thể bỏ qua và thêm sau.
    </Text>

    {#if form?.error}
      <div class="alert alert-danger py-2 small mb-3">
        <i class="fa-solid fa-triangle-exclamation me-1"></i>{form.error}
      </div>
    {/if}

    <!-- Preview of what will be seeded -->
    <div class="seed-preview">
      {#each seedItems as item}
        <div class="seed-preview__item">
          <div class="seed-preview__icon">
            <i class={item.icon}></i>
          </div>
          <div>
            <p class="seed-preview__label">{item.label}</p>
            <p class="seed-preview__desc">{item.desc}</p>
          </div>
        </div>
      {/each}
    </div>

    <div class="seed-actions">
      <form
        method="POST"
        action="?/seed"
        use:enhance={() => {
          seeding = true;
          return async ({ update }) => { seeding = false; await update(); };
        }}
      >
        <button type="submit" class="btn btn-success btn-lg w-100" disabled={seeding || skipping}>
          {#if seeding}
            <span class="spinner-border spinner-border-sm me-2"></span>Đang nạp dữ liệu…
          {:else}
            <i class="fa-solid fa-cloud-arrow-down me-2"></i>Nạp dữ liệu &amp; Vào Dashboard
          {/if}
        </button>
      </form>

      <form
        method="POST"
        action="?/skip"
        use:enhance={() => {
          skipping = true;
          return async ({ update }) => { skipping = false; await update(); };
        }}
      >
        <button type="submit" class="btn btn-outline-secondary btn-lg w-100" disabled={seeding || skipping}>
          {#if skipping}
            <span class="spinner-border spinner-border-sm me-2"></span>
          {:else}
            <i class="fa-solid fa-forward me-2"></i>
          {/if}
          Bỏ qua, vào Dashboard
        </button>
      </form>
    </div>

  </div>
</div>

<style>
  :global(body) { padding-top: 0 !important; padding-bottom: 0 !important; }

  /* ── Step indicator ──────────────────────────────── */
  .steps {
    display: flex;
    align-items: center;
    margin-bottom: 24px;
  }
  .steps__item {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    flex: 0 0 auto;
  }
  .steps__dot {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.85rem;
    font-weight: 700;
  }
  .steps__item--done   .steps__dot { background: #16a34a; color: #fff; }
  .steps__item--active .steps__dot { background: #16a34a; color: #fff; }
  .steps__label {
    font-size: 0.72rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .steps__item--done   .steps__label { color: #16a34a; }
  .steps__item--active .steps__label { color: #16a34a; }
  .steps__line {
    flex: 1;
    height: 2px;
    background: #e5e7eb;
    margin: 0 8px;
    margin-bottom: 20px;
  }
  .steps__line--done { background: #16a34a; }

  /* ── Seed preview ────────────────────────────────── */
  .seed-preview {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin: 20px 0 24px;
  }
  .seed-preview__item {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 14px;
    background: #f9fafb;
    border: 1px solid #e5e7eb;
    border-radius: 10px;
  }
  .seed-preview__icon {
    width: 38px;
    height: 38px;
    border-radius: 8px;
    background: #dcfce7;
    color: #16a34a;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1rem;
    flex-shrink: 0;
  }
  .seed-preview__label {
    font-size: 0.9rem;
    font-weight: 600;
    color: #0c1a12;
    margin: 0 0 2px;
  }
  .seed-preview__desc {
    font-size: 0.78rem;
    color: #5e7a68;
    margin: 0;
  }

  /* ── Action buttons ──────────────────────────────── */
  .seed-actions {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
</style>
