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

  let confirmHide = $state<{ id: number; reviewer_name: string } | null>(null);
  let loadingShowId = $state<number | null>(null);

  function goToPage(p: number) {
    const params = new URLSearchParams(page.url.searchParams);
    params.set("page", String(p));
    goto(`?${params.toString()}`, { replaceState: false });
  }

  function fmtDate(iso: string): string {
    return new Date(iso).toLocaleDateString("vi-VN", {
      day: "2-digit",
      month: "2-digit",
      year: "numeric",
    });
  }

  function stars(n: number): string {
    return "★".repeat(n) + "☆".repeat(5 - n);
  }
</script>

<AdminPageHeader
  title="Đánh giá"
  icon="fa-star"
  sub="Kiểm duyệt đánh giá từ nông dân"
>
  <span class="badge bg-secondary rounded-pill fs-6"
    >{data.result.total} đánh giá</span
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

<div class="admin-card mx-4 mt-4">
  {#if data.result.items.length === 0}
    <div class="text-center py-5 text-muted">
      <i class="fa-solid fa-star fs-1 d-block mb-3 opacity-50"></i>
      <Text class="mb-0 fw-semibold">Chưa có đánh giá nào.</Text>
    </div>
  {:else}
    <div class="table-responsive">
      <table class="table table-hover align-middle mb-0">
        <thead class="admin-table-head">
          <tr>
            <th>#</th>
            <th>Nhà cung cấp</th>
            <th>Người đánh giá</th>
            <th>Nội dung</th>
            <th class="text-center">Tạo lúc</th>
            <th class="text-center">Cập nhật bởi</th>
            <th class="text-center">Hành động</th>
          </tr>
        </thead>
        <tbody>
          {#each data.result.items as review (review.id)}
            <tr class:table-warning={!review.is_visible}>
              <td class="text-muted small">{review.id}</td>
              <td class="fw-semibold">{review.business_name}</td>
              <td>
                <div>{review.reviewer_name}</div>
                <div class="stars" style="font-size:0.85rem">{stars(review.rating)}</div>
              </td>
              <td>
                <div
                  class="small"
                  style="max-width:220px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis"
                >
                  {review.comment ?? "—"}
                </div>
              </td>
              <td class="text-center">
                <div class="d-flex flex-column" style="font-size:0.75rem; color:#5e7a68">
                  <span>{fmtDate(review.created_on)}</span>
                </div>
              </td>
              <td class="text-center">
                <div class="d-flex flex-column" style="font-size:0.75rem; color:#5e7a68">
                  {#if review.updated_by_name}
                    <span class="fw-semibold text-dark">{review.updated_by_name}</span>
                  {/if}
                  <span>{fmtDate(review.updated_on)}</span>
                </div>
              </td>
              <td class="text-center">
                {#if review.is_visible}
                  <!-- Hide: requires confirmation with reason -->
                  <Button
                    type="button"
                    class="btn-sm btn-outline-warning"
                    title="Ẩn đánh giá"
                    onclick={() => confirmHide = { id: review.id, reviewer_name: review.reviewer_name }}
                  >
                    <i class="fa-solid fa-eye-slash me-1"></i>Ẩn
                  </Button>
                {:else}
                  <!-- Show: direct submit with loading indicator -->
                  <form
                    method="POST"
                    action="?/toggle_visible"
                    use:enhance={() => {
                      loadingShowId = review.id;
                      return async ({ update }) => {
                        loadingShowId = null;
                        await update();
                      };
                    }}
                  >
                    <input type="hidden" name="id" value={review.id} />
                    <input type="hidden" name="is_visible" value="true" />
                    <Button
                      type="submit"
                      class="btn-sm btn-outline-success"
                      title="Hiện đánh giá"
                      disabled={loadingShowId === review.id}
                    >
                      {#if loadingShowId === review.id}
                        <Spinner class="me-1" />
                      {:else}
                        <i class="fa-solid fa-eye me-1"></i>
                      {/if}
                      Hiện
                    </Button>
                  </form>
                {/if}
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
      type="button"
      class="btn-sm btn-outline-secondary"
      onclick={() => goToPage(data.result.page - 1)}
      disabled={data.result.page <= 1}
      ><i class="fa-solid fa-chevron-left"></i></Button
    >
    {#each Array.from({ length: data.result.total_pages }, (_, i) => i + 1) as p}
      <Button
        type="button"
        class="btn-sm {p === data.result.page
          ? 'btn-success'
          : 'btn-outline-secondary'}"
        onclick={() => goToPage(p)}>{p}</Button
      >
    {/each}
    <Button
      type="button"
      class="btn-sm btn-outline-secondary"
      onclick={() => goToPage(data.result.page + 1)}
      disabled={data.result.page >= data.result.total_pages}
      ><i class="fa-solid fa-chevron-right"></i></Button
    >
  </nav>
{/if}

{#if confirmHide}
  <ConfirmModal
    action="?/toggle_visible"
    hiddenFields={{ id: confirmHide.id, is_visible: 'false' }}
    title="Ẩn đánh giá"
    message="Đánh giá này sẽ bị ẩn khỏi trang nhà cung cấp và không hiển thị cho người dùng."
    targetName={confirmHide.reviewer_name}
    icon="fa-solid fa-eye-slash text-warning"
    confirmLabel="Ẩn đánh giá"
    confirmClass="btn-outline-warning"
    requireReason={true}
    reasonLabel="Lý do ẩn đánh giá"
    reasonPlaceholder="vd: Vi phạm quy định, nội dung không phù hợp..."
    onCancel={() => confirmHide = null}
  />
{/if}

<style>
  .stars {
    letter-spacing: 0.1em;
    color: #d97706;
    font-size: 1rem;
  }
</style>
