<!-- Expand corners — titlebar enter focus mode. -->
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
    <polyline points="1,6 1,1 6,1" />
    <polyline points="9,14 14,14 14,9" />
    <line x1="1" y1="1" x2="6" y2="6" />
    <line x1="14" y1="14" x2="9" y2="9" />
</svg>
