// Explore search page (v0.3.0 M10b): `#/explore/search?q=&sort=&page=`. One `explore_search` call per submit (never per
// keystroke: GitHub's search bucket is 10 a minute without a token). Results in GitHub's list style: name, description,
// topics, stars, forks, language, licence, updated; each result has Open (the Explore repository page) and Save to
// shelf / Open in shelf. Typing `owner/name` or pasting a github.com address also offers a direct Open.

import { Compass, GitFork, Search, Star } from 'lucide-react';
import { useState, type FormEvent } from 'react';
import { ExploreFailure, ExploreOff } from '../../components/explore/ExploreErrors';
import { ExploreTopicChips } from '../../components/explore/ExploreAbout';
import { QuotaBar } from '../../components/explore/QuotaBar';
import { SaveToShelfButton } from '../../components/explore/SaveToShelf';
import { LangLabel } from '../../components/dashboard/meta';
import { Box } from '../../components/layout/Sidebar';
import { Badge, Empty, Shimmer } from '../../components/ui/bits';
import { Button } from '../../components/ui/Button';
import { exploreSearch } from '../../ipc/api';
import { qk, useQuery } from '../../ipc/query';
import type { ExploreRepo, ExploreSort } from '../../ipc/types';
import { track } from '../../source';
import { href, navigate } from '../../router/router';
import { Link } from '../../router/Link';
import type { Route } from '../../router/routes';
import { useSettings } from '../../state/settings';
import { formatCount } from '../../util/format';
import { RelativeTime } from '../../components/ui/RelativeTime';

const SORTS: { id: ExploreSort | ''; label: string }[] = [
  { id: '', label: 'Best match' },
  { id: 'stars', label: 'Most stars' },
  { id: 'forks', label: 'Most forks' },
  { id: 'updated', label: 'Recently updated' },
];

const EXAMPLES = ['language:rust cli', 'topic:wasm stars:>1000', 'ripgrep', 'user:sharkdp'];

