<script lang="ts">
    import Spinner from '$lib/components/atoms/Spinner.svelte';
    import Button from '$lib/components/atoms/Button.svelte';

    interface Props {
        submitting: boolean;
        icon: string;           // Font Awesome solid class, e.g. "fa-check"
        label: string;
        loadingLabel?: string;
        class?: string;
    }
    let {
        submitting,
        icon,
        label,
        loadingLabel = 'Đang xử lý…',
        class: extraClass = ''
    }: Props = $props();
</script>

<Button type="submit" class="btn-success w-100 {extraClass}" disabled={submitting}>
    {#if submitting}
        <Spinner size="sm" />
        <span>{loadingLabel}</span>
    {:else}
        <i class="fa-solid {icon}"></i>
        <span>{label}</span>
    {/if}
</Button>
