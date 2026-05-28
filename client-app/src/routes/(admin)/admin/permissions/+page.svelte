<script lang="ts">
  import Heading from "$lib/components/atoms/Heading.svelte";
  import Text from "$lib/components/atoms/Text.svelte";
  import type { PageData } from "./$types";

  const { data }: { data: PageData } = $props();

  const servicePerms = $derived(
    data.permissions.filter((p) => p.code.startsWith("Agri"))
  );
  const providerPerms = $derived(
    data.permissions.filter((p) => p.code.startsWith("Provider"))
  );
</script>

<svelte:head>
  <title>Quyền hạn — Agri Admin</title>
  <meta name="robots" content="noindex, nofollow" />
</svelte:head>

<div class="perm-header">
  <div>
    <Heading level={1} class="perm-title">
      <i class="fa-solid fa-key me-2 text-success"></i>Quyền hạn
    </Heading>
    <Text class="perm-sub">
      Danh sách các quyền hạn hệ thống. Gán quyền cho từng người dùng tại trang
      chi tiết tài khoản.
    </Text>
  </div>
</div>

<div class="perm-body">
  {#if data.permissions.length === 0}
    <div class="alert alert-warning">
      <i class="fa-solid fa-triangle-exclamation me-2"></i>Chưa có quyền hạn nào.
      Vui lòng nạp dữ liệu mẫu trước.
    </div>
  {:else}
    <div class="row g-4">
      <!-- Agri permissions -->
      <div class="col-md-6">
        <div class="perm-group">
          <div class="perm-group__header">
            <i class="fa-solid fa-store me-2 text-success"></i>Quyền nền tảng Agri
            <span class="badge bg-success-subtle text-success ms-auto"
              >{servicePerms.length}</span
            >
          </div>
          <ul class="perm-list">
            {#each servicePerms as p}
              <li class="perm-item">
                <code class="perm-code">{p.code}</code>
                <span class="perm-name">{p.name}</span>
              </li>
            {/each}
          </ul>
        </div>
      </div>

      <!-- Provider permissions -->
      <div class="col-md-6">
        <div class="perm-group">
          <div class="perm-group__header">
            <i class="fa-solid fa-user-tie me-2 text-primary"></i>Quyền nhà cung cấp
            <span class="badge bg-primary-subtle text-primary ms-auto"
              >{providerPerms.length}</span
            >
          </div>
          <ul class="perm-list">
            {#each providerPerms as p}
              <li class="perm-item">
                <code class="perm-code">{p.code}</code>
                <span class="perm-name">{p.name}</span>
              </li>
            {/each}
          </ul>
        </div>
      </div>
    </div>

    <div class="alert alert-light border mt-4 d-flex align-items-start gap-2">
      <i class="fa-solid fa-circle-info text-muted mt-1 flex-shrink-0"></i>
      <span class="small text-muted">
        Để gán hoặc thu hồi quyền hạn cho một tài khoản, vào <strong>Người
          dùng</strong> → chọn tài khoản → kéo xuống mục <strong>Quyền hạn</strong>.
      </span>
    </div>
  {/if}
</div>

<style>
  .perm-header {
    padding: 28px 24px 16px;
    border-bottom: 1px solid #e5e7eb;
    background: #fff;
  }
  :global(.perm-title) {
    font-size: 1.4rem;
    font-weight: 800;
    margin: 0 0 4px;
    color: #0c1a12;
  }
  :global(.perm-sub) {
    font-size: 0.875rem;
    color: #5e7a68;
    margin: 0;
  }

  .perm-body {
    padding: 24px;
  }

  .perm-group {
    background: #fff;
    border: 1px solid #e5e7eb;
    border-radius: 12px;
    overflow: hidden;
  }

  .perm-group__header {
    display: flex;
    align-items: center;
    padding: 12px 16px;
    background: #f9fafb;
    border-bottom: 1px solid #e5e7eb;
    font-size: 0.85rem;
    font-weight: 700;
    color: #374151;
  }

  .perm-list {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .perm-item {
    display: flex;
    align-items: baseline;
    gap: 12px;
    padding: 10px 16px;
    border-bottom: 1px solid #f3f4f6;
    font-size: 0.83rem;
  }

  .perm-item:last-child {
    border-bottom: none;
  }

  .perm-code {
    font-size: 0.75rem;
    background: #f3f4f6;
    color: #4b5563;
    padding: 2px 6px;
    border-radius: 4px;
    flex-shrink: 0;
    white-space: nowrap;
  }

  .perm-name {
    color: #374151;
    line-height: 1.4;
  }
</style>
