// Smaller repos: a cloned Python repo (non-GitHub host), an imported Go repo, an empty repo, a missing repo and the
// security README repo (hostile content the frontend must neutralise as a second line of defence).

import { DAY, HOUR, NOW } from './gen';
import type { RepoDef } from './model';

const base = {
  isBare: false,
  license: null,
  fetchable: false,
  canFastForward: null,
  dirty: null,
  remoteBranches: [],
  markdown: {},
};

export function quill(): RepoDef {
  const readme = {
    html: `<h1><a href="#user-content-quill" id="user-content-quill" rel="noopener noreferrer"></a>quill</h1>
<p>Write release notes from your commit history. <code>pipx install quill-notes</code></p>
<pre><code class="language-python">from quill import notes
print(notes.since("v1.4.0"))
</code></pre>
`,
    headings: [{ level: 1, text: 'quill', anchor: 'user-content-quill' }],
    sourcePath: 'README.md',
  };
  return {
    ...base,
    summary: {
      id: 3,
      owner: 'inkwell',
      name: 'quill',
      source: 'cloned',
      path: 'C:\\Users\\demo\\Documents\\RepoShelf\\codeberg.org\\inkwell\\quill.git',
      description: 'Release-notes generator for conventional commits.',
      topics: ['python', 'cli', 'changelog'],
      homepage: null,
      originUrl: 'https://codeberg.org/inkwell/quill.git',
      originHost: 'codeberg.org',
      defaultBranch: 'trunk',
      primaryLanguage: 'Python',
      lastCommitAt: NOW - 9 * DAY,
      addedAt: NOW - 9 * DAY,
      clonedAt: NOW - 9 * DAY,
      lastOpenedAt: NOW - 6 * DAY,
      lastFetchedAt: NOW - 6 * DAY,
      status: { type: 'ok' },
    },
    isBare: true,
    head: { type: 'branch', name: 'trunk' },
    license: 'GPL-3.0',
    fetchable: true,
    canFastForward: false,
    files: {
      'pyproject.toml': '[project]\nname = "quill-notes"\nversion = "1.5.0"\nrequires-python = ">=3.10"\n',
      'README.md': '# quill\n',
      'quill/__init__.py': '"""Release notes from commits."""\n\n__version__ = "1.5.0"\n',
      'quill/notes.py':
        'import subprocess\n\n\ndef since(tag: str) -> str:\n    """Return markdown notes for commits after `tag`."""\n    out = subprocess.run(["git", "log", f"{tag}..HEAD", "--oneline"], capture_output=True, text=True)\n    return "\\n".join(f"- {line}" for line in out.stdout.splitlines())\n',
      'tests/test_notes.py': 'def test_placeholder():\n    assert True\n',
    },
    history: { seed: 'quill', count: 64, newest: NOW - 9 * DAY },
    branches: [{ name: 'trunk', at: 0 }],
    remoteBranches: [{ name: 'origin/trunk', at: 0 }],
    tags: [
      { name: 'v1.5.0', at: 0 },
      { name: 'v1.4.0', at: 12 },
    ],
    readmes: { '': readme },
    markdown: { 'README.md': readme },
    languages: [
      { name: 'Python', bytes: 9_800, percent: 92.0 },
      { name: 'TOML', bytes: 850, percent: 8.0 },
    ],
  };
}

export function dotgo(): RepoDef {
  return {
    ...base,
    summary: {
      id: 4,
      owner: 'local',
      name: 'beacon',
      source: 'imported',
      path: 'C:\\Code\\beacon',
      description: 'Tiny HTTP health-check daemon.',
      topics: ['go', 'monitoring'],
      homepage: null,
      originUrl: null,
      originHost: null,
      defaultBranch: 'main',
      primaryLanguage: 'Go',
      lastCommitAt: NOW - 70 * DAY,
      addedAt: NOW - 30 * DAY,
      clonedAt: null,
      lastOpenedAt: null,
      lastFetchedAt: null,
      status: { type: 'ok' },
    },
    head: { type: 'detached', oid: '' },
    files: {
      'go.mod': 'module example.local/beacon\n\ngo 1.24\n',
      'main.go':
        'package main\n\nimport (\n\t"log"\n\t"net/http"\n)\n\nfunc main() {\n\thttp.HandleFunc("/healthz", func(w http.ResponseWriter, _ *http.Request) {\n\t\tw.WriteHeader(http.StatusOK)\n\t})\n\tlog.Fatal(http.ListenAndServe(":8080", nil))\n}\n',
      Makefile: 'build:\n\tgo build ./...\n',
      'README.txt': 'beacon\n======\n\nRun `make build`, then ./beacon.\n',
    },
    history: { seed: 'beacon', count: 23, newest: NOW - 70 * DAY },
    branches: [{ name: 'main', at: 2 }],
    tags: [],
    readmes: {
      '': {
        html: '<pre class="readme-plain">beacon\n======\n\nRun `make build`, then ./beacon.\n</pre>',
        headings: [],
        sourcePath: 'README.txt',
      },
    },
    languages: [
      { name: 'Go', bytes: 2_100, percent: 88.0 },
      { name: 'Makefile', bytes: 290, percent: 12.0 },
    ],
  };
}

