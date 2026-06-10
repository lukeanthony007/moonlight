import { useState } from "react";
import { motion } from "framer-motion";
import { ArrowRight, Gamepad2, Moon, MonitorPlay, Sparkles } from "lucide-react";
import * as api from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/misc";
import { useLibraryStore } from "@/stores/libraryStore";
import { useScanStore } from "@/stores/scanStore";

interface OnboardingProps {
  onDone: (openEmulatorWizard: boolean) => void;
}

/** First-run experience: welcome → choose how to import games. */
export function Onboarding({ onDone }: OnboardingProps) {
  const [step, setStep] = useState<"welcome" | "sources">("welcome");
  const [busy, setBusy] = useState(false);
  const startScan = useScanStore((s) => s.start);
  const refresh = useLibraryStore((s) => s.refresh);

  async function finish(openWizard: boolean, scanSteam: boolean) {
    setBusy(true);
    try {
      await api.completeOnboarding();
      if (scanSteam) {
        await startScan({ kind: "steam" });
      }
      await refresh();
      onDone(openWizard);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-base">
      <div
        aria-hidden
        className="pointer-events-none absolute inset-0 bg-[radial-gradient(ellipse_at_top,rgba(139,140,240,0.14),transparent_55%)]"
      />
      {step === "welcome" && (
        <motion.div
          className="relative flex max-w-lg flex-col items-center gap-6 text-center"
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
        >
          <div className="card-shadow flex size-20 items-center justify-center rounded-panel bg-accent-soft">
            <Moon className="size-10 text-accent" />
          </div>
          <div>
            <h1 className="text-4xl font-bold tracking-tight">Moonlight</h1>
            <p className="mx-auto mt-3 max-w-md text-ink-dim">
              Your games — Steam, ROMs, emulators and native titles — in one beautiful, local-first library.
              No account. No cloud. Just play.
            </p>
          </div>
          <Button size="lg" onClick={() => setStep("sources")}>
            Get started <ArrowRight />
          </Button>
        </motion.div>
      )}

      {step === "sources" && (
        <motion.div
          className="relative w-full max-w-2xl px-6"
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
        >
          <h2 className="mb-2 text-center text-2xl font-bold tracking-tight">Where are your games?</h2>
          <p className="mb-8 text-center text-sm text-ink-dim">
            Pick a starting point — you can add more sources anytime in Settings.
          </p>
          <div className="grid grid-cols-3 gap-4">
            <SourceCard
              icon={<MonitorPlay className="size-7" />}
              title="Steam"
              description="Import installed Steam games with their artwork. No login needed."
              actionLabel={busy ? "Importing…" : "Import Steam library"}
              disabled={busy}
              onClick={() => finish(false, true)}
            />
            <SourceCard
              icon={<Gamepad2 className="size-7" />}
              title="Emulators & ROMs"
              description="Connect RetroArch, Dolphin, PCSX2, Ryujinx or any emulator, then scan your ROM folders."
              actionLabel="Connect an emulator"
              disabled={busy}
              onClick={() => finish(true, false)}
            />
            <SourceCard
              icon={<Sparkles className="size-7" />}
              title="Start empty"
              description="Skip for now and add games manually whenever you like."
              actionLabel="Open my library"
              disabled={busy}
              onClick={() => finish(false, false)}
            />
          </div>
          {busy && (
            <div className="mt-6 flex justify-center">
              <Spinner />
            </div>
          )}
        </motion.div>
      )}
    </div>
  );
}

function SourceCard({
  icon,
  title,
  description,
  actionLabel,
  disabled,
  onClick,
}: {
  icon: React.ReactNode;
  title: string;
  description: string;
  actionLabel: string;
  disabled: boolean;
  onClick: () => void;
}) {
  return (
    <div className="glass card-shadow flex flex-col items-start gap-3 rounded-panel p-6">
      <div className="flex size-12 items-center justify-center rounded-xl bg-accent-soft text-accent">{icon}</div>
      <h3 className="font-semibold">{title}</h3>
      <p className="flex-1 text-xs leading-relaxed text-ink-dim">{description}</p>
      <Button variant="glass" size="sm" className="w-full" disabled={disabled} onClick={onClick}>
        {actionLabel}
      </Button>
    </div>
  );
}
