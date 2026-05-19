<script lang="ts">
  import Skeleton from "$lib/components/atoms/Skeleton.svelte";

  interface Props {
    count?: number;
    type?: "provider" | "product";
  }
  let { count = 3, type = "provider" }: Props = $props();
</script>

{#each Array(count) as _, i (i)}
  {#if type === "provider"}
    <div class="card skel-card">
      <div class="card-body d-flex flex-column p-3 gap-2">
        <div class="d-flex align-items-start gap-2">
          <Skeleton width="60%" height="1.1rem" radius="8px" />
        </div>
        <Skeleton width="80%" height="0.85rem" radius="6px" />
        <div class="d-flex gap-1 mt-1">
          <Skeleton width="56px" height="22px" radius="9999px" />
          <Skeleton width="72px" height="22px" radius="9999px" />
        </div>
        <div class="mt-auto pt-3 border-top">
          <div class="d-flex gap-2">
            <Skeleton width="100%" height="44px" radius="12px" />
            <Skeleton width="100%" height="44px" radius="12px" />
          </div>
        </div>
      </div>
    </div>
  {:else}
    <div class="card skel-card">
      <Skeleton
        width="100%"
        height="0"
        class="ratio ratio-1x1 skel-img"
        radius="0"
      />
      <div class="card-body d-flex flex-column p-3 gap-2">
        <Skeleton width="90%" height="1rem" radius="6px" />
        <Skeleton width="60%" height="1rem" radius="6px" />
        <Skeleton width="40%" height="0.8rem" radius="6px" />
        <Skeleton width="100%" height="40px" radius="10px" class="mt-1" />
      </div>
    </div>
  {/if}
{/each}

<style>
  .skel-card {
    overflow: hidden;
  }
  :global(.skel-img) {
    aspect-ratio: 1;
    height: auto !important;
    display: block;
  }
</style>
