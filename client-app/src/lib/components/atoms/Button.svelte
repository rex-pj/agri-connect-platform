<script lang="ts">
  import type { Snippet } from "svelte";
  import type { ClassValue } from "svelte/elements";

  interface Props {
    href?: string;
    type?: "button" | "submit" | "reset";
    class?: ClassValue;
    disabled?: boolean;
    onclick?: (e: MouseEvent) => void;
    children: Snippet;
    [key: string]: unknown;
  }

  let {
    href,
    type = "button",
    class: extraClass = "",
    disabled = false,
    onclick,
    children,
    ...rest
  }: Props = $props();

  const cls = ["btn", extraClass].filter(Boolean).join(" ");
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
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
    pointer-events: none;
  }
</style>
