<script lang="ts">
  import { enhance } from "$app/forms";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import AdminPageHeader from "$lib/components/molecules/AdminPageHeader.svelte";
  import Button from "$lib/components/atoms/Button.svelte";
  import Heading from "$lib/components/atoms/Heading.svelte";
  import Text from "$lib/components/atoms/Text.svelte";
  import type { PageData, ActionData } from "./$types";
  import type { AdminProvider } from "./+page.server";
  import { ROUTES } from "$lib/routes";

  const { data, form }: { data: PageData; form: ActionData } = $props();

  function goToPage(p: number) {
    const params = new URLSearchParams(page.url.searchParams);
    params.set("page", String(p));
    goto(`?${params.toString()}`, { replaceState: false });
  }

  let overrides = $state<Map<number, boolean>>(new Map());
  let assignTarget = $state<AdminProvider | null>(null);
  let showAssignModal = $state(false);
  let showPassword = $state(false);

  function verifiedFor(p: AdminProvider): boolean {
    return overrides.has(p.id)
      ? (overrides.get(p.id) as boolean)
      : p.is_verified;
  }

  function fmtDate(iso: string): string {
    return new Date(iso).toLocaleDateString("vi-VN", {
      day: "2-digit",
      month: "2-digit",
      year: "numeric",
    });
  }

  function openAssignModal(p: AdminProvider) {
    assignTarget = p;
    showPassword = false;
    showAssignModal = true;
  }

  function closeAssignModal() {
    showAssignModal = false;
    assignTarget = null;
  }

  $effect(() => {
    if (form?.success && (form as any).message?.includes("gán tài khoản")) {
      showAssignModal = false;
    }
  });
</script>

<AdminPageHeader
  title="Nhà cung cấp"
  icon="fa-store"
  sub="Quản lý xác minh và trạng thái hoạt động"
>
  <span class="badge bg-secondary rounded-pill fs-6"
    >{data.result.total} nhà cung cấp</span
  >
</AdminPageHeader>

