<!-- Calendar with plus — activity bar new daily note. -->
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
    <rect x="1.5" y="2.5" width="12" height="11" rx="1" />
    <line x1="1.5" y1="6" x2="13.5" y2="6" />
    <line x1="5" y1="1.5" x2="5" y2="3.5" />
    <line x1="10" y1="1.5" x2="10" y2="3.5" />
    <line x1="7.5" y1="8.5" x2="7.5" y2="11.5" />
    <line x1="6" y1="10" x2="9" y2="10" />
</svg>
