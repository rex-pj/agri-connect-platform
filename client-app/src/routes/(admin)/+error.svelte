<script lang="ts">
  import { page } from "$app/state";
  import Heading from "$lib/components/atoms/Heading.svelte";
  import Text from "$lib/components/atoms/Text.svelte";
  import Button from "$lib/components/atoms/Button.svelte";
  import { ROUTES } from "$lib/routes";
</script>

<svelte:head>
  <title>{page.status} – Admin · Agri-Connect</title>
</svelte:head>

<div class="admin-error">
  <div class="admin-error__icon">
    {#if page.status === 404}
      <i class="fa-solid fa-file-circle-question"></i>
    {:else}
      <i class="fa-solid fa-triangle-exclamation"></i>
    {/if}
  </div>

  <Heading level={1} class="admin-error__code">{page.status}</Heading>

  <Text class="admin-error__msg">
    {#if page.status === 404}
      Trang quản trị không tồn tại.
    {:else if page.status === 403}
      Bạn không có quyền truy cập.
    {:else}
      Lỗi máy chủ. Vui lòng thử lại sau.
    {/if}
  </Text>

  {#if page.error?.message && page.status >= 500}
    <code class="admin-error__detail">{page.error.message}</code>
  {/if}

  <div class="admin-error__actions">
    <Button
      href={ROUTES.ADMIN.DASHBOARD}
      class="admin-error__btn admin-error__btn--primary"
    >
      <i class="fa-solid fa-gauge"></i>
      Dashboard
    </Button>
    <Button href={ROUTES.HOME} class="admin-error__btn">
      <i class="fa-solid fa-arrow-left"></i>
      Trang chủ
    </Button>
  </div>
</div>

<style>
  .admin-error {
    min-height: 100dvh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 40px 24px;
    text-align: center;
    background: #f8fafc;
    font-family:
      system-ui,
      -apple-system,
      sans-serif;
  }

  .admin-error__icon {
    font-size: 3rem;
    color: #94a3b8;
    margin-bottom: 16px;
  }

  .admin-error__code {
    font-size: 5rem;
    font-weight: 800;
    color: #1e293b;
    margin: 0 0 8px;
    line-height: 1;
  }

  .admin-error__msg {
    font-size: 1rem;
    color: #64748b;
    margin: 0;
  }

  .admin-error__detail {
    display: block;
    margin-top: 12px;
    font-size: 0.8rem;
    color: #94a3b8;
    font-family: monospace;
    background: #f1f5f9;
    padding: 8px 16px;
    border-radius: 8px;
    max-width: 480px;
    word-break: break-word;
  }

  .admin-error__actions {
    display: flex;
    gap: 12px;
    margin-top: 28px;
    flex-wrap: wrap;
    justify-content: center;
  }

  :global(.admin-error__btn) {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 12px 24px;
    border-radius: 8px;
    font-size: 0.9rem;
    font-weight: 600;
    text-decoration: none;
    background: #e2e8f0;
    color: #1e293b;
    transition: background 0.15s;
    min-height: 44px;
  }

  :global(.admin-error__btn:hover) {
    background: #cbd5e1;
  }

  :global(.admin-error__btn--primary) {
    background: var(--p700, #065f46);
    color: #fff;
  }

  :global(.admin-error__btn--primary:hover) {
    background: var(--p800, #064e3b);
  }
</style>
