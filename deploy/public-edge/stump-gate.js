// Stump public edge: the device pass, as nginx (njs) checks it.
//
// Same pass format the shared /mnt/data2/containers/edge-common/gate.js has always made (and
// the iaboy edge's iaboy-gate.js, which this file is a copy of: only the names differ)
// (cookie <EDGE_COOKIE> = <device>.<issued>.<mac>, mac = base64url HMAC-SHA256(EDGE_KEY,
// "dev|<device>|<issued>"), 180 days, renewed daily), plus what the admin page needs:
//
//   * revoked devices and per-device budget resets come from a state file the app writes
//     (/etc/nginx/state/state.json, read-only here), polled once a second into a shared
//     dictionary: no reload, no control endpoint, nothing for a client to reach;
//   * until the first good read every pass is refused (fail closed); a bad file keeps the last
//     good state; a file is applied whenever its "v" DIFFERS from the one held (so an app
//     restart can never freeze the edge);
//   * the whole state (revoked + budgets) is ONE value in the dictionary, replaced in a single
//     set(), so a request never sees a half-applied update;
//   * X-Edge-Pass tells the app which pass a request carries: it is the cookie's value, set only
//     for a valid, unrevoked pass (a client's own header is replaced); the app checks the
//     signature itself and believes it only for requests through this edge.
//
// Invite pages and the sign-in page are the app's (/api/_i/…): this file never makes a pass. (The app,
// Stump, checks the pass again itself with the same key; vectors.json pins the format for both.)
//
// READING-APP KEYS (friend onboarding): a reading app (KOReader, a Kobo) cannot hold a cookie, so its
// address carries a key, `/opds/<id>.<mac>/v1.2/<route>`, mac = base64url HMAC-SHA256(EDGE_KEY,
// "opds|<id>"). opdsKey() accepts it only when ALL of these hold, judged on the raw request line (the
// text the client sent, which is what Stump gets): the exact shape with no dot segment, no empty
// segment and no escaped slash, dot or NUL; the route is one of OPDS 1.2's; the MAC checks out; the
// app has not revoked the id (the state file, like passes; until the first good read every key is
// refused). A valid key makes device() the key's id for that one address, which is what lets the
// request through without a pass. A pass is not a key and a key is not a pass (different domain tags).
// At most DL_CAP files a day may be downloaded per key (a leaked key cannot pull a whole library).
import cr from 'crypto';
import fs from 'fs';

const KEY = process.env.EDGE_KEY || '', NAME = process.env.EDGE_COOKIE || '__Host-edge';
const STATE_FILE = '/etc/nginx/state/state.json';
const AGE = 180 * 86400, RENEW = 86400;
const SIGNIN_CAP = 8;                      // below the app's own lock (10 failures a day)
const PASS = /^([a-z0-9-]{1,24})\.(\d{10})\.([A-Za-z0-9_-]{43})$/;
const DL_CAP = parseInt(process.env.EDGE_DL_CAP || '300', 10) || 300;   // files a reading-app key may download per UTC day (EDGE_DL_CAP is for the tests)
const OPDS_ROUTES = ['catalog', 'search', 'keep-reading', 'smart-lists', 'reading-lists', 'authors', 'genres', 'libraries', 'series', 'books'];
const OPDS_KEY = /^\/opds\/([a-z0-9-]{1,24})\.([A-Za-z0-9_-]{43})\/v1\.2\/(.+)$/;
const mac = (s) => cr.createHmac('sha256', KEY).update(s).digest('base64url');
const now = () => Math.floor(Date.now() / 1000);

function verify(m) {                       // MACs compared through a second HMAC: no timing signal
  if (!m || KEY.length < 64) return null;  // no key: nothing passes
  return mac('c|' + m[3]) === mac('c|' + mac('dev|' + m[1] + '|' + m[2])) ? { name: m[1], ts: +m[2], raw: m[0] } : null;
}

function cookiePass(r) {                   // the first valid, fresh pass among the request's cookies
  const parts = (r.headersIn.Cookie || '').split(';');
  for (let i = 0; i < parts.length; i++) {
    const p = parts[i].trim();
    if (!p.startsWith(NAME + '=')) continue;
    const c = verify(PASS.exec(p.slice(NAME.length + 1)));
    if (c && now() - c.ts <= AGE && c.ts - now() <= 86400) return c;
  }
  return null;
}

