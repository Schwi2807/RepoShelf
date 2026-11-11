import type { LucideIcon } from 'lucide-react';
import type { ButtonHTMLAttributes, ReactNode, Ref } from 'react';

export type ButtonVariant = 'default' | 'primary' | 'danger' | 'invisible';

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: 'sm' | 'md';
  icon?: LucideIcon;
  /** Icon-only button: `children` is ignored, `label` becomes aria-label + title. */
  label?: string;
  trailing?: ReactNode;
  ref?: Ref<HTMLButtonElement>;
}

export function Button({
  variant = 'default',
  size = 'md',
  icon: Icon,
  label,
  trailing,
  className,
  children,
  type = 'button',
  ref,
  ...rest
}: ButtonProps) {
  const iconOnly = label !== undefined && children === undefined;
  const cls = [
    'btn',
    variant !== 'default' ? `btn--${variant}` : '',
    size === 'sm' ? 'btn--sm' : '',
    iconOnly ? 'btn--icon' : '',
    className ?? '',
  ]
    .filter(Boolean)
    .join(' ');
  return (
    <button ref={ref} type={type} className={cls} aria-label={label} title={label} {...rest}>
      {Icon && <Icon size={16} aria-hidden="true" />}
      {!iconOnly && children}
      {trailing}
    </button>
  );
}