export function emptyRepo(): RepoDef {
  return {
    ...base,
    summary: {
      id: 5,
      owner: 'local',
      name: 'scratchpad',
      source: 'imported',
      path: 'C:\\Code\\scratchpad',
      description: null,
      topics: [],
      homepage: null,
      originUrl: null,
      originHost: null,
      defaultBranch: 'main',
      primaryLanguage: null,
      lastCommitAt: null,
      addedAt: NOW - 3 * DAY,
      clonedAt: null,
      lastOpenedAt: null,
      lastFetchedAt: null,
      status: { type: 'empty' },
    },
    head: { type: 'unborn', name: 'main' },
    files: {},
    history: null,
    branches: [],
    tags: [],
    readmes: {},
    languages: [],
  };
}

export function missingRepo(): RepoDef {
  return {
    ...base,
    summary: {
      id: 6,
      owner: 'orbit-labs',
      name: 'telemetry',
      source: 'imported',
      path: 'E:\\Archive\\telemetry',
      description: 'Ground-station telemetry decoder.',
      topics: ['c', 'embedded'],
      homepage: null,
      originUrl: 'https://gitlab.example.com/orbit-labs/telemetry.git',
      originHost: 'gitlab.example.com',
      defaultBranch: 'main',
      primaryLanguage: 'C',
      lastCommitAt: NOW - 200 * DAY,
      addedAt: NOW - 90 * DAY,
      clonedAt: null,
      lastOpenedAt: NOW - 60 * DAY,
      lastFetchedAt: null,
      status: { type: 'missing' },
    },
    head: { type: 'branch', name: 'main' },
    files: {},
    history: null,
    branches: [],
    tags: [],
    readmes: {},
    languages: [],
  };
}

/**
 * Security README. The backend's ammonia pass would already strip most of this; the HTML here deliberately keeps
 * what the frontend must handle itself (remote images, external links) plus a few things ammonia never lets through
 * (`javascript:`, `data:` link, inline handler) to prove the frontend's second line of defence.
 */
