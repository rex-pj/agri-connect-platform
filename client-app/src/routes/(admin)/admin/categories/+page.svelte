<script lang="ts">
  import { enhance } from "$app/forms";
  import Spinner from "$lib/components/atoms/Spinner.svelte";
  import AdminPageHeader from "$lib/components/molecules/AdminPageHeader.svelte";
  import Button from '$lib/components/atoms/Button.svelte';
  import Heading from '$lib/components/atoms/Heading.svelte';
  import Text from '$lib/components/atoms/Text.svelte';
  import type { PageData, ActionData } from "./$types";
  import type { AdminCategory } from "./+page.server";
  import { ROUTES } from '$lib/routes';

  const { data, form }: { data: PageData; form: ActionData } = $props();

  let showCreate = $state(false);
  let editTarget = $state<AdminCategory | null>(null);

  let submittingCreate = $state(false);
  let submittingUpdate = $state(false);

  function openEdit(c: AdminCategory) {
    editTarget = { ...c };
  }
  function closeEdit() {
    editTarget = null;
  }

  $effect(() => {
    if ((form as any)?.success) {
      showCreate = false;
      editTarget = null;
    }
  });

  const typeLabel: Record<string, string> = {
    service: "Dịch vụ",
    product: "Sản phẩm",
  };
</script>

