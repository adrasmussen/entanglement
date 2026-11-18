// Browser checks for grid navigation: the pinned search row (gallery, collections list, collection and library
// pages) and the previous/next arrows on the media detail page.  Prints grids with a computed
// match column; exits non-zero on any mismatch.
//
// needs the dev stack up, docker/dev-certs/ generated and playwright installed:
//   node docker/devtest/ui_nav_check.js            (from the repo root, playwright resolvable)
//   PW_MODULE=/path/to/node_modules node docker/devtest/ui_nav_check.js
//   SHOTS=/some/dir  also saves screenshots there
//
// it needs a collection and a library of dev's whose media have real files (the counttest rows
// have none); it picks the largest at runtime and prints which.  it creates nothing.
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
const SHOTS = process.env.SHOTS || '';
const sleep = ms => new Promise(r => setTimeout(r, ms));
const psql = q => execFileSync('docker', ['exec', '-i', 'entanglement-postgres-1', 'psql', '-U', 'entanglement',
  '-d', 'entanglement', '-At', '-v', 'ON_ERROR_STOP=1'], { input: q }).toString().trim();

const grids = {};
let fails = 0;
function check(grid, name, observed, expected) {
  const o = JSON.stringify(observed), e = JSON.stringify(expected);
  const ok = o === e;
  if (!ok) fails++;
  (grids[grid] = grids[grid] || []).push([name, e, o, ok ? 'OK' : 'FAIL']);
}
// a numeric comparison whose rows show both numbers
function checkLess(grid, name, a, b) {
  const ok = a < b;
  if (!ok) fails++;
  (grids[grid] = grids[grid] || []).push([name, `${a} < ${b}`, `${a} vs ${b}`, ok ? 'OK' : 'FAIL']);
}

async function open(page, url, sel) {
  for (let i = 0; i < 4; i++) {
    try {
      await page.goto(url);
      await page.waitForSelector(sel, { timeout: 20000 });
      await sleep(900);
      return;
    } catch (e) {
      if (i === 3) throw e;
      await sleep(500);
    }
  }
}

const uuidOf = u => (u.match(/\/gallery\/([0-9a-f-]{36})/) || [])[1] || null;
const detailUuid = page => uuidOf(page.url());
async function waitUrlChange(page, from) {
  for (let i = 0; i < 40; i++) {
    if (page.url() !== from) { await sleep(250); return; }
    await sleep(100);
  }
}
const cardOrder = page => page.$$eval('#media-grid .media-card a[href]', as =>
  as.map(a => (a.getAttribute('href').match(/gallery\/([0-9a-f-]{36})/) || [])[1]));

// ---- the pinned row
async function measure(page, cardSel = '#media-grid .media-card') {
  return page.evaluate(cardSel => {
    const r = s => { const e = document.querySelector(s); return e ? e.getBoundingClientRect() : null; };
    const sticky = r('.sticky-header'), hero = r('.page-hero'), input = r('.sticky-header input[name="search_filter"]');
    const app = r('.app-header');
    const cards = [...document.querySelectorAll(cardSel)];
    const first = cards.find(c => c.getBoundingClientRect().bottom > sticky.bottom);
    const t = document.querySelector('.sticky-header .search-title');
    return {
      scrollY: Math.round(window.scrollY),
      docH: document.scrollingElement.scrollHeight,
      stickyH: sticky.height, stickyTop: sticky.top, stickyBottom: sticky.bottom,
      appBottom: app.bottom,
      heroBottom: hero ? hero.bottom : null,
      inputInView: !!input && input.top >= sticky.top - 1 && input.bottom <= window.innerHeight && input.width > 0 && input.height > 0,
      title: t ? (getComputedStyle(t).display === 'none' ? null : t.textContent) : null,
      firstCardTop: cards.length ? cards[0].getBoundingClientRect().top : null,
      firstVisibleCardTop: first ? first.getBoundingClientRect().top : null,
      innerH: window.innerHeight,
      rowPx: cards.length > 1 ? (() => {
        const t0 = cards[0].getBoundingClientRect().top;
        const next = cards.find(c => c.getBoundingClientRect().top > t0 + 5);
        return next ? next.getBoundingClientRect().top - t0 : 0;
      })() : 0,
    };
  }, cardSel);
}

