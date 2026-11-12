// M12 (v0.2.0 frontend): Back/Forward buttons (F1), remote images through the backend (F2), release cards with
// assets + downloads (F4), auto-download options (F4), Network settings + Edit About on `aboutUser` (F6), jobs
// popover texts for the new kinds.

import { act } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { resultLines, jobTarget } from './components/layout/JobsPopover';
import { downloadJob } from './components/refs/ReleaseAssets';
import type { JobInfo } from './ipc/types';
import { rewriteHtml } from './markdown/linkRewrite';
import { activeRequests, loadRemoteImage, MAX_PARALLEL, resetRemoteImages } from './markdown/remoteImages';
import { historyPos } from './router/router';
import { parseSizeCap } from './screens/Settings';
import { applyJobUpdate } from './state/jobs';
import { openDialog } from './state/ui';
import { boot, click, go, installHarness, settle, typeInto } from './test/appHarness';
import { notesReport } from './util/jobText';

installHarness();

type Invoke = (cmd: string, args: Record<string, unknown>) => Promise<unknown>;

/** Record every IPC call of the mock backend (and optionally replace some). */
function spy(backend: { invoke: unknown }, override?: (cmd: string, args: Record<string, unknown>) => Promise<unknown> | undefined) {
  const calls: { cmd: string; args: Record<string, unknown> }[] = [];
  const orig = (backend.invoke as Invoke).bind(backend);
  (backend as { invoke: Invoke }).invoke = (cmd, args) => {
    calls.push({ cmd, args });
    return override?.(cmd, args) ?? orig(cmd, args);
  };
  return calls;
}

/** IntersectionObserver that reports every observed element as visible (happy-dom's never fires). */
class VisibleObserver {
  constructor(private cb: IntersectionObserverCallback) {}
  observe(el: Element) {
    queueMicrotask(() => this.cb([{ isIntersecting: true, target: el } as IntersectionObserverEntry], this as unknown as IntersectionObserver));
  }
  unobserve() {}
  disconnect() {}
  takeRecords() {
    return [];
  }
}

afterEach(() => {
  vi.unstubAllGlobals();
  resetRemoteImages();
});

const buttonByText = (root: ParentNode, text: string) =>
  [...root.querySelectorAll<HTMLButtonElement>('button')].find((b) => b.textContent?.trim() === text);

describe('F1 Back / Forward buttons', () => {
  it('disabled at the start, enabled after navigating, forward after back; a push cuts forward', async () => {
    const { host } = await boot('', '#/');
    const backBtn = () => host.querySelector<HTMLButtonElement>('button[aria-label^="Back"]')!;
    const fwdBtn = () => host.querySelector<HTMLButtonElement>('button[aria-label^="Forward"]')!;
    expect(backBtn().disabled).toBe(true);
    expect(fwdBtn().disabled).toBe(true);
    await go('#/settings', 100);
    await go('#/lumen-ui/lumen', 200);
    expect(backBtn().disabled).toBe(false);
    expect(fwdBtn().disabled).toBe(true);
    expect(history.state).toMatchObject({ idx: 2 });

    const wait = () => new Promise<void>((r) => window.addEventListener('popstate', () => r(), { once: true }));
    let p = wait();
    await click(backBtn());
    await act(async () => p);
    await settle(50);
    expect(location.hash).toBe('#/settings');
    expect(historyPos()).toEqual({ canBack: true, canForward: true });
    expect(fwdBtn().disabled).toBe(false);

    p = wait();
    await click(fwdBtn());
    await act(async () => p);
    await settle(50);
    expect(location.hash).toBe('#/lumen-ui/lumen');
    expect(fwdBtn().disabled).toBe(true);

    p = wait();
    await click(backBtn());
    await act(async () => p);
    await go('#/', 50);
    expect(historyPos()).toEqual({ canBack: true, canForward: false });

    // Alt+Left still goes back (router keyboard handler)
    p = wait();
    await act(async () => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', altKey: true }));
    });
    await act(async () => p);
    expect(location.hash).toBe('#/settings');
  });
});

