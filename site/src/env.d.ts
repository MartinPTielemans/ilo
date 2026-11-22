interface ImportMetaEnv {
  /** The browser repo's commit the live interface in public/demo was built from, from its SOURCE. */
  readonly ILO_DEMO_SOURCE: string;
  /** The versions published on GitHub, when the deploy gives them (isOut in src/changelog.ts). */
  readonly ILO_PUBLISHED?: string;
}
