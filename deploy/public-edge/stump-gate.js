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
// Invite pages are the app's (/api/_i/…): this file never makes a pass. (The app, Stump, checks the
// pass again itself with the same key; vectors.json pins the format for both.)
import cr from 'crypto';
import fs from 'fs';

const KEY = process.env.EDGE_KEY || '', NAME = process.env.EDGE_COOKIE || '__Host-edge';
const STATE_FILE = '/etc/nginx/state/state.json';
const AGE = 180 * 86400, RENEW = 86400;
const SIGNIN_CAP = 8;                      // below the app's own lock (10 failures a day)
const PASS = /^([a-z0-9-]{1,24})\.(\d{10})\.([A-Za-z0-9_-]{43})$/;
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

function device(r) { const c = valid(r); return c ? c.name : ''; }
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
  if (!c || !s) return '0';
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

export default { device, pass_header, renew, signin, poll };
