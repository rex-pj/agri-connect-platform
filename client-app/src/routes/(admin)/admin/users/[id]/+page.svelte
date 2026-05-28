<script lang="ts">
  import { untrack } from "svelte";
  import { enhance } from "$app/forms";
  import Button from "$lib/components/atoms/Button.svelte";
  import Heading from "$lib/components/atoms/Heading.svelte";
  import Spinner from "$lib/components/atoms/Spinner.svelte";
  import ConfirmModal from "$lib/components/molecules/ConfirmModal.svelte";
  import type { PageData, ActionData } from "./$types";
  import { ROUTES } from "$lib/routes";

  const { data, form }: { data: PageData; form: ActionData } = $props();

  const u = $derived(data.user);
  // isActive derived from server data so it updates automatically after form submission
  const isActive = $derived(data.user.is_active);
  let showRoleModal  = $state(false);
  let selectedRole   = $state(untrack(() => data.user.role_code));
  let submittingRole = $state(false);
  let loadingActivate = $state(false);
  let confirmDeactivate = $state(false);
  let togglingPermId = $state<number | null>(null);

  // Track granted permission IDs reactively so toggle is instant
  let grantedIds = $state(new Set<number>(untrack(() => data.userPermIds)));

  $effect(() => {
    const f = form as any;

    // Clear permission spinner as soon as the server responds (success OR error).
    // Must check BEFORE the early-return so errors also clear it.
    if (f?.action?.includes('permission')) {
      togglingPermId = null;
    }

    if (!f?.success) return;
    showRoleModal = false;
    confirmDeactivate = false;

    // Use untrack when reading grantedIds to avoid making it a dependency
    // of this effect (which would cause an infinite re-run loop).
    if (f.action === 'assign_permission' && f.permissionId) {
      grantedIds = untrack(() => new Set([...grantedIds, f.permissionId]));
    }
    if (f.action === 'revoke_permission' && f.permissionId) {
      grantedIds = untrack(() => {
        const next = new Set(grantedIds);
        next.delete(f.permissionId);
        return next;
      });
    }
  });

  function openRoleModal() {
    selectedRole = data.user.role_code; // always reflect current server state
    showRoleModal = true;
  }

  function fmtDate(iso: string): string {
    return new Date(iso).toLocaleDateString("vi-VN", {
      day: "2-digit",
      month: "2-digit",
      year: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  const roleColors: Record<string, string> = {
    admin: "bg-danger-subtle text-danger border border-danger-subtle",
    staff: "bg-primary-subtle text-primary border border-primary-subtle",
    provider: "bg-success-subtle text-success border border-success-subtle",
  };
</script>

<!-- ── Breadcrumb ──────────────────────────────────────────────────────────── -->
<nav aria-label="breadcrumb" class="mx-4 mt-4">
  <ol class="breadcrumb mb-0">
    <li class="breadcrumb-item">
      <a href={ROUTES.ADMIN.USERS} class="text-success fw-semibold text-decoration-none">
        <i class="fa-solid fa-users me-1"></i>Người dùng
      </a>
    </li>
    <li class="breadcrumb-item active" aria-current="page">{u.username}</li>
  </ol>
</nav>

<!-- ── Alert strip ─────────────────────────────────────────────────────────── -->
{#if (form as any)?.success && (form as any)?.message && !(form as any)?.action?.includes('permission')}
  <div class="alert alert-success d-flex align-items-center gap-2 mx-4 mb-0">
    <i class="fa-solid fa-circle-check flex-shrink-0"></i>{(form as any).message}
  </div>
{/if}
{#if (form as any) && !(form as any)?.success && (form as any)?.error && !(form as any)?.action?.includes('permission')}
  <div class="alert alert-danger d-flex align-items-center gap-2 mx-4 mb-0">
    <i class="fa-solid fa-triangle-exclamation flex-shrink-0"></i>{(form as any).error}
  </div>
{/if}

<!-- ── Detail card ─────────────────────────────────────────────────────────── -->
<div class="admin-card mx-4 mt-3 p-4">
  <div class="d-flex align-items-start justify-content-between gap-3 flex-wrap">
    <div class="d-flex align-items-start gap-3">
      <span class="user-avatar">
        <i class="fa-solid fa-user fs-4"></i>
      </span>
      <div>
        <Heading level={2} class="detail-name mb-1">{u.username}</Heading>
        {#if u.full_name}
          <div class="text-muted mb-2">{u.full_name}</div>
        {/if}
        <div class="d-flex flex-wrap gap-2 align-items-center">
          <span
            class="badge {roleColors[u.role_code] ??
              'bg-secondary-subtle text-secondary border'}"
          >
            {u.role_name}
          </span>
          {#if isActive}
            <span
              class="badge bg-success-subtle text-success border border-success-subtle"
              >Hoạt động</span
            >
          {:else}
            <span
              class="badge bg-secondary-subtle text-secondary border border-secondary-subtle"
              >Tắt</span
            >
          {/if}
        </div>
      </div>
    </div>
    <div class="d-flex gap-2 flex-wrap">
      <Button
        type="button"
        class="btn-outline-primary"
        onclick={openRoleModal}
      >
        <i class="fa-solid fa-shield-halved me-1"></i>Đổi vai trò
      </Button>
      {#if isActive}
        <!-- Deactivate: requires confirmation with reason -->
        <Button
          type="button"
          class="btn-outline-warning"
          onclick={() => (confirmDeactivate = true)}
        >
          <i class="fa-solid fa-toggle-on me-1"></i>Vô hiệu hoá
        </Button>
      {:else}
        <!-- Activate: direct submit with loading indicator -->
        <form
          method="POST"
          action="?/toggle_active"
          use:enhance={() => {
            loadingActivate = true;
            return async ({ update }) => {
              loadingActivate = false;
              await update();
            };
          }}
        >
          <input type="hidden" name="is_active" value="true" />
          <Button
            type="submit"
            class="btn-outline-success"
            disabled={loadingActivate}
          >
            {#if loadingActivate}
              <Spinner class="me-1" />Đang xử lý…
            {:else}
              <i class="fa-solid fa-toggle-off me-1"></i>Kích hoạt
            {/if}
          </Button>
        </form>
      {/if}
    </div>
  </div>

  <hr class="my-3" />

  <div class="row g-3">
    <div class="col-sm-6 col-md-4">
      <div class="detail-field">
        <span class="detail-label"
          ><i class="fa-solid fa-envelope me-1"></i>Email</span
        >
        <span class="detail-value">{u.email}</span>
      </div>
    </div>
    <div class="col-sm-6 col-md-4">
      <div class="detail-field">
        <span class="detail-label"
          ><i class="fa-solid fa-id-badge me-1"></i>ID</span
        >
        <span class="detail-value text-muted">{u.id}</span>
      </div>
    </div>
    <div class="col-sm-6 col-md-4">
      <div class="detail-field">
        <span class="detail-label"
          ><i class="fa-solid fa-calendar me-1"></i>Ngày tạo</span
        >
        <span class="detail-value">{fmtDate(u.created_on)}</span>
      </div>
    </div>
  </div>
</div>

<!-- ── Role change modal ───────────────────────────────────────────────────── -->
{#if showRoleModal}
  <div
    class="modal-backdrop show"
    onclick={() => { showRoleModal = false; }}
    role="presentation"
  ></div>
  <div class="modal d-block" tabindex="-1" role="dialog" aria-modal="true">
    <div class="modal-dialog modal-dialog-centered">
      <div class="modal-content rounded-3">
        <div class="modal-header">
          <Heading level={5} class="modal-title"
            ><i class="fa-solid fa-shield-halved text-primary me-2"></i>Đổi vai
            trò</Heading
          >
          <button
            type="button"
            class="btn-close"
            aria-label="Đóng"
            onclick={() => (showRoleModal = false)}
          ></button>
        </div>
        <form
          method="POST"
          action="?/update_role"
          use:enhance={() => {
            submittingRole = true;
            return async ({ update }) => {
              submittingRole = false;
              await update();
            };
          }}
        >
          <fieldset class="form-fieldset" disabled={submittingRole}>
            <div class="modal-body d-flex flex-column gap-3">
              <div class="alert alert-light border py-2 mb-0">
                <i class="fa-solid fa-user text-muted me-1"></i><strong
                  >{u.username}</strong
                >
              </div>
              <div>
                <label class="form-label fw-semibold" for="role_select"
                  >Vai trò mới</label
                >
                <select
                  id="role_select"
                  name="role_code"
                  class="form-select"
                  bind:value={selectedRole}
                >
                  {#each data.roles as r}
                    <option value={r.code}>{r.name}</option>
                  {/each}
                </select>
              </div>
            </div>
            <div class="modal-footer">
              <Button
                type="button"
                class="btn-outline-secondary"
                onclick={() => (showRoleModal = false)}>Huỷ</Button
              >
              <Button type="submit" class="btn-primary px-4">
                {#if submittingRole}
                  <Spinner class="me-1" />Đang lưu…
                {:else}
                  <i class="fa-solid fa-floppy-disk me-1"></i>Lưu
                {/if}
              </Button>
            </div>
          </fieldset>
        </form>
      </div>
    </div>
  </div>
{/if}

<!-- ── Deactivate confirmation modal ──────────────────────────────────────── -->
{#if confirmDeactivate}
  <ConfirmModal
    action="?/toggle_active"
    hiddenFields={{ is_active: 'false' }}
    title="Vô hiệu hoá tài khoản"
    message="Tài khoản này sẽ bị khoá và không thể đăng nhập cho đến khi được kích hoạt lại."
    targetName={u.username}
    icon="fa-solid fa-user-slash text-warning"
    confirmLabel="Vô hiệu hoá"
    confirmClass="btn-outline-warning"
    requireReason={true}
    reasonLabel="Lý do vô hiệu hoá"
    reasonPlaceholder="vd: Vi phạm điều khoản, tài khoản nghi ngờ..."
    onCancel={() => (confirmDeactivate = false)}
  />
{/if}

<!-- ── Permissions panel ────────────────────────────────────────────────── -->
{#if data.allPerms.length > 0}
  <div class="admin-card mx-4 mt-3 p-4">
    <Heading level={3} class="perm-panel-title">
      <i class="fa-solid fa-key me-2 text-success"></i>Quyền hạn
    </Heading>
    <p class="text-muted small mb-3">
      Bật / tắt từng quyền hạn cho tài khoản này.
    </p>

    {#if (form as any)?.success && (form as any)?.action?.includes('permission') && (form as any)?.message}
      <div class="alert alert-success py-2 small mb-3">
        <i class="fa-solid fa-circle-check me-1"></i>{(form as any).message}
      </div>
    {/if}
    {#if (form as any) && !(form as any)?.success && (form as any)?.action?.includes('permission')}
      <div class="alert alert-danger py-2 small mb-3">
        <i class="fa-solid fa-triangle-exclamation me-1"></i>{(form as any).error}
      </div>
    {/if}

    <div class="perm-grid">
      {#each data.allPerms as p}
        {@const granted = grantedIds.has(p.id)}
        {@const toggling = togglingPermId === p.id}
        <div class="perm-row {granted ? 'perm-row--on' : ''} {toggling ? 'perm-row--loading' : ''}">
          <div class="perm-row__info">
            <span class="perm-row__code">{p.code}</span>
            <span class="perm-row__name">{p.name}</span>
          </div>
          <form
            method="POST"
            action={granted ? '?/revoke_permission' : '?/assign_permission'}
            use:enhance={() => {
              togglingPermId = p.id;
              return async ({ update }) => {
                await update({ invalidateAll: false });
              };
            }}
          >
            <input type="hidden" name="permission_id" value={p.id} />
            <button
              type="submit"
              class="perm-toggle {granted ? 'perm-toggle--on' : 'perm-toggle--off'}"
              title={granted ? 'Thu hồi quyền' : 'Cấp quyền'}
              aria-label={granted ? 'Thu hồi quyền' : 'Cấp quyền'}
              disabled={toggling}
            >
              {#if toggling}
                <span class="perm-spinner" role="status" aria-hidden="true"></span>
              {:else}
                <i class="fa-solid {granted ? 'fa-toggle-on' : 'fa-toggle-off'}"></i>
              {/if}
            </button>
          </form>
        </div>
      {/each}
    </div>
  </div>
{/if}

<style>
  .user-avatar {
    width: 56px;
    height: 56px;
    flex-shrink: 0;
    background: #eef2ff;
    border-radius: 14px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #4f46e5;
  }

  :global(.perm-panel-title) {
    font-size: 1rem;
    font-weight: 700;
    color: #0c1a12;
    margin: 0 0 4px;
  }

  .perm-grid {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .perm-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 14px;
    border-radius: 8px;
    border: 1px solid #e5e7eb;
    background: #f9fafb;
    transition: background 0.1s, border-color 0.1s;
  }

  .perm-row--on {
    background: #f0fdf4;
    border-color: #bbf7d0;
  }

  .perm-row--loading {
    opacity: 0.5;
    pointer-events: none;
    cursor: not-allowed;
  }

  .perm-row__info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .perm-row__code {
    font-size: 0.72rem;
    font-family: monospace;
    color: #4b5563;
    background: #f3f4f6;
    padding: 1px 5px;
    border-radius: 3px;
    width: fit-content;
  }

  .perm-row__name {
    font-size: 0.83rem;
    color: #374151;
  }

  .perm-toggle {
    background: none;
    border: none;
    cursor: pointer;
    font-size: 1.5rem;
    padding: 0 4px;
    line-height: 1;
    flex-shrink: 0;
    transition: color 0.15s;
    display: flex;
    align-items: center;
    justify-content: center;
    min-width: 32px;
    min-height: 32px;
  }

  .perm-toggle:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .perm-toggle--on {
    color: #16a34a;
  }

  .perm-toggle--off {
    color: #9ca3af;
  }

  .perm-toggle:hover.perm-toggle--off {
    color: #6b7280;
  }

  .perm-spinner {
    display: inline-block;
    width: 1.1rem;
    height: 1.1rem;
    border: 0.18em solid #16a34a;
    border-right-color: transparent;
    border-radius: 50%;
    animation: perm-spin 0.65s linear infinite;
  }

  @keyframes perm-spin {
    to { transform: rotate(360deg); }
  }
</style>
