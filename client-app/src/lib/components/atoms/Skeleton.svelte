<script lang="ts">
  interface Props {
    width?: string;
    height?: string;
    radius?: string;
    class?: string;
  }
  let {
    width = "100%",
    height = "1rem",
    radius = "6px",
    class: extraClass = "",
  }: Props = $props();
</script>

<span
  class="skeleton {extraClass}"
  style="width:{width};height:{height};border-radius:{radius}"
  aria-hidden="true"
></span>

<style>
  .skeleton {
    display: inline-block;
    background: linear-gradient(90deg, #e8efe9 25%, #d4e3d6 50%, #e8efe9 75%);
    background-size: 200% 100%;
    animation: shimmer 1.4s ease-in-out infinite;
  }
  @keyframes shimmer {
    0% {
      background-position: 200% 0;
    }
    100% {
      background-position: -200% 0;
    }
  }
</style>
