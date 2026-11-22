import type { ReactNode } from "react";
import { type Here, Shell } from "./shell";
import { Side } from "./side";

/**
 * A page of reading, as the app's Help menu and Settings open them: `title` and `lead` on bare
 * ground, with `top` under them, then the page's sections as panels on the field. `index` lists the
 * sections to keep in reach beside them, each by its id and its name.
 */
export function TextPage({
  here,
  title,
  lead,
  top,
  index,
  fine,
  children,
}: {
  here?: Here;
  title: string;
  lead: ReactNode;
  top?: ReactNode;
  index?: [id: string, name: string][];
  fine?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <Shell here={here} fine={fine}>
      <div className="top">
        <div data-clear="0">
          <h1>{title}</h1>
          <p className="lede">{lead}</p>
        </div>
        {top}
      </div>
      {children && (
        <div className={index ? "cols wide" : "cols alone"}>
          {index && (
            <Side label="On this page">
              {index.map(([id, name]) => (
                <a key={id} href={`#${id}`}>
                  {name}
                </a>
              ))}
            </Side>
          )}
          <div className="text">{children}</div>
        </div>
      )}
    </Shell>
  );
}
