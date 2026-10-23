import { memo, type ReactNode } from "react";
import { Settings2 } from "lucide-react";
import type { ActivityBlock as ActivityBlockData } from "../../api";
import { formatDuration } from "../../lib/format";
import { AppIcon } from "../AppIcon";
import { Block } from "../Block";

/** Below `TEXT_MIN_PX` (px) a block shows no text; below `TWO_LINES_PX` (memory) or `ONE_LINE_PX` (summary), one line without the duration. */
const TEXT_MIN_PX = 12;
const TWO_LINES_PX = 28;
const ONE_LINE_PX = 40;

interface Props {
  block: ActivityBlockData;
  /** The summary lane's look, a pinned app lane's, or another activity lane's. */
  look: "summary" | "memory" | "pinned";
  top: number;
  height: number;
  focused: boolean;
  /** Its span is in the selection or the open editor's spans. */
  selected: boolean;
}

type BlockProps = Omit<Props, "look">;

const flags = (block: ActivityBlockData, focused: boolean, selected: boolean) => [block.private && "private", block.covered && "dimmed", focused && "focused", selected && "selected"];

const placement = (block: ActivityBlockData) => ({ left: `${(block.col / block.cols) * 100}%`, width: `${100 / block.cols}%` });

const ariaLabel = (block: ActivityBlockData) => (block.label === block.app ? block.app : `${block.app}: ${block.label}`);

function Gear({ block }: { block: ActivityBlockData }) {
  return (
    <button type="button" className="block-gear icon-btn" tabIndex={-1} aria-label={`Actions for ${block.app}`} data-block-gear>
      <Settings2 size="0.846rem" aria-hidden="true" />
    </button>
  );
}

/** A filled block with the app icon and label, the active time at the bottom right when tall. */
function SummaryBlock({ block, top, height, focused, selected }: BlockProps) {
  const text = height >= TEXT_MIN_PX;
  const classes = ["summary-block", height < ONE_LINE_PX && "one-line", ...flags(block, focused, selected)];
  return (
    <Block
      className={classes.filter(Boolean).join(" ")}
      data-block-id={block.id}
      aria-label={ariaLabel(block)}
      top={top}
      height={height}
      {...placement(block)}
      title={
        text ? (
          <>
            <AppIcon app={block.app} bundleId={block.bundleId} size={16} />
            <span>{block.label}</span>
          </>
        ) : undefined
      }
      trailing={height >= ONE_LINE_PX ? formatDuration(block.activeMs) : undefined}
    >
      {text && <Gear block={block} />}
    </Block>
  );
}

/** An activity lane's block: the bar as long as the activity, and `children` once tall enough for text. */
function MemoryFrame({ block, top, height, focused, selected, modifier, children }: BlockProps & { modifier?: string; children: ReactNode }) {
  const classes = ["memory", modifier, ...flags(block, focused, selected)];
  return (
    <div className={classes.filter(Boolean).join(" ")} data-block-id={block.id} aria-label={ariaLabel(block)} style={{ top, height, ...placement(block) }}>
      <span className="memory-bar" aria-hidden="true" />
      {height >= TEXT_MIN_PX && children}
    </div>
  );
}

/** The app icon over a bar as long as the activity, with the label and, when tall enough, the duration. */
function MemoryBlock(props: BlockProps) {
  const { block, height } = props;
  return (
    <MemoryFrame {...props}>
      <AppIcon app={block.app} bundleId={block.bundleId} size={16} />
      <span className="memory-text">
        <span className="memory-title">{block.label}</span>
        {height >= TWO_LINES_PX && <span className="memory-duration">{formatDuration(block.activeMs)}</span>}
      </span>
      <Gear block={block} />
    </MemoryFrame>
  );
}

/** A pinned app lane's memory: the icon and the duration; the hover card tells the rest. */
function PinnedBlock(props: BlockProps) {
  const { block } = props;
  return (
    <MemoryFrame {...props} modifier="memory--pinned">
      <AppIcon app={block.app} bundleId={block.bundleId} size={16} />
      <span className="memory-duration">{formatDuration(block.activeMs)}</span>
    </MemoryFrame>
  );
}

const LOOKS = { summary: SummaryBlock, memory: MemoryBlock, pinned: PinnedBlock };

export const ActivityBlock = memo(function ActivityBlock({ look, ...props }: Props) {
  const Look = LOOKS[look];
  return <Look {...props} />;
});
