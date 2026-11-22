import { useEffect, useRef, useState } from "react";
import { scroller } from "../live";

/**
 * A clip of ilo from /media/cuts: its still, and over it the film, silent and looping while it's on
 * screen and fetched only when it nears it. A visitor who prefers reduced motion sees the still
 * until they press it; a press stops or plays it for anyone. `label` says what it shows.
 */
export function Reel({
  name,
  width,
  height,
  label,
}: {
  name: string;
  width: number;
  height: number;
  label: string;
}) {
  const reel = useRef<HTMLButtonElement>(null);
  const film = useRef<HTMLVideoElement>(null);
  // What the visitor chose by pressing it; until then, motion follows their preference.
  const [chosen, setChosen] = useState<boolean>();
  const [reduced, setReduced] = useState(false);
  const [near, setNear] = useState(false);
  // Whether the film has been asked for: from then on the video has its address.
  const [fetched, setFetched] = useState(false);
  const moving = chosen ?? !reduced;

  useEffect(() => {
    const reduce = matchMedia("(prefers-reduced-motion: reduce)");
    const update = () => setReduced(reduce.matches);
    update();
    reduce.addEventListener("change", update);
    return () => reduce.removeEventListener("change", update);
  }, []);

  useEffect(() => {
    if (!reel.current) return;
    const watch = new IntersectionObserver(([entry]) => setNear(!!entry?.isIntersecting), {
      root: scroller(),
      rootMargin: "12% 0px",
    });
    watch.observe(reel.current);
    return () => watch.disconnect();
  }, []);

  useEffect(() => {
    const video = film.current;
    if (!video) return;
    if (!moving || !near) return video.pause();
    if (!fetched) return setFetched(true);
    video.muted = true;
    // A browser that won't play it leaves the still.
    video.play().catch(() => {});
  }, [moving, near, fetched]);

  return (
    <button
      ref={reel}
      type="button"
      className="reel"
      data-stopped={moving ? undefined : ""}
      aria-label={`${moving ? "Pause" : "Play"}: ${label}`}
      onClick={() => setChosen(!moving)}
    >
      <img
        src={`/media/cuts/${name}.webp`}
        width={width}
        height={height}
        alt=""
        loading="lazy"
        decoding="async"
      />
      <video
        ref={film}
        src={fetched ? `/media/cuts/${name}.mp4` : undefined}
        muted
        loop
        playsInline
        preload="none"
        aria-hidden
      />
    </button>
  );
}