async function layoutCase(ctx, grid, vp, url, expectTitle, shotName, cardSel = '#media-grid .media-card') {
  const page = await ctx.newPage();
  await page.setViewportSize(vp);
  const errors = [];
  page.on('pageerror', e => errors.push(e.message));
  await open(page, url, cardSel);
  const before = await measure(page, cardSel);
  if (SHOTS) await page.screenshot({ path: `${SHOTS}/${shotName}_top_${vp.width}.png` });
  await page.evaluate(y => window.scrollTo(0, y), Math.round(before.rowPx * 3));
  await sleep(900);
  const after = await measure(page, cardSel);
  await sleep(700);
  const later = await measure(page, cardSel);
  if (SHOTS) await page.screenshot({ path: `${SHOTS}/${shotName}_scrolled_${vp.width}.png` });

  const g = `${grid} @ ${vp.width}x${vp.height}`;
  check(g, 'scrolled ~3 rows (scrollY > 0)', after.scrollY > 0, true);
  check(g, 'search input still in the viewport', after.inputInView, true);
  check(g, 'big header block is out of view (bottom <= pinned top)', after.heroBottom <= after.stickyTop, true);
  check(g, 'compact title in the pinned row', after.title, expectTitle);
  check(g, 'compact title hidden before scrolling', before.title, null);
  check(g, 'pinned row height unchanged by scrolling', after.stickyH, before.stickyH);
  checkLess(g, 'photo space grows: pinned bottom after < first card top before', after.stickyBottom, before.firstCardTop);
  check(g, 'no jitter: scrollY and document height stable', [later.scrollY, later.docH], [after.scrollY, after.docH]);
  check(g, 'no page errors', errors, []);
  await page.close();
  return { before, after };
}

