<script lang="ts">
    interface Props {
        size?: 'sm' | 'md';
        class?: string;
    }
    let { size = 'sm', class: extraClass = '' }: Props = $props();
</script>

<span
    class="spinner spinner--{size} {extraClass}"
    role="status"
    aria-hidden="true"
></span>

<style>
    .spinner {
        display: inline-block;
        width: 1.5rem;
        height: 1.5rem;
        vertical-align: -0.125em;
        border: 0.2em solid currentColor;
        border-right-color: transparent;
        border-radius: 50%;
        animation: spinner-border 0.75s linear infinite;
    }
    .spinner--sm {
        width: 1rem;
        height: 1rem;
        border-width: 0.15em;
    }
    @keyframes spinner-border {
        to { transform: rotate(360deg); }
    }
</style>
