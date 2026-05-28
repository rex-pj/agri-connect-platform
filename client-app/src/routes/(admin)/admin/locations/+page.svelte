<script lang="ts">
  import { enhance } from "$app/forms";
  import Spinner from "$lib/components/atoms/Spinner.svelte";
  import AdminPageHeader from "$lib/components/molecules/AdminPageHeader.svelte";
  import ConfirmModal from "$lib/components/molecules/ConfirmModal.svelte";
  import Button from "$lib/components/atoms/Button.svelte";
  import Heading from "$lib/components/atoms/Heading.svelte";
  import Text from "$lib/components/atoms/Text.svelte";
  import type { PageData, ActionData } from "./$types";
  import type { AdminLocation } from "./+page.server";
  import { ROUTES } from "$lib/routes";

  const { data, form }: { data: PageData; form: ActionData } = $props();

  let showCreate = $state(false);
  let editTarget = $state<AdminLocation | null>(null);
  let confirmHide = $state<AdminLocation | null>(null);
  let loadingShowId = $state<number | null>(null);
  let submittingCreate = $state(false);
  let submittingUpdate = $state(false);

  function openEdit(loc: AdminLocation) { editTarget = { ...loc }; }
  function closeEdit() { editTarget = null; }

  $effect(() => {
    if ((form as any)?.success) {
      showCreate = false;
      editTarget = null;
    }
  });

  function fmtCoord(n: number): string { return n.toFixed(4); }
</script>

<AdminPageHeader
  title="Khu vực"
  icon="fa-map-location-dot"
  sub="Quản lý danh sách khu vực trong dropdown tìm kiếm địa lý"
>
  <Button type="button" class="btn-success" onclick={() => (showCreate = true)}>
    <i class="fa-solid fa-plus me-1"></i>Thêm khu vực
  </Button>
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
  <div class="alert alert-warning mx-4 mt-3">{data.error}</div>
{/if}