function state() {                         // {r: [revoked ids], g: {id: reset epoch}} or null (not ready)
  const dict = ngx.shared.edge_state, raw = dict && dict.get('state');
  if (!raw) return null;
  try { return JSON.parse(raw); } catch (e) { return null; }
}

function valid(r) {                        // a valid pass the app has not revoked, once the state is known
  const c = cookiePass(r), s = state();
  if (!c || !s || s.r.indexOf(c.name) !== -1) return null;
  return c;
}

function opdsKey(r) {                      // {id, file} for a valid reading-app key in the raw request line, else null
  const raw = r.variables.request_uri || '', q = raw.indexOf('?');
  const path = q === -1 ? raw : raw.slice(0, q);
  const m = OPDS_KEY.exec(path);
  if (!m || KEY.length < 64 || /%(?:2e|2f|5c|00)/i.test(path)) return null;
  const segs = m[3].split('/');
  if (OPDS_ROUTES.indexOf(segs[0]) === -1) return null;
  for (let i = 0; i < segs.length; i++) {  // no empty segment, no dot segment, only what a URL path may hold
    if (!segs[i] || segs[i] === '.' || segs[i] === '..' || !/^[A-Za-z0-9._~%-]+$/.test(segs[i])) return null;
  }
  if (mac('c|' + m[2]) !== mac('c|' + mac('opds|' + m[1]))) return null;   // second HMAC: no timing signal
  const s = state();
  if (!s || s.r.indexOf(m[1]) !== -1) return null;
  return { id: m[1], file: segs[0] === 'books' && segs[2] === 'file', epoch: (s.g && s.g[m[1]]) || 0 };
}

function device(r) {                       // a valid pass's device, else the id of a valid reading-app key for its address
  const c = valid(r);
  if (c) return c.name;
  const k = opdsKey(r);
  return k ? k.id : '';
}
function opds_key(r) { const k = opdsKey(r); return k ? k.id : ''; }
function opds_dl(r) {                      // "0" once a key has downloaded DL_CAP files today (a budget reset starts again)
  const k = opdsKey(r);
  if (!k || !k.file) return '1';
  try { return ngx.shared.edge_dl.incr(`dl:${k.id}:${Math.floor(now() / 86400)}:${k.epoch}`, 1, 0) <= DL_CAP ? '1' : '0'; }
  catch (e) { return '0'; }
}
function pass_header(r) { const c = valid(r); return c ? c.raw : ''; }

function renew(r) {                        // a pass in use is renewed once a day
  const c = valid(r);
  if (!c || now() - c.ts <= RENEW) return '';
  const t = now();
  return `${NAME}=${c.name}.${t}.${mac('dev|' + c.name + '|' + t)}; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=${AGE}`;
}

function signin(r) {                       // 8 sign-in POSTs per device per UTC day (and reset epoch)
  if (r.method !== 'POST') return '1';
  const c = valid(r), s = state();
  if (!s) return '0';                      // the app's state is not known yet: nothing signs in (fail closed)
  if (!c) return '1';                      // no valid pass: the web app's own sign-in screen. Its budget is the
                                           // app's guard and the edge's per-client and global zones, not a device's
  const epoch = (s.g && s.g[c.name]) || 0;
  try { return ngx.shared.edge_signin.incr(`${c.name}:${Math.floor(now() / 86400)}:${epoch}`, 1, 0) <= SIGNIN_CAP ? '1' : '0'; }
  catch (e) { return '0'; }
}

// js_periodic, once a second: follow the app's state file.
function poll(s) {
  const dict = ngx.shared.edge_state;
  let text;
  try { text = fs.readFileSync(STATE_FILE, 'utf8'); }
  catch (e) { return warn(dict, 'unreadable'); }
  let data;
  try {
    data = JSON.parse(text);
    if (typeof data.v !== 'number' || !Array.isArray(data.revoked) || typeof data.budget !== 'object') throw new Error('shape');
  } catch (e) { return warn(dict, 'invalid'); }  // keep the last good state
  if (String(data.v) === dict.get('v')) return;  // nothing new
  dict.set('state', JSON.stringify({ r: data.revoked, g: data.budget }));  // one atomic replace
  dict.set('v', String(data.v));
  dict.delete('warned');
}

function warn(dict, what) {                // one warning per problem, not one a second
  if (dict.get('warned') !== what) { ngx.log(ngx.WARN, 'stump edge: state file ' + what + (dict.get('state') ? ' (keeping the last good state)' : ' (no state yet: every pass refused)')); dict.set('warned', what); }
}

export default { device, pass_header, renew, signin, opds_key, opds_dl, poll };
