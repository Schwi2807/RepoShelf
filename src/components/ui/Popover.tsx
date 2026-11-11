import { useEffect, useRef, type ReactNode, type Ref, type RefObject } from 'react';

/** Close on outside pointer-down and on Escape (focus goes back to the anchor). */
export function useDismiss(
  open: boolean,
  onClose: () => void,
  refs: RefObject<HTMLElement | null>[],
  restoreFocusTo?: RefObject<HTMLElement | null>,
): void {
  const closeRef = useRef(onClose);
  closeRef.current = onClose;
  useEffect(() => {
    if (!open) return;
    const down = (e: PointerEvent) => {
      const t = e.target as Node | null;
      if (refs.some((r) => r.current && t && r.current.contains(t))) return;
      closeRef.current();
    };
    const key = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        closeRef.current();
        restoreFocusTo?.current?.focus();
      }
    };
    document.addEventListener('pointerdown', down, true);
    document.addEventListener('keydown', key);
    return () => {
      document.removeEventListener('pointerdown', down, true);
      document.removeEventListener('keydown', key);
    };
    // refs are stable objects; only `open` matters
  }, [open]);
}

export interface PopoverProps {
  open: boolean;
  align?: 'left' | 'right';
  className?: string;
  label?: string;
  children: ReactNode;
  ref?: Ref<HTMLDivElement>;
}

/** Positioned panel under its (relatively positioned) parent. Open/close handled by the owner + `useDismiss`. */
export function Popover({ open, align = 'left', className, label, children, ref }: PopoverProps) {
  if (!open) return null;
  return (
    <div ref={ref} className={`popover popover--${align} ${className ?? ''}`} role="dialog" aria-label={label}>
      {children}
    </div>
  );
}
