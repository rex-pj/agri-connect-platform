<script lang="ts">
  import CategoryItem from "$lib/components/molecules/CategoryItem.svelte";

  export interface CategoryGridItem {
    name: string;
    icon: string; // Font Awesome solid class, e.g. "fa-water"
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
    grid-template-columns: repeat(4, 1fr);
    gap: 4px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  @media (min-width: 480px) {
    .cat-grid {
      grid-template-columns: repeat(5, 1fr);
    }
  }

  @media (min-width: 768px) {
    .cat-grid {
      grid-template-columns: repeat(auto-fit, minmax(88px, 1fr));
      gap: 6px;
    }
  }
</style>
