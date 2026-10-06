<!-- Three-node graph — activity bar graph view. -->
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
    <circle cx="7.5" cy="2" r="1.5" />
    <circle cx="2" cy="12" r="1.5" />
    <circle cx="13" cy="12" r="1.5" />
    <line x1="7.5" y1="3.5" x2="2.7" y2="10.5" />
    <line x1="7.5" y1="3.5" x2="12.3" y2="10.5" />
    <line x1="3.5" y1="12" x2="11.5" y2="12" />
</svg>
