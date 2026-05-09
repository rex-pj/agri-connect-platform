<script lang="ts">
    import { navigating } from '$app/stores';
</script>

{#if $navigating}
    <div class="nav-progress" role="progressbar" aria-label="Đang tải trang…" aria-live="polite">
        <div class="nav-progress__bar"></div>
    </div>
{/if}

<style>
    .nav-progress {
        position: fixed;
        top: 0;
        left: 0;
        right: 0;
        height: 3px;
        z-index: 9999;
        overflow: hidden;
        background: rgba(5, 150, 105, 0.15);
        pointer-events: none;
    }

    .nav-progress__bar {
        position: absolute;
        top: 0;
        bottom: 0;
        width: 45%;
        background: #059669;
        border-radius: 0 3px 3px 0;
        animation: nav-sweep 1.2s ease-in-out infinite;
    }

    @keyframes nav-sweep {
        0%   { left: -45%; }
        100% { left: 100%; }
    }
</style>
