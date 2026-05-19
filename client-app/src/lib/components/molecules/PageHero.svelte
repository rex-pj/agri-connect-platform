<script lang="ts">
    import type { Snippet } from 'svelte';
    import Heading from '$lib/components/atoms/Heading.svelte';
    import Text from '$lib/components/atoms/Text.svelte';
    import Button from '$lib/components/atoms/Button.svelte';

    interface Props {
        title: string;
        icon?: string;      // Font Awesome solid class, e.g. "fa-bag-shopping"
        sub?: string;
        backLink?: { href: string; label: string };
        variant?: 'detail' | 'compact';
        children?: Snippet; // rendered inside .page-hero__row alongside the title
    }
    let { title, icon, sub, backLink, variant, children }: Props = $props();
</script>

<div class="page-hero"
    class:page-hero--detail={variant === 'detail'}
    class:page-hero--compact={variant === 'compact'}>
    {#if backLink}
        <Button variant="ghost" size="sm" href={backLink.href} class="back-link">
            <i class="fa-solid fa-arrow-left"></i> {backLink.label}
        </Button>
    {/if}
    {#if children}
        <div class="page-hero__row">
            <Heading level={1} class="page-hero__title">
                {#if icon}<i class="fa-solid {icon}"></i>{/if}
                {title}
            </Heading>
            {@render children()}
        </div>
    {:else}
        <Heading level={1} class="page-hero__title">
            {#if icon}<i class="fa-solid {icon}"></i>{/if}
            {title}
        </Heading>
    {/if}
    {#if sub}
        <Text class="page-hero__sub">{sub}</Text>
    {/if}
</div>
