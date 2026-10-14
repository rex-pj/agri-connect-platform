<script lang="ts">
  import { enhance } from "$app/forms";
  import { PRODUCT_NAME_ICON } from "$lib/categoryIcons";
  import Spinner from "$lib/components/atoms/Spinner.svelte";
  import Heading from "$lib/components/atoms/Heading.svelte";
  import Text from "$lib/components/atoms/Text.svelte";
  import Button from "$lib/components/atoms/Button.svelte";
  import { ROUTES } from "$lib/routes";

  let submittingProfile = $state(false);
  let submittingAdd = $state(false);
  let submittingEdit = $state(false);
  import type { PageData, ActionData } from "./$types";

  const { data, form }: { data: PageData; form: ActionData } = $props();

  // ── Optimistic product list ───────────────────────────────────────────────
  // Mirror the server list locally so deletes feel instant.
  let products = $state([...data.products]);
  let pendingDelete = $state<Set<number>>(new Set());

  $effect(() => {
    products = [...data.products];
  });

  // ── GPS detection for profile form ────────────────────────────────────────
  let detecting = $state(false);
  let gpsError = $state<string | null>(null);
  let lat = $state<string>(data.profile?.lat?.toString() ?? "");
  let lng = $state<string>(data.profile?.lng?.toString() ?? "");

  function detectGps() {
    if (!navigator.geolocation) {
      gpsError = "Trình duyệt không hỗ trợ định vị.";
      return;
    }
    detecting = true;
    gpsError = null;
    navigator.geolocation.getCurrentPosition(
      ({ coords }) => {
        lat = coords.latitude.toFixed(6);
        lng = coords.longitude.toFixed(6);
        detecting = false;
      },
      () => {
        gpsError = "Không lấy được vị trí. Hãy nhập thủ công.";
        detecting = false;
      },
      { timeout: 10_000, enableHighAccuracy: true },
    );
  }

  // ── Bootstrap modal control ───────────────────────────────────────────────
  let editingProduct = $state<(typeof data.products)[number] | null>(null);

  function openEditModal(product: (typeof data.products)[number]) {
    editingProduct = product;
    const el = document.getElementById("editProductModal");
    if (el) {
      // @ts-ignore
      window.bootstrap?.Modal.getOrCreateInstance(el).show();
    }
  }

  function closeEditModal() {
    const el = document.getElementById("editProductModal");
    if (el) {
      // @ts-ignore
      window.bootstrap?.Modal.getInstance(el)?.hide();
    }
  }

  $effect(() => {
    if (form?.action === "add_product" && form?.success) {
      const el = document.getElementById("addProductModal");
      if (el) {
        // @ts-ignore
        const modal = window.bootstrap?.Modal.getInstance(el);
        modal?.hide();
      }
    }
    if (form?.action === "edit_product" && form?.success) {
      closeEditModal();
    }
  });

  function fmtPrice(p: number): string {
    return p.toLocaleString("vi-VN") + " ₫";
  }

  function productIcon(cat: string): string {
    return PRODUCT_NAME_ICON[cat] ?? "fa-tag";
  }
</script>

<svelte:head>
  <title>Trang cá nhân — Agri-Connect</title>
  <meta name="robots" content="noindex, nofollow" />
</svelte:head>

<!-- ══════════════════════════════════════════════════════════════════════════
     PAGE HEADER
