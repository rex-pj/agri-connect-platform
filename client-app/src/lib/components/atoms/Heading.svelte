<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    level: 1 | 2 | 3 | 4 | 5 | 6;
    class?: string;
    children: Snippet;
    [key: string]: unknown;
  }

  let { level, class: extraClass = "", children, ...rest }: Props = $props();
</script>

{#if level === 1}
  <h1 class="h1 {extraClass}" {...rest}>{@render children()}</h1>
{:else if level === 2}
  <h2 class="h2 {extraClass}" {...rest}>{@render children()}</h2>
{:else if level === 3}
  <h3 class="h3 {extraClass}" {...rest}>{@render children()}</h3>
{:else if level === 4}
  <h4 class="h4 {extraClass}" {...rest}>{@render children()}</h4>
{:else if level === 5}
  <h5 class="h5 {extraClass}" {...rest}>{@render children()}</h5>
{:else}
  <h6 class="h6 {extraClass}" {...rest}>{@render children()}</h6>
{/if}

<style>
  .h1,
  .h2,
  .h3,
  .h4,
  .h5,
  .h6 {
    margin: 0;
    line-height: 1.25;
  }
</style>
