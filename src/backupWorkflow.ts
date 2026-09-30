export const BACKUP_CHECK_INTERVAL_MS = 15 * 60 * 1000;

type BackupHost = {
  setInterval: (callback: () => void, milliseconds: number) => number;
  clearInterval: (id: number) => void;
  addEventListener: (event: "focus", callback: () => void) => void;
  removeEventListener: (event: "focus", callback: () => void) => void;
};

export function startBackupSchedule(
  host: BackupHost,
  ensureBackup: () => Promise<unknown>,
  onError: (error: unknown) => void,
) {
  let disposed = false;
  let pending = false;
  const check = () => {
    if (disposed || pending) return;
    pending = true;
    void Promise.resolve()
      .then(ensureBackup)
      .catch((error) => {
        if (!disposed) onError(error);
      })
      .finally(() => {
        pending = false;
      });
  };
  const timer = host.setInterval(check, BACKUP_CHECK_INTERVAL_MS);
  host.addEventListener("focus", check);
  check();
  return () => {
    disposed = true;
    host.clearInterval(timer);
    host.removeEventListener("focus", check);
  };
}
