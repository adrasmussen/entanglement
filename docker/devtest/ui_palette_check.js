// Browser checks for the ⌘K search palette (phase 6): opening and closing it, its grouped
// results against raw SQL, keyboard navigation, opening a hit, "See all" handing the search
// to the prefilled page, debouncing, and that no other group's records appear.  Prints a grid
// with a computed match column; exits non-zero on any mismatch.
//
// needs the dev stack up with the phase 6 build, docker/dev-certs/, playwright, the counttest
// and search seeds (seed_counttest.sql, seed_search.sql) and seed_rbac.sql:
//   PW_MODULE=/path/to/node_modules node docker/devtest/ui_palette_check.js
//
// reads no .env, hard-codes no uuids, writes nothing to the database.
const path = require('path');
const { execFileSync } = require('child_process');
let playwright;
try {
  playwright = require('playwright');
} catch (e) {
  playwright = require(path.join(process.env.PW_MODULE || '', 'playwright'));
}

const CERTS = path.join(__dirname, '..', 'dev-certs') + path.sep;
const ORIGIN = 'https://127.0.0.1:8444';
const BASE = ORIGIN + '/entanglement/app';
const GID = 'devgroup';
const sleep = ms => new Promise(r => setTimeout(r, ms));

function sql(q, vars = {}) {
  const args = ['exec', '-i', 'entanglement-postgres-1', 'psql', '-U', 'entanglement', '-d', 'entanglement', '-At', '-v', 'ON_ERROR_STOP=1'];
  for (const [k, v] of Object.entries(vars)) args.push('-v', `${k}=${v}`);
  return execFileSync('docker', args, { input: q, encoding: 'utf8' }).trim();
}
const count = (q, vars) => parseInt(sql(q, vars), 10);

// raw SQL, written independently of the server: the visibility rule, and a plain word as a
// prefix or (3+ characters) anywhere inside the date, description or tags
const VIS = `m.hidden = FALSE AND (
    m.library_uuid IN (SELECT library_uuid FROM libraries WHERE gid = '${GID}')
    OR EXISTS (SELECT 1 FROM collection_contents cc JOIN collections c USING (collection_uuid)
               WHERE cc.media_uuid = m.media_uuid AND c.gid = '${GID}'))`;
const TEXT = "lower(m.date || ' ' || m.note || ' ' || array_to_string(akeys(m.tags), ' '))";
const word = v => `(m.ts_vec @@ to_tsquery('english', quote_literal(:'${v}') || ':*') OR ` +
  `(char_length(:'${v}') >= 3 AND position(lower(:'${v}') in ${TEXT}) > 0))`;
const media = (...conds) => `SELECT count(*) FROM media m WHERE ${VIS}` + conds.map(c => ' AND ' + c).join('');
const colls = (...conds) => `SELECT count(*) FROM collections c WHERE c.gid = '${GID}'` + conds.map(c => ' AND ' + c).join('');

const rows = [];
let fails = 0;
function check(name, raw, observed, expected) {
  const o = JSON.stringify(observed), e = JSON.stringify(expected);
  const ok = o === e;
  if (!ok) fails++;
  rows.push([name, String(raw), o, e, ok ? 'OK' : 'FAIL']);
}

// the numbers in a heading: "Media · showing 5, 40 more match" -> {shown: 5, total: 45}; "Media · 3" -> 3/3
function parseHeading(t) {
  let m = /(\w+) · showing (\d+), (\d+) more match/.exec(t);
  if (m) return { group: m[1], shown: +m[2], total: +m[2] + +m[3] };
  m = /(\w+) · (\d+)/.exec(t);
  return m ? { group: m[1], shown: +m[2], total: +m[2] } : null;
}

