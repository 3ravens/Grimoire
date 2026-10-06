<!-- Simple folder + plus beside it — new folder. -->
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
    <!-- Wide folder with clear top tab (not a tall document) -->
    <path
        d="M1.25 5.25 A0.75 0.75 0 0 1 2 4.5 H4.5 L5.75 5.75 H8.5 A0.75 0.75 0 0 1 9.25 6.5 V11.25 A0.75 0.75 0 0 1 8.5 12 H2 A0.75 0.75 0 0 1 1.25 11.25 Z"
    />
    <!-- Plus to the right of the folder -->
    <line x1="12.25" y1="5.5" x2="12.25" y2="10.5" />
    <line x1="10.5" y1="8" x2="14" y2="8" />
</svg>
