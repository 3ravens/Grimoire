<!-- Pane with divider — titlebar toggle folders. -->
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
    <rect x="1" y="1" width="13" height="13" rx="1" />
    <line x1="5" y1="1" x2="5" y2="14" />
</svg>
