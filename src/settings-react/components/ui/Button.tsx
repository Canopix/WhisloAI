import { forwardRef, type ButtonHTMLAttributes } from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "../../lib/cn";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-xl text-sm font-semibold transition-all duration-duration-fast ease-ease-standard disabled:pointer-events-none disabled:opacity-50 focus-visible:outline-none min-h-[44px] px-4 cursor-pointer",
  {
    variants: {
      variant: {
        primary:
          "bg-accent text-accent-contrast border border-accent hover:brightness-110 hover:-translate-y-px",
        secondary:
          "bg-surface text-accent-strong border border-border-soft hover:border-accent hover:bg-surface-soft hover:-translate-y-px",
        ghost:
          "bg-transparent text-ink-muted hover:bg-surface-soft hover:text-ink",
        danger:
          "bg-state-error-bg text-state-error-fg border border-state-error-border hover:brightness-95",
        outline:
          "bg-transparent border border-border text-ink hover:bg-surface-soft",
      },
      size: {
        default: "h-11 px-4",
        sm: "h-9 px-3 text-xs",
        lg: "h-12 px-6 text-base",
        icon: "h-10 w-10 p-0",
      },
    },
    defaultVariants: {
      variant: "secondary",
      size: "default",
    },
  },
);

export interface ButtonProps
  extends ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, ...props }, ref) => {
    return (
      <button
        ref={ref}
        className={cn(buttonVariants({ variant, size }), className)}
        {...props}
      />
    );
  },
);
Button.displayName = "Button";

export { buttonVariants };
