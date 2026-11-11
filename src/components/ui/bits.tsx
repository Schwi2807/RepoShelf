// Small presentational pieces: Badge, Chip, Counter, Shimmer, Avatar, Empty, Spinner.

import { LoaderCircle, type LucideIcon } from 'lucide-react';
import type { ReactNode } from 'react';
import { hash32 } from '../../util/hash';
import { initials } from '../../util/format';

export type Tone = 'neutral' | 'accent' | 'indigo' | 'success' | 'danger' | 'attention';

export function Badge({ tone = 'neutral', icon: Icon, children, title }: { tone?: Tone; icon?: LucideIcon; children: ReactNode; title?: string }) {
  return (
    <span className={`badge${tone !== 'neutral' ? ` badge--${tone}` : ''}`} title={title}>
      {Icon && <Icon size={12} aria-hidden="true" />}
      {children}
    </span>
  );
}

export function Chip({ children, onRemove, removeLabel }: { children: ReactNode; onRemove?: () => void; removeLabel?: string }) {
  return (
    <span className="chip">
      {children}
      {onRemove && (
        <button type="button" className="chip__remove" aria-label={removeLabel ?? 'Remove'} onClick={onRemove}>
          ×
        </button>
      )}
    </span>
  );
}

export function Counter({ n }: { n: number | string | null | undefined }) {
  if (n === null || n === undefined) return null;
  return <span className="counter">{n}</span>;
}

/** Loading placeholder; width in px or %. */
export function Shimmer({ width = 120, height, block }: { width?: number | string; height?: number; block?: boolean }) {
  return (
    <span
      className={`shimmer${block ? ' shimmer--block' : ''}`}
      style={{ width: typeof width === 'number' ? `${width}px` : width, height }}
      aria-hidden="true"
    />
  );
}

export function Spinner({ size = 16, label = 'Loading' }: { size?: number; label?: string }) {
  return <LoaderCircle size={size} className="spin" aria-label={label} role="status" />;
}

/** Initials circle (no images, no network). Colour from a hash of the name. */
export function Avatar({ name, size = 'md' }: { name: string; size?: 'sm' | 'md' }) {
  const c = hash32(name.toLowerCase()) % 5;
  return (
    <span className={`avatar avatar--${size} avatar--c${c}`} aria-hidden="true" title={name}>
      {initials(name)}
    </span>
  );
}

export function Empty({
  icon: Icon,
  title,
  children,
  actions,
}: {
  icon?: LucideIcon;
  title: ReactNode;
  children?: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <div className="empty">
      {Icon && <Icon size={32} className="empty__icon" aria-hidden="true" />}
      <h2 className="empty__title">{title}</h2>
      {children && <div className="empty__body">{children}</div>}
      {actions && <div className="row">{actions}</div>}
    </div>
  );
}
