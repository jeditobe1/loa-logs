<script lang="ts">
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
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
  import QuickTooltip from "$lib/components/QuickTooltip.svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import {
    abbreviateNumber,
    formatTimestamp,
    getClassIcon,
    isNameValid,
    isSupportSpec,
    LOA_BIBLE_URL,
    timestampToMinutesAndSeconds
  } from "$lib/utils";
  import { IconExternalLink } from "$lib/icons";


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

  // stats ordered to match attempts (backend may return in different order)
  let statsInOrder = $derived(attempts.map((a) => stats.find((s) => s.id === a.id)));

  // Summary
  let gateName = $derived(attempts.length > 0 ? raidGates[attempts[0].bossName] || attempts[0].bossName : "");
  let localPlayer = $derived(attempts.length > 0 ? attempts[0].localPlayer : "");
  let difficulty = $derived(attempts.length > 0 ? attempts[0].difficulty || "" : "");
  let totalTime = $derived(attempts.reduce((sum, a) => sum + a.duration, 0));
  let hasClear = $derived(attempts.some((a) => a.cleared));
  let isLocalSupport = $derived(attempts.length > 0 && isSupportSpec(attempts[0].spec));
  let bestDps = $derived(Math.max(...attempts.map((a) => a.myDps)));
  let avgDps = $derived(
    attempts.length > 0 ? Math.round(attempts.reduce((sum, a) => sum + a.myDps, 0) / attempts.length) : 0
  );
  let hasWipeBars = $derived(attempts.some((a) => a.wipeBars));
  let hasPhases = $derived(stats.some((s) => s.phases && s.phases.filter((p) => p.phaseType !== "intermission").length > 1));
  let hasWipeData = $derived(stats.some((s) => s.wipePhase != null && s.wipePhaseEndHp != null));

  // Total phases across all attempts — used to normalize progress scores for cross-phase comparison.
  // A P1 wipe only has 1 phase in its data, so we use the max seen across all attempts.
  let totalPhases = $derived.by(() => {
    let max = 1;
    for (const s of stats) {
      const combat = (s.phases ?? []).filter((p) => p.phaseType !== "intermission");
      if (combat.length > max) max = combat.length;
      if (s.wipePhase && s.wipePhase > max) max = s.wipePhase;
    }
    return max;
  });

  // Comparable 0–1 progress value. Phase number takes precedence; end HP breaks ties within a phase.
  // P1 @ 6% HP remaining  = (0 + 0.94) / 2 = 0.47
  // P2 @ 30% HP remaining = (1 + 0.70) / 2 = 0.85
  function normalizedProgress(s: ProgressionEncounterStats, cleared: boolean): number | null {
    if (cleared) return 1.0;
    if (s.wipePhase == null || s.wipePhaseEndHp == null) return null;
    return ((s.wipePhase - 1) + (1 - s.wipePhaseEndHp)) / totalPhases;
  }

  // "Best Progress" for the header badge: find the attempt with highest normalized score
  let bestProgressAttempt = $derived.by((): { phase: number; endHp: number } | null => {
    let best: { score: number; phase: number; endHp: number } | null = null;
    for (let i = 0; i < statsInOrder.length; i++) {
      const s = statsInOrder[i];
      if (!s) continue;
      const cleared = attempts[i]?.cleared ?? false;
      if (cleared) return { phase: totalPhases, endHp: 0 };
      const score = normalizedProgress(s, false);
      if (score != null && (best == null || score > best.score)) {
        best = { score, phase: s.wipePhase!, endHp: s.wipePhaseEndHp! };
      }
    }
    return best ? { phase: best.phase, endHp: best.endHp } : null;
  });

  // Determine local player's party members from party_info
  let myPartyNames = $derived.by((): Set<string> => {
    if (attempts.length === 0 || stats.length === 0) return new Set();
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

  // Region from first stat that has it (for character profile links)
  let region = $derived(stats.find((s) => s.region)?.region ?? "");

  // Full raid roster grouped by party. Party comp may vary across attempts — use the first stat with partyInfo.
  let raidGroups = $derived.by((): { title: string; members: { name: string; classId: number }[] }[] => {
    // name -> classId from all stats (first seen wins)
    const classMap = new Map<string, number>();
    for (const s of statsInOrder) {
      if (!s) continue;
      for (const p of s.players) {
        if (!classMap.has(p.name)) classMap.set(p.name, p.classId);
      }
    }
    if (classMap.size === 0) return [];

    const partyInfo = stats.find((s) => s.partyInfo)?.partyInfo;
    if (partyInfo) {
      return Object.entries(partyInfo)
        .sort(([a], [b]) => Number(a) - Number(b))
        .map(([idx, names]) => ({
          title: `Party ${idx}`,
          members: names.filter((n) => classMap.has(n)).map((n) => ({ name: n, classId: classMap.get(n)! }))
        }))
        .filter((g) => g.members.length > 0);
    }
    return [{ title: "", members: [...classMap.entries()].map(([name, classId]) => ({ name, classId })) }];
  });

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

  // ──────────────────── Chart click navigation ────────────────────

  function navigateToEncounter(dataIndex: number) {
    const attempt = attempts[dataIndex];
    if (!attempt) return;
    const back = encodeURIComponent(page.url.pathname + page.url.search);
    goto(`/logs/${attempt.id}?back=${back}`);
  }

  function clickableChart(el: HTMLElement, options: EChartsOptions) {
    const handle = chartable(el, options);
    handle.echartsInstance.on("click", (e: { dataIndex?: number }) => {
      if (e.dataIndex != null) navigateToEncounter(e.dataIndex);
    });
    return {
      destroy: handle.destroy,
      update: (newOptions: EChartsOptions) => handle.update(newOptions)
    };
  }

  // ──────────────────── Chart helpers ────────────────────

  function makeLineChart(
    title: string,
    data: (number | null)[],
    color: string,
    opts?: {
      yFormatter?: (v: number) => string;
      tooltipFormatter?: (params: { dataIndex: number; value: number }[]) => string;
      areaColor?: string;
      clearIndices?: Set<number>;
      itemColorFn?: (dataIndex: number) => string;
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
            color: opts?.itemColorFn
              ? (params: { dataIndex: number }) => opts!.itemColorFn!(params.dataIndex)
              : (params: { dataIndex: number }) => (clearSet.has(params.dataIndex) ? "#84cc16" : color)
          },
          ...(opts?.areaColor ? { areaStyle: { color: opts.areaColor } } : {})
        }
      ]
    };
  }

  const phaseColors = ["#3b82f6", "#f59e0b", "#ef4444", "#a78bfa", "#06b6d4"];

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
    const statsById = new Map(stats.map((s) => [s.id, s]));
    const tooltipFormatter = hasPhases
      ? (params: any[]) => {
          const p = params[0];
          const a = attempts[p.dataIndex];
          const s = a ? statsById.get(a.id) : undefined;
          const phaseLabel = s?.wipePhase ? ` (P${s.wipePhase})` : "";
          return `#${p.dataIndex + 1}${a?.cleared ? " (Clear)" : ""}<br/>Bars: ${p.value ?? "—"}${phaseLabel}`;
        }
      : undefined;
    const chart = makeLineChart(
      "Bars Remaining at Wipe",
      attempts.map((a) => (a.cleared ? 0 : a.wipeBars ?? null)),
      "#ef4444",
      { tooltipFormatter }
    );
    return { ...chart, yAxis: { ...(chart.yAxis as object), inverse: true } };
  });

  let progressChart: EChartsOptions = $derived.by(() => {
    if (!hasWipeData || attempts.length === 0) return {};

    const completionData = statsInOrder.map((s, i) => {
      const cleared = attempts[i]?.cleared ?? false;
      const score = s ? normalizedProgress(s, cleared) : null;
      return score != null ? +(score * 100).toFixed(1) : null;
    });

    const presentPhases = [...new Set(
      statsInOrder.map((s, i) => (!attempts[i]?.cleared && s?.wipePhase != null ? s.wipePhase : null))
        .filter((p): p is number => p != null)
    )].sort((a, b) => a - b);
    const hasClears = attempts.some((a) => a.cleared);

    const scatterSeries = [
      ...presentPhases.map((phase) => ({
        name: `P${phase}`,
        type: "scatter" as const,
        data: statsInOrder.map((s, i) =>
          !attempts[i]?.cleared && s?.wipePhase === phase ? completionData[i] : null
        ),
        itemStyle: { color: phaseColors[(phase - 1) % phaseColors.length] },
        symbolSize: 8,
        z: 3
      })),
      ...(hasClears ? [{
        name: "Clear",
        type: "scatter" as const,
        data: statsInOrder.map((_, i) => attempts[i]?.cleared ? completionData[i] : null),
        itemStyle: { color: "#84cc16" },
        symbolSize: 8,
        z: 3
      }] : [])
    ];

    const legendNames = [...presentPhases.map((p) => `P${p}`), ...(hasClears ? ["Clear"] : [])];
    const showLegend = presentPhases.length > 1 || hasClears;

    return {
      ...defaultOptions,
      title: { text: "Completion %", textStyle: { color: "#e5e5e5", fontSize: 14 } },
      legend: showLegend ? { data: legendNames, textStyle: { color: "#a3a3a3" }, top: 24 } : { show: false },
      grid: { ...(defaultOptions.grid as object), top: showLegend ? "28%" : "18%" },
      tooltip: {
        trigger: "axis",
        formatter: (params: any[]) => {
          const line = params.find((p: any) => p.seriesType === "line");
          if (!line || line.value == null) return "";
          const idx = line.dataIndex;
          const s = statsInOrder[idx];
          const cleared = attempts[idx]?.cleared ?? false;
          if (cleared) return `#${idx + 1} (Clear)<br/>Completion: 100%`;
          const phaseLabel = s?.wipePhase ? `P${s.wipePhase}` : "";
          const withinPhase = s?.wipePhaseEndHp != null
            ? ` · ${((1 - s.wipePhaseEndHp) * 100).toFixed(1)}% depleted`
            : "";
          return `#${idx + 1}<br/>${phaseLabel}${withinPhase}<br/>Completion: ${line.value}%`;
        }
      },
      xAxis: { type: "category", data: completionData.map((_, i) => `#${i + 1}`), axisLabel: { color: "#a3a3a3" } },
      yAxis: { type: "value", axisLabel: { color: "#a3a3a3", formatter: (v: number) => `${v}%` } },
      series: [
        {
          type: "line",
          data: completionData,
          smooth: true,
          lineStyle: { color: "#22c55e" },
          symbol: "none",
          z: 2
        },
        ...scatterSeries
      ]
    };
  });

  // ──────────────────── My DPS charts ────────────────────

  let myDpsChart: EChartsOptions = $derived.by(() => {
    if (attempts.length === 0) return {};
    const hasUdps = attempts.some((a) => a.udps && a.udps !== a.myDps);
    const clearSet = new Set(attempts.map((a, i) => (a.cleared ? i : -1)).filter((i) => i >= 0));
    if (!hasUdps) {
      return makeLineChart("My DPS", attempts.map((a) => a.myDps), "#8b5cf6", {
        yFormatter: (v) => abbreviateNumber(v),
        tooltipFormatter: (params) => {
          const p = params[0];
          const a = attempts[p.dataIndex];
          return `#${p.dataIndex + 1}${a?.cleared ? " (Clear)" : ""}<br/>DPS: ${abbreviateNumber(p.value)}`;
        }
      });
    }
    return {
      ...defaultOptions,
      title: { text: "My DPS", textStyle: { color: "#e5e5e5", fontSize: 14 } },
      legend: { data: ["Buffed", "Unbuffed"], textStyle: { color: "#a3a3a3" }, top: 24 },
      grid: { ...defaultOptions.grid as object, top: "28%" },
      tooltip: {
        trigger: "axis",
        formatter: (params: { dataIndex: number; value: number; seriesName: string }[]) => {
          const idx = params[0]?.dataIndex;
          const a = attempts[idx];
          let s = `#${idx + 1}${a?.cleared ? " (Clear)" : ""}`;
          for (const p of params) {
            if (p.value != null) s += `<br/>${p.seriesName}: ${abbreviateNumber(p.value)}`;
          }
          return s;
        }
      },
      xAxis: { type: "category", data: attempts.map((_, i) => `#${i + 1}`), axisLabel: { color: "#a3a3a3" } },
      yAxis: { type: "value", axisLabel: { color: "#a3a3a3", formatter: (v: number) => abbreviateNumber(v) } },
      series: [
        {
          name: "Buffed",
          type: "line",
          data: attempts.map((a) => a.myDps),
          smooth: true,
          lineStyle: { color: "#8b5cf6" },
          itemStyle: { color: (p: { dataIndex: number }) => clearSet.has(p.dataIndex) ? "#84cc16" : "#8b5cf6" }
        },
        {
          name: "Unbuffed",
          type: "line",
          data: attempts.map((a) => a.udps ?? null),
          smooth: true,
          lineStyle: { color: "#06b6d4", type: "dashed" },
          itemStyle: { color: "#06b6d4" }
        }
      ]
    };
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
      grid: { ...defaultOptions.grid as object, top: "28%" },
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
    if (statsInOrder.length === 0) return {};
    const contribData = statsInOrder.map((s, i) => {
      if (!s) return 0;
      const duration = (attempts[i]?.duration ?? 0) / 1000;
      const partyPlayers = getMyPartyPlayers(s);
      const totalPartyDps = partyPlayers.reduce((sum, p) => sum + p.dps, 0);
      const totalUnbuffed = partyPlayers.reduce((sum, p) => sum + (p.unbuffedDps ?? p.dps), 0);
      return Math.round(Math.max(0, totalPartyDps - totalUnbuffed) * duration);
    });
    if (contribData.every((v) => v === 0)) return {};
    return makeLineChart("My Support Damage Contribution", contribData, "#a78bfa", {
      yFormatter: (v) => abbreviateNumber(v),
      areaColor: "rgba(167, 139, 250, 0.15)",
      tooltipFormatter: (params) => {
        const p = params[0];
        const s = statsInOrder[p.dataIndex];
        const a = attempts[p.dataIndex];
        const duration = (a?.duration ?? 0) / 1000;
        const partyDamage = s ? Math.round(getMyPartyPlayers(s).reduce((sum, pl) => sum + pl.dps, 0) * duration) : 0;
        return `#${p.dataIndex + 1}<br/>Contribution: ${abbreviateNumber(p.value)}<br/>Party Damage: ${abbreviateNumber(partyDamage)}`;
      }
    });
  });

  // ──────────────────── Raid charts ────────────────────

  let raidDpsChart: EChartsOptions = $derived.by(() => {
    if (statsInOrder.length === 0) return {};
    const totalDamageData = statsInOrder.map((s, i) => {
      if (!s) return null;
      const duration = (attempts[i]?.duration ?? 0) / 1000;
      return Math.round(s.totalDps * duration);
    });
    return makeLineChart("Total Raid Damage", totalDamageData, "#8b5cf6", {
      yFormatter: (v) => abbreviateNumber(v),
      tooltipFormatter: (params) => {
        const p = params[0];
        const s = statsInOrder[p.dataIndex];
        const a = attempts[p.dataIndex];
        const duration = (a?.duration ?? 0) / 1000;
        let lines = `#${p.dataIndex + 1}${a?.cleared ? " (Clear)" : ""}<br/>Total: ${abbreviateNumber(p.value)}`;
        if (s) {
          for (const player of s.players) {
            lines += `<br/>${player.name}: ${abbreviateNumber(Math.round(player.dps * duration))}`;
          }
        }
        return lines;
      }
    });
  });


  // Raid: per-player DPS stacked area
  let raidPlayerDpsChart: EChartsOptions = $derived.by(() => {
    if (statsInOrder.length === 0) return {};
    // Get all unique player names across attempts, in consistent order
    const allNames: string[] = [];
    const seen = new Set<string>();
    for (const s of statsInOrder) {
      if (!s) continue;
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
      title: { text: "Individual Player Damage", textStyle: { color: "#e5e5e5", fontSize: 14 } },
      legend: { data: allNames, textStyle: { color: "#a3a3a3", fontSize: 10 }, top: 24, type: "scroll" },
      grid: { ...defaultOptions.grid as object, top: "28%" },
      tooltip: { trigger: "axis" },
      xAxis: {
        type: "category",
        data: statsInOrder.map((_, i) => `#${i + 1}`),
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
        data: statsInOrder.map((s, i) => {
          const player = s?.players.find((p) => p.name === name);
          const duration = (attempts[i]?.duration ?? 0) / 1000;
          return player ? Math.round(player.dps * duration) : 0;
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

{#snippet raidCard()}
  {#if raidGroups.length > 0}
    <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-4">
      <h3 class="mb-3 text-sm font-medium text-neutral-400">Raid</h3>
      <div class="flex flex-wrap gap-x-6 gap-y-4">
        {#each raidGroups as group}
          <div class="flex flex-col gap-1">
            {#if group.title}
              <p class="mb-1 text-xs text-neutral-500">{group.title}</p>
            {/if}
            {#each group.members as member}
              <div class="flex items-center gap-1.5">
                <QuickTooltip tooltip={member.name} class="shrink-0">
                  <img src={getClassIcon(member.classId)} alt="class-{member.classId}" class="size-7" />
                </QuickTooltip>
                <span class="truncate text-sm" class:text-accent-400={member.name === localPlayer}>
                  {member.name}
                </span>
                {#if region && isNameValid(member.name)}
                  <button
                    class="shrink-0 text-neutral-500 hover:text-neutral-200"
                    title="View Character Profile"
                    onclick={() => openUrl(`${LOA_BIBLE_URL}/character/${region}/${member.name}`)}
                  >
                    <IconExternalLink class="size-3" />
                  </button>
                {/if}
              </div>
            {/each}
          </div>
        {/each}
      </div>
    </div>
  {/if}
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
            {#if bestProgressAttempt != null}
              {#if hasClear}
                {@render badge(`Best: Clear`)}
              {:else}
                {@render badge(`Best: P${bestProgressAttempt.phase} · ${((1 - bestProgressAttempt.endHp) * 100).toFixed(1)}%`)}
              {/if}
            {/if}
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
              <span class="text-sm font-normal text-neutral-400">— {localPlayer}</span>
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
              <div class="h-64" use:clickableChart={supportBuffChart}></div>
            </div>
          {:else}
            <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
              <div class="h-64" use:clickableChart={myDpsChart}></div>
            </div>
          {/if}
          {#if Object.keys(raidDpsChart).length > 0}
            <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
              <div class="h-64" use:clickableChart={raidDpsChart}></div>
            </div>
          {/if}
          <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
            <div class="h-64" use:clickableChart={durationChart}></div>
          </div>
          {#if hasWipeData && Object.keys(progressChart).length > 0}
            <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
              <div class="h-64" use:clickableChart={progressChart}></div>
            </div>
          {:else if hasWipeBars && Object.keys(barsChart).length > 0}
            <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
              <div class="h-64" use:clickableChart={barsChart}></div>
            </div>
          {/if}
          {@render raidCard()}
        {/if}

        {#if viewMode === "dps"}
          <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
            <div class="h-64" use:clickableChart={myDpsChart}></div>
          </div>
          <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
            <div class="h-64" use:clickableChart={durationChart}></div>
          </div>
          {#if hasWipeData && Object.keys(progressChart).length > 0}
            <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
              <div class="h-64" use:clickableChart={progressChart}></div>
            </div>
          {:else if hasWipeBars && Object.keys(barsChart).length > 0}
            <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
              <div class="h-64" use:clickableChart={barsChart}></div>
            </div>
          {/if}
        {/if}

        {#if viewMode === "support"}
          {#if Object.keys(supportBuffChart).length > 0}
            <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
              <div class="h-64" use:clickableChart={supportBuffChart}></div>
            </div>
          {/if}
          {#if Object.keys(supportContribChart).length > 0}
            <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
              <div class="h-64" use:clickableChart={supportContribChart}></div>
            </div>
          {/if}
          <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
            <div class="h-64" use:clickableChart={durationChart}></div>
          </div>
          {#if hasWipeData && Object.keys(progressChart).length > 0}
            <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
              <div class="h-64" use:clickableChart={progressChart}></div>
            </div>
          {:else if hasWipeBars && Object.keys(barsChart).length > 0}
            <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
              <div class="h-64" use:clickableChart={barsChart}></div>
            </div>
          {/if}
        {/if}

        {#if viewMode === "raid"}
          {#if Object.keys(raidDpsChart).length > 0}
            <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
              <div class="h-64" use:clickableChart={raidDpsChart}></div>
            </div>
          {/if}
          {#if Object.keys(raidPlayerDpsChart).length > 0}
            <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
              <div class="h-64" use:clickableChart={raidPlayerDpsChart}></div>
            </div>
          {/if}
          <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
            <div class="h-64" use:clickableChart={durationChart}></div>
          </div>
          {#if hasWipeData && Object.keys(progressChart).length > 0}
            <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
              <div class="h-64" use:clickableChart={progressChart}></div>
            </div>
          {:else if hasWipeBars && Object.keys(barsChart).length > 0}
            <div class="rounded-md border border-neutral-700/70 bg-neutral-800/30 p-2">
              <div class="h-64" use:clickableChart={barsChart}></div>
            </div>
          {/if}
          {@render raidCard()}
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
                <th class="px-3 py-2 text-right">{hasPhases ? "Progress" : "Bars"}</th>
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
                  {@const s = stats.find((st) => st.id === attempt.id)}
                  <td class="px-3 py-2 text-right">
                    {#if attempt.cleared}
                      <span class="text-lime-400">Clear</span>
                    {:else if s?.wipePhase != null && s.wipePhaseEndHp != null}
                      <span class="text-red-300">
                        {#if hasPhases}P{s.wipePhase} · {/if}{((1 - s.wipePhaseEndHp) * 100).toFixed(1)}%
                      </span>
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
