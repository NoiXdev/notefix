export interface OssLib {
  /** Display name (project / family, not every sub-package). */
  name: string;
  /** SPDX-ish license label. */
  license: string;
  /** Project homepage. */
  url: string;
}

/**
 * Open-source projects Notefix builds on, for the About → Open Source
 * acknowledgements. Curated by family (e.g. all @tiptap/* → "Tiptap") rather
 * than listing every transitive package. Licenses verified against each
 * project's package metadata.
 */
export const OSS_LIBS: OssLib[] = [
  { name: 'React', license: 'MIT', url: 'https://react.dev' },
  { name: 'Tiptap', license: 'MIT', url: 'https://tiptap.dev' },
  { name: 'ProseMirror', license: 'MIT', url: 'https://prosemirror.net' },
  { name: 'Tauri', license: 'MIT / Apache-2.0', url: 'https://tauri.app' },
  { name: 'Tailwind CSS', license: 'MIT', url: 'https://tailwindcss.com' },
  { name: 'dnd kit', license: 'MIT', url: 'https://dndkit.com' },
  { name: 'Font Awesome Free', license: 'CC BY 4.0 / MIT', url: 'https://fontawesome.com' },
  { name: 'highlight.js', license: 'BSD-3-Clause', url: 'https://highlightjs.org' },
  { name: 'lowlight', license: 'MIT', url: 'https://github.com/wooorm/lowlight' },
  { name: 'marked', license: 'MIT', url: 'https://marked.js.org' },
  { name: 'Turndown', license: 'MIT', url: 'https://github.com/mixmark-io/turndown' },
  { name: 'jsPDF', license: 'MIT', url: 'https://github.com/parallax/jsPDF' },
  { name: 'html2canvas-pro', license: 'MIT', url: 'https://github.com/yorickshan/html2canvas-pro' },
  { name: 'i18next', license: 'MIT', url: 'https://www.i18next.com' },
  { name: 'react-select', license: 'MIT', url: 'https://react-select.com' },
  { name: 'react-grid-layout', license: 'MIT', url: 'https://github.com/react-grid-layout/react-grid-layout' },
  { name: 'react-simple-code-editor', license: 'MIT', url: 'https://github.com/react-simple-code-editor/react-simple-code-editor' },
  { name: 'emoji-picker-react', license: 'MIT', url: 'https://github.com/ealush/emoji-picker-react' },
  { name: 'SQLite (rusqlite)', license: 'Public Domain / MIT', url: 'https://www.sqlite.org' },
  { name: 'axum', license: 'MIT', url: 'https://github.com/tokio-rs/axum' },
  { name: 'Tokio', license: 'MIT', url: 'https://tokio.rs' },
  { name: 'reqwest', license: 'MIT / Apache-2.0', url: 'https://github.com/seanmonstar/reqwest' },
  { name: 'Serde', license: 'MIT / Apache-2.0', url: 'https://serde.rs' },
];
