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
    <span class="cat-label" class:cat-label--active={active}>{name}</span>
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
    <span class="cat-label" class:cat-label--active={active}>{name}</span>
  </Button>
{/if}

<style>
  :global(.btn.cat-item) {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 8px 4px;
    box-shadow: none;
    white-space: normal;
    border-radius: 12px;
    background: transparent;
    border: none;
  }

  :global(.btn.cat-item:hover) {
    background: var(--p50, #ecfdf5);
  }

  .cat-icon {
    width: 56px;
    height: 56px;
    background: var(--p100, #d1fae5);
    border-radius: 16px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1.5rem;
    color: var(--p700, #065f46);
    box-shadow: 0 1px 4px rgba(4, 120, 87, 0.15);
    border: 2px solid var(--p300, #6ee7b7);
    transition:
      transform 0.15s ease,
      background 0.15s ease,
      color 0.15s ease,
      box-shadow 0.15s ease,
      border-color 0.15s ease;
    flex-shrink: 0;
  }

  :global(.btn.cat-item:hover) .cat-icon {
    background: var(--p200, #a7f3d0);
    border-color: var(--p400, #34d399);
    transform: translateY(-2px);
    box-shadow: 0 4px 10px rgba(4, 120, 87, 0.20);
  }

  :global(.btn.cat-item:active) .cat-icon {
    transform: scale(0.92);
    box-shadow: none;
  }

  :global(.cat-item:focus-visible) .cat-icon {
    box-shadow: 0 0 0 3px var(--p400, #34d399);
  }

  .cat-icon--active {
    background: var(--p700, #065f46);
    color: #fff;
    border-color: var(--p700, #065f46);
    box-shadow: 0 3px 10px rgba(4, 120, 87, 0.30);
  }

  :global(.btn.cat-item:hover) .cat-icon--active {
    background: var(--p800, #064e3b);
    border-color: var(--p800, #064e3b);
    transform: translateY(-2px);
  }

  .cat-label {
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--text-muted, #6b7280);
    text-align: center;
    line-height: 1.25;
    white-space: normal;
    word-break: break-word;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    width: 100%;
    transition: color 0.15s ease;
  }

  :global(.btn.cat-item:hover) .cat-label {
    color: var(--p700, #065f46);
  }

  .cat-label--active {
    color: var(--p700, #065f46);
    font-weight: 700;
  }
</style>
