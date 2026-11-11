// Mock copy of rs-core `validate::clone_url` (D-1): same accept/reject rules and messages for the common cases, so
// the Clone dialog behaves in the browser preview as in the app.

import { ipcError } from '../errors';
import type { ClonePreview } from '../types';

const SEG = /^[A-Za-z0-9._-]+$/;
const SEG_TILDE = /^[A-Za-z0-9._~-]+$/;

export function mockClonePreview(input: string, archiveDir: string): ClonePreview {
  const bad = (m: string) => ipcError('invalidUrl', m);
  const s = input.trim();
  if (s.length === 0) throw bad('Enter a repository URL, e.g. https://github.com/owner/repo or owner/repo.');
  if (s.length > 2048) throw bad('That URL is too long.');
  if (/\s|[\u0000-\u001f]/.test(s)) throw bad('The URL contains spaces or control characters.');
  const lower = s.toLowerCase();
  if (/^(git@|ssh:\/\/|git\+ssh:\/\/|ssh\+git:\/\/)/.test(lower)) {
    throw bad('SSH addresses are not supported. Use the https:// address of the repository.');
  }
  if (lower.startsWith('git://')) throw bad('git:// addresses are not supported. Use the https:// address of the repository.');
  if (lower.startsWith('file:')) {
    throw bad('Local repositories cannot be cloned. Use Import folder… to add a repository from this PC.');
  }
  if (lower.startsWith('http://')) throw bad('Only https:// addresses are allowed (http:// is not encrypted).');
  if (s.includes('\\')) throw bad('Backslashes are not allowed. Local paths cannot be cloned; use Import folder… instead.');
  if (/^[A-Za-z]:/.test(s) || s.startsWith('/') || s.startsWith('.')) {
    throw bad('Local paths cannot be cloned. Use Import folder… to add a repository from this PC.');
  }
  let full: string;
  const schemeAt = s.indexOf('://');
  if (schemeAt >= 0) {
    if (s.slice(0, schemeAt).toLowerCase() !== 'https') throw bad('Only https:// addresses are supported.');
    full = s;
  } else {
    if (s.includes('@')) {
      throw bad('Addresses with a user name or password are not allowed. Only public repositories can be cloned.');
    }
    const segs = s.replace(/\/+$/, '').split('/');
    if (segs.length < 2) throw bad('Enter owner/repo or a full https:// address.');
    if (segs.length === 2) {
      if (!segs.every((x) => SEG.test(x))) throw bad("owner/repo may only contain letters, digits, '.', '_' and '-'.");
      full = `https://github.com/${segs[0]}/${segs[1]}`;
    } else {
      if (!segs[0].includes('.')) throw bad('Add the host, e.g. github.com/owner/repo or https://host/owner/repo.');
      full = `https://${s}`;
    }
  }
  const after = full.slice(full.indexOf('://') + 3);
  const cut = after.search(/[/?#]/);
  const authority = cut >= 0 ? after.slice(0, cut) : after;
  if (authority.length === 0) throw bad('The address has no host.');
  if (authority.includes('@')) {
    throw bad('Addresses with a user name or password are not allowed. Only public repositories can be cloned.');
  }
  const rawPath = (cut >= 0 ? after.slice(cut) : '').split(/[?#]/)[0];
  const rawSegs = rawPath.split('/').slice(1);
  if (rawSegs[rawSegs.length - 1] === '') rawSegs.pop();
  if (rawSegs.some((x) => x === '' || x === '.' || x === '..' || x.includes('%'))) {
    throw bad("The repository path contains empty, '.', '..' or encoded parts.");
  }
  let url: URL;
  try {
    url = new URL(full);
  } catch {
    throw bad('That is not a valid URL.');
  }
  const host = url.hostname.toLowerCase();
  if (host.length === 0) throw bad('The address has no host.');
  const segs = url.pathname.split('/').filter((x) => x.length > 0);
  if (segs.length > 0) segs[segs.length - 1] = segs[segs.length - 1].replace(/\.git$/, '');
  if (segs.length < 2) {
    throw bad('The address needs an owner and a repository name, e.g. https://github.com/owner/repo.');
  }
  if (!segs.every((x) => SEG_TILDE.test(x))) {
    throw bad("The repository path may only contain letters, digits, '.', '_', '-' and '~'.");
  }
  const owner = segs[0];
  const name = segs[segs.length - 1];
  const authorityOut = url.port ? `${host}:${url.port}` : host;
  const hostDir = url.port ? `${host}_${url.port}` : host;
  return {
    url: `https://${authorityOut}/${segs.join('/')}.git`,
    host,
    owner,
    name,
    targetDir: `${archiveDir}\\${hostDir}\\${owner}\\${segs.slice(1).join('-')}.git`,
  };
}
