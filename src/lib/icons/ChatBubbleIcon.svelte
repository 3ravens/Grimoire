<!-- Chat bubble — titlebar toggle chat. -->
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
    <path d="M2 2h11a1 1 0 0 1 1 1v7a1 1 0 0 1-1 1H5l-3 3V3a1 1 0 0 1 1-1z" />
</svg>
