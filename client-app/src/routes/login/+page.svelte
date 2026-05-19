<script lang="ts">
  import { enhance } from "$app/forms";
  import SubmitButton from "$lib/components/atoms/SubmitButton.svelte";
  import Button from "$lib/components/atoms/Button.svelte";
  import Heading from "$lib/components/atoms/Heading.svelte";
  import Text from "$lib/components/atoms/Text.svelte";
  import type { PageData, ActionData } from "./$types";
  import { ROUTES } from "$lib/routes";

  const { data, form }: { data: PageData; form: ActionData } = $props();

  let submitting = $state(false);

  const MAX_ATTEMPTS = 5;
  let failedAttempts = $state(0);

  $effect(() => {
    if (form?.error && !(form as any)?.locked) {
      failedAttempts += 1;
    }
    if ((form as any)?.locked) {
      failedAttempts = MAX_ATTEMPTS;
    }
  });

  const attemptsLeft = $derived(MAX_ATTEMPTS - failedAttempts);
  const showAttemptsWarning = $derived(
    failedAttempts >= 2 && failedAttempts < MAX_ATTEMPTS,
  );
</script>

<svelte:head>
  <title
    >{data.firstRun ? "Thiết lập hệ thống" : "Đăng nhập"} — Agri-Connect</title
  >
  <meta name="robots" content="noindex, nofollow" />
</svelte:head>

