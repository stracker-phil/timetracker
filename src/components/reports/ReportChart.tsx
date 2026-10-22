import { useState } from "react";
import type { ChartSlice, ReportCharts } from "../../api";
import { formatDuration, plural } from "../../lib/format";
import { accentColor, AccentDot } from "../Accent";

type ChartBy = "client" | "project" | "tag";

const CHART_BYS: { by: ChartBy; label: string; slices: keyof ReportCharts; one: string; many: string }[] = [
  { by: "client", label: "Client", slices: "byClient", one: "client", many: "clients" },
  { by: "project", label: "Project", slices: "byProject", one: "project", many: "projects" },
  { by: "tag", label: "Tag", slices: "byTag", one: "tag", many: "tags" },
];

const SIZE = 220;
const CENTER = SIZE / 2;
const RADIUS = 66;
const LABEL_RADIUS = 94;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;
/** The space between two slices, along the ring. */
const GAP = 1.5;
/** Smaller slices carry no percentage, so labels never collide. */
const LABELED_SHARE = 0.03;

const percentLabel = (slice: ChartSlice) => `${slice.percent} %`;

function labelPosition(slice: ChartSlice) {
  const angle = (slice.offset + slice.share / 2) * 2 * Math.PI - Math.PI / 2;
  return { x: CENTER + LABEL_RADIUS * Math.cos(angle), y: CENTER + LABEL_RADIUS * Math.sin(angle) };
}

function Donut({ slices, totalMs, caption, name }: { slices: ChartSlice[]; totalMs: number; caption: string; name: string }) {
  const gap = slices.length > 1 ? GAP : 0;
  const summary = slices.map((s) => `${s.label} ${formatDuration(s.roundedMs)} (${percentLabel(s)})`).join(", ");
  return (
    <svg className="donut" viewBox={`0 0 ${SIZE} ${SIZE}`} role="img" aria-label={`${name}: ${summary}`}>
      <circle className="donut-track" cx={CENTER} cy={CENTER} r={RADIUS} />
      <g transform={`rotate(-90 ${CENTER} ${CENTER})`}>
        {slices.map((s) => (
          <circle
            key={s.key}
            className="donut-slice"
            cx={CENTER}
            cy={CENTER}
            r={RADIUS}
            style={{ stroke: accentColor(s.color, s.shade) }}
            strokeDasharray={`${Math.max(s.share * CIRCUMFERENCE - gap, 0)} ${CIRCUMFERENCE}`}
            strokeDashoffset={-s.offset * CIRCUMFERENCE}
          />
        ))}
      </g>
      {slices
        .filter((s) => s.share >= LABELED_SHARE)
        .map((s) => (
          <text key={s.key} className="donut-percent" {...labelPosition(s)}>
            {percentLabel(s)}
          </text>
        ))}
      <text className="donut-total" x={CENTER} y={CENTER - 6}>
        {formatDuration(totalMs)}
      </text>
      <text className="donut-caption" x={CENTER} y={CENTER + 16}>
        {caption}
      </text>
    </svg>
  );
}

/** A donut of the report's time by client, project or tag (a picker switches), with its legend. */
export function ReportChart({ charts, totalMs, initial }: { charts: ReportCharts; totalMs: number; initial: ChartBy }) {
  const [by, setBy] = useState(initial);
  const chart = CHART_BYS.find((c) => c.by === by) ?? CHART_BYS[0];
  const slices = charts[chart.slices];
  const name = `By ${by}`;
  return (
    <section className="report-chart" aria-label={name}>
      <header className="report-chart-head">
        <span>By</span>
        <select className="input" aria-label="Chart by" value={by} onChange={(e) => setBy(e.target.value as ChartBy)}>
          {CHART_BYS.map((c) => (
            <option key={c.by} value={c.by}>
              {c.label}
            </option>
          ))}
        </select>
      </header>
      <div className="report-chart-body">
        <Donut slices={slices} totalMs={totalMs} caption={plural(slices.length, chart.one, chart.many)} name={name} />
        <ul className="report-legend" aria-label={`${name} legend`}>
          {slices.map((s) => (
            <li key={s.key}>
              <AccentDot accent={s.color} shade={s.shade} />
              <span className="grow-text">{s.label}</span>
              <span className="report-hours">{formatDuration(s.roundedMs)}</span>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}