(async () => {
  // a collection and a library with real files: the largest with a served thumbnail is assumed
  const coll = psql(`SELECT c.collection_uuid || '|' || c.name FROM collections c
      JOIN collection_contents cc USING (collection_uuid) JOIN media m USING (media_uuid)
      JOIN libraries l USING (library_uuid)
      WHERE c.gid = 'devgroup' AND l.path NOT LIKE '/counttest%' AND NOT m.hidden
      GROUP BY c.collection_uuid, c.name ORDER BY count(*) DESC, c.name LIMIT 1`).split('|');
  const lib = psql(`SELECT library_uuid || '|' || path FROM libraries
      WHERE gid = 'devgroup' AND path NOT LIKE '/counttest%' ORDER BY count DESC, path LIMIT 1`).split('|');
  const [CU, CNAME] = coll, [LU, LPATH] = lib;
  console.log(`collection used: ${CNAME} (${CU})\nlibrary used:    ${LPATH} (${LU})\n`);

  const browser = await playwright.chromium.launch();
  const ctx = await browser.newContext({
    ignoreHTTPSErrors: true,
    viewport: { width: 1440, height: 900 },
    clientCertificates: [{ origin: ORIGIN, certPath: CERTS + 'client.crt', keyPath: CERTS + 'client.key' }],
  });
  const desktop = { width: 1440, height: 900 }, phone = { width: 390, height: 844 };

  // ---- pinned row
  for (const vp of [desktop, phone]) {
    await layoutCase(ctx, 'gallery', vp, BASE + '/gallery', vp === phone ? null : 'Gallery', 'gallery');
    await layoutCase(ctx, 'collection', vp, BASE + '/collections/' + CU, vp === phone ? null : CNAME, 'collection');
    await layoutCase(ctx, 'library', vp, BASE + '/library/' + LU, vp === phone ? null : LPATH, 'library');
    await layoutCase(ctx, 'collections list', vp, BASE + '/collections', vp === phone ? null : 'Collections', 'collections',
      '.collections-grid .collection-card:not(.loading)');
  }

  // ---- arrows, from the collection
  const page = await ctx.newPage();
  await page.setViewportSize(desktop);
  const errors = [];
  page.on('pageerror', e => errors.push(e.message));
  const G = 'arrows from the collection';

  const sqlOrder = psql(`SELECT m.media_uuid FROM media m JOIN collection_contents cc USING (media_uuid)
      WHERE cc.collection_uuid = '${CU}' AND NOT m.hidden ORDER BY m.date DESC, m.media_uuid DESC`).split('\n');

  await open(page, BASE + '/collections/' + CU, '#media-grid .media-card');
  const dom = await cardOrder(page);
  check(G, `grid cards (${dom.length}) are the head of the SQL order (${sqlOrder.length} rows)`, dom, sqlOrder.slice(0, dom.length));
  check(G, 'collection is big enough for the walk (>= 20 cards)', dom.length >= 20, true);

  const K = 12;
  const clickCard = async k => {
    const a = page.locator('#media-grid .media-card a').nth(k);
    await a.scrollIntoViewIfNeeded();
    await page.evaluate(i => {
      const el = document.querySelectorAll('#media-grid .media-card')[i];
      window.scrollTo(0, window.scrollY + el.getBoundingClientRect().top - 300);
    }, k);
    await sleep(500);
    await a.click();
    await page.waitForSelector('.media-detail-page');
    await sleep(600);
  };
  const press = async (key, mods) => {
    const from = page.url();
    await page.keyboard.press(mods ? mods + '+' + key : key);
    await waitUrlChange(page, from);
  };

  await clickCard(K);
  check(G, `clicked card ${K} opens its detail page`, detailUuid(page), dom[K]);
  check(G, 'nav bar shows position', await page.textContent('.grid-nav-pos'), `${K + 1} / ${dom.length}`);
  check(G, 'nav bar names the collection', (await page.textContent('.grid-nav-back')).trim(), `← ${CNAME}`);
  if (SHOTS) await page.screenshot({ path: `${SHOTS}/detail_arrows.png` });

  // walk: right x3, left x5
  const keys = ['ArrowRight', 'ArrowRight', 'ArrowRight', 'ArrowLeft', 'ArrowLeft', 'ArrowLeft', 'ArrowLeft', 'ArrowLeft'];
  let idx = K;
  const visited = [], expected = [];
  for (const k of keys) {
    await press(k);
    idx += k === 'ArrowRight' ? 1 : -1;
    visited.push(detailUuid(page));
    expected.push(sqlOrder[idx]);
  }
  check(G, 'visited uuids after R,R,R,L,L,L,L,L vs raw SQL order', visited, expected);
  check(G, 'visited uuids equal the grid DOM order', visited, keys.map((_, i) => dom[K + [1, 2, 3, 2, 1, 0, -1, -2][i]]));
  check(G, 'position text follows', await page.textContent('.grid-nav-pos'), `${K - 1} / ${dom.length}`);

  // typing, modifiers and modals do not navigate
  const at = page.url();
  await page.focus('input[name="tags"]');
  await page.keyboard.press('ArrowRight');
  await sleep(500);
  check(G, 'ArrowRight while typing in an input does nothing', page.url(), at);
  await page.focus('textarea[name="note"]');
  await page.keyboard.press('ArrowLeft');
  await sleep(500);
  check(G, 'ArrowLeft while typing in a textarea does nothing', page.url(), at);
  await page.evaluate(() => document.activeElement && document.activeElement.blur());
  await page.keyboard.press('Shift+ArrowRight');
  await page.keyboard.press('Control+ArrowRight');
  await sleep(500);
  check(G, 'Shift/Ctrl + ArrowRight do nothing', page.url(), at);
  await page.click('.media-detail-image');
  await page.waitForSelector('.modal-content, .modal-overlay');
  await sleep(400);
  await page.keyboard.press('ArrowRight');
  await sleep(500);
  check(G, 'ArrowRight while a modal is open does nothing', page.url(), at);
  await page.keyboard.press('Escape');
  await sleep(300);
  if (await page.$('.modal-overlay')) await page.mouse.click(5, 450);
  await sleep(400);
  await press('ArrowRight');
  check(G, 'after closing the modal ArrowRight works again', detailUuid(page), sqlOrder[K - 1]);

  // the Next button
  const from = page.url();
  await page.click('button[aria-label="Next photo"]');
  await waitUrlChange(page, from);
  check(G, 'Next button goes to the next card', detailUuid(page), sqlOrder[K]);

  // Back: one step to the grid, with the originally clicked card in view
  await page.goBack();
  await page.waitForSelector('#media-grid .media-card');
  await sleep(1200);
  const back = await page.evaluate(k => {
    const el = document.querySelectorAll('#media-grid .media-card')[k];
    const r = el.getBoundingClientRect();
    return { url: location.pathname, inView: r.top >= 0 && r.bottom <= window.innerHeight, top: Math.round(r.top) };
  }, K);
  check(G, 'one Back after 10 arrow moves lands on the collection page', back.url, '/entanglement/app/collections/' + CU);
  check(G, 'the originally clicked card is in view (and near where it was, top ~300px)', [back.inView, Math.abs(back.top - 300) < 40], [true, true]);
  if (SHOTS) await page.screenshot({ path: `${SHOTS}/after_back.png` });

  // ---- edges, same collection
  const E = 'edges';
  await open(page, BASE + '/collections/' + CU, '#media-grid .media-card');
  await clickCard(0);
  const first = page.url();
  await page.keyboard.press('ArrowLeft');
  await sleep(600);
  check(E, 'first card: ArrowLeft does nothing', page.url(), first);
  check(E, 'first card: Previous button disabled', await page.isDisabled('button[aria-label="Previous photo"]'), true);
  await page.goBack();
  await page.waitForSelector('#media-grid .media-card');
  await sleep(800);
  // click the last card that is loaded right now, straight through the DOM: scrolling to it
  // would trigger the next page, and the snapshot would then contain more than this card
  const loaded = await page.evaluate(() => {
    const as = document.querySelectorAll('#media-grid .media-card a');
    as[as.length - 1].click();
    return as.length;
  });
  await page.waitForSelector('.media-detail-page');
  await sleep(600);
  const last = page.url();
  const pos = (await page.textContent('.grid-nav-pos')).split(' / ');
  check(E, 'last loaded card is the last of the snapshot (position == total)', pos[0] === pos[1], true);
  await page.keyboard.press('ArrowRight');
  await sleep(600);
  check(E, `last loaded card (${loaded}): ArrowRight does nothing (known gap: no paging)`, page.url(), last);
  check(E, 'last loaded card: Next button disabled', await page.isDisabled('button[aria-label="Next photo"]'), true);

  // ---- deep link: no context, no arrows, no errors
  const D = 'deep link';
  await open(page, BASE + '/gallery/' + sqlOrder[3], '.media-detail-page');
  check(D, 'no nav bar', await page.$('.grid-nav'), null);
  await page.keyboard.press('ArrowRight');
  await sleep(500);
  check(D, 'ArrowRight does nothing', page.url(), BASE + '/gallery/' + sqlOrder[3]);

  // ---- arrows from the gallery: the DOM order, and the head of the raw SQL gallery order
  const GG = 'arrows from the gallery';
  await open(page, BASE + '/gallery', '#media-grid .media-card');
  const gdom = await cardOrder(page);
  const gsql = psql(`SELECT m.media_uuid FROM media m JOIN libraries l USING (library_uuid)
      WHERE NOT m.hidden AND (l.gid = 'devgroup' OR EXISTS (SELECT 1 FROM collection_contents cc
        JOIN collections c USING (collection_uuid) WHERE cc.media_uuid = m.media_uuid AND c.gid = 'devgroup'))
      ORDER BY m.date DESC, m.media_uuid DESC LIMIT ${gdom.length}`).split('\n');
  check(GG, `grid cards (${gdom.length}) are the head of the SQL gallery order`, gdom, gsql);
  await clickCard(2);
  const gv = [detailUuid(page)];
  for (const k of ['ArrowRight', 'ArrowRight', 'ArrowLeft']) { await press(k); gv.push(detailUuid(page)); }
  check(GG, 'click 2 then R,R,L visits cards 2,3,4,3 of the SQL order', gv, [gsql[2], gsql[3], gsql[4], gsql[3]]);
  check(GG, 'nav bar names the gallery', (await page.textContent('.grid-nav-back')).trim(), '← Gallery');

  check('page errors', 'no page errors across the arrow walks', errors, []);
  await page.close();

  // ---- phone: buttons work
  const P = 'phone buttons';
  const pp = await ctx.newPage();
  await pp.setViewportSize(phone);
  await open(pp, BASE + '/collections/' + CU, '#media-grid .media-card');
  await pp.locator('#media-grid .media-card a').nth(1).click();
  await pp.waitForSelector('.grid-nav');
  await sleep(600);
  const vis = await pp.evaluate(() => [...document.querySelectorAll('.grid-nav button')].map(b => {
    const r = b.getBoundingClientRect();
    return r.width > 0 && r.right <= window.innerWidth && r.left >= 0;
  }));
  check(P, 'Previous/Next buttons fit inside the phone viewport', vis, [true, true]);
  if (SHOTS) await pp.screenshot({ path: `${SHOTS}/detail_phone.png` });
  const f2 = pp.url();
  await pp.click('button[aria-label="Next photo"]');
  await waitUrlChange(pp, f2);
  check(P, 'Next on the phone goes to card 2', detailUuid(pp), sqlOrder[2]);
  await pp.close();

  await browser.close();

  for (const [name, rows] of Object.entries(grids)) {
    const hdr = ['case', 'expected', 'observed', 'match'];
    const cut = s => s.length > 70 ? s.slice(0, 67) + '...' : s;
    const rr = rows.map(r => r.map(cut));
    const w = [0, 1, 2, 3].map(i => Math.max(...rr.map(r => r[i].length), hdr[i].length));
    const line = r => r.map((c, i) => c.padEnd(w[i])).join(' | ');
    console.log(`== ${name}`);
    console.log(line(hdr));
    console.log(w.map(x => '-'.repeat(x)).join('-+-'));
    rr.forEach(r => console.log(line(r)));
    console.log('');
  }
  const total = Object.values(grids).reduce((n, r) => n + r.length, 0);
  console.log(`${total - fails}/${total} cases match`);
  console.log(fails ? `${fails} CASE(S) FAILED` : 'ALL CASES MATCH');
  process.exit(fails ? 1 : 0);
})();
