<script lang="ts">
  import { enhance } from "$app/forms";
  import Button from "$lib/components/atoms/Button.svelte";
  import Heading from "$lib/components/atoms/Heading.svelte";
  import Spinner from "$lib/components/atoms/Spinner.svelte";
  import ConfirmModal from "$lib/components/molecules/ConfirmModal.svelte";
  import type { PageData, ActionData } from "./$types";
  import { ROUTES } from "$lib/routes";

  const { data, form }: { data: PageData; form: ActionData } = $props();

  const p = $derived(data.provider);
  let isVerified = $state(data.provider.is_verified);
  let confirmUnverify = $state(false);
  let loadingVerify = $state(false);

  $effect(() => {
    if ((form as any)?.success) {
      confirmUnverify = false;
      if ((form as any)?.is_verified !== undefined) {
        isVerified = (form as any).is_verified;
      }
    }
  });

  function fmtDate(iso: string): string {
    return new Date(iso).toLocaleDateString("vi-VN", {
      day: "2-digit",
      month: "2-digit",
      year: "numeric",
    });
  }

  function fmtPrice(price: number | null, unit: string | null): string {
    if (price == null) return "—";
    return (
      new Intl.NumberFormat("vi-VN").format(price) +
      " ₫" +
      (unit ? `/${unit}` : "")
    );
  }
</script>

<!-- ── Breadcrumb ──────────────────────────────────────────────────────────── -->
<nav aria-label="breadcrumb" class="mx-4 mt-4">
  <ol class="breadcrumb mb-0">
    <li class="breadcrumb-item">
      <a href={ROUTES.ADMIN.EXPLORE} class="text-success fw-semibold text-decoration-none">
        <i class="fa-solid fa-store me-1"></i>Nhà cung cấp
      </a>
    </li>
    <li class="breadcrumb-item active" aria-current="page">{p.name}</li>
  </ol>
</nav>

