<script lang="ts">
    import { enhance } from '$app/forms';
    import Spinner   from '$lib/components/atoms/Spinner.svelte';
    import FormField from '$lib/components/molecules/FormField.svelte';
    import Heading from '$lib/components/atoms/Heading.svelte';
    import Text from '$lib/components/atoms/Text.svelte';
    import Button from '$lib/components/atoms/Button.svelte';

    interface FormState {
        success?: boolean;
        message?: string;
        field_errors?: Record<string, string> | null;
        values?: Record<string, string> | null;
    }

    interface Props {
        provider: { id: number; name: string; services: string[] };
        form?: FormState | null;
        onClose: () => void;
    }
    const { provider, form, onClose }: Props = $props();

    const fe   = $derived(form?.field_errors ?? {});
    const vals = $derived(form?.values ?? {});

    let submitting = $state(false);
</script>

<dialog class="sheet-backdrop" open
    aria-label="Yêu cầu liên hệ"
    onclick={(e) => { if (e.target === e.currentTarget) onClose(); }}
    onkeydown={(e) => { if (e.key === 'Escape') onClose(); }}>
    <div class="sheet">
        <div class="sheet__handle" aria-hidden="true"></div>
        <div class="sheet__header">
            <Heading level={2} class="sheet__title"><i class="fa-solid fa-phone"></i> Yêu cầu liên hệ</Heading>
            <Button type="button" class="sheet__close" onclick={onClose} aria-label="Đóng">
                <i class="fa-solid fa-xmark"></i>
            </Button>
        </div>
        <form class="sheet__body" method="POST" action="?/contact_request"
            use:enhance={() => {
                submitting = true;
                return async ({ result, update }) => {
                    submitting = false;
                    if (result.type === 'success') {
                        await update({ reset: true });
                    } else {
                        await update({ reset: false });
                    }
                };
            }}>
            <fieldset class="form-fieldset" disabled={submitting}>
            <input type="hidden" name="business_id" value={provider.id} />
            {#if form && !form.success && form.message && !form.field_errors}
                <div class="status-banner status-banner--error">
                    <i class="fa-solid fa-triangle-exclamation"></i> {form.message}
                </div>
            {/if}
            <Text class="sheet__sub">Gửi yêu cầu tới <strong>{provider.name}</strong></Text>

            <FormField label="Họ và tên" forId="cs_name" required error={fe.requester_name}>
                <input id="cs_name" name="requester_name" type="text"
                       class="field__input" class:field__input--error={fe.requester_name}
                       placeholder="Nguyễn Văn A" required
                       value={vals.requester_name ?? ''} />
            </FormField>

            <FormField label="Số điện thoại" forId="cs_phone" required error={fe.requester_phone}>
                <input id="cs_phone" name="requester_phone" type="tel"
                       class="field__input" class:field__input--error={fe.requester_phone}
                       placeholder="0901234567" pattern={"0\\d{9}"} required
                       value={vals.requester_phone ?? ''} />
            </FormField>

            <FormField label="Dịch vụ cần" forId="cs_topic" required error={fe.topic}>
                <select id="cs_topic" name="topic"
                        class="field__input" class:field__input--error={fe.topic} required>
                    <option value="">-- Chọn dịch vụ --</option>
                    {#each provider.services as svc}
                        <option value={svc} selected={vals.topic === svc}>{svc}</option>
                    {/each}
                </select>
            </FormField>

            <FormField label="Ghi chú thêm" forId="cs_note" error={fe.note}>
                <textarea id="cs_note" name="note" class="field__input" class:field__input--error={fe.note}
                          rows="3" maxlength="1000"
                          placeholder="Ví dụ: ruộng 5 sào, cần thợ sáng sớm…">{vals.note ?? ''}</textarea>
            </FormField>

            <div class="sheet__actions">
                <Button type="button" class="btn-cancel" onclick={onClose}>Huỷ</Button>
                <Button type="submit" class="btn-submit">
                    {#if submitting}
                        <Spinner class="text-white" /> Đang gửi…
                    {:else}
                        <i class="fa-solid fa-paper-plane"></i> GỬI YÊU CẦU
                    {/if}
                </Button>
            </div>
            </fieldset>
        </form>
    </div>
</dialog>

<style>
dialog.sheet-backdrop {
    position: fixed; inset: 0; z-index: 1050;
    background: rgba(0,0,0,0.5);
    display: flex; align-items: flex-end;
    padding: 0; border: none; margin: 0;
    width: 100%; height: 100%;
    max-width: 100%; max-height: 100%;
}
.sheet {
    width: 100%; max-height: 90dvh;
    background: #fff; border-radius: 20px 20px 0 0;
    display: flex; flex-direction: column;
    animation: slideUp 0.25s ease-out;
    padding-bottom: env(safe-area-inset-bottom, 0px);
}
@keyframes slideUp {
    from { transform: translateY(100%); }
    to   { transform: translateY(0); }
}
.sheet__handle {
    width: 40px; height: 4px;
    background: #D0DDD2; border-radius: 2px;
    margin: 12px auto 0; flex-shrink: 0;
}
.sheet__header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 16px 20px 12px;
    border-bottom: 1px solid #D0DDD2;
    flex-shrink: 0;
}
.sheet__title {
    font-size: 1.1rem; font-weight: 800; margin: 0;
    display: flex; align-items: center; gap: 8px;
}
.sheet__title i { color: #1B6B3A; }
.sheet__close {
    width: 36px; height: 36px;
    display: flex; align-items: center; justify-content: center;
    border: none; background: #F4F7F2; border-radius: 50%;
    font-size: 1rem; cursor: pointer;
}
.sheet__body {
    padding: 16px 20px;
    overflow-y: auto;
    display: flex; flex-direction: column; gap: 14px;
}
.sheet__sub    { font-size: 0.9rem; color: #5A6B5D; margin: 0; }
.sheet__actions { display: flex; gap: 10px; margin-top: 4px; }

.status-banner {
    display: flex; align-items: center; gap: 10px;
    padding: 12px 14px; border-radius: 10px;
    font-size: 0.9rem; font-weight: 500;
}
.status-banner--error { background: #FFEBEE; color: #B71C1C; }

/* Input styles defined here apply to inputs passed via FormField snippet,
   because Svelte scopes styles to where elements are defined (ContactSheet). */
.field__input {
    width: 100%; min-height: 52px;
    padding: 12px 14px;
    border: 1.5px solid #D0DDD2; border-radius: 10px;
    font-size: 1rem; font-family: inherit;
    color: #1A1A1A; background: #fff;
    transition: border-color 0.15s;
    appearance: auto;
}
.field__input:focus {
    outline: none;
    border-color: #1B6B3A;
    box-shadow: 0 0 0 3px rgba(27,107,58,0.12);
}
.field__input--error {
    border-color: #C0392B;
    background: #FFF5F5;
}
.field__input--error:focus {
    border-color: #C0392B;
    box-shadow: 0 0 0 3px rgba(192,57,43,0.12);
}
textarea.field__input { min-height: unset; resize: none; }

.btn-cancel {
    flex: 0 0 auto; min-height: 52px; padding: 0 20px;
    border: 1.5px solid #D0DDD2; border-radius: 12px;
    background: transparent; color: #5A6B5D;
    font-size: 0.95rem; font-weight: 600; cursor: pointer;
}
.btn-submit {
    flex: 1; min-height: 52px;
    display: flex; align-items: center; justify-content: center; gap: 8px;
    border: none; border-radius: 12px;
    background: #1B6B3A; color: #fff;
    font-size: 0.95rem; font-weight: 800;
    cursor: pointer;
    transition: background 0.15s, transform 0.1s;
}
.btn-submit:active { transform: scale(0.97); }
</style>
