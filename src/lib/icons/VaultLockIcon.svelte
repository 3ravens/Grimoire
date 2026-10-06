<!-- Padlock with keyhole — activity bar lock vault (distinct from LockClosedIcon). -->
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
    <rect x="3" y="7" width="9" height="7" rx="1" />
    <path d="M5 7V5a2.5 2.5 0 0 1 5 0v2" />
    <circle cx="7.5" cy="10.5" r="0.75" fill="currentColor" stroke="none" />
</svg>
