<script lang="ts">
  import { enhance } from "$app/forms";
  import SubmitButton from "$lib/components/atoms/SubmitButton.svelte";
  import Button from "$lib/components/atoms/Button.svelte";
  import Heading from "$lib/components/atoms/Heading.svelte";
  import Text from "$lib/components/atoms/Text.svelte";
  import type { ActionData } from "./$types";
  import { ROUTES } from "$lib/routes";

  const { form }: { form: ActionData } = $props();

  let submitting = $state(false);
</script>

<svelte:head>
  <title>Đăng ký Nhà cung cấp — Agri-Connect</title>
  <meta name="robots" content="noindex, nofollow" />
</svelte:head>

<div class="auth-shell">
  <div class="auth-card">
    <div class="auth-brand">
      <i class="fa-solid fa-seedling auth-brand__icon"></i>
      <span class="auth-brand__name">Agri-Connect</span>
    </div>

    <Heading level={1} class="auth-title">Đăng ký Nhà cung cấp</Heading>
    <Text class="auth-sub"
      >Tạo tài khoản để đăng tin dịch vụ và sản phẩm của bạn.</Text
    >

    {#if form?.error}
      <div class="alert alert-danger py-2 small mb-3">
        <i class="fa-solid fa-triangle-exclamation me-1"></i>{form.error}
      </div>
    {/if}

    <form
      method="POST"
      use:enhance={() => {
        submitting = true;
        return async ({ update }) => {
          submitting = false;
          await update();
        };
      }}
      class="auth-form"
    >
      <fieldset class="form-fieldset" disabled={submitting}>
        <p
          class="d-block text-uppercase fw-bold small text-success border-bottom border-success-subtle pb-1 mb-3"
        >
          Thông tin doanh nghiệp
        </p>

        <div class="mb-3">
          <label class="form-label fw-semibold" for="r_biz_name">
            Tên doanh nghiệp <span class="text-danger">*</span>
          </label>
          <input
            id="r_biz_name"
            name="business_name"
            type="text"
            class="form-control form-control-lg"
            placeholder="VD: Dịch vụ cơ giới Minh Hòa"
            required
          />
        </div>

        <div class="mb-4">
          <label class="form-label fw-semibold" for="r_biz_phone">
            Số điện thoại doanh nghiệp <span class="text-danger">*</span>
          </label>
          <input
            id="r_biz_phone"
            name="business_phone"
            type="tel"
            class="form-control form-control-lg"
            placeholder="0901234567"
            autocomplete="tel"
            pattern={"0\\d{9}"}
            required
          />
          <div class="form-text">10 chữ số, bắt đầu bằng 0.</div>
        </div>

        <p
          class="d-block text-uppercase fw-bold small text-success border-bottom border-success-subtle pb-1 mb-3"
        >
          Thông tin tài khoản
        </p>

        <div class="mb-3">
          <label class="form-label fw-semibold" for="r_fullname"
            >Họ và tên</label
          >
          <input
            id="r_fullname"
            name="full_name"
            type="text"
            class="form-control form-control-lg"
            placeholder="Nguyễn Văn A"
            autocomplete="name"
          />
        </div>

        <div class="mb-3">
          <label class="form-label fw-semibold" for="r_email">
            Email <span class="text-danger">*</span>
          </label>
          <input
            id="r_email"
            name="email"
            type="email"
            class="form-control form-control-lg"
            placeholder="email@example.com"
            autocomplete="email"
            required
          />
        </div>

        <div class="mb-3">
          <label class="form-label fw-semibold" for="r_user">
            Tên đăng nhập <span class="text-danger">*</span>
          </label>
          <input
            id="r_user"
            name="username"
            type="text"
            class="form-control form-control-lg"
            placeholder="VD: minhhoa_dichvu"
            autocomplete="username"
            minlength="3"
            required
          />
        </div>

        <div class="mb-3">
          <label class="form-label fw-semibold" for="r_pw">
            Mật khẩu <span class="text-danger">*</span>
          </label>
          <input
            id="r_pw"
            name="password"
            type="password"
            class="form-control form-control-lg"
            placeholder="Tối thiểu 8 ký tự, có chữ hoa, số và ký tự đặc biệt"
            autocomplete="new-password"
            minlength="8"
            required
          />
          <div class="form-text">
            Ít nhất 8 ký tự · 1 chữ hoa · 1 chữ số · 1 ký tự đặc biệt
          </div>
        </div>

        <div class="mb-4">
          <label class="form-label fw-semibold" for="r_pw2">
            Xác nhận mật khẩu <span class="text-danger">*</span>
          </label>
          <input
            id="r_pw2"
            name="password2"
            type="password"
            class="form-control form-control-lg"
            placeholder="Nhập lại mật khẩu"
            autocomplete="new-password"
            minlength="8"
            required
          />
        </div>

        <SubmitButton
          {submitting}
          icon="fa-user-plus"
          label="Tạo tài khoản"
          loadingLabel="Đang đăng ký…"
          class="btn-lg w-100"
        />
      </fieldset>
    </form>

    <Text class="auth-footer">
      Đã có tài khoản?
      <Button href={ROUTES.LOGIN}>Đăng nhập</Button>
      <span class="mx-2 text-muted">·</span>
      <Button href={ROUTES.HOME} class="text-muted">Về trang chủ</Button>
    </Text>

    <div class="alert alert-success py-2 small text-center mt-3">
      <i class="fa-solid fa-circle-info me-1"></i>
      Hồ sơ doanh nghiệp sẽ được xét duyệt trước khi hiển thị công khai.
    </div>
  </div>
</div>

<style>
  :global(body) {
    padding-top: 0 !important;
    padding-bottom: 0 !important;
  }
  .auth-card {
    max-width: 480px;
  }
</style>
