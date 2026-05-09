<script lang="ts">
  import { page } from '$app/state';
  import Button from '$lib/components/atoms/Button.svelte';
  import Heading from '$lib/components/atoms/Heading.svelte';
  import Text from '$lib/components/atoms/Text.svelte';
  import { ROUTES } from '$lib/routes';
</script>

<svelte:head>
  <title>{page.status} – Agri-Connect</title>
</svelte:head>

<div class="error-page">
  <div class="error-card">
    <div class="error-icon">
      {#if page.status === 404}
        <i class="fa-solid fa-map-location-dot"></i>
      {:else}
        <i class="fa-solid fa-triangle-exclamation"></i>
      {/if}
    </div>

    <Heading level={1} class="error-code">{page.status}</Heading>

    <Text class="error-message">
      {#if page.status === 404}
        Trang bạn tìm không tồn tại.
      {:else if page.status === 403}
        Bạn không có quyền truy cập trang này.
      {:else}
        Đã xảy ra lỗi. Vui lòng thử lại sau.
      {/if}
    </Text>

    {#if page.error?.message && page.status >= 500}
      <Text class="error-detail">{page.error.message}</Text>
    {/if}

    <Button href={ROUTES.HOME} class="error-btn">
      <i class="fa-solid fa-house"></i>
      Về trang chủ
    </Button>
  </div>
</div>

<style>
  :global(html, body) {
    margin: 0;
    padding: 0;
  }

  .error-page {
    min-height: 100dvh;
    display: flex;
    align-items: center;
    justify-content: center;
    background: linear-gradient(135deg, #f0fdf4 0%, #dcfce7 100%);
    padding: 24px;
    font-family: system-ui, -apple-system, sans-serif;
  }

  .error-card {
    background: #fff;
    border-radius: 20px;
    padding: 48px 36px;
    max-width: 380px;
    width: 100%;
    text-align: center;
    box-shadow: 0 8px 40px rgba(6, 78, 59, 0.12);
  }

  .error-icon {
    font-size: 3rem;
    color: var(--p700, #065f46);
    margin-bottom: 16px;
    opacity: 0.8;
  }

  :global(.error-code) {
    font-size: 4rem;
    font-weight: 800;
    color: var(--p900, #022c22);
    margin: 0 0 8px;
    line-height: 1;
  }

  :global(.error-message) {
    font-size: 1rem;
    color: #4b5563;
    margin: 0 0 8px;
    line-height: 1.5;
  }

  :global(.error-detail) {
    font-size: 0.82rem;
    color: #9ca3af;
    margin: 0 0 24px;
    font-family: monospace;
  }

  :global(.error-btn) {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    margin-top: 24px;
    padding: 14px 28px;
    background: var(--p700, #065f46);
    color: #fff;
    text-decoration: none;
    border-radius: 100px;
    font-size: 0.95rem;
    font-weight: 600;
    min-height: 48px;
    transition: background 0.15s;
  }

  :global(.error-btn):hover {
    background: var(--p800, #064e3b);
  }
</style>
