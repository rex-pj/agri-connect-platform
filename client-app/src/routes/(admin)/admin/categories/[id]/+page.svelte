<script lang="ts">
    import { enhance } from '$app/forms';
    import Button from '$lib/components/atoms/Button.svelte';
    import Heading from '$lib/components/atoms/Heading.svelte';
    import type { PageData, ActionData } from './$types';
    import { ROUTES } from '$lib/routes';

    const { data, form }: { data: PageData; form: ActionData } = $props();

    const c = $derived(data.category);
    let showEdit  = $state(false);
    let isActive  = $state(data.category.is_active);
    let editName  = $state(data.category.name);
    let editSlug  = $state(data.category.slug);
    let editIcon  = $state(data.category.icon ?? '');
    let editOrder = $state(data.category.display_order);

    $effect(() => {
        if ((form as any)?.success) {
            showEdit = false;
            if ((form as any).message?.includes('hiện')) isActive = true;
            if ((form as any).message?.includes('ẩn'))   isActive = false;
        }
    });

    const typeLabel: Record<string, string> = { service: 'Dịch vụ', product: 'Sản phẩm' };
</script>

<!-- ── Breadcrumb ──────────────────────────────────────────────────────────── -->
<nav aria-label="breadcrumb" class="mx-4 mt-4">
    <ol class="breadcrumb mb-0">
        <li class="breadcrumb-item">
            <Button href={ROUTES.ADMIN.CATEGORIES} class="text-success fw-semibold text-decoration-none">
                <i class="fa-solid fa-tags me-1"></i>Danh mục
            </Button>
        </li>
        <li class="breadcrumb-item active" aria-current="page">{c.name}</li>
    </ol>
</nav>