<AdminPageHeader title="Danh mục" icon="fa-tags" sub="Quản lý danh mục dịch vụ và sản phẩm">
  <Button type="button" class="btn btn-success" onclick={() => (showCreate = true)}>
    <i class="fa-solid fa-plus me-1"></i>Thêm danh mục
  </Button>
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
  {#if data.categories.length === 0}
    <div class="text-center py-5 text-muted">
      <i class="fa-solid fa-tags fs-1 d-block mb-3 opacity-50"></i>
      <Text class="mb-0 fw-semibold">Chưa có danh mục nào.</Text>
    </div>
  {:else}
    <div class="table-responsive">
      <table class="table table-hover align-middle mb-0">
        <thead class="admin-table-head">
          <tr>
            <th>#</th>
            <th>Tên</th>
            <th>Slug</th>
            <th class="text-center">Loại</th>
            <th>Icon</th>
            <th class="text-center">Thứ tự</th>
            <th class="text-center">Trạng thái</th>
            <th class="text-center">Hành động</th>
          </tr>
        </thead>
        <tbody>
          {#each data.categories as cat (cat.id)}
            <tr class:table-warning={!cat.is_active}>
              <td class="text-muted small">{cat.id}</td>
              <td class="fw-semibold">{cat.name}</td>
              <td><code class="small text-muted">{cat.slug}</code></td>
              <td class="text-center">
                <span
                  class="badge {cat.category_type === 'service'
                    ? 'bg-info-subtle text-info border border-info-subtle'
                    : 'bg-warning-subtle text-warning border border-warning-subtle'}"
                >
                  {typeLabel[cat.category_type] ?? cat.category_type}
                </span>
              </td>
              <td class="small text-muted">{cat.icon ?? "—"}</td>
              <td class="text-center">{cat.display_order}</td>
              <td class="text-center">
                {#if cat.is_active}
                  <span
                    class="badge bg-success-subtle text-success border border-success-subtle"
                    >Hiển thị</span
                  >
                {:else}
                  <span
                    class="badge bg-secondary-subtle text-secondary border border-secondary-subtle"
                    >Ẩn</span
                  >
                {/if}
              </td>
              <td class="text-center">
                <div class="d-flex gap-2 justify-content-center">
                  <Button
                    href={ROUTES.ADMIN.CATEGORY(cat.id)}
                    class="btn btn-sm btn-outline-secondary"
                    title="Chi tiết"
                  >
                    <i class="fa-solid fa-eye"></i>
                  </Button>
                  <Button
                    type="button"
                    class="btn btn-sm btn-outline-primary"
                    onclick={() => openEdit(cat)}
                  >
                    <i class="fa-solid fa-pen-to-square"></i>
                  </Button>
                  <form method="POST" action="?/toggle_active" use:enhance>
                    <input type="hidden" name="id" value={cat.id} />
                    <input
                      type="hidden"
                      name="is_active"
                      value={String(!cat.is_active)}
                    />
                    <Button
                      type="submit"
                      class="btn btn-sm {cat.is_active ? 'btn-outline-secondary' : 'btn-outline-success'}"
                      title={cat.is_active ? "Ẩn" : "Hiện"}
                    >
                      <i class="fa-solid {cat.is_active ? 'fa-eye-slash' : 'fa-eye'}"></i>
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

<!-- Create modal -->
{#if showCreate}
  <div class="modal-backdrop show" onclick={() => (showCreate = false)} role="presentation"></div>
  <div class="modal d-block" tabindex="-1" role="dialog" aria-modal="true">
    <div class="modal-dialog modal-dialog-centered">
      <div class="modal-content rounded-3">
        <div class="modal-header">
          <Heading level={5} class="modal-title">
            <i class="fa-solid fa-plus text-success me-2"></i>Thêm danh mục
          </Heading>
          <button type="button" class="btn-close" onclick={() => (showCreate = false)}></button>
        </div>
        <form method="POST" action="?/create" use:enhance={() => { submittingCreate = true; return async ({ update }) => { submittingCreate = false; await update(); }; }}>
          <fieldset class="form-fieldset" disabled={submittingCreate}>
          <div class="modal-body d-flex flex-column gap-3">
            <div class="row g-3">
              <div class="col-8">
                <label class="form-label fw-semibold" for="c_name">Tên <span class="text-danger">*</span></label>
                <input id="c_name" name="name" type="text" class="form-control" required />
              </div>
              <div class="col-4">
                <label class="form-label fw-semibold" for="c_order">Thứ tự</label>
                <input id="c_order" name="display_order" type="number" class="form-control" value="0" min="0" />
              </div>
            </div>
            <div class="row g-3">
              <div class="col-8">
                <label class="form-label fw-semibold" for="c_slug">Slug <span class="text-danger">*</span></label>
                <input id="c_slug" name="slug" type="text" class="form-control" placeholder="vd: muc-bun" required />
              </div>
              <div class="col-4">
                <label class="form-label fw-semibold" for="c_type">Loại <span class="text-danger">*</span></label>
                <select id="c_type" name="category_type" class="form-select">
                  <option value="service">Dịch vụ</option>
                  <option value="product">Sản phẩm</option>
                </select>
              </div>
            </div>
            <div>
              <label class="form-label fw-semibold" for="c_icon">Icon (Bootstrap Icons class)</label>
              <input id="c_icon" name="icon" type="text" class="form-control" placeholder="vd: bi-water" />
            </div>
          </div>
          <div class="modal-footer">
            <Button type="button" class="btn btn-outline-secondary" onclick={() => (showCreate = false)}>Huỷ</Button>
            <Button type="submit" class="btn btn-success px-4">
              {#if submittingCreate}<Spinner class="me-1" />Đang tạo…{:else}<i class="fa-solid fa-plus me-1"></i>Tạo{/if}
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
  <div class="modal-backdrop show" onclick={closeEdit} role="presentation"></div>
  <div class="modal d-block" tabindex="-1" role="dialog" aria-modal="true">
    <div class="modal-dialog modal-dialog-centered">
      <div class="modal-content rounded-3">
        <div class="modal-header">
          <Heading level={5} class="modal-title">
            <i class="fa-solid fa-pen-to-square text-primary me-2"></i>Sửa danh mục
          </Heading>
          <button type="button" class="btn-close" onclick={closeEdit}></button>
        </div>
        <form method="POST" action="?/update" use:enhance={() => { submittingUpdate = true; return async ({ update }) => { submittingUpdate = false; await update(); }; }}>
          <fieldset class="form-fieldset" disabled={submittingUpdate}>
          <input type="hidden" name="id" value={editTarget.id} />
          <div class="modal-body d-flex flex-column gap-3">
            <div class="row g-3">
              <div class="col-8">
                <label class="form-label fw-semibold" for="e_name">Tên <span class="text-danger">*</span></label>
                <input id="e_name" name="name" type="text" class="form-control" value={editTarget.name} required />
              </div>
              <div class="col-4">
                <label class="form-label fw-semibold" for="e_order">Thứ tự</label>
                <input id="e_order" name="display_order" type="number" class="form-control" value={editTarget.display_order} min="0" />
              </div>
            </div>
            <div>
              <label class="form-label fw-semibold" for="e_slug">Slug <span class="text-danger">*</span></label>
              <input id="e_slug" name="slug" type="text" class="form-control" value={editTarget.slug} required />
            </div>
            <div>
              <label class="form-label fw-semibold" for="e_icon">Icon</label>
              <input id="e_icon" name="icon" type="text" class="form-control" value={editTarget.icon ?? ""} />
            </div>
          </div>
          <div class="modal-footer">
            <Button type="button" class="btn btn-outline-secondary" onclick={closeEdit}>Huỷ</Button>
            <Button type="submit" class="btn btn-primary px-4">
              {#if submittingUpdate}<Spinner class="me-1" />Đang lưu…{:else}<i class="fa-solid fa-floppy-disk me-1"></i>Lưu{/if}
            </Button>
          </div>
          </fieldset>
        </form>
      </div>
    </div>
  </div>
{/if}

