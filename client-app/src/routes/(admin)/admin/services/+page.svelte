<script lang="ts">
  import { enhance } from "$app/forms";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import AdminPageHeader from "$lib/components/molecules/AdminPageHeader.svelte";
  import ConfirmModal from "$lib/components/molecules/ConfirmModal.svelte";
  import Button from "$lib/components/atoms/Button.svelte";
  import Heading from "$lib/components/atoms/Heading.svelte";
  import Spinner from "$lib/components/atoms/Spinner.svelte";
  import Text from "$lib/components/atoms/Text.svelte";
  import type { PageData, ActionData } from "./$types";
  import type { AdminServiceEntry } from "./+page.server";

  const { data, form }: { data: PageData; form: ActionData } = $props();

  let showCreate      = $state(false);
  let editTarget      = $state<AdminServiceEntry | null>(null);
  let confirmDelete   = $state<{ id: number; name: string } | null>(null);
  let confirmHide     = $state<{ id: number; name: string } | null>(null);
  let loadingShowId   = $state<number | null>(null);
  let submitting      = $state(false);

  $effect(() => {
    if ((form as any)?.success) {
      showCreate = false;
      editTarget = null;
    }
  });

  function goToPage(p: number) {
    const params = new URLSearchParams(page.url.searchParams);
    params.set("page", String(p));
    goto(`?${params.toString()}`, { replaceState: false });
  }

  function fmtDate(iso: string): string {
    return new Date(iso).toLocaleDateString("vi-VN", { day: "2-digit", month: "2-digit", year: "numeric" });
  }
</script>

<svelte:head>
  <meta name="robots" content="noindex" />
</svelte:head>

<AdminPageHeader
  title="Dịch vụ"
  icon="fa-screwdriver-wrench"
  sub="Quản lý tất cả dịch vụ của các nhà cung cấp"
