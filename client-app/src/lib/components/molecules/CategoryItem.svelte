<script lang="ts">
  import Button from "$lib/components/atoms/Button.svelte";

  interface Props {
    name: string;
    icon: string;
    href?: string;
    active?: boolean;
    onSelect?: () => void;
  }
  let { name, icon, href, active = false, onSelect }: Props = $props();
</script>

{#if href}
  <Button {href} class="cat-item" aria-current={active ? "page" : undefined}>
    <span class="cat-icon" class:cat-icon--active={active}>
      <i class="fa-solid {icon}"></i>
    </span>
    <span class="cat-label">{name}</span>
  </Button>
{:else}
  <Button
    type="button"
    class="cat-item"
    onclick={onSelect}
    aria-pressed={active}
  >
    <span class="cat-icon" class:cat-icon--active={active}>
      <i class="fa-solid {icon}"></i>
    </span>
    <span class="cat-label">{name}</span>
  </Button>
{/if}

<style>
  :global(.btn.cat-item) {
    display: flex !important;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    width: 72px;
    box-shadow: none !important;
  }

  .cat-icon {
    width: 60px;
    height: 60px;
    background: var(--p50, #ecfdf5);
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1.4rem;
    color: var(--p700, #065f46);
    box-shadow: 0 2px 8px rgba(4, 120, 87, 0.12);
    border: 2px solid rgba(4, 120, 87, 0.15);
    transition: all 0.15s;
    flex-shrink: 0;
  }

  :global(.cat-item:active) .cat-icon {
    transform: scale(0.93);
  }

  :global(.cat-item:focus-visible) .cat-icon {
    box-shadow: 0 0 0 3px var(--p400, #34d399);
  }

  .cat-icon--active {
    background: var(--p700, #065f46);
    color: #fff;
    border-color: var(--p700, #065f46);
    box-shadow: 0 4px 14px rgba(4, 120, 87, 0.35);
  }

  .cat-label {
    font-size: 0.72rem;
    font-weight: 700;
    color: #374151;
    text-align: center;
    line-height: 1.25;
    white-space: normal;
    word-break: break-word;
  }
</style>
