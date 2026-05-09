<script lang="ts">
    import '../app.css';
    import NavProgress from '$lib/components/atoms/NavProgress.svelte';
    import type { LayoutProps } from './$types';
    const { children }: LayoutProps = $props();
</script>

<NavProgress />
{@render children()}
