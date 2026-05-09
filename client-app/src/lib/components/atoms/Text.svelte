<script lang="ts">
    import type { Snippet } from 'svelte';

    interface Props {
        class?: string;
        children: Snippet;
        [key: string]: unknown;
    }

    let { class: extraClass = '', children, ...rest }: Props = $props();
</script>

<p class="text {extraClass}" {...rest}>{@render children()}</p>

<style>
    .text {
        margin: 0;
        font-size: 1rem;
        line-height: 1.6;
        color: var(--text, #0c1a12);
        font-weight: 400;
    }
</style>
