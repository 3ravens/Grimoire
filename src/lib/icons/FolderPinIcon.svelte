<!-- Small pin marker for folder-pinned notes in the note list. -->
<script>
    /** @type {{ class?: string, size?: number }} */
    let { class: className = "", size = 12 } = $props();
</script>

<svg
    width={size}
    height={size}
    class={className}
    viewBox="0 0 12 12"
    fill="currentColor"
    aria-hidden="true"
>
    <!-- Head and needle both centered on x=6 -->
    <path
        d="M6 1C4.6 1 3.5 2.1 3.5 3.5c0 1 .6 1.9 1.4 2.3V6.5H4.2c-.3 0-.5.2-.5.5v.5h4.6v-.5c0-.3-.2-.5-.5-.5H7.1V5.8C7.9 5.4 8.5 4.5 8.5 3.5 8.5 2.1 7.4 1 6 1zm-.4 6.5h.8v3.7c0 .2-.2.4-.4.4s-.4-.2-.4-.4V7.5z"
    />
</svg>
