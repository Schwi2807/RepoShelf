// Newer / Older (FR-26): links, so Back/Forward and reloads work; both keep the pinned `?tip=`. A capped path scan
// (20,000 commits looked at without filling the page) turns Older into "Search older commits", which continues
// from the returned walk position (D-37).

import { ChevronLeft, ChevronRight, Search } from 'lucide-react';
import { Link } from '../../router/Link';

export function Pager({ newer, older, capped }: { newer: string | null; older: string | null; capped: boolean }) {
  if (newer === null && older === null) return null;
  return (
    <nav className="pager" aria-label="Commit pages">
      <div className="btn-group">
        {newer ? (
          <Link to={newer} className="btn" rel="prev">
            <ChevronLeft size={16} aria-hidden="true" />
            Newer
          </Link>
        ) : (
          <button type="button" className="btn" disabled>
            <ChevronLeft size={16} aria-hidden="true" />
            Newer
          </button>
        )}
        {older ? (
          <Link to={older} className="btn" rel="next">
            {capped ? (
              <>
                <Search size={16} aria-hidden="true" />
                Search older commits
              </>
            ) : (
              <>
                Older
                <ChevronRight size={16} aria-hidden="true" />
              </>
            )}
          </Link>
        ) : (
          <button type="button" className="btn" disabled>
            Older
            <ChevronRight size={16} aria-hidden="true" />
          </button>
        )}
      </div>
    </nav>
  );
}
