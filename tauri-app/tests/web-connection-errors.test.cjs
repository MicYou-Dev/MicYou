const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { join } = require('node:path');
const { test } = require('node:test');
const vm = require('node:vm');

const html = readFileSync(join(__dirname, '../crates/micyou-core/assets/web_client.html'), 'utf8');
const script = html.match(/<script>\s*([\s\S]*?)<\/script>/)[1];

function loadApp(overrides = {}) {
    const resources = { stopped: 0, closed: 0, sockets: [] };
    const stream = { getTracks: () => [{ stop: () => resources.stopped++ }] };
    class AudioContext {
        audioWorklet = { addModule: async () => {} };
        createMediaStreamSource() { return { connect() {} }; }
        close() { resources.closed++; }
    }
    class WebSocket {
        constructor() { resources.sockets.push(this); }
        close() { this.onclose?.(); }
    }
    const context = vm.createContext({
        Blob, AbortController,
        URL: { createObjectURL: () => 'blob:test' },
        location: { origin: 'https://127.0.0.1:8443', protocol: 'https:', host: '127.0.0.1:8443' },
        navigator: { mediaDevices: { getUserMedia: async () => stream } },
        fetch: async () => ({ ok: true, status: 200 }),
        window: { AudioContext },
        AudioWorkletNode: class {
            port = {};
            connect() {}
            disconnect() {}
        },
        WebSocket,
        setTimeout: () => 1,
        clearTimeout() {},
        console: { error() {} },
        ...overrides,
    });
    vm.runInContext(script, context);
    const app = context.micYouApp();
    app.lang = 'en';
    return { app, resources, stream };
}

test('origin rejection is diagnosed before asking for the microphone', async () => {
    const { app } = loadApp({
        fetch: async () => ({ status: 403, ok: false, text: async () => 'Invalid origin' }),
        navigator: { mediaDevices: { getUserMedia: () => assert.fail('must not request microphone') } },
    });
    await app.startStreaming();
    assert.equal(app.streamState, 'error');
    assert.match(app.errorMessage, /rejected this page address/);
    assert.doesNotMatch(app.statusLabel, /permission/);
});

test('unavailable server and unrelated HTTP rejection are not misreported as microphone errors', async () => {
    for (const fetch of [
        async () => { throw new TypeError('Failed to fetch'); },
        async () => ({ status: 502, ok: false }),
        async () => ({ status: 403, ok: false, text: async () => 'Proxy authentication required' }),
    ]) {
        const { app } = loadApp({ fetch });
        await app.startStreaming();
        assert.equal(app.streamState, 'error');
        assert.match(app.errorMessage, /Cannot reach the PC Web service/);
    }
});

test('microphone failures give specific permission, missing-device and busy-device advice', async () => {
    for (const [name, expected] of [
        ['NotAllowedError', /access was blocked/],
        ['NotFoundError', /No microphone was found/],
        ['NotReadableError', /Cannot read the microphone/],
    ]) {
        const { app } = loadApp({ navigator: { mediaDevices: { getUserMedia: async () => { throw { name }; } } } });
        await app.startStreaming();
        assert.equal(app.streamState, 'error');
        assert.match(app.errorMessage, expected);
    }
});

test('WebSocket failure releases audio resources and preserves the visible error', async () => {
    const { app, resources } = loadApp();
    await app.startStreaming();
    const socket = resources.sockets[0];
    socket.onerror();
    assert.equal(resources.stopped, 1);
    assert.equal(resources.closed, 1);
    assert.equal(app.ws, null);
    assert.equal(socket.onclose, null);
    assert.equal(app.streamState, 'error');
    assert.match(app.errorMessage, /proxy WebSocket forwarding/);
});

test('canceling microphone acquisition stops a late stream instead of connecting', async () => {
    let resolveMic;
    let acquired;
    const acquiring = new Promise(resolve => { acquired = resolve; });
    const { app, resources, stream } = loadApp({
        navigator: { mediaDevices: { getUserMedia: () => new Promise(resolve => { resolveMic = resolve; acquired(); }) } },
    });
    const starting = app.startStreaming();
    await acquiring;
    app.stopStreaming();
    resolveMic(stream);
    await starting;
    assert.equal(resources.stopped, 1);
    assert.equal(resources.sockets.length, 0);
    assert.equal(app.streamState, 'idle');
});

test('canceling while a forbidden response is being read does not restart acquisition', async () => {
    let resolveBody;
    let reading;
    const bodyStarted = new Promise(resolve => { reading = resolve; });
    const { app } = loadApp({
        fetch: async () => ({
            status: 403, ok: false,
            text: () => new Promise(resolve => { resolveBody = resolve; reading(); }),
        }),
        navigator: { mediaDevices: { getUserMedia: () => assert.fail('canceled attempt must not acquire microphone') } },
    });
    const starting = app.startStreaming();
    await bodyStarted;
    app.stopStreaming();
    resolveBody('Invalid origin');
    await starting;
    assert.equal(app.streamState, 'idle');
    assert.equal(app.errorMessage, '');
});
