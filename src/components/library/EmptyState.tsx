import type { ReactNode } from "react";
import { Gamepad2 } from "lucide-react";

export function EmptyState({
  title,
  description,
  action,
  icon,
}: {
  title: string;
  description?: string;
  action?: ReactNode;
  icon?: ReactNode;
}) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-4 p-10 text-center animate-fade-in">
      <div className="flex size-20 items-center justify-center rounded-panel glass">
        {icon ?? <Gamepad2 className="size-9 text-ink-faint" />}
      </div>
      <div>
        <h2 className="text-lg font-semibold">{title}</h2>
        {description && <p className="mx-auto mt-1.5 max-w-md text-sm text-ink-dim">{description}</p>}
      </div>
      {action}
    </div>
  );
}
