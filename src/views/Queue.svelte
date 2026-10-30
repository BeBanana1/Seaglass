<script lang="ts">
  import QueueList from "../components/QueueList.svelte";
</script>

<aside>
  <h2>File d'attente</h2>
  <QueueList />
</aside>

<style>
  aside {
    height: 100%;
    overflow: auto;
    padding: 16px 12px;
    border-left: 1px solid var(--border);
    background: var(--bg);
  }

  h2 {
    margin: 0 8px 12px;
    font-size: 15px;
    font-weight: 600;
  }
</style>