describe('F2 remote images', () => {
  it('rewriteHtml keeps the chip and hands back the inert image (inside its link)', () => {
    const r = rewriteHtml(
      '<p><a href="https://ci.example.com"><img src="https://img.shields.io/badge/ci-ok" alt="ci" width="80"></a> <img src="https://x.example.org/a.png" alt="a"></p>',
      { owner: 'o', repo: 'r', rev: 'main' },
    );
    const div = document.createElement('div');
    div.append(r.fragment);
    expect(div.querySelectorAll('img').length).toBe(0);
    expect(div.querySelectorAll('a.md-remote-image').length).toBe(2);
    expect(r.remoteImages.map((x) => x.url)).toEqual(['https://img.shields.io/badge/ci-ok', 'https://x.example.org/a.png']);
    const [badge, plain] = r.remoteImages;
    expect(badge.restore.tagName).toBe('A');
    expect(badge.restore.getAttribute('data-external')).toBe('1');
    expect(badge.img.hasAttribute('src')).toBe(false);
    expect(badge.img.getAttribute('width')).toBe('80');
    expect(plain.restore).toBe(plain.img);
  });

  it('README images load through image_fetch → image_bytes → blob:, lazily; a failing host keeps its chip', async () => {
    vi.stubGlobal('IntersectionObserver', VisibleObserver);
    let n = 0;
    vi.spyOn(URL, 'createObjectURL').mockImplementation(() => `blob:mock-${++n}`);
    vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => undefined);
    const { host, backend } = await boot('', '#/');
    const calls = spy(backend);
    await go('#/lumen-ui/lumen', 600);
    const body = host.querySelector('.markdown')!;
    const imgs = [...body.querySelectorAll('img')];
    console.log('IMGS', imgs.map((i) => i.alt).join('|'), body.querySelectorAll('a.md-remote-image').length);
    expect(imgs.length).toBe(8);
    expect(imgs.every((i) => i.src.startsWith('blob:mock-'))).toBe(true);
    // the logo keeps its presentational size; the badge sits in its link again
    expect(body.querySelector('img[alt="Lumen logo"]')?.getAttribute('width')).toBe('96');
    expect(body.querySelector('a[href="https://github.com/lumen-ui/lumen/actions"] > img[alt="build"]')).not.toBeNull();
    const chips = [...body.querySelectorAll('a.md-remote-image')];
    expect(chips.map((c) => c.textContent)).toEqual(['Remote image: fail.example.com']);
    expect(chips[0].classList.contains('md-remote-image--failed')).toBe(true);
    expect(calls.filter((c) => c.cmd === 'image_fetch').length).toBe(10);
    expect(calls.filter((c) => c.cmd === 'image_bytes').length).toBe(9);
  });

  it('nothing is requested without visibility (no IntersectionObserver callback) or with the setting off', async () => {
    const { host, backend } = await boot('', '#/');
    const calls = spy(backend);
    await go('#/lumen-ui/lumen', 400);
    expect(host.querySelectorAll('.markdown a.md-remote-image').length).toBe(10);
    expect(calls.some((c) => c.cmd === 'image_fetch')).toBe(false);

    vi.stubGlobal('IntersectionObserver', VisibleObserver);
    await go('#/settings', 200);
    await click(host.querySelector('input[name="loadRemoteImages"]'));
    await settle(50);
    await go('#/lumen-ui/lumen', 400);
    expect(host.querySelectorAll('.markdown a.md-remote-image').length).toBe(10);
    expect(calls.some((c) => c.cmd === 'image_fetch')).toBe(false);
  });

  it(`never more than ${MAX_PARALLEL} requests at once; same URL shared; skipped when no longer wanted`, async () => {
    const { backend } = await boot('', '#/');
    let peak = 0;
    const gates: (() => void)[] = [];
    const calls = spy(backend, (cmd) => {
      if (cmd !== 'image_fetch') return undefined;
      peak = Math.max(peak, activeRequests());
      return new Promise((resolve) => gates.push(() => resolve({ key: 'f'.repeat(64), mime: 'image/png', size: 1 })));
    });
    const bytes = spy(backend, (cmd) => (cmd === 'image_bytes' ? Promise.resolve(new ArrayBuffer(1)) : undefined));
    void bytes;
    const urls = Array.from({ length: 10 }, (_, i) => `https://img.example.org/${i}.png`);
    const all = urls.map((u) => loadRemoteImage(u));
    const dup = loadRemoteImage(urls[0]);
    const skipped = loadRemoteImage('https://img.example.org/gone.png', () => false);
    await settle(10);
    expect(gates.length).toBe(MAX_PARALLEL);
    while (gates.length > 0) {
      gates.shift()!();
      await settle(5);
    }
    await Promise.all(all);
    expect(await dup).toBe(await all[0]);
    await expect(skipped).rejects.toThrow();
    expect(peak).toBeLessThanOrEqual(MAX_PARALLEL);
    expect(calls.filter((c) => c.cmd === 'image_fetch').length).toBe(10);
    // a second view of the same image is served from memory
    await loadRemoteImage(urls[3]);
    expect(calls.filter((c) => c.cmd === 'image_fetch').length).toBe(10);
  });
});

