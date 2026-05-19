<script lang="ts">
  import Button from "$lib/components/atoms/Button.svelte";
  import { ROUTES } from "$lib/routes";

  interface Props {
    title?: string;
    sub?: string;
    btn?: string;
  }
  const {
    title = "Tìm thợ gần bạn ngay",
    sub = "Cơ khí · Phun thuốc · Múc bùn — trong 10 km",
    btn = "Bật định vị",
  }: Props = $props();
</script>

<Button
  href={ROUTES.AGRI_CONNECT}
  class="geo-cta"
  aria-label="Tìm dịch vụ gần tôi"
>
  <span class="ripple r1" aria-hidden="true"></span>
  <span class="ripple r2" aria-hidden="true"></span>
  <span class="ripple r3" aria-hidden="true"></span>

  <div class="geo-cta__body">
    <span class="geo-cta__pin">
      <i class="fa-solid fa-location-dot"></i>
    </span>
    <div class="geo-cta__text">
      <strong class="geo-cta__title">{title}</strong>
      <span class="geo-cta__sub">{sub}</span>
    </div>
  </div>
  <span class="geo-cta__btn">
    {btn} <i class="fa-solid fa-arrow-right"></i>
  </span>
</Button>

<style>
  :global(.geo-cta) {
    position: relative;
    display: flex !important;
    width: 100%;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 20px;
    background: linear-gradient(135deg, #022c22 0%, #047857 100%);
    border-radius: 20px;
    text-decoration: none;
    overflow: hidden;
    -webkit-tap-highlight-color: transparent;
    transition:
      transform 0.15s,
      box-shadow 0.15s;
  }
  :global(.geo-cta:active) {
    transform: scale(0.98);
  }

  .ripple {
    position: absolute;
    border-radius: 50%;
    border: 2px solid rgba(52, 211, 153, 0.35);
    top: 50%;
    left: 28px;
    transform: translate(-50%, -50%);
    animation: geo-pulse 3s ease-out infinite;
    pointer-events: none;
  }
  .r1 {
    width: 60px;
    height: 60px;
    animation-delay: 0s;
  }
  .r2 {
    width: 90px;
    height: 90px;
    animation-delay: 0.8s;
  }
  .r3 {
    width: 120px;
    height: 120px;
    animation-delay: 1.6s;
  }
  @keyframes geo-pulse {
    0% {
      opacity: 0.9;
      transform: translate(-50%, -50%) scale(0.7);
    }
    100% {
      opacity: 0;
      transform: translate(-50%, -50%) scale(1.1);
    }
  }

  .geo-cta__body {
    display: flex;
    align-items: center;
    gap: 14px;
    z-index: 1;
    min-width: 0;
  }
  .geo-cta__pin {
    width: 52px;
    height: 52px;
    flex-shrink: 0;
    background: rgba(255, 255, 255, 0.12);
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1.6rem;
    color: #6ee7b7;
    z-index: 1;
    backdrop-filter: blur(4px);
  }
  .geo-cta__text {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .geo-cta__title {
    font-size: 1rem;
    font-weight: 800;
    color: #fff;
    white-space: nowrap;
  }
  .geo-cta__sub {
    font-size: 0.72rem;
    color: rgba(255, 255, 255, 0.65);
    line-height: 1.4;
  }
  .geo-cta__btn {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 10px 14px;
    background: #f59e0b;
    color: #1c1917;
    font-size: 0.8rem;
    font-weight: 800;
    border-radius: 100px;
    white-space: nowrap;
    z-index: 1;
    letter-spacing: 0.02em;
  }
</style>
