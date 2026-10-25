// Mesures de référence du site en production, en lecture seule (aucun formulaire, aucune connexion).
//
//   node site.mjs <lighthouse|shots|checks> --lh <dossier contenant node_modules/lighthouse>
//                 --results <dossier_résultats> [--raw <dossier des rapports complets>]
//                 [--pages /,/download/] [--delay 75]
//
// lighthouse : pages × mobile/ordinateur × 3 passes, Chrome neuf à chaque passe (cache vide).
//              Résumé → site-lighthouse.json ; rapports complets → --raw (non archivés, trop lourds).
// shots      : parcours clavier (12 premiers Tab), puis captures de la partie visible à
//              375/768/1024/1440/1920 px, thèmes sombre et clair, + détection du défilement
//              horizontal → site-shots.json, shots/site/*.jpg.
// checks     : HTML de production (requêtes simples) : version affichée, mentions macOS,
//              promesses à vérifier → site-checks.json.
// Options : --pages /,/download/ (liste réduite) ; --delay 75 (secondes entre deux chargements).

import { mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';

const args = Object.fromEntries(process.argv.slice(3).reduce((a, x, i, l) => (x.startsWith('--') ? [...a, [x.slice(2), l[i + 1]]] : a), []));
const mode = process.argv[2];
const mod = (p) => import(pathToFileURL(join(args.lh, 'node_modules', p)).href);
const CHROME = 'C:/Program Files/Google/Chrome/Application/chrome.exe';
// --site http://localhost:4321 : copie locale (dist servi par serve_dist.mjs) au lieu de la production.
const SITE = args.site ?? 'https://morph.underskydev.com';
const PAGES = ['/', '/en/', '/download/', '/formats/', '/convert/', '/convert/png-to-webp/',
  '/convert/mp4-to-gif/', '/convert/docx-to-pdf/', '/reviews/', '/account/'];
const results = args.results;
const save = (name, data) => {
  writeFileSync(join(results, name), JSON.stringify(data, null, 2));
  console.log(name, 'écrit');
};
const median = (v) => { const s = [...v].sort((a, b) => a - b); const m = s.length >> 1; return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2; };
const spread = (v) => { const m = median(v); return m ? Math.round(((Math.max(...v) - Math.min(...v)) / m) * 1000) / 10 : 0; };

// L'hébergeur (o2switch, Tiger Protect) renvoie HTTP 429 au-delà d'un certain débit :
// --pages réduit la liste, --delay espace les chargements (secondes), et tout s'arrête au premier 429.
const pages = args.pages ? args.pages.split(',') : PAGES;
const delay = Number(args.delay ?? 0) * 1000;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let blocked = false;

async function lighthouse() {
  const { default: lh } = await mod('lighthouse/core/index.js');
  const { default: desktop } = await mod('lighthouse/core/config/desktop-config.js');
  const chromeLauncher = await mod('chrome-launcher/dist/index.js');
  if (args.raw) mkdirSync(args.raw, { recursive: true });
  const runs = [];
  for (let pass = 1; pass <= 3 && !blocked; pass++) {
    for (const path of pages) {
      for (const ff of ['mobile', 'desktop']) {
        if (blocked) break;
        if (runs.length) await sleep(delay);
        const chrome = await chromeLauncher.launch({ chromePath: CHROME, chromeFlags: ['--headless=new', '--no-first-run'] });
        try {
          const r = await lh(SITE + path, { port: chrome.port, output: 'json', logLevel: 'error' }, ff === 'desktop' ? desktop : undefined);
          const L = r.lhr, a = L.audits;
          const status = a['network-requests']?.details?.items?.[0]?.statusCode ?? null;
          if (status !== 200) {
            blocked = true;
            runs.push({ path, ff, pass, error: `HTTP ${status}` });
            console.log('ARRÊT : statut', status, path, ff, pass);
            break;
          }
          const items = a['resource-summary']?.details?.items ?? [];
          const kind = (t) => items.find((x) => x.resourceType === t) ?? {};
          const run = {
            path, ff, pass,
            perf: Math.round(L.categories.performance.score * 100),
            a11y: Math.round(L.categories.accessibility.score * 100),
            bp: Math.round(L.categories['best-practices'].score * 100),
            seo: Math.round(L.categories.seo.score * 100),
            fcp_ms: Math.round(a['first-contentful-paint'].numericValue),
            lcp_ms: Math.round(a['largest-contentful-paint'].numericValue),
            tbt_ms: Math.round(a['total-blocking-time'].numericValue),
            cls: Math.round(a['cumulative-layout-shift'].numericValue * 1000) / 1000,
            si_ms: Math.round(a['speed-index'].numericValue),
            total_kb: Math.round((a['total-byte-weight'].numericValue ?? 0) / 1024),
            script_kb: Math.round((kind('script').transferSize ?? 0) / 1024),
            script_count: kind('script').requestCount ?? 0,
            font_kb: Math.round((kind('font').transferSize ?? 0) / 1024),
            image_kb: Math.round((kind('image').transferSize ?? 0) / 1024),
            requests: kind('total').requestCount ?? 0,
            lcp_element: a['largest-contentful-paint-element']?.details?.items?.[0]?.items?.[0]?.node?.snippet ?? null,
            a11y_failed: Object.values(L.categories.accessibility.auditRefs)
              .map((ref) => a[ref.id]).filter((x) => x && x.score === 0).map((x) => x.id),
            runtime_error: L.runtimeError?.code ?? null,
          };
          runs.push(run);
          console.log(JSON.stringify(run).slice(0, 220));
          if (args.raw && pass === 1) writeFileSync(join(args.raw, `${ff}${path.replaceAll('/', '_')}.json`), r.report);
        } catch (e) {
          runs.push({ path, ff, pass, error: String(e) });
          console.log('erreur', path, ff, pass, String(e));
        } finally {
          await chrome.kill();
        }
      }
    }
  }
  const summary = {};
  for (const path of pages) for (const ff of ['mobile', 'desktop']) {
    const r = runs.filter((x) => x.path === path && x.ff === ff && !x.error);
    if (!r.length) continue;
    const s = {};
    for (const k of ['perf', 'a11y', 'bp', 'seo', 'fcp_ms', 'lcp_ms', 'tbt_ms', 'cls', 'si_ms', 'total_kb', 'script_kb', 'requests'])
      s[k] = { values: r.map((x) => x[k]), median: median(r.map((x) => x[k])), spread_pct: spread(r.map((x) => x[k])) };
    s.a11y_failed = [...new Set(r.flatMap((x) => x.a11y_failed))];
    s.lcp_element = r[0].lcp_element;
    summary[`${ff} ${path}`] = s;
  }
  save('site-lighthouse.json', { site: SITE, date: new Date().toISOString(), delay_s: delay / 1000, blocked, runs, summary });
}

async function browser() {
  const { default: puppeteer } = await mod('puppeteer-core/lib/puppeteer/puppeteer-core.js');
  return puppeteer.launch({ executablePath: CHROME, headless: true, args: ['--no-first-run'] });
}

async function shots() {
  // Un seul chargement par page : largeurs et thèmes sont changés sur place (le site applique
  // son thème par l'attribut data-theme, sombre par défaut, sans suivre le système).
  const b = await browser();
  const local = SITE.startsWith('http://localhost');
  const dir = join(results, 'shots', local ? 'site-local' : 'site');
  mkdirSync(dir, { recursive: true });
  const out = [], keyboard = {};
  for (const [i, path] of pages.entries()) {
    if (blocked) break;
    if (i) await sleep(delay);
    const p = await b.newPage();
    await p.setViewport({ width: 1440, height: 900 });
    const res = await p.goto(SITE + path, { waitUntil: 'networkidle0', timeout: 60000 });
    if (res.status() !== 200) {
      blocked = true;
      console.log('ARRÊT : statut', res.status(), path);
      break;
    }
    await sleep(1500);
    // Parcours clavier : 12 premiers Tab depuis le haut de la page.
    const seq = [];
    for (let k = 0; k < 12; k++) {
      await p.keyboard.press('Tab');
      seq.push(await p.evaluate(() => {
        const e = document.activeElement;
        if (!e || e === document.body) return null;
        const cs = getComputedStyle(e);
        const r = e.getBoundingClientRect();
        return { tag: e.tagName.toLowerCase(), text: (e.innerText || e.getAttribute('aria-label') || '').trim().slice(0, 40),
          href: e.getAttribute('href'), outline: cs.outlineStyle !== 'none' && cs.outlineWidth !== '0px', boxShadow: cs.boxShadow !== 'none',
          visible: r.width > 0 && r.height > 0 };
      }));
    }
    keyboard[path] = seq;
    await p.evaluate(() => { document.activeElement?.blur(); window.scrollTo(0, 0); });
    for (const width of [375, 768, 1024, 1440, 1920]) {
      for (const theme of ['dark', 'light']) {
        await p.emulateMediaFeatures([{ name: 'prefers-color-scheme', value: theme }]);
        await p.evaluate((t) => { document.documentElement.dataset.theme = t; window.scrollTo(0, 0); }, theme);
        await p.setViewport({ width, height: width < 768 ? 812 : width < 1024 ? 1024 : width === 1024 ? 768 : width === 1440 ? 900 : 1080, deviceScaleFactor: 1 });
        await sleep(1200);
        const m = await p.evaluate(() => ({
          scrollWidth: document.documentElement.scrollWidth,
          clientWidth: document.documentElement.clientWidth,
          pageHeight: document.documentElement.scrollHeight,
          theme: document.documentElement.dataset.theme,
          wide: [...document.querySelectorAll('body *')].filter((e) => e.getBoundingClientRect().right > document.documentElement.clientWidth + 1)
            .slice(0, 5).map((e) => e.tagName.toLowerCase() + (e.className && typeof e.className === 'string' ? '.' + e.className.trim().split(/\s+/).slice(0, 3).join('.') : '')),
        }));
        const file = `${path === '/' ? 'home' : path.replaceAll('/', ' ').trim().replaceAll(' ', '_')}-${width}-${theme}.jpg`;
        await p.screenshot({ path: join(dir, file), type: 'jpeg', quality: 70 });
        out.push({ path, width, theme, file, horizontal_overflow: m.scrollWidth > m.clientWidth, ...m });
        console.log(path, width, theme, m.scrollWidth > m.clientWidth ? 'DÉBORDE' : 'ok', m.theme);
      }
    }
    await p.close();
  }
  await b.close();
  save(local ? 'site-shots-local.json' : 'site-shots.json', { site: SITE, delay_s: delay / 1000, blocked, shots: out, keyboard });
}

async function checks() {
  const out = { pages: {} };
  const terms = ['macOS', 'Mac', 'v1.', 'parallèle', 'parallel', 'centaines', 'hundreds', 'Flouter', 'Blur', 'hors ligne', 'offline', 'gratuit', 'free', 'GPU', 'carte graphique'];
  for (const path of ['/', '/en/', '/download/', '/en/download/', '/faq/', '/en/faq/', '/formats/', '/changelog/']) {
    const res = await fetch(SITE + path);
    const html = await res.text();
    const text = html.replace(/<script[\s\S]*?<\/script>/g, ' ').replace(/<style[\s\S]*?<\/style>/g, ' ').replace(/<[^>]+>/g, ' ').replace(/\s+/g, ' ');
    const hits = {};
    for (const t of terms) {
      const re = new RegExp(t.replace('.', '\\.'), 'g');
      const ctx = [];
      let m;
      while ((m = re.exec(text)) && ctx.length < 4) ctx.push(text.slice(Math.max(0, m.index - 60), m.index + 60).trim());
      if (ctx.length) hits[t] = ctx;
    }
    out.pages[path] = { status: res.status, bytes: html.length, versions: [...new Set(text.match(/v?\d+\.\d+(\.\d+)?/g) ?? [])].filter((v) => /^v?1\.\d/.test(v)), hits };
    console.log(path, res.status, Object.keys(hits).join(', '));
  }
  save('site-checks.json', out);
}

mkdirSync(results, { recursive: true });
// lhshots : extrait la capture finale de chaque rapport Lighthouse complet (--raw), prise en
// production par Lighthouse lui-même (mobile 412 px, ordinateur 1350 px, thème par défaut).
async function lhshots() {
  const dir = join(results, 'shots', 'site-lighthouse');
  mkdirSync(dir, { recursive: true });
  for (const f of readdirSync(args.raw).filter((x) => x.endsWith('.json'))) {
    const L = JSON.parse(readFileSync(join(args.raw, f), 'utf-8'));
    const data = L.audits['final-screenshot']?.details?.data;
    if (!data) continue;
    const name = f.replace(/_?\.json$/, '');
    writeFileSync(join(dir, `${name}.jpg`), Buffer.from(data.split(',')[1], 'base64'));
    console.log(name);
  }
}

await { lighthouse, shots, checks, lhshots }[mode]();
