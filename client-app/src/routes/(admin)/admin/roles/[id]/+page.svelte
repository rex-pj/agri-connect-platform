<script lang="ts">
    import Button from '$lib/components/atoms/Button.svelte';
    import Heading from '$lib/components/atoms/Heading.svelte';
    import type { PageData } from './$types';
    import { ROUTES } from '$lib/routes';

    const { data }: { data: PageData } = $props();

    const r = $derived(data.role);

    const roleIcons: Record<string, string> = {
        admin:    'fa-solid fa-crown',
        staff:    'fa-solid fa-user-tie',
        provider: 'fa-solid fa-store',
    };

    const roleColors: Record<string, { avatar: string; badge: string }> = {
        admin:    { avatar: 'role-avatar--admin',    badge: 'bg-danger-subtle text-danger border border-danger-subtle' },
        staff:    { avatar: 'role-avatar--staff',    badge: 'bg-primary-subtle text-primary border border-primary-subtle' },
        provider: { avatar: 'role-avatar--provider', badge: 'bg-success-subtle text-success border border-success-subtle' },
    };

    const colors = $derived(roleColors[r.code] ?? { avatar: '', badge: 'bg-secondary-subtle text-secondary border' });
    const icon   = $derived(roleIcons[r.code] ?? 'fa-solid fa-shield-halved');
</script>

<!-- ── Breadcrumb ──────────────────────────────────────────────────────────── -->
<nav aria-label="breadcrumb" class="mx-4 mt-4">
    <ol class="breadcrumb mb-0">
        <li class="breadcrumb-item">
            <a href={ROUTES.ADMIN.ROLES} class="text-success fw-semibold text-decoration-none">
                <i class="fa-solid fa-shield-halved me-1"></i>Vai trò
            </a>
        </li>
        <li class="breadcrumb-item active" aria-current="page">{r.name}</li>
    </ol>
</nav>

<!-- ── Detail card ─────────────────────────────────────────────────────────── -->
<div class="admin-card mx-4 mt-3 p-4 mb-4">
    <div class="d-flex align-items-start gap-3">
        <span class="role-avatar {colors.avatar}">
            <i class="{icon} fs-4"></i>
        </span>
        <div>
            <Heading level={2} class="detail-name mb-1">{r.name}</Heading>
            <div class="d-flex flex-wrap gap-2 align-items-center">
                <span class="badge {colors.badge}">{r.code}</span>
                {#if r.is_active}
                    <span class="badge bg-success-subtle text-success border border-success-subtle">Hoạt động</span>
                {:else}
                    <span class="badge bg-secondary-subtle text-secondary border border-secondary-subtle">Tắt</span>
                {/if}
            </div>
        </div>
    </div>

    <hr class="my-3" />

    <div class="row g-3">
        <div class="col-sm-6 col-md-3">
            <div class="detail-field">
                <span class="detail-label"><i class="fa-solid fa-hashtag me-1"></i>ID</span>
                <span class="detail-value text-muted">{r.id}</span>
            </div>
        </div>
        <div class="col-sm-6 col-md-3">
            <div class="detail-field">
                <span class="detail-label"><i class="fa-solid fa-code me-1"></i>Mã vai trò</span>
                <code class="detail-value">{r.code}</code>
            </div>
        </div>
        <div class="col-12">
            <div class="detail-field">
                <span class="detail-label"><i class="fa-solid fa-align-left me-1"></i>Mô tả</span>
                <span class="detail-value">{r.description ?? '—'}</span>
            </div>
        </div>
    </div>

    <div class="alert alert-light border mt-3 d-flex align-items-center gap-2 py-2 mb-0">
        <i class="fa-solid fa-circle-info text-muted flex-shrink-0"></i>
        <span class="small text-muted">Vai trò hệ thống được quản lý qua migration — không thể thay đổi tại đây.</span>
    </div>
</div>

<style>
.role-avatar { width:56px; height:56px; flex-shrink:0; border-radius:14px; display:flex; align-items:center; justify-content:center; }
.role-avatar--admin    { background:#FEE2E2; color:#DC2626; }
.role-avatar--staff    { background:#EEF2FF; color:#4F46E5; }
.role-avatar--provider { background:#ECFDF5; color:#047857; }
</style>
