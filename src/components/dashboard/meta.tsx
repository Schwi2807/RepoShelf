// Small repo metadata pieces shared by the dashboard and the Code tab: language dot, topic chips.

import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { langClass } from '../../util/lang-colors';

export function LangDot({ name }: { name: string }) {
  return <span className={`lang-dot ${langClass(name)}`} aria-hidden="true" />;
}

/** Language dot + name ("● Rust"). */
export function LangLabel({ name }: { name: string }) {
  return (
    <span className="lang-label">
      <LangDot name={name} />
      {name}
    </span>
  );
}

/** Topic chips; each links to the dashboard filtered by that topic. */
export function TopicChips({ topics }: { topics: string[] }) {
  if (topics.length === 0) return null;
  return (
    <div className="chips">
      {topics.map((t) => (
        <Link key={t} to={href('dashboard', {}, { q: t })} className="chip" title={`Repositories matching “${t}”`}>
          {t}
        </Link>
      ))}
    </div>
  );
}
