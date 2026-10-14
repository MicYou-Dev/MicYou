export type ThemeMode = 'system' | 'preset' | 'custom';

export interface HslColor {
  h: number;
  s: number;
  l: number;
}

export interface ThemeManifest {
  id: string;
  name: string;
  version: string;
  author: string;
  description: string;
  preview?: string;
  entry?: string;
  minAppVersion?: string;
  resourceUrl?: string;
  controlsThemeColor?: boolean;
}

export interface ThemePackage extends ThemeManifest {
  tokens?: Record<string, string | number>;
  css?: string;
}

export interface ThemeCatalogIndex {
  version: number;
  themes: ThemeManifest[];
}

export interface ThemeCatalogProvider {
  load(): Promise<ThemeCatalogIndex>;
}
