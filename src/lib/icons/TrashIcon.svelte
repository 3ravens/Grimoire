<!-- Trash can — chat clear conversation. -->
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
    <path
        d="M2 4h11M5 4V2.5a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1V4M6 7v4M9 7v4M3 4l.75 8.5a1 1 0 0 0 1 .9h5.5a1 1 0 0 0 1-.9L12 4"
    />
</svg>
