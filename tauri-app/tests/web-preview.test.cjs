const assert = require('node:assert/strict');
const { test } = require('node:test');

test('preview serves the actual page and Alpine, but no repository files or audio socket', async t => {
    const { createWebPreviewServer } = await import('../scripts/web-preview.mjs');
    const server = createWebPreviewServer();
    await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
    t.after(() => new Promise(resolve => server.close(resolve)));
    const base = `http://127.0.0.1:${server.address().port}`;

    const page = await fetch(base);
    assert.equal(page.status, 200);
    assert.match(page.headers.get('content-type'), /text\/html/);
    assert.match(await page.text(), /function micYouApp\(\)/);
    const alpine = await fetch(`${base}/alpine.min.js`);
    assert.equal(alpine.status, 200);
    assert.match(alpine.headers.get('content-type'), /javascript/);
    assert.equal((await fetch(`${base}/package.json`)).status, 404);
    assert.equal((await fetch(`${base}/ws`)).status, 404);
    assert.equal((await fetch(base, { method: 'POST' })).status, 404);
});
