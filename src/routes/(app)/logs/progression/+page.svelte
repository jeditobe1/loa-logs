<script lang="ts">
  import { page } from "$app/state";
  import {
    getEncountersByIds,
    getProgressionStats,
    type ProgressionEncounterStats,
    type ProgressionPlayerStats
  } from "$lib/api";
  import { chartable, defaultOptions, type EChartsOptions } from "$lib/charts";
  import { raidGates } from "$lib/constants/encounters";
  import { IconArrowLeft } from "$lib/icons";
  import type { EncounterPreview } from "$lib/types";
  import {
    abbreviateNumber,
    formatTimestamp,
    isSupportSpec,
    timestampToMinutesAndSeconds
  } from "$lib/utils";


  const SHORT_THRESHOLD = 15_000;

  type ViewMode = "overview" | "dps" | "support" | "raid";

  let ids = $derived(
    (page.url.searchParams.get("ids") || "")
      .split(",")
      .map(Number)
      .filter((n) => !isNaN(n) && n > 0)
  );

  let allAttempts: EncounterPreview[] = $state([]);
  let allStats: ProgressionEncounterStats[] = $state([]);
  let loading = $state(true);
  let hideShort = $state(true);
  let viewMode: ViewMode = $state("overview");

  // Filtered attempts
  let attempts = $derived(
    hideShort ? allAttempts.filter((a) => a.duration >= SHORT_THRESHOLD || a.cleared) : allAttempts
  );
  let stats = $derived.by(() => {
    const validIds = new Set(attempts.map((a) => a.id));
    return allStats.filter((s) => validIds.has(s.id));
  });
  let hiddenCount = $derived(allAttempts.length - attempts.length);

  // Summary
  let gateName = $derived(attempts.length > 0 ? raidGates[attempts[0].bossName] || attempts[0].bossName : "");
  let difficulty = $derived(attempts.length > 0 ? attempts[0].difficulty || "" : "");
  let totalTime = $derived(attempts.reduce((sum, a) => sum + a.duration, 0));
  let hasClear = $derived(attempts.some((a) => a.cleared));
  let isLocalSupport = $derived(attempts.length > 0 && isSupportSpec(attempts[0].spec));
  let bestDps = $derived(Math.max(...attempts.map((a) => a.myDps)));
  let avgDps = $derived(
    attempts.length > 0 ? Math.round(attempts.reduce((sum, a) => sum + a.myDps, 0) / attempts.length) : 0
  );
  let hasWipeBars = $derived(attempts.some((a) => a.wipeBars));

  // Determine local player's party members from party_info
  let myPartyNames = $derived.by((): Set<string> => {
    if (attempts.length === 0 || stats.length === 0) return new Set();
    const localPlayer = attempts[0].localPlayer;
    // Find the first encounter with party_info
    for (const s of stats) {
      if (!s.partyInfo) continue;
      for (const members of Object.values(s.partyInfo)) {
        if (members.includes(localPlayer)) {
          return new Set(members);
        }
      }
    }
    // Fallback: all players
    return new Set();
  });

  let hasPartyInfo = $derived(myPartyNames.size > 0);

  // Helper: get my party's players from a stats entry
  function getMyPartyPlayers(s: ProgressionEncounterStats): ProgressionPlayerStats[] {
    if (!hasPartyInfo) return s.players;
    return s.players.filter((p) => myPartyNames.has(p.name));
  }

  // Helper: find the party support from stats
  function getPartySupport(s: ProgressionEncounterStats): ProgressionPlayerStats | undefined {
    return getMyPartyPlayers(s).find((p) => {
      // check if they have support buff data
      return p.supportAp != null || p.supportBrand != null;
    });
  }

  $effect(() => {
    if (ids.length === 0) {
      loading = false;
      return;
    }

    (async () => {
      loading = true;
      const [encounters, progressionStats] = await Promise.all([
        getEncountersByIds(ids),
        getProgressionStats(ids)
      ]);
      allAttempts = encounters;
      allStats = progressionStats;
      loading = false;
    })();
  });

  // ──────────────────── Chart helpers ────────────────────

  function makeLineChart(
    title: string,
    data: (number | null)[],
    color: string,
    opts?: {
      yFormatter?: (v: number) => string;
      tooltipFormatter?: (params: { dataIndex: number; value: number }[]) => string;
      markMinMax?: boolean;
      areaColor?: string;
      clearIndices?: Set<number>;
    }
  ): EChartsOptions {
    if (data.length === 0) return {};
    const clearSet = opts?.clearIndices ?? new Set(attempts.map((a, i) => (a.cleared ? i : -1)).filter((i) => i >= 0));
    return {
      ...defaultOptions,
      title: { text: title, textStyle: { color: "#e5e5e5", fontSize: 14 } },
      ...(opts?.tooltipFormatter
        ? { tooltip: { trigger: "axis", formatter: opts.tooltipFormatter } }
        : { tooltip: { trigger: "axis" } }),
      xAxis: {
        type: "category",
        data: data.map((_, i) => `#${i + 1}`),
        axisLabel: { color: "#a3a3a3" }
      },
      yAxis: {
        type: "value",
        axisLabel: {
          color: "#a3a3a3",
          ...(opts?.yFormatter ? { formatter: opts.yFormatter } : {})
        }
      },
      series: [
        {
          type: "line",
          data,
          smooth: true,
          lineStyle: { color },
          itemStyle: {
            color: (params: { dataIndex: number }) => (clearSet.has(params.dataIndex) ? "#84cc16" : color)
          },
          ...(opts?.markMinMax
            ? {
                markPoint: {
                  label: { formatter: (params: { value: number }) => opts?.yFormatter ? opts.yFormatter(params.value) : abbreviateNumber(params.value) },
                  data: [{ type: "max", name: "Best" }, { type: "min", name: "Worst" }]
                }
              }
            : {}),
          ...(opts?.areaColor ? { areaStyle: { color: opts.areaColor } } : {})
        }
      ]
    };
  }

  // ──────────────────── Shared charts ────────────────────

  let durationChart: EChartsOptions = $derived.by(() => {
    if (attempts.length === 0) return {};
    return makeLineChart("Duration Over Attempts", attempts.map((a) => a.duration), "#f59e0b", {
      yFormatter: (v) => timestampToMinutesAndSeconds(v),
      tooltipFormatter: (params) => {
        const p = params[0];
        return `#${p.dataIndex + 1}<br/>${timestampToMinutesAndSeconds(p.value)}`;
      }
    });
  });

  let barsChart: EChartsOptions = $derived.by(() => {
    if (!hasWipeBars || attempts.length === 0) return {};
    const chart = makeLineChart(
      "Bars Remaining at Wipe",
      attempts.map((a) => (a.cleared ? 0 : a.wipeBars ?? null)),
      "#ef4444"
    );
    return { ...chart, yAxis: { ...(chart.yAxis as object), inverse: true } };
  });

  // ──────────────────── My DPS charts ────────────────────

  let myDpsChart: EChartsOptions = $derived.by(() => {
    if (attempts.length === 0) return {};
    return makeLineChart("My DPS", attempts.map((a) => a.myDps), "#8b5cf6", {
      yFormatter: (v) => abbreviateNumber(v),
      markMinMax: true,
      tooltipFormatter: (params) => {
        const p = params[0];
        const a = attempts[p.dataIndex];
        return `#${p.dataIndex + 1}${a?.cleared ? " (Clear)" : ""}<br/>DPS: ${abbreviateNumber(p.value)}`;
      }
    });
  });

  let myUdpsChart: EChartsOptions = $derived.by(() => {
    if (attempts.length === 0) return {};
    const hasUdps = attempts.some((a) => a.udps && a.udps !== a.myDps);
    if (!hasUdps) return {};
    return makeLineChart("My Unbuffed DPS", attempts.map((a) => a.udps ?? null), "#06b6d4", {
      yFormatter: (v) => abbreviateNumber(v),
      markMinMax: true
    });
  });

  // ──────────────────── Support charts ────────────────────

  function buffLine(label: string, color: string, accessor: (a: EncounterPreview) => number | undefined) {
    return {
      name: label,
      type: "line" as const,
      data: attempts.map((a) => {
        const v = accessor(a);
        return v != null ? +(v * 100).toFixed(1) : null;
      }),
      smooth: true,
      lineStyle: { color },
      itemStyle: { color }
    };
  }

  let supportBuffChart: EChartsOptions = $derived.by(() => {
    if (attempts.length === 0) return {};
    const hasBuff = attempts.some((a) => a.supportAp != null);
    if (!hasBuff) return {};
    return {
      ...defaultOptions,
      title: { text: "My Buff Uptime", textStyle: { color: "#e5e5e5", fontSize: 14 } },
      legend: { data: ["AP", "Brand", "Identity", "Hyper"], textStyle: { color: "#a3a3a3" }, top: 24 },
      tooltip: { trigger: "axis", valueFormatter: (v: number) => `${v.toFixed(1)}%` },
      xAxis: {
        type: "category",
        data: attempts.map((_, i) => `#${i + 1}`),
        axisLabel: { color: "#a3a3a3" }
      },
      yAxis: { type: "value", max: 100, axisLabel: { color: "#a3a3a3", formatter: (v: number) => `${v}%` } },
      series: [
        buffLine("AP", "#ef4444", (a) => a.supportAp),
        buffLine("Brand", "#22c55e", (a) => a.supportBrand),
        buffLine("Identity", "#eab308", (a) => a.supportIdentity),
        buffLine("Hyper", "#3b82f6", (a) => a.supportHyper)
      ]
    };
  });

  // Support DPS contribution: total party DPS minus sum of unbuffed DPS for my party
  let supportContribChart: EChartsOptions = $derived.by(() => {
    if (stats.length === 0) return {};
    const contribData = stats.map((s) => {
      const partyPlayers = getMyPartyPlayers(s);
      const totalPartyDps = partyPlayers.reduce((sum, p) => sum + p.dps, 0);
      const totalUnbuffed = partyPlayers.reduce((sum, p) => sum + (p.unbuffedDps ?? p.dps), 0);
      return Math.max(0, totalPartyDps - totalUnbuffed);
    });
    if (contribData.every((v) => v === 0)) return {};
    return makeLineChart("My Support DPS Contribution", contribData, "#a78bfa", {
      yFormatter: (v) => abbreviateNumber(v),
      areaColor: "rgba(167, 139, 250, 0.15)",
      tooltipFormatter: (params) => {
        const p = params[0];
        const s = stats[p.dataIndex];
        const partyDps = s ? getMyPartyPlayers(s).reduce((sum, pl) => sum + pl.dps, 0) : 0;
        return `#${p.dataIndex + 1}<br/>Contribution: ${abbreviateNumber(p.value)}<br/>Party DPS: ${abbreviateNumber(partyDps)}`;
      }
    });
  });

  // ──────────────────── Raid charts ────────────────────

  let raidDpsChart: EChartsOptions = $derived.by(() => {
    if (stats.length === 0) return {};
    return makeLineChart("Total Raid DPS", stats.map((s) => s.totalDps), "#8b5cf6", {
      yFormatter: (v) => abbreviateNumber(v),
      markMinMax: true,
      tooltipFormatter: (params) => {
        const p = params[0];
        const s = stats[p.dataIndex];
        const a = attempts[p.dataIndex];
        let lines = `#${p.dataIndex + 1}${a?.cleared ? " (Clear)" : ""}<br/>Total: ${abbreviateNumber(p.value)}`;
        if (s) {
          for (const player of s.players) {
            lines += `<br/>${player.name}: ${abbreviateNumber(player.dps)}`;
          }
        }
        return lines;
      }
    });
  });

  let raidDeathsChart: EChartsOptions = $derived.by(() => {
    if (stats.length === 0) return {};
    const deathData = stats.map((s) => s.players.filter((p) => p.isDead).length);
    if (deathData.every((d) => d === 0)) return {};
    return {
      ...defaultOptions,
      title: { text: "Deaths Per Attempt", textStyle: { color: "#e5e5e5", fontSize: 14 } },
      tooltip: {
        trigger: "axis",
        formatter: (params: { dataIndex: number; value: number }[]) => {
          const p = params[0];
          const s = stats[p.dataIndex];
          let lines = `#${p.dataIndex + 1} — ${p.value} death${p.value !== 1 ? "s" : ""}`;
          if (s) {
            for (const player of s.players.filter((pl) => pl.isDead)) {
              lines += `<br/><span style="color:#ef4444">✗</span> ${player.name}`;
            }
          }
          return lines;
        }
      },
      xAxis: {
        type: "category",
        data: stats.map((_, i) => `#${i + 1}`),
        axisLabel: { color: "#a3a3a3" }
      },
      yAxis: { type: "value", minInterval: 1, axisLabel: { color: "#a3a3a3" } },
      series: [
        {
          name: "Deaths",
          type: "bar",
          data: deathData,
          itemStyle: {
            color: (params: { dataIndex: number }) =>
              attempts[params.dataIndex]?.cleared ? "#84cc16" : "#ef4444"
          }
        }
      ]
    };
  });

  // Raid: per-player DPS stacked area
  let raidPlayerDpsChart: EChartsOptions = $derived.by(() => {
    if (stats.length === 0) return {};
    // Get all unique player names across attempts, in consistent order
    const allNames: string[] = [];
    const seen = new Set<string>();
    for (const s of stats) {
      for (const p of s.players) {
        if (!seen.has(p.name)) {
          seen.add(p.name);
          allNames.push(p.name);
        }
      }
    }
    if (allNames.length === 0) return {};

    const colors = ["#8b5cf6", "#3b82f6", "#06b6d4", "#22c55e", "#eab308", "#f59e0b", "#ef4444", "#ec4899",
                     "#a78bfa", "#60a5fa", "#67e8f9", "#86efac", "#fde047", "#fdba74", "#fca5a5", "#f9a8d4"];

    return {
      ...defaultOptions,
      title: { text: "Individual Player DPS", textStyle: { color: "#e5e5e5", fontSize: 14 } },
      legend: { data: allNames, textStyle: { color: "#a3a3a3", fontSize: 10 }, top: 24, type: "scroll" },
      tooltip: { trigger: "axis" },
      xAxis: {
        type: "category",
        data: stats.map((_, i) => `#${i + 1}`),
        axisLabel: { color: "#a3a3a3" }
      },
      yAxis: {
        type: "value",
        axisLabel: { color: "#a3a3a3", formatter: (v: number) => abbreviateNumber(v) }
      },
      series: allNames.map((name, idx) => ({
        name,
        type: "line" as const,
        stack: "total",
        areaStyle: {},
        emphasis: { focus: "series" as const },
        data: stats.map((s) => {
          const player = s.players.find((p) => p.name === name);
          return player ? player.dps : 0;
        }),
        lineStyle: { color: colors[idx % colors.length], width: 1 },
        itemStyle: { color: colors[idx % colors.length] }
      }))
    };
  });

  // ──────────────────── Table helpers ────────────────────

  const buffColors = ["text-red-300", "text-green-300", "text-yellow-300", "text-blue-300"];

  const tabs: { key: ViewMode; label: string }[] = [
    { key: "overview", label: "Overview" },
    { key: "dps", label: "My DPS" },
    { key: "support", label: "My Support" },
    { key: "raid", label: "Raid" }
  ];
