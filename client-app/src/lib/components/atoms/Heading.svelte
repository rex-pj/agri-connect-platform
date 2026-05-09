<script lang="ts">
    import type { Snippet } from 'svelte';

    interface Props {
        level: 1 | 2 | 3 | 4 | 5 | 6;
        class?: string;
        children: Snippet;
        [key: string]: unknown;
    }

    let { level, class: extraClass = '', children, ...rest }: Props = $props();
</script>

{#if level === 1}
    <h1 class="h1 {extraClass}" {...rest}>{@render children()}</h1>
{:else if level === 2}
    <h2 class="h2 {extraClass}" {...rest}>{@render children()}</h2>
{:else if level === 3}
    <h3 class="h3 {extraClass}" {...rest}>{@render children()}</h3>
{:else if level === 4}
    <h4 class="h4 {extraClass}" {...rest}>{@render children()}</h4>
{:else if level === 5}
    <h5 class="h5 {extraClass}" {...rest}>{@render children()}</h5>
{:else}
    <h6 class="h6 {extraClass}" {...rest}>{@render children()}</h6>
{/if}

<style>
    .h1, .h2, .h3, .h4, .h5, .h6 {
        margin: 0;
        color: var(--text, #0c1a12);
        line-height: 1.25;
        font-weight: 800;
        letter-spacing: -0.02em;
    }
    .h1 { font-size: 2.25rem; }
    .h2 { font-size: 1.75rem; }
    .h3 { font-size: 1.375rem; }
    .h4 { font-size: 1.125rem; }
    .h5 { font-size: 1rem; }
    .h6 { font-size: 0.875rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.05em; }
</style>
