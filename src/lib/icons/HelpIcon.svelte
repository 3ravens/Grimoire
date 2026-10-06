<!-- Question mark in circle — activity bar help. -->
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
    <circle cx="7.5" cy="7.5" r="6" />
    <path d="M5.5 5.5a2 2 0 1 1 2 2c0 1-0.5 1.5-0.5 2" />
    <circle cx="7.5" cy="11.5" r="0.6" fill="currentColor" stroke="none" />
</svg>