<!-- ── Toast / alert strip ──────────────────────────────────────────────── -->
{#if form?.success && form?.message}
  <div
    class="alert alert-success d-flex align-items-center gap-2 mx-4 mb-0"
    role="alert"
  >
    <i class="fa-solid fa-circle-check flex-shrink-0"></i>
    <span>{form.message}</span>
  </div>
{/if}
{#if form && !form?.success && form?.error}
  <div
    class="alert alert-danger d-flex align-items-center gap-2 mx-4 mb-0"
    role="alert"
  >
    <i class="fa-solid fa-triangle-exclamation flex-shrink-0"></i>
    <span>{form.error}</span>
  </div>
{/if}
{#if data.error}
  <div
    class="alert alert-warning d-flex align-items-center gap-2 mx-4"
    role="alert"
  >
    <i class="fa-solid fa-triangle-exclamation flex-shrink-0"></i>
    <span>{data.error}</span>
  </div>
{/if}

<!-- ── Main card ─────────────────────────────────────────────────────────── -->
<div class="admin-card mx-4 mt-4">
  {#if data.result.items.length === 0 && !data.error}
    <div class="text-center py-5 text-muted">
      <i class="fa-solid fa-inbox fs-1 d-block mb-3 opacity-50"></i>
      <Text class="mb-0 fw-semibold">Chưa có nhà cung cấp nào.</Text>
    </div>
  {:else}
    <div class="table-responsive">
      <table class="table table-hover align-middle mb-0">
        <thead class="admin-table-head">
          <tr>
            <th scope="col" style="width:40px">#</th>
            <th scope="col">Tên / Địa chỉ</th>
            <th scope="col">Số điện thoại</th>
            <th scope="col" class="text-center">Dịch vụ</th>
            <th scope="col" class="text-center">Sản phẩm</th>
            <th scope="col" class="text-center">Trạng thái</th>
            <th scope="col" class="text-center">Ngày đăng ký</th>
            <th scope="col" class="text-center">Tài khoản</th>
            <th scope="col" class="text-center">Xác minh</th>
            <th scope="col" class="text-center">Hành động</th>
          </tr>
        </thead>
        <tbody>
          {#each data.result.items as provider (provider.id)}
            {@const verified = verifiedFor(provider)}
            <tr class:table-warning={!provider.is_active}>
              <!-- ID -->
              <td class="text-muted small">{provider.id}</td>

              <!-- Name + address -->
              <td>
                <div class="d-flex align-items-start gap-2">
                  <span
                    class="provider-avatar"
                    class:provider-avatar--verified={verified}
                  >
                    <i class="fa-solid fa-store"></i>
                  </span>
                  <div>
                    <div class="fw-bold">
                      {provider.name}
                      {#if verified}
                        <i
                          class="fa-solid fa-circle-check text-primary ms-1"
                          title="Đã xác minh"
                          style="font-size:.8rem"
                        ></i>
                      {/if}
                    </div>
                    {#if provider.address}
                      <div class="text-muted small">
                        <i class="fa-solid fa-location-dot me-1"
                        ></i>{provider.address}
                      </div>
                    {/if}
                  </div>
                </div>
              </td>

              <!-- Phone -->
              <td>
                <Button
                  href="tel:{provider.phone}"
                  class="text-decoration-none fw-semibold"
                >
                  {provider.phone}
                </Button>
              </td>

              <!-- Service count -->
              <td class="text-center">
                <span class="badge rounded-pill bg-light text-dark border">
                  {provider.service_count}
                </span>
              </td>

              <!-- Product count -->
              <td class="text-center">
                <span class="badge rounded-pill bg-light text-dark border">
                  {provider.product_count}
                </span>
              </td>

              <!-- Active status -->
              <td class="text-center">
                {#if provider.is_active}
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

              <!-- Registration date -->
              <td class="text-center text-muted small"
                >{fmtDate(provider.created_on)}</td
              >

              <!-- Account link -->
              <td class="text-center">
                {#if provider.linked_username}
                  <span
                    class="badge bg-success-subtle text-success border border-success-subtle account-badge"
                  >
                    <i class="fa-solid fa-user-check me-1"
                    ></i>{provider.linked_username}
                  </span>
                {:else}
                  <Button
                    type="button"
                    class="btn btn-sm btn-outline-primary assign-btn"
                    onclick={() => openAssignModal(provider)}
                  >
                    <i class="fa-solid fa-user-plus me-1"></i>Gán tài khoản
                  </Button>
                {/if}
              </td>

              <!-- Verify toggle -->
              <td class="text-center">
                <form
                  method="POST"
                  action="?/toggle_verify"
                  use:enhance={({ formData }) => {
                    // Optimistic update: flip the badge before the response arrives.
                    const id = Number(formData.get("id"));
                    overrides.set(id, !verified);
                    overrides = new Map(overrides); // trigger reactivity
                    return async ({ update }) => {
                      await update();
                    };
                  }}
                >
                  <input type="hidden" name="id" value={provider.id} />
                  <Button
                    type="submit"
                    class="btn btn-sm {verified
                      ? 'btn-outline-secondary'
                      : 'btn-primary'} verify-btn"
                    title={verified ? "Huỷ xác minh" : "Xác minh nhà cung cấp"}
                  >
                    {#if verified}
                      <i class="fa-solid fa-circle-xmark me-1"></i>Huỷ
                    {:else}
                      <i class="fa-solid fa-circle-check me-1"></i>Xác minh
                    {/if}
                  </Button>
                </form>
              </td>

              <!-- Detail link -->
              <td class="text-center">
                <Button
                  href={ROUTES.ADMIN.PROVIDER(provider.id)}
                  class="btn btn-sm btn-outline-secondary"
                  title="Chi tiết"
                >
                  <i class="fa-solid fa-eye"></i>
                </Button>
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
      class="btn btn-sm btn-outline-secondary"
      onclick={() => goToPage(data.result.page - 1)}
      disabled={data.result.page <= 1}
      ><i class="fa-solid fa-chevron-left"></i></Button
    >
    {#each Array.from({ length: data.result.total_pages }, (_, i) => i + 1) as p}
      <Button
        type="button"
        class="btn btn-sm {p === data.result.page
          ? 'btn-success'
          : 'btn-outline-secondary'}"
        onclick={() => goToPage(p)}>{p}</Button
      >
    {/each}
    <Button
      type="button"
      class="btn btn-sm btn-outline-secondary"
      onclick={() => goToPage(data.result.page + 1)}
      disabled={data.result.page >= data.result.total_pages}
      ><i class="fa-solid fa-chevron-right"></i></Button
    >
  </nav>
{/if}

<!-- ── Assign account modal (Svelte-controlled, no Bootstrap JS needed) ──── -->
{#if showAssignModal && assignTarget}
  <div
    class="modal-backdrop show"
    onclick={closeAssignModal}
    role="presentation"
  ></div>
  <div
    class="modal d-block"
    tabindex="-1"
    role="dialog"
    aria-modal="true"
    aria-labelledby="assignModalTitle"
  >
    <div class="modal-dialog modal-dialog-centered">
      <div class="modal-content rounded-3">
        <div class="modal-header">
          <Heading level={5} class="modal-title" id="assignModalTitle">
            <i class="fa-solid fa-user-plus text-primary me-2"></i>Gán tài khoản
            provider
          </Heading>
          <button
            type="button"
            class="btn-close"
            onclick={closeAssignModal}
            aria-label="Đóng"
          ></button>
        </div>
        <form method="POST" action="?/assign_user" use:enhance>
          <input type="hidden" name="business_id" value={assignTarget.id} />
          <div class="modal-body d-flex flex-column gap-3">
            <div
              class="alert alert-light border d-flex align-items-center gap-2 py-2 mb-0"
            >
              <i class="fa-solid fa-store text-success flex-shrink-0"></i>
              <span class="fw-semibold">{assignTarget.name}</span>
            </div>

            {#if form && !form.success && (form as any).error}
              <div class="alert alert-danger py-2 mb-0">
                <i class="fa-solid fa-triangle-exclamation me-1"></i>{(
                  form as any
                ).error}
              </div>
            {/if}

            <div>
              <label class="form-label fw-semibold" for="assign_username">
                Tên đăng nhập <span class="text-danger">*</span>
              </label>
              <input
                id="assign_username"
                name="username"
                type="text"
                class="form-control"
                value={assignTarget.phone}
                pattern="[a-zA-Z0-9_.@-]+"
                title="Chỉ dùng chữ cái, số và ký tự . _ @ - (không có khoảng trắng)"
                autocomplete="off"
                required
              />
              <div class="form-text">
                Mặc định là số điện thoại. Nhà cung cấp sẽ dùng tên này để đăng
                nhập.
              </div>
            </div>

            <div>
              <label class="form-label fw-semibold" for="assign_password">
                Mật khẩu <span class="text-danger">*</span>
              </label>
              <div class="input-group">
                <input
                  id="assign_password"
                  name="password"
                  type={showPassword ? "text" : "password"}
                  class="form-control"
                  placeholder="Tối thiểu 8 ký tự"
                  autocomplete="new-password"
                  required
                  minlength="8"
                />
                <Button
                  type="button"
                  class="btn btn-outline-secondary pw-toggle"
                  onclick={() => (showPassword = !showPassword)}
                  title={showPassword ? "Ẩn mật khẩu" : "Hiện mật khẩu"}
                >
                  <i class="fa-solid {showPassword ? 'fa-eye-slash' : 'fa-eye'}"
                  ></i>
                </Button>
              </div>
            </div>

            <div
              class="alert alert-warning py-2 mb-0 d-flex align-items-start gap-2"
            >
              <i class="fa-solid fa-triangle-exclamation flex-shrink-0 mt-1"
              ></i>
              <span class="small"
                >Ghi lại tên đăng nhập và mật khẩu để cung cấp cho nhà cung cấp.
                Họ sẽ đăng nhập tại trang <strong>/login</strong>.</span
              >
            </div>
          </div>
          <div class="modal-footer">
            <Button
              type="button"
              class="btn btn-outline-secondary"
              onclick={closeAssignModal}>Huỷ</Button
            >
            <Button type="submit" class="btn btn-primary px-4">
              <i class="fa-solid fa-user-plus me-1"></i>Gán tài khoản
            </Button>
          </div>
        </form>
      </div>
    </div>
  </div>
{/if}

<style>
  .provider-avatar {
    width: 36px;
    height: 36px;
    flex-shrink: 0;
    background: #f0f4f2;
    border-radius: 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.9rem;
    color: #5e7a68;
  }
  .provider-avatar--verified {
    background: #ecfdf5;
    color: #047857;
  }
  :global(.verify-btn) {
    min-width: 96px;
    font-size: 0.8rem;
    font-weight: 700;
    border-radius: 8px;
    white-space: nowrap;
  }
  :global(.assign-btn) {
    font-size: 0.8rem;
    font-weight: 700;
    border-radius: 8px;
    white-space: nowrap;
  }
  .account-badge {
    font-size: 0.75rem;
    white-space: nowrap;
  }
</style>
