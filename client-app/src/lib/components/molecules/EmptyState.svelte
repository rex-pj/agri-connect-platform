<script lang="ts">
    import type { Snippet } from 'svelte';
    import Heading from '$lib/components/atoms/Heading.svelte';
    import Text from '$lib/components/atoms/Text.svelte';

    interface Props {
        icon: string;   // Font Awesome solid class, e.g. "fa-inbox"
        title: string;
        hint: string;
        action?: Snippet;
    }
    let { icon, title, hint, action }: Props = $props();
</script>

<div class="empty-state">
    <i class="fa-solid {icon} empty-state__icon"></i>
    <Heading level={3} class="empty-state__title">{title}</Heading>
    <Text class="empty-state__hint">{hint}</Text>
    {#if action}
        <div class="empty-state__action">
            {@render action()}
        </div>
    {/if}
</div>

<style>
    .empty-state__action {
        margin-top: 16px;
    }
</style>
