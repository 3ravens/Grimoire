<!-- Pencil / edit — folder new-note and rename. -->
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
    <path d="M11.5 1.5 L13.5 3.5 L5 12 L2 12.5 L2.5 9.5 Z" />
    <line x1="9.5" y1="3.5" x2="11.5" y2="5.5" />
</svg>