</script>

{#snippet badge(text: string)}
  <p class="rounded-sm bg-neutral-700/80 px-2 py-0.5">{text}</p>
{/snippet}

<div>
  <div class="sticky top-0 z-20 bg-neutral-900/70 px-6 shadow-md drop-shadow-lg backdrop-blur-lg">
    <div class="h-18 mx-auto flex max-w-[180rem] items-center">
      <div class="flex flex-col px-1 py-4">
        <div class="flex gap-2 overflow-x-auto text-nowrap py-1 text-xs">
          <a
            href="/logs"
            class="bg-accent-500/70 hover:bg-accent-500/80 flex items-center gap-1 rounded-sm py-0.5 pl-1 pr-2"
          >
            <IconArrowLeft class="shrink-0" />
            Back
          </a>
          {#if !loading && attempts.length > 0}
            {#if difficulty}
              <p
                class="rounded-sm bg-neutral-700/80 px-2 py-0.5"
                class:text-yellow-300={difficulty === "Hard"}
                class:text-amber-600={difficulty === "Inferno" || difficulty === "Challenge" || difficulty === "Trial"}
                class:text-cyan-400={difficulty === "Solo"}
                class:text-purple-500={difficulty.includes("Extreme") || difficulty === "The First"}
              >
                {difficulty}
              </p>
            {/if}
            {@render badge(`${attempts.length} attempts`)}
            {@render badge(`Total: ${timestampToMinutesAndSeconds(totalTime)}`)}
            {#if hasClear}
              <p class="rounded-sm bg-lime-900/50 px-2 py-0.5 text-lime-400">Cleared</p>
            {:else}
              <p class="rounded-sm bg-red-900/50 px-2 py-0.5 text-red-300">In Progress</p>
            {/if}
            {@render badge(`Best: ${abbreviateNumber(bestDps)} DPS`)}
            {@render badge(`Avg: ${abbreviateNumber(avgDps)} DPS`)}
            <label class="flex items-center gap-1.5 rounded-sm bg-neutral-700/80 px-2 py-0.5">
              <input
                type="checkbox"
                bind:checked={hideShort}
                class="form-checkbox size-3.5 rounded-sm border-0 bg-neutral-600 checked:text-accent-600/80 focus:ring-0"
              />
              Hide short
              {#if hiddenCount > 0}
                <span class="text-neutral-400">({hiddenCount})</span>
              {/if}
            </label>
          {/if}
        </div>
        {#if !loading && attempts.length > 0}
          <div class="mt-1">
            <h1 class="text-xl font-semibold tracking-tight">
              <span class:text-lime-400={hasClear}>Progression:</span>
              {gateName}
            </h1>
          </div>
        {:else}
          <div class="mt-1">
            <h1 class="text-xl font-semibold tracking-tight">Progression</h1>
          </div>
        {/if}
      </div>
    </div>
  </div>

  <div class="mx-auto max-w-[180rem] px-6 py-4">
    {#if loading}
      <p class="text-neutral-400">Loading...</p>
    {:else if allAttempts.length === 0}
      <p class="text-neutral-400">No attempts found.</p>
    {:else}

      <!-- View Mode Tabs -->
      <div class="mb-4 flex gap-1">
        {#each tabs as tab}
          <button
            class="rounded-sm px-3 py-1 text-sm transition {viewMode === tab.key
              ? 'bg-accent-500/70 text-white'
              : 'bg-neutral-700/80 text-neutral-300 hover:bg-neutral-700'}"
            onclick={() => (viewMode = tab.key)}
          >
            {tab.label}
          </button>
        {/each}
      </div>

      <!-- Charts Grid -->
      <div class="grid gap-6 lg:grid-cols-2">

        {#if viewMode === "overview"}
          <!-- Overview: key charts from each area -->
          {#if isLocalSupport && Object.keys(supportBuffChart).length > 0}
            <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
              <div class="h-64" use:chartable={supportBuffChart}></div>
            </div>
          {:else}
            <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
              <div class="h-64" use:chartable={myDpsChart}></div>
            </div>
          {/if}
          {#if Object.keys(raidDpsChart).length > 0}
            <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
              <div class="h-64" use:chartable={raidDpsChart}></div>
            </div>
          {/if}
          <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
            <div class="h-64" use:chartable={durationChart}></div>
          </div>
          {#if hasWipeBars && Object.keys(barsChart).length > 0}
            <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
              <div class="h-64" use:chartable={barsChart}></div>
            </div>
          {/if}
        {/if}

        {#if viewMode === "dps"}
          <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
            <div class="h-64" use:chartable={myDpsChart}></div>
          </div>
          {#if Object.keys(myUdpsChart).length > 0}
            <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
              <div class="h-64" use:chartable={myUdpsChart}></div>
            </div>
          {/if}
          <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
            <div class="h-64" use:chartable={durationChart}></div>
          </div>
          {#if hasWipeBars && Object.keys(barsChart).length > 0}
            <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
              <div class="h-64" use:chartable={barsChart}></div>
            </div>
          {/if}
        {/if}

        {#if viewMode === "support"}
          {#if Object.keys(supportBuffChart).length > 0}
            <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
              <div class="h-64" use:chartable={supportBuffChart}></div>
            </div>
          {/if}
          {#if Object.keys(supportContribChart).length > 0}
            <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
              <div class="h-64" use:chartable={supportContribChart}></div>
            </div>
          {/if}
          <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
            <div class="h-64" use:chartable={durationChart}></div>
          </div>
          {#if hasWipeBars && Object.keys(barsChart).length > 0}
            <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
              <div class="h-64" use:chartable={barsChart}></div>
            </div>
          {/if}
        {/if}

        {#if viewMode === "raid"}
          {#if Object.keys(raidDpsChart).length > 0}
            <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
              <div class="h-64" use:chartable={raidDpsChart}></div>
            </div>
          {/if}
          {#if Object.keys(raidPlayerDpsChart).length > 0}
            <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
              <div class="h-64" use:chartable={raidPlayerDpsChart}></div>
            </div>
          {/if}
          {#if Object.keys(raidDeathsChart).length > 0}
            <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
              <div class="h-64" use:chartable={raidDeathsChart}></div>
            </div>
          {/if}
          <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
            <div class="h-64" use:chartable={durationChart}></div>
          </div>
          {#if hasWipeBars && Object.keys(barsChart).length > 0}
            <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
              <div class="h-64" use:chartable={barsChart}></div>
            </div>
          {/if}
        {/if}
      </div>

      <!-- Attempt Table -->
      <div class="mt-6 overflow-x-auto rounded-md border border-neutral-700/70">
        <table class="w-full text-sm">
          <thead class="bg-neutral-800/80 text-neutral-400">
            <tr>
              <th class="px-3 py-2 text-left">#</th>
              <th class="px-3 py-2 text-left">Boss</th>
              {#if viewMode === "support"}
                <th class="px-3 py-2 text-right">AP</th>
                <th class="px-3 py-2 text-right">Brand</th>
                <th class="px-3 py-2 text-right">Identity</th>
                <th class="px-3 py-2 text-right">Hyper</th>
              {:else if viewMode === "raid"}
                <th class="px-3 py-2 text-right">Raid DPS</th>
                <th class="px-3 py-2 text-right">Deaths</th>
              {:else}
                <th class="px-3 py-2 text-right">DPS</th>
              {/if}
              <th class="px-3 py-2 text-right">Duration</th>
              {#if hasWipeBars}
                <th class="px-3 py-2 text-right">Bars</th>
              {/if}
              <th class="px-3 py-2 text-right">Result</th>
              <th class="px-3 py-2 text-right">Date</th>
            </tr>
          </thead>
          <tbody class="text-neutral-200">
            {#each [...attempts].reverse() as attempt, i (attempt.id)}
              {@const attemptNum = attempts.length - i}
              {@const encounterStats = stats.find((s) => s.id === attempt.id)}
              {@const deaths = encounterStats?.players.filter((p) => p.isDead).length ?? 0}
              <tr class="border-t border-neutral-700/50 hover:bg-neutral-800/50">
                <td class="px-3 py-2">{attemptNum}</td>
                <td class="px-3 py-2">
                  <a href="/logs/{attempt.id}" class="hover:text-accent-500 hover:underline">
                    {attempt.bossName}
                  </a>
                </td>
                {#if viewMode === "support"}
                  {@const buffs = [attempt.supportAp, attempt.supportBrand, attempt.supportIdentity, attempt.supportHyper]}
                  {#each buffs as buff, bi}
                    <td class="px-3 py-2 text-right {buffColors[bi]}">
                      {buff != null ? `${(buff * 100).toFixed(1)}%` : "-"}
                    </td>
                  {/each}
                {:else if viewMode === "raid"}
                  <td class="px-3 py-2 text-right">{encounterStats ? abbreviateNumber(encounterStats.totalDps) : "-"}</td>
                  <td class="px-3 py-2 text-right">
                    {#if deaths > 0}
                      <span class="text-red-300">{deaths}</span>
                    {:else}
                      <span class="text-neutral-500">0</span>
                    {/if}
                  </td>
                {:else}
                  <td class="px-3 py-2 text-right">{abbreviateNumber(attempt.myDps)}</td>
                {/if}
                <td class="px-3 py-2 text-right">{timestampToMinutesAndSeconds(attempt.duration)}</td>
                {#if hasWipeBars}
                  <td class="px-3 py-2 text-right">
                    {#if attempt.cleared}
                      <span class="text-lime-400">Clear</span>
                    {:else if attempt.wipeBars}
                      <span class="text-red-300">{attempt.wipeBars}x</span>
                    {:else}
                      -
                    {/if}
                  </td>
                {/if}
                <td class="px-3 py-2 text-right">
                  {#if attempt.cleared}
                    <span class="text-lime-400">Clear</span>
                  {:else}
                    <span class="text-red-300">Wipe</span>
                  {/if}
                </td>
                <td class="px-3 py-2 text-right text-xs">{formatTimestamp(attempt.fightStart)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>
</div>
