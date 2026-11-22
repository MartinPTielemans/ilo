import { TextPage } from "./text-page";

/**
 * What an address with no page shows. The catch-all route renders it, and the build prerenders that
 * once as /404.html, which Cloudflare serves for any address it has no file for. Its title and
 * noindex are in the component, which React puts in the head.
 */
export function NotFound() {
  return (
    <>
      <title>Nothing here · ilo</title>
      <meta name="robots" content="noindex" />
      <TextPage
        title="Nothing here"
        lead={
          <>
            There's no page at this address. ilo's site is small: try <a href="/">the home page</a>,{" "}
            <a href="/help">Help</a> or <a href="/changelog">Changelog</a>.
          </>
        }
      >
        {null}
      </TextPage>
    </>
  );
}
