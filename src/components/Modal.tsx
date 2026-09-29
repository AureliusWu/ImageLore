export function Modal({
  title,
  description,
  children,
  onClose,
  onConfirm,
  confirmLabel,
  danger = false,
}: {
  title: string;
  description: string;
  children: React.ReactNode;
  onClose: () => void;
  onConfirm: () => void | Promise<void>;
  confirmLabel: string;
  danger?: boolean;
}) {
  return (
    <div
      className="modal-backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <section className="modal glass-surface" role="dialog" aria-modal="true">
        <div className="modal-head">
          <div>
            <strong>{title}</strong>
            <p>{description}</p>
          </div>
          <button className="icon-button" onClick={onClose}>
            ×
          </button>
        </div>
        <div className="modal-body">{children}</div>
        <div className="modal-actions">
          <button className="button secondary" onClick={onClose}>
            取消
          </button>
          <button className={`button ${danger ? "danger" : "primary"}`} onClick={onConfirm}>
            {confirmLabel}
          </button>
        </div>
      </section>
    </div>
  );
}
