<script lang="ts">
    import Button from '$lib/components/atoms/Button.svelte';

    interface Props {
        currentPage: number;
        totalPages: number;
        onPageChange: (p: number) => void;
    }
    let { currentPage, totalPages, onPageChange }: Props = $props();

    // Build a compact page list: always first/last, ±2 around current, ellipsis gaps
    const pages = $derived(buildPages(currentPage, totalPages));

    function buildPages(cur: number, total: number): (number | '...')[] {
        if (total <= 7) return Array.from({ length: total }, (_, i) => i + 1);
        const set = new Set<number>([1, total]);
        for (let i = Math.max(2, cur - 2); i <= Math.min(total - 1, cur + 2); i++) set.add(i);
        const sorted = Array.from(set).sort((a, b) => a - b);
        const result: (number | '...')[] = [];
        for (let i = 0; i < sorted.length; i++) {
            if (i > 0 && sorted[i] - sorted[i - 1] > 1) result.push('...');
            result.push(sorted[i]);
        }
        return result;
    }
</script>

<nav class="pagination-row" aria-label="Phân trang">
    <Button type="button" class="page-btn"
        onclick={() => onPageChange(currentPage - 1)}
        disabled={currentPage <= 1}
        aria-label="Trang trước">
        <i class="fa-solid fa-chevron-left"></i>
    </Button>
    {#each pages as p}
        {#if p === '...'}
            <span class="page-ellipsis">…</span>
        {:else}
            <Button type="button" class="page-btn{p === currentPage ? ' page-btn--active' : ''}"
                onclick={() => onPageChange(p as number)}
                aria-label="Trang {p}"
                aria-current={p === currentPage ? 'page' : undefined}>{p}</Button>
        {/if}
    {/each}
    <Button type="button" class="page-btn"
        onclick={() => onPageChange(currentPage + 1)}
        disabled={currentPage >= totalPages}
        aria-label="Trang sau">
        <i class="fa-solid fa-chevron-right"></i>
    </Button>
</nav>

<style>
    .page-ellipsis {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        min-width: 40px;
        height: 40px;
        color: #9ab5a1;
        font-size: 1rem;
        user-select: none;
    }
</style>
