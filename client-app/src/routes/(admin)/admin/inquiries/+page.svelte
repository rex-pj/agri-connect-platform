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

  const statusLabel: Record<string, string> = {
    new: "Mới",
    contacted: "Đã liên hệ",
    closed: "Đã đóng",
    spam: "Spam",
  };

  const statusClass: Record<string, string> = {
    new: "bg-primary-subtle text-primary border-primary-subtle",
    contacted: "bg-success-subtle text-success border-success-subtle",
    closed: "bg-secondary-subtle text-secondary border-secondary-subtle",
    spam: "bg-danger-subtle text-danger border-danger-subtle",
  };
</script>

<AdminPageHeader title="Yêu cầu liên hệ" icon="fa-envelope-open-text" sub="Theo dõi và cập nhật trạng thái yêu cầu từ nông dân">
  <span class="badge bg-secondary rounded-pill fs-6">{data.result.total} yêu cầu</span>
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
      <i class="fa-solid fa-envelope-open-text fs-1 d-block mb-3 opacity-50"
      ></i>
      <Text class="mb-0 fw-semibold">Chưa có yêu cầu nào.</Text>
    </div>
  {:else}
    <div class="table-responsive">
      <table class="table table-hover align-middle mb-0">
        <thead class="admin-table-head">
          <tr>
            <th>#</th>
            <th>Nhà cung cấp</th>
            <th>Người yêu cầu</th>
            <th>Số điện thoại</th>
            <th>Chủ đề</th>
            <th class="text-center">Ngày gửi</th>
            <th class="text-center">Trạng thái</th>
          </tr>
        </thead>
        <tbody>
          {#each data.result.items as inq (inq.id)}
            <tr>
              <td class="text-muted small">{inq.id}</td>
              <td class="fw-semibold">{inq.business_name}</td>
              <td>{inq.requester_name}</td>
              <td>
                <Button
                  href="tel:{inq.requester_phone}"
                  class="text-decoration-none"
                >
                  {inq.requester_phone}
                </Button>
              </td>
              <td
                class="text-muted small"
                style="max-width:200px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis"
              >
                {inq.topic}
              </td>
              <td class="text-center text-muted small"
                >{fmtDate(inq.created_on)}</td
              >
              <td class="text-center">
                <form
                  method="POST"
                  action="?/update_status"
                  use:enhance
                  class="d-inline-flex"
                >
                  <input type="hidden" name="id" value={inq.id} />
                  <select
                    name="status"
                    class="form-select form-select-sm status-select"
                    onchange={(e) => e.currentTarget.form?.requestSubmit()}
                    value={inq.status}
                  >
                    {#each Object.entries(statusLabel) as [val, label]}
                      <option value={val} selected={inq.status === val}
                        >{label}</option
                      >
                    {/each}
                  </select>
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
  .status-select { font-size: 0.8rem; min-width: 120px; }
</style>
