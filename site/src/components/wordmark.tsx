// ilo's wordmark: a dot, two stems and a ring. The dot takes the colour of the field the page is on.
export function Wordmark({ className, label }: { className?: string; label?: string }) {
  return (
    <svg
      className={className}
      viewBox="7 16 214 154"
      role={label ? "img" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
    >
      <circle className="dot" cx="19" cy="31" r="15" />
      <g fill="currentColor">
        <rect x="7" y="70" width="24" height="100" rx="12" />
        <rect x="67" y="20" width="24" height="150" rx="12" />
      </g>
      <circle cx="171" cy="120" r="38" fill="none" stroke="currentColor" strokeWidth="24" />
    </svg>
  );
}