export function hostile(): RepoDef {
  const html = `<h1><a href="#user-content-security-fixture" id="user-content-security-fixture" rel="noopener noreferrer"></a>Security fixture</h1>
<p>This README is hostile on purpose. Nothing below may run, load from the network or navigate the app.</p>
<p><img src="https://images.example.com/tracker.png" alt="remote image"></p>
<p><a href="javascript:alert('js-link')">javascript link</a></p>
<p><a href="data:text/html;base64,PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg==">data html link</a></p>
<p><a href="//evil.example.com/x">scheme-relative link</a></p>
<p><img src="x.png" onerror="alert('onerror')" alt="handler"></p>
<p><a href="https://example.com/docs" rel="noopener noreferrer">external link</a> and <a href="mailto:security@example.com">mail</a></p>
<p><a href="rsrepo:/docs/page.md" rel="noopener noreferrer">relative link</a> and <a href="#user-content-security-fixture" rel="noopener noreferrer">fragment</a></p>
<p><img src="rsrepo:/svg-with-script.svg" alt="relative image"></p>
<p><img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=" alt="png data"></p>
`;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="120" height="40"><script>alert('svg')</script><rect width="120" height="40" fill="#c33"/></svg>`;
  const readme = {
    html,
    headings: [{ level: 1, text: 'Security fixture', anchor: 'user-content-security-fixture' }],
    sourcePath: 'README.md',
  };
  return {
    ...base,
    summary: {
      id: 7,
      owner: 'sandbox',
      name: 'hostile-readme',
      source: 'imported',
      path: 'C:\\Code\\hostile-readme',
      description: 'Renderer test: every trick in one README.',
      topics: ['security', 'test'],
      homepage: null,
      originUrl: null,
      originHost: null,
      defaultBranch: 'main',
      primaryLanguage: 'Markdown',
      lastCommitAt: NOW - 14 * DAY,
      addedAt: NOW - 14 * DAY,
      clonedAt: null,
      lastOpenedAt: null,
      lastFetchedAt: null,
      status: { type: 'ok' },
    },
    head: { type: 'branch', name: 'main' },
    files: {
      'README.md': '# Security fixture\n',
      'svg-with-script.svg': { svg },
      'docs/page.md': '# Page\n',
    },
    history: { seed: 'hostile', count: 3, newest: NOW - 14 * DAY },
    branches: [{ name: 'main', at: 0 }],
    tags: [],
    readmes: { '': readme },
    markdown: { 'README.md': readme },
    languages: [{ name: 'Markdown', bytes: 1_200, percent: 100 }],
  };
}

/** A small repo created at runtime (import, clone, locate). */
export function genericRepo(
  id: number,
  owner: string,
  name: string,
  source: 'imported' | 'cloned',
  path: string,
  originUrl: string | null,
): RepoDef {
  const host = originUrl ? new URL(originUrl).hostname : null;
  return {
    ...base,
    summary: {
      id,
      owner,
      name,
      source,
      path,
      description: null,
      topics: [],
      homepage: null,
      originUrl,
      originHost: host,
      defaultBranch: 'main',
      primaryLanguage: 'JavaScript',
      lastCommitAt: NOW - 4 * DAY,
      addedAt: Math.floor(Date.now() / 1000),
      clonedAt: source === 'cloned' ? Math.floor(Date.now() / 1000) : null,
      lastOpenedAt: null,
      lastFetchedAt: source === 'cloned' ? Math.floor(Date.now() / 1000) : null,
      status: { type: 'ok' },
    },
    isBare: source === 'cloned',
    fetchable: originUrl !== null && originUrl.startsWith('https://'),
    canFastForward: source === 'cloned' ? false : null,
    head: { type: 'branch', name: 'main' },
    files: {
      'README.md': `# ${name}\n`,
      'index.js': "export const hello = () => 'hello';\n",
      'package.json': `{\n  "name": "${name}",\n  "version": "1.0.0"\n}\n`,
    },
    history: { seed: `${owner}/${name}`, count: 17, newest: NOW - 4 * DAY },
    branches: [{ name: 'main', at: 0 }],
    remoteBranches: originUrl ? [{ name: 'origin/main', at: 0 }] : [],
    tags: [],
    readmes: {
      '': {
        html: `<h1><a href="#user-content-${name}" id="user-content-${name}" rel="noopener noreferrer"></a>${name}</h1>\n`,
        headings: [{ level: 1, text: name, anchor: `user-content-${name}` }],
        sourcePath: 'README.md',
      },
    },
    languages: [{ name: 'JavaScript', bytes: 600, percent: 100 }],
  };
}

/** Recently cloned, still "fresh" repo to make the dashboard list longer (HOUR used for recency). */
export function freshClone(): RepoDef {
  return {
    ...base,
    summary: {
      id: 8,
      owner: 'tidewater',
      name: 'harbor-cli',
      source: 'cloned',
      path: 'C:\\Users\\demo\\Documents\\RepoShelf\\github.com\\tidewater\\harbor-cli.git',
      description: 'Command-line client for container registries.',
      topics: ['go', 'cli', 'containers'],
      homepage: null,
      originUrl: 'https://github.com/tidewater/harbor-cli.git',
      originHost: 'github.com',
      defaultBranch: 'main',
      primaryLanguage: 'Go',
      lastCommitAt: NOW - 5 * HOUR,
      addedAt: NOW - 1 * HOUR,
      clonedAt: NOW - 1 * HOUR,
      lastOpenedAt: null,
      lastFetchedAt: NOW - 1 * HOUR,
      status: { type: 'ok' },
      // v0.2.0 (D-102): About comes from GitHub only (the user never edited it)
      aboutUser: { description: null, homepage: null, topics: [] },
      aboutGithub: {
        description: 'Command-line client for container registries.',
        homepage: null,
        topics: ['go', 'cli', 'containers'],
        htmlUrl: 'https://github.com/tidewater/harbor-cli',
        fetchedAt: NOW - 1 * HOUR,
      },
    },
    isBare: true,
    fetchable: true,
    canFastForward: false,
    head: { type: 'branch', name: 'main' },
    files: {
      'go.mod': 'module example.com/tidewater/harbor-cli\n\ngo 1.24\n',
      'cmd/harbor/main.go': 'package main\n\nfunc main() {}\n',
      'README.md': '# harbor-cli\n',
    },
    history: { seed: 'harbor', count: 310, newest: NOW - 5 * HOUR },
    branches: [{ name: 'main', at: 0 }],
    remoteBranches: [{ name: 'origin/main', at: 0 }],
    tags: [{ name: 'v0.4.2', at: 3 }],
    readmes: {},
    languages: [{ name: 'Go', bytes: 120_000, percent: 100 }],
  };
}