<!-- ── Alert strip ─────────────────────────────────────────────────────────── -->
{#if (form as any)?.success && (form as any)?.message}
    <div class="alert alert-success d-flex align-items-center gap-2 mx-4 mb-0">
        <i class="fa-solid fa-circle-check flex-shrink-0"></i>{(form as any).message}
    </div>
{/if}
{#if (form as any) && !(form as any)?.success && (form as any)?.error}
    <div class="alert alert-danger d-flex align-items-center gap-2 mx-4 mb-0">
        <i class="fa-solid fa-triangle-exclamation flex-shrink-0"></i>{(form as any).error}
    </div>
{/if}

<!-- ── Detail card ─────────────────────────────────────────────────────────── -->
<div class="admin-card mx-4 mt-3 p-4 mb-4">
    <div class="d-flex align-items-start justify-content-between gap-3 flex-wrap">
        <div class="d-flex align-items-start gap-3">
            <span class="cat-avatar {c.category_type === 'service' ? 'cat-avatar--service' : 'cat-avatar--product'}">
                {#if c.icon}
                    <i class="{c.icon} fs-4"></i>
                {:else}
                    <i class="fa-solid fa-tags fs-4"></i>
                {/if}
            </span>
            <div>
                <Heading level={2} class="detail-name mb-1">{c.name}</Heading>
                <div class="d-flex flex-wrap gap-2 align-items-center">
                    <span class="badge {c.category_type === 'service' ? 'bg-info-subtle text-info border border-info-subtle' : 'bg-warning-subtle text-warning border border-warning-subtle'}">
                        {typeLabel[c.category_type] ?? c.category_type}
                    </span>
                    {#if isActive}
                        <span class="badge bg-success-subtle text-success border border-success-subtle">Hiển thị</span>
                    {:else}
                        <span class="badge bg-secondary-subtle text-secondary border border-secondary-subtle">Ẩn</span>
                    {/if}
                </div>
            </div>
        </div>
        <div class="d-flex gap-2 flex-wrap">
            <Button type="button" class="btn btn-outline-primary" onclick={() => { editName = c.name; editSlug = c.slug; editIcon = c.icon ?? ''; editOrder = c.display_order; showEdit = true; }}>
                <i class="fa-solid fa-pen-to-square me-1"></i>Sửa
            </Button>
            <form method="POST" action="?/toggle_active" use:enhance>
                <input type="hidden" name="is_active" value={String(!isActive)} />
                <Button type="submit" class="btn {isActive ? 'btn-outline-secondary' : 'btn-outline-success'}">
                    <i class="fa-solid {isActive ? 'fa-eye-slash' : 'fa-eye'} me-1"></i>
                    {isActive ? 'Ẩn' : 'Hiện'}
                </Button>
            </form>
        </div>
    </div>

    <hr class="my-3" />

    <div class="row g-3">
        <div class="col-sm-6 col-md-3">
            <div class="detail-field">
                <span class="detail-label"><i class="fa-solid fa-hashtag me-1"></i>ID</span>
                <span class="detail-value text-muted">{c.id}</span>
            </div>
        </div>
        <div class="col-sm-6 col-md-3">
            <div class="detail-field">
                <span class="detail-label"><i class="fa-solid fa-link me-1"></i>Slug</span>
                <code class="detail-value small">{c.slug}</code>
            </div>
        </div>
        <div class="col-sm-6 col-md-3">
            <div class="detail-field">
                <span class="detail-label"><i class="fa-solid fa-sort me-1"></i>Thứ tự hiển thị</span>
                <span class="detail-value">{c.display_order}</span>
            </div>
        </div>
        <div class="col-sm-6 col-md-3">
            <div class="detail-field">
                <span class="detail-label"><i class="fa-solid fa-icons me-1"></i>Icon class</span>
                <span class="detail-value text-muted">{c.icon ?? '—'}</span>
            </div>
        </div>
    </div>
</div>

<!-- ── Edit modal ─────────────────────────────────────────────────────────── -->
{#if showEdit}
<div class="modal-backdrop show" onclick={() => (showEdit = false)} role="presentation"></div>
<div class="modal d-block" tabindex="-1" role="dialog" aria-modal="true">
    <div class="modal-dialog modal-dialog-centered">
        <div class="modal-content rounded-3">
            <div class="modal-header">
                <Heading level={5} class="modal-title"><i class="fa-solid fa-pen-to-square text-primary me-2"></i>Sửa danh mục</Heading>
                <button type="button" class="btn-close" onclick={() => (showEdit = false)}></button>
            </div>
            <form method="POST" action="?/update" use:enhance>
                <div class="modal-body d-flex flex-column gap-3">
                    <div class="row g-3">
                        <div class="col-8">
                            <label class="form-label fw-semibold" for="e_name">Tên <span class="text-danger">*</span></label>
                            <input id="e_name" name="name" type="text" class="form-control" bind:value={editName} required />
                        </div>
                        <div class="col-4">
                            <label class="form-label fw-semibold" for="e_order">Thứ tự</label>
                            <input id="e_order" name="display_order" type="number" class="form-control" bind:value={editOrder} min="0" />
                        </div>
                    </div>
                    <div>
                        <label class="form-label fw-semibold" for="e_slug">Slug <span class="text-danger">*</span></label>
                        <input id="e_slug" name="slug" type="text" class="form-control" bind:value={editSlug} required />
                    </div>
                    <div>
                        <label class="form-label fw-semibold" for="e_icon">Icon class</label>
                        <input id="e_icon" name="icon" type="text" class="form-control" bind:value={editIcon} placeholder="vd: bi-water, fa-solid fa-tractor" />
                    </div>
                </div>
                <div class="modal-footer">
                    <Button type="button" class="btn btn-outline-secondary" onclick={() => (showEdit = false)}>Huỷ</Button>
                    <Button type="submit" class="btn btn-primary px-4"><i class="fa-solid fa-floppy-disk me-1"></i>Lưu</Button>
                </div>
            </form>
        </div>
    </div>
</div>
{/if}

<style>
.cat-avatar { width:56px; height:56px; flex-shrink:0; border-radius:14px; display:flex; align-items:center; justify-content:center; }
.cat-avatar--service { background:#E0F2FE; color:#0284C7; }
.cat-avatar--product { background:#FEF9C3; color:#A16207; }
</style>
