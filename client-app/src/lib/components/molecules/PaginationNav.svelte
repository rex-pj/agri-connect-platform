<script lang="ts">
    import Button from '$lib/components/atoms/Button.svelte';

    interface Props {
        currentPage: number;
        totalPages: number;
        onPageChange: (p: number) => void;
    }
    let { currentPage, totalPages, onPageChange }: Props = $props();
    const pages = $derived(Array.from({ length: totalPages }, (_, i) => i + 1));
</script>

<nav class="pagination-row" aria-label="Phân trang">
    <Button type="button" class="page-btn"
        onclick={() => onPageChange(currentPage - 1)}
        disabled={currentPage <= 1}
        aria-label="Trang trước">
        <i class="fa-solid fa-chevron-left"></i>
    </Button>
    {#each pages as p}
        <Button type="button" class="page-btn{p === currentPage ? ' page-btn--active' : ''}"
            onclick={() => onPageChange(p)}>{p}</Button>
    {/each}
    <Button type="button" class="page-btn"
        onclick={() => onPageChange(currentPage + 1)}
        disabled={currentPage >= totalPages}
        aria-label="Trang sau">
        <i class="fa-solid fa-chevron-right"></i>
    </Button>
</nav>