>
  <div class="d-flex align-items-center gap-2">
    {#if data.result}
      <span class="badge bg-secondary rounded-pill fs-6">{data.result.total} dịch vụ</span>
    {/if}
    <Button type="button" class="btn-success" onclick={() => (showCreate = true)}>
      <i class="fa-solid fa-plus me-1"></i>Thêm dịch vụ
    </Button>
  </div>
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
        <i class="fa-solid fa-screwdriver-wrench fs-1 d-block mb-3 opacity-50"></i>
        <Text class="mb-0 fw-semibold">Chưa có dịch vụ nào.</Text>
      </div>
    {:else}
      <div class="table-responsive">
        <table class="table table-hover align-middle mb-0">
          <thead class="admin-table-head">
            <tr>
              <th>#</th>
              <th>Dịch vụ</th>
              <th>Nhà cung cấp</th>
              <th>Ghi chú giá</th>
              <th class="text-center">Ngày tạo</th>
              <th class="text-center">Trạng thái</th>
              <th class="text-center">Hành động</th>
            </tr>
          </thead>
          <tbody>
            {#each data.result.items as s (s.id)}
              <tr class:table-warning={!s.is_active}>
                <td class="text-muted small">{s.id}</td>
                <td>
                  <div class="fw-semibold">{s.name}</div>
                  <span class="badge bg-info-subtle text-info border border-info-subtle small">{s.category}</span>
                </td>
                <td>
                  <a
                    href="/admin/explore/{s.business_id}"
                    class="text-success fw-semibold text-decoration-none"
                  >{s.business_name}</a>
                </td>
                <td class="text-muted small">{s.price_note ?? '—'}</td>
                <td class="text-center small text-muted">{fmtDate(s.created_on)}</td>
                <td class="text-center">
                  {#if s.is_active}
                    <span class="badge bg-success-subtle text-success border border-success-subtle">Hiển thị</span>
                  {:else}
                    <span class="badge bg-secondary-subtle text-secondary border border-secondary-subtle">Ẩn</span>
                  {/if}
                </td>
                <td class="text-center">
                  <div class="d-flex gap-2 justify-content-center">
                    <Button
                      type="button"
                      class="btn-sm btn-outline-primary"
                      title="Sửa"
                      onclick={() => { editTarget = { ...s }; }}
                    >
                      <i class="fa-solid fa-pen-to-square"></i>
                    </Button>
                    {#if s.is_active}
                      <!-- Hide: requires confirmation with reason -->
                      <Button
                        type="button"
                        class="btn-sm btn-outline-warning"
                        title="Ẩn dịch vụ"
                        onclick={() => confirmHide = { id: s.id, name: s.name }}
                      >
                        <i class="fa-solid fa-eye-slash"></i>
                      </Button>
                    {:else}
                      <!-- Show: direct submit with loading indicator -->
                      <form
                        method="POST"
                        action="?/toggle_active"
                        use:enhance={() => {
                          loadingShowId = s.id;
                          return async ({ update }) => {
                            loadingShowId = null;
                            await update();
                          };
                        }}
                      >
                        <input type="hidden" name="id" value={s.id} />
                        <input type="hidden" name="is_active" value="true" />
                        <Button
                          type="submit"
                          class="btn-sm btn-outline-success"
                          title="Hiển thị"
                          disabled={loadingShowId === s.id}
                        >
                          {#if loadingShowId === s.id}
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
                      onclick={() => confirmDelete = { id: s.id, name: s.name }}
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

<!-- Create modal -->
{#if showCreate}
  <div class="modal-backdrop show" onclick={() => (showCreate = false)} role="presentation"></div>
  <div class="modal d-block" tabindex="-1" role="dialog" aria-modal="true">
    <div class="modal-dialog modal-dialog-centered">
      <div class="modal-content rounded-3">
        <div class="modal-header">
          <Heading level={5} class="modal-title">
            <i class="fa-solid fa-plus text-success me-2"></i>Thêm dịch vụ
          </Heading>
          <button type="button" class="btn-close" aria-label="Đóng" onclick={() => (showCreate = false)}></button>
        </div>
        <form
          method="POST"
          action="?/create"
          use:enhance={() => {
            submitting = true;
            return async ({ update }) => { submitting = false; await update(); };
          }}
        >
          <fieldset class="form-fieldset" disabled={submitting}>
            <div class="modal-body d-flex flex-column gap-3">
              <div>
                <label class="form-label fw-semibold" for="c_biz_id">
                  ID Nhà cung cấp <span class="text-danger">*</span>
                </label>
                <input id="c_biz_id" name="business_id" type="number" class="form-control" min="1" required />
                <div class="form-text">Nhập ID của nhà cung cấp từ danh sách Nhà cung cấp.</div>
              </div>
              <div>
                <label class="form-label fw-semibold" for="c_cat">
                  Danh mục <span class="text-danger">*</span>
                </label>
                <select id="c_cat" name="category_id" class="form-select" required>
                  <option value="">— Chọn danh mục —</option>
                  {#each data.serviceCategories as cat}
                    <option value={cat.id}>{cat.name}</option>
                  {/each}
                </select>
              </div>
              <div>
                <label class="form-label fw-semibold" for="c_name">
                  Tên dịch vụ <span class="text-danger">*</span>
                </label>
                <input id="c_name" name="name" type="text" class="form-control" required />
              </div>
              <div>
                <label class="form-label fw-semibold" for="c_price">Ghi chú giá</label>
                <input id="c_price" name="price_note" type="text" class="form-control" placeholder="vd: 500.000đ/công" />
              </div>
              <div>
                <label class="form-label fw-semibold" for="c_desc">Mô tả</label>
                <textarea id="c_desc" name="description" class="form-control" rows="3"></textarea>
              </div>
            </div>
            <div class="modal-footer">
              <Button type="button" class="btn-outline-secondary" onclick={() => (showCreate = false)}>Huỷ</Button>
              <Button type="submit" class="btn-success px-4">
                {#if submitting}<Spinner class="me-1" />Đang tạo…{:else}<i class="fa-solid fa-plus me-1"></i>Tạo{/if}
              </Button>
            </div>
          </fieldset>
        </form>
      </div>
    </div>
  </div>
{/if}

<!-- Edit modal -->
{#if editTarget}
  <div class="modal-backdrop show" onclick={() => editTarget = null} role="presentation"></div>
  <div class="modal d-block" tabindex="-1" role="dialog" aria-modal="true">
    <div class="modal-dialog modal-dialog-centered">
      <div class="modal-content rounded-3">
        <div class="modal-header">
          <Heading level={5} class="modal-title">
            <i class="fa-solid fa-pen-to-square text-primary me-2"></i>Sửa dịch vụ
          </Heading>
          <button type="button" class="btn-close" aria-label="Đóng" onclick={() => editTarget = null}></button>
        </div>
        <form
          method="POST"
          action="?/update"
          use:enhance={() => {
            submitting = true;
            return async ({ update }) => { submitting = false; await update(); };
          }}
        >
          <fieldset class="form-fieldset" disabled={submitting}>
            <input type="hidden" name="id" value={editTarget.id} />
            <div class="modal-body d-flex flex-column gap-3">
              <div>
                <label class="form-label fw-semibold" for="e_cat">
                  Danh mục <span class="text-danger">*</span>
                </label>
                <select id="e_cat" name="category_id" class="form-select" required>
                  {#each data.serviceCategories as cat}
                    <option value={cat.id} selected={cat.id === editTarget.category_id}>{cat.name}</option>
                  {/each}
                </select>
              </div>
              <div>
                <label class="form-label fw-semibold" for="e_name">
                  Tên dịch vụ <span class="text-danger">*</span>
                </label>
                <input id="e_name" name="name" type="text" class="form-control" value={editTarget.name} required />
              </div>
              <div>
                <label class="form-label fw-semibold" for="e_price">Ghi chú giá</label>
                <input id="e_price" name="price_note" type="text" class="form-control" value={editTarget.price_note ?? ''} />
              </div>
              <div>
                <label class="form-label fw-semibold" for="e_desc">Mô tả</label>
                <textarea id="e_desc" name="description" class="form-control" rows="3">{editTarget.description ?? ''}</textarea>
              </div>
            </div>
            <div class="modal-footer">
              <Button type="button" class="btn-outline-secondary" onclick={() => editTarget = null}>Huỷ</Button>
              <Button type="submit" class="btn-primary px-4">
                {#if submitting}<Spinner class="me-1" />Đang lưu…{:else}<i class="fa-solid fa-floppy-disk me-1"></i>Lưu{/if}
              </Button>
            </div>
          </fieldset>
        </form>
      </div>
    </div>
  </div>
{/if}

<!-- Hide confirmation modal -->
{#if confirmHide}
  <ConfirmModal
    action="?/toggle_active"
    hiddenFields={{ id: confirmHide.id, is_active: 'false' }}
    title="Ẩn dịch vụ"
    message="Dịch vụ này sẽ bị ẩn khỏi giao diện người dùng."
    targetName={confirmHide.name}
    icon="fa-solid fa-eye-slash text-warning"
    confirmLabel="Ẩn dịch vụ"
    confirmClass="btn-outline-warning"
    requireReason={true}
    reasonLabel="Lý do ẩn dịch vụ"
    reasonPlaceholder="vd: Tạm ngưng cung cấp, thông tin không chính xác..."
    onCancel={() => confirmHide = null}
  />
{/if}

<!-- Delete confirmation modal -->
{#if confirmDelete}
  <ConfirmModal
    action="?/delete"
    hiddenFields={{ id: confirmDelete.id }}
    title="Xác nhận xoá dịch vụ"
    message="Dịch vụ này sẽ bị xoá vĩnh viễn. Hành động này không thể hoàn tác."
    targetName={confirmDelete.name}
    icon="fa-solid fa-triangle-exclamation text-danger"
    confirmLabel="Xoá dịch vụ"
    confirmClass="btn-outline-danger"
    requireReason={true}
    reasonLabel="Lý do xoá"
    reasonPlaceholder="vd: Dịch vụ trùng lặp, nhà cung cấp yêu cầu..."
    onCancel={() => confirmDelete = null}
  />
{/if}
