<!-- Bug — activity bar report bug. -->
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
    <path d="M7.5 2.5v-1M5.2 3.2L4.5 2M9.8 3.2l.7-.8" />
    <ellipse cx="7.5" cy="8.5" rx="2.3" ry="3.3" />
    <path d="M5.2 6.2L3 5.5M9.8 6.2L12 5.5M5 8.5H2.5M10 8.5h2.5M5.2 10.8L3 11.5M9.8 10.8L12 11.5" />
</svg>
