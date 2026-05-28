<script lang="ts">
  import { enhance } from "$app/forms";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import AdminPageHeader from "$lib/components/molecules/AdminPageHeader.svelte";
  import ConfirmModal from "$lib/components/molecules/ConfirmModal.svelte";
  import Button from "$lib/components/atoms/Button.svelte";
  import Spinner from "$lib/components/atoms/Spinner.svelte";
  import Text from "$lib/components/atoms/Text.svelte";
  import type { PageData, ActionData } from "./$types";

  const { data, form }: { data: PageData; form: ActionData } = $props();

  let confirmDelete = $state<{ id: number; name: string } | null>(null);
  let confirmHide = $state<{ id: number; name: string } | null>(null);
  let loadingShowId = $state<number | null>(null);

  function goToPage(p: number) {
    const params = new URLSearchParams(page.url.searchParams);
    params.set("page", String(p));
    goto(`?${params.toString()}`, { replaceState: false });
  }

  function fmtPrice(price: number | null, unit: string | null): string {
    if (price == null) return '—';
    const fmt = price.toLocaleString('vi-VN') + ' đ';
    return unit ? `${fmt} / ${unit}` : fmt;
  }

  function fmtDate(iso: string): string {
    return new Date(iso).toLocaleDateString("vi-VN", { day: "2-digit", month: "2-digit", year: "numeric" });
  }
</script>

<svelte:head>
  <meta name="robots" content="noindex" />
</svelte:head>

<AdminPageHeader
  title="Sản phẩm"
  icon="fa-box-open"
  sub="Quản lý tất cả sản phẩm của các nhà cung cấp"
