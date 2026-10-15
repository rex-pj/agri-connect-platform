<script lang="ts">
  import { enhance } from "$app/forms";
  import SubmitButton from "$lib/components/molecules/SubmitButton.svelte";
  import Heading from "$lib/components/atoms/Heading.svelte";
  import Text from "$lib/components/atoms/Text.svelte";
  import Button from "$lib/components/atoms/Button.svelte";
  import type { PageData, ActionData } from "./$types";
  import { ROUTES } from "$lib/routes";

  const { data, form }: { data: PageData; form: ActionData } = $props();

  let submittingPw = $state(false);
  let submittingLogout = $state(false);
  let showCur = $state(false);
  let showNew = $state(false);
  let showNew2 = $state(false);

  const roleLabel: Record<string, string> = {
    admin: "Quản trị viên",
    provider: "Nhà cung cấp",
  };

  const roleIcon: Record<string, string> = {
    admin: "fa-shield-halved",
    provider: "fa-store",
  };

  const expiresAt = $derived(new Date(data.sessionExpiresAt));
  const createdAt = $derived(new Date(data.createdOn));

  function fmtDate(d: Date): string {
    return d.toLocaleString("vi-VN", {
      day: "2-digit",
      month: "2-digit",
      year: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  }
</script>

<svelte:head>
  <title>Tài khoản — Agri-Connect</title>
  <meta name="robots" content="noindex, nofollow" />
</svelte:head>

<div class="acct-header">
  <div class="acct-avatar">
    <i class="fa-solid fa-circle-user"></i>
  </div>
  <div>
    <Heading level={1} class="acct-name"
      >{data.fullName ?? data.username}</Heading
    >
    {#if data.fullName}
      <Text class="acct-sub">@{data.username}</Text>
    {/if}
    <span class="acct-role">
      <i class="fa-solid {roleIcon[data.role] ?? 'fa-user'} me-1"></i>
      {roleLabel[data.role] ?? data.role}
    </span>
  </div>
</div>

<div class="px-3 pt-2 pb-5 d-flex flex-column gap-4">
  <!-- ── Profile ──────────────────────────────────────────────────────────────── -->
  <div class="card rounded-3 overflow-hidden">
    <div class="card-header bg-white py-3 px-4">
      <Heading
        level={2}
        class="card-title mb-0 fw-bold d-flex align-items-center"
      >
        <i class="fa-solid fa-id-card me-2 text-success"></i>Thông tin tài khoản
      </Heading>
    </div>
    <div class="card-body px-4 py-3 d-flex flex-column gap-0">
      {#if data.fullName}
        <div class="acct-row">
          <span class="acct-row__label">Họ và tên</span>
          <span class="acct-row__value">{data.fullName}</span>
        </div>
      {/if}

      <div class="acct-row">
        <span class="acct-row__label">Tên đăng nhập</span>
        <span class="acct-row__value">{data.username}</span>
      </div>

      <div class="acct-row">
        <span class="acct-row__label">Email</span>
        <span class="acct-row__value">{data.email}</span>
      </div>

      <div class="acct-row">
        <span class="acct-row__label">Vai trò</span>
        <span class="acct-row__value">
          <i class="fa-solid {roleIcon[data.role] ?? 'fa-user'} me-1 text-success"></i>
          {roleLabel[data.role] ?? data.role}
        </span>
      </div>

      <div class="acct-row">
        <span class="acct-row__label">Ngày tham gia</span>
        <span class="acct-row__value acct-row__value--muted">
          <i class="fa-regular fa-calendar me-1"></i>{fmtDate(createdAt)}
        </span>
      </div>

      <div class="acct-row">
        <span class="acct-row__label">Phiên hết hạn</span>
        <span class="acct-row__value acct-row__value--muted">
          <i class="fa-regular fa-clock me-1"></i>{fmtDate(expiresAt)}
        </span>
      </div>

      {#if data.role === "provider"}
        <hr class="my-1" />
        <Button
          href={ROUTES.PROVIDER_DASHBOARD}
          class="btn-outline-success btn-sm"
        >
          <i class="fa-solid fa-gauge me-2"></i>Quản lý hồ sơ doanh nghiệp
        </Button>
      {:else if data.role === "admin"}
        <hr class="my-1" />
        <Button
          href={ROUTES.ADMIN.DASHBOARD}
          class="btn-outline-danger btn-sm"
        >
          <i class="fa-solid fa-shield-halved me-2"></i>Quản trị hệ thống
        </Button>
      {/if}
    </div>
  </div>

  <!-- ── Security ─────────────────────────────────────────────────────────────── -->
  <div class="card rounded-3 overflow-hidden">
    <div class="card-header bg-white py-3 px-4">
      <Heading
        level={2}
        class="card-title mb-0 fw-bold d-flex align-items-center"
      >
        <i class="fa-solid fa-lock me-2 text-warning"></i>Bảo mật
      </Heading>
    </div>
    <div class="card-body px-4 py-3">
      <Text class="text-muted small mb-3"
        >Đổi mật khẩu định kỳ để bảo vệ tài khoản của bạn.</Text
      >

      <form
        method="POST"
        action="?/change_password"
        use:enhance={() => {
          submittingPw = true;
          return async ({ update }) => {
            submittingPw = false;
            await update();
          };
        }}
        class="d-flex flex-column gap-3"
      >
        <fieldset class="form-fieldset" disabled={submittingPw}>
          {#if form?.action === "change_password" && form?.success}
            <div
              class="alert alert-success py-2 d-flex align-items-center gap-2 mb-0"
            >
              <i class="fa-solid fa-circle-check"></i> Mật khẩu đã được đổi thành
              công.
            </div>
          {/if}
          {#if form?.action === "change_password" && !form?.success && form?.error}
            <div class="alert alert-danger py-2 mb-0">
              <i class="fa-solid fa-triangle-exclamation me-1"></i>{form.error}
            </div>
          {/if}

          <div>
            <label class="form-label fw-semibold" for="cp_cur">
              Mật khẩu hiện tại <span class="text-danger">*</span>
            </label>
            <div class="pw-wrap">
              <input
                id="cp_cur"
                name="current_password"
                type={showCur ? 'text' : 'password'}
                class="form-control pw-input"
                autocomplete="current-password"
                required
              />
              <button type="button" class="pw-eye" onclick={() => (showCur = !showCur)} aria-label={showCur ? 'Ẩn mật khẩu' : 'Hiện mật khẩu'}>
                <i class="fa-solid {showCur ? 'fa-eye-slash' : 'fa-eye'}"></i>
              </button>
            </div>
          </div>
          <div>
            <label class="form-label fw-semibold" for="cp_new">
              Mật khẩu mới <span class="text-danger">*</span>
            </label>
            <div class="pw-wrap">
              <input
                id="cp_new"
                name="new_password"
                type={showNew ? 'text' : 'password'}
                class="form-control pw-input"
                autocomplete="new-password"
                minlength="8"
                required
              />
              <button type="button" class="pw-eye" onclick={() => (showNew = !showNew)} aria-label={showNew ? 'Ẩn mật khẩu' : 'Hiện mật khẩu'}>
                <i class="fa-solid {showNew ? 'fa-eye-slash' : 'fa-eye'}"></i>
              </button>
            </div>
            <div class="pw-hint">
              <i class="fa-solid fa-circle-info"></i>
              Ít nhất 8 ký tự · 1 chữ hoa · 1 chữ số · 1 ký tự đặc biệt
            </div>
          </div>
          <div>
            <label class="form-label fw-semibold" for="cp_new2">
              Xác nhận mật khẩu mới <span class="text-danger">*</span>
            </label>
            <div class="pw-wrap">
              <input
                id="cp_new2"
                name="password2"
                type={showNew2 ? 'text' : 'password'}
                class="form-control pw-input"
                autocomplete="new-password"
                minlength="8"
                required
              />
              <button type="button" class="pw-eye" onclick={() => (showNew2 = !showNew2)} aria-label={showNew2 ? 'Ẩn mật khẩu' : 'Hiện mật khẩu'}>
                <i class="fa-solid {showNew2 ? 'fa-eye-slash' : 'fa-eye'}"></i>
              </button>
            </div>
          </div>

          <div class="d-flex justify-content-end">
            <SubmitButton
              submitting={submittingPw}
              icon="fa-key"
              label="Đổi mật khẩu"
              loadingLabel="Đang lưu…"
              class="btn-warning px-4"
            />
          </div>
        </fieldset>
      </form>
    </div>
  </div>

  <!-- ── Logout ──────────────────────────────────────────────────────────────── -->
  <div class="card rounded-3 overflow-hidden">
    <div class="card-body px-4 py-3">
      <Text class="text-muted small mb-3"
        >Đăng xuất sẽ xoá phiên đăng nhập hiện tại trên thiết bị này.</Text
      >
      <form
        method="POST"
        action={ROUTES.LOGOUT}
        onsubmit={() => (submittingLogout = true)}
      >
        <fieldset class="form-fieldset" disabled={submittingLogout}>
          <SubmitButton
            submitting={submittingLogout}
            icon="fa-right-from-bracket"
            label="Đăng xuất"
            loadingLabel="Đang đăng xuất…"
            class="btn-outline-danger w-100"
          />
        </fieldset>
      </form>
    </div>
  </div>
</div>

<style>
  .acct-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 0;
    border-bottom: 1px solid #f0f4f1;
    gap: 12px;
  }
  .acct-row:last-of-type { border-bottom: none; }
  .acct-row__label {
    font-size: 0.92rem;
    color: #5e7a68;
    flex-shrink: 0;
  }
  .acct-row__value {
    font-size: 0.95rem;
    font-weight: 600;
    color: #0c1a12;
    text-align: right;
    word-break: break-all;
  }
  .acct-row__value--muted {
    font-weight: 400;
    color: #5e7a68;
  }

  .pw-wrap {
    position: relative;
  }
  .pw-input {
    padding-right: 48px;
  }
  .pw-eye {
    position: absolute;
    right: 0;
    top: 0;
    width: 44px;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    color: #5e7a68;
    font-size: 1rem;
    cursor: pointer;
    -webkit-tap-highlight-color: transparent;
  }
  .pw-eye:hover { color: #065f46; }
  .pw-hint {
    margin-top: 6px;
    font-size: 0.88rem;
    color: #4b7a5e;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .pw-hint i { font-size: 0.82rem; flex-shrink: 0; }

  .acct-header {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 28px 20px 20px;
  }
  .acct-avatar {
    font-size: 3.5rem;
    color: var(--p700, #065f46);
    line-height: 1;
    flex-shrink: 0;
  }
  .acct-name {
    font-size: 1.3rem;
    font-weight: 800;
    margin: 0 0 6px;
    line-height: 1.2;
  }
  .acct-sub {
    font-size: 0.82rem;
    color: var(--muted, #5e7a68);
    margin: 0 0 6px;
  }
  .acct-role {
    display: inline-flex;
    align-items: center;
    background: var(--p50, #ecfdf5);
    color: var(--p700, #065f46);
    border: 1px solid var(--p200, #a7f3d0);
    font-size: 0.75rem;
    font-weight: 600;
    padding: 3px 10px;
    border-radius: 20px;
  }
</style>
