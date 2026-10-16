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

const BASE_URL = process.env.BASE_URL ?? 'http://localhost:8080';
const executablePath = process.env.CHROME || undefined;

const browser = await chromium.launch({ executablePath, args: ['--no-sandbox'] });
const page = await browser.newPage({ viewport: { width: 1200, height: 900 } });
const pageErrors = [];
page.on('pageerror', (e) => pageErrors.push(String(e)));

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
