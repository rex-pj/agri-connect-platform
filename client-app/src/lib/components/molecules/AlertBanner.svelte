<script lang="ts">
    interface Props {
        type: 'error' | 'success';
        message: string;
    }
    let { type, message }: Props = $props();
</script>

{#if type === 'error'}
    <div class="alert alert--error">
        <i class="fa-solid fa-triangle-exclamation alert__icon"></i>
        <span>{message}</span>
    </div>
{:else if type === 'success'}
    <div class="alert alert--success">
        <i class="fa-solid fa-circle-check alert__icon"></i>
        <span>{message}</span>
    </div>
{/if}

<style>
    .alert {
        display: flex;
        align-items: center;
        gap: 0.75rem;
        padding: 0.75rem 1rem;
        border-radius: 0.75rem;
        font-size: 0.9375rem;
        font-weight: 500;
        margin-bottom: 1rem;
        border: 1px solid transparent;
    }
    .alert__icon {
        flex-shrink: 0;
        font-size: 1.1rem;
    }
    .alert--error {
        background-color: #fef2f2;
        color: #991b1b;
        border-color: #fee2e2;
    }
    .alert--success {
        background-color: #f0fdf4;
        color: #166534;
        border-color: #dcfce7;
    }
</style>
