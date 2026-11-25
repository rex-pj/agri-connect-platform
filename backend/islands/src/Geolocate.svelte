<!--
  <ac-geolocate href="/explore?radius=10000" label="…" busy-label="…"
                denied="…" failed="…" unsupported="…" button-class="…">

  Asks the browser for the position and goes to `href` with lat and lng
  added. With `lat-target` and `lng-target` (ids of two inputs) it fills
  those boxes instead and stays on the page, showing `done`: the provider
  profile form uses this to set its location. With `max-accuracy` (metres)
  a position less precise than that is refused with `inaccurate`, whose
  `{m}` is the error the browser gave. All text comes from the template (Vietnamese lives there, not here).
  Without JS the tag renders nothing, so pages always offer a form next to it
  (a location preset) that works on its own.

  Light DOM (shadow: "none") and no <style>: the page CSP is style-src 'self',
  which would block a style element; classes come from app.css.
-->
<svelte:options
  customElement={{
    tag: "ac-geolocate",
    shadow: "none",
    props: {
      href: { type: "String" },
      label: { type: "String" },
      busyLabel: { type: "String", attribute: "busy-label" },
      denied: { type: "String" },
      failed: { type: "String" },
      unsupported: { type: "String" },
      buttonClass: { type: "String", attribute: "button-class" },
      latTarget: { type: "String", attribute: "lat-target" },
      lngTarget: { type: "String", attribute: "lng-target" },
      done: { type: "String" },
      maxAccuracy: { type: "Number", attribute: "max-accuracy" },
      inaccurate: { type: "String" },
    },
  }}
/>

<script>
  /** Coordinates kept in the URL: ~10 cm, more than any phone GPS gives. */
  const DECIMALS = 6;
  const TIMEOUT_MS = 10_000;
  const PERMISSION_DENIED = 1;

  let {
    href = "",
    label = "",
    busyLabel = "",
    denied = "",
    failed = "",
    unsupported = "",
    buttonClass = "btn btn-success",
    latTarget = "",
    lngTarget = "",
    done = "",
    maxAccuracy = 0,
    inaccurate = "",
  } = $props();

  let busy = $state(false);
  let error = $state("");
  let filled = $state(false);

  /** Puts `value` into the input with id `id`, as if typed. */
  function fill(id, value) {
    const input = document.getElementById(id);
    if (!input) return;
    input.value = value;
    input.dispatchEvent(new Event("input", { bubbles: true }));
  }

  function locate() {
    if (!navigator.geolocation) {
      error = unsupported;
      return;
    }
    busy = true;
    error = "";
    filled = false;
    navigator.geolocation.getCurrentPosition(
      ({ coords }) => {
        // A position this rough (a computer placed by its internet address)
        // would put the business in the wrong commune: say so, keep nothing.
        if (maxAccuracy > 0 && coords.accuracy > maxAccuracy) {
          busy = false;
          error = inaccurate.replace("{m}", String(Math.round(coords.accuracy)));
          return;
        }
        if (latTarget && lngTarget) {
          fill(latTarget, coords.latitude.toFixed(DECIMALS));
          fill(lngTarget, coords.longitude.toFixed(DECIMALS));
          busy = false;
          filled = true;
          return;
        }
        const target = new URL(href, window.location.href);
        target.searchParams.set("lat", coords.latitude.toFixed(DECIMALS));
        target.searchParams.set("lng", coords.longitude.toFixed(DECIMALS));
        // A chosen preset no longer applies once we have a real position.
        target.searchParams.delete("location_id");
        window.location.assign(target.toString());
      },
      (err) => {
        busy = false;
        error = err.code === PERMISSION_DENIED ? denied : failed;
      },
      { timeout: TIMEOUT_MS, enableHighAccuracy: true },
    );
  }
</script>

<button type="button" class={buttonClass} onclick={locate} disabled={busy} aria-busy={busy}>
  {#if busy}
    <span class="spinner-border spinner-border-sm" aria-hidden="true"></span>
    {busyLabel}
  {:else}
    <i class="fa-solid fa-crosshairs" aria-hidden="true"></i>
    {label}
  {/if}
</button>
{#if error}
  <p class="geo-error" role="alert">
    <i class="fa-solid fa-triangle-exclamation" aria-hidden="true"></i>
    {error}
  </p>
{/if}
{#if filled && done}
  <p class="small text-success mt-1 mb-0" role="status">
    <i class="fa-solid fa-circle-check" aria-hidden="true"></i>
    {done}
  </p>
{/if}
