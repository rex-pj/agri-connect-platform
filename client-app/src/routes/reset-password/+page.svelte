<script lang="ts">
  import { enhance } from "$app/forms";
  import Spinner from "$lib/components/atoms/Spinner.svelte";
  import Button from "$lib/components/atoms/Button.svelte";
  import Heading from "$lib/components/atoms/Heading.svelte";
  import Text from "$lib/components/atoms/Text.svelte";
  import type { PageData, ActionData } from "./$types";
  import { ROUTES } from "$lib/routes";

  const { data, form }: { data: PageData; form: ActionData } = $props();

  // Use the token from form (after failed submit) or URL load.
  const token = $derived((form as any)?.token ?? data.token ?? "");

  let submitting = $state(false);
</script>

<svelte:head>
  <title>Đặt lại mật khẩu — Agri-Connect</title>
  <meta name="robots" content="noindex, nofollow" />
</svelte:head>

<div class="auth-shell">
  <div class="auth-card">
    <div class="auth-brand">
      <i class="fa-solid fa-seedling auth-brand__icon"></i>
      <span class="auth-brand__name">Agri-Connect</span>
    </div>

    {#if !token}
      <!-- ── No token: link is invalid ─────────────────────────── -->
      <div class="text-center">
        <div class="invalid-icon"><i class="fa-solid fa-link-slash"></i></div>
        <Heading level={1} class="auth-title">Liên kết không hợp lệ</Heading>
        <Text class="auth-sub">
          Liên kết đặt lại mật khẩu không hợp lệ hoặc đã hết hạn. Vui lòng yêu
          cầu lại.
        </Text>
        <Button
          href={ROUTES.FORGOT_PASSWORD}
          variant="primary"
          size="lg"
          class="w-100"
        >
          <i class="fa-solid fa-rotate-left"></i>Yêu cầu liên kết mới
        </Button>
      </div>
    {:else}
      <!-- ── Reset form ─────────────────────────────────────────── -->
      <Heading level={1} class="auth-title">Đặt lại mật khẩu</Heading>
      <Text class="auth-sub">Nhập mật khẩu mới cho tài khoản của bạn.</Text>

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
          <input type="hidden" name="token" value={token} />

          <div class="mb-3">
            <label class="form-label fw-semibold" for="rp_pw">
              Mật khẩu mới <span class="text-danger">*</span>
            </label>
            <input
              id="rp_pw"
              name="new_password"
              type="password"
              class="form-control form-control-lg"
              placeholder="Tối thiểu 8 ký tự, có chữ hoa, số và ký tự đặc biệt"
              autocomplete="new-password"
              minlength="8"
              required
            />
            <div class="form-text">
              Ít nhất 8 ký tự · 1 chữ hoa · 1 chữ số · 1 ký tự đặc biệt
            </div>
          </div>

          <div class="mb-4">
            <label class="form-label fw-semibold" for="rp_pw2">
              Xác nhận mật khẩu <span class="text-danger">*</span>
            </label>
            <input
              id="rp_pw2"
              name="password2"
              type="password"
              class="form-control form-control-lg"
              placeholder="Nhập lại mật khẩu"
              autocomplete="new-password"
              minlength="8"
              required
            />
          </div>

          <Button type="submit" variant="primary" size="lg" class="w-100">
            {#if submitting}
              <Spinner />Đang xử lý…
            {:else}
              <i class="fa-solid fa-lock"></i>Đặt lại mật khẩu
            {/if}
          </Button>
        </fieldset>
      </form>

      <Text class="auth-footer">
        <Button href={ROUTES.FORGOT_PASSWORD}>
          <i class="fa-solid fa-rotate-left me-1"></i>Yêu cầu liên kết mới
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

  .invalid-icon {
    font-size: 3rem;
    color: #dc2626;
    margin-bottom: 16px;
  }
</style>
