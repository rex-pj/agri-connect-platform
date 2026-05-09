<script lang="ts">
    import { enhance } from '$app/forms';
    import SubmitButton from '$lib/components/atoms/SubmitButton.svelte';
    import Heading from '$lib/components/atoms/Heading.svelte';
    import Text from '$lib/components/atoms/Text.svelte';
    import Button from '$lib/components/atoms/Button.svelte';
    import type { PageData, ActionData } from './$types';
    import { ROUTES } from '$lib/routes';

    const { data, form }: { data: PageData; form: ActionData } = $props();

    let submittingPw     = $state(false);
    let submittingLogout = $state(false);

    const roleLabel: Record<string, string> = {
        admin:    'Quản trị viên',
        provider: 'Nhà cung cấp',
    };

    const roleIcon: Record<string, string> = {
        admin:    'fa-shield-halved',
        provider: 'fa-store',
    };

    const expiresAt = $derived(new Date(data.sessionExpiresAt));
    const createdAt = $derived(new Date(data.createdOn));

    function fmtDate(d: Date): string {
        return d.toLocaleString('vi-VN', {
            day: '2-digit', month: '2-digit', year: 'numeric',
            hour: '2-digit', minute: '2-digit',
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
        <Heading level={1} class="acct-name">{data.fullName ?? data.username}</Heading>
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
        <Heading level={2} class="card-title mb-0 fw-bold d-flex align-items-center">
            <i class="fa-solid fa-id-card me-2 text-success"></i>Thông tin tài khoản
        </Heading>
    </div>
    <div class="card-body px-4 py-3 d-flex flex-column gap-3">

        {#if data.fullName}
            <div class="d-flex justify-content-between align-items-center">
                <span class="text-muted small">Họ và tên</span>
                <span class="fw-semibold text-end">{data.fullName}</span>
            </div>
        {/if}

        <div class="d-flex justify-content-between align-items-center">
            <span class="text-muted small">Tên đăng nhập</span>
            <span class="fw-semibold text-end">{data.username}</span>
        </div>

        <div class="d-flex justify-content-between align-items-center">
            <span class="text-muted small">Email</span>
            <span class="fw-semibold text-end">{data.email}</span>
        </div>

        <div class="d-flex justify-content-between align-items-center">
            <span class="text-muted small">Vai trò</span>
            <span class="fw-semibold text-end">
                <i class="fa-solid {roleIcon[data.role] ?? 'fa-user'} me-1 text-success"></i>
                {roleLabel[data.role] ?? data.role}
            </span>
        </div>

        <div class="d-flex justify-content-between align-items-center">
            <span class="text-muted small">Ngày tham gia</span>
            <span class="text-muted small text-end">
                <i class="fa-regular fa-calendar me-1"></i>{fmtDate(createdAt)}
            </span>
        </div>

        <div class="d-flex justify-content-between align-items-center">
            <span class="text-muted small">Phiên hết hạn</span>
            <span class="text-muted small text-end">
                <i class="fa-regular fa-clock me-1"></i>{fmtDate(expiresAt)}
            </span>
        </div>

        {#if data.role === 'provider'}
            <hr class="my-1" />
            <Button href={ROUTES.PROVIDER_DASHBOARD} class="btn btn-outline-success btn-sm">
                <i class="fa-solid fa-gauge me-2"></i>Quản lý hồ sơ doanh nghiệp
            </Button>
        {/if}
    </div>
</div>

<!-- ── Security ─────────────────────────────────────────────────────────────── -->
<div class="card rounded-3 overflow-hidden">
    <div class="card-header bg-white py-3 px-4">
        <Heading level={2} class="card-title mb-0 fw-bold d-flex align-items-center">
            <i class="fa-solid fa-lock me-2 text-warning"></i>Bảo mật
        </Heading>
    </div>
    <div class="card-body px-4 py-3">

        <Text class="text-muted small mb-3">Đổi mật khẩu định kỳ để bảo vệ tài khoản của bạn.</Text>

        <form method="POST" action="?/change_password" use:enhance={() => { submittingPw = true; return async ({ update }) => { submittingPw = false; await update(); }; }} class="d-flex flex-column gap-3">
            <fieldset class="form-fieldset" disabled={submittingPw}>

            {#if form?.action === 'change_password' && form?.success}
                <div class="alert alert-success py-2 d-flex align-items-center gap-2 mb-0">
                    <i class="fa-solid fa-circle-check"></i> Mật khẩu đã được đổi thành công.
                </div>
            {/if}
            {#if form?.action === 'change_password' && !form?.success && form?.error}
                <div class="alert alert-danger py-2 mb-0">
                    <i class="fa-solid fa-triangle-exclamation me-1"></i>{form.error}
                </div>
            {/if}

            <div>
                <label class="form-label fw-semibold" for="cp_cur">
                    Mật khẩu hiện tại <span class="text-danger">*</span>
                </label>
                <input id="cp_cur" name="current_password" type="password" class="form-control" autocomplete="current-password" required />
            </div>
            <div>
                <label class="form-label fw-semibold" for="cp_new">
                    Mật khẩu mới <span class="text-danger">*</span>
                </label>
                <input id="cp_new" name="new_password" type="password" class="form-control" autocomplete="new-password" minlength="8" required />
                <div class="form-text">Ít nhất 8 ký tự · 1 chữ hoa · 1 chữ số · 1 ký tự đặc biệt</div>
            </div>
            <div>
                <label class="form-label fw-semibold" for="cp_new2">
                    Xác nhận mật khẩu mới <span class="text-danger">*</span>
                </label>
                <input id="cp_new2" name="password2" type="password" class="form-control" autocomplete="new-password" minlength="8" required />
            </div>

            <div class="d-flex justify-content-end">
                <SubmitButton
                    submitting={submittingPw}
                    icon="fa-key"
                    label="Đổi mật khẩu"
                    loadingLabel="Đang lưu…"
                    class="btn btn-warning px-4"
                />
            </div>
            </fieldset>
        </form>
    </div>
</div>

<!-- ── Logout ──────────────────────────────────────────────────────────────── -->
<div class="card rounded-3 overflow-hidden">
    <div class="card-body px-4 py-3">
        <Text class="text-muted small mb-3">Đăng xuất sẽ xoá phiên đăng nhập hiện tại trên thiết bị này.</Text>
        <form method="POST" action="?/logout" use:enhance={() => { submittingLogout = true; return async ({ update }) => { submittingLogout = false; await update(); }; }}>
            <fieldset class="form-fieldset" disabled={submittingLogout}>
            <SubmitButton
                submitting={submittingLogout}
                icon="fa-right-from-bracket"
                label="Đăng xuất"
                loadingLabel="Đang đăng xuất…"
                class="btn btn-outline-danger w-100"
            />
            </fieldset>
        </form>
    </div>
</div>

</div>

<style>
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
.acct-name { font-size: 1.3rem; font-weight: 800; margin: 0 0 6px; line-height: 1.2; }
.acct-sub  { font-size: 0.82rem; color: var(--muted, #5e7a68); margin: 0 0 6px; }
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
