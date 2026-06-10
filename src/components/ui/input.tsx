import * as React from "react";
import { cn } from "@/lib/utils";

const Input = React.forwardRef<HTMLInputElement, React.InputHTMLAttributes<HTMLInputElement>>(
  ({ className, type, ...props }, ref) => (
    <input
      type={type}
      className={cn(
        "focusable glass flex h-10 w-full rounded-xl px-4 py-2 text-sm text-ink transition-colors",
        "placeholder:text-ink-faint selection:bg-accent-soft",
        "hover:border-edge-strong focus:border-edge-strong",
        "disabled:cursor-not-allowed disabled:opacity-50",
        className,
      )}
      ref={ref}
      {...props}
    />
  ),
);
Input.displayName = "Input";

const Textarea = React.forwardRef<HTMLTextAreaElement, React.TextareaHTMLAttributes<HTMLTextAreaElement>>(
  ({ className, ...props }, ref) => (
    <textarea
      className={cn(
        "focusable glass flex min-h-24 w-full rounded-xl px-4 py-3 text-sm text-ink transition-colors",
        "placeholder:text-ink-faint hover:border-edge-strong focus:border-edge-strong",
        "disabled:cursor-not-allowed disabled:opacity-50 resize-y",
        className,
      )}
      ref={ref}
      {...props}
    />
  ),
);
Textarea.displayName = "Textarea";

export { Input, Textarea };
