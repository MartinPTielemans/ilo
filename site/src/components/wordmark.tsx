// ilo's wordmark: a dot, two stems and a ring. The dot takes the colour of the Space shown.
export function Wordmark({ label }: { label?: string }) {
  return (
    <svg
      className="wm"
      viewBox="7 16 214 154"
      role={label ? "img" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
    >
      <circle className="d" cx="19" cy="31" r="15" />
      <rect className="s" x="7" y="70" width="24" height="100" rx="12" />
      <rect className="s" x="67" y="20" width="24" height="150" rx="12" />
      <circle className="o" cx="171" cy="120" r="38" />
    </svg>
  );
}
