<script lang="ts">
  import { enhance } from '$app/forms';
  import Button from '$lib/components/atoms/Button.svelte';
  import Spinner from '$lib/components/atoms/Spinner.svelte';
  import Heading from '$lib/components/atoms/Heading.svelte';

  interface Props {
    action: string;
    hiddenFields?: Record<string, string | number | boolean>;
    title: string;
    message: string;
    targetName?: string;
    icon?: string;
    confirmLabel?: string;
    confirmClass?: string;
    requireReason?: boolean;
    reasonLabel?: string;
    reasonPlaceholder?: string;
    onCancel: () => void;
  }

  const {
    action,
    hiddenFields = {},
    title,
    message,
    targetName,
    icon = 'fa-solid fa-triangle-exclamation text-warning',
    confirmLabel = 'Xác nhận',
    confirmClass = 'btn-outline-danger',
    requireReason = false,
    reasonLabel = 'Lý do',
    reasonPlaceholder = 'Nhập lý do (tuỳ chọn)...',
    onCancel,
  }: Props = $props();

  let submitting = $state(false);
</script>

<div class="modal-backdrop show" onclick={onCancel} role="presentation"></div>
<div class="modal d-block" tabindex="-1" role="dialog" aria-modal="true">
  <div class="modal-dialog modal-dialog-centered">
    <div class="modal-content rounded-3">
      <div class="modal-header">
        <Heading level={5} class="modal-title">
          <i class="{icon} me-2"></i>{title}
        </Heading>
        <button type="button" class="btn-close" aria-label="Đóng" onclick={onCancel}></button>
      </div>
      <form
        method="POST"
        {action}
        use:enhance={() => {
          submitting = true;
          return async ({ update }) => {
            submitting = false;
            await update();
            onCancel();
          };
        }}
      >
        {#each Object.entries(hiddenFields) as [name, value]}
          <input type="hidden" {name} value={String(value)} />
        {/each}
        <fieldset class="form-fieldset" disabled={submitting}>
          <div class="modal-body d-flex flex-column gap-3">
            {#if targetName}
              <div class="alert alert-light border py-2 mb-0">
                <i class="fa-solid fa-info-circle text-muted me-1"></i>
                <strong>{targetName}</strong>
              </div>
            {/if}
            <p class="mb-0 text-secondary small">{message}</p>
            {#if requireReason}
              <div>
                <label class="form-label fw-semibold" for="confirm_reason">{reasonLabel}</label>
                <textarea
                  id="confirm_reason"
                  name="reason"
                  class="form-control"
                  rows="2"
                  placeholder={reasonPlaceholder}
                ></textarea>
              </div>
            {/if}
          </div>
          <div class="modal-footer">
            <Button type="button" class="btn-outline-secondary" onclick={onCancel}>Huỷ</Button>
            <Button type="submit" class="{confirmClass} px-4">
              {#if submitting}
                <Spinner class="me-1" />Đang xử lý…
              {:else}
                {confirmLabel}
              {/if}
            </Button>
          </div>
        </fieldset>
      </form>
    </div>
  </div>
</div>
