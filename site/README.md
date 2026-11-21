# ilo-site

The website for [ilo](https://github.com/MartinPTielemans/ilo), an open-source browser.

It's [TanStack Start](https://tanstack.com/start) on [Vite+](https://viteplus.dev/guide), with
every page prerendered to static HTML. With Vite+'s `vp` installed:

```sh
vp install
vp dev      # the site with hot reload
vp check    # formatting, lint and types
vp test
vp build    # static files in dist/client
```

It's served from Cloudflare at [ilobrowser.com](https://ilobrowser.com). Each push to main deploys
there (`.github/workflows/deploy.yml`), and each release of ilo puts its live demo on the site
(`.github/workflows/release.yml`). They need the repository secrets `CLOUDFLARE_API_TOKEN`, a token
with Workers Scripts Edit on the account, and `CLOUDFLARE_ACCOUNT_ID`. By hand: `vp build`, then
`wrangler deploy`.

The site's code is MIT-licensed (see `LICENSE`). The ilo name and logo are trademarks and aren't
covered by that license.
