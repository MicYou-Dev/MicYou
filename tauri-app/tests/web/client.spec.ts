import { expect, test } from '@playwright/test';

test('language selection updates the rendered UI and survives reloads', async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.goto('/');
    await expect(page.getByRole('button', { name: '点击开始连接', exact: true })).toBeVisible();

    await page.getByRole('button', { name: '中文', exact: true }).click();
    await page.getByRole('button', { name: 'English', exact: true }).click();
    await expect(page.getByRole('button', { name: 'Tap to connect', exact: true })).toBeVisible();
    await page.reload();
    await expect(page.getByRole('button', { name: 'Tap to connect', exact: true })).toBeVisible();

    await page.getByRole('button', { name: 'EN', exact: true }).click();
    await page.getByRole('button', { name: '简体中文', exact: true }).click();
    await page.reload();
    await expect(page.getByRole('button', { name: '点击开始连接', exact: true })).toBeVisible();
    expect(errors).toEqual([]);
});

test('help opens and closes without microphone permission or a running audio backend', async ({ page }) => {
    await page.goto('/');
    await page.getByRole('button', { name: '中文', exact: true }).click();
    await page.getByRole('button', { name: 'English', exact: true }).click();
    await page.getByRole('button', { name: 'Help', exact: true }).click();
    await expect(page.getByRole('heading', { name: 'Instructions', exact: true })).toBeVisible();
    await expect(page.locator('.help-overlay')).toHaveCSS('opacity', '1');
    await page.locator('.help-close-btn').click();
    // The existing sheet animates out with opacity rather than display: none.
    await expect(page.locator('.help-overlay')).toHaveCSS('opacity', '0');
    await expect(page.locator('.help-overlay')).toHaveCSS('pointer-events', 'none');
});
