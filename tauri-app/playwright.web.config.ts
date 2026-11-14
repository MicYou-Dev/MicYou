import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
    testDir: './tests/web',
    fullyParallel: true,
    forbidOnly: !!process.env.CI,
    retries: process.env.CI ? 1 : 0,
    reporter: 'list',
    use: {
        baseURL: 'http://127.0.0.1:8787',
        trace: 'retain-on-failure',
    },
    projects: [
        { name: 'chromium', use: { ...devices['Desktop Chrome'] } },
        { name: 'webkit', use: { ...devices['iPhone 13'] } },
    ],
    webServer: {
        command: 'node scripts/web-preview.mjs',
        url: 'http://127.0.0.1:8787',
        reuseExistingServer: false,
    },
});
