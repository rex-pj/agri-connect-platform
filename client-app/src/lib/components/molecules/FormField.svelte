<script lang="ts">
    import type { Snippet } from 'svelte';

    interface Props {
        label: string;
        forId: string;
        required?: boolean;
        error?: string | null;
        children: Snippet;
    }
    let { label, forId, required = false, error, children }: Props = $props();
</script>

<div class="field" class:field--error={!!error}>
    <label class="field__label" for={forId}>
        {label}{#if required} <span class="req">*</span>{/if}
    </label>
    {@render children()}
    {#if error}
        <span class="field__error">
            <i class="fa-solid fa-circle-exclamation"></i> {error}
        </span>
    {/if}
</div>

<style>
    .field { display: flex; flex-direction: column; gap: 6px; }
    .field__label { font-size: 0.9rem; font-weight: 700; color: #1A1A1A; }
    .req { color: #C0392B; }
    .field__error {
        font-size: 0.8rem;
        font-weight: 600;
        color: #C0392B;
        display: flex;
        align-items: center;
        gap: 5px;
    }
</style>
