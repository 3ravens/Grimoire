<!-- Document with lines — activity bar Wikipedia. -->
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
    stroke-linejoin="round"
    aria-hidden="true"
>
    <rect x="1.5" y="1.5" width="12" height="12" rx="1" />
    <line x1="4" y1="4.5" x2="11" y2="4.5" />
    <line x1="4" y1="7.5" x2="11" y2="7.5" />
    <line x1="4" y1="10.5" x2="8" y2="10.5" />
</svg>
