<script lang="ts">
  import { enhance } from "$app/forms";
  import Button from "$lib/components/atoms/Button.svelte";
  import Heading from "$lib/components/atoms/Heading.svelte";
  import Spinner from "$lib/components/atoms/Spinner.svelte";
  import ConfirmModal from "$lib/components/molecules/ConfirmModal.svelte";
  import type { PageData, ActionData } from "./$types";
  import { ROUTES } from "$lib/routes";

  const { data, form }: { data: PageData; form: ActionData } = $props();

  const loc = $derived(data.location);

  let showEdit      = $state(false);
  let showConfirm   = $state(false);
  let confirmHide   = $state(false);
  let submitting    = $state(false);
  let loadingShow   = $state(false);

  let editLabel = $state(data.location.label);
  let editLat   = $state(data.location.lat);
  let editLng   = $state(data.location.lng);
  let editOrder = $state(data.location.display_order);

  $effect(() => {
    if ((form as any)?.success) {
      showEdit = false;
      confirmHide = false;
    }
  });
</script>

<!-- ── Breadcrumb ──────────────────────────────────────────────────────────── -->
<nav aria-label="breadcrumb" class="mx-4 mt-4">
  <ol class="breadcrumb mb-0">
    <li class="breadcrumb-item">
      <a href={ROUTES.ADMIN.LOCATIONS} class="text-success fw-semibold text-decoration-none">
        <i class="fa-solid fa-map-location-dot me-1"></i>Khu vực
      </a>
    </li>
    <li class="breadcrumb-item active" aria-current="page">{loc.label}</li>
  </ol>
</nav>

