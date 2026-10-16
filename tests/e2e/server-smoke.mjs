// Copyright (c) 2026 APSOS — App and Software Solutions Wörner. All rights reserved.
//
// End-to-end test against the real Axum server (not demo mode): register in the UI,
// save a song, log out, log back in, and find the song again. Also checks that
// nothing is written to the browser's localStorage in server mode.
//
//   BASE_URL=http://localhost:8080 CHROME=/path/to/chrome node tests/e2e/server-smoke.mjs
//
// The server must be running with APP_URL on localhost (users are verified automatically)
// and serving a freshly built frontend.

import { chromium } from 'playwright';

const BASE_URL = process.env.BASE_URL ?? 'http://localhost:8080';
const executablePath = process.env.CHROME || undefined;
const email = `e2e-${Date.now()}@example.com`;
const password = 'Str0ng-Passw0rd!';
const songName = `E2E Song ${Date.now()}`;

const browser = await chromium.launch({ executablePath, args: ['--no-sandbox'] });
const page = await browser.newPage({ viewport: { width: 1200, height: 900 } });
const pageErrors = [];
page.on('pageerror', (e) => pageErrors.push(String(e)));
const apiCalls = [];
page.on('response', (r) => {
    if (r.url().includes('/api/')) apiCalls.push(`${r.request().method()} ${new URL(r.url()).pathname} ${r.status()}`);
});

let step = 'start';
async function check(name, fn) {
    step = name;
    await fn();
    console.log(`ok - ${name}`);
}
function expect(cond, msg) {
    if (!cond) throw new Error(msg);
}

async function logIn() {
    // Landing page: "Log in". After logging out the form may still be in register mode.
    const toggle = page.getByRole('button', { name: /Already have an account/ });
    if (await toggle.count()) {
        await toggle.click();
    } else {
        await page.getByRole('button', { name: 'Log in', exact: true }).first().click();
    }
    await page.getByPlaceholder('Email address').fill(email);
    await page.getByPlaceholder('Password').fill(password);
    await page.getByRole('button', { name: 'Log in', exact: true }).last().click();
    await page.getByText('My Songs').waitFor({ timeout: 15000 });
}

try {
    await check('the app finds the server and shows the landing page', async () => {
        await page.goto(BASE_URL);
        await page.getByRole('button', { name: 'Get started', exact: true }).waitFor({ timeout: 30000 });
        expect(apiCalls.some((c) => c.startsWith('GET /api/songs 401')), `no API probe seen: ${apiCalls}`);
    });

    await check('register creates an account and logs in', async () => {
        await page.getByRole('button', { name: 'Get started', exact: true }).click();
        await page.getByPlaceholder('Email address').fill(email);
        await page.getByPlaceholder('Password').fill(password);
        await page.getByRole('button', { name: 'Create account' }).click();
        await page.getByText('My Songs').waitFor({ timeout: 15000 });
        expect(apiCalls.some((c) => c.startsWith('POST /api/auth/register 201')), `register not called: ${apiCalls}`);
        expect(apiCalls.some((c) => c.startsWith('POST /api/auth/login 200')), `login not called: ${apiCalls}`);
    });

    await check('a new song can be saved to the server', async () => {
        await page.getByTitle('New Song').click();
        await page.getByPlaceholder('Song name').fill(songName);
        await page.getByPlaceholder('Artist').fill('E2E Band');
        await page.getByRole('button', { name: /Save to Library/ }).click();
        await page.getByText(/Saved \(id \d+\)/).waitFor({ timeout: 15000 });
        expect(apiCalls.some((c) => c.startsWith('POST /api/songs 200')), `save not called: ${apiCalls}`);
    });

    await check('after logging out and in again the song is listed and opens', async () => {
        await page.getByRole('button', { name: '← Library' }).click();
        await page.getByRole('button', { name: 'Log out' }).first().click();
        await logIn();
        await page.getByText(songName).waitFor({ timeout: 15000 });
        await page.getByText(songName).first().click();
        await page.waitForFunction(
            (name) => document.querySelector('input[placeholder="Song name"]')?.value === name,
            songName,
            { timeout: 15000 },
        );
        expect(apiCalls.some((c) => /^GET \/api\/songs\/\d+ 200/.test(c)), `load not called: ${apiCalls}`);
    });

    await check('a reload keeps the data on the server (login required again)', async () => {
        await page.reload();
        await page.getByRole('button', { name: 'Get started', exact: true }).waitFor({ timeout: 30000 });
        await logIn();
        await page.getByText(songName).waitFor({ timeout: 15000 });
    });

    await check('server mode does not use localStorage for accounts or songs', async () => {
        const used = await page.evaluate(() => ({
            users: localStorage.getItem('chord_shifter_users'),
            songs: localStorage.getItem('chord_shifter_songs'),
        }));
        expect(used.users === null && used.songs === null, `localStorage used: ${JSON.stringify(used)}`);
    });

    await check('a song can be deleted', async () => {
        await page.getByTitle('Delete').last().click();
        await page.getByText(songName).waitFor({ state: 'detached', timeout: 15000 });
        expect(apiCalls.some((c) => /^DELETE \/api\/songs\/\d+ 204/.test(c)), `delete not called: ${apiCalls}`);
    });

    await check('no uncaught page errors', async () => {
        expect(pageErrors.length === 0, pageErrors.join('\n'));
    });
} catch (e) {
    console.error(`FAILED at step "${step}": ${e.message}`);
    console.error('API calls:', apiCalls.join(' | '));
    await page.screenshot({ path: 'server-smoke-failure.png', fullPage: true }).catch(() => {});
    await browser.close();
    process.exit(1);
}
await browser.close();
