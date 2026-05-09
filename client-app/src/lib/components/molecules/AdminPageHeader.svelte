<script lang="ts">
    import type { Snippet } from 'svelte';
    import Heading from '$lib/components/atoms/Heading.svelte';
    import Text from '$lib/components/atoms/Text.svelte';

    interface Props {
        title: string;
        icon?: string;
        sub?: string;
        children?: Snippet;
    }

    let { title, icon, sub, children }: Props = $props();
</script>

<div class="admin-page-header">
    <div>
        <Heading level={1} class="admin-page-title">
            {#if icon}<i class="fa-solid {icon} me-2 text-success"></i>{/if}{title}
        </Heading>
        {#if sub}<Text class="admin-page-sub">{sub}</Text>{/if}
    </div>
    {#if children}{@render children()}{/if}
</div>
