<script lang="ts">
  import AdminPageHeader from "$lib/components/molecules/AdminPageHeader.svelte";
  import Button from '$lib/components/atoms/Button.svelte';
  import Text from '$lib/components/atoms/Text.svelte';
  import type { PageData } from "./$types";
  import { ROUTES } from '$lib/routes';

  const { data }: { data: PageData } = $props();
</script>

<AdminPageHeader
  title="Vai trò"
  icon="fa-shield-halved"
  sub="Vai trò hệ thống — được quản lý qua migration, không thay đổi tại đây."
/>

{#if data.error}
  <div class="alert alert-danger mx-4 mb-0">
    <i class="fa-solid fa-triangle-exclamation me-1"></i>{data.error}
  </div>
{/if}

<div class="admin-card mx-4 mt-4">
  {#if data.roles.length === 0}
    <div class="text-center py-5 text-muted">
      <i class="fa-solid fa-shield-halved fs-1 d-block mb-3 opacity-50"></i>
      <Text class="mb-0 fw-semibold">Chưa có vai trò nào.</Text>
    </div>
  {:else}
    <div class="table-responsive">
      <table class="table table-hover align-middle mb-0">
        <thead class="admin-table-head">
          <tr>
            <th>#</th>
            <th>Mã</th>
            <th>Tên</th>
            <th>Mô tả</th>
            <th class="text-center">Trạng thái</th>
            <th class="text-center">Hành động</th>
          </tr>
        </thead>
        <tbody>
          {#each data.roles as role (role.id)}
            <tr class:table-warning={!role.is_active}>
              <td class="text-muted small">{role.id}</td>
              <td
                ><code class="badge bg-light text-dark border">{role.code}</code
                ></td
              >
              <td class="fw-semibold">{role.name}</td>
              <td class="text-muted small">{role.description ?? "—"}</td>
              <td class="text-center">
                {#if role.is_active}
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
              </td>
              <td class="text-center">
                <Button
                  href={ROUTES.ADMIN.ROLE(role.id)}
                  class="btn btn-sm btn-outline-secondary"
                >
                  <i class="fa-solid fa-eye"></i>
                </Button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

