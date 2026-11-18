import { describe, expect, it } from "vitest";
import { guessPlatform } from "./platform.svelte";

describe("guessPlatform", () => {
  it("reconnaît la WebView d'Android", () => {
    const s21 =
      "Mozilla/5.0 (Linux; Android 14; SM-G991B Build/UP1A.231005.007; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/157.0.8103.50 Mobile Safari/537.36";
    expect(guessPlatform(s21)).toBe("android");
  });

  it("retombe sur le bureau ailleurs", () => {
    const webview2 =
      "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0";
    const webkitgtk = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko)";
    expect(guessPlatform(webview2)).toBe("desktop");
    expect(guessPlatform(webkitgtk)).toBe("desktop");
  });
});
