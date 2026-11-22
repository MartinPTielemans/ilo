import numbers from "./github.json";

// ilo on GitHub, as the site shows it: its stars, and the people who have written it, most commits
// first. tools/numbers.mjs asks GitHub for both at each deploy and saves each face beside the pages,
// so a visitor's browser asks GitHub for nothing; what's committed is what the site last knew.

export const STARS: number = numbers.stars;

/** Each contributor's GitHub login, and their face's file in FACES when GitHub gave one. */
export const CONTRIBUTORS: { login: string; face?: string }[] = numbers.contributors;

/** Where the faces are. */
export const FACES = "/media/contributors";

/** How many faces the site shows before it counts the rest. */
export const FACES_SHOWN = 6;
