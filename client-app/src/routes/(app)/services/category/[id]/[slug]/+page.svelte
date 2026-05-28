<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import ProviderCard from "$lib/components/organisms/ProviderCard.svelte";
  import CategoryGrid from "$lib/components/organisms/CategoryGrid.svelte";
  import GeoCta from "$lib/components/molecules/GeoCta.svelte";
  import ContactSheet from "$lib/components/organisms/ContactSheet.svelte";
  import PageHero from "$lib/components/molecules/PageHero.svelte";
  import AlertBanner from "$lib/components/molecules/AlertBanner.svelte";
  import EmptyState from "$lib/components/molecules/EmptyState.svelte";
  import PaginationNav from "$lib/components/molecules/PaginationNav.svelte";
  import { SERVICE_DISPLAY, DEFAULT_DISPLAY } from "$lib/categoryIcons";
  import { ROUTES } from "$lib/routes";
  import Text from "$lib/components/atoms/Text.svelte";
  import type { PageData, ActionData } from "./$types";
  import type { BrowseProvider } from "./+page.server";

  const { data, form }: { data: PageData; form: ActionData } = $props();

  const catDisplay = $derived(
    data.currentCategory ? (SERVICE_DISPLAY[data.currentCategory.slug] ?? DEFAULT_DISPLAY) : DEFAULT_DISPLAY
  );

  let activeProvider = $state<BrowseProvider | null>(null);
  let contactSuccess = $state(false);

  function goToPage(p: number) {
    const params = new URLSearchParams(page.url.searchParams);
    params.set("page", String(p));
    goto(`?${params.toString()}`, { replaceState: false });
  }

  function openContact(p: BrowseProvider) {
    activeProvider = p;
    contactSuccess = false;
  }

  $effect(() => {
    if (form?.success) {
      contactSuccess = true;
      activeProvider = null;
    }
  });
</script>

<svelte:head>
  <title
    >{data.currentCategory?.name ?? "Danh mục dịch vụ"} — Agri-Connect</title
  >
  <meta
    name="description"
    content="Tìm nhà cung cấp dịch vụ {data.currentCategory?.name ??
      'nông nghiệp'} uy tín tại Đồng bằng sông Cửu Long. Kết nối trực tiếp, không qua trung gian."
  />
</svelte:head>

<PageHero
  title={data.currentCategory?.name ?? "Danh mục dịch vụ"}
  icon={catDisplay.icon}
  sub={catDisplay.desc ? `${catDisplay.desc}${data.result.total > 0 ? ` · ${data.result.total} nhà cung cấp` : ''}` : undefined}
  backLink={{ href: ROUTES.SERVICES, label: "Dịch vụ" }}
/>

{#if data.categories.length > 0}
  <div class="px-3 pt-3">
    <CategoryGrid
      items={data.categories.map((cat) => {
        const d = SERVICE_DISPLAY[cat.slug] ?? DEFAULT_DISPLAY;
        const isActive = cat.id === data.currentCategory?.id;
        return {
          name: cat.name,
          icon: d.icon,
          active: isActive,
          href: isActive
            ? undefined
            : `/services/category/${cat.id}/${cat.slug}`,
          onSelect: isActive ? () => {} : undefined,
        };
      })}
    />
  </div>
{/if}

<div class="px-3 pt-3">
  <GeoCta
    title="Tìm nhà cung cấp gần bạn"
    sub="Trong 10 km — bật định vị để lọc theo khoảng cách"
    btn="Bật định vị"
  />
</div>

<section class="p-3">
  {#if data.error}
    <AlertBanner type="error" message={data.error} />
  {/if}
  {#if contactSuccess}
    <AlertBanner
      type="success"
      message="Đã gửi yêu cầu! Nhà cung cấp sẽ liên hệ lại với bạn sớm."
    />
  {/if}

  {#if data.result.items.length === 0 && !data.error}
    <EmptyState
      icon="fa-inbox"
      title="Chưa có nhà cung cấp nào trong danh mục này"
      hint="Hãy thử chọn danh mục khác hoặc xem tất cả dịch vụ."
    />
  {:else}
    <Text class="small text-muted mb-3"
      ><strong>{data.result.total}</strong> nhà cung cấp</Text
    >

    <div class="row row-cols-1 row-cols-md-2 row-cols-lg-3 g-3">
      {#each data.result.items as provider (provider.id)}
        <div class="col">
          <ProviderCard {provider} onContact={() => openContact(provider)} />
        </div>
      {/each}
    </div>

    {#if data.result.total_pages > 1}
      <PaginationNav
        currentPage={data.result.page}
        totalPages={data.result.total_pages}
        onPageChange={goToPage}
      />
    {/if}
  {/if}
</section>

{#if activeProvider}
  <ContactSheet
    provider={activeProvider}
    {form}
    onClose={() => (activeProvider = null)}
  />
{/if}
