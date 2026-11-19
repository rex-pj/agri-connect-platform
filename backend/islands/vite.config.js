// One script with every island, written next to app.js so rust-embed
// serves it under /static with a versioned URL. Deterministic: no hashes in
// the file name, no timestamps, so a rebuild of unchanged sources is
// byte-identical and CI can diff it.
import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
    plugins: [
        svelte({
            compilerOptions: {
                customElement: true,
                // Islands carry no <style>: the page CSP is style-src 'self',
                // so an injected style element would be blocked. Styles live
                // in app.css.
                css: 'external',
            },
        }),
    ],
    build: {
        outDir: '../crates/web/static/js',
        emptyOutDir: false,
        minify: true,
        sourcemap: false,
        lib: {
            entry: 'src/main.js',
            // IIFE, not ES: Vite does not minify whitespace in ES library
            // builds, and a plain deferred <script> is all a page needs.
            formats: ['iife'],
            name: 'AgriConnectIslands',
            fileName: () => 'islands.js',
        },
    },
});
