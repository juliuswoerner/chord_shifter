// Copyright (c) 2026 APSOS — App and Software Solutions Wörner. All rights reserved.
//
// Smoke test for the demo: opens the built web app with ?demo, logs in as the
// demo user, opens a song and exports the PDFs.
//
//   BASE_URL=http://localhost:8099 CHROME=/path/to/chrome node tests/e2e/demo-smoke.mjs
//
// Requires the `playwright` package (npm i playwright).

import { chromium } from 'playwright';
import { execFileSync } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import { inflateSync } from 'node:zlib';

const BASE_URL = process.env.BASE_URL ?? 'http://localhost:8080';
const executablePath = process.env.CHROME || undefined;

const browser = await chromium.launch({ executablePath, args: ['--no-sandbox'] });
const page = await browser.newPage({ viewport: { width: 1200, height: 900 } });
const pageErrors = [];
page.on('pageerror', (e) => pageErrors.push(String(e)));

/** The drawing commands of a PDF: all content streams, inflated when compressed. */
function pdfContent(buffer) {
    const text = buffer.toString('latin1');
    let out = '';
    for (const m of text.matchAll(/stream\r?\n([\s\S]*?)\r?\nendstream/g)) {
        const raw = Buffer.from(m[1], 'latin1');
        try {
            out += inflateSync(raw).toString('latin1');
        } catch {
            out += m[1];
        }
    }
    return out;
}

let step = 'start';
async function check(name, fn) {
    step = name;
    await fn();
    console.log(`ok - ${name}`);
}
function expect(cond, msg) {
    if (!cond) throw new Error(msg);
}

