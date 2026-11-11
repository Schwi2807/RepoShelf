// Layout primitives: two-column page with a right sidebar (stacks under 1000 px), sidebar sections, bordered box.

import type { ReactNode } from 'react';

export function Columns({ children, ratio = 'sidebar' }: { children: ReactNode; ratio?: 'sidebar' | '3-1' }) {
  return <div className={ratio === '3-1' ? 'cols cols--3-1' : 'cols'}>{children}</div>;
}

export function Sidebar({ children, label }: { children: ReactNode; label?: string }) {
  return (
    <aside className="sidebar" aria-label={label}>
      {children}
    </aside>
  );
}

export function SidebarSection({ title, actions, children }: { title: ReactNode; actions?: ReactNode; children: ReactNode }) {
  return (
    <section className="sidebar__section">
      <div className="row">
        <h2 className="sidebar__title">{title}</h2>
        <span className="spacer" />
        {actions}
      </div>
      {children}
    </section>
  );
}

export function Box({ header, children, className }: { header?: ReactNode; children?: ReactNode; className?: string }) {
  return (
    <div className={`box ${className ?? ''}`}>
      {header !== undefined && <div className="box__header">{header}</div>}
      {children}
    </div>
  );
}
