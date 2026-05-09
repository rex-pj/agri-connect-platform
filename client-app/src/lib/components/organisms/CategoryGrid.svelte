<script lang="ts">
  import CategoryItem from '$lib/components/molecules/CategoryItem.svelte';

  export interface CategoryGridItem {
    name: string;
    icon: string;  // Font Awesome solid class, e.g. "fa-water"
    href?: string;
    active?: boolean;
    onSelect?: () => void;
  }

  const { items }: { items: CategoryGridItem[] } = $props();
</script>

<ul class="cat-grid">
  {#each items as item}
    <li class="cat-item-wrapper">
      <CategoryItem
        name={item.name}
        icon={item.icon}
        href={item.href}
        active={item.active}
        onSelect={item.onSelect}
      />
    </li>
  {/each}
</ul>

<style>
  .cat-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(70px, 1fr));
    gap: 12px;
    margin: 0;
    padding: 0;
    list-style: none;
    justify-items: center;
  }
  .cat-grid::-webkit-scrollbar {
    display: none;
  }

  .cat-item-wrapper {
    flex-shrink: 0;
  }
</style>
