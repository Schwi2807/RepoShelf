// Edit About (FR-20, S10): description, homepage, topics chip input. Stored in the app DB only (`repo_set_metadata`);
// the repository's files are never touched. Client rules mirror `validate::topics`; server errors are shown as sent.
// v0.2.0: prefilled with and saving `aboutUser` only; GitHub's fetched values appear as placeholders.

import { Info, Save } from 'lucide-react';
import { useId, useState, type KeyboardEvent } from 'react';
import { repoSetMetadata } from '../../ipc/api';
import { errorText } from '../../ipc/errors';
import type { RepoSummary } from '../../ipc/types';
import { applySummary } from '../../state/repoCache';
import { addTopics, MAX_DESCRIPTION, MAX_TOPICS } from '../../util/topics';
import { Button } from '../ui/Button';
import { Chip } from '../ui/bits';
import { Dialog } from '../ui/Dialog';

export function EditAboutDialog({ summary, onClose }: { summary: RepoSummary; onClose: () => void }) {
  const ids = useId();
  // v0.2.0 (D-102): the user's own values, never the effective ones (which may be GitHub's); GitHub's show as hints.
  const user = summary.aboutUser;
  const gh = summary.aboutGithub;
  const [description, setDescription] = useState(user.description ?? '');
  const [homepage, setHomepage] = useState(user.homepage ?? '');
  const [topics, setTopics] = useState<string[]>(user.topics);
  const [draft, setDraft] = useState('');
  const [topicError, setTopicError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const commitDraft = (): string[] | null => {
    if (draft.trim().length === 0) return topics;
    const r = addTopics(topics, draft);
    setTopics(r.list);
    setTopicError(r.error);
    if (r.error) return null;
    setDraft('');
    return r.list;
  };

  const onTopicKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter' || e.key === ',' || e.key === ' ') {
      e.preventDefault();
      commitDraft();
    } else if (e.key === 'Backspace' && draft.length === 0 && topics.length > 0) {
      setTopics(topics.slice(0, -1));
      setTopicError(null);
    }
  };

  const save = async () => {
    const list = commitDraft();
    if (list === null) return;
    setBusy(true);
    setError(null);
    try {
      const r = await repoSetMetadata(summary.id, description.trim() || null, homepage.trim() || null, list);
      applySummary(r);
      onClose();
    } catch (e) {
      setError(errorText(e));
      setBusy(false);
    }
  };

  const tooLong = description.trim().length > MAX_DESCRIPTION;

  return (
    <Dialog
      title="Edit repository details"
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" icon={Save} disabled={busy || tooLong} onClick={save}>
            Save changes
          </Button>
        </>
      }
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
      >
        <div className="field">
          <label className="label" htmlFor={`${ids}-desc`}>
            Description
          </label>
          <textarea
            id={`${ids}-desc`}
            className="input textarea"
            rows={3}
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder={gh?.description ? `From GitHub: ${gh.description}` : 'Short description of this repository'}
          />
          <p className={tooLong ? 'field__error' : 'field__hint'}>
            {description.trim().length} / {MAX_DESCRIPTION} characters
          </p>
        </div>
        <div className="field">
          <label className="label" htmlFor={`${ids}-home`}>
            Website
          </label>
          <input
            id={`${ids}-home`}
            className="input"
            value={homepage}
            onChange={(e) => setHomepage(e.target.value)}
            placeholder={gh?.homepage ? `From GitHub: ${gh.homepage}` : 'https://example.org'}
            spellCheck={false}
            autoComplete="off"
          />
        </div>
        <div className="field">
          <label className="label" htmlFor={`${ids}-topics`}>
            Topics <span className="muted small">({topics.length}/{MAX_TOPICS})</span>
          </label>
          <div className="chip-input" onClick={() => document.getElementById(`${ids}-topics`)?.focus()}>
            {topics.map((t) => (
              <Chip
                key={t}
                removeLabel={`Remove topic ${t}`}
                onRemove={() => {
                  setTopics(topics.filter((x) => x !== t));
                  setTopicError(null);
                }}
              >
                {t}
              </Chip>
            ))}
            <input
              id={`${ids}-topics`}
              className="chip-input__field"
              value={draft}
              onChange={(e) => {
                setDraft(e.target.value);
                setTopicError(null);
              }}
              onKeyDown={onTopicKey}
              onBlur={() => void commitDraft()}
              placeholder={topics.length === 0 ? 'Add topics (Enter or comma)' : ''}
              spellCheck={false}
              autoComplete="off"
              aria-invalid={topicError !== null}
            />
          </div>
          {topicError ? (
            <p className="field__error" role="alert">
              {topicError}
            </p>
          ) : (
            <p className="field__hint">Lowercase letters, digits and hyphens. Saved on this PC only; the repository is not changed.</p>
          )}
        </div>
        {gh && (
          <p className="notice" data-testid="about-github-note">
            <Info size={16} aria-hidden="true" />
            <span>
              GitHub info is shown for any field you leave empty
              {gh.topics.length > 0 && topics.length === 0 ? ` (topics: ${gh.topics.join(', ')})` : ''}. Your values win when set.
            </span>
          </p>
        )}
      </form>
      {error && (
        <p className="notice notice--danger mt-3" role="alert">
          {error}
        </p>
      )}
    </Dialog>
  );
}
