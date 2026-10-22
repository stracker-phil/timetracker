import { memo, useEffect } from "react";
import { useStore } from "../state/store";

interface Props {
  /** Shown as its initial while there is no icon. */
  app: string;
  /** `null` for apps without one, such as external sources. */
  bundleId: string | null;
  /** 16 in timeline blocks, 20 in cards and lists. */
  size?: 16 | 20;
}

/** The installed app's icon, or the app's initial. */
export const AppIcon = memo(function AppIcon({ app, bundleId, size = 20 }: Props) {
  const url = useStore((s) => (bundleId ? s.appIcons[bundleId] : null));
  const requestAppIcon = useStore((s) => s.requestAppIcon);
  useEffect(() => {
    if (bundleId) requestAppIcon(bundleId);
  }, [bundleId, requestAppIcon]);

  const className = `app-icon app-icon--${size}`;
  if (url) return <img className={className} src={url} alt="" draggable={false} />;
  return (
    <span className={`${className} app-icon--initial`} aria-hidden="true">
      {[...app.trim()][0]?.toUpperCase()}
    </span>
  );
});
