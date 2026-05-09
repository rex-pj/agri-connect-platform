<script lang="ts">
    interface Props {
        text: string;
    }
    let { text }: Props = $props();
</script>

<span class="tag">{text}</span>

<style>
    .tag {
        display: inline-flex;
        align-items: center;
        padding: 0.125rem 0.625rem;
        border-radius: 9999px;
        font-size: 0.75rem;
        font-weight: 700;
        background: var(--p50, #f0fdf4);
        color: var(--p700, #15803d);
        border: 1px solid var(--p100, #dcfce7);
        white-space: nowrap;
        line-height: 1.5;
    }
</style>