<div class="admin-card mx-2 mx-md-4 mt-3 mt-md-4">
  {#if data.locations.length === 0}
    <div class="text-center py-5 text-muted">
      <i class="fa-solid fa-map-location-dot fs-1 d-block mb-3 opacity-50"></i>
      <Text class="mb-0 fw-semibold">Chưa có khu vực nào.</Text>
    </div>
  {:else}
    <div class="table-responsive">
      <table class="table table-hover align-middle mb-0">
        <thead class="admin-table-head">
          <tr>
            <th>#</th>
            <th>Tên khu vực</th>
            <th class="text-center">Lat</th>
            <th class="text-center">Lng</th>
            <th class="text-center">Thứ tự</th>
            <th class="text-center">Trạng thái</th>
            <th class="text-center">Hành động</th>
          </tr>
        </thead>
        <tbody>
          {#each data.locations as loc (loc.id)}
            <tr class:table-warning={!loc.is_active}>
              <td class="text-muted small">{loc.id}</td>
              <td class="fw-semibold">{loc.label}</td>
              <td class="text-center font-monospace small">{fmtCoord(loc.lat)}</td>
              <td class="text-center font-monospace small">{fmtCoord(loc.lng)}</td>
              <td class="text-center">
                <span class="badge bg-light text-dark border">#{loc.display_order}</span>
              </td>
              <td class="text-center">
                {#if loc.is_active}
                  <span class="badge bg-success-subtle text-success border border-success-subtle">Hiển thị</span>
                {:else}
                  <span class="badge bg-secondary-subtle text-secondary border border-secondary-subtle">Ẩn</span>
                {/if}
              </td>
              <td class="text-center">
                <div class="d-flex gap-2 justify-content-center">
                  <Button
                    href={ROUTES.ADMIN.LOCATION(loc.id)}
                    class="btn-sm btn-outline-secondary"
                    title="Chi tiết"
                  >
                    <i class="fa-solid fa-eye"></i>
                  </Button>
                  <Button
                    type="button"
                    class="btn-sm btn-outline-primary"
                    onclick={() => openEdit(loc)}
                    title="Sửa"
                  >
                    <i class="fa-solid fa-pen-to-square"></i>
                  </Button>
                  {#if loc.is_active}
                    <!-- Hide: requires confirmation with reason -->
                    <Button
                      type="button"
                      class="btn-sm btn-outline-warning"
                      title="Ẩn khu vực"
                      onclick={() => confirmHide = loc}
                    >
                      <i class="fa-solid fa-eye-slash me-1"></i>Ẩn
                    </Button>
                  {:else}
                    <!-- Show: direct submit with loading indicator -->
                    <form
                      method="POST"
                      action="?/toggle_active"
                      use:enhance={() => {
                        loadingShowId = loc.id;
                        return async ({ update }) => {
                          loadingShowId = null;
                          await update();
                        };
                      }}
                    >
                      <input type="hidden" name="id" value={loc.id} />
                      <input type="hidden" name="is_active" value="true" />
                      <Button
                        type="submit"
                        class="btn-sm btn-outline-success"
                        title="Hiện khu vực"
                        disabled={loadingShowId === loc.id}
                      >
                        {#if loadingShowId === loc.id}
                          <Spinner class="me-1" />
                        {:else}
                          <i class="fa-solid fa-eye me-1"></i>
                        {/if}
                        Hiện
                      </Button>
                    </form>
                  {/if}
                </div>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<!-- ── Create modal ──────────────────────────────────────────────────────────── -->
{#if showCreate}
  <div class="modal-backdrop show" onclick={() => (showCreate = false)} role="presentation"></div>
  <div class="modal d-block" tabindex="-1" role="dialog" aria-modal="true">
    <div class="modal-dialog modal-dialog-centered">
      <div class="modal-content rounded-3">
        <div class="modal-header">
          <Heading level={5} class="modal-title">
            <i class="fa-solid fa-plus text-success me-2"></i>Thêm khu vực
          </Heading>
          <button type="button" class="btn-close" onclick={() => (showCreate = false)}></button>
        </div>
        <form
          method="POST"
          action="?/create"
          use:enhance={() => {
            submittingCreate = true;
            return async ({ update }) => { submittingCreate = false; await update(); };
          }}
        >
          <fieldset class="form-fieldset" disabled={submittingCreate}>
            <div class="modal-body d-flex flex-column gap-3">
              <div>
                <label class="form-label fw-semibold" for="c_label">Tên khu vực <span class="text-danger">*</span></label>
                <input id="c_label" name="label" type="text" class="form-control" placeholder="vd: Long An — Cai Lậy" required />
              </div>
              <div class="row g-3">
                <div class="col-6">
                  <label class="form-label fw-semibold" for="c_lat">Vĩ độ (Lat) <span class="text-danger">*</span></label>
                  <input id="c_lat" name="lat" type="number" step="0.0001" class="form-control font-monospace" placeholder="10.4167" required />
                </div>
                <div class="col-6">
                  <label class="form-label fw-semibold" for="c_lng">Kinh độ (Lng) <span class="text-danger">*</span></label>
                  <input id="c_lng" name="lng" type="number" step="0.0001" class="form-control font-monospace" placeholder="105.9500" required />
                </div>
              </div>
              <div>
                <label class="form-label fw-semibold" for="c_order">Thứ tự hiển thị</label>
                <input id="c_order" name="display_order" type="number" class="form-control" value="99" min="0" />
              </div>
            </div>
            <div class="modal-footer">
              <Button type="button" class="btn-outline-secondary" onclick={() => (showCreate = false)}>Huỷ</Button>
              <Button type="submit" class="btn-success px-4">
                {#if submittingCreate}<Spinner class="me-1" />Đang tạo…{:else}<i class="fa-solid fa-plus me-1"></i>Tạo{/if}
              </Button>
            </div>
          </fieldset>
        </form>
      </div>
    </div>
  </div>
{/if}

<!-- ── Edit modal ────────────────────────────────────────────────────────────── -->
{#if editTarget}
  <div class="modal-backdrop show" onclick={closeEdit} role="presentation"></div>
  <div class="modal d-block" tabindex="-1" role="dialog" aria-modal="true">
    <div class="modal-dialog modal-dialog-centered">
      <div class="modal-content rounded-3">
        <div class="modal-header">
          <Heading level={5} class="modal-title">
            <i class="fa-solid fa-pen-to-square text-primary me-2"></i>Sửa khu vực
          </Heading>
          <button type="button" class="btn-close" onclick={closeEdit}></button>
        </div>
        <form
          method="POST"
          action="?/update"
          use:enhance={() => {
            submittingUpdate = true;
            return async ({ update }) => { submittingUpdate = false; await update(); };
          }}
        >
          <fieldset class="form-fieldset" disabled={submittingUpdate}>
            <input type="hidden" name="id" value={editTarget.id} />
            <div class="modal-body d-flex flex-column gap-3">
              <div>
                <label class="form-label fw-semibold" for="e_label">Tên khu vực <span class="text-danger">*</span></label>
                <input id="e_label" name="label" type="text" class="form-control" value={editTarget.label} required />
              </div>
              <div class="row g-3">
                <div class="col-6">
                  <label class="form-label fw-semibold" for="e_lat">Vĩ độ (Lat)</label>
                  <input id="e_lat" name="lat" type="number" step="0.0001" class="form-control font-monospace" value={editTarget.lat} required />
                </div>
                <div class="col-6">
                  <label class="form-label fw-semibold" for="e_lng">Kinh độ (Lng)</label>
                  <input id="e_lng" name="lng" type="number" step="0.0001" class="form-control font-monospace" value={editTarget.lng} required />
                </div>
              </div>
              <div>
                <label class="form-label fw-semibold" for="e_order">Thứ tự hiển thị</label>
                <input id="e_order" name="display_order" type="number" class="form-control" value={editTarget.display_order} min="0" />
              </div>
            </div>
            <div class="modal-footer">
              <Button type="button" class="btn-outline-secondary" onclick={closeEdit}>Huỷ</Button>
              <Button type="submit" class="btn-primary px-4">
                {#if submittingUpdate}<Spinner class="me-1" />Đang lưu…{:else}<i class="fa-solid fa-floppy-disk me-1"></i>Lưu{/if}
              </Button>
            </div>
          </fieldset>
        </form>
      </div>
    </div>
  </div>
{/if}

<!-- ── Hide confirmation modal ────────────────────────────────────────────────── -->
{#if confirmHide}
  <ConfirmModal
    action="?/toggle_active"
    hiddenFields={{ id: confirmHide.id, is_active: 'false' }}
    title="Ẩn khu vực"
    message="Khu vực này sẽ bị ẩn khỏi dropdown tìm kiếm địa lý trên giao diện người dùng."
    targetName={confirmHide.label}
    icon="fa-solid fa-eye-slash text-warning"
    confirmLabel="Ẩn khu vực"
    confirmClass="btn-outline-warning"
    requireReason={true}
    reasonLabel="Lý do ẩn khu vực"
    reasonPlaceholder="vd: Khu vực không còn phục vụ, thông tin sai..."
    onCancel={() => confirmHide = null}
  />
{/if}
