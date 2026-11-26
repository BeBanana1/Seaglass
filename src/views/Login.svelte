<script lang="ts">
  import { commands, type SessionInfo } from "../lib/ipc";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { t } from "../lib/i18n.svelte";

  // `restoreError`: the saved session could not be resumed (no network at startup, transient
  // outage); it is not lost and the app retries on its own (#252).
  let {
    onLogin,
    restoreError = null,
  }: { onLogin: (s: SessionInfo) => void; restoreError?: string | null } = $props();

  let pending = $state(false);
  let error = $state<string | null>(null);
  // Linux (#17): sign-in in the system browser, then the return address pasted here.
  let inBrowser = $state(false);
  let loginUrl = $state<string | null>(null);
  let copied = $state(false);
  let redirect = $state("");

  commands.loginInBrowser().then(
    (v) => (inBrowser = v),
    () => {},
  );

  async function run(action: () => Promise<void>) {
    pending = true;
    error = null;
    try {
      await action();
    } catch (e) {
      error = String(e);
    } finally {
      pending = false;
    }
  }

  const login = () => run(async () => onLogin(await commands.login()));

  const openBrowser = () =>
    run(async () => {
      loginUrl = await commands.browserLoginStart();
      copied = false;
    });

  // Fallback when no browser opens: the address can be copied into any of them.
  async function copyLoginUrl(input: HTMLInputElement) {
    input.select();
    try {
      await navigator.clipboard.writeText(input.value);
      copied = true;
    } catch {
      // Text selected: Ctrl+C still works.
    }
  }

  let urlInput = $state<HTMLInputElement>();

  // #38: the published builds carry TIDAL's client identifier; the README explains it.
  async function openClientNotice() {
    try {
      await commands.openProjectPage("clientId");
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }

  const finishBrowser = () => run(async () => onLogin(await commands.browserLoginFinish(redirect)));
</script>

<section>
  <h1>{t.views.login.welcome}</h1>
  <p>{t.views.login.intro}</p>
  {#if !inBrowser}
    <button class="primary" onclick={login} disabled={pending}>
      {pending ? t.views.login.pending : t.views.login.signIn}
    </button>
  {:else if loginUrl === null}
    <button class="primary" onclick={openBrowser} disabled={pending}>
      {t.views.login.signInBrowser}
    </button>
  {:else}
    <p class="steps">{t.views.login.browserFallback}</p>
    <div class="redirect">
      <input
        bind:this={urlInput}
        type="text"
        readonly
        value={loginUrl}
        aria-label={t.views.login.loginUrlLabel}
      />
      <button class="secondary" onclick={() => urlInput && copyLoginUrl(urlInput)}>
        {copied ? t.views.login.copied : t.views.login.copy}
      </button>
    </div>
    <p class="steps">{t.views.login.browserSteps}</p>
    <form
      class="redirect"
      onsubmit={(e) => {
        e.preventDefault();
        finishBrowser();
      }}
    >
      <input
        type="url"
        bind:value={redirect}
        placeholder="https://tidal.com/android/login/auth?code=…"
        aria-label={t.views.login.redirectLabel}
        autocomplete="off"
        spellcheck="false"
      />
      <button class="primary" type="submit" disabled={pending || redirect.trim() === ""}>
        {pending ? t.views.login.pending : t.views.login.finish}
      </button>
    </form>
    <button class="link" onclick={openBrowser} disabled={pending}>
      {t.views.login.reopenBrowser}
    </button>
  {/if}
  {#if error}
    <p class="error">{error}</p>
  {:else if restoreError}
    <p class="error">{t.views.login.unreachable}<br />{restoreError}</p>
  {/if}
  <p class="disclaimer">{t.views.login.disclaimer}</p>
  <p class="client-notice">
    {t.views.login.clientNotice}
    <button class="link" onclick={openClientNotice}>{t.views.login.clientNoticeMore}</button>
  </p>
</section>

<style>
  section {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    height: 100%;
    text-align: center;
  }

  h1 {
    margin: 0;
    font-weight: 600;
  }

  p {
    margin: 0;
    color: var(--text-muted);
  }

  .primary {
    margin-top: 8px;
    padding: 10px 24px;
    border-radius: 999px;
    font-weight: 600;
    color: var(--accent-contrast);
    background: var(--accent);
  }

  .primary:hover:not(:disabled) {
    background: var(--accent-strong);
  }

  .primary:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .steps {
    max-width: 420px;
  }

  .redirect {
    display: flex;
    gap: 8px;
    width: min(520px, 90%);
  }

  .redirect input {
    flex: 1;
    min-width: 0;
    padding: 8px 12px;
    border: 1px solid var(--border);
    border-radius: 999px;
    color: var(--text);
    background: var(--surface);
  }

  .redirect .primary {
    margin-top: 0;
  }

  .secondary {
    padding: 8px 18px;
    border: 1px solid var(--border);
    border-radius: 999px;
    color: var(--text);
    background: var(--surface);
  }

  .link {
    color: var(--text-muted);
    background: none;
    text-decoration: underline;
  }

  .link:hover:not(:disabled) {
    color: var(--text);
  }

  .error {
    max-width: 420px;
    color: var(--danger);
  }

  .disclaimer {
    margin-top: 24px;
    font-size: 12px;
  }

  .client-notice {
    max-width: 420px;
    font-size: 12px;
    color: var(--text-muted);
  }

  .client-notice .link {
    padding: 0;
    font-size: inherit;
  }
</style>
