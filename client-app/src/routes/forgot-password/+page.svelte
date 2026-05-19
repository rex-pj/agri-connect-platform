<script lang="ts">
  import { enhance } from "$app/forms";
  import Spinner from "$lib/components/atoms/Spinner.svelte";
  import Button from "$lib/components/atoms/Button.svelte";
  import Heading from "$lib/components/atoms/Heading.svelte";
  import Text from "$lib/components/atoms/Text.svelte";
  import type { ActionData } from "./$types";
  import { ROUTES } from "$lib/routes";

  const { form }: { form: ActionData } = $props();

  let submitting = $state(false);
</script>

<svelte:head>
  <title>Quên mật khẩu — Agri-Connect</title>
  <meta name="robots" content="noindex, nofollow" />
</svelte:head>

<div class="auth-shell">
  <div class="auth-card">
    <div class="auth-brand">
      <i class="fa-solid fa-seedling auth-brand__icon"></i>
      <span class="auth-brand__name">Agri-Connect</span>
    </div>

    {#if (form as any)?.sent}
      <!-- ── Sent state ─────────────────────────────────────────── -->
      <div class="sent-state">
        <div class="sent-icon">
          <i class="fa-solid fa-envelope-circle-check"></i>
        </div>
        <Heading level={1} class="auth-title">Kiểm tra hộp thư</Heading>
        <Text class="auth-sub">
          Nếu địa chỉ email tồn tại trong hệ thống, chúng tôi đã gửi hướng dẫn
          đặt lại mật khẩu. Vui lòng kiểm tra cả thư mục Spam.
        </Text>
        <Button
          href={ROUTES.LOGIN}
          variant="primary"
          size="lg"
          class="w-100 mt-2"
        >
          <i class="fa-solid fa-right-to-bracket me-2"></i>Về trang đăng nhập
        </Button>
      </div>
    {:else}
      <!-- ── Form state ─────────────────────────────────────────── -->
      <Heading level={1} class="auth-title">Quên mật khẩu</Heading>
      <Text class="auth-sub">
        Nhập email đã đăng ký. Chúng tôi sẽ gửi liên kết đặt lại mật khẩu (hiệu
        lực trong 1 giờ).
      </Text>

      {#if form?.error}
        <div class="alert alert-danger py-2 small mb-3">
          <i class="fa-solid fa-triangle-exclamation me-1"></i>{form.error}
        </div>
      {/if}

      <form
        method="POST"
        use:enhance={() => {
          submitting = true;
          return async ({ update }) => {
            submitting = false;
            await update();
          };
        }}
        class="auth-form"
      >
        <fieldset class="form-fieldset" disabled={submitting}>
          <div class="mb-4">
            <label class="form-label fw-semibold" for="fp_email">
              Địa chỉ email <span class="text-danger">*</span>
            </label>
            <input
              id="fp_email"
              name="email"
              type="email"
              class="form-control form-control-lg"
              placeholder="email@example.com"
              autocomplete="email"
              required
            />
          </div>

          <Button type="submit" variant="primary" size="lg" class="w-100">
            {#if submitting}
              <Spinner />Đang gửi…
            {:else}
              <i class="fa-solid fa-paper-plane"></i>Gửi liên kết đặt lại
            {/if}
          </Button>
        </fieldset>
      </form>

      <Text class="auth-footer">
        <Button href={ROUTES.LOGIN}>
          <i class="fa-solid fa-arrow-left me-1"></i>Quay lại đăng nhập
        </Button>
      </Text>
    {/if}
  </div>
</div>

<style>
  :global(body) {
    padding-top: 0 !important;
    padding-bottom: 0 !important;
  }

  .sent-state {
    text-align: center;
  }
  .sent-icon {
    font-size: 3rem;
    color: #047857;
    margin-bottom: 16px;
  }
</style>
