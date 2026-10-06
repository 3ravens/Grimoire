<!-- Three horizontal lines — titlebar toggle notes list. -->
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
    <line x1="4" y1="4" x2="11" y2="4" />
    <line x1="4" y1="7.5" x2="11" y2="7.5" />
    <line x1="4" y1="11" x2="9" y2="11" />
</svg>
