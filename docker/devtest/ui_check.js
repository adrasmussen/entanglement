// browser checks for the phase 4 search UI (QueryBar, chips, completions, the filter builder
// and ?q= prefill) against the dev stack, with Playwright and chromium.
//
//   cd <a directory with `npm i playwright` done> && node <repo>/docker/devtest/ui_check.js
//   (or NODE_PATH=<that directory>/node_modules node docker/devtest/ui_check.js)
//
// needs the dev stack up with the phase 4 build deployed, docker/dev-certs/ generated, the
// counttest data of api_check.sh and docker/devtest/seed_search.sql, and the dev postgres
// container for the raw SQL.  reads no .env, hard-codes no uuids (they are looked up in psql),
// and writes nothing to the database.  results print as grids of raw values, with the match
// column computed here; the exit status is non-zero when any case does not match.

const path = require('path');
const { execFileSync } = require('child_process');

let playwright;
try {
  playwright = require('playwright');
} catch (e) {
  playwright = require(path.join(process.env.PW_MODULE || '', 'playwright'));
}
const { chromium } = playwright;

const CERTS = path.join(__dirname, '..', 'dev-certs') + path.sep;
const ORIGIN = 'https://127.0.0.1:8444';
const BASE = ORIGIN + '/entanglement/app';
const PG = 'entanglement-postgres-1';
const GID = 'devgroup'; // the dev user's group; the raw SQL applies it like the backend
const STORAGE = k => 'entanglement_' + k;
const sleep = ms => new Promise(r => setTimeout(r, ms));

// ---- raw SQL (read-only), values passed as psql variables ----
function sql(q, vars = {}) {
  const args = ['exec', '-i', PG, 'psql', '-U', 'entanglement', '-d', 'entanglement', '-At', '-v', 'ON_ERROR_STOP=1'];
  for (const [k, v] of Object.entries(vars)) args.push('-v', `${k}=${v}`);
  return execFileSync('docker', args, { input: q, encoding: 'utf8' }).trim();
}
const count = (q, vars) => parseInt(sql(q, vars), 10);

const MEDIA_VISIBLE = `SELECT count(*) FROM media m WHERE m.hidden = FALSE AND (
    m.library_uuid IN (SELECT library_uuid FROM libraries WHERE gid = '${GID}')
    OR EXISTS (SELECT 1 FROM collection_contents cc JOIN collections c USING (collection_uuid)
               WHERE cc.media_uuid = m.media_uuid AND c.gid = '${GID}'))`;
const rawTag = v => `m.tags ? :'${v}'`;
const rawDatePrefix = v => `left(m.date, length(:'${v}')) = :'${v}'`;
const rawDateRange = (a, b) => `(m.date >= :'${a}' AND (m.date <= :'${b}' OR ${rawDatePrefix(b)}))`;
const rawType = v => `m.media_type::text = :'${v}'`;
const rawText = v => `m.ts_vec @@ to_tsquery('english', quote_literal(:'${v}'))`;
const media = (...conds) => MEDIA_VISIBLE + conds.map(c => ' AND ' + c).join('');

const lookup = (q, what) => {
  const v = sql(q);
  if (!v) throw new Error('could not look up ' + what);
  return v;
};

// ---- grids ----
let FAILED = 0;
const sections = [];
function section(title, header) {
  const s = { title, header, rows: [] };
  sections.push(s);
  return s;
}
const show = v => (typeof v === 'string' ? v : JSON.stringify(v));
// one case: observed and expected are compared here, as JSON
function check(s, name, raw, observed, expected) {
  const ok = JSON.stringify(observed) === JSON.stringify(expected);
  if (!ok) FAILED++;
  s.rows.push([name, show(raw), show(observed), show(expected), ok ? 'OK' : 'FAIL']);
}
function printGrids() {
  for (const s of sections) {
    const cells = [s.header, ...s.rows];
    const w = s.header.map((_, i) => Math.max(...cells.map(r => String(r[i]).length)));
    console.log(`\n== ${s.title}`);
    cells.forEach((r, n) => {
      console.log(r.map((c, i) => String(c).padEnd(w[i])).join(' | ').trimEnd());
      if (n === 0) console.log(w.map(x => '-'.repeat(x)).join('-+-'));
    });
  }
  console.log(`\n${FAILED === 0 ? 'ALL CASES MATCH' : FAILED + ' CASE(S) FAILED'}`);
}
const HEADER = ['case', 'raw SQL / source', 'observed', 'expected', 'match'];

