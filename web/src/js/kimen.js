// The Kimen components the site uses, registered once per page. scripts/build.mjs bundles this
// file with esbuild into /assets/kimen.js. Register a component here before a page uses its tag:
// an unregistered ki-* element never upgrades and stays as plain markup.
import { defineCustomElement as defineTopNav } from '@kimen/elements/ki-top-nav';
import { defineCustomElement as defineIconButton } from '@kimen/elements/ki-icon-button';
import { defineCustomElement as defineCodeBlock } from '@kimen/elements/ki-code-block';

defineTopNav();
defineIconButton();
defineCodeBlock();
