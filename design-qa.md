# Design QA

Date: 2026-07-22

## Scope and sources

- Source reference: `/tmp/codexmanager-issue126-reference.png` (GitHub issue #126).
- Implementation: `apps/src/app/skills/page.tsx`, `apps/src/app/skills/marketplace-dialog.tsx`, `apps/src/components/modals/add-account-modal.tsx`, and the shared application shell.
- Combined comparison: `/tmp/codexmanager-design-qa.9mgegn/issue126-reference-comparison.png`.
- Visual browser and screenshot source: Firefox 152 with geckodriver 0.36. The repository's separate Playwright navigation regression also passed, but it was not used as visual evidence.

## Viewports and evidence

- Desktop: 1440 x 900.
  - Skills page: `/tmp/codexmanager-design-qa.9mgegn/skills-page-final-desktop.png`.
  - Marketplace: `/tmp/codexmanager-design-qa.9mgegn/skills-marketplace-final-desktop-top.png`.
  - Install confirmation: `/tmp/codexmanager-design-qa.9mgegn/skills-marketplace-confirm-final-desktop.png`.
  - Device Code login: `/tmp/codexmanager-design-qa.9mgegn/device-code-modal-final-desktop.png`.
- Narrow viewport: true 390 x 844 set through Firefox BiDi and confirmed with `window.innerWidth`.
  - Skills page: `/tmp/codexmanager-design-qa.9mgegn/skills-page-final-390.png`.
  - Marketplace: `/tmp/codexmanager-design-qa.9mgegn/skills-marketplace-final-390.png`.
  - Install confirmation: `/tmp/codexmanager-design-qa.9mgegn/skills-marketplace-confirm-final-390.png`.
  - Device Code login: `/tmp/codexmanager-design-qa.9mgegn/device-code-modal-final-390.png`.
- No horizontal document overflow was present at either viewport. At 390 px, the shared sidebar now collapses automatically while remaining manually expandable.

## Interaction and console checks

- Loaded a real Codex Marketplace and displayed three compatible plugins from `openai/role-specific-plugins`.
- Searched plugins and inspected the complete Skill lists, real GitHub source, installed state, version, author, and category metadata.
- Installed the complete `product-design` plugin through the UI and confirmed the resulting success state.
- Opened the nested install confirmation; Escape closed only the confirmation and preserved the Marketplace dialog.
- Started a real Device Code login, received a user code and verification URL, then closed the dialog to cancel the pending login.
- Confirmed the Device Code selector renders localized labels instead of raw protocol values.
- Firefox BiDi console capture after a fresh Skills load reported zero warnings and zero errors.

## Visual review

- Typography: uses the existing CodexManager system-font and monospace hierarchy; long descriptions, versions, authors, and URLs truncate or wrap without covering controls.
- Spacing: desktop remains dense and operational; cards and dialog sections have consistent gaps. At 390 px, actions stack cleanly and both dialogs remain fully usable.
- Color and contrast: existing theme tokens, borders, muted text, primary actions, installed badges, and destructive actions remain visually distinct in the tested theme. This is a visual risk check, not a full WCAG contrast certification.
- Images and icons: the existing CodexManager logo and Lucide icon system render correctly; no missing or stretched assets were observed.
- Copy: installation scope, GitHub source, complete-plugin warning, host filesystem location, read-only system Skills, and Device Code expiry guidance are explicit. The implementation intentionally supports Codex Skills/plugins only, rather than the reference image's multi-client providers.
- Reference comparison: the implementation preserves the reference's core installed-Skills management goal while following CodexManager's existing shell, glass surfaces, compact controls, and security boundaries.

final result: passed
