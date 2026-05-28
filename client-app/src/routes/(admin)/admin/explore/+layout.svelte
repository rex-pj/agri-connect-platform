<script lang="ts">
    import AdminShell from '$lib/components/organisms/AdminShell.svelte';
    import type { LayoutProps } from './$types';

    const { children, data }: LayoutProps = $props();
</script>

<AdminShell username={data.username}>
    {@render children()}
</AdminShell>
