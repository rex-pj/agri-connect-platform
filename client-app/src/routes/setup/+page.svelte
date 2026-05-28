<script lang="ts">
  import { enhance } from "$app/forms";
  import SubmitButton from "$lib/components/molecules/SubmitButton.svelte";
  import Heading from "$lib/components/atoms/Heading.svelte";
  import Text from "$lib/components/atoms/Text.svelte";
  import type { ActionData } from "./$types";

  const { form }: { form: ActionData } = $props();

  let submitting = $state(false);
  let goSeed     = $state(true);
</script>

<svelte:head>
  <title>Thiết lập hệ thống — Agri-Connect</title>
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
      <div class="steps__item steps__item--active">
        <span class="steps__dot">1</span>
        <span class="steps__label">Tài khoản</span>
      </div>
      <div class="steps__line"></div>
      <div class="steps__item steps__item--pending">
        <span class="steps__dot">2</span>
        <span class="steps__label">Dữ liệu mẫu</span>
      </div>
    </div>

    <Heading level={1} class="auth-title">Tạo tài khoản quản trị viên</Heading>
    <Text class="auth-sub">Bước 1/2 — Đây là tài khoản admin đầu tiên của hệ thống.</Text>

    {#if form?.error}
      <div class="alert alert-danger py-2 small">
        <i class="fa-solid fa-triangle-exclamation me-1"></i>{form.error}
      </div>
    {/if}

    <form
      method="POST"
      action="?/create"
      use:enhance={() => {
        submitting = true;
        return async ({ update }) => { submitting = false; await update(); };
      }}
      class="auth-form"
    >
      <!-- Hidden flag tells server which redirect to use -->
      <input type="hidden" name="go_seed" value={goSeed ? '1' : '0'} />

      <fieldset class="form-fieldset" disabled={submitting}>
        <div class="mb-3">
          <label class="form-label fw-semibold" for="su_name">Họ và tên</label>
          <input id="su_name" name="full_name" type="text"
            class="form-control form-control-lg" placeholder="Nguyễn Văn A" />
        </div>
        <div class="mb-3">
          <label class="form-label fw-semibold" for="su_email">
            Email <span class="text-danger">*</span>
          </label>
          <input id="su_email" name="email" type="email"
            class="form-control form-control-lg" placeholder="admin@example.com"
            autocomplete="email" required />
        </div>
        <div class="mb-3">
          <label class="form-label fw-semibold" for="su_user">
            Tên đăng nhập <span class="text-danger">*</span>
          </label>
          <input id="su_user" name="username" type="text"
            class="form-control form-control-lg" placeholder="admin"
            autocomplete="username" required />
        </div>
        <div class="mb-3">
          <label class="form-label fw-semibold" for="su_pw">
            Mật khẩu <span class="text-danger">*</span>
          </label>
          <input id="su_pw" name="password" type="password"
            class="form-control form-control-lg"
            placeholder="Tối thiểu 8 ký tự"
            autocomplete="new-password" required minlength="8" />
        </div>
        <div class="mb-4">
          <label class="form-label fw-semibold" for="su_pw2">
            Xác nhận mật khẩu <span class="text-danger">*</span>
          </label>
          <input id="su_pw2" name="password2" type="password"
            class="form-control form-control-lg" placeholder="Nhập lại mật khẩu"
            autocomplete="new-password" required />
        </div>

        <!-- Seed choice toggle -->
        <div class="seed-toggle">
          <label class="seed-toggle__label">
            <input type="checkbox" bind:checked={goSeed} class="seed-toggle__checkbox" />
            <span class="seed-toggle__text">
              <i class="fa-solid fa-database me-1 text-success"></i>
              Tiếp tục đến bước nạp dữ liệu mẫu
            </span>
          </label>
          <Text class="seed-toggle__hint">
            Thêm nhà cung cấp, dịch vụ và sản phẩm mẫu để kiểm thử giao diện.
          </Text>
        </div>

        <SubmitButton {submitting}
          icon="fa-arrow-right"
          label={goSeed ? 'Tạo tài khoản & Tiếp tục' : 'Tạo tài khoản & Vào Dashboard'}
          loadingLabel="Đang xử lý…"
          class="btn-lg w-100 btn-success" />
      </fieldset>
    </form>
  </div>
</div>

<style>
  :global(body) { padding-top: 0 !important; padding-bottom: 0 !important; }

  /* ── Step indicator ──────────────────────────────── */
  .steps {
    display: flex;
    align-items: center;
    gap: 0;
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
  .steps__item--active .steps__dot {
    background: #16a34a;
    color: #fff;
  }
  .steps__item--pending .steps__dot {
    background: #e5e7eb;
    color: #9ca3af;
  }
  .steps__label {
    font-size: 0.72rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .steps__item--active .steps__label  { color: #16a34a; }
  .steps__item--pending .steps__label { color: #9ca3af; }
  .steps__line {
    flex: 1;
    height: 2px;
    background: #e5e7eb;
    margin: 0 8px;
    margin-bottom: 20px;
  }

  /* ── Seed toggle ─────────────────────────────────── */
  .seed-toggle {
    background: #f0fdf4;
    border: 1px solid #bbf7d0;
    border-radius: 10px;
    padding: 12px 14px;
    margin-bottom: 20px;
  }
  .seed-toggle__label {
    display: flex;
    align-items: center;
    gap: 10px;
    cursor: pointer;
    font-size: 0.9rem;
    font-weight: 600;
    color: #14532d;
    margin-bottom: 4px;
  }
  .seed-toggle__checkbox { width: 18px; height: 18px; accent-color: #16a34a; cursor: pointer; }
  .seed-toggle__text { flex: 1; }
  :global(.seed-toggle__hint) {
    font-size: 0.8rem;
    color: #4b7c5e;
    margin: 0;
    padding-left: 28px;
  }
</style>
