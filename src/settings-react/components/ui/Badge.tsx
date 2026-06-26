import { type HTMLAttributes } from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "../../lib/cn";

const badgeVariants = cva(
  "inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-xs font-semibold transition-colors",
  {
    variants: {
      variant: {
        neutral: "border-border bg-surface-soft text-ink-muted",
        success:
          "border-state-success-border bg-state-success-bg text-state-success-fg",
        error:
          "border-state-error-border bg-state-error-bg text-state-error-fg",
        warning:
          "border-[#e6c34a] bg-[#fff8e1] text-[#7a5a00]",
        accent: "border-accent bg-accent-soft text-accent-strong",
      },
    },
    defaultVariants: { variant: "neutral" },
  },
);

export interface BadgeProps
  extends HTMLAttributes<HTMLSpanElement>,
    VariantProps<typeof badgeVariants> {}

export function Badge({ className, variant, ...props }: BadgeProps) {
  return (
    <span className={cn(badgeVariants({ variant }), className)} {...props} />
  );
}
