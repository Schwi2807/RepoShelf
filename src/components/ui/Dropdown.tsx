import { ChevronDown, type LucideIcon } from 'lucide-react';
import { useRef, useState, type KeyboardEvent, type ReactNode, type RefObject } from 'react';
import { Button, type ButtonProps } from './Button';
import { useDismiss } from './Popover';

export interface MenuItem {
  key: string;
  label: ReactNode;
  icon?: LucideIcon;
  onSelect: () => void;
  danger?: boolean;
  disabled?: boolean;
}

export type MenuEntry = MenuItem | 'separator' | { note: ReactNode; key: string };

export interface DropdownProps {
  /** Button text; omit with `label` for an icon-only trigger. */
  text?: ReactNode;
  label?: string;
  icon?: LucideIcon;
  caret?: boolean;
  align?: 'left' | 'right';
  items: MenuEntry[];
  header?: ReactNode;
  variant?: ButtonProps['variant'];
  size?: ButtonProps['size'];
  /** Custom trigger (the global header uses its own buttons). */
  renderTrigger?: (p: {
    open: boolean;
    toggle: () => void;
    ref: RefObject<HTMLButtonElement | null>;
    onKeyDown: (e: KeyboardEvent) => void;
  }) => ReactNode;
}

function isItem(e: MenuEntry): e is MenuItem {
  return typeof e === 'object' && 'onSelect' in e;
}

/** Button + menu. Keyboard: Enter/Space/ArrowDown open, arrows move, Escape closes and refocuses the trigger. */
export function Dropdown({
  text,
  label,
  icon,
  caret = true,
  align = 'left',
  items,
  header,
  variant,
  size,
  renderTrigger,
}: DropdownProps) {
  const [open, setOpen] = useState(false);
  const wrap = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLUListElement>(null);
  useDismiss(open, () => setOpen(false), [wrap], trigger);

  const focusItem = (dir: 1 | -1 | 0) => {
    const els = [...(menu.current?.querySelectorAll<HTMLButtonElement>('button.menu__item:not(:disabled)') ?? [])];
    if (els.length === 0) return;
    const i = els.indexOf(document.activeElement as HTMLButtonElement);
    const next = dir === 0 ? 0 : i < 0 ? (dir === 1 ? 0 : els.length - 1) : (i + dir + els.length) % els.length;
    els[next].focus();
  };

  const openAndFocus = () => {
    setOpen(true);
    requestAnimationFrame(() => focusItem(0));
  };

  const onTriggerKey = (e: KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      openAndFocus();
    }
  };

  const onMenuKey = (e: KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      focusItem(1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      focusItem(-1);
    } else if (e.key === 'Tab') {
      setOpen(false);
    }
  };

  const toggle = () => setOpen((o) => !o);

  return (
    <div className="dropdown" ref={wrap}>
      {renderTrigger ? (
        renderTrigger({ open, toggle, ref: trigger, onKeyDown: onTriggerKey })
      ) : (
        <Button
          ref={trigger}
          variant={variant}
          size={size}
          icon={icon}
          label={text === undefined ? label : undefined}
          aria-haspopup="menu"
          aria-expanded={open}
          onClick={toggle}
          onKeyDown={onTriggerKey}
          trailing={caret && text !== undefined ? <ChevronDown size={14} className="btn__caret" aria-hidden="true" /> : undefined}
        >
          {text}
        </Button>
      )}
      {open && (
        <div className={`popover popover--${align}`}>
          {header && <div className="popover__header">{header}</div>}
          <ul className="menu" role="menu" ref={menu} onKeyDown={onMenuKey}>
            {items.map((it, i) =>
              it === 'separator' ? (
                <li key={`sep-${i}`} className="menu__sep" role="separator" />
              ) : isItem(it) ? (
                <li key={it.key} role="none">
                  <button
                    type="button"
                    role="menuitem"
                    className={`menu__item${it.danger ? ' menu__item--danger' : ''}`}
                    disabled={it.disabled}
                    onClick={() => {
                      setOpen(false);
                      trigger.current?.focus();
                      it.onSelect();
                    }}
                  >
                    {it.icon && <it.icon size={16} aria-hidden="true" />}
                    {it.label}
                  </button>
                </li>
              ) : (
                <li key={it.key} className="menu__note" role="none">
                  {it.note}
                </li>
              ),
            )}
          </ul>
        </div>
      )}
    </div>
  );
}
