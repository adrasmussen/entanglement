// Browser checks for the create/edit collection modal: spacing, the close button, and tags
// typed with any separator.  Prints a grid with a computed match column; exits non-zero on
// any mismatch.
//
// needs the dev stack up, docker/dev-certs/ generated and playwright installed:
//   node docker/devtest/ui_modal_check.js            (from the repo root, playwright resolvable)
//   PW_MODULE=/path/to/node_modules node docker/devtest/ui_modal_check.js
//
// it creates one collection named counttest-modal-<timestamp> (gid devgroup), which the
// counttest cleanup removes (collections WHERE name LIKE 'counttest-%'); it also deletes it
// itself at the end.
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
const sleep = ms => new Promise(r => setTimeout(r, ms));
const psql = q => execFileSync('docker', ['exec', '-i', 'entanglement-postgres-1', 'psql', '-U', 'entanglement',
  '-d', 'entanglement', '-At', '-v', 'ON_ERROR_STOP=1'], { input: q }).toString().trim();

const rows = [];
let fails = 0;
function check(name, observed, expected) {
  const o = JSON.stringify(observed), e = JSON.stringify(expected);
  const ok = o === e;
  if (!ok) fails++;
  rows.push([name, e, o, ok ? 'OK' : 'FAIL']);
}

async function openCreate(page) {
  await page.goto(BASE + '/collections');
  await page.waitForSelector('button:has-text("Create Collection")');
  await sleep(800);
  await page.click('button:has-text("Create Collection")');
  await page.waitForSelector('.modal-content');
  await sleep(400);
}

(async () => {
  const browser = await playwright.chromium.launch();
  const ctx = await browser.newContext({
    ignoreHTTPSErrors: true,
    viewport: { width: 1440, height: 900 },
    clientCertificates: [{ origin: ORIGIN, certPath: CERTS + 'client.crt', keyPath: CERTS + 'client.key' }],
  });
  const page = await ctx.newPage();
  page.on('pageerror', e => console.log('PAGEERROR', e.message));

  // ---- layout
  await openCreate(page);
  const layout = await page.evaluate(() => {
    const r = s => document.querySelector(s).getBoundingClientRect();
    const modal = r('.modal-content'), input = r('.modal-body .form-input'), close = r('.btn-close');
    const cs = getComputedStyle(document.querySelector('.btn-close'));
    return {
      modal_width: Math.round(modal.width),
      inset_left: Math.round(input.left - modal.left),
      inset_right: Math.round(modal.right - input.right),
      close_w: Math.round(close.width),
      close_h: Math.round(close.height),
      close_has_icon: !!document.querySelector('.btn-close svg'),
      close_label: document.querySelector('.btn-close').getAttribute('aria-label'),
      close_bg_transparent: cs.backgroundColor === 'rgba(0, 0, 0, 0)',
      textarea_monospace: /mono/i.test(getComputedStyle(document.querySelector('.modal-body .form-textarea')).fontFamily),
    };
  });
  check('modal fills its Medium max-width (600px)', layout.modal_width, 600);
  check('controls inset from the modal edge (px, left/right)', [layout.inset_left, layout.inset_right], [24, 24]);
  check('close button is a 36px icon button', {
    w: layout.close_w, h: layout.close_h, icon: layout.close_has_icon, label: layout.close_label, bg: layout.close_bg_transparent,
  }, { w: 36, h: 36, icon: true, label: 'Close', bg: true });
  check('textareas use the page font, not monospace', layout.textarea_monospace, false);
  check('one click opens exactly one create modal', await page.$$eval('.modal-content', m => m.length), 1);

  await page.hover('.btn-close');
  await sleep(250);
  const hoverBg = await page.$eval('.btn-close', e => getComputedStyle(e).backgroundColor);
  check('close button shows a hover background', hoverBg !== 'rgba(0, 0, 0, 0)', true);
  await page.click('.btn-close');
  await sleep(400);
  check('close button closes the modal', await page.$('.modal-content') === null, true);

  // ---- tags: any separator in, a clean set out
  await openCreate(page);
  const tagsInput = page.locator('.modal-body .form-group:has(label:has-text("Tags")) input');
  const help = await page.locator('.modal-body .form-group:has(label:has-text("Tags")) .form-help').textContent();
  check('tags help says no spaces, use underscores', {
    mentions_underscores: /underscore/i.test(help), mentions_spaces: /space/i.test(help),
    tooltip: (await tagsInput.getAttribute('title')) === help,
  }, { mentions_underscores: true, mentions_spaces: true, tooltip: true });

  const name = `counttest-modal-${Date.now()}`;
  await page.fill('.modal-body .form-group:has(label:has-text("Collection Name")) input', name);
  await tagsInput.fill('beach, summer_trip|family; #joy  park/dusk');
  await page.click('.modal-footer button:has-text("Create Collection")');
  await sleep(2500);
  const stored = psql(`SELECT array_to_string(ARRAY(SELECT unnest(akeys(tags)) ORDER BY 1), ',') FROM collections WHERE name = '${name}'`);
  check('typed "beach, summer_trip|family; #joy  park/dusk" is stored as', stored, 'beach,dusk,family,joy,park,summer_trip');

  // ---- edit modal: prefilled with the readable form, and edits parse the same way
  const uuid = psql(`SELECT collection_uuid FROM collections WHERE name = '${name}'`);
  await page.goto(BASE + '/collections/' + uuid);
  await page.waitForSelector('button:has-text("Edit")');
  await sleep(800);
  const shown = await page.evaluate(() => (document.body.innerText.match(/Tags: ([^\n]*)/) || [])[1] || null);
  check('collection page shows tags comma-separated', shown, 'beach, dusk, family, joy, park, summer_trip');
  await page.click('button:has-text("Edit")');
  await page.waitForSelector('.modal-content');
  await sleep(800);
  check('one click opens exactly one edit modal', await page.$$eval('.modal-content', m => m.length), 1);
  const editInput = page.locator('.modal-body .form-group:has(label:has-text("Tags")) input');
  check('edit modal prefills tags comma-separated', await editInput.inputValue(), 'beach, dusk, family, joy, park, summer_trip');
  await editInput.fill('beach;sunset');
  await page.click('.modal-footer button:has-text("Update"), .modal-footer button:has-text("Save")');
  await sleep(2500);
  const edited = psql(`SELECT array_to_string(ARRAY(SELECT unnest(akeys(tags)) ORDER BY 1), ',') FROM collections WHERE name = '${name}'`);
  check('edited "beach;sunset" is stored as', edited, 'beach,sunset');

  await browser.close();
  psql(`DELETE FROM collection_contents WHERE collection_uuid IN (SELECT collection_uuid FROM collections WHERE name = '${name}');
        DELETE FROM collections WHERE name = '${name}';`);

  const w = [0, 1, 2, 3].map(i => Math.max(...rows.map(r => r[i].length), ['case', 'expected', 'observed', 'match'][i].length));
  const line = r => r.map((c, i) => c.padEnd(w[i])).join(' | ');
  console.log('== create / edit collection modal');
  console.log(line(['case', 'expected', 'observed', 'match']));
  console.log(w.map(x => '-'.repeat(x)).join('-+-'));
  rows.forEach(r => console.log(line(r)));
  console.log(fails ? `\n${fails} CASE(S) FAILED` : '\nALL CASES MATCH');
  process.exit(fails ? 1 : 0);
})();