describe('F4 release cards + assets', () => {
  it('GitHub-like card: tag chip, commit link, Open on GitHub, Assets with sizes/counts/for this PC, source links', async () => {
    const { host } = await boot('?gh', '#/lumen-ui/lumen/releases');
    await settle(300);
    const card = host.querySelector('.rcard[data-tag="v1.1.0"]')!;
    expect(card.querySelector('.rcard__tagchip')?.getAttribute('href')).toBe('#/lumen-ui/lumen/tree/refs%2Ftags%2Fv1.1.0');
    expect(card.querySelector('.rcard__oid')?.getAttribute('href')).toMatch(/^#\/lumen-ui\/lumen\/commit\/[0-9a-f]{40}$/);
    expect(card.querySelector('.rcard__gh')?.getAttribute('href')).toBe('https://github.com/lumen-ui/lumen/releases/tag/v1.1.0');
    expect(card.querySelector('.markdown a.user-mention')?.textContent).toBe('@mira-dev');
    const rows = [...card.querySelectorAll('.rasset')];
    expect(rows.map((r) => r.querySelector('.rasset__name')?.textContent)).toEqual([
      'lumen-1.1.0-x86_64-pc-windows-msvc.zip',
      'lumen-1.1.0-x86_64-unknown-linux-gnu.tar.gz',
      'lumen-1.1.0-aarch64-apple-darwin.tar.gz',
      'SHA256SUMS',
      'Source code (zip)',
      'Source code (tar.gz)',
    ]);
    expect(card.querySelector('.rassets__summary .counter')?.textContent).toBe('6');
    expect(rows[0].classList.contains('rasset--pc')).toBe(true);
    expect(rows[0].textContent).toContain('For this PC');
    expect(rows[0].querySelector('.rasset__size')?.textContent).toBe('2.3 MB');
    expect(rows[0].querySelector('.rasset__count')?.textContent).toBe('318 downloads');
    expect(rows[3].querySelector('.rasset__count')?.textContent).toBe('');
    // Open on GitHub asks first (ConfirmExternalLink)
    await click(rows[1].querySelector('button[aria-label^="Open "]'));
    expect(document.querySelector('[role="dialog"]')?.textContent).toContain('lumen-1.1.0-x86_64-unknown-linux-gnu.tar.gz');
  });

  it('without the GitHub list only the source rows and a hint show; other hosts show no assets', async () => {
    const { host } = await boot('', '#/lumen-ui/lumen/releases');
    await settle(300);
    const card = host.querySelector('.rcard[data-tag="v1.1.0"]')!;
    expect(card.querySelectorAll('.rasset').length).toBe(2);
    expect(card.querySelector('.rassets__note')?.textContent).toContain('after the GitHub release info is loaded');
    await go('#/inkwell/quill/releases', 300);
    expect(host.querySelector('.rassets')).toBeNull();
  });

  it('Download → job progress in the row → Downloaded + Show in folder (release_reveal)', async () => {
    const { host, backend } = await boot('?gh', '#/lumen-ui/lumen/releases');
    const calls = spy(backend);
    await settle(300);
    const row = () => host.querySelector('.rcard[data-tag="v1.1.0"] .rasset[data-asset="lumen-1.1.0-x86_64-pc-windows-msvc.zip"]')!;
    await click(row().querySelector('button[aria-label^="Download "]'));
    expect(calls.find((c) => c.cmd === 'release_download')?.args).toEqual({ repoId: 2, tag: 'v1.1.0', asset: 'lumen-1.1.0-x86_64-pc-windows-msvc.zip' });
    await settle(600);
    expect(row().textContent).toContain('Downloaded');
    await click(buttonByText(row(), 'Show in folder'));
    await settle(20);
    expect(calls.find((c) => c.cmd === 'release_reveal')?.args).toEqual({ repoId: 2, tag: 'v1.1.0', asset: 'lumen-1.1.0-x86_64-pc-windows-msvc.zip' });
    expect(row().querySelector('[role="alert"]')).toBeNull();
  });

  it('a running download shows progress + Cancel; a failed one shows the error + Retry', () => {
    const base: JobInfo = {
      id: 77,
      kind: { type: 'releaseDownload', repoId: 2, tag: 'v1.1.0', asset: 'a.zip' },
      state: 'running',
      progress: { type: 'downloading', bytes: 512, total: 1024 },
      title: 'Download a.zip (v1.1.0)',
      result: null,
      error: null,
      startedAt: 0,
    };
    expect(downloadJob([base], 2, 'v1.1.0', 'a.zip')?.id).toBe(77);
    expect(downloadJob([base], 2, 'v1.1.0', 'b.zip')).toBeUndefined();
    expect(jobTarget(base)).toBe('a.zip (v1.1.0)');
    expect(jobTarget({ ...base, kind: { type: 'githubSync', repoId: 2, autoDownload: null }, title: 'GitHub info lumen-ui/lumen' })).toBe('lumen-ui/lumen');
    expect(
      resultLines({
        ...base,
        state: 'done',
        result: { type: 'downloaded', repoId: 2, tag: 'v1.1.0', file: 'a.zip', path: 'C:\\x\\a.zip', size: 2_412_800, sha256Checked: true, already: false },
      }),
    ).toEqual(['Downloaded a.zip (2.3 MB, SHA-256 checked).']);
    expect(resultLines({ ...base, kind: { type: 'githubSync', repoId: 2, autoDownload: null }, state: 'done', result: { type: 'synced', repoId: 2, notes: [] } })).toEqual([
      'GitHub info updated.',
    ]);
    expect(notesReport(['no release download: no file for this PC', '', 'Rate limited.'])).toEqual(['No release download: no file for this PC.', 'Rate limited.']);
  });

  it('the row follows a job from the jobs store (progress, failure)', async () => {
    const { host } = await boot('?gh', '#/lumen-ui/lumen/releases');
    await settle(300);
    const job: JobInfo = {
      id: 9001,
      kind: { type: 'releaseDownload', repoId: 2, tag: 'v1.1.0', asset: 'SHA256SUMS' },
      state: 'running',
      progress: { type: 'downloading', bytes: 156, total: 312 },
      title: 'Download SHA256SUMS (v1.1.0)',
      result: null,
      error: null,
      startedAt: 0,
    };
    await act(async () => applyJobUpdate(job));
    const row = () => host.querySelector('.rcard[data-tag="v1.1.0"] .rasset[data-asset="SHA256SUMS"]')!;
    expect(row().querySelector('[role="progressbar"]')?.getAttribute('aria-valuenow')).toBe('50');
    expect(row().textContent).toContain('Downloading: 50%');
    expect(buttonByText(row(), 'Cancel')).toBeDefined();
    await act(async () => applyJobUpdate({ ...job, state: 'failed', progress: null, error: 'SHA256SUMS is larger than the download limit (1 MB).' }));
    expect(row().querySelector('[role="alert"]')?.textContent).toContain('larger than the download limit');
    expect(buttonByText(row(), 'Retry')).toBeDefined();
  });

  it('jobs popover: a finished download offers Show in folder', async () => {
    const { host, backend } = await boot('?gh', '#/lumen-ui/lumen/releases');
    const calls = spy(backend);
    await settle(300);
    await click(host.querySelector('.rcard[data-tag="v1.1.0"] button[aria-label="Download SHA256SUMS"]'));
    await click(host.querySelector('button[aria-label^="Background jobs"]'));
    await settle(400);
    const jobRow = document.querySelector('.job[data-kind="releaseDownload"]')!;
    expect(jobRow.querySelector('.job__kind')?.textContent).toBe('Download');
    expect(jobRow.querySelector('.job__target')?.textContent).toBe('SHA256SUMS (v1.1.0)');
    expect(jobRow.textContent).toContain('Downloaded SHA256SUMS (312 B).');
    await click(buttonByText(jobRow, 'Show in folder'));
    expect(calls.some((c) => c.cmd === 'release_reveal')).toBe(true);
  });
});

describe('F4 auto-download options', () => {
  it('Clone dialog: checkbox for github.com (default from the setting) is sent as autoDownload', async () => {
    const { backend } = await boot();
    const calls = spy(backend);
    await act(async () => openDialog('clone'));
    await typeInto(document.querySelector('#clone-url'), 'codeberg.org/a/b');
    await settle(350);
    expect(document.querySelector('[data-testid="autodl"]')).toBeNull();
    await typeInto(document.querySelector('#clone-url'), 'someone/newtool');
    await settle(350);
    const box = document.querySelector<HTMLInputElement>('[data-testid="autodl"] input')!;
    expect(box.checked).toBe(true);
    await click(box);
    expect(box.checked).toBe(false);
    await click(buttonByText(document, 'Clone'));
    await settle(20);
    expect(calls.find((c) => c.cmd === 'clone_start')?.args).toEqual({ input: 'someone/newtool', autoDownload: false });
  });

  it('Clone dialog untouched sends null (the backend applies the setting); default follows the setting', async () => {
    const { host, backend } = await boot('', '#/settings');
    await click(host.querySelector('input[name="autoDownloadLatestRelease"]'));
    await settle(50);
    const calls = spy(backend);
    await act(async () => openDialog('clone'));
    await typeInto(document.querySelector('#clone-url'), 'someone/othertool');
    await settle(350);
    expect(document.querySelector<HTMLInputElement>('[data-testid="autodl"] input')!.checked).toBe(false);
    await click(buttonByText(document, 'Clone'));
    await settle(20);
    expect(calls.find((c) => c.cmd === 'clone_start')?.args).toEqual({ input: 'someone/othertool', autoDownload: null });
  });

  it('Import dialog: checkbox for a github.com origin → import_confirm autoDownload', async () => {
    const { backend } = await boot();
    const calls = spy(backend);
    await act(async () => openDialog('import'));
    await click(buttonByText(document, 'Choose folder…'));
    await settle(50);
    const box = document.querySelector<HTMLInputElement>('[data-testid="autodl"] input');
    expect(box).not.toBeNull();
    await click(box);
    await click(buttonByText(document, 'Import'));
    await settle(50);
    expect(calls.find((c) => c.cmd === 'import_confirm')?.args).toMatchObject({ autoDownload: false });
  });

  it('Fetch menu (github.com): with / without download; plain Fetch sends null; other hosts have no menu', async () => {
    const { host, backend } = await boot('', '#/lumen-ui/lumen');
    const calls = spy(backend);
    await settle(300);
    const menuBtn = () => host.querySelector<HTMLButtonElement>('button[aria-label="Fetch options"]');
    await click(menuBtn());
    expect(document.querySelector('.popover__header')?.textContent).toContain('on by default');
    await click([...document.querySelectorAll('.menu__item')].find((b) => b.textContent === 'Fetch without downloading a release'));
    await settle(20);
    expect(calls.filter((c) => c.cmd === 'fetch_start').map((c) => c.args)).toEqual([{ repoId: 2, autoDownload: false }]);
    await settle(400);
    await click(buttonByText(host, 'Fetch'));
    await settle(20);
    expect(calls.filter((c) => c.cmd === 'fetch_start').map((c) => c.args)[1]).toEqual({ repoId: 2, autoDownload: null });
    await go('#/inkwell/quill', 300);
    expect(menuBtn()).toBeNull();
  });
});

describe('F6 Network settings + About', () => {
  it('six controls, saved through settings_set; the size cap commits on Enter and is clamped', async () => {
    const { host, backend } = await boot('', '#/settings');
    const calls = spy(backend);
    await settle(100);
    const net = host.querySelector('[data-testid="network-settings"]')!;
    const boxes = [...net.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')];
    expect(boxes.map((b) => [b.name, b.checked])).toEqual([
      ['loadRemoteImages', true],
      ['fetchReleaseNotes', true],
      ['fetchRepoInfo', true],
      ['autoDownloadLatestRelease', true],
      ['autoDownloadOnFetchAll', false],
    ]);
    await click(boxes[4]);
    await settle(20);
    expect(calls.at(-1)).toMatchObject({ cmd: 'settings_set', args: { settings: { autoDownloadOnFetchAll: true } } });
    const cap = net.querySelector<HTMLInputElement>('#setting-cap')!;
    expect(cap.value).toBe('1024');
    await typeInto(cap, '12x');
    expect(net.querySelector('.setting-cap .field__error')).not.toBeNull();
    const n = calls.length;
    await typeInto(cap, '99999');
    expect(calls.length).toBe(n);
    await act(async () => {
      cap.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    });
    await settle(20);
    expect(calls.at(-1)).toMatchObject({ cmd: 'settings_set', args: { settings: { downloadSizeCapMb: 16384 } } });
    expect(cap.value).toBe('16384');
    expect(parseSizeCap('0')).toBe(1);
    expect(parseSizeCap(' 250 ')).toBe(250);
    expect(parseSizeCap('-3')).toBeNull();
  });

  it('About shows GitHub values when the user has none; Edit About prefills and saves aboutUser only', async () => {
    const { host, backend } = await boot('', '#/tidewater/harbor-cli');
    await settle(300);
    expect(host.querySelector('.about__desc')?.textContent).toBe('Command-line client for container registries.');
    expect(host.querySelector('[data-testid="about-from-github"]')).not.toBeNull();
    await click(host.querySelector('button[aria-label="Edit repository details"]'));
    const dlg = document.querySelector('[role="dialog"]')!;
    console.log('DLG', dlg?.outerHTML.slice(0, 600));
    const desc = dlg.querySelector<HTMLTextAreaElement>('textarea')!;
    expect(desc.value).toBe('');
    expect(desc.placeholder).toBe('From GitHub: Command-line client for container registries.');
    expect(dlg.querySelectorAll('.chip').length).toBe(0);
    expect(dlg.querySelector('[data-testid="about-github-note"]')?.textContent).toContain('topics: go, cli, containers');
    const calls = spy(backend);
    await typeInto(desc, 'My own words');
    await click(buttonByText(dlg, 'Save changes'));
    await settle(100);
    expect(calls.find((c) => c.cmd === 'repo_set_metadata')?.args).toEqual({ repoId: expect.any(Number), description: 'My own words', homepage: null, topics: [] });
    expect(host.querySelector('.about__desc')?.textContent).toBe('My own words');
    // topics still GitHub's → the source line stays
    expect(host.querySelector('[data-testid="about-from-github"]')).not.toBeNull();
  });

  it('lumen (user values equal to the effective ones) shows no GitHub line', async () => {
    const { host } = await boot('', '#/lumen-ui/lumen');
    await settle(300);
    expect(host.querySelector('[data-testid="about-from-github"]')).toBeNull();
  });
});
