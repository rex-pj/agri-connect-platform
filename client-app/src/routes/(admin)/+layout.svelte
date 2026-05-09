<script lang="ts">
    import type { LayoutProps } from './$types';
    const { children }: LayoutProps = $props();
</script>

<svelte:head>
    <meta name="robots" content="noindex, nofollow" />
</svelte:head>

{@render children()}
