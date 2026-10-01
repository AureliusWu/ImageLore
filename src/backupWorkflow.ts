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
  let paused = false;
  let pending: Promise<unknown> | null = null;
  const check = () => {
    if (disposed || paused || pending) return;
    const task = Promise.resolve().then(ensureBackup);
    pending = task;
    void task
      .catch((error) => {
        if (!disposed) onError(error);
      })
      .finally(() => {
        if (pending === task) pending = null;
      });
  };
  const timer = host.setInterval(check, BACKUP_CHECK_INTERVAL_MS);
  host.addEventListener("focus", check);
  check();
  return Object.assign(
    () => {
      disposed = true;
      host.clearInterval(timer);
      host.removeEventListener("focus", check);
    },
    {
      async pauseAndWait() {
        paused = true;
        // Await the raw result so a failed automatic backup can keep the
        // window open; its normal scheduler error handler still reports it.
        await pending;
      },
      resume() {
        paused = false;
      },
    },
  );
}
