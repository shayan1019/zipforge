import type { ReactNode } from "react";
import { X } from "lucide-react";

export default function Modal(props: {
  open: boolean;
  title: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  width?: number;
}) {
  const { open, title, onClose, children, footer, width } = props;
  if (!open) return null;
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div className="absolute inset-0 bg-zinc-950/45 backdrop-blur-[2px]" onClick={onClose} />
      <div className="card relative w-full shadow-2xl shadow-zinc-950/20" style={{ maxWidth: width ?? 560 }}>
        <div className="flex h-12 items-center justify-between border-b border-zinc-200 px-5 dark:border-zinc-800">
          <h2 className="text-sm font-semibold">{title}</h2>
          <button className="btn-icon" onClick={onClose} aria-label="close">
            <X size={16} />
          </button>
        </div>
        <div className="max-h-[65vh] overflow-auto px-5 py-4">{children}</div>
        {footer && (
          <div className="flex items-center justify-end gap-2 border-t border-zinc-200 px-5 py-3 dark:border-zinc-800">
            {footer}
          </div>
        )}
      </div>
    </div>
  );
}
