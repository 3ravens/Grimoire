<!-- List with magnifier — activity bar quick switcher. -->
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
    <line x1="2" y1="4" x2="8" y2="4" />
    <line x1="2" y1="7.5" x2="6.5" y2="7.5" />
    <line x1="2" y1="11" x2="5.5" y2="11" />
    <circle cx="11" cy="10" r="2.5" />
    <line x1="12.8" y1="11.8" x2="14" y2="13" />
</svg>
