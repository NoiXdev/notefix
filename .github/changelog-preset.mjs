// Section mapping for the conventional-changelog "conventionalcommits" preset.
// Each visible commit type becomes a section in CHANGELOG.md; hidden ones are
// parsed (for version bumps) but kept out of the changelog.
//
// As of conventional-changelog-conventionalcommits v8+ this package is ESM-only
// and exposes `createPreset` as its default export, so this file must be ESM
// (.mjs) and export the resolved preset config as its default export — which is
// what conventional-changelog-cli v5+ loads via dynamic import().
import createPreset from 'conventional-changelog-conventionalcommits';

export default createPreset({
  types: [
    { type: 'feat', section: 'Features' },
    { type: 'fix', section: 'Bug Fixes' },
    { type: 'perf', section: 'Performance' },
    { type: 'refactor', section: 'Refactor' },
    { type: 'docs', section: 'Documentation' },
    { type: 'style', section: 'Styling' },
    { type: 'test', section: 'Testing' },
    { type: 'build', section: 'Build System' },
    { type: 'ci', section: 'CI/CD' },
    { type: 'revert', section: 'Reverts' },
    { type: 'chore', hidden: true },
  ],
});
