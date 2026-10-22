import { memo } from "react";
import { Settings2 } from "lucide-react";
import type { ActivityBlock as ActivityBlockData } from "../../api";
import { formatHm } from "../../lib/format";
import { AppIcon } from "../AppIcon";
import { Block } from "../Block";

/** Below this height (px) a block shows app and title on one line; below `TEXT_MIN_PX`, no text. */
const ONE_LINE_PX = 40;
const TEXT_MIN_PX = 10;

interface Props {
  block: ActivityBlockData;
  top: number;
  height: number;
  focused: boolean;
}

export const ActivityBlock = memo(function ActivityBlock({ block, top, height, focused }: Props) {
  const classes = [height < ONE_LINE_PX && "one-line", block.private && "private", block.covered && "dimmed", focused && "focused"];
  const text = height >= TEXT_MIN_PX;
  return (
    <Block
      className={classes.filter(Boolean).join(" ")}
      data-block-id={block.id}
      aria-label={block.label === block.app ? block.app : `${block.app}: ${block.label}`}
      top={top}
      height={height}
      left={`${(block.col / block.cols) * 100}%`}
      width={`${100 / block.cols}%`}
      title={
        text ? (
          <>
            <AppIcon app={block.app} bundleId={block.bundleId} size={16} />
            <span>{block.app}</span>
          </>
        ) : undefined
      }
      subtitle={block.label !== block.app ? block.label : undefined}
      trailing={<span className="mono">{formatHm(block.end - block.start)}</span>}
    >
      {text && (
        <button type="button" className="block-gear icon-btn" tabIndex={-1} aria-label={`Actions for ${block.app}`} data-block-gear>
          <Settings2 size="0.846rem" aria-hidden="true" />
        </button>
      )}
    </Block>
  );
});
