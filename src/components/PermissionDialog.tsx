import { api } from "../api";
import { fail, useStore } from "../state/store";
import { Modal } from "./Modal";

/** Shown while the tracker needs the Accessibility permission; closes itself once it is granted. */
export function PermissionDialog() {
  const needsPermission = useStore((s) => s.tracker?.state.kind === "needsPermission");
  const requestPermission = useStore((s) => s.requestPermission);
  const hidden = useStore((s) => s.permissionDialogHidden);
  const setHidden = useStore((s) => s.setPermissionDialogHidden);

  if (!needsPermission || hidden) return null;
  const openSettings = () => void api.openPrivacySettings().catch((e: unknown) => fail(e, "Could not open System Settings"));
  return (
    <Modal onClose={() => setHidden(true)} labelledBy="permission-title" className="permission-dialog">
      <h2 id="permission-title">Allow TimeTracker to read window titles</h2>
      <p>
        TimeTracker records which app and window are in front, so your day shows what you worked on. macOS shares window titles only with
        apps you allow under Privacy &amp; Security → Accessibility.
      </p>
      <p className="muted">Never captured: keystrokes, screen contents, screenshots, audio or the clipboard. Everything stays on this Mac.</p>
      <div className="actions">
        <button type="button" className="btn" onClick={openSettings}>
          Open System Settings
        </button>
        <button type="button" className="btn btn-primary" autoFocus onClick={() => void requestPermission()}>
          Check again
        </button>
      </div>
    </Modal>
  );
}
