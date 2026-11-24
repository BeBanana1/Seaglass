<script lang="ts">
  // Banner of the website demo (#28, `__DEMO__` builds only): says that the catalog is
  // invented and that nothing plays, with a link to the download page.
  import { t } from "../lib/i18n.svelte";
</script>

<aside class="demo-banner">
  <strong>{t.common.demo.label}</strong>
  <span>{t.common.demo.text}</span>
  <a href="/download/" target="_top">{t.common.demo.download}</a>
</aside>

<style>
  .demo-banner {
    position: fixed;
    right: 16px;
    bottom: calc(var(--player-height) + 16px);
    z-index: 50;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 4px 6px 4px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    background: var(--dialog-bg);
    color: var(--text-muted);
    font: 12px/1.4 var(--font-ui);
    box-shadow: var(--popover-shadow);
    white-space: nowrap;
  }

  strong {
    color: var(--text);
  }

  a {
    padding: 2px 10px;
    border-radius: var(--radius-pill);
    background: var(--accent);
    color: var(--accent-contrast);
    text-decoration: none;
    font-weight: 600;
  }

  @media (max-width: 640px) {
    span {
      display: none;
    }
  }
</style>