try {
    await check('demo banner is shown with ?demo', async () => {
        await page.goto(`${BASE_URL}/?demo`);
        await page.getByText('DEMO MODE').waitFor({ timeout: 30000 });
    });

    await check('log in as demo user', async () => {
        await page.getByRole('button', { name: 'Log in' }).first().click();
        await page.getByRole('button', { name: 'Log in as demo user' }).click();
        await page.getByText('My Songs').waitFor();
    });

    await check('all demo songs are listed', async () => {
        for (const title of ['Sunrise Ballad', 'Midnight Groove', 'Piano Waltz', 'Twelve Bar Blues']) {
            await page.getByText(title).first().waitFor();
        }
    });

    await check('images load (assets are bundled in the release build)', async () => {
        const broken = await page.evaluate(async () => {
            const imgs = [...document.querySelectorAll('img')];
            await Promise.all(imgs.map((i) => i.decode().catch(() => {})));
            return imgs.filter((i) => !i.complete || i.naturalWidth === 0).map((i) => i.src);
        });
        expect(broken.length === 0, `broken images: ${broken.join(', ')}`);
    });

    await check('a song opens with its chords', async () => {
        await page.getByText('Sunrise Ballad').first().click();
        await page.getByText('Fmaj7').first().waitFor();
    });

    await check('PDF export downloads a ZIP of PDFs', async () => {
        const [download] = await Promise.all([
            page.waitForEvent('download', { timeout: 30000 }),
            page.getByRole('button', { name: /Export as PDF/ }).click(),
        ]);
        const file = await download.path();
        // `unzip -t` verifies the central directory and every entry's CRC.
        execFileSync('unzip', ['-tq', file]);
        const entries = execFileSync('unzip', ['-Z1', file], { encoding: 'utf8' })
            .split('\n')
            .filter((n) => n.endsWith('.pdf'));
        expect(entries.length > 0, 'ZIP contains no .pdf entry');
        const pdf = execFileSync('unzip', ['-p', file, entries[0]], { maxBuffer: 64 * 1024 * 1024 });
        expect(pdf.subarray(0, 5).toString() === '%PDF-', `${entries[0]} is not a PDF`);
        expect(pdf.length > 1000, `${entries[0]} is too small (${pdf.length} bytes)`);
    });

    // ── Piano sheet ──────────────────────────────────────────────────────────
    const openPianoSheet = async () => {
        await page.getByText('Piano Waltz').first().click();
        await page.getByTitle('Piano sheet').click();
        await page.locator('svg ellipse').first().waitFor({ timeout: 15000 });
    };
    const headCount = () => page.locator('svg ellipse').count();
    let headsBefore = 0;

    await check('the demo piano sheet is drawn as a grand staff with its notes', async () => {
        await page.getByRole('button', { name: '← Library' }).click();
        await page.getByText('My Songs').waitFor();
        await openPianoSheet();
        headsBefore = await headCount();
        // Melody + left hand of the demo piece: well over a dozen note heads.
        expect(headsBefore >= 20, `expected the demo melody on the staff, saw ${headsBefore} heads`);
        expect((await page.locator('svg text', { hasText: '3' }).count()) >= 2, 'time signature 3/4 missing');
    });

    await check('a column can be edited into a chord with another note length and saved', async () => {
        // The first clickable group is the time signature; the next is the first beat column.
        await page.locator('svg g[style*="cursor: pointer"]').nth(1).click();
        await page.getByPlaceholder('e.g. C5,E5,G5').fill('C5,E5,G5');
        await page.getByRole('button', { name: '1/2', exact: true }).click();
        await page.getByRole('button', { name: /Apply/ }).click();
        await page.waitForFunction((n) => document.querySelectorAll('svg ellipse').length === n, headsBefore + 2, { timeout: 15000 });
        await page.getByRole('button', { name: /Save Piano Sheet/ }).click();
        await page.getByText(/Saved/).first().waitFor({ timeout: 15000 });
        await page.getByRole('button', { name: /Save to Library/ }).click();
        await page.getByText(/Saved \(id \d+\)/).waitFor({ timeout: 15000 });
    });

    await check('the edited piano sheet survives a reload', async () => {
        await page.reload();
        await page.getByText('DEMO MODE').waitFor({ timeout: 30000 });
        await page.getByRole('button', { name: 'Log in', exact: true }).first().click();
        await page.getByRole('button', { name: 'Log in as demo user' }).click();
        await page.getByText('My Songs').waitFor();
        await openPianoSheet();
        expect((await headCount()) === headsBefore + 2, 'the edited chord was not saved');
    });

    await check('the piano PDF contains the sheet music with the 3/4 time signature', async () => {
        const [download] = await Promise.all([
            page.waitForEvent('download', { timeout: 30000 }),
            page.getByRole('button', { name: /Export as PDF/ }).click(),
        ]);
        const file = await download.path();
        const entries = execFileSync('unzip', ['-Z1', file], { encoding: 'utf8' }).split('\n').filter(Boolean);
        const pianoEntry = entries.find((n) => /Piano\.pdf$/.test(n));
        expect(pianoEntry, `no piano PDF in the ZIP: ${entries}`);
        const raw = execFileSync('unzip', ['-p', file, pianoEntry], { maxBuffer: 64 * 1024 * 1024 });
        expect(raw.subarray(0, 5).toString() === '%PDF-', 'not a PDF');
        const pdf = pdfContent(raw);
        // Time signature digits are printed as text, once per staff.
        expect((pdf.match(/<33> Tj/g) ?? []).length === 2, 'the 3 of 3/4 should appear once per staff');
        expect((pdf.match(/\nf\n/g) ?? []).length >= 20, 'too few filled note heads in the PDF');
    });

    await check('all songs can be exported as JSON in demo (localStorage) mode', async () => {
        await page.getByRole('button', { name: '← Library' }).click();
        await page.getByText('My Songs').waitFor();
        const [download] = await Promise.all([
            page.waitForEvent('download', { timeout: 15000 }),
            page.getByRole('button', { name: 'Export all songs' }).click(),
        ]);
        const text = await readFile(await download.path(), 'utf8');
        const file = JSON.parse(text);
        const names = file.songs.map((x) => x.name);
        for (const n of ['Sunrise Ballad', 'Midnight Groove', 'Piano Waltz', 'Twelve Bar Blues']) {
            expect(names.includes(n), `export is missing ${n}`);
        }
        // Importing the same file again must not overwrite: every title gets a "(2)" twin.
        await page.setInputFiles('#song-import-input', { name: 'again.json', mimeType: 'application/json', buffer: Buffer.from(text) });
        await page.getByText(/Imported \d+ song\(s\), \d+ renamed because that title already exists\./).waitFor({ timeout: 15000 });
        await page.getByText('Sunrise Ballad (2)').waitFor({ timeout: 15000 });
        await page.getByText('Sunrise Ballad', { exact: true }).waitFor();
    });

    await check('no uncaught page errors', async () => {
        expect(pageErrors.length === 0, pageErrors.join('\n'));
    });
} catch (e) {
    console.error(`FAILED at step "${step}": ${e.message}`);
    await page.screenshot({ path: 'demo-smoke-failure.png', fullPage: true }).catch(() => {});
    await browser.close();
    process.exit(1);
}
await browser.close();
