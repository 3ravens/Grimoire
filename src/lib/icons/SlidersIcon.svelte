<!-- Vertical sliders — chat context options. -->
<script>
    /** @type {{ class?: string, size?: number }} */
    let { class: className = "", size = 13 } = $props();
</script>

<svg
    width={size}
    height={size}
    class={className}
    viewBox="0 0 15 15"
    fill="none"
    stroke="currentColor"
    stroke-width="1.5"
    stroke-linecap="round"
    aria-hidden="true"
>
    <line x1="3" y1="3" x2="3" y2="12" />
    <line x1="8" y1="3" x2="8" y2="12" />
    <line x1="13" y1="3" x2="13" y2="12" />
    <rect x="1" y="5.5" width="4" height="2" rx="1" />
    <rect x="6" y="8.5" width="4" height="2" rx="1" />
    <rect x="11" y="4" width="4" height="2" rx="1" />
</svg>