<div class="auth-shell">
  <div class="auth-card">
    <div class="auth-brand">
      <i class="fa-solid fa-seedling auth-brand__icon"></i>
      <span class="auth-brand__name">Agri Admin</span>
    </div>

    {#if data.passwordReset}
      <div class="alert alert-success py-2 small mb-3">
        <i class="fa-solid fa-circle-check me-1"></i>Mật khẩu đã được đặt lại
        thành công. Vui lòng đăng nhập lại.
      </div>
    {/if}

    {#if data.firstRun}
      <!-- ── First-run setup ─────────────────────────────────── -->
      <Heading level={1} class="auth-title">Thiết lập tài khoản Admin</Heading>
      <Text class="auth-sub">
        Chưa có tài khoản nào. Hãy tạo tài khoản quản trị viên đầu tiên.
      </Text>

      {#if form?.error}
        <div class="alert alert-danger py-2 small">
          <i class="fa-solid fa-triangle-exclamation me-1"></i>{form.error}
        </div>
      {/if}

      <form
        method="POST"
        action="?/setup"
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
          <div class="mb-3">
            <label class="form-label fw-semibold" for="su_name">Họ và tên</label
            >
            <input
              id="su_name"
              name="full_name"
              type="text"
              class="form-control form-control-lg"
              placeholder="Nguyễn Văn A"
            />
          </div>
          <div class="mb-3">
            <label class="form-label fw-semibold" for="su_email">
              Email <span class="text-danger">*</span>
            </label>
            <input
              id="su_email"
              name="email"
              type="email"
              class="form-control form-control-lg"
              placeholder="admin@example.com"
              autocomplete="email"
              required
            />
          </div>
          <div class="mb-3">
            <label class="form-label fw-semibold" for="su_user">
              Tên đăng nhập <span class="text-danger">*</span>
            </label>
            <input
              id="su_user"
              name="username"
              type="text"
              class="form-control form-control-lg"
              placeholder="admin"
              autocomplete="username"
              required
            />
          </div>
          <div class="mb-3">
            <label class="form-label fw-semibold" for="su_pw">
              Mật khẩu <span class="text-danger">*</span>
            </label>
            <input
              id="su_pw"
              name="password"
              type="password"
              class="form-control form-control-lg"
              placeholder="Tối thiểu 8 ký tự, có chữ hoa, số và ký tự đặc biệt"
              autocomplete="new-password"
              required
              minlength="8"
            />
          </div>
          <div class="mb-4">
            <label class="form-label fw-semibold" for="su_pw2">
              Xác nhận mật khẩu <span class="text-danger">*</span>
            </label>
            <input
              id="su_pw2"
              name="password2"
              type="password"
              class="form-control form-control-lg"
              placeholder="Nhập lại mật khẩu"
              autocomplete="new-password"
              required
            />
          </div>
          <SubmitButton
            {submitting}
            icon="fa-check"
            label="Tạo tài khoản &amp; Đăng nhập"
            loadingLabel="Đang xử lý…"
            class="btn-lg w-100"
          />
        </fieldset>
      </form>
    {:else}
      <!-- ── Normal login ────────────────────────────────────── -->
      <Heading level={1} class="auth-title">Đăng nhập</Heading>
      <Text class="auth-sub">Dành cho quản trị viên nền tảng.</Text>

      {#if (form as any)?.locked}
        <div class="lockout-banner">
          <div class="lockout-banner__icon">
            <i class="fa-solid fa-lock"></i>
          </div>
          <Text class="lockout-banner__title">Tài khoản tạm thời bị khoá</Text>
          <Text class="lockout-banner__body">
            Đăng nhập sai quá nhiều lần. Tài khoản sẽ tự mở khoá sau
            <strong>15 phút</strong>.
          </Text>
          <Text class="lockout-banner__body">
            Hoặc đặt lại mật khẩu để mở khoá ngay lập tức:
          </Text>
          <Button
            href={ROUTES.FORGOT_PASSWORD}
            variant="primary"
            size="lg"
            class="w-100 mt-1"
          >
            <i class="fa-solid fa-key"></i>Đặt lại mật khẩu
          </Button>
        </div>
      {:else}
        {#if form?.error}
          <div class="alert alert-danger py-2 small">
            <i class="fa-solid fa-triangle-exclamation me-1"></i>{form.error}
          </div>
        {/if}

        {#if showAttemptsWarning}
          <div class="alert alert-warning py-2 small">
            <i class="fa-solid fa-shield-halved me-1"></i>
            Cảnh báo: còn <strong>{attemptsLeft}</strong> lần thử trước khi tài khoản
            bị khoá tạm thời.
          </div>
        {/if}

        <form
          method="POST"
          action="?/login"
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
            <div class="mb-3">
              <label class="form-label fw-semibold" for="lg_user"
                >Tên đăng nhập</label
              >
              <input
                id="lg_user"
                name="username"
                type="text"
                class="form-control form-control-lg"
                placeholder="admin"
                autocomplete="username"
                required
              />
            </div>
            <div class="mb-4">
              <label class="form-label fw-semibold" for="lg_pw">Mật khẩu</label>
              <input
                id="lg_pw"
                name="password"
                type="password"
                class="form-control form-control-lg"
                placeholder="••••••••"
                autocomplete="current-password"
                required
              />
            </div>
            <SubmitButton
              {submitting}
              icon="fa-right-to-bracket"
              label="Đăng nhập"
              loadingLabel="Đang đăng nhập…"
              class="btn-lg w-100"
            />
          </fieldset>
          <div class="text-center mt-3">
            <Button href={ROUTES.FORGOT_PASSWORD} class="text-muted small"
              >Quên mật khẩu?</Button
            >
          </div>
        </form>
      {/if}
    {/if}

    <Text class="auth-footer">
      <Button href={ROUTES.HOME} class="text-muted small">
        <i class="fa-solid fa-arrow-left me-1"></i>Về trang chủ
      </Button>
      {#if !data.firstRun}
        <span class="text-muted small mx-2">·</span>
        <Button href={ROUTES.REGISTER} class="text-muted small"
          >Đăng ký nhà cung cấp</Button
        >
      {/if}
    </Text>
  </div>
</div>

<style>
  :global(body) {
    padding-top: 0 !important;
    padding-bottom: 0 !important;
  }

  .lockout-banner {
    text-align: center;
    background: #fff8e1;
    border: 1.5px solid #f59e0b;
    border-radius: 10px;
    padding: 24px 20px 20px;
    margin-bottom: 8px;
  }
  .lockout-banner__icon {
    font-size: 2.5rem;
    color: #d97706;
    margin-bottom: 12px;
  }
  :global(.lockout-banner__title) {
    font-weight: 700;
    font-size: 1.05rem;
    color: #92400e;
    margin-bottom: 8px;
  }
  :global(.lockout-banner__body) {
    font-size: 0.875rem;
    color: #78350f;
    margin-bottom: 4px;
  }
</style>
