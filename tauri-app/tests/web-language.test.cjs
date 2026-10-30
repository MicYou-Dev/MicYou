const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { join } = require('node:path');
const { test } = require('node:test');
const vm = require('node:vm');

const html = readFileSync(join(__dirname, '../crates/micyou-core/assets/web_client.html'), 'utf8');
const script = html.match(/<script>\s*([\s\S]*?)<\/script>/)[1];

function loadApp(storage) {
    const context = vm.createContext({
        localStorage: storage,
        Blob,
        URL: { createObjectURL: () => 'blob:test' },
    });
    vm.runInContext(script, context);
    const app = context.micYouApp();
    app.init();
    return app;
}

test('language selection survives a new page instance in both directions', () => {
    const values = new Map();
    const storage = {
        getItem: key => values.get(key) ?? null,
        setItem: (key, value) => values.set(key, value),
    };
    const first = loadApp(storage);
    assert.equal(first.lang, 'zh');
    first.setLang('en');
    const second = loadApp(storage);
    assert.equal(second.lang, 'en');
    second.setLang('zh');
    assert.equal(loadApp(storage).lang, 'zh');
});

test('invalid saved values and unsupported selections are ignored', () => {
    const app = loadApp({
        getItem: () => 'invalid',
        setItem: () => assert.fail('invalid selections must not be saved'),
    });
    assert.equal(app.lang, 'zh');
    app.setLang('invalid');
    assert.equal(app.lang, 'zh');
});

test('blocked storage does not break initialization or language switching', () => {
    const app = loadApp({
        getItem: () => { throw new Error('Storage blocked'); },
        setItem: () => { throw new Error('Quota exceeded'); },
    });
    assert.equal(app.lang, 'zh');
    app.setLang('en');
    assert.equal(app.lang, 'en');
});