<!-- ── Alerts ─────────────────────────────────────────────────────────────── -->
{#if (form as any)?.success && (form as any)?.message}
  <div class="alert alert-success d-flex align-items-center gap-2 mx-4 mt-3 mb-0">
    <i class="fa-solid fa-circle-check flex-shrink-0"></i>{(form as any).message}
  </div>
{/if}
{#if (form as any) && !(form as any)?.success && (form as any)?.error}
  <div class="alert alert-danger d-flex align-items-center gap-2 mx-4 mt-3 mb-0">
    <i class="fa-solid fa-triangle-exclamation flex-shrink-0"></i>{(form as any).error}
  </div>
{/if}

<!-- ── Detail card ────────────────────────────────────────────────────────── -->
<div class="admin-card mx-4 mt-3 p-4 mb-4">
  <div class="d-flex align-items-start justify-content-between gap-3 flex-wrap">
    <div class="d-flex align-items-start gap-3">
      <span class="loc-avatar">
        <i class="fa-solid fa-map-pin fs-4"></i>
      </span>
      <div>
        <Heading level={2} class="detail-name mb-1">{loc.label}</Heading>
        <div class="d-flex flex-wrap gap-2 align-items-center">
          <span class="badge {loc.is_active
            ? 'bg-success-subtle text-success border border-success-subtle'
            : 'bg-secondary-subtle text-secondary border border-secondary-subtle'}">
            {loc.is_active ? 'Hiển thị' : 'Ẩn'}
          </span>
          <span class="badge bg-light text-dark border">#{loc.display_order}</span>
        </div>
      </div>
    </div>
    <div class="d-flex gap-2 flex-wrap">
      <Button type="button" class="btn-outline-primary" onclick={() => (showEdit = true)}>
        <i class="fa-solid fa-pen-to-square me-1"></i>Sửa
      </Button>
      {#if loc.is_active}
        <!-- Hide: requires confirmation with reason -->
        <Button
          type="button"
          class="btn btn-outline-warning"
          onclick={() => (confirmHide = true)}
        >
          <i class="fa-solid fa-eye-slash me-1"></i>Ẩn
        </Button>
      {:else}
        <!-- Show: direct submit with loading indicator -->
        <form
          method="POST"
          action="?/toggle_active"
          use:enhance={() => {
            loadingShow = true;
            return async ({ update }) => {
              loadingShow = false;
              await update();
            };
          }}
        >
          <input type="hidden" name="is_active" value="true" />
          <Button
            type="submit"
            class="btn btn-outline-success"
            disabled={loadingShow}
          >
            {#if loadingShow}
              <Spinner class="me-1" />Đang xử lý…
            {:else}
              <i class="fa-solid fa-eye me-1"></i>Hiện
            {/if}
          </Button>
        </form>
      {/if}
    </div>
  </div>

  <hr class="my-3" />

  <div class="row g-3">
    <div class="col-sm-6 col-md-3">
      <div class="detail-field">
        <span class="detail-label"><i class="fa-solid fa-hashtag me-1"></i>ID</span>
        <span class="detail-value text-muted">{loc.id}</span>
      </div>
    </div>
    <div class="col-sm-6 col-md-3">
      <div class="detail-field">
        <span class="detail-label"><i class="fa-solid fa-location-crosshairs me-1"></i>Vĩ độ (Lat)</span>
        <code class="detail-value small">{loc.lat}</code>
      </div>
    </div>
    <div class="col-sm-6 col-md-3">
      <div class="detail-field">
        <span class="detail-label"><i class="fa-solid fa-location-crosshairs me-1"></i>Kinh độ (Lng)</span>
        <code class="detail-value small">{loc.lng}</code>
      </div>
    </div>
    <div class="col-sm-6 col-md-3">
      <div class="detail-field">
        <span class="detail-label"><i class="fa-solid fa-sort me-1"></i>Thứ tự hiển thị</span>
        <span class="detail-value">{loc.display_order}</span>
      </div>
    </div>
  </div>

  <hr class="my-3" />

  <!-- Impact notice inline -->
  <div class="d-flex gap-2 p-3 rounded-3 bg-info-subtle text-info-emphasis small">
    <i class="fa-solid fa-circle-info flex-shrink-0 mt-1"></i>
    <span>
      Khu vực này chỉ xuất hiện trong dropdown chọn địa điểm trên trang Agri-Connect.
      Không có dữ liệu nhà cung cấp, sản phẩm hay yêu cầu nào liên kết với khu vực —
      thay đổi hoặc xoá sẽ không ảnh hưởng đến dữ liệu khác.
      Dùng <strong>Ẩn</strong> thay vì xoá nếu muốn giữ lại lịch sử.
    </span>
  </div>

  <hr class="my-3" />

  <!-- Danger zone inline -->
  <div class="d-flex align-items-center justify-content-between gap-3 p-3 rounded-3 border border-danger bg-danger-subtle">
    <div class="small text-danger-emphasis">
      <i class="fa-solid fa-triangle-exclamation me-1"></i>
      Xoá vĩnh viễn khu vực <strong>"{loc.label}"</strong>. Hành động không thể hoàn tác.
    </div>
    <Button type="button" class="btn-danger btn-sm flex-shrink-0" onclick={() => (showConfirm = true)}>
      <i class="fa-solid fa-trash me-1"></i>Xoá khu vực
    </Button>
  </div>
</div>

<!-- ── Edit modal ────────────────────────────────────────────────────────── -->
{#if showEdit}
  <div class="modal-backdrop show" onclick={() => (showEdit = false)} role="presentation"></div>
  <div class="modal d-block" tabindex="-1" role="dialog" aria-modal="true">
    <div class="modal-dialog modal-dialog-centered">
      <div class="modal-content rounded-3">
        <div class="modal-header">
          <Heading level={5} class="modal-title">
            <i class="fa-solid fa-pen-to-square text-primary me-2"></i>Sửa khu vực
          </Heading>
          <button type="button" class="btn-close" onclick={() => (showEdit = false)}></button>
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
            <div class="modal-body d-flex flex-column gap-3">
              <div>
                <label class="form-label fw-semibold" for="e_label">Tên khu vực <span class="text-danger">*</span></label>
                <input id="e_label" name="label" type="text" class="form-control" bind:value={editLabel} required />
              </div>
              <div class="row g-3">
                <div class="col-6">
                  <label class="form-label fw-semibold" for="e_lat">Vĩ độ (Lat)</label>
                  <input id="e_lat" name="lat" type="number" step="0.0001" class="form-control font-monospace" bind:value={editLat} required />
                </div>
                <div class="col-6">
                  <label class="form-label fw-semibold" for="e_lng">Kinh độ (Lng)</label>
                  <input id="e_lng" name="lng" type="number" step="0.0001" class="form-control font-monospace" bind:value={editLng} required />
                </div>
              </div>
              <div>
                <label class="form-label fw-semibold" for="e_order">Thứ tự hiển thị</label>
                <input id="e_order" name="display_order" type="number" class="form-control" bind:value={editOrder} min="0" />
              </div>
            </div>
            <div class="modal-footer">
              <Button type="button" class="btn-outline-secondary" onclick={() => (showEdit = false)}>Huỷ</Button>
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

<!-- ── Hide confirmation modal ──────────────────────────────────────────── -->
{#if confirmHide}
  <ConfirmModal
    action="?/toggle_active"
    hiddenFields={{ is_active: 'false' }}
    title="Ẩn khu vực"
    message="Khu vực này sẽ bị ẩn khỏi dropdown tìm kiếm địa lý. Dùng tùy chọn này thay vì xoá nếu muốn giữ lại lịch sử."
    targetName={loc.label}
    icon="fa-solid fa-eye-slash text-warning"
    confirmLabel="Ẩn khu vực"
    confirmClass="btn-outline-warning"
    requireReason={true}
    reasonLabel="Lý do ẩn khu vực"
    reasonPlaceholder="vd: Khu vực không còn phục vụ, thông tin sai..."
    onCancel={() => (confirmHide = false)}
  />
{/if}

<!-- ── Delete confirm modal ──────────────────────────────────────────────── -->
{#if showConfirm}
  <ConfirmModal
    action="?/delete"
    hiddenFields={{}}
    title="Xoá vĩnh viễn khu vực"
    message="Khu vực này sẽ bị xoá vĩnh viễn. Hành động này không thể hoàn tác."
    targetName={loc.label}
    icon="fa-solid fa-triangle-exclamation text-danger"
    confirmLabel="Xoá khu vực"
    confirmClass="btn-outline-danger"
    requireReason={true}
    reasonLabel="Lý do xoá"
    reasonPlaceholder="vd: Khu vực trùng lặp, nhập sai thông tin..."
    onCancel={() => (showConfirm = false)}
  />
{/if}

<style>
  .loc-avatar {
    width: 56px;
    height: 56px;
    flex-shrink: 0;
    border-radius: 14px;
    background: #d1fae5;
    color: #065f46;
    display: flex;
    align-items: center;
    justify-content: center;
  }
</style>
