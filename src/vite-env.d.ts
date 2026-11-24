/// <reference types="svelte" />
/// <reference types="vite/client" />

/** Contents of the VERSION file, injected by vite.config.ts. */
declare const __APP_VERSION__: string;

/** True in the website demo build (`vite build --mode demo`, #28): the mock backend is kept. */
declare const __DEMO__: boolean;
