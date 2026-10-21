import { memo } from "react";
import type { ActivityBlock as Block } from "../../api";

/** Below this height (px) a block shows app and title on one line; below `TEXT_MIN_PX`, no text. */
const ONE_LINE_PX = 40;
const TEXT_MIN_PX = 10;

interface Props {
  block: Block;
  top: number;
  height: number;
  focused: boolean;
}

export const ActivityBlock = memo(function ActivityBlock({ block, top, height, focused }: Props) {
  const classes = ["block", height < ONE_LINE_PX && "one-line", block.private && "private", block.covered && "dimmed", focused && "focused"];
  return (
    <div
      className={classes.filter(Boolean).join(" ")}
      data-block-id={block.id}
      aria-label={`${block.app}: ${block.title}`}
      style={{ top, height, left: `${(block.col / block.cols) * 100}%`, width: `${100 / block.cols}%` }}
    >
      {height >= TEXT_MIN_PX && (
        <>
          <span className="block-app">{block.app}</span>
          <span className="block-title mono">{block.title}</span>
        </>
      )}
    </div>
  );
});
