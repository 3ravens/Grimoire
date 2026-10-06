<!-- Magnifying glass — activity bar search. -->
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
    <circle cx="6.5" cy="6.5" r="4.5" />
    <line x1="10" y1="10" x2="13.5" y2="13.5" />
</svg>
