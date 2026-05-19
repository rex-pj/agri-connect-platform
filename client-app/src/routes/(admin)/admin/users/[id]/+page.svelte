<script lang="ts">
  import { enhance } from "$app/forms";
  import Button from "$lib/components/atoms/Button.svelte";
  import Heading from "$lib/components/atoms/Heading.svelte";
  import type { PageData, ActionData } from "./$types";
  import { ROUTES } from "$lib/routes";

  const { data, form }: { data: PageData; form: ActionData } = $props();

  const u = $derived(data.user);
  let showRoleModal = $state(false);
  let selectedRole = $state(data.user.role_code);
  let isActive = $state(data.user.is_active);

  $effect(() => {
    if ((form as any)?.success) {
      showRoleModal = false;
      if ((form as any).message?.includes("kích hoạt")) isActive = true;
      if ((form as any).message?.includes("vô hiệu")) isActive = false;
    }
  });

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
      <Button
        href={ROUTES.ADMIN.USERS}
        class="text-success fw-semibold text-decoration-none"
      >
        <i class="fa-solid fa-users me-1"></i>Người dùng
      </Button>
    </li>
    <li class="breadcrumb-item active" aria-current="page">{u.username}</li>
  </ol>
</nav>

<!-- ── Alert strip ─────────────────────────────────────────────────────────── -->
{#if (form as any)?.success && (form as any)?.message}
  <div class="alert alert-success d-flex align-items-center gap-2 mx-4 mb-0">
    <i class="fa-solid fa-circle-check flex-shrink-0"></i>{(form as any)
      .message}
  </div>
{/if}
{#if (form as any) && !(form as any)?.success && (form as any)?.error}
  <div class="alert alert-danger d-flex align-items-center gap-2 mx-4 mb-0">
    <i class="fa-solid fa-triangle-exclamation flex-shrink-0"></i>{(form as any)
      .error}
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
        onclick={() => (showRoleModal = true)}
      >
        <i class="fa-solid fa-shield-halved me-1"></i>Đổi vai trò
      </Button>
      <form method="POST" action="?/toggle_active" use:enhance>
        <input type="hidden" name="is_active" value={String(!isActive)} />
        <Button
          type="submit"
          class={isActive ? "btn-outline-secondary" : "btn-outline-success"}
        >
          <i class="fa-solid {isActive ? 'fa-toggle-on' : 'fa-toggle-off'} me-1"
          ></i>
          {isActive ? "Vô hiệu hoá" : "Kích hoạt"}
        </Button>
      </form>
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
    onclick={() => (showRoleModal = false)}
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
            onclick={() => (showRoleModal = false)}
          ></button>
        </div>
        <form method="POST" action="?/update_role" use:enhance>
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
            <Button type="submit" class="btn-primary px-4"
              ><i class="fa-solid fa-floppy-disk me-1"></i>Lưu</Button
            >
          </div>
        </form>
      </div>
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
</style>
