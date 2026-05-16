<!-- Closed padlock — matches folder re-lock control. -->
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
    <rect x="2" y="7" width="11" height="7" rx="1" />
    <path d="M5 7V4.5a2.5 2.5 0 0 1 5 0V7" />
</svg>
