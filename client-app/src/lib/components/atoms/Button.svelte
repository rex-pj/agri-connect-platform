<script lang="ts">
  import type { Snippet } from "svelte";
  import type { ClassValue } from "svelte/elements";

  interface Props {
    href?: string;
    type?: "button" | "submit" | "reset";
    variant?: string;
    size?: "sm" | "lg";
    class?: ClassValue;
    disabled?: boolean;
    onclick?: (e: MouseEvent) => void;
    children: Snippet;
    [key: string]: unknown;
  }

  let {
    href,
    type = "button",
    variant,
    size,
    class: extraClass = "",
    disabled = false,
    onclick,
    children,
    ...rest
  }: Props = $props();

  const cls = $derived([
    "btn",
    variant ? `btn-${variant}` : "",
    size ? `btn-${size}` : "",
    extraClass,
  ].filter(Boolean).join(" "));
</script>

{#if href}
  <a
    {href}
    class={cls}
    class:disabled
    aria-disabled={disabled || undefined}
    onclick={(e) => (disabled ? e.preventDefault() : onclick?.(e))}
    {...rest}
  >
    {@render children()}
  </a>
{:else}
  <button {type} class={cls} {disabled} {onclick} {...rest}>
    {@render children()}
  </button>
{/if}

<style>
  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-weight: 600;
    font-size: 0.9375rem;
    padding: 0.625rem 1.25rem;
    border-radius: 0.75rem;
    transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    cursor: pointer;
    border: 1px solid transparent;
    gap: 0.5rem;
    text-decoration: none;
    line-height: 1.25;
    user-select: none;
    -webkit-tap-highlight-color: transparent;
    color: var(--text, #0c1a12);
    background: transparent;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
    pointer-events: none;
  }

  .btn-primary {
    background-color: var(--p600, #047857);
    color: white;
  }
  .btn-primary:hover {
    background-color: var(--p700, #065f46);
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(4, 120, 87, 0.25);
  }

  .btn-secondary {
    background-color: var(--s100, #f0fdf4);
    color: var(--p700, #065f46);
    border-color: var(--p200, #bbf7d0);
  }
  .btn-secondary:hover {
    background-color: var(--s200, #dcfce7);
  }

  .btn-outline {
    background-color: transparent;
    border-color: var(--border, #e5e7eb);
    color: var(--text, #0c1a12);
  }
  .btn-outline:hover {
    border-color: var(--p600, #047857);
    color: var(--p600, #047857);
    background-color: var(--p50, #f0fdf4);
  }

  .btn-danger {
    background-color: #fef2f2;
    color: #dc2626;
    border-color: #fecaca;
  }
  .btn-danger:hover {
    background-color: #fee2e2;
    border-color: #f9a8d4;
  }

  .btn-ghost {
    background: transparent;
    border-color: transparent;
    padding-left: 0.5rem;
    padding-right: 0.5rem;
  }
  .btn-ghost:hover {
    background-color: rgba(0, 0, 0, 0.05);
  }

  .btn-sm {
    padding: 0.375rem 0.75rem;
    font-size: 0.8125rem;
    border-radius: 0.5rem;
  }

  .btn-lg {
    padding: 0.75rem 1.5rem;
    font-size: 1.0625rem;
    border-radius: 0.875rem;
  }
</style>
