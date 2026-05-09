<script lang="ts">
  import { enhance } from "$app/forms";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import AdminPageHeader from "$lib/components/molecules/AdminPageHeader.svelte";
  import Button from '$lib/components/atoms/Button.svelte';
  import Text from '$lib/components/atoms/Text.svelte';
  import type { PageData, ActionData } from "./$types";

  const { data, form }: { data: PageData; form: ActionData } = $props();

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

<AdminPageHeader title="Đánh giá" icon="fa-star" sub="Kiểm duyệt đánh giá từ nông dân">
  <span class="badge bg-secondary rounded-pill fs-6">{data.result.total} đánh giá</span>
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
            <th class="text-center">Sao</th>
            <th>Nội dung</th>
            <th class="text-center">Ngày</th>
            <th class="text-center">Hiển thị</th>
          </tr>
        </thead>
        <tbody>
          {#each data.result.items as review (review.id)}
            <tr class:table-warning={!review.is_visible}>
              <td class="text-muted small">{review.id}</td>
              <td class="fw-semibold">{review.business_name}</td>
              <td>{review.reviewer_name}</td>
              <td class="text-center stars">{stars(review.rating)}</td>
              <td
                class="text-muted small"
                style="max-width:220px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis"
              >
                {review.comment ?? "—"}
              </td>
              <td class="text-center text-muted small"
                >{fmtDate(review.created_on)}</td
              >
              <td class="text-center">
                <form method="POST" action="?/toggle_visible" use:enhance>
                  <input type="hidden" name="id" value={review.id} />
                  <input
                    type="hidden"
                    name="is_visible"
                    value={String(!review.is_visible)}
                  />
                  <Button
                    type="submit"
                    class="btn btn-sm {review.is_visible ? 'btn-outline-secondary' : 'btn-outline-success'}"
                    title={review.is_visible ? "Ẩn đánh giá" : "Hiện đánh giá"}
                  >
                    <i
                      class="fa-solid {review.is_visible ? 'fa-eye-slash' : 'fa-eye'} me-1"
                    ></i>
                    {review.is_visible ? "Ẩn" : "Hiện"}
                  </Button>
                </form>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

{#if data.result.total_pages > 1}
  <nav class="d-flex justify-content-center align-items-center gap-1 my-3" aria-label="Phân trang">
    <Button type="button" class="btn btn-sm btn-outline-secondary" onclick={() => goToPage(data.result.page - 1)}
      disabled={data.result.page <= 1}><i class="fa-solid fa-chevron-left"></i></Button>
    {#each Array.from({ length: data.result.total_pages }, (_, i) => i + 1) as p}
      <Button type="button" class="btn btn-sm {p === data.result.page ? 'btn-success' : 'btn-outline-secondary'}"
        onclick={() => goToPage(p)}>{p}</Button>
    {/each}
    <Button type="button" class="btn btn-sm btn-outline-secondary" onclick={() => goToPage(data.result.page + 1)}
      disabled={data.result.page >= data.result.total_pages}><i class="fa-solid fa-chevron-right"></i></Button>
  </nav>
{/if}

<style>
  .stars { letter-spacing: 0.1em; color: #d97706; font-size: 1rem; }
</style>
