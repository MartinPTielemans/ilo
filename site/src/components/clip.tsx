import { CLIPS } from "../changelog";

/**
 * A changelog clip, as the page gives it: the still, and a video with nothing to fetch yet. The
 * page's script (src/reading.ts) plays it from `data-clip` while it's on screen, and names the
 * button for what a press does.
 */
export function Clip({ name, label }: { name: string; label: string }) {
  return (
    <button
      type="button"
      className="clip"
      data-clip={`${CLIPS}/${name}.mp4`}
      data-label={label}
      aria-label={`Play: ${label}`}
    >
      <img src={`${CLIPS}/${name}-still.webp`} alt="" loading="lazy" decoding="async" />
      <video
        poster={`${CLIPS}/${name}.webp`}
        muted
        autoPlay
        loop
        playsInline
        preload="none"
        aria-hidden
      />
    </button>
  );
}
