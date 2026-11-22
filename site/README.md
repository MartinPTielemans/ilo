# ilobrowser.com

The website for ilo, an open-source browser, in the browser's repository.

It's [TanStack Start](https://tanstack.com/start) on [Vite+](https://viteplus.dev/guide), with
every page prerendered to static HTML. With Vite+'s `vp` installed:

```sh
vp install
vp dev      # the site with hot reload
vp check    # formatting, lint and types
vp test
vp build    # static files in dist/client
```

It's served from Cloudflare at [ilobrowser.com](https://ilobrowser.com). A push to main that
changes `site/` deploys there, and so does publishing a release of ilo, which brings its changelog
entry (`.github/workflows/site.yml`). The secrets and the switch it needs are in
`docs/releasing.md`.

The site's code is MIT-licensed (see `LICENSE`). The ilo name and logo are trademarks and aren't
covered by that license.