(async () => {
  const browser = await playwright.chromium.launch();
  const ctx = await browser.newContext({
    ignoreHTTPSErrors: true,
    viewport: { width: 1440, height: 900 },
    clientCertificates: [{ origin: ORIGIN, certPath: CERTS + 'client.crt', keyPath: CERTS + 'client.key' }],
  });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', e => errors.push(e.message));
  const omniRequests = [];
  page.on('request', r => {
    if (r.method() === 'POST' && /\/api\/OmniSearch$/.test(r.url())) omniRequests.push(JSON.parse(r.postData()).query);
  });

  const isOpen = () => page.$('[data-testid=palette]').then(e => e !== null);
  const headings = () => page.$$eval('[data-testid=palette-heading]', els => els.map(e => e.textContent));
  const notes = () => page.$$eval('[data-testid=palette-note]', els => els.map(e => e.textContent));
  const activeHref = () => page.$eval('.palette-row.active', e => e.dataset.href).catch(() => null);
  async function search(text) {
    await page.fill('[data-testid=palette-input]', text);
    // results are shown only for exactly what is typed; wait for them (or a note)
    await page.waitForFunction(() => document.querySelector('[data-testid=palette-heading], [data-testid=palette-empty]')
      || [...document.querySelectorAll('[data-testid=palette-note]')].some(n => !/Searching/.test(n.textContent)), null, { timeout: 10000 });
    await sleep(300);
  }
  async function openPalette(key = 'Control+k') {
    await page.keyboard.press(key);
    await page.waitForSelector('[data-testid=palette-input]', { timeout: 5000 });
  }

  // ---- opening and closing
  await page.goto(BASE + '/gallery');
  await page.waitForSelector('[data-testid=query-input]');
  await sleep(800);
  check('closed until asked', '-', await isOpen(), false);
  await openPalette('Control+k');
  check('Ctrl+K opens it with the box focused', '-',
    await page.evaluate(() => document.activeElement && document.activeElement.dataset.testid), 'palette-input');
  await page.keyboard.press('Escape');
  await sleep(200);
  check('Esc closes it', '-', await isOpen(), false);
  await openPalette('Meta+k');
  check('⌘K (Meta+K) opens it too', '-', await isOpen(), true);
  await page.keyboard.press('Control+k');
  await sleep(200);
  check('the shortcut again closes it', '-', await isOpen(), false);
  await openPalette();
  await page.click('.palette-overlay', { position: { x: 10, y: 10 } });
  await sleep(200);
  check('a click outside the panel closes it', '-', await isOpen(), false);

  // ---- grouped results vs raw SQL; libraries have no tag: field
  await openPalette();
  await search('tag:ct-mod7 sunset');
  const vars = { v0: 'ct-mod7', w0: 'sunset' };
  const rawMedia = count(media("m.tags ? :'v0'", word('w0')), vars);
  const rawColls = count(colls("c.tags ? :'v0'", "position(lower(:'w0') in lower(c.name)) > 0"), vars);
  const hs = (await headings()).map(parseHeading);
  const mediaHead = hs.find(h => h && h.group === 'Media');
  check('tag:ct-mod7 sunset: Media heading total and shown', `${rawMedia} (raw SQL)`,
    mediaHead && { total: mediaHead.total, shown: mediaHead.shown }, { total: rawMedia, shown: Math.min(rawMedia, 5) });
  check('tag:ct-mod7 sunset: Collections group (none match: left out)', `${rawColls} (raw SQL)`,
    hs.some(h => h && h.group === 'Collections'), rawColls > 0);
  check('tag:ct-mod7 sunset: Libraries note', '-', await notes(), ['Libraries have no tag: field']);
  check('the first hit is active', '-', (await activeHref() || '').startsWith('/gallery/'), true);

  // media hit titles are the file names of those records
  const titles = await page.$$eval('[data-testid=palette-hit]', els => els.map(e => ({
    href: e.dataset.href, title: e.querySelector('[data-testid=palette-title]').textContent })));
  const titleOk = titles.filter(t => t.href.startsWith('/gallery/')).every(t =>
    t.title === sql(`SELECT regexp_replace(path, '^.*/', '') FROM media WHERE media_uuid = '${t.href.split('/').pop()}'`));
  check('media hits are titled with their file names', 'regexp_replace(path, ...)', titleOk, true);

  // ↓ through the media hits to "See all", then Enter: the gallery with ?q= and the same total
  for (let i = 0; i < mediaHead.shown; i++) await page.keyboard.press('ArrowDown');
  const seeAll = await activeHref();
  await page.keyboard.press('Enter');
  await page.waitForURL(/\/gallery\/\?q=/, { timeout: 5000 });
  let pageTotal = null;
  for (let i = 0; i < 50; i++) {
    const s = await page.evaluate(() => (document.querySelector('[data-testid=query-status]') || {}).textContent || '');
    const m = /of (\d+)/.exec(s);
    if (m) { pageTotal = +m[1]; if (pageTotal === rawMedia) break; }
    await sleep(200);
  }
  check('See all: opens the gallery with the search prefilled', '-', {
    href: seeAll, url: new URL(page.url()).pathname + new URL(page.url()).search,
    input: await page.inputValue('[data-testid=query-input]'), palette_closed: !(await isOpen()),
  }, {
    href: '/gallery/?q=tag%3Act-mod7%20sunset', url: '/entanglement/app/gallery/?q=tag%3Act-mod7%20sunset',
    input: 'tag:ct-mod7 sunset', palette_closed: true,
  });
  check('See all: the page total equals the group total', `${rawMedia} (raw SQL)`, pageTotal, rawMedia);

  // ---- a collection hit opens the collection; Back returns
  const c007 = sql("SELECT collection_uuid FROM collections WHERE name = 'counttest-c007'");
  const before = page.url();
  await openPalette();
  await search('c007');
  let target = null;
  for (let i = 0; i < 12; i++) {
    const h = await activeHref();
    if (h && h.startsWith('/collections/')) { target = h; break; }
    await page.keyboard.press('ArrowDown');
  }
  await page.keyboard.press('Enter');
  await page.waitForURL(/\/collections\/[0-9a-f-]{36}/, { timeout: 5000 });
  check('c007: Enter on the collection hit opens it', `${c007} (psql)`,
    new URL(page.url()).pathname, `/entanglement/app/collections/${c007}`);
  await page.goBack();
  await sleep(800);
  check('Back returns to where the palette was opened', '-', page.url(), before);

  // ---- no other group's records: foreign notes say 'lighthouse', foreign files are rbac_NNNNN
  await openPalette();
  await search('lighthouse');
  const rawLight = count(media(word('w0')), { w0: 'lighthouse' });
  const lh = (await headings()).map(parseHeading).find(h => h && h.group === 'Media');
  const never = new Set(sql(`SELECT regexp_replace(path, '^.*/', '') FROM media WHERE path LIKE '/counttest-rbac/%'
      AND NOT (path ~ 'rbac_002(0[1-9]|1[01])\\.jpg$' AND hidden = FALSE)`).split('\n').filter(Boolean));
  const shownTitles = await page.$$eval('[data-testid=palette-title]', els => els.map(e => e.textContent));
  check('lighthouse: media total over visible records only', `${rawLight} (raw SQL)`, lh && lh.total, rawLight);
  check('lighthouse: no foreign file or collection shown', '-', {
    foreign_files: shownTitles.filter(t => never.has(t)), foreign_collection: shownTitles.includes('counttest-rbac-other'),
  }, { foreign_files: [], foreign_collection: false });
  await page.keyboard.press('Escape');

  // ---- typing debounces: a quick word sends at most two searches, the last for the whole word
  await openPalette();
  omniRequests.length = 0;
  await page.type('[data-testid=palette-input]', 'checkpoint', { delay: 30 });
  await sleep(1200);
  check("typing 'checkpoint' quickly", '-', {
    at_most_two: omniRequests.length <= 2, last: omniRequests[omniRequests.length - 1],
  }, { at_most_two: true, last: 'checkpoint' });
  await page.keyboard.press('Escape');

  // ---- a modal owns the keyboard
  await page.goto(BASE + '/collections');
  await page.waitForSelector('button:has-text("Create Collection")');
  await sleep(800);
  await page.click('button:has-text("Create Collection")');
  await page.waitForSelector('.modal-content');
  await page.keyboard.press('Control+k');
  await sleep(300);
  check('the shortcut does nothing while a modal is open', '-', await isOpen(), false);

  check('no page errors', '-', errors, []);
  await browser.close();

  const w = [0, 1, 2, 3, 4].map(i => Math.max(...rows.map(r => r[i].length), ['case', 'raw SQL', 'observed', 'expected', 'match'][i].length));
  const line = r => r.map((c, i) => c.padEnd(w[i])).join(' | ');
  console.log('== the ⌘K search palette');
  console.log(line(['case', 'raw SQL', 'observed', 'expected', 'match']));
  console.log(w.map(x => '-'.repeat(x)).join('-+-'));
  rows.forEach(r => console.log(line(r)));
  console.log(fails ? `\n${fails} CASE(S) FAILED` : '\nALL CASES MATCH');
  process.exit(fails ? 1 : 0);
})().catch(e => {
  console.error(e);
  process.exit(2);
});
