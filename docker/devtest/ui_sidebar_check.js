// Browser checks for the Advanced sidebar's Collection Labels tab: the sidebar scrolls as a
// whole, the collection list has no scroll box of its own, and the collection search stays
// pinned at the top.  Desktop and phone widths.  Prints a grid with a computed match column;
// exits non-zero on any mismatch.
//
// needs the dev stack up, docker/dev-certs/ generated and playwright installed:
//   node docker/devtest/ui_sidebar_check.js            (from the repo root, playwright resolvable)
//   PW_MODULE=/path/to/node_modules node docker/devtest/ui_sidebar_check.js
//   SHOTS=/some/dir  also saves screenshots there
//
// it needs enough devgroup collections to overflow the sidebar (the counttest/pagetest seeds
// have them; the count is printed).  it creates nothing.
const path = require('path');
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

const rows = [];
let fails = 0;
function check(name, observed, expected) {
  const o = JSON.stringify(observed), e = JSON.stringify(expected);
  const ok = o === e;
  if (!ok) fails++;
  rows.push([name, e, o, ok ? 'OK' : 'FAIL']);
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

(async () => {
  const browser = await playwright.chromium.launch();
  const ctx = await browser.newContext({
    ignoreHTTPSErrors: true,
    clientCertificates: [{ origin: ORIGIN, certPath: CERTS + 'client.crt', keyPath: CERTS + 'client.key' }],
  });
  const errors = [];

  for (const vp of [{ width: 1440, height: 900 }, { width: 390, height: 844 }]) {
    const at = `(${vp.width}px)`;
    const page = await ctx.newPage();
    page.on('pageerror', e => errors.push(e.message));
    await page.setViewportSize(vp);
    await open(page, BASE + '/gallery', '#media-grid .media-card');
    await page.click('button:has-text("Advanced")');
    await page.click('.tab-button:has-text("Collection Labels")');
    await page.waitForSelector('.available-collections > *', { timeout: 20000 });
    await sleep(600);

    const before = await page.evaluate(() => {
      const body = document.querySelector('.sidebar-body');
      const list = document.querySelector('.available-collections');
      return { scrolls: body.scrollHeight > body.clientHeight + 1, listScrolls: list.scrollHeight > list.clientHeight + 1,
               items: list.children.length };
    });
    check(`${at} the sidebar overflows (${before.items} collections listed)`, before.scrolls, true);
    check(`${at} the collection list has no scroll box of its own`, before.listScrolls, false);

    await page.evaluate(() => { const b = document.querySelector('.sidebar-body'); b.scrollTop = b.scrollHeight; });
    await sleep(300);
    const after = await page.evaluate(() => {
      const body = document.querySelector('.sidebar-body').getBoundingClientRect();
      const search = document.querySelector('.collection-color-search').getBoundingClientRect();
      const label = [...document.querySelectorAll('.sidebar-body .form-label')]
        .find(e => e.textContent.trim() === 'Collection Colors').getBoundingClientRect();
      const input = document.querySelector('.collection-color-search input');
      const r = input.getBoundingClientRect();
      const hit = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2);
      return { pinned: Math.abs(search.top - body.top) <= 1, labelGone: label.bottom <= search.top, onTop: hit === input };
    });
    check(`${at} scrolled to the bottom: the search sits flush at the sidebar top`, after.pinned, true);
    check(`${at} the "Collection Colors" text has scrolled away above it`, after.labelGone, true);
    check(`${at} the search input is on top and clickable`, after.onTop, true);
    if (SHOTS) await page.screenshot({ path: `${SHOTS}/collection_labels_${vp.width}.png` });
    await page.close();
  }
  check('no page errors', errors, []);

  await browser.close();

  const hdr = ['case', 'expected', 'observed', 'match'];
  const w = [0, 1, 2, 3].map(i => Math.max(...rows.map(r => r[i].length), hdr[i].length));
  const line = r => r.map((c, i) => c.padEnd(w[i])).join(' | ');
  console.log('== advanced sidebar: Collection Labels tab');
  console.log(line(hdr));
  console.log(w.map(x => '-'.repeat(x)).join('-+-'));
  rows.forEach(r => console.log(line(r)));
  console.log(fails ? `\n${fails} CASE(S) FAILED` : '\nALL CASES MATCH');
  process.exit(fails ? 1 : 0);
})();
