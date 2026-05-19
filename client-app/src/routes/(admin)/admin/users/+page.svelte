<script lang="ts">
  import { enhance } from "$app/forms";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import AdminPageHeader from "$lib/components/molecules/AdminPageHeader.svelte";
  import Button from "$lib/components/atoms/Button.svelte";
  import Heading from "$lib/components/atoms/Heading.svelte";
  import Text from "$lib/components/atoms/Text.svelte";
  import type { PageData, ActionData } from "./$types";
  import type { AdminUser } from "./+page.server";
  import { ROUTES } from "$lib/routes";

  const { data, form }: { data: PageData; form: ActionData } = $props();

  function fmtDate(iso: string): string {
    return new Date(iso).toLocaleDateString("vi-VN", { day: "2-digit", month: "2-digit", year: "numeric" });
  }

  function goToPage(p: number) {
    const params = new URLSearchParams(page.url.searchParams);
    params.set("page", String(p));
    goto(`?${params.toString()}`, { replaceState: false });
  }

  let roleTarget = $state<AdminUser | null>(null);
  let selectedRole = $state("");

  function openRoleModal(u: AdminUser) {
    roleTarget = u;
    selectedRole = u.role_code;
  }
  function closeRoleModal() {
    roleTarget = null;
  }

  $effect(() => {
    if ((form as any)?.success) roleTarget = null;
  });
</script>

<AdminPageHeader
  title="Người dùng"
  icon="fa-users"
  sub="Quản lý tài khoản và vai trò người dùng"
>
  <span class="badge bg-secondary rounded-pill fs-6"
    >{data.result.total} tài khoản</span
  >
</AdminPageHeader>

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

<div class="admin-card mx-2 mx-md-4 mt-3 mt-md-4">
  {#if data.result.items.length === 0}
    <div class="text-center py-5 text-muted">
      <i class="fa-solid fa-users fs-1 d-block mb-3 opacity-50"></i>
      <Text class="mb-0 fw-semibold">Chưa có người dùng nào.</Text>
    </div>
  {:else}
    <div class="table-responsive">
      <table class="table table-hover align-middle mb-0">
        <thead class="admin-table-head">
          <tr>
            <th>#</th>
            <th>Người dùng</th>
            <th class="text-center">Vai trò / Trạng thái</th>
            <th class="text-center">Tạo bởi</th>
            <th class="text-center">Cập nhật bởi</th>
            <th class="text-center">Hành động</th>
          </tr>
        </thead>
        <tbody>
          {#each data.result.items as user (user.id)}
            <tr class:table-warning={!user.is_active}>
              <td class="text-muted small">{user.id}</td>
              <td>
                <div class="fw-bold">{user.username}</div>
                {#if user.full_name}
                  <div class="small text-muted">{user.full_name}</div>
                {/if}
                <div class="small text-muted">{user.email}</div>
              </td>
              <td class="text-center">
                <div class="d-flex flex-column align-items-center gap-1">
                  <span class="badge bg-primary-subtle text-primary border border-primary-subtle">
                    {user.role_name}
                  </span>
                  {#if user.is_active}
                    <span class="badge bg-success-subtle text-success border border-success-subtle">Hoạt động</span>
                  {:else}
                    <span class="badge bg-secondary-subtle text-secondary border border-secondary-subtle">Tắt</span>
                  {/if}
                </div>
              </td>
              <td class="text-center">
                <div class="d-flex flex-column" style="font-size:0.75rem; color:#5e7a68">
                  {#if user.created_by_name}
                    <span class="fw-semibold text-dark">{user.created_by_name}</span>
                  {/if}
                  <span>{fmtDate(user.created_on)}</span>
                </div>
              </td>
              <td class="text-center">
                <div class="d-flex flex-column" style="font-size:0.75rem; color:#5e7a68">
                  {#if user.updated_by_name}
                    <span class="fw-semibold text-dark">{user.updated_by_name}</span>
                  {/if}
                  <span>{fmtDate(user.updated_on)}</span>
                </div>
              </td>
              <td class="text-center">
                <div class="d-flex gap-2 justify-content-center">
                  <Button
                    href={ROUTES.ADMIN.USER(user.id)}
                    class="btn-sm btn-outline-secondary"
                    title="Chi tiết"
                  >
                    <i class="fa-solid fa-eye"></i>
                  </Button>
                  <Button
                    type="button"
                    class="btn-sm btn-outline-primary"
                    onclick={() => openRoleModal(user)}
                    title="Đổi vai trò"
                  >
                    <i class="fa-solid fa-shield-halved"></i>
                  </Button>
                  <form method="POST" action="?/toggle_active" use:enhance>
                    <input type="hidden" name="id" value={user.id} />
                    <input
                      type="hidden"
                      name="is_active"
                      value={String(!user.is_active)}
                    />
                    <Button
                      type="submit"
                      class="btn-sm {user.is_active
                        ? 'btn-outline-secondary'
                        : 'btn-outline-success'}"
                      title={user.is_active ? "Vô hiệu hoá" : "Kích hoạt"}
                    >
                      <i
                        class="fa-solid {user.is_active
                          ? 'fa-toggle-on'
                          : 'fa-toggle-off'}"
                      ></i>
                    </Button>
                  </form>
                </div>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

{#if data.result.total_pages > 1}
  <nav
    class="d-flex justify-content-center align-items-center gap-1 my-3"
    aria-label="Phân trang"
  >
    <Button
      class="btn-sm btn-outline-secondary"
      onclick={() => goToPage(data.result.page - 1)}
      disabled={data.result.page <= 1}
      ><i class="fa-solid fa-chevron-left"></i></Button
    >
    {#each Array.from({ length: data.result.total_pages }, (_, i) => i + 1) as p}
      <Button
        class="btn-sm {p === data.result.page
          ? 'btn-success'
          : 'btn-outline-secondary'}"
        onclick={() => goToPage(p)}>{p}</Button
      >
    {/each}
    <Button
      class="btn-sm btn-outline-secondary"
      onclick={() => goToPage(data.result.page + 1)}
      disabled={data.result.page >= data.result.total_pages}
      ><i class="fa-solid fa-chevron-right"></i></Button
    >
  </nav>
{/if}

{#if roleTarget}
  <div
    class="modal-backdrop show"
    onclick={closeRoleModal}
    role="presentation"
  ></div>
  <div class="modal d-block" tabindex="-1" role="dialog" aria-modal="true">
    <div class="modal-dialog modal-dialog-centered">
      <div class="modal-content rounded-3">
        <div class="modal-header">
          <Heading level={5} class="modal-title">
            <i class="fa-solid fa-shield-halved text-primary me-2"></i>Đổi vai
            trò
          </Heading>
          <button type="button" class="btn-close" onclick={closeRoleModal}
          ></button>
        </div>
        <form method="POST" action="?/update_role" use:enhance>
          <input type="hidden" name="id" value={roleTarget.id} />
          <div class="modal-body d-flex flex-column gap-3">
            <div class="alert alert-light border py-2 mb-0">
              <i class="fa-solid fa-user text-muted me-1"></i>
              <strong>{roleTarget.username}</strong>
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
              onclick={closeRoleModal}>Huỷ</Button
            >
            <Button type="submit" class="btn-primary px-4">
              <i class="fa-solid fa-floppy-disk me-1"></i>Lưu
            </Button>
          </div>
        </form>
      </div>
    </div>
  </div>
{/if}
