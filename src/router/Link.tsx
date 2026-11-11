import type { AnchorHTMLAttributes, MouseEvent, ReactNode } from 'react';
import { navigate } from './router';

export interface LinkProps extends Omit<AnchorHTMLAttributes<HTMLAnchorElement>, 'href'> {
  /** In-app hash, e.g. from `href(…)`. */
  to: string;
  replace?: boolean;
  children?: ReactNode;
}

/** In-app link. Every click is routed in-app (single-window app: modifier clicks never open a new window). */
export function Link({ to, replace, onClick, children, ...rest }: LinkProps) {
  const handle = (e: MouseEvent<HTMLAnchorElement>) => {
    onClick?.(e);
    if (e.defaultPrevented || e.button !== 0) return;
    e.preventDefault();
    navigate(to, { replace });
  };
  return (
    <a href={to} onClick={handle} {...rest}>
      {children}
    </a>
  );
}
