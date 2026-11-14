import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const assets = new URL('../crates/micyou-core/assets/', import.meta.url);
const routes = new Map([
    ['/', ['web_client.html', 'text/html; charset=utf-8']],
    ['/index.html', ['web_client.html', 'text/html; charset=utf-8']],
    ['/alpine.min.js', ['alpine.min.js', 'text/javascript; charset=utf-8']],
]);

// Serve only the web client's two assets, never the repository or user files.
export function createWebPreviewServer() {
    return createServer(async (request, response) => {
        const route = routes.get(new URL(request.url, 'http://localhost').pathname);
        if (!route || !['GET', 'HEAD'].includes(request.method)) {
            response.writeHead(404).end();
            return;
        }
        try {
            const body = await readFile(new URL(route[0], assets));
            response.writeHead(200, {
                'Content-Type': route[1],
                'Cache-Control': 'no-store',
                'Content-Length': body.length,
            });
            response.end(request.method === 'HEAD' ? undefined : body);
        } catch (error) {
            console.error(error);
            response.writeHead(500).end();
        }
    });
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
    const server = createWebPreviewServer();
    server.on('error', error => {
        console.error(`Web preview failed: ${error.message}`);
        process.exitCode = 1;
    });
    server.listen(8787, '127.0.0.1', () => {
        console.log('Web UI preview: http://127.0.0.1:8787 (no audio server)');
    });
}
