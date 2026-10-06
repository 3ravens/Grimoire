<!-- Tray with down arrow — import file as note. -->
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
    <!-- Open tray (lower half) -->
    <path d="M2.5 8 V12.5 H12.5 V8" />
    <!-- Large down arrow into tray -->
    <line x1="7.5" y1="1.75" x2="7.5" y2="9.25" />
    <polyline points="4.5,6.25 7.5,9.25 10.5,6.25" />
</svg>