══════════════════════════════════════════════════════════════════════════ -->
<div class="dash-header">
  <div>
    <Heading level={1} class="dash-title">
      <i class="fa-solid fa-gauge me-2 text-success"></i>Trang cá nhân
    </Heading>
    <Text class="dash-sub">Quản lý hồ sơ và sản phẩm của bạn</Text>
  </div>
  {#if data.profile?.is_verified}
    <span class="badge bg-primary d-flex align-items-center gap-1 px-3 py-2">
      <i class="fa-solid fa-circle-check"></i> Đã xác minh
    </span>
  {/if}
</div>

<!-- ══════════════════════════════════════════════════════════════════════════
     GLOBAL ALERTS
══════════════════════════════════════════════════════════════════════════ -->
{#if data.profileError}
  <div class="alert alert-warning mx-3 mt-2 mb-0">
    <i class="fa-solid fa-triangle-exclamation me-2"></i>{data.profileError}
  </div>
{/if}

{#if form?.success && form.action === "update_profile"}
  <div
    class="alert alert-success mx-3 mt-3 mb-0 d-flex align-items-center gap-2"
  >
    <i class="fa-solid fa-circle-check"></i> Hồ sơ đã được cập nhật thành công.
  </div>
{/if}
{#if form?.success && form.action === "edit_product"}
  <div
    class="alert alert-success mx-3 mt-3 mb-0 d-flex align-items-center gap-2"
  >
    <i class="fa-solid fa-circle-check"></i> Sản phẩm đã được cập nhật.
  </div>
{/if}
{#if form?.success && form.action === "delete_product"}
  <div
    class="alert alert-success mx-3 mt-3 mb-0 d-flex align-items-center gap-2"
  >
    <i class="fa-solid fa-circle-check"></i> Sản phẩm đã được xoá.
  </div>
{/if}
{#if !form?.success && form?.error}
  <div
    class="alert alert-danger mx-3 mt-3 mb-0 d-flex align-items-center gap-2"
  >
    <i class="fa-solid fa-triangle-exclamation"></i>
    {form.error}
  </div>
{/if}

<div class="px-3 pt-4 pb-5 d-flex flex-column gap-4">
  <!-- ══════════════════════════════════════════════════════════════════════════
     PROFILE CARD
══════════════════════════════════════════════════════════════════════════ -->
  {#if !data.profile && !data.profileError}
    <div class="dash-card text-center py-5">
      <i class="fa-solid fa-store fs-1 text-muted opacity-50 d-block mb-3"></i>
      <Text class="fw-bold mb-1"
        >Tài khoản chưa được liên kết với doanh nghiệp.</Text
      >
      <Text class="text-muted small"
        >Liên hệ quản trị viên để được kích hoạt.</Text
      >
    </div>
  {:else if data.profile}
    <div class="dash-card">
      <div class="dash-card__head">
        <Heading level={2} class="dash-card__title">
          <i class="fa-solid fa-user-tie me-2"></i>Hồ sơ doanh nghiệp
        </Heading>
      </div>

      <form
        method="POST"
        action="?/update_profile"
        class="dash-card__body"
        use:enhance={() => {
          submittingProfile = true;
          return async ({ update }) => {
            submittingProfile = false;
            await update();
          };
        }}
      >
        <fieldset class="form-fieldset" disabled={submittingProfile}>
          <div class="row g-3">
            <!-- Name -->
            <div class="col-12 col-md-6">
              <label class="form-label fw-semibold" for="pf_name"
                >Tên doanh nghiệp <span class="text-danger">*</span></label
              >
              <input
                id="pf_name"
                name="name"
                type="text"
                class="form-control"
                value={data.profile.name}
                required
              />
            </div>

            <!-- Phone -->
            <div class="col-12 col-md-6">
              <label class="form-label fw-semibold" for="pf_phone"
                >Số điện thoại <span class="text-danger">*</span></label
              >
              <input
                id="pf_phone"
                name="phone"
                type="tel"
                class="form-control"
                value={data.profile.phone}
                pattern={"0\\d{9}"}
                autocomplete="tel"
                required
              />
            </div>

            <!-- Address -->
            <div class="col-12">
              <label class="form-label fw-semibold" for="pf_address"
                >Địa chỉ</label
              >
              <input
                id="pf_address"
                name="address"
                type="text"
                class="form-control"
                value={data.profile.address ?? ""}
                placeholder="Số nhà, ấp, xã, huyện, tỉnh"
              />
            </div>

            <!-- Description -->
            <div class="col-12">
              <label class="form-label fw-semibold" for="pf_desc"
                >Giới thiệu</label
              >
              <textarea
                id="pf_desc"
                name="description"
                class="form-control"
                rows="3"
                placeholder="Mô tả ngắn về dịch vụ, kinh nghiệm, khu vực phục vụ…"
                >{data.profile.description ?? ""}</textarea
              >
            </div>

            <!-- GPS -->
            <div class="col-12">
              <label
                class="form-label fw-semibold d-flex align-items-center gap-2"
              >
                <i class="fa-solid fa-location-dot text-success"></i> Vị trí GPS
                <span class="text-muted small fw-normal"
                  >(để trống nếu không thay đổi)</span
                >
              </label>
              <div class="row g-2 align-items-end">
                <div class="col">
                  <input
                    name="lat"
                    type="number"
                    step="any"
                    class="form-control"
                    placeholder="Vĩ độ (lat)"
                    bind:value={lat}
                  />
                </div>
                <div class="col">
                  <input
                    name="lng"
                    type="number"
                    step="any"
                    class="form-control"
                    placeholder="Kinh độ (lng)"
                    bind:value={lng}
                  />
                </div>
                <div class="col-auto">
                  <Button
                    type="button"
                    class="btn-outline-success"
                    onclick={detectGps}
                    disabled={detecting}
                  >
                    {#if detecting}
                      <Spinner class="me-1" />
                    {:else}
                      <i class="fa-solid fa-crosshairs me-1"></i>
                    {/if}
                    Xác định
                  </Button>
                </div>
              </div>
              {#if gpsError}
                <Text class="text-danger small mt-1 mb-0">{gpsError}</Text>
              {/if}
            </div>
          </div>

          <div class="mt-4 d-flex justify-content-end">
            <Button type="submit" class="btn-success px-4">
              {#if submittingProfile}
                <Spinner class="me-2" />Đang lưu…
              {:else}
                <i class="fa-solid fa-floppy-disk me-2"></i>Lưu hồ sơ
              {/if}
            </Button>
          </div>
        </fieldset>
      </form>
    </div>
  {/if}

  <!-- ══════════════════════════════════════════════════════════════════════════
     PRODUCTS SECTION
══════════════════════════════════════════════════════════════════════════ -->
  <div class="dash-card">
    <div class="dash-card__head">
      <Heading level={2} class="dash-card__title">
        <i class="fa-solid fa-box me-2"></i>Sản phẩm của tôi
        <span class="badge bg-secondary rounded-pill ms-2 fs-6"
          >{products.length}</span
        >
      </Heading>

      <!-- Add product button — triggers Bootstrap modal -->
      <Button
        type="button"
        class="btn-sm btn-success"
        data-bs-toggle="modal"
        data-bs-target="#addProductModal"
        disabled={!data.profile}
        title={!data.profile ? "Cần có hồ sơ doanh nghiệp trước" : ""}
      >
        <i class="fa-solid fa-plus me-1"></i>Thêm sản phẩm
      </Button>
    </div>

    <!-- Product grid -->
    {#if products.length === 0}
      <div class="dash-card__body text-center py-5 text-muted">
        <i class="fa-solid fa-box-open fs-1 d-block mb-3 opacity-40"></i>
        <Text class="mb-1 fw-semibold">Chưa có sản phẩm nào.</Text>
        <Text class="small">Nhấn "Thêm sản phẩm" để bắt đầu.</Text>
      </div>
    {:else}
      <div class="dash-card__body">
        <div class="row row-cols-1 row-cols-md-2 row-cols-lg-3 g-3">
          {#each products as product (product.id)}
            <div class="col" class:opacity-50={pendingDelete.has(product.id)}>
              <div class="product-tile">
                <!-- Thumbnail or category icon -->
                {#if product.image_url}
                  <img
                    src={product.image_url}
                    alt={product.name}
                    class="product-tile__thumb"
                    loading="lazy"
                  />
                {:else}
                  <div class="product-tile__icon">
                    <i class="fa-solid {productIcon(product.category)}"></i>
                  </div>
                {/if}

                <div class="product-tile__body">
                  <Text class="product-tile__name">{product.name}</Text>
                  <span
                    class="badge bg-light text-secondary border product-tile__cat"
                  >
                    {product.category}
                  </span>
                  {#if product.price != null}
                    <Text class="product-tile__price"
                      >{fmtPrice(product.price)}
                      {#if product.unit}<span class="product-tile__unit"
                          >/{product.unit}</span
                        >{/if}
                    </Text>
                  {:else}
                    <Text class="product-tile__price-contact">Liên hệ giá</Text>
                  {/if}
                  {#if product.description}
                    <Text class="product-tile__desc">{product.description}</Text
                    >
                  {/if}
                </div>

                <!-- Edit + Delete actions -->
                <div class="product-tile__actions d-flex flex-column gap-1">
                  <Button
                    type="button"
                    class="btn-sm btn-outline-secondary"
                    onclick={() => openEditModal(product)}
                    title="Chỉnh sửa sản phẩm"
                  >
                    <i class="fa-solid fa-pen"></i>
                  </Button>

                  <form
                    method="POST"
                    action="?/delete_product"
                    use:enhance={() => {
                      pendingDelete.add(product.id);
                      pendingDelete = new Set(pendingDelete);
                      return async ({ update }) => {
                        await update();
                        pendingDelete.delete(product.id);
                        pendingDelete = new Set(pendingDelete);
                      };
                    }}
                  >
                    <input type="hidden" name="id" value={product.id} />
                    <Button
                      type="submit"
                      class="btn-sm btn-outline-danger"
                      onclick={(e) => {
                        if (!confirm(`Xoá sản phẩm "${product.name}"?`))
                          e.preventDefault();
                      }}
                      title="Xoá sản phẩm"
                    >
                      <i class="fa-solid fa-trash-can"></i>
                    </Button>
                  </form>
                </div>
              </div>
            </div>
          {/each}
        </div>
      </div>
    {/if}
  </div>

  <!-- ══════════════════════════════════════════════════════════════════════════
     ADD PRODUCT MODAL
══════════════════════════════════════════════════════════════════════════ -->
  <div
    class="modal fade"
    id="addProductModal"
    tabindex="-1"
    aria-labelledby="addProductModalLabel"
    aria-hidden="true"
  >
    <div class="modal-dialog modal-dialog-centered">
      <div class="modal-content" style="border-radius:16px; border:none;">
        <div class="modal-header border-0 pb-0">
          <Heading
            level={5}
            class="modal-title fw-bold"
            id="addProductModalLabel"
          >
            <i class="fa-solid fa-plus-circle text-success me-2"></i>Thêm sản
            phẩm mới
          </Heading>
          <button
            type="button"
            class="btn-close"
            data-bs-dismiss="modal"
            aria-label="Đóng"
          ></button>
        </div>

        <form
          method="POST"
          action="?/add_product"
          use:enhance={() => {
            submittingAdd = true;
            return async ({ update }) => {
              submittingAdd = false;
              await update();
            };
          }}
        >
          <fieldset class="form-fieldset" disabled={submittingAdd}>
            <div class="modal-body d-flex flex-column gap-3 pt-3">
              {#if form?.action === "add_product" && !form?.success && form?.error}
                <div class="alert alert-danger py-2 mb-0">
                  <i class="fa-solid fa-triangle-exclamation me-1"
                  ></i>{form.error}
                </div>
              {/if}

              <!-- Name -->
              <div>
                <label class="form-label fw-semibold" for="ap_name"
                  >Tên sản phẩm <span class="text-danger">*</span></label
                >
                <input
                  id="ap_name"
                  name="name"
                  type="text"
                  class="form-control"
                  placeholder="Gạo ST25, Phân NPK Bình Điền…"
                  required
                />
              </div>

              <!-- Category -->
              <div>
                <label class="form-label fw-semibold" for="ap_cat"
                  >Danh mục <span class="text-danger">*</span></label
                >
                <select
                  id="ap_cat"
                  name="category_id"
                  class="form-select"
                  required
                >
                  <option value="">-- Chọn danh mục --</option>
                  {#each data.categories as cat}
                    <option value={cat.id}>{cat.name}</option>
                  {/each}
                </select>
              </div>

              <!-- Price + Unit (inline) -->
              <div class="row g-2">
                <div class="col">
                  <label class="form-label fw-semibold" for="ap_price"
                    >Giá (₫)</label
                  >
                  <input
                    id="ap_price"
                    name="price"
                    type="number"
                    min="1"
                    class="form-control"
                    placeholder="Để trống = Liên hệ"
                  />
                </div>
                <div class="col-auto" style="min-width:130px">
                  <label class="form-label fw-semibold" for="ap_unit"
                    >Đơn vị</label
                  >
                  <input
                    id="ap_unit"
                    name="unit"
                    type="text"
                    class="form-control"
                    placeholder="kg, cái, bao…"
                  />
                </div>
              </div>

              <!-- Description -->
              <div>
                <label class="form-label fw-semibold" for="ap_desc">Mô tả</label
                >
                <textarea
                  id="ap_desc"
                  name="description"
                  class="form-control"
                  rows="3"
                  placeholder="Xuất xứ, quy cách, đặc điểm nổi bật…"
                ></textarea>
              </div>
            </div>

            <div class="modal-footer border-0 pt-0">
              <Button
                type="button"
                class="btn-outline-secondary"
                data-bs-dismiss="modal">Huỷ</Button
              >
              <Button type="submit" class="btn-success px-4">
                {#if submittingAdd}
                  <Spinner class="me-1" />Đang lưu…
                {:else}
                  <i class="fa-solid fa-plus me-1"></i>Thêm sản phẩm
                {/if}
              </Button>
            </div>
          </fieldset>
        </form>
      </div>
    </div>
  </div>

  <!-- ══════════════════════════════════════════════════════════════════════════
     SECURITY LINK
══════════════════════════════════════════════════════════════════════════ -->
  <div class="dash-card">
    <Button href={ROUTES.ACCOUNT_SECURITY} class="security-link">
      <span class="security-link__icon"><i class="fa-solid fa-key"></i></span>
      <span class="security-link__body">
        <span class="security-link__title">Đổi mật khẩu</span>
        <span class="security-link__sub"
          >Quản lý bảo mật tài khoản tại trang hồ sơ cá nhân</span
        >
      </span>
      <i class="fa-solid fa-chevron-right security-link__arrow"></i>
    </Button>
  </div>
</div>
<!-- /main column -->

<!-- ══════════════════════════════════════════════════════════════════════════
     EDIT PRODUCT MODAL
══════════════════════════════════════════════════════════════════════════ -->
<div
  class="modal fade"
  id="editProductModal"
  tabindex="-1"
  aria-labelledby="editProductModalLabel"
  aria-hidden="true"
>
  <div class="modal-dialog modal-dialog-centered">
    <div class="modal-content" style="border-radius:16px; border:none;">
      <div class="modal-header border-0 pb-0">
        <Heading
          level={5}
          class="modal-title fw-bold"
          id="editProductModalLabel"
        >
          <i class="fa-solid fa-pen-to-square text-warning me-2"></i>Chỉnh sửa
          sản phẩm
        </Heading>
        <button
          type="button"
          class="btn-close"
          data-bs-dismiss="modal"
          aria-label="Đóng"
        ></button>
      </div>

      {#if editingProduct}
        <form
          method="POST"
          action="?/edit_product"
          use:enhance={() => {
            submittingEdit = true;
            return async ({ update }) => {
              submittingEdit = false;
              await update();
            };
          }}
        >
          <fieldset class="form-fieldset" disabled={submittingEdit}>
            <input type="hidden" name="id" value={editingProduct.id} />
            <div class="modal-body d-flex flex-column gap-3 pt-3">
              {#if form?.action === "edit_product" && !form?.success && form?.error}
                <div class="alert alert-danger py-2 mb-0">
                  <i class="fa-solid fa-triangle-exclamation me-1"
                  ></i>{form.error}
                </div>
              {/if}

              <!-- Name -->
              <div>
                <label class="form-label fw-semibold" for="ep_name"
                  >Tên sản phẩm <span class="text-danger">*</span></label
                >
                <input
                  id="ep_name"
                  name="name"
                  type="text"
                  class="form-control"
                  value={editingProduct.name}
                  required
                />
              </div>

              <!-- Category -->
              <div>
                <label class="form-label fw-semibold" for="ep_cat"
                  >Danh mục <span class="text-danger">*</span></label
                >
                <select
                  id="ep_cat"
                  name="category_id"
                  class="form-select"
                  required
                >
                  <option value="">-- Chọn danh mục --</option>
                  {#each data.categories as cat}
                    <option
                      value={cat.id}
                      selected={cat.id === editingProduct.category_id}
                    >
                      {cat.name}
                    </option>
                  {/each}
                </select>
              </div>

              <!-- Price + Unit -->
              <div class="row g-2">
                <div class="col">
                  <label class="form-label fw-semibold" for="ep_price"
                    >Giá (₫)</label
                  >
                  <input
                    id="ep_price"
                    name="price"
                    type="number"
                    min="1"
                    class="form-control"
                    value={editingProduct.price ?? ""}
                    placeholder="Để trống = Liên hệ"
                  />
                </div>
                <div class="col-auto" style="min-width:130px">
                  <label class="form-label fw-semibold" for="ep_unit"
                    >Đơn vị</label
                  >
                  <input
                    id="ep_unit"
                    name="unit"
                    type="text"
                    class="form-control"
                    value={editingProduct.unit ?? ""}
                    placeholder="kg, cái, bao…"
                  />
                </div>
              </div>

              <!-- Description -->
              <div>
                <label class="form-label fw-semibold" for="ep_desc">Mô tả</label
                >
                <textarea
                  id="ep_desc"
                  name="description"
                  class="form-control"
                  rows="3">{editingProduct.description ?? ""}</textarea
                >
              </div>
            </div>

            <div class="modal-footer border-0 pt-0">
              <Button
                type="button"
                class="btn-outline-secondary"
                data-bs-dismiss="modal">Huỷ</Button
              >
              <Button type="submit" class="btn-warning px-4">
                {#if submittingEdit}
                  <Spinner class="me-1" />Đang lưu…
                {:else}
                  <i class="fa-solid fa-floppy-disk me-1"></i>Lưu thay đổi
                {/if}
              </Button>
            </div>
          </fieldset>
        </form>
      {/if}
    </div>
  </div>
</div>

<style>
  /* ── Security link ───────────────────────────────────── */
  :global(.security-link) {
    display: flex !important;
    align-items: center;
    gap: 14px;
    padding: 16px 20px;
    text-decoration: none !important;
    color: inherit !important;
    text-align: left;
    width: 100%;
    transition: background 0.15s;
  }

  :global(.security-link:hover) {
    background: #f8faf9;
    color: inherit;
  }
  .security-link__icon {
    width: 40px;
    height: 40px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #ecfdf5;
    border-radius: 10px;
    color: #047857;
    font-size: 1rem;
    flex-shrink: 0;
  }
  .security-link__body {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }
  .security-link__title {
    font-size: 0.95rem;
    font-weight: 700;
    color: var(--text, #0c1a12);
  }
  .security-link__sub {
    font-size: 0.8rem;
    color: var(--muted, #5e7a68);
  }
  .security-link__arrow {
    color: var(--muted, #5e7a68);
    font-size: 0.75rem;
    flex-shrink: 0;
  }

  /* ── Page header ─────────────────────────────────────── */
  .dash-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 24px 16px 0;
    flex-wrap: wrap;
    gap: 12px;
  }
  .dash-title {
    font-size: 1.4rem;
    font-weight: 800;
    margin: 0 0 4px;
  }
  .dash-sub {
    font-size: 0.875rem;
    color: var(--muted, #5e7a68);
    margin: 0;
  }

  /* ── Card ────────────────────────────────────────────── */
  .dash-card {
    background: var(--surface, #fff);
    border-radius: 16px;
    border: 1px solid var(--border, #e5e7eb);
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.06);
    overflow: hidden;
  }
  .dash-card__head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border, #e5e7eb);
    flex-wrap: wrap;
    gap: 8px;
  }
  .dash-card__title {
    font-size: 1rem;
    font-weight: 700;
    margin: 0;
    display: flex;
    align-items: center;
  }
  .dash-card__body {
    padding: 20px;
  }

  /* ── Product tile ────────────────────────────────────── */
  .product-tile {
    display: flex;
    gap: 12px;
    align-items: flex-start;
    background: var(--bg, #f5f8f6);
    border-radius: 14px;
    padding: 14px;
    height: 100%;
    transition: box-shadow 0.15s;
  }
  .product-tile:hover {
    box-shadow: 0 4px 16px rgba(6, 78, 59, 0.1);
  }

  .product-tile__icon {
    width: 44px;
    height: 44px;
    flex-shrink: 0;
    background: var(--p50, #ecfdf5);
    border-radius: 12px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1.1rem;
    color: var(--p700, #065f46);
  }
  .product-tile__thumb {
    width: 56px;
    height: 56px;
    flex-shrink: 0;
    border-radius: 10px;
    object-fit: cover;
    border: 1px solid var(--border, #e5e7eb);
  }

  .product-tile__body {
    flex: 1;
    min-width: 0;
  }
  :global(.product-tile__name) {
    font-weight: 700;
    font-size: 0.95rem;
    margin: 0 0 4px;
    line-height: 1.3;
  }
  .product-tile__cat {
    font-size: 0.7rem;
    display: inline-block;
    margin-bottom: 6px;
  }
  :global(.product-tile__price) {
    font-size: 1rem;
    font-weight: 700;
    color: #ea580c;
    margin: 4px 0 0;
  }
  :global(.product-tile__price-contact) {
    font-size: 0.85rem;
    color: #b45309;
    font-weight: 600;
    margin: 4px 0 0;
  }
  .product-tile__unit {
    font-size: 0.8rem;
    color: #6b7280;
    font-weight: 400;
  }
  :global(.product-tile__desc) {
    font-size: 0.8rem;
    color: var(--muted, #5e7a68);
    margin: 6px 0 0;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .product-tile__actions {
    flex-shrink: 0;
  }
</style>
