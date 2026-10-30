<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { getVersion } from "@tauri-apps/api/app";
  import { _, locale } from "svelte-i18n";
  import WindowControls from "./components/WindowControls.svelte";
  import { frameless, maximized } from "./lib/window";
  import { LayoutGrid, SlidersHorizontal, Sun, Moon } from "lucide-svelte";
  import { api } from "./lib/api";
  import { busy } from "./lib/jobs";
  import { theme, toggleTheme } from "./lib/theme";
  import { SUPPORTED_LOCALES, LOCALE_NAMES, setLocale, type SupportedLocale } from "./i18n";
  import type { EnvironmentInfo } from "./lib/layouts";
  import AboutDialog from "./components/AboutDialog.svelte";
  import DialogHost from "./components/DialogHost.svelte";
  import Layouts from "./views/Layouts.svelte";
  import Settings from "./views/Settings.svelte";

  // The frame of the other Soplos 3.0 programs without a sidebar (WebApp
  // Manager, AppImage Manager, Plymouth Manager): a hero with the title and the
  // language, a thin footer with the theme button, the session and the version,
  // and the progress bar along the bottom edge. What is inside is the 2.x
  // program's two pages, Layout and Settings, which stay loaded when switching.

  type Page = "layout" | "settings";

  let appVersion = "";
  let environment: EnvironmentInfo = { desktopEnvironment: "unknown", displayProtocol: "unknown", shellVersion: "" };
  let aboutOpen = false;
  let page: Page = "layout";

  onMount(async () => {
    try {
      appVersion = await getVersion();
    } catch {
      appVersion = "";
    }
    try {
      environment = await api.invoke<EnvironmentInfo>("get_environment_info");
    } catch (error) {
      console.error("Reading the environment failed", error);
    }
  });

  // "3.0.0-1-beta" → "3.0.0-beta": the footer shows the app version, not the
  // Debian package revision.
  $: cleanVersion = appVersion.replace(/-\d+-/, "-");

  $: pages = [
    { id: "layout" as Page, label: $_("tabs.layout"), icon: LayoutGrid },
    { id: "settings" as Page, label: $_("tabs.settings"), icon: SlidersHorizontal },
  ];

  function onLanguageChange(e: Event) {
    setLocale((e.currentTarget as HTMLSelectElement).value as SupportedLocale);
  }

  // F1 -> About, Ctrl+Q -> quit.
  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "F1") {
      e.preventDefault();
      aboutOpen = true;
    } else if (e.ctrlKey && !e.shiftKey && (e.key === "q" || e.key === "Q")) {
      e.preventDefault();
      getCurrentWindow().close();
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

<div class="h-screen bg-dark {$frameless && !$maximized ? 'window-card' : ''}">
<div class="relative flex flex-col h-full overflow-hidden">
  {#if $frameless}
    <div class="relative h-7 shrink-0" data-tauri-drag-region>
      <div class="absolute right-2 top-[3px]"><WindowControls /></div>
    </div>
  {/if}

  <div class="p-4 pb-0 shrink-0 {$frameless ? 'pt-0' : ''}" data-tauri-drag-region="deep">
    <div
      class="relative rounded border border-border overflow-hidden flex flex-col items-start justify-end p-4 min-h-[110px]"
      style="background: linear-gradient(rgba(20, 20, 22, 0.25), rgba(20, 20, 22, 0.82)), url('/assets/hero/layout-switcher-background.jpg'), linear-gradient(135deg, #3a2412, #1b1b1d); background-size: cover; background-position: center;"
    >
      <div class="absolute top-3 right-3">
        <select
          value={$locale}
          on:change={onLanguageChange}
          class="appearance-none bg-primary/20 backdrop-blur-sm border border-primary/30 text-primary rounded px-2 py-1 text-[0.7rem] focus:outline-none"
          title={$_("sidebar.language")}
        >
          {#each SUPPORTED_LOCALES as code}
            <option value={code}>{LOCALE_NAMES[code]}</option>
          {/each}
        </select>
      </div>

      <p class="text-white font-semibold text-[0.95rem]">{$_("hero.title")}</p>
      <p class="text-[0.75rem] text-white/85 mt-0.5">{$_("hero.description")}</p>
    </div>
  </div>

  <div class="flex justify-center px-4 pt-4 shrink-0">
    <div class="flex gap-1.5 p-1 rounded-xl border border-border bg-surface shadow-sm" role="tablist">
      {#each pages as item (item.id)}
        <button
          role="tab"
          aria-selected={page === item.id}
          class="flex items-center gap-2 px-4 py-2 rounded-lg text-[0.84rem] transition-colors {page === item.id ? 'bg-primary text-black font-semibold shadow' : 'text-text-muted hover:text-text-bright hover:bg-surface-2'}"
          on:click={() => (page = item.id)}
        >
          <svelte:component this={item.icon} size={15} />
          {item.label}
        </button>
      {/each}
    </div>
  </div>

  <!-- Both pages stay loaded, so what was chosen on one is still there when
       coming back to it. -->
  <div class="flex-1 overflow-y-auto content-scroll p-4">
    <div class:hidden={page !== "layout"}><Layouts /></div>
    <div class:hidden={page !== "settings"}><Settings /></div>
  </div>

  <footer class="flex items-center justify-between px-4 py-1.5 border-t border-border text-[0.7rem] text-text-muted shrink-0">
    <div class="flex items-center gap-1.5">
      <button
        class="p-1 -m-1 rounded text-text-muted hover:bg-surface hover:text-text-bright"
        on:click={toggleTheme}
        title={$theme === "dark" ? $_("sidebar.switchToLight") : $_("sidebar.switchToDark")}
      >
        <svelte:component this={$theme === "dark" ? Moon : Sun} size={13} />
      </button>
      <span>
        {$_(`environment.${environment.desktopEnvironment}`)} · {$_(`environment.${environment.displayProtocol}`)}
      </span>
    </div>
    <button
      class="text-text-muted hover:text-text-bright hover:underline"
      on:click={() => (aboutOpen = true)}
      title={$_("sidebar.aboutTooltip")}
    >
      {cleanVersion}
    </button>
  </footer>

  <!-- The progress bar, as in the Welcome, the Store and the GRUB Editor: the
       last thing of the column, along the bottom edge of the window. Its track
       is only drawn while a job runs. -->
  <div class="h-1.5 bg-surface-2 overflow-hidden shrink-0" class:invisible={!$busy}>
    {#if $busy}
      <div class="h-full w-1/3 bg-primary progress-slide"></div>
    {/if}
  </div>
</div>

{#if aboutOpen}
  <AboutDialog onClose={() => (aboutOpen = false)} />
{/if}

<DialogHost />
</div>