/** `owner/name`, `github.com/owner/name[.git]` or a pasted https address → the repository it names. */
export function parseRepoAddress(input: string): { owner: string; name: string } | null {
  let t = input.trim();
  t = t.replace(/^https?:\/\/(www\.)?github\.com\//i, '').replace(/^github\.com\//i, '');
  t = t.replace(/[?#].*$/, '').replace(/\/+$/, '');
  const m = /^([A-Za-z0-9](?:[A-Za-z0-9-]{0,38}))\/([A-Za-z0-9._-]{1,100})$/.exec(t.replace(/\.git$/i, ''));
  if (!m || m[2] === '.' || m[2] === '..') return null;
  return { owner: m[1], name: m[2] };
}

function sortOf(v: string | undefined): ExploreSort | null {
  return v === 'stars' || v === 'forks' || v === 'updated' ? v : null;
}

export default function ExploreSearch({ route }: { route: Route }) {
  const { settings, loaded } = useSettings();
  if (!loaded) return <main className="container page" />;
  if (!settings.exploreEnabled) return <ExploreOff />;
  return <SearchPage route={route} />;
}

function SearchPage({ route }: { route: Route }) {
  const q = (route.query.q ?? '').trim();
  const sort = sortOf(route.query.sort);
  const pageNo = Math.max(1, Number.parseInt(route.query.page ?? '1', 10) || 1);
  const [text, setText] = useState(q);
  const [sortText, setSortText] = useState<ExploreSort | ''>(sort ?? '');
  // the route (not the typed text) decides what is searched: a new query only on submit
  const [seen, setSeen] = useState(q);
  if (seen !== q) {
    setSeen(q);
    setText(q);
    setSortText(sort ?? '');
  }

  const results = useQuery(q ? qk('explore_search', q, sort, pageNo) : null, () => track(exploreSearch(q, sort, pageNo)));
  const direct = parseRepoAddress(text);

  const submit = (e: FormEvent) => {
    e.preventDefault();
    const t = text.trim();
    if (t.length === 0) return;
    navigate(href('exploreSearch', {}, { q: t, sort: sortText || null }));
  };

  return (
    <main className="container page explore-search" data-testid="explore-search">
      <div className="stack">
        <QuotaBar bucket="search" />
        <h1 className="page-title">Explore GitHub</h1>
        <form className="explore-search__form" role="search" onSubmit={submit} data-testid="explore-search-form">
          <div className="filter-input explore-search__input">
            <Search size={14} aria-hidden="true" className="filter-input__icon" />
            <input
              className="input filter-input__field"
              type="search"
              placeholder="Search repositories, or type owner/name"
              aria-label="Search GitHub repositories"
              data-testid="explore-search-input"
              value={text}
              onChange={(e) => setText(e.target.value)}
              spellCheck={false}
              autoComplete="off"
              autoFocus
            />
          </div>
          <select
            className="input explore-search__sort"
            aria-label="Sort results"
            value={sortText}
            onChange={(e) => setSortText(e.target.value as ExploreSort | '')}
          >
            {SORTS.map((s) => (
              <option key={s.id} value={s.id}>
                {s.label}
              </option>
            ))}
          </select>
          <Button type="submit" variant="primary" icon={Search} disabled={text.trim().length === 0}>
            Search
          </Button>
        </form>
        {direct && (
          <Link to={href('code', { explore: true, owner: direct.owner, repo: direct.name })} className="explore-direct" data-testid="explore-direct">
            <Compass size={16} aria-hidden="true" />
            Open <strong>{direct.owner}/{direct.name}</strong> on GitHub
          </Link>
        )}
        {q === '' ? (
          <Box>
            <Empty icon={Compass} title="Find a repository on GitHub">
              <p>
                Browse code, commits, branches, tags and releases without cloning. Searches use GitHub's search (10 a minute without a
                token); everything you open is saved for a while, so opening it again costs nothing.
              </p>
              <p className="explore-search__examples">
                Try:{' '}
                {EXAMPLES.map((x) => (
                  <Link key={x} to={href('exploreSearch', {}, { q: x })} className="chip">
                    {x}
                  </Link>
                ))}
              </p>
            </Empty>
          </Box>
        ) : results.error ? (
          <ExploreFailure inline error={results.error} what="The search" onRetry={results.refetch} />
        ) : !results.data ? (
          <Box>
            {[1, 2, 3, 4].map((i) => (
              <div key={i} className="box__row xresult">
                <Shimmer width="30%" />
                <Shimmer width="70%" />
              </div>
            ))}
          </Box>
        ) : (
          <>
            <p className="muted small" data-testid="explore-result-count">
              {formatCount(results.data.totalCount)} {results.data.totalCount === 1 ? 'repository' : 'repositories'} for{' '}
              <span className="mono">{results.data.query}</span>
              {results.data.incomplete && ' (GitHub says the results may be incomplete)'}
            </p>
            {results.data.items.length === 0 ? (
              <Box>
                <Empty icon={Search} title="No repositories found">
                  Try fewer words, or a qualifier such as <span className="mono">language:go</span> or <span className="mono">topic:cli</span>.
                </Empty>
              </Box>
            ) : (
              <ul className="box xresults" data-testid="explore-results">
                {results.data.items.map((r) => (
                  <ResultRow key={r.fullName} r={r} />
                ))}
              </ul>
            )}
            {(pageNo > 1 || results.data.hasMore) && (
              <nav className="pager" aria-label="Search pages">
                {pageNo > 1 ? (
                  <Link to={href('exploreSearch', {}, { q, sort, page: pageNo > 2 ? pageNo - 1 : null })} className="btn btn--sm">
                    Previous
                  </Link>
                ) : (
                  <span />
                )}
                <span className="muted small">Page {pageNo}</span>
                {results.data.hasMore ? (
                  <Link to={href('exploreSearch', {}, { q, sort, page: pageNo + 1 })} className="btn btn--sm">
                    Next
                  </Link>
                ) : (
                  <span />
                )}
              </nav>
            )}
          </>
        )}
      </div>
    </main>
  );
}

function ResultRow({ r }: { r: ExploreRepo }) {
  const to = href('code', { explore: true, owner: r.owner, repo: r.name });
  return (
    <li className="xresult" data-testid="explore-result" data-repo={r.fullName}>
      <div className="xresult__main">
        <div className="xresult__title">
          <Link to={to} className="xresult__name">
            {r.owner}/<strong>{r.name}</strong>
          </Link>
          {r.archived && <Badge tone="attention">Archived</Badge>}
          {r.fork && <Badge>Fork</Badge>}
        </div>
        {r.description && <p className="xresult__desc">{r.description}</p>}
        <ExploreTopicChips topics={r.topics.slice(0, 8)} />
        <div className="xresult__meta muted small">
          <span title="Stars">
            <Star size={14} aria-hidden="true" /> {formatCount(r.stars)}
          </span>
          <span title="Forks">
            <GitFork size={14} aria-hidden="true" /> {formatCount(r.forks)}
          </span>
          {r.language && <LangLabel name={r.language} />}
          {r.license && <span>{r.license}</span>}
          {r.pushedAt !== null && (
            <span>
              Updated <RelativeTime secs={r.pushedAt} />
            </span>
          )}
        </div>
      </div>
      <div className="xresult__actions">
        <Link to={to} className="btn btn--sm" data-testid="explore-open">
          Open
        </Link>
        <SaveToShelfButton repo={r} />
      </div>
    </li>
  );
}
