<script lang="ts">
    import type { Snippet } from 'svelte';

    interface Props {
        variant?: 'primary' | 'danger';
        icon?: string;  // Font Awesome solid class, e.g. "fa-circle-check"
        class?: string;
        children?: Snippet;
    }
    let { variant = 'primary', icon, class: extraClass = '', children }: Props = $props();
</script>

<span class="badge badge--{variant} {extraClass}">
    {#if icon}<i class="fa-solid {icon} badge__icon"></i>{/if}
    {#if children}{@render children()}{/if}
</span>

<style>
    .badge {
        display: inline-flex;
        align-items: center;
        gap: 0.375rem;
        padding: 0.25rem 0.625rem;
        border-radius: 9999px;
        font-size: 0.75rem;
        font-weight: 700;
        line-height: 1;
        white-space: nowrap;
    }
    .badge__icon {
        font-size: 0.8em;
    }
    .badge--primary {
        background-color: var(--p100, #d1fae5);
        color: var(--p800, #064e3b);
    }
    .badge--danger {
        background-color: #fee2e2;
        color: #991b1b;
    }
</style>