// ---- browser helpers ----
async function launch() {
  const browser = await chromium.launch();
  const ctx = await browser.newContext({
    ignoreHTTPSErrors: true,
    viewport: { width: 1440, height: 900 },
    clientCertificates: [{ origin: ORIGIN, certPath: CERTS + 'client.crt', keyPath: CERTS + 'client.key' }],
  });
  const page = await ctx.newPage();
  const search = []; // the structured queries of the media search requests, in order
  page.on('request', r => {
    if (r.method() === 'POST' && /\/api\/BatchSearchAndSort$/.test(r.url())) {
      try {
        search.push(JSON.parse(r.postData()).req.Media.opts.filter);
      } catch (e) {
        /* not a media search */
      }
    }
  });
  page.on('pageerror', e => console.log('PAGEERROR', e.message));
  page.on('console', m => {
    if (m.type() === 'error' || /panick/i.test(m.text())) console.log('CONSOLE', m.type(), m.text().slice(0, 200));
  });
  return { browser, ctx, page, search };
}

const INPUT = '[data-testid=query-input]';
const statusText = page => page.evaluate(() => (document.querySelector('[data-testid=query-status]') || {}).textContent || '');
// the total in a status line: "Showing 60 of 123 results", "Showing 5 of 5 collections", "Found 1 libraries"
const totalOf = s => {
  let m = /of (\d+)/.exec(s) || /Found (\d+)/.exec(s) || /Showing (\d+)/.exec(s);
  return m ? +m[1] : null;
};
const loadedOf = s => {
  const m = /Showing (\d+)/.exec(s);
  return m ? +m[1] : 0;
};

// wait for the status total to reach `want` (the status lingers from the previous search while
// the next one loads, so it is the value after settling that counts); returns the last total seen
async function settleTotal(page, want, ms = 15000) {
  const end = Date.now() + ms;
  let last = null;
  while (Date.now() < end) {
    last = totalOf(await statusText(page));
    if (last === want) {
      await sleep(400);
      last = totalOf(await statusText(page));
      if (last === want) return last;
    }
    await sleep(200);
  }
  return last;
}

const ALL_KEYS = ['media_search', 'media_in_collection_search', 'media_in_library_search', 'collection_search', 'library_search'];
async function open(page, route, query) {
  if (query === '') {
    // an empty search means "whatever was stored", so start from nothing stored
    await page.goto(BASE + '/');
    await page.evaluate(ks => ks.forEach(k => localStorage.removeItem(k)), ALL_KEYS.map(STORAGE));
  }
  await page.goto(BASE + route + (query === undefined ? '' : '?q=' + encodeURIComponent(query)));
  await page.waitForSelector(INPUT);
  await page.waitForFunction(() => /Showing|Found|Error/.test((document.querySelector('[data-testid=query-status]') || {}).textContent || ''));
  await sleep(500);
}
const inputValue = page => page.inputValue(INPUT);
const chipLabels = page =>
  page.$$eval('[data-testid=query-chips] [data-testid=query-chip] .query-chip-label', els => els.map(e => e.textContent));
const issueLines = page => page.$$eval('[data-testid=query-issue]', els => els.map(e => e.textContent));
const suggestions = page => page.$$eval('[data-testid=query-suggestion]', els => els.map(e => e.dataset.insert));
const urlQ = page => new URL(page.url()).searchParams.get('q');
const storedSearch = (page, key) => page.evaluate(k => JSON.parse(localStorage.getItem(k) || 'null'), STORAGE(key));

// type a search and commit it with Enter
async function submit(page, text) {
  await page.click(INPUT);
  await page.fill(INPUT, text);
  await page.keyboard.press('Escape'); // close the completions, so that Enter submits
  await page.keyboard.press('Enter');
}

