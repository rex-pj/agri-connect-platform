<script lang="ts">
    import Heading from '$lib/components/atoms/Heading.svelte';
    import Button from '$lib/components/atoms/Button.svelte';

    interface Props {
        title: string;
        moreHref?: string;
        moreLabel?: string;
    }
    let { title, moreHref, moreLabel = 'Xem thêm' }: Props = $props();
</script>

<div class="section-header mb-3">
    <Heading level={2} class="section-title">{title}</Heading>
    {#if moreHref}
        <Button href={moreHref} class="section-more">{moreLabel}</Button>
    {/if}
</div>
