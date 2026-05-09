<script lang="ts">
  import { page } from "$app/state";
  import Heading from "$lib/components/atoms/Heading.svelte";
  import Text from "$lib/components/atoms/Text.svelte";
  import Button from "$lib/components/atoms/Button.svelte";
  import { ROUTES } from "$lib/routes";
</script>

<svelte:head>
  <title>{page.status} – Agri-Connect</title>
</svelte:head>

<div class="error-wrap">
  <div class="error-icon">
    {#if page.status === 404}
      <i class="fa-solid fa-map-location-dot"></i>
    {:else}
      <i class="fa-solid fa-triangle-exclamation"></i>
    {/if}
  </div>

  <Heading level={1} class="error-code">{page.status}</Heading>

  <Text class="error-msg">
    {#if page.status === 404}
      Trang bạn tìm không tồn tại.
    {:else if page.status === 403}
      Bạn không có quyền truy cập trang này.
    {:else}
      Đã xảy ra lỗi máy chủ. Vui lòng thử lại sau.
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

<style>
  .error-wrap {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    min-height: 60dvh;
    padding: 40px 24px;
    text-align: center;
  }

  .error-icon {
    font-size: 3.5rem;
    color: var(--p700, #065f46);
    margin-bottom: 16px;
    opacity: 0.75;
  }

  :global(.error-code) {
    font-size: 5rem;
    font-weight: 800;
    color: var(--p900, #022c22);
    margin: 0 0 8px;
    line-height: 1;
  }

  :global(.error-msg) {
    font-size: 1rem;
    color: #4b5563;
    margin: 0;
    line-height: 1.5;
  }

  :global(.error-detail) {
    font-size: 0.8rem;
    color: #9ca3af;
    margin: 8px 0 0;
    font-family: monospace;
  }

  :global(.error-btn) {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    margin-top: 28px;
    padding: 14px 32px;
    background: var(--p700, #065f46);
    color: #fff;
    text-decoration: none;
    border-radius: 100px;
    font-size: 0.95rem;
    font-weight: 600;
    min-height: 48px;
    transition: background 0.15s;
  }

  :global(.error-btn:hover) {
    background: var(--p800, #064e3b);
  }
</style>