>
  {#if data.result}
    <span class="badge bg-secondary rounded-pill fs-6">{data.result.total} sản phẩm</span>
  {/if}
</AdminPageHeader>

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
{#if data.error}
  <div class="alert alert-danger mx-4 mt-3">{data.error}</div>
{/if}

{#if data.result}
  <div class="admin-card mx-2 mx-md-4 mt-3 mt-md-4">
    {#if data.result.items.length === 0}
      <div class="text-center py-5 text-muted">
        <i class="fa-solid fa-box-open fs-1 d-block mb-3 opacity-50"></i>
        <Text class="mb-0 fw-semibold">Chưa có sản phẩm nào.</Text>
      </div>
    {:else}
      <div class="table-responsive">
        <table class="table table-hover align-middle mb-0">
          <thead class="admin-table-head">
            <tr>
              <th>#</th>
              <th>Sản phẩm</th>
              <th>Nhà cung cấp</th>
              <th class="text-end">Giá</th>
              <th class="text-center">Ngày tạo</th>
              <th class="text-center">Trạng thái</th>
              <th class="text-center">Hành động</th>
            </tr>
          </thead>
          <tbody>
            {#each data.result.items as p (p.id)}
              <tr class:table-warning={!p.is_active}>
                <td class="text-muted small">{p.id}</td>
                <td>
                  <div class="fw-semibold">{p.name}</div>
                  <span class="badge bg-warning-subtle text-warning border border-warning-subtle small">{p.category}</span>
                </td>
                <td>
                  <a
                    href="/admin/explore/{p.business_id}"
                    class="text-success fw-semibold text-decoration-none"
                  >{p.business_name}</a>
                </td>
                <td class="text-end text-muted small">{fmtPrice(p.price, p.unit)}</td>
                <td class="text-center small text-muted">{fmtDate(p.created_on)}</td>
                <td class="text-center">
                  {#if p.is_active}
                    <span class="badge bg-success-subtle text-success border border-success-subtle">Hiển thị</span>
                  {:else}
                    <span class="badge bg-secondary-subtle text-secondary border border-secondary-subtle">Ẩn</span>
                  {/if}
                </td>
                <td class="text-center">
                  <div class="d-flex gap-2 justify-content-center">
                    {#if p.is_active}
                      <!-- Hide: requires confirmation with reason -->
                      <Button
                        type="button"
                        class="btn-sm btn-outline-warning"
                        title="Ẩn sản phẩm"
                        onclick={() => confirmHide = { id: p.id, name: p.name }}
                      >
                        <i class="fa-solid fa-eye-slash"></i>
                      </Button>
                    {:else}
                      <!-- Show: direct submit with loading indicator -->
                      <form
                        method="POST"
                        action="?/toggle_active"
                        use:enhance={() => {
                          loadingShowId = p.id;
                          return async ({ update }) => {
                            loadingShowId = null;
                            await update();
                          };
                        }}
                      >
                        <input type="hidden" name="id" value={p.id} />
                        <input type="hidden" name="is_active" value="true" />
                        <Button
                          type="submit"
                          class="btn-sm btn-outline-success"
                          title="Hiển thị"
                          disabled={loadingShowId === p.id}
                        >
                          {#if loadingShowId === p.id}
                            <Spinner />
                          {:else}
                            <i class="fa-solid fa-eye"></i>
                          {/if}
                        </Button>
                      </form>
                    {/if}
                    <Button
                      type="button"
                      class="btn-sm btn-outline-danger"
                      title="Xoá"
                      onclick={() => confirmDelete = { id: p.id, name: p.name }}
                    >
                      <i class="fa-solid fa-trash"></i>
                    </Button>
                  </div>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>

      <!-- Pagination -->
      {#if data.result.total_pages > 1}
        <div class="d-flex justify-content-center gap-2 p-3">
          <Button
            type="button"
            class="btn-sm btn-outline-secondary"
            onclick={() => goToPage(data.result!.page - 1)}
            disabled={data.result.page <= 1}
          ><i class="fa-solid fa-chevron-left"></i></Button>
          {#each Array.from({ length: data.result.total_pages }, (_, i) => i + 1) as p}
            <Button
              type="button"
              class="btn-sm {data.result!.page === p ? 'btn-success' : 'btn-outline-secondary'}"
              onclick={() => goToPage(p)}
            >{p}</Button>
          {/each}
          <Button
            type="button"
            class="btn-sm btn-outline-secondary"
            onclick={() => goToPage(data.result!.page + 1)}
            disabled={data.result.page >= data.result.total_pages}
          ><i class="fa-solid fa-chevron-right"></i></Button>
        </div>
      {/if}
    {/if}
  </div>
{/if}

<!-- Hide confirmation modal -->
{#if confirmHide}
  <ConfirmModal
    action="?/toggle_active"
    hiddenFields={{ id: confirmHide.id, is_active: 'false' }}
    title="Ẩn sản phẩm"
    message="Sản phẩm này sẽ bị ẩn khỏi giao diện người dùng."
    targetName={confirmHide.name}
    icon="fa-solid fa-eye-slash text-warning"
    confirmLabel="Ẩn sản phẩm"
    confirmClass="btn-outline-warning"
    requireReason={true}
    reasonLabel="Lý do ẩn sản phẩm"
    reasonPlaceholder="vd: Hết hàng, thông tin không chính xác..."
    onCancel={() => confirmHide = null}
  />
{/if}

<!-- Delete confirmation modal -->
{#if confirmDelete}
  <ConfirmModal
    action="?/delete"
    hiddenFields={{ id: confirmDelete.id }}
    title="Xác nhận xoá sản phẩm"
    message="Sản phẩm này sẽ bị xoá vĩnh viễn. Hành động này không thể hoàn tác."
    targetName={confirmDelete.name}
    icon="fa-solid fa-triangle-exclamation text-danger"
    confirmLabel="Xoá sản phẩm"
    confirmClass="btn-outline-danger"
    requireReason={true}
    reasonLabel="Lý do xoá"
    reasonPlaceholder="vd: Sản phẩm trùng lặp, nhà cung cấp yêu cầu..."
    onCancel={() => confirmDelete = null}
  />
{/if}
