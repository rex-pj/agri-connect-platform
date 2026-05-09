<script lang="ts">
  import Button from "$lib/components/atoms/Button.svelte";

  const {
    options,
    selected = "",
    onSelect,
  }: {
    options: { value: string; label: string }[];
    selected?: string;
    onSelect: (v: string) => void;
  } = $props();
</script>

<div class="chips" role="group" aria-label="Lọc dịch vụ">
  <Button
    type="button"
    class="chip{selected === '' ? ' chip--on' : ''}"
    onclick={() => onSelect("")}
    aria-pressed={selected === ""}
  >
    Tất cả
  </Button>
  {#each options as opt}
    <Button
      type="button"
      class="chip{selected === opt.value ? ' chip--on' : ''}"
      onclick={() => onSelect(opt.value)}
      aria-pressed={selected === opt.value}
    >
      {opt.label}
    </Button>
  {/each}
</div>

<style>
  .chips {
    display: flex;
    gap: 8px;
    overflow-x: auto;
    padding: 12px 16px;
    scrollbar-width: none;
    -webkit-overflow-scrolling: touch;
    border-bottom: 1px solid var(--border, #c8d8ce);
    background: var(--surface, #fff);
  }
  .chips::-webkit-scrollbar {
    display: none;
  }

  :global(.chip) {
    flex-shrink: 0;
    padding: 0 16px;
    min-height: 36px;
    border: 1.5px solid var(--border, #c8d8ce);
    border-radius: 18px;
    background: var(--surface, #fff);
    color: var(--text2, #2d4a38);
    font-size: 0.82rem;
    font-weight: 600;
    cursor: pointer;
    white-space: nowrap;
    transition:
      background 0.15s,
      border-color 0.15s,
      color 0.15s,
      transform 0.1s;
    -webkit-tap-highlight-color: transparent;
  }

  :global(.chip:active) {
    transform: scale(0.95);
  }

  :global(.chip--on) {
    background: var(--p700, #065f46);
    border-color: var(--p700, #065f46);
    color: #fff;
  }
</style>