(async () => {
  // ---- what the checks need, looked up by name
  const CT_LIB = lookup(`SELECT library_uuid FROM libraries WHERE path = '/counttest'`, 'library /counttest');
  const CT_C007 = lookup(`SELECT collection_uuid FROM collections WHERE name = 'counttest-c007'`, 'collection counttest-c007');
  const media_in_collection = (...conds) =>
    `SELECT count(*) FROM collection_contents cc JOIN collections c ON c.collection_uuid = cc.collection_uuid
     JOIN media m ON m.media_uuid = cc.media_uuid
     WHERE c.gid = '${GID}' AND c.collection_uuid = '${CT_C007}' AND m.hidden = FALSE` + conds.map(c => ' AND ' + c).join('');
  const media_in_library = (...conds) =>
    `SELECT count(*) FROM media m WHERE m.library_uuid = '${CT_LIB}'
     AND m.library_uuid IN (SELECT library_uuid FROM libraries WHERE gid = '${GID}') AND m.hidden = FALSE` +
    conds.map(c => ' AND ' + c).join('');
  const collections = (...conds) =>
    `SELECT count(*) FROM collections c WHERE c.gid = '${GID}'` + conds.map(c => ' AND ' + c).join('');
  const libraries = (...conds) =>
    `SELECT count(*) FROM libraries l WHERE l.gid = '${GID}'` + conds.map(c => ' AND ' + c).join('');

  const t = await launch();
  const { page, ctx } = t;

  // ================================================================ 1. chips and counts
  let s = section('gallery: typed text becomes chips, and the status total matches raw SQL', HEADER);
  await open(page, '/gallery', '');
  await submit(page, 'tag:ct-mod7 date:2022');
  let raw = count(media(rawTag('v0'), rawDatePrefix('v1')), { v0: 'ct-mod7', v1: '2022' });
  let got = await settleTotal(page, raw);
  check(s, 'tag:ct-mod7 date:2022', media(rawTag('v0'), rawDatePrefix('v1')) + ' v0=ct-mod7 v1=2022', {
    chips: await chipLabels(page),
    total: got,
    nonzero: raw > 0,
  }, { chips: ['Tag: ct-mod7', 'Date: 2022'], total: raw, nonzero: true });

  // x on a chip removes the clause and rewrites the text
  await page.click('[data-testid=query-chip-remove] >> nth=0');
  raw = count(media(rawDatePrefix('v0')), { v0: '2022' });
  got = await settleTotal(page, raw);
  check(s, 'x on the Tag chip', media(rawDatePrefix('v0')) + ' v0=2022', {
    chips: await chipLabels(page),
    input: await inputValue(page),
    total: got,
    url_q: urlQ(page),
  }, { chips: ['Date: 2022'], input: 'date:2022', total: raw, url_q: 'date:2022' });

  await page.click('[data-testid=query-chip-remove] >> nth=0');
  raw = count(media());
  got = await settleTotal(page, raw);
  check(s, 'x on the last chip', media(), {
    chips: await chipLabels(page),
    input: await inputValue(page),
    total: got,
  }, { chips: [], input: '', total: raw });

  // ranges and comparisons read as one chip each
  await submit(page, 'date:2021-01..2021-06 type:image words');
  check(s, 'range chip, enum case, a words chip', '-', {
    chips: await chipLabels(page),
  }, { chips: ['Words: words', 'Date: 2021-01 … 2021-06', 'Type: Image'] });
  const tips = await page.$$eval('[data-testid=query-chips] [data-testid=query-chip]', els => els.map(e => e.title));
  check(s, 'chip tooltips are the field help', '-', {
    tips: tips.map(x => x.length > 0),
    date_tip: tips[1],
  }, {
    tips: [true, true, true],
    date_tip: 'Media from a date or date prefix; use > >= < <= or a..b for an inclusive range',
  });

  // ================================================================ 2. completions
  s = section('gallery: completions', HEADER);
  await open(page, '/gallery', '');
  await page.click(INPUT);
  await page.fill(INPUT, 'ta');
  check(s, "typing 'ta'", '-', { inserts: await suggestions(page) }, { inserts: ['tag:'] });
  const first = await page.$eval('[data-testid=query-suggestion]', e => e.textContent);
  check(s, "'ta' row shows label, help and example", '-', {
    label: first.startsWith('Tag'),
    help: first.includes('Media that has exactly this tag'),
    example: first.includes('tag:holiday'),
  }, { label: true, help: true, example: true });
  await page.fill(INPUT, 'type:');
  check(s, "after 'type:'", '-', { inserts: await suggestions(page) }, {
    inserts: ['type:Image', 'type:Video', 'type:VideoSlice', 'type:Audio'],
  });
  await page.fill(INPUT, 'date:');
  check(s, "after 'date:'", '-', { inserts: await suggestions(page) }, {
    inserts: ['date:>', 'date:>=', 'date:<', 'date:<=', 'date:2020-06..2020-08'],
  });
  const storedBeforeEnter = await storedSearch(page, 'media_search');
  await page.fill(INPUT, 'type:v');
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowUp');
  const activeBefore = await page.$$eval('[data-testid=query-suggestion].active', els => els.map(e => e.dataset.insert));
  await page.keyboard.press('Enter');
  check(s, "'type:v', down, down, up, Enter accepts the active row", '-', {
    active: activeBefore,
    input: await inputValue(page),
    stored_search_unchanged: (await storedSearch(page, 'media_search')) === storedBeforeEnter,
  }, { active: ['type:Video'], input: 'type:Video ', stored_search_unchanged: true });
  await page.fill(INPUT, 'ta');
  await page.keyboard.press('Tab');
  check(s, "'ta', Tab accepts the first row", '-', { input: await inputValue(page) }, { input: 'tag:' });
  await page.fill(INPUT, 'ty');
  await page.keyboard.press('Escape');
  check(s, 'Esc closes the list', '-', { inserts: await suggestions(page) }, { inserts: [] });
  await page.fill(INPUT, 'ty');
  await page.$eval('[data-testid=query-suggestion]', e => e.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, cancelable: true })));
  check(s, 'clicking a row accepts it', '-', { input: await inputValue(page) }, { input: 'type:' });
  await page.click('[data-testid=query-help-toggle]');
  const helpText = await page.$eval('[data-testid=query-help]', e => e.textContent);
  check(s, 'the ? panel lists every field with its help and example', '-', {
    fields: ['Tag', 'Description', 'Date', 'Type'].map(f => helpText.includes(f)),
    examples: ['tag:holiday', 'description:sunset', 'date:2020-06..2020-08', 'type:Image'].map(e => helpText.includes(e)),
  }, { fields: [true, true, true, true], examples: [true, true, true, true] });

  // ================================================================ 3. issues
  s = section('gallery: parse issues', HEADER);
  await open(page, '/gallery', '');
  t.search.length = 0;
  await submit(page, 'colour:red tag:ct-mod7 date:soon');
  raw = count(media(rawTag('v0'), rawText('v1')), { v0: 'ct-mod7', v1: 'colour:red' });
  got = await settleTotal(page, raw);
  const sent = t.search[t.search.length - 1];
  check(s, 'unknown field and a bad date', media(rawTag('v0'), rawText('v1')) + ' v0=ct-mod7 v1=colour:red', {
    issues: await issueLines(page),
    chips: await chipLabels(page),
    sent_text: sent && sent.Query.text,
    sent_clauses: sent && sent.Query.clauses.map(c => `${c.field}:${c.op}:${c.value}`),
    total: got,
  }, {
    issues: [
      "unknown field 'colour'; searched as text",
      'date:soon: date must look like 2020, 2020-06 or 2020-06-15; ignored',
    ],
    chips: ['Words: colour:red', 'Tag: ct-mod7'],
    sent_text: ['colour:red'],
    sent_clauses: ['tag:Has:ct-mod7'],
    total: raw,
  });

  // ================================================================ 4. the filter builder
  s = section('gallery: the filter builder in the Advanced tab', HEADER);
  await open(page, '/gallery', '');
  await page.getByRole('button', { name: 'Advanced', exact: true }).click();
  await page.waitForSelector('[data-testid=builder-field]');
  const fields = await page.$$eval('[data-testid=builder-field] option', els => els.map(e => e.value));
  const removed = await page.evaluate(() => document.body.textContent);
  check(s, 'fields come from the metadata; the dead controls are gone', '-', {
    fields,
    search_mode: removed.includes('Search Mode'),
    case_sensitive: removed.includes('Case Sensitive'),
  }, { fields: ['tag', 'description', 'date', 'type'], search_mode: false, case_sensitive: false });

  await page.selectOption('[data-testid=builder-field]', 'type');
  await page.selectOption('[data-testid=builder-value]', 'Image');
  await page.click('[data-testid=builder-add]');
  raw = count(media(rawType('v0')), { v0: 'Image' });
  got = await settleTotal(page, raw);
  check(s, 'add type is Image', media(rawType('v0')) + ' v0=Image', {
    chips: await chipLabels(page),
    input: await inputValue(page),
    total: got,
    nonzero: raw > 0,
  }, { chips: ['Type: Image'], input: 'type:Image', total: raw, nonzero: true });

  await page.selectOption('[data-testid=builder-field]', 'date');
  const ops = await page.$$eval('[data-testid=builder-op] option', els => els.map(e => e.value));
  await page.selectOption('[data-testid=builder-op]', 'Range');
  await page.fill('[data-testid=builder-value]', '2021-01');
  await page.fill('[data-testid=builder-value-to]', '2021-06');
  await page.click('[data-testid=builder-add]');
  raw = count(media(rawType('v0'), rawDateRange('v1', 'v2')), { v0: 'Image', v1: '2021-01', v2: '2021-06' });
  got = await settleTotal(page, raw);
  check(s, 'add date between 2021-01 and 2021-06', media(rawType('v0'), rawDateRange('v1', 'v2')) + ' v0=Image v1=2021-01 v2=2021-06', {
    date_ops: ops,
    chips: await chipLabels(page),
    input: await inputValue(page),
    total: got,
  }, {
    date_ops: ['Eq', 'Gt', 'Ge', 'Lt', 'Le', 'Range'],
    chips: ['Type: Image', 'Date: 2021-01 … 2021-06'],
    input: 'type:Image date:2021-01..2021-06',
    total: raw,
  });
  const inBuilder = await page.$$eval('[data-testid=builder-chips] [data-testid=query-chip] .query-chip-label', els => els.map(e => e.textContent));
  check(s, "the builder's current filters are the same chips", '-', { chips: inBuilder }, {
    chips: ['Type: Image', 'Date: 2021-01 … 2021-06'],
  });
  await page.click('[data-testid=builder-chips] [data-testid=query-chip-remove] >> nth=1');
  raw = count(media(rawType('v0')), { v0: 'Image' });
  got = await settleTotal(page, raw);
  check(s, "remove from the builder's list", media(rawType('v0')) + ' v0=Image', {
    input: await inputValue(page),
    total: got,
  }, { input: 'type:Image', total: raw });
  await page.selectOption('[data-testid=builder-field]', 'tag');
  await page.fill('[data-testid=builder-value]', '');
  await page.click('[data-testid=builder-add]');
  check(s, 'an empty value is refused with a message', '-', {
    error: await page.$eval('[data-testid=builder-error]', e => e.textContent),
    input: await inputValue(page),
  }, { error: 'Tag needs a value', input: 'type:Image' });

  // ================================================================ 5. ?q= prefill
  s = section('?q= prefill, committing to the URL, and local storage', HEADER);
  // gallery
  await open(page, '/gallery', 'tag:ct-mod7');
  raw = count(media(rawTag('v0')), { v0: 'ct-mod7' });
  got = await settleTotal(page, raw);
  check(s, '/gallery?q=tag:ct-mod7', media(rawTag('v0')) + ' v0=ct-mod7', {
    input: await inputValue(page),
    chips: await chipLabels(page),
    total: got,
    stored: await storedSearch(page, 'media_search'),
  }, { input: 'tag:ct-mod7', chips: ['Tag: ct-mod7'], total: raw, stored: 'tag:ct-mod7' });

  // collection detail
  await open(page, '/collections/' + CT_C007, 'tag:ct-odd');
  raw = count(media_in_collection(rawTag('v0')), { v0: 'ct-odd' });
  got = await settleTotal(page, raw);
  check(s, '/collections/<counttest-c007>?q=tag:ct-odd', media_in_collection(rawTag('v0')) + ' v0=ct-odd', {
    input: await inputValue(page),
    total: got,
  }, { input: 'tag:ct-odd', total: raw });

  // library detail
  await open(page, '/library/' + CT_LIB, 'tag:ct-even date:2022');
  raw = count(media_in_library(rawTag('v0'), rawDatePrefix('v1')), { v0: 'ct-even', v1: '2022' });
  got = await settleTotal(page, raw);
  check(s, '/library/<counttest>?q=tag:ct-even date:2022', media_in_library(rawTag('v0'), rawDatePrefix('v1')) + ' v0=ct-even v1=2022', {
    input: await inputValue(page),
    total: got,
  }, { input: 'tag:ct-even date:2022', total: raw });

  // collections list
  await open(page, '/collections/', 'tag:ct-mod7');
  raw = count(collections('c.tags ? :\'v0\''), { v0: 'ct-mod7' });
  got = await settleTotal(page, raw);
  check(s, '/collections?q=tag:ct-mod7', collections("c.tags ? :'v0'") + ' v0=ct-mod7', {
    input: await inputValue(page),
    chips: await chipLabels(page),
    total: got,
    nonzero: raw > 0,
  }, { input: 'tag:ct-mod7', chips: ['Tag: ct-mod7'], total: raw, nonzero: true });
  await submit(page, 'name:"phrase big" description:spaces');
  raw = count(collections("position(lower(:'v0') in lower(c.name)) > 0", "position(lower(:'v1') in lower(c.note)) > 0"), { v0: 'phrase big', v1: 'spaces' });
  got = await settleTotal(page, raw);
  check(s, 'collections: a phrase and description typed', 'phrase big / spaces', {
    chips: await chipLabels(page),
    total: got,
    nonzero: raw > 0,
    url_q: urlQ(page),
  }, { chips: ['Name: phrase big', 'Description: spaces'], total: raw, nonzero: true, url_q: 'name:"phrase big" description:spaces' });
  await submit(page, 'c01 c02');
  raw = count(collections("(position(lower(:'v0') in lower(c.name)) > 0 OR position(lower(:'v1') in lower(c.name)) > 0)"), { v0: 'c01', v1: 'c02' });
  got = await settleTotal(page, raw);
  check(s, 'collections: two plain words are an OR', "name has c01 OR c02", { total: got, nonzero: raw > 0 }, { total: raw, nonzero: true });
  await page.fill(INPUT, 'na');
  check(s, "collections: typing 'na' suggests name", '-', { inserts: await suggestions(page) }, { inserts: ['name:'] });

  // libraries
  await open(page, '/library/', 'path:count');
  raw = count(libraries("position(lower(:'v0') in lower(l.path)) > 0"), { v0: 'count' });
  got = await settleTotal(page, raw);
  check(s, '/library?q=path:count', libraries("position(lower(:'v0') in lower(l.path)) > 0") + ' v0=count', {
    input: await inputValue(page),
    chips: await chipLabels(page),
    total: got,
    nonzero: raw > 0,
  }, { input: 'path:count', chips: ['Path: count'], total: raw, nonzero: true });

  // committing puts the search in the URL, and a reload restores it
  await open(page, '/gallery', '');
  const historyBefore = await page.evaluate(() => history.length);
  await submit(page, 'tag:ct-mod7 date:2022');
  raw = count(media(rawTag('v0'), rawDatePrefix('v1')), { v0: 'ct-mod7', v1: '2022' });
  await settleTotal(page, raw);
  const urlAfterCommit = page.url();
  const historyAdded = (await page.evaluate(() => history.length)) - historyBefore;
  await page.reload();
  await page.waitForSelector(INPUT);
  got = await settleTotal(page, raw);
  check(s, 'commit shows ?q= in the URL; reload restores it', media(rawTag('v0'), rawDatePrefix('v1')) + ' v0=ct-mod7 v1=2022', {
    url_has_q: /[?&]q=tag%3Act-mod7%20date%3A2022$/.test(urlAfterCommit),
    after_reload_input: await inputValue(page),
    after_reload_total: got,
    history_entries_added: historyAdded,
  }, { url_has_q: true, after_reload_input: 'tag:ct-mod7 date:2022', after_reload_total: raw, history_entries_added: 0 });

  // text with spaces, quotes, : % & and = round-trips through the URL
  const hard = 'description:"a b" tag:x%y&z=1 "q#?/"';
  await submit(page, hard);
  await sleep(600);
  const hardUrl = page.url();
  await page.reload();
  await page.waitForSelector(INPUT);
  await sleep(600);
  check(s, 'hard characters survive commit and reload', hardUrl.slice(hardUrl.indexOf('?')), {
    input: await inputValue(page),
    stored: await storedSearch(page, 'media_search'),
    amp_is_double_encoded: hardUrl.includes('%2526'),
  }, { input: hard, stored: hard, amp_is_double_encoded: true });

  // no ?q=: the last search comes back from local storage; a ?q= wins over it and is stored
  await submit(page, 'tag:ct-mod7');
  await sleep(600);
  await page.goto(BASE + '/gallery');
  await page.waitForSelector(INPUT);
  await sleep(600);
  check(s, 'no ?q=: the stored search is used', '-', {
    input: await inputValue(page),
    url_has_q: /[?&]q=/.test(page.url()) && urlQ(page) !== '',
  }, { input: 'tag:ct-mod7', url_has_q: false });
  await open(page, '/gallery', 'type:Image');
  check(s, '?q= wins over the stored search, and replaces it', '-', {
    input: await inputValue(page),
    stored: await storedSearch(page, 'media_search'),
  }, { input: 'type:Image', stored: 'type:Image' });
  // the older saved searches are plain words
  await page.evaluate(k => localStorage.setItem(k, JSON.stringify('lighthouse beach')), STORAGE('media_search'));
  await page.goto(BASE + '/gallery');
  await page.waitForSelector(INPUT);
  await sleep(600);
  raw = count(media("m.ts_vec @@ (to_tsquery('english', quote_literal(:'v0')) || to_tsquery('english', quote_literal(:'v1')))"), { v0: 'lighthouse', v1: 'beach' });
  got = await settleTotal(page, raw);
  check(s, 'an old saved search of plain words still works', 'ts_vec @@ lighthouse | beach', {
    input: await inputValue(page),
    chips: await chipLabels(page),
    total: got,
  }, { input: 'lighthouse beach', chips: ['Words: lighthouse, beach'], total: raw });

  // the nav link stays active with a search in the URL
  await open(page, '/gallery', 'tag:ct-mod7');
  const active = await page.$$eval('.nav-link.active', els => els.map(e => e.textContent));
  check(s, 'nav link stays active with ?q=', '-', { active }, { active: ['Gallery'] });

  // ================================================================ 6. back navigation
  s = section('back-navigation from a media page still lands on the same card', HEADER);
  // type:Image matches media that have files on disk (newest first); the counttest seed has no
  // files, so its thumbnails 404 and the cards collapse after the router has restored the scroll
  await open(page, '/gallery', 'type:Image');
  for (let i = 0; i < 400 && loadedOf(await statusText(page)) < 240; i++) {
    await page.mouse.wheel(0, 500);
    await sleep(120);
  }
  await sleep(1200);
  await page.mouse.wheel(0, 300);
  await sleep(1200);
  const pickCard = () => page.evaluate(() => {
    let best = null;
    for (const c of document.querySelectorAll('.media-card')) {
      const r = c.getBoundingClientRect();
      if (r.top > 150 && r.bottom < innerHeight - 20) {
        const d = Math.abs(r.top + r.height / 2 - innerHeight / 2);
        if (!best || d < best.d) best = { d, x: Math.round(r.left + r.width / 2), y: Math.round(r.top + r.height / 2) };
      }
    }
    return best;
  });
  const under = (x, y) => page.evaluate(([x, y]) => {
    for (const c of document.querySelectorAll('.media-card')) {
      const r = c.getBoundingClientRect();
      if (r.left <= x && r.right >= x && r.top <= y && r.bottom >= y) {
        return { href: c.querySelector('a').getAttribute('href').split('/').pop().split('?')[0], top: Math.round(r.top) };
      }
    }
    return null;
  }, [x, y]);
  const P = await pickCard();
  const before = await under(P.x, P.y);
  const loadedBefore = loadedOf(await statusText(page));
  await page.mouse.click(P.x, P.y);
  await page.waitForURL(/gallery\/[0-9a-f-]{36}/);
  await sleep(1500);
  await page.goBack();
  await sleep(3000);
  const afterCard = await under(P.x, P.y);
  check(s, 'gallery?q=type:Image, back from a card', '-', {
    same_card: !!(before && afterCard && before.href === afterCard.href),
    top_within_4px: !!(before && afterCard && Math.abs(afterCard.top - before.top) <= 4),
    url_q: urlQ(page),
    input: await inputValue(page),
    loaded_at_least_before: loadedOf(await statusText(page)) >= Math.min(loadedBefore, 60),
  }, { same_card: true, top_within_4px: true, url_q: 'type:Image', input: 'type:Image', loaded_at_least_before: true });

  await t.browser.close();
  printGrids();
  process.exit(FAILED ? 1 : 0);
})().catch(e => {
  console.error(e);
  process.exit(2);
});
