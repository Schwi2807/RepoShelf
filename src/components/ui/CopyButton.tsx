import { Check, Copy } from 'lucide-react';
import { useEffect, useRef, useState, type ReactNode } from 'react';
import { Button, type ButtonProps } from './Button';

export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

/** Copies `text`; shows a check mark for 1.5 s. */
export function CopyButton({
  text,
  label = 'Copy',
  children,
  size = 'sm',
  variant = 'invisible',
}: {
  text: string;
  label?: string;
  children?: ReactNode;
  size?: ButtonProps['size'];
  variant?: ButtonProps['variant'];
}) {
  const [ok, setOk] = useState(false);
  const t = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => () => {
    if (t.current) clearTimeout(t.current);
  }, []);
  return (
    <Button
      size={size}
      variant={variant}
      icon={ok ? Check : Copy}
      label={children === undefined ? (ok ? 'Copied' : label) : undefined}
      className={ok ? 'copy-ok' : undefined}
      onClick={async () => {
        if (await copyText(text)) {
          setOk(true);
          if (t.current) clearTimeout(t.current);
          t.current = setTimeout(() => setOk(false), 1500);
        }
      }}
    >
      {children}
    </Button>
  );
}
