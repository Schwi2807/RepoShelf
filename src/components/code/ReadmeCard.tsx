// README card (FR-17): header with book icon + file name + outline button (heading list), body = sanitised markdown
// through MarkdownBody (link rewrite, repo images as blob: URLs, highlighted fences). Hidden when there is no README.

import { BookOpen, List } from 'lucide-react';
import { useRef, useState } from 'react';
import { readmeRender } from '../../ipc/api';
import { qk, useQuery } from '../../ipc/query';
import { MarkdownBody } from '../../markdown/MarkdownBody';
import { Button } from '../ui/Button';
import { Shimmer } from '../ui/bits';
import { useDismiss } from '../ui/Popover';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { baseName, joinPath } from '../../util/paths';

export interface ReadmeCardProps {
  repoId: number;
  owner: string;
  repo: string;
  refName: string;
  commit: string;
  /** Directory being listed. */
  dir: string;
}

export function ReadmeCard({ repoId, owner, repo, refName, commit, dir }: ReadmeCardProps) {
  const readme = useQuery(qk('readme_render', repoId, commit, dir), () => readmeRender(repoId, commit, dir), { stable: true });
  const [open, setOpen] = useState(false);
  const card = useRef<HTMLElement>(null);
  const wrap = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  useDismiss(open, () => setOpen(false), [wrap], trigger);

  if (readme.loading) {
    return (
      <section className="box readme">
        <div className="box__header">
          <Shimmer width={120} />
        </div>
        <div className="box__body stack">
          <Shimmer width="50%" height={20} />
          <Shimmer width="90%" />
          <Shimmer width="80%" />
        </div>
      </section>
    );
  }
  const r = readme.data;
  if (!r) return null;
  const name = baseName(r.sourcePath) || 'README';
  const headings = r.headings.filter((h) => h.level <= 4);
  return (
    <section className="box readme" ref={card} aria-label={name} id="readme">
      <div className="box__header readme__header">
        <BookOpen size={16} aria-hidden="true" className="muted" />
        <Link to={href('blob', { owner, repo, ref: refName, path: joinPath(dir, name) })} className="readme__name">
          {name}
        </Link>
        <span className="spacer" />
        {headings.length > 1 && (
          <div className="dropdown" ref={wrap}>
            <Button
              ref={trigger}
              size="sm"
              variant="invisible"
              icon={List}
              label="Outline"
              aria-haspopup="dialog"
              aria-expanded={open}
              onClick={() => setOpen((o) => !o)}
            />
            {open && (
              <div className="popover popover--right outline" role="dialog" aria-label="Outline">
                <div className="popover__header">Outline</div>
                <ul className="outline__list">
                  {headings.map((h, i) => (
                    <li key={`${h.anchor}-${i}`}>
                      <button
                        type="button"
                        className={`outline__item outline__item--h${h.level}`}
                        onClick={() => {
                          setOpen(false);
                          const target = [...(card.current?.querySelectorAll<HTMLElement>('[id]') ?? [])].find((x) => x.id === h.anchor);
                          target?.scrollIntoView({ block: 'start' });
                        }}
                      >
                        {h.text}
                      </button>
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        )}
      </div>
      <div className="box__body readme__body">
        <MarkdownBody html={r.html} repoId={repoId} owner={owner} repo={repo} rev={refName} commit={commit} />
      </div>
    </section>
  );
}
