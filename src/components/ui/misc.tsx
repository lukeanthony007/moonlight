/** Small UI primitives: Label, Badge, Switch, Checkbox, Tabs, Tooltip, Spinner. */

import * as React from "react";
import * as LabelPrimitive from "@radix-ui/react-label";
import * as SwitchPrimitive from "@radix-ui/react-switch";
import * as CheckboxPrimitive from "@radix-ui/react-checkbox";
import * as TabsPrimitive from "@radix-ui/react-tabs";
import * as TooltipPrimitive from "@radix-ui/react-tooltip";
import { Check, Loader2 } from "lucide-react";
import { cn } from "@/lib/utils";

export const Label = React.forwardRef<
  React.ComponentRef<typeof LabelPrimitive.Root>,
  React.ComponentPropsWithoutRef<typeof LabelPrimitive.Root>
>(({ className, ...props }, ref) => (
  <LabelPrimitive.Root
    ref={ref}
    className={cn("text-xs font-medium uppercase tracking-wider text-ink-dim", className)}
    {...props}
  />
));
Label.displayName = "Label";

export function Badge({
  className,
  variant = "default",
  ...props
}: React.HTMLAttributes<HTMLSpanElement> & { variant?: "default" | "accent" | "success" | "danger" }) {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-[11px] font-medium",
        variant === "default" && "bg-glass-strong text-ink-dim border border-edge",
        variant === "accent" && "bg-accent-soft text-accent border border-accent/25",
        variant === "success" && "bg-success/10 text-success border border-success/25",
        variant === "danger" && "bg-danger/10 text-danger border border-danger/25",
        className,
      )}
      {...props}
    />
  );
}

export const Switch = React.forwardRef<
  React.ComponentRef<typeof SwitchPrimitive.Root>,
  React.ComponentPropsWithoutRef<typeof SwitchPrimitive.Root>
>(({ className, ...props }, ref) => (
  <SwitchPrimitive.Root
    ref={ref}
    className={cn(
      "focusable peer inline-flex h-6 w-11 shrink-0 cursor-pointer items-center rounded-full border border-edge transition-colors",
      "data-[state=checked]:bg-accent data-[state=unchecked]:bg-glass-strong disabled:cursor-not-allowed disabled:opacity-50",
      className,
    )}
    {...props}
  >
    <SwitchPrimitive.Thumb
      className={cn(
        "pointer-events-none block size-4.5 rounded-full bg-ink shadow-lg transition-transform",
        "data-[state=checked]:translate-x-5.5 data-[state=unchecked]:translate-x-0.5 data-[state=checked]:bg-[#0d0d18]",
      )}
    />
  </SwitchPrimitive.Root>
));
Switch.displayName = "Switch";

export const Checkbox = React.forwardRef<
  React.ComponentRef<typeof CheckboxPrimitive.Root>,
  React.ComponentPropsWithoutRef<typeof CheckboxPrimitive.Root>
>(({ className, ...props }, ref) => (
  <CheckboxPrimitive.Root
    ref={ref}
    className={cn(
      "focusable peer size-5 shrink-0 cursor-pointer rounded-md border border-edge-strong bg-glass transition-colors",
      "data-[state=checked]:border-accent data-[state=checked]:bg-accent disabled:cursor-not-allowed disabled:opacity-50",
      className,
    )}
    {...props}
  >
    <CheckboxPrimitive.Indicator className="flex items-center justify-center text-[#0d0d18]">
      <Check className="size-3.5" strokeWidth={3} />
    </CheckboxPrimitive.Indicator>
  </CheckboxPrimitive.Root>
));
Checkbox.displayName = "Checkbox";

export const Tabs = TabsPrimitive.Root;

export const TabsList = React.forwardRef<
  React.ComponentRef<typeof TabsPrimitive.List>,
  React.ComponentPropsWithoutRef<typeof TabsPrimitive.List>
>(({ className, ...props }, ref) => (
  <TabsPrimitive.List
    ref={ref}
    className={cn("glass inline-flex h-10 items-center justify-center rounded-xl p-1 gap-1", className)}
    {...props}
  />
));
TabsList.displayName = "TabsList";

export const TabsTrigger = React.forwardRef<
  React.ComponentRef<typeof TabsPrimitive.Trigger>,
  React.ComponentPropsWithoutRef<typeof TabsPrimitive.Trigger>
>(({ className, ...props }, ref) => (
  <TabsPrimitive.Trigger
    ref={ref}
    className={cn(
      "focusable inline-flex items-center justify-center whitespace-nowrap rounded-lg px-4 py-1.5 text-sm text-ink-dim transition-all cursor-pointer",
      "data-[state=active]:bg-glass-strong data-[state=active]:text-ink data-[state=active]:shadow-sm",
      className,
    )}
    {...props}
  />
));
TabsTrigger.displayName = "TabsTrigger";

export const TabsContent = TabsPrimitive.Content;

export function TooltipProvider({ children }: { children: React.ReactNode }) {
  return <TooltipPrimitive.Provider delayDuration={400}>{children}</TooltipPrimitive.Provider>;
}

export function Tooltip({
  content,
  children,
  side = "top",
}: {
  content: React.ReactNode;
  children: React.ReactNode;
  side?: "top" | "bottom" | "left" | "right";
}) {
  return (
    <TooltipPrimitive.Root>
      <TooltipPrimitive.Trigger asChild>{children}</TooltipPrimitive.Trigger>
      <TooltipPrimitive.Portal>
        <TooltipPrimitive.Content
          side={side}
          sideOffset={6}
          className="glass-strong z-50 rounded-lg bg-[#16161f]/95 px-3 py-1.5 text-xs text-ink card-shadow"
        >
          {content}
        </TooltipPrimitive.Content>
      </TooltipPrimitive.Portal>
    </TooltipPrimitive.Root>
  );
}

export function Spinner({ className }: { className?: string }) {
  return <Loader2 className={cn("size-5 animate-spin text-accent", className)} aria-label="Loading" />;
}

/** Section heading used across settings panes. */
export function SectionTitle({ title, description }: { title: string; description?: string }) {
  return (
    <div className="mb-5">
      <h2 className="text-xl font-semibold tracking-tight">{title}</h2>
      {description && <p className="mt-1 text-sm text-ink-dim">{description}</p>}
    </div>
  );
}

/** Labeled row in settings panes. */
export function SettingRow({
  label,
  description,
  children,
}: {
  label: string;
  description?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-6 py-3.5 border-b border-edge last:border-b-0">
      <div className="min-w-0">
        <div className="text-sm font-medium text-ink">{label}</div>
        {description && <div className="mt-0.5 text-xs text-ink-dim">{description}</div>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}
