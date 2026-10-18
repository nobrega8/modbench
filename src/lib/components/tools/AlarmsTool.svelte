<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import { byIndex, check, type Alarm } from "#lib/alarms.ts";
  import { formatValue } from "#lib/format.ts";
  import { registerId } from "#lib/profiles.ts";
  import { bench } from "#lib/state.svelte.ts";
  import Select from "../Select.svelte";

  const open = $derived(bench.map);
  const registers = $derived(open?.map.registers ?? []);
  const set = $derived(byIndex(bench.alarms, registers));
  const rows = $derived([...set].sort(([a], [b]) => a - b));
  let pick = $state(-1);

  const options = $derived(
    registers
      .map((reg, index) => ({ reg, index }))
      .filter(
        ({ reg, index }) =>
          reg.type !== "string" && reg.access !== "W" && !set.has(index),
      ),
  );

  function add(index: number) {
    const reg = registers[index];
    bench.setAlarms([
      ...bench.alarms,
      {
        id: registerId(reg),
        low: null,
        high: null,
        when: reg.type === "bool" ? true : null,
      },
    ]);
    pick = -1;
  }

  function update(alarm: Alarm, change: Partial<Alarm>) {
    bench.setAlarms(
      bench.alarms.map((a) => (a.id === alarm.id ? { ...a, ...change } : a)),
    );
  }

  function remove(alarm: Alarm) {
    bench.setAlarms(bench.alarms.filter((a) => a.id !== alarm.id));
  }

  /** A typed limit: empty for none, otherwise a number. */
  function limit(text: string): number | null | undefined {
    const t = text.trim().replace(",", ".");
    if (t === "") return null;
    const n = Number(t);
    return Number.isFinite(n) ? n : undefined;
  }

  function setLimit(alarm: Alarm, side: "low" | "high", e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const n = limit(input.value);
    if (n === undefined) {
      input.setAttribute("aria-invalid", "true");
      return;
    }
    input.removeAttribute("aria-invalid");
    update(alarm, { [side]: n });
  }
</script>

<header class="tool-head">
  <h1>{t("tools.alarms")}</h1>
  <p>{t("alarmstool.values_out_of_limits")}</p>
</header>

{#if !open}
  <p class="empty">{t("alarmstool.open_a_device_map")}</p>
{:else}
  <div class="row">
    <Select
      bind:value={pick}
      width="320px"
      label={t("alarmstool.add_an_alarm")}
      placeholder={t("alarmstool.add_an_alarm_on")}
      disabled={!options.length}
      options={options.map(({ reg, index }) => ({
        value: index,
        label: `${reg.name}${reg.unit ? ` (${reg.unit})` : ""}${reg.group ? ` · ${reg.group}` : ""}`,
      }))}
      onchange={add}
    />
  </div>

  {#if rows.length}
    <table class="data">
      <thead>
        <tr>
          <th>{t("req.register")}</th>
          <th class="num">{t("maptable.value")}</th>
          <th>{t("alarmstool.alarm_when")}</th>
          <th></th>
        </tr>
      </thead>
      <tbody>
        {#each rows as [index, alarm] (alarm.id)}
          {@const reg = registers[index]}
          {@const r = bench.mapResults.get(index)}
          {@const state = r && !r.error ? check(alarm, r.value) : null}
          <tr>
            <td
              >{reg.name}{#if reg.group}<span class="dim grp">{reg.group}</span
                >{/if}</td
            >
            <td class="num mono" class:bad={state !== null}
              >{r && !r.error ? formatValue(reg, r.value) : "—"}
              {#if r && !r.error}<span class="dim">{reg.unit}</span>{/if}</td
            >
            <td>
              {#if reg.type === "bool"}
                <Select
                  value={alarm.when ? 1 : 0}
                  label={t("alarmstool.alarm_state_of_name", {
                    name: reg.name,
                  })}
                  options={[
                    { value: 1, label: "ON" },
                    { value: 0, label: "OFF" },
                  ]}
                  onchange={(v) => update(alarm, { when: v === 1 })}
                />
              {:else}
                <span class="limits">
                  <span class="dim">{t("alarmstool.below")}</span>
                  <input
                    class="input num"
                    value={alarm.low ?? ""}
                    placeholder="—"
                    aria-label={t("alarmstool.low_limit_of_name", {
                      name: reg.name,
                    })}
                    onchange={(e) => setLimit(alarm, "low", e)}
                  />
                  <span class="dim">{t("alarmstool.or_above")}</span>
                  <input
                    class="input num"
                    value={alarm.high ?? ""}
                    placeholder="—"
                    aria-label={t("alarmstool.high_limit_of_name", {
                      name: reg.name,
                    })}
                    onchange={(e) => setLimit(alarm, "high", e)}
                  />
                  <span class="dim">{reg.unit}</span>
                </span>
              {/if}
            </td>
            <td class="act">
              <button
                class="btn ghost"
                aria-label={t("alarmstool.remove_the_alarm_on", {
                  name: reg.name,
                })}
                onclick={() => remove(alarm)}>{t("maptable.remove")}</button
              >
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {:else}
    <p class="empty">{t("alarmstool.no_alarms_yet")}</p>
  {/if}
{/if}

<style>
  p {
    margin: 0;
  }

  .grp {
    margin-left: 8px;
    font-size: 12px;
  }

  .limits {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }

  .limits input {
    width: 90px;
  }

  .limits input:global([aria-invalid="true"]) {
    box-shadow: 0 0 0 2px rgb(var(--alert-rgb) / 0.55);
  }

  td {
    vertical-align: middle !important;
  }

  .act {
    text-align: right;
  }
</style>
