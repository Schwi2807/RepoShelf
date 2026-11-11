import { describe, expect, it } from 'vitest';
import { parseRsrepo, rewriteHtml } from './linkRewrite';

const ctx = { owner: 'northwind', repo: 'parsekit', rev: 'feature/x' };

function run(html: string) {
  const r = rewriteHtml(html, ctx);
  const div = document.createElement('div');
  div.append(r.fragment);
  return { ...r, div };
}

describe('linkRewrite', () => {
  it('rsrepo: link → in-app blob route (ref encoded as one segment)', () => {
    const { div } = run('<p><a href="rsrepo:/docs/My%20File.md#part">doc</a></p>');
    const a = div.querySelector('a')!;
    expect(a.getAttribute('href')).toBe('#/northwind/parsekit/blob/feature%2Fx/docs/My%20File.md');
    expect(a.dataset.route).toBe('1');
  });

  it('rsrepo: image → collected for blob_bytes, no src until loaded', () => {
    const { div, images } = run('<p><img src="rsrepo:/docs/logo.svg" alt="logo"></p>');
    const img = div.querySelector('img')!;
    expect(img.hasAttribute('src')).toBe(false);
    expect(images.map((i) => i.path)).toEqual(['docs/logo.svg']);
  });

  it('remote image → "Remote image: <host>" link chip, nothing loads', () => {
    const { div } = run('<p><img src="https://images.example.com/tracker.png" alt="t"></p>');
    expect(div.querySelector('img')).toBeNull();
    const chip = div.querySelector('a.md-remote-image')!;
    expect(chip.textContent).toBe('Remote image: images.example.com');
    expect(chip.getAttribute('data-external')).toBe('1');
  });

  it('a badge (link around a remote image) becomes one chip', () => {
    const { div } = run('<p><a href="https://ci.example.com/x"><img src="https://ci.example.com/badge.svg" alt="b"></a></p>');
    expect(div.querySelectorAll('a')).toHaveLength(1);
    expect(div.querySelector('a')!.className).toBe('md-remote-image');
  });

  it('javascript:, data:, scheme-relative and unknown hrefs are dropped', () => {
    const { div } = run(
      '<a href="javascript:alert(1)">j</a><a href="data:text/html,x">d</a><a href="//evil.example.com/x">s</a><a href="file:///C:/x">f</a><a href=" JAVASCRIPT:alert(1)">J</a>',
    );
    for (const a of div.querySelectorAll('a')) expect(a.hasAttribute('href')).toBe(false);
  });

  it('fragment links are kept; external links are marked', () => {
    const { div } = run('<a href="#user-content-usage">u</a><a href="https://example.com">e</a><a href="mailto:a@example.com">m</a>');
    const [frag, ext, mail] = [...div.querySelectorAll('a')];
    expect(frag.getAttribute('href')).toBe('#user-content-usage');
    expect(frag.dataset.fragment).toBe('user-content-usage');
    expect(ext.dataset.external).toBe('1');
    expect(mail.dataset.external).toBe('1');
  });

  it('strips scripts, handlers, styles, frames and forms; keeps data:image/png', () => {
    const { div } = run(
      '<script>alert(1)</script><p style="position:fixed" onclick="x()">p</p><iframe src="https://x.example"></iframe><form><input></form><img src="data:image/png;base64,AAAA" alt="ok"><img src="data:image/svg+xml;base64,AAAA" alt="svg"><img src="x.png" onerror="alert(1)" alt="rel">',
    );
    expect(div.querySelector('script, iframe, form')).toBeNull();
    const p = div.querySelector('p')!;
    expect(p.hasAttribute('style')).toBe(false);
    expect(p.hasAttribute('onclick')).toBe(false);
    const imgs = [...div.querySelectorAll('img')];
    expect(imgs.map((i) => i.getAttribute('alt'))).toEqual(['ok']);
    expect(div.querySelectorAll('.md-image-missing')).toHaveLength(2);
  });

  it('collects fenced code with its language', () => {
    const { codeBlocks } = run('<pre><code class="language-rust">fn a() {}</code></pre><pre><code>plain</code></pre>');
    expect(codeBlocks.map((c) => c.lang)).toEqual(['rust']);
  });

  it('parseRsrepo refuses dot segments', () => {
    expect(parseRsrepo('rsrepo:/a/../../escape')).toBeNull();
    expect(parseRsrepo('rsrepo:/a%20b.png')).toEqual({ path: 'a b.png', fragment: null });
    expect(parseRsrepo('rsrepo:/README.md#user-content-usage')).toEqual({ path: 'README.md', fragment: 'user-content-usage' });
  });
});