<!-- ── Alert strip ─────────────────────────────────────────────────────────── -->
{#if (form as any)?.success && (form as any)?.message}
  <div class="alert alert-success d-flex align-items-center gap-2 mx-4 mt-2 mb-0">
    <i class="fa-solid fa-circle-check flex-shrink-0"></i>{(form as any)
      .message}
  </div>
{/if}
{#if (form as any) && !(form as any)?.success && (form as any)?.error}
  <div class="alert alert-danger d-flex align-items-center gap-2 mx-4 mt-2 mb-0">
    <i class="fa-solid fa-triangle-exclamation flex-shrink-0"></i>{(form as any)
      .error}
  </div>
{/if}

<!-- ── Summary card ────────────────────────────────────────────────────────── -->
<div class="admin-card mx-4 mt-3 p-4">
  <div class="d-flex align-items-start justify-content-between gap-3 flex-wrap">
    <div class="d-flex align-items-start gap-3">
      <span
        class="provider-avatar"
        class:provider-avatar--verified={isVerified}
      >
        <i class="fa-solid fa-store fs-4"></i>
      </span>
      <div>
        <Heading level={2} class="detail-name mb-1">
          {p.name}
          {#if isVerified}
            <i
              class="fa-solid fa-circle-check text-primary ms-1"
              title="Đã xác minh"
              style="font-size:1rem"
            ></i>
          {/if}
        </Heading>
        <div class="d-flex flex-wrap gap-2 align-items-center">
          {#if p.is_active}
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
          {#if isVerified}
            <span
              class="badge bg-primary-subtle text-primary border border-primary-subtle"
              >Đã xác minh</span
            >
          {:else}
            <span
              class="badge bg-warning-subtle text-warning border border-warning-subtle"
              >Chưa xác minh</span
            >
          {/if}
        </div>
      </div>
    </div>

    <div class="d-flex gap-2 flex-wrap">
      {#if isVerified}
        <!-- Un-verify: requires confirmation with reason -->
        <Button
          type="button"
          class="btn btn-outline-secondary"
          onclick={() => (confirmUnverify = true)}
        >
          <i class="fa-solid fa-circle-xmark me-1"></i>Huỷ xác minh
        </Button>
      {:else}
        <!-- Verify: direct submit with loading indicator -->
        <form
          method="POST"
          action="?/toggle_verify"
          use:enhance={() => {
            loadingVerify = true;
            return async ({ update }) => {
              loadingVerify = false;
              await update();
            };
          }}
        >
          <Button
            type="submit"
            class="btn btn-primary"
            disabled={loadingVerify}
          >
            {#if loadingVerify}
              <Spinner class="me-1" />Đang xử lý…
            {:else}
              <i class="fa-solid fa-circle-check me-1"></i>Xác minh
            {/if}
          </Button>
        </form>
      {/if}
    </div>
  </div>

  <hr class="my-3" />

  <div class="row g-3">
    <div class="col-sm-6 col-md-4">
      <div class="detail-field">
        <span class="detail-label"
          ><i class="fa-solid fa-phone me-1"></i>Số điện thoại</span
        >
        <a
          href="tel:{p.phone}"
          class="detail-value fw-bold text-decoration-none">{p.phone}</a
        >
      </div>
    </div>
    {#if p.email}
      <div class="col-sm-6 col-md-4">
        <div class="detail-field">
          <span class="detail-label"
            ><i class="fa-solid fa-envelope me-1"></i>Email</span
          >
          <span class="detail-value">{p.email}</span>
        </div>
      </div>
    {/if}
    {#if p.address}
      <div class="col-sm-6 col-md-4">
        <div class="detail-field">
          <span class="detail-label"
            ><i class="fa-solid fa-location-dot me-1"></i>Địa chỉ</span
          >
          <span class="detail-value">{p.address}</span>
        </div>
      </div>
    {/if}
    <div class="col-sm-6 col-md-4">
      <div class="detail-field">
        <span class="detail-label"
          ><i class="fa-solid fa-calendar me-1"></i>Ngày đăng ký</span
        >
        <span class="detail-value">{fmtDate(p.created_on)}</span>
      </div>
    </div>
    <div class="col-sm-6 col-md-4">
      <div class="detail-field">
        <span class="detail-label"
          ><i class="fa-solid fa-user me-1"></i>Tài khoản</span
        >
        {#if p.linked_username}
          <span class="detail-value fw-semibold text-success"
            >{p.linked_username}</span
          >
        {:else}
          <span class="detail-value text-muted">Chưa gán</span>
        {/if}
      </div>
    </div>
    {#if p.description}
      <div class="col-12">
        <div class="detail-field">
          <span class="detail-label"
            ><i class="fa-solid fa-align-left me-1"></i>Mô tả</span
          >
          <span class="detail-value">{p.description}</span>
        </div>
      </div>
    {/if}
  </div>
</div>

<!-- ── Services ────────────────────────────────────────────────────────────── -->
<div class="admin-card mx-4 mt-3">
  <div class="px-3 py-3 border-bottom">
    <Heading level={3} class="fs-6 fw-bold d-flex align-items-center gap-2 mb-0">
      <i class="fa-solid fa-wrench text-info"></i>Dịch vụ
      <span class="badge bg-secondary rounded-pill">{p.services.length}</span>
    </Heading>
  </div>
  {#if p.services.length === 0}
    <div class="text-center py-4 text-muted">
      <i class="fa-solid fa-inbox d-block mb-2 opacity-50"></i>
      <span class="small">Chưa có dịch vụ nào.</span>
    </div>
  {:else}
    <div class="table-responsive">
      <table class="table table-hover align-middle mb-0">
        <thead class="admin-table-head">
          <tr>
            <th>#</th>
            <th>Tên dịch vụ</th>
            <th>Danh mục</th>
            <th>Giá / ghi chú</th>
            <th class="text-center">Trạng thái</th>
          </tr>
        </thead>
        <tbody>
          {#each p.services as svc (svc.id)}
            <tr class:table-warning={!svc.is_active}>
              <td class="text-muted small">{svc.id}</td>
              <td class="fw-semibold">{svc.name}</td>
              <td
                ><span
                  class="badge bg-info-subtle text-info border border-info-subtle"
                  >{svc.category}</span
                ></td
              >
              <td class="text-muted small">{svc.price_note ?? "—"}</td>
              <td class="text-center">
                {#if svc.is_active}
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
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<!-- ── Products ────────────────────────────────────────────────────────────── -->
<div class="admin-card mx-4 mt-3">
  <div class="px-3 py-3 border-bottom">
    <Heading level={3} class="fs-6 fw-bold d-flex align-items-center gap-2 mb-0">
      <i class="fa-solid fa-box text-warning"></i>Sản phẩm
      <span class="badge bg-secondary rounded-pill">{p.products.length}</span>
    </Heading>
  </div>
  {#if p.products.length === 0}
    <div class="text-center py-4 text-muted">
      <i class="fa-solid fa-inbox d-block mb-2 opacity-50"></i>
      <span class="small">Chưa có sản phẩm nào.</span>
    </div>
  {:else}
    <div class="table-responsive">
      <table class="table table-hover align-middle mb-0">
        <thead class="admin-table-head">
          <tr>
            <th>#</th>
            <th>Tên sản phẩm</th>
            <th>Danh mục</th>
            <th>Giá</th>
            <th class="text-center">Trạng thái</th>
          </tr>
        </thead>
        <tbody>
          {#each p.products as prod (prod.id)}
            <tr class:table-warning={!prod.is_active}>
              <td class="text-muted small">{prod.id}</td>
              <td class="fw-semibold">{prod.name}</td>
              <td
                ><span
                  class="badge bg-warning-subtle text-warning border border-warning-subtle"
                  >{prod.category}</span
                ></td
              >
              <td class="text-muted small">{fmtPrice(prod.price, prod.unit)}</td
              >
              <td class="text-center">
                {#if prod.is_active}
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
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<!-- ── Un-verify confirmation modal ──────────────────────────────────────── -->
{#if confirmUnverify}
  <ConfirmModal
    action="?/toggle_verify"
    hiddenFields={{}}
    title="Huỷ xác minh nhà cung cấp"
    message="Huy hiệu xác minh sẽ bị gỡ. Nhà cung cấp có thể yêu cầu xác minh lại sau."
    targetName={p.name}
    icon="fa-solid fa-circle-xmark text-danger"
    confirmLabel="Huỷ xác minh"
    confirmClass="btn-outline-danger"
    requireReason={true}
    reasonLabel="Lý do huỷ xác minh"
    reasonPlaceholder="vd: Thông tin không còn chính xác, phát sinh khiếu nại..."
    onCancel={() => (confirmUnverify = false)}
  />
{/if}

<style>
  .provider-avatar {
    width: 56px;
    height: 56px;
    flex-shrink: 0;
    background: #f0f4f2;
    border-radius: 14px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #5e7a68;
  }
  .provider-avatar--verified {
    background: #ecfdf5;
    color: #047857;
  }
</style>
